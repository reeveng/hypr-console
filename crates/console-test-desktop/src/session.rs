//! Which compositor is which, and waiting for one to arrive.
//!
//! A nested Hyprland picks its own signature and its own display and announces
//! neither. The signature is the directory it makes under `hypr/`, named for
//! the second it started in, so the one new name there is this one's --
//! `Starting` is what keeps that true, by letting one session come up at a
//! time. The display it writes down itself: `hyprland.lock` in that directory
//! is its pid and the `wayland-` name it bound, and that file is where
//! `hyprctl instances` reads both from as well.
//!
//! What this used to do was watch the runtime directory for a `wayland-` name
//! that had not been there before, which is only sound about a compositor
//! no one killed. libwayland takes the lowest name whose lock is free and
//! unlinks whatever socket is sitting on it, so a compositor that was killed
//! leaves its name in that directory for good -- and killing one is exactly
//! what this does to a session it decides never came up. The next session
//! binds that name, arrives under a name that was already there, waits out its
//! patience, is killed for never coming up, and hands the same name to the one
//! after it. Run one at a time nothing shows: the lowest free name is the one
//! the session before just gave back, so it always reads as new. Run four at
//! once and a different pair of them fails every time, saying the compositor
//! never came up about compositors that were up and drawing.
//!
//! A name is still held to `wayland-` and a number and nothing else, because
//! the line read out of a file a compositor is in the middle of writing is
//! whatever happened to be flushed. A display is that shape; half a pid is
//! not.

use std::collections::BTreeSet;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Duration;

use console_compositor::session::{InstanceStatus, responds};
use console_compositor::socket::{self, Socket};
use console_core_never::Never;
use console_panel::description::{Description, KeyboardFocus};
use console_waiting::Schedule;

use crate::{Process, process_presence, runtime, stages};

pub const COMING_UP: Duration = Duration::from_secs(15);

const BREATH: Duration = Duration::from_millis(100);

pub fn instances() -> Result<BTreeSet<String>, Never> {
  let Ok(runtime) = runtime();

  Ok(match console_compositor::session::signatures(&runtime) {
    Ok(found) => found,
    Err(_instance_listing_unavailable) => BTreeSet::new(),
  })
}

pub fn display(signature: &str) -> Result<Option<String>, Never> {
  let Ok(runtime) = runtime();
  let Ok(directory) = instance_directory(&runtime, signature);

  match directory {
    Some(at) => display_in(&at),
    None => Ok(None),
  }
}

fn instance_directory(runtime: &Path, signature: &str) -> Result<Option<PathBuf>, Never> {
  Ok(match socket::instance_directory(runtime, signature) {
    Ok(path) => Some(path),
    Err(_invalid_name) => None,
  })
}

fn display_in(directory: &Path) -> Result<Option<String>, Never> {
  Ok(match console_compositor::session::display_in(directory) {
    Ok(display) => display,
    Err(_display_unavailable) => None,
  })
}

