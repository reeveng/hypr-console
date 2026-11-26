//! Clean desktop exits take a checkpoint before handing off to the system.
//!
//! A snapshot that exceeds its deadline may be skipped once its worker has
//! been reaped. If the kernel still has a worker in an uninterruptible write,
//! the finish action is withheld.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use console_core_external_programs::Program;
use console_core_state_machine::{EffectDependencies, Machine, Never, Queue, Service};
use console_notifications::{Application, saying::LiveNotification};
use console_operating_system_linux::program_lifetime::{BoundToParent, alongside};
use console_waiting::{
  Outcome, Ready, Schedule,
  clock::{Clock, LiveClock},
  until_handed,
};

use crate::{
  Unresumed, already,
  session::{Duplicates, Really, Sessions, under},
};

const ENDING_FOR: Duration = Duration::from_secs(30);
const CAPTURE_FOR: Duration = Duration::from_secs(5);
const REAP_FOR: Duration = Duration::from_millis(250);

pub const CAPTURE_WORKER: &str = "--capture-checkpoint";
const DISPLAYS: &str = "displays";
const WINDOWS_BY_PROGRAM: &str = "windows-by-program";
const WINDOWS_BY_INSTANCE: &str = "windows-by-instance";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
  Checkpoint,
  Logout,
  Reboot,
  Poweroff,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
  Quiet,
  Notify,
}

impl Action {
  fn preparing(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Checkpoint => "Saving your session…",
      Self::Logout => "Preparing to log out…",
      Self::Reboot => "Preparing to restart…",
      Self::Poweroff => "Preparing to shut down…",
    })
  }

  fn finishing(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Checkpoint => "Session saved",
      Self::Logout => "Logging out…",
      Self::Reboot => "Restarting…",
      Self::Poweroff => "Shutting down…",
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Capture {
  Displays,
  Windows,
  Music,
}

const CAPTURES: [Capture; 3] = [Capture::Displays, Capture::Windows, Capture::Music];

#[derive(Debug, PartialEq, Eq)]
enum Effect {
  Capture(Capture),
  End(Action),
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Capture(Capture::Displays) => {
        &[Service::Compositor, Service::Display, Service::FileSystem]
      }
      Self::Capture(Capture::Windows) => &[Service::Compositor, Service::FileSystem],
      Self::Capture(Capture::Music) => &[Service::MediaPlayer],
      Self::End(Action::Logout) => &[Service::Compositor],
      Self::End(Action::Reboot | Action::Poweroff) => &[Service::Power],
      Self::End(Action::Checkpoint) => &[],
    })
  }
}

struct Ending;

struct Capturing {
  action: Action,
  pending: BTreeSet<Capture>,
}

impl Machine for Ending {
  type Input = Action;
  type State = Capturing;
  type Request = Capture;
  type Effect = Effect;

  fn initialize(
    action: &Action,
    _: Option<Capturing>,
    effects: &mut Queue<Effect>,
  ) -> Result<Capturing, Never> {
    let Ok(()) = effects.offer_all(CAPTURES.into_iter().map(Effect::Capture));

    Ok(Capturing {
      action: *action,
      pending: BTreeSet::from(CAPTURES),
    })
  }

  fn handle(
    mut state: Capturing,
    captured: Capture,
    effects: &mut Queue<Effect>,
  ) -> Result<Capturing, Never> {
    match {
      ::core::primitive::bool::then_some(
        state.pending.remove(&captured)
          && state.pending.is_empty()
          && state.action != Action::Checkpoint,
        (),
      )
    } {
      ::core::option::Option::Some(()) => {
        let Ok(()) = effects.offer(Effect::End(state.action));
      }
      ::core::option::Option::None => {}
    }

    Ok(state)
  }
}

