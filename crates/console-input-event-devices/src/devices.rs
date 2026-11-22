//! Every event node, and the ones plugged in after the program started.
//!
//! At boot a pad can be enumerated after the unit that wants it, so
//! `/dev/input` is watched and a node that appears is opened; one that stops
//! answering is dropped. What waits is `poll` over all of them and over one
//! more descriptor the caller hands in, which is how the greeter hears the
//! login window and a press in the same wait. Nothing a person does arrives on
//! a schedule, so a wait with no patience lasts until something is pressed; a
//! caller drawing something that moves on its own -- the greeter's clock --
//! hands in how long until it moves, and a wait that runs out wakes with
//! nothing in it. A node that is a touchscreen is asked its axes when it is
//! opened, and a finger on it wakes the wait the way a press does.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::fd::BorrowedFd;
use std::path::{Path, PathBuf};
use std::time::Duration;

use console_core_iteration::Step;
use console_core_logging::warn;
use console_core_never::Never;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::inotify::{self, CreateFlags, WatchFlags};
use rustix::fs::{Mode, OFlags};

use crate::device::{Device, INPUT, NODE};
use crate::event::EMPTY_BATCH;
use crate::presses::{ButtonPress, decode_presses};
use crate::touches::{ScreenTouch, Touchscreen};

struct Node {
  path: PathBuf,
  file: File,
  screen: Option<Touchscreen>,
}

