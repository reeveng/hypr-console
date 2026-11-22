#![forbid(unsafe_code)]

use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use console_browser_renderer::{self as channel, Packet};
use console_core_external_programs::{Installed, Program, installed};
use console_core_never::Never;
use console_core_web_engine::{Request, encoded_request};
use console_operating_system_linux::program_lifetime::{Forked, forked};
use rustix::fs::{OFlags, fcntl_setfl};
use rustix::net::{AddressFamily, SocketFlags, SocketType, socketpair};

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

struct Profile(PathBuf);

impl Drop for Profile {
  fn drop(&mut self) {
    match std::fs::remove_dir_all(&self.0) {
      Ok(()) => {}
      Err(fault) => {
        console_core_logging::warn!("removing browser profile {}: {fault}", self.0.display())
      }
    }
  }
}

pub struct Worker {
  pub channel: OwnedFd,
  _process: Forked,
  _profile: Option<Profile>,
}

impl Worker {
  pub fn start(renderer: Renderer, executable: &Path) -> io::Result<Self> {
    let (mut command, profile) = match renderer {
      Renderer::Chromium => {
        let Ok(name) = Program::Chromium.name();
        let Ok(chromium) = installed(name);

        match chromium {
          Installed::No => {
            return Err(io::Error::new(
              io::ErrorKind::NotFound,
              "Chromium is not installed",
            ));
          }
          Installed::Yes => {}
        }

        let directory = console_core_temporary_directories::private("browser-chromium")
          .map_err(io::Error::other)?;
        let profile = Profile(directory);
        let mut command = Command::new(executable);
        command.arg(crate::chromium::WORKER).arg(&profile.0);
        (command, Some(profile))
      }
      Renderer::Servo | Renderer::WebKit => (Command::new(executable), None),
    };
    let (channel, peer) = socketpair(
      AddressFamily::UNIX,
      SocketType::SEQPACKET,
      SocketFlags::CLOEXEC,
      None,
    )?;
    fcntl_setfl(&channel, OFlags::NONBLOCK)?;
    command.stdin(Stdio::from(peer));
    let process = forked(&mut command)?;

    Ok(Self {
      channel,
      _process: process,
      _profile: profile,
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