pub fn capture_worker(arguments: &[OsString]) -> Result<(), Unresumed> {
  match arguments {
    [part, at] => match part.to_str() {
      Some(DISPLAYS) => Sessions::save_setup(Path::new(at)),
      Some(WINDOWS_BY_PROGRAM) => {
        Sessions::save_windows(Path::new(at), Duplicates::OnePerProgram)
      }
      Some(WINDOWS_BY_INSTANCE) => {
        Sessions::save_windows(Path::new(at), Duplicates::OnePerWindow)
      }
      Some(_) | None => Err(Unresumed::Ending("unknown checkpoint capture".to_owned())),
    },
    _ => Err(Unresumed::Ending(
      "checkpoint worker needs a capture and a path".to_owned(),
    )),
  }
}

fn music_command() -> Result<Command, Never> {
  let Ok(mut bus) = console_bus::cli::command([
    "--user",
    "--quiet",
    "--timeout=1s",
    "--auto-start=no",
    "call",
    "org.mpris.MediaPlayer2.console",
    "/org/mpris/MediaPlayer2",
    "org.console.MusicPlayer.Session",
    "Checkpoint",
  ]);
  bus.stdout(Stdio::null()).stderr(Stdio::null());

  Ok(bus)
}

impl Capture {
  fn start(
    self,
    executable: &io::Result<PathBuf>,
    at: &Path,
    duplicates: Duplicates,
  ) -> Result<BoundToParent, Unresumed> {
    let part = match (self, duplicates) {
      (Self::Music, _) => None,
      (Self::Displays, _) => Some(DISPLAYS),
      (Self::Windows, Duplicates::OnePerProgram) => Some(WINDOWS_BY_PROGRAM),
      (Self::Windows, Duplicates::OnePerWindow) => Some(WINDOWS_BY_INSTANCE),
    };
    let mut command = match part {
      None => {
        let Ok(command) = music_command();
        command
      }
      Some(file_part) => {
        let executable_path = executable.as_ref().map_err(|fault| {
          Unresumed::Ending(format!("finding the checkpoint executable: {fault}"))
        })?;
        let mut command = Command::new(executable_path);
        command.arg(CAPTURE_WORKER).arg(file_part).arg(at);
        command
      }
    };

    command.stdin(Stdio::null());
    alongside(&mut command)
      .map_err(|fault| Unresumed::Unsaid(format!("{self:?} checkpoint"), fault))
  }
}

#[derive(Clone, Copy)]
enum Probe {
  Checking,
  Failed,
}

struct Worker {
  which: Capture,
  child: BoundToParent,
  probe: Probe,
}

impl Worker {
  fn check_exit(&mut self) -> Result<Ready, Never> {
    match self.probe {
      Probe::Checking => {}
      Probe::Failed => return Ok(Ready::NotYet),
    }

    match self.child.try_wait() {
      Ok(Some(status)) => {
        match (self.which, status.success().then_some(())) {
          (Capture::Music, _) | (_, Some(())) => {}
          (which, None) => {
            console_core_logging::warn!(
              "console-resume: {which:?} checkpoint worker exited with {status}"
            );
          }
        }

        Ok(Ready::Yes)
      }
      Ok(None) => Ok(Ready::NotYet),
      Err(fault) => {
        let which = self.which;
        console_core_logging::warn!("console-resume: checking {which:?} checkpoint: {fault}");
        self.probe = Probe::Failed;
        Ok(Ready::NotYet)
      }
    }
  }
}

fn reconcile(workers: &mut Vec<Worker>) -> Result<Ready, Never> {
  workers.retain_mut(|worker| {
    let Ok(readiness) = worker.check_exit();
    readiness == Ready::NotYet
  });

  Ok(match workers.first() {
    Some(_) => Ready::NotYet,
    None => Ready::Yes,
  })
}

fn wait_for_captures(mut workers: Vec<Worker>, patience: Schedule) -> Result<(), Unresumed> {
  let Ok(waited) = until_handed(patience, &mut workers, reconcile);

  match waited {
    Outcome::Happened => return Ok(()),
    Outcome::RanOut => {}
  }

  let mut stopped = Ok(());

  for Worker {
    which,
    mut child,
    probe: _,
  } in workers
  {
    console_core_logging::warn!("console-resume: {which:?} checkpoint exceeded its deadline");

    match child.terminate_within(REAP_FOR) {
      Ok(_) => {}
      Err(fault) => {
        let pending = Unresumed::Unsaid(format!("stopping {which:?} checkpoint"), fault);
        console_core_logging::warn!("console-resume: {pending}");
        stopped = Err(pending);
      }
    }
  }

  stopped
}