pub struct Devices {
  told: File,
  held: Vec<Node>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ready {
  Yes,
  No,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Woke {
  pub presses: Vec<ButtonPress>,
  pub touches: Vec<ScreenTouch>,
  pub also: Ready,
}

impl Devices {
  pub fn open() -> io::Result<Devices> {
    let notification = inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK)?;
    inotify::add_watch(
      &notification,
      INPUT,
      WatchFlags::CREATE | WatchFlags::ATTRIB,
    )?;
    let told = File::from(notification);

    let mut devices = Devices {
      told,
      held: Vec::new(),
    };
    let Ok(()) = devices.open_all();

    Ok(devices)
  }

  fn open_all(&mut self) -> Result<(), Never> {
    let entries = match fs::read_dir(INPUT) {
      Ok(entries) => entries,
      Err(why) => {
        warn!("cannot list {INPUT}: {why}");

        return Ok(());
      }
    };
    let known: HashSet<PathBuf> = self.held.iter().map(|device| device.path.clone()).collect();

    for entry in entries.flatten() {
      let path = entry.path();
      let node = entry.file_name().as_encoded_bytes().starts_with(NODE);
      let known_2 = known.contains(&path);

      match (node, known_2) {
        (true, false) => match rustix::fs::open(
          &path,
          OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
          Mode::empty(),
        ) {
          Ok(opened) => {
            let Ok(screen) = screen(&path);
            let file = File::from(opened);

            self.held.push(Node { path, file, screen });
          }
          Err(why) => warn!("cannot open {}: {why}", path.display()),
        },
        (true, true) | (false, _) => {}
      }
    }

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "Devices::open creates the inotify descriptor with NONBLOCK, so draining an empty queue returns immediately"
    )
  )]
  fn drain(&mut self) -> Result<(), Never> {
    let _drained =
      console_core_iteration::iterate((&mut self.told, [0_u8; 4096]), |(told, mut buffer)| {
        Ok(match told.read(&mut buffer) {
          Ok(0) => Step::Halt(()),
          Err(_the_read_failed) => Step::Halt(()),
          Ok(_) => Step::Again((told, buffer)),
        })
      });

    Ok(())
  }

  pub fn wait(
    &mut self,
    also: Option<BorrowedFd<'_>>,
    patience: Option<Duration>,
  ) -> io::Result<Woke> {
    let ended = console_core_iteration::iterate(self, |devices| {
      Ok(match devices.waited(also, patience) {
        Ok(Some(woke)) => Step::Halt(Ok(woke)),
        Ok(None) => Step::Again(devices),
        Err(fault) => Step::Halt(Err(fault)),
      })
    });

    match ended {
      Ok(woke) => woke,
      Err(endless) => Err(io::Error::other(endless)),
    }
  }

  fn waited(
    &mut self,
    also: Option<BorrowedFd<'_>>,
    patience: Option<Duration>,
  ) -> io::Result<Option<Woke>> {
    let (timed_out, plugged, also_ready, readable) = {
      let mut waiting = vec![PollFd::new(&self.told, PollFlags::IN)];
      waiting.extend(also.map(|fd| PollFd::from_borrowed_fd(fd, PollFlags::IN)));
      waiting.extend(
        self
          .held
          .iter()
          .map(|device| PollFd::new(&device.file, PollFlags::IN)),
      );
      let timeout = patience
        .map(|duration| {
          let milliseconds = duration
            .as_millis()
            .min(u128::from(i32::MAX.unsigned_abs()));
          let Ok(milliseconds_2) = console_core_number_conversion::fitted::<_, u64>(milliseconds);
          Timespec::try_from(Duration::from_millis(milliseconds_2))
        })
        .transpose()
        .map_err(io::Error::other)?;
      let timed_out = poll(&mut waiting, timeout.as_ref()).map(|ready| ready == 0)?;
      let mut waiting_2 = waiting.into_iter();
      let plugged = match ::core::primitive::bool::then_some(
        waiting_2.next().is_some_and(|fd| !fd.revents().is_empty()),
        (),
      ) {
        ::core::option::Option::Some(()) => Ready::Yes,
        ::core::option::Option::None => Ready::No,
      };
      let also_ready = match also {
        ::core::option::Option::Some(_) => match ::core::primitive::bool::then_some(
          waiting_2.next().is_some_and(|fd| !fd.revents().is_empty()),
          (),
        ) {
          ::core::option::Option::Some(()) => Ready::Yes,
          ::core::option::Option::None => Ready::No,
        },
        ::core::option::Option::None => Ready::No,
      };
      let readable = (0..)
        .zip(waiting_2)
        .filter_map(|(index, fd)| (!fd.revents().is_empty()).then_some(index))
        .collect();

      (timed_out, plugged, also_ready, readable)
    };

    match (timed_out, patience) {
      (true, Some(_)) => {
        return Ok(Some(Woke {
          presses: Vec::new(),
          touches: Vec::new(),
          also: Ready::No,
        }));
      }
      (false, _) | (true, None) => {}
    }

    let Ok((presses, touches)) = self.read(&readable);

    match plugged {
      Ready::Yes => {
        let Ok(()) = self.drain();
        let Ok(()) = self.open_all();
      }
      Ready::No => {}
    }

    Ok(match (presses.is_empty(), touches.is_empty(), also_ready) {
      (true, true, Ready::No) => None,
      (false, _, _) | (_, false, _) | (true, true, Ready::Yes) => Some(Woke {
        presses,
        touches,
        also: also_ready,
      }),
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "the hotplug reader opens every Node with O_NONBLOCK, so a readiness race cannot block read"
    )
  )]
  fn read(
    &mut self,
    readable: &HashSet<u32>,
  ) -> Result<(Vec<ButtonPress>, Vec<ScreenTouch>), Never> {
    let mut presses = Vec::new();
    let mut touches = Vec::new();
    let mut kept = Vec::new();
    let mut buffer = EMPTY_BATCH;

    for (at, mut device) in (0..).zip(std::mem::take(&mut self.held)) {
      match readable.contains(&at).then_some(()) {
        Some(()) => {}
        None => {
          kept.push(device);
          continue;
        }
      }

      let report = match device.file.read(&mut buffer) {
        Ok(0) => continue,
        Ok(count) => buffer.get(..count),
        Err(why)
          if matches!(
            why.kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
          ) =>
        {
          kept.push(device);
          continue;
        }
        Err(why) => {
          warn!("{} went away: {why}", device.path.display());
          continue;
        }
      };
      let bytes = match report {
        Some(bytes) => bytes,
        None => {
          warn!(
            "{} returned more bytes than its input buffer",
            device.path.display()
          );
          continue;
        }
      };
      let Ok(heard) = decode_presses(bytes);
      presses.extend(heard);

      match device.screen.as_mut() {
        Some(screen) => {
          let Ok(felt) = screen.decode(bytes);
          touches.extend(felt);
        }
        None => {}
      }

      kept.push(device);
    }

    self.held = kept;

    Ok((presses, touches))
  }
}

fn screen(path: &Path) -> Result<Option<Touchscreen>, Never> {
  match Device::open(path) {
    Ok(device) => Touchscreen::of(&device),
    Err(why) => {
      warn!("cannot ask {} what it is: {why}", path.display());

      Ok(None)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use rustix::pipe::{PipeFlags, pipe_with};

  #[test]
  fn a_readiness_race_keeps_the_node_until_it_reaches_eof() -> io::Result<()> {
    let flags = PipeFlags::CLOEXEC | PipeFlags::NONBLOCK;
    let (notified, _notifying) = pipe_with(flags)?;
    let (read_end, write_end) = pipe_with(flags)?;
    let mut devices = Devices {
      told: File::from(notified),
      held: vec![Node {
        path: PathBuf::from("event0"),
        file: File::from(read_end),
        screen: None,
      }],
    };
    let readable = HashSet::from([0]);

    let Ok((presses, touches)) = devices.read(&readable);
    assert!(presses.is_empty());
    assert!(touches.is_empty());
    assert_eq!(devices.held.len(), 1);

    drop(write_end);
    let Ok((after_presses, after_touches)) = devices.read(&readable);
    assert!(after_presses.is_empty());
    assert!(after_touches.is_empty());
    assert!(devices.held.is_empty());

    Ok(())
  }
}
