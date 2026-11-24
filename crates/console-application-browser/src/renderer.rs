#![forbid(unsafe_code)]

use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use console_browser_renderer::{self as channel, Packet};
use console_core_never::Never;
use console_core_web_engine::{Request, encoded_request};
use console_operating_system_linux::program_lifetime::{Forked, forked};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{OFlags, fcntl_setfl};
use rustix::net::{AddressFamily, SocketFlags, SocketType, socketpair};

use crate::downloads::{self, Staging};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Renderer {
  #[default]
  WebKit,
  Chromium,
  Servo,
}

impl Renderer {
  #[cfg(test)]
  pub const ALL: [Self; 3] = [Self::WebKit, Self::Chromium, Self::Servo];

  pub fn label(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::WebKit => "WebKit",
      Self::Chromium => "Chromium",
      Self::Servo => "Servo",
    })
  }
}

impl std::fmt::Display for Renderer {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let Ok(label) = self.label();

    to.write_str(label)
  }
}

pub struct Worker {
  pub channel: OwnedFd,
  engine: Engine,
  _downloads: Option<Staging>,
}

enum Engine {
  Dedicated { _process: Forked },
  Shared(PoolId),
}

struct TemporaryProfile(PathBuf);

impl Drop for TemporaryProfile {
  fn drop(&mut self) {
    {
      match std::fs::remove_dir_all(&self.0) {
        Err(fault) => {
          console_core_logging::warn!(
            "removing temporary Chromium directory {}: {fault}",
            self.0.display()
          );
        }
        ::core::result::Result::Ok(_) => {}
      };
    }
  }
}

#[derive(Default)]
pub(crate) struct PrivateChromium {
  pool: Option<Pool>,
  next_id: u64,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct PoolId(u64);

struct Pool {
  id: PoolId,
  control: OwnedFd,
  _process: Forked,
  _profile: TemporaryProfile,
}

enum PoolConnection {
  Live,
  Disconnected,
}

impl Pool {
  fn start(executable: &Path, id: PoolId) -> io::Result<Self> {
    let directory = console_core_temporary_directories::private("browser-chromium")
      .map_err(io::Error::other)?;
    let profile = TemporaryProfile(directory);
    let (control, peer) = engine_channel()?;
    let mut command = Command::new(executable);
    command
      .arg("--chromium-pool")
      .arg(&profile.0)
      .stdin(Stdio::from(peer));
    let process = forked(&mut command)?;
    Ok(Self {
      id,
      control,
      _process: process,
      _profile: profile,
    })
  }

  fn connection(&self) -> io::Result<PoolConnection> {
    let mut watched = [PollFd::new(&self.control, PollFlags::RDHUP)];
    poll(&mut watched, Some(&Timespec::default()))?;
    let [ready] = watched;
    Ok(
      match bool::then_some(
        ready
          .revents()
          .intersects(PollFlags::HUP | PollFlags::RDHUP | PollFlags::ERR),
        (),
      ) {
        Some(()) => PoolConnection::Disconnected,
        None => PoolConnection::Live,
      },
    )
  }

  fn tab(&self, downloads: Option<Staging>) -> io::Result<Worker> {
    let (channel, peer) = engine_channel()?;
    let directory = match downloads
      .as_ref()
      .and_then(|staging| staging.as_ref().to_str())
    {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => "",
    };
    channel::send(
      self.control.as_fd(),
      &format!("tab {directory}"),
      Some(&peer),
    )?;
    Ok(Worker {
      channel,
      engine: Engine::Shared(self.id),
      _downloads: downloads,
    })
  }
}

impl PrivateChromium {
  pub(crate) fn tab(&mut self, executable: &Path) -> io::Result<Worker> {
    let available = match self.pool.as_ref() {
      Some(pool) => {
        let connection = pool.connection()?;

        match connection {
          PoolConnection::Live => Some(pool),
          PoolConnection::Disconnected => None,
        }
      }
      None => None,
    };

    match available {
      Some(pool) => {
        let Ok(staging) = download_staging();

        match pool.tab(staging) {
          Ok(tab) => return Ok(tab),
          Err(fault) => match matches!(
            fault.kind(),
            io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
          )
          .then_some(())
          {
            Some(()) => {}
            None => return Err(fault),
          },
        }
      }
      None => {}
    }

    let id = PoolId(self.next_id);
    let next_id = self
      .next_id
      .checked_add(1)
      .ok_or_else(|| io::Error::other("private Chromium pool numbers exhausted"))?;
    let pool = Pool::start(executable, id)?;
    let Ok(staging) = download_staging();
    let tab = pool.tab(staging)?;
    self.next_id = next_id;
    self.pool = Some(pool);
    Ok(tab)
  }

  pub(crate) fn retain<'a>(
    &mut self,
    mut workers: impl Iterator<Item = &'a Worker>,
  ) -> Result<(), Never> {
    let id = match self.pool.as_ref() {
      Some(pool) => pool.id,
      None => return Ok(()),
    };
    let active = workers.find_map(|worker| match worker.engine {
      Engine::Shared(used) => (used == id).then_some(()),
      Engine::Dedicated { .. } => None,
    });

    match active {
      Some(()) => {}
      None => self.pool = None,
    }

    Ok(())
  }
}

fn download_staging() -> Result<Option<Staging>, Never> {
  Ok(
    match downloads::folder().and_then(|root| Staging::new(&root)) {
      Ok(staging) => Some(staging),
      Err(fault) => {
        console_core_logging::warn!("browser downloads are unavailable: {fault}");
        None
      }
    },
  )
}

fn engine_channel() -> io::Result<(OwnedFd, OwnedFd)> {
  let (channel, peer) = socketpair(
    AddressFamily::UNIX,
    SocketType::SEQPACKET,
    SocketFlags::CLOEXEC,
    None,
  )?;
  fcntl_setfl(&channel, OFlags::NONBLOCK)?;
  Ok((channel, peer))
}

impl Worker {
  pub fn start(executable: &Path) -> io::Result<Self> {
    let (channel, peer) = engine_channel()?;
    let mut command = Command::new(executable);
    command.env("TZ", "UTC").stdin(Stdio::from(peer));
    let process = forked(&mut command)?;

    Ok(Self {
      channel,
      engine: Engine::Dedicated { _process: process },
      _downloads: None,
    })
  }

  pub fn request(&self, request: &Request) -> io::Result<()> {
    let Ok(line) = encoded_request(request);
    channel::send(self.channel.as_fd(), &line, None)
  }

  pub fn receive(&self) -> io::Result<Packet> {
    channel::receive(self.channel.as_fd())
  }

  pub fn release_frame(&self) -> io::Result<()> {
    channel::send(self.channel.as_fd(), channel::RELEASED, None)
  }
}