fn take_snapshots(
  sessions: &Sessions,
  name: &str,
  action: Action,
) -> Result<Vec<Effect>, Unresumed> {
  let Ok(initial) = Ending::initial_transition(&action, None);
  let requested: Vec<Capture> = initial
    .effects
    .into_iter()
    .filter_map(|effect| match effect {
      Effect::Capture(which) => Some(which),
      Effect::End(_) => None,
    })
    .collect();

  match under(&sessions.at, name) {
    Ok(at) => {
      let Ok(clock) = LiveClock::started();
      let executable = std::env::current_exe();
      let mut workers = Vec::new();

      for which in &requested {
        match which.start(&executable, &at, sessions.duplicates) {
          Ok(child) => workers.push(Worker {
            which: *which,
            child,
            probe: Probe::Checking,
          }),
          Err(fault) => console_core_logging::warn!("console-resume: {fault}"),
        }
      }

      let Ok(elapsed) = clock.elapsed();
      let remaining = CAPTURE_FOR.saturating_sub(elapsed);
      let Ok(patience) = Schedule::asking_every(remaining, Duration::from_millis(10));
      wait_for_captures(workers, patience)?;
    }
    Err(fault) => console_core_logging::warn!("console-resume: {fault}"),
  }

  let mut state = initial.state;
  let mut effects = Vec::new();

  for which in requested {
    let Ok(done) = Ending::transition(state, which);
    state = done.state;
    effects.extend(done.effects);
  }

  Ok(effects)
}

pub fn run(
  sessions: &Sessions,
  name: &str,
  action: Action,
  progress: Progress,
) -> Result<(), Unresumed> {
  match { ::core::primitive::bool::then_some(sessions.really == Really::Simulated, ()) } {
    ::core::option::Option::Some(()) => {
      println!("checkpoint session {name:?} before {action:?}");
      return Ok(());
    }
    ::core::option::Option::None => {}
  }

  let coordination = match Coordination::lock() {
    Ok(coordination) => coordination,
    Err(fault) => {
      console_core_logging::warn!("console-resume: {fault}");
      None
    }
  };

  match coordination.as_ref().map(Coordination::status) {
    Some(Ok(ShutdownStatus::Ending)) => return Ok(()),
    Some(Ok(ShutdownStatus::Available)) | None => {}
  }

  let mut notice = match progress {
    Progress::Quiet => None,
    Progress::Notify => {
      let Ok(made) = LiveNotification::new(Application("Settings"));

      Some(made)
    }
  };
  let Ok(preparing) = action.preparing();
  let Ok(()) = announce(&mut notice, preparing);
  let effects = take_snapshots(sessions, name, action)?;

  for effect in effects {
    match effect {
      Effect::End(action_2) => {
        match coordination.as_ref().map(Coordination::mark) {
          Some(Ok(())) | None => {}
          Some(Err(fault)) => {
            console_core_logging::warn!("console-resume: {fault}");
          }
        }

        drop(coordination);
        let Ok(finishing) = action_2.finishing();
        let Ok(()) = announce(&mut notice, finishing);
        return match end(action_2) {
          Ok(()) => {
            let Ok(()) = notice.map_or(Ok(()), LiveNotification::leave_until_expiry);

            Ok(())
          }
          Err(fault) => {
            let Ok(()) = Coordination::clear_stamp();

            Err(fault)
          }
        };
      }
      Effect::Capture(_) => {}
    }
  }

  Ok(())
}

fn announce(notice: &mut Option<LiveNotification>, summary: &str) -> Result<(), Never> {
  match notice.as_mut() {
    Some(shown) => shown.update(summary),
    None => Ok(()),
  }
}