fn until<T>(patience: Duration, mut look: impl FnMut() -> Option<T>) -> Result<Option<T>, Never> {
  let Ok(patience_2) = Schedule::asking_every(patience, BREATH);

  console_waiting::until_some_handed(patience_2, &mut look, |look_2| Ok(look_2()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Came {
  pub signature: String,
  pub display: String,
}

pub fn wait_for_one(was: &BTreeSet<String>) -> Result<Option<Came>, Never> {
  until(COMING_UP, || {
    let Ok(instances) = instances();
    let Ok(runtime) = runtime();

    instances.difference(was).find_map(|name| {
      let Ok(directory) = instance_directory(&runtime, name);
      let at = directory?;

      match console_compositor::session::connects(&at.join(Socket::Requests)) {
        Ok(()) => {}
        Err(fault) => {
          console_core_logging::trace!(
            "console-desktop: compositor {name} is not listening yet: {fault}"
          );

          return None;
        }
      }

      let Ok(said) = display_in(&at);

      said.map(|display| Came {
        signature: name.clone(),
        display,
      })
    })
  })
}

pub const A_LINE: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wrote {
  Some,
  None,
}

pub fn lines(at: &Path) -> Result<u64, Never> {
  match std::fs::read(at) {
    Ok(read) => {
      console_core_number_conversion::fitted(read.iter().filter(|byte| **byte == b'\n').count())
    }
    Err(_unreadable) => Ok(NOT_A_LINE),
  }
}

const NOT_A_LINE: u64 = 0;

pub fn wait_for_more_than(at: &Path, already: u64, patience: Duration) -> Result<Wrote, Never> {
  wait_for_count_past(at, already, patience, lines)
}

fn wait_for_count_past(
  at: &Path,
  already: u64,
  patience: Duration,
  count: fn(&Path) -> Result<u64, Never>,
) -> Result<Wrote, Never> {
  let Ok(found) = until(patience, || {
    let Ok(now) = count(at);

    match ::core::primitive::bool::then_some(now > already, ()) {
      ::core::option::Option::Some(()) => Some(()),
      ::core::option::Option::None => None,
    }
  });

  Ok(match found {
    Some(()) => Wrote::Some,
    None => Wrote::None,
  })
}

fn last_told(at: &Path) -> Result<Option<Description>, Never> {
  let read = match std::fs::read_to_string(at) {
    Ok(read) => read,
    Err(_unreadable) => return Ok(None),
  };

  Ok(
    match read.lines().last().map(console_panel::description::read) {
      Some(Ok(told)) => Some(told),
      Some(Err(unread)) => {
        eprintln!("console-desktop: {}: {unread}", at.display());

        None
      }
      None => None,
    },
  )
}

pub fn presses(at: &Path) -> Result<u64, Never> {
  let Ok(told) = last_told(at);

  Ok(match told {
    Some(told_2) => told_2.presses,
    None => NOT_PRESSED,
  })
}

const NOT_PRESSED: u64 = 0;

pub fn wait_for_the_keyboard(at: &Path, patience: Duration) -> Result<Wrote, Never> {
  let Ok(found) = until(patience, || {
    let Ok(told) = last_told(at);

    match told.map(|told_2| told_2.keyboard) {
      Some(KeyboardFocus::Here) => Some(()),
      Some(KeyboardFocus::Elsewhere) | None => None,
    }
  });

  Ok(match found {
    Some(()) => Wrote::Some,
    None => Wrote::None,
  })
}

pub fn wait_for_presses_past(
  at: &Path,
  already: u64,
  patience: Duration,
) -> Result<Wrote, Never> {
  wait_for_count_past(at, already, patience, presses)
}

pub fn wait_for_written(at: &Path, patience: Duration) -> Result<Wrote, Never> {
  wait_for_more_than(at, NOT_A_LINE, patience)
}

pub fn remove_tree(at: &Path) -> std::io::Result<()> {
  std::fs::remove_dir_all(at).or_else(|fault| {
    match (fault.kind() == std::io::ErrorKind::NotFound).then_some(()) {
      Some(()) => Ok(()),
      None => Err(fault),
    }
  })
}

pub fn left_behind(signature: &str) -> Result<(), Never> {
  let Ok(runtime) = runtime();
  let Ok(directory) = instance_directory(&runtime, signature);

  match directory {
    Some(at) => match remove_tree(&at) {
      Ok(()) => {}
      Err(fault) => {
        console_core_logging::warn!("console-desktop: clearing {}: {fault}", at.display())
      }
    },
    None => {
      console_core_logging::warn!("console-desktop: invalid compositor signature {signature:?}")
    }
  }

  Ok(())
}

pub fn swept() -> Result<(), Never> {
  let Ok(abandoned) = abandoned();

  for path in abandoned {
    let Ok(named) = crate::scope_of(&path);

    match named {
      Some(unit) => {
        let Ok(()) = console_operating_system_linux::systemd::nothing_left_in(&unit);
      }
      None => {}
    }

    match remove_tree(&path) {
      Ok(()) => {}
      Err(fault) => {
        console_core_logging::warn!("console-desktop: clearing {}: {fault}", path.display())
      }
    }
  }

  Ok(())
}

pub fn abandoned() -> Result<Vec<PathBuf>, Never> {
  let Ok(stages) = stages();

  let entries = match std::fs::read_dir(stages) {
    Ok(entries) => entries,
    Err(_unreadable) => return Ok(Vec::new()),
  };

  let mut found: Vec<PathBuf> = entries
    .flatten()
    .map(|entry| entry.path())
    .filter_map(|path| {
      let name = path.file_name().and_then(std::ffi::OsStr::to_str)?;
      let raw = name.strip_prefix("session-")?;
      let digits = raw
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .then_some(raw)?;
      let number = match digits.parse::<i32>() {
        Ok(number) => number,
        Err(fault) => {
          console_core_logging::debug!("console-desktop: invalid process ID {digits:?}: {fault}");

          return None;
        }
      };
      let pid = rustix::process::Pid::from_raw(number)?;
      let Ok(state) = process_presence(pid);

      match state {
        Process::Gone => Some(path),
        Process::Present => None,
      }
    })
    .collect();
  found.sort();

  Ok(found)
}

#[cfg_attr(
  dylint_lib = "explicit029_no_asking_per_item",
  allow(
    explicit029_no_asking_per_item,
    reason = "a compositor is asked whether it is still there one at a time because there is no one to ask about all of them at once, and the list is the sessions left behind on this machine"
  )
)]
pub fn dead_instances() -> Result<Vec<PathBuf>, Never> {
  let Ok(said) = console_compositor::instance();

  let ours = match said {
    Some(ours) => ours,
    None => String::new(),
  };
  let Ok(instances) = instances();
  let Ok(runtime) = runtime();

  Ok(
    instances
      .into_iter()
      .filter(|name| *name != ours)
      .filter(|name| {
        let Ok(status) = responds(name);

        status == InstanceStatus::Unreachable
      })
      .filter_map(|name| {
        let Ok(directory) = instance_directory(&runtime, &name);

        directory
      })
      .collect(),
  )
}

pub struct Starting {
  _held: Option<File>,
}

impl Starting {
  pub fn now() -> Result<Self, Never> {
    let Ok(stages) = stages();

    match std::fs::create_dir_all(&stages) {
      Ok(()) => {}
      Err(fault) => {
        console_core_logging::warn!("console-desktop: preparing {}: {fault}", stages.display());

        return Ok(Starting { _held: None });
      }
    }

    #[cfg_attr(
      dylint_lib = "explicit040_no_torn_write",
      allow(
        explicit040_no_torn_write,
        reason = "the lock two nested desktops start under, whose whole point is the open file and not its bytes"
      )
    )]
    let held = match File::create(stages.join("starting.lock")) {
      Ok(file) => match rustix::fs::flock(&file, rustix::fs::FlockOperation::LockExclusive) {
        Ok(()) => Some(file),
        Err(fault) => {
          console_core_logging::warn!(
            "console-desktop: the startup lock would not be taken: {fault}"
          );

          None
        }
      },
      Err(fault) => {
        console_core_logging::warn!("console-desktop: opening the startup lock: {fault}");

        None
      }
    };

    Ok(Starting { _held: held })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_compositor_running_this_screen_is_never_one_of_the_dead() {
    let Ok(signature) = console_compositor::instance();
    let running = match signature {
      Some(signature_2) => signature_2,
      None => return,
    };

    let Ok(dead) = dead_instances();

    assert!(!dead.iter().any(|path| path.ends_with(&running)));
  }

  #[test]
  fn a_stage_is_abandoned_only_when_the_session_it_names_has_ended() -> Result<(), crate::Unnested>
  {
    let stage = crate::stage()?;
    let Ok(abandoned) = abandoned();

    assert!(
      !abandoned.contains(&stage),
      "this session's own stage is not abandoned"
    );

    Ok(())
  }
}