fn end(action: Action) -> Result<(), Unresumed> {
  let word = match action {
    Action::Checkpoint => return Ok(()),
    Action::Logout => {
      return match console_compositor::exit() {
        Ok(console_compositor::DispatchResult::Success) => Ok(()),
        Ok(console_compositor::DispatchResult::Failure(why)) => Err(Unresumed::Ending(why)),
      };
    }
    Action::Reboot => "reboot",
    Action::Poweroff => "poweroff",
  };
  let Ok(mut command) = Program::Systemctl.command();
  let status = command
    .arg(word)
    .status()
    .map_err(|fault| Unresumed::Unsaid("systemctl".to_owned(), fault))?;

  match { ::core::primitive::bool::then_some(!status.success(), ()) } {
    ::core::option::Option::Some(()) => {
      return Err(Unresumed::Ending(format!("systemctl {word}: {status}")));
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

pub(crate) enum ShutdownStatus {
  Ending,
  Available,
}

pub(crate) struct Coordination {
  _lock: File,
  mark: PathBuf,
}

impl Coordination {
  pub(crate) fn lock() -> Result<Option<Self>, Unresumed> {
    let Ok(instance) = already::mark();

    let instance_2 = match instance {
      Some(matched_instance) => matched_instance,
      ::core::option::Option::None => return Ok(None),
    };

    let at = instance_2.with_extension("ending.lock");

    match at.parent() {
      Some(parent) => {
        std::fs::create_dir_all(parent)
          .map_err(|fault| Unresumed::Making(parent.to_owned(), fault))?;
      }
      ::core::option::Option::None => {}
    }

    let lock = console_core_atomic_writes::open_lock_file(&at)
      .map_err(|fault| Unresumed::Unsaid(at.display().to_string(), fault))?;
    lock
      .lock()
      .map_err(|fault| Unresumed::Unsaid(at.display().to_string(), fault))?;

    Ok(Some(Self {
      _lock: lock,
      mark: at.with_extension("stamp"),
    }))
  }

  pub(crate) fn status(&self) -> Result<ShutdownStatus, Never> {
    let stamp = std::fs::metadata(&self.mark).and_then(|metadata| metadata.modified());
    let changed = match stamp {
      Ok(time) => time,
      Err(_stamp_unavailable) => return Ok(ShutdownStatus::Available),
    };

    Ok(match changed.elapsed() {
      Ok(age) => match age.cmp(&ENDING_FOR) {
        std::cmp::Ordering::Less => ShutdownStatus::Ending,
        std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => ShutdownStatus::Available,
      },
      Err(_stamp_from_future) => ShutdownStatus::Available,
    })
  }

  fn mark(&self) -> Result<(), Unresumed> {
    console_core_atomic_writes::whole(&self.mark, b"").map_err(Unresumed::Unwritten)
  }

  fn clear_stamp() -> Result<(), Never> {
    match Self::lock() {
      Ok(Some(guard)) => match std::fs::remove_file(&guard.mark) {
        Ok(()) => {}
        Err(fault) => match (fault.kind() == std::io::ErrorKind::NotFound).then_some(()) {
          Some(()) => {}
          None => console_core_logging::debug!("clearing the shutdown stamp: {fault}"),
        },
      },
      Ok(None) => {}
      Err(fault) => console_core_logging::debug!("reopening the shutdown lock: {fault}"),
    }

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn ending_follows_the_snapshot_effect() {
    let Ok(initial) = Ending::initial_transition(&Action::Reboot, None);
    assert_eq!(initial.effects, CAPTURES.map(Effect::Capture));

    for [first, second, third] in [
      CAPTURES,
      [Capture::Music, Capture::Windows, Capture::Displays],
    ] {
      let Ok(initial_2) = Ending::initial_transition(&Action::Reboot, None);
      let Ok(partial) = Ending::transition(initial_2.state, first);
      assert!(partial.effects.is_empty());
      let Ok(almost_finished) = Ending::transition(partial.state, second);
      assert!(almost_finished.effects.is_empty());
      let Ok(finished) = Ending::transition(almost_finished.state, third);
      assert_eq!(finished.effects, [Effect::End(Action::Reboot)]);
      let Ok(repeated) = Ending::transition(finished.state, third);
      assert!(repeated.effects.is_empty());
    }
  }
}
