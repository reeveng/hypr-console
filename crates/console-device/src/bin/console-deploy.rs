//! Put this checkout on the device and bring the machine to match it.
//!
//! Everything decided is in `console_device::deploying`. What is here is the
//! half that touches this machine: the environment the device is named in, the
//! top of the tree, and the lock -- which is a `mkdir` and four small files,
//! and is the one effect no other program on this device will ever want.
//!
//! The interpreter releases its lock when the program runtime finishes. A
//! process killed before cleanup leaves a directory the next run can take over.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{fmt, io};

use console_core_arguments::read;
use console_core_atomic_writes::{Unread, Unwritten};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_device::deploying::{
  self, Alive, Deploy, DeployingEffect, DeployingEvent, Deployment, Holder,
};
use console_device_name::device;
use console_operating_system_linux::program_runtime::{Clock, Interpreter};
use console_program_contract::Event;

const NAMED: &str = "/proc/sys/kernel/hostname";

struct Locking {
  held: Option<PathBuf>,
}

#[derive(Debug)]
enum LockError {
  Acquired(PathBuf),
  Create(PathBuf, io::Error),
  Read(Unread),
  Write(Unwritten),
  Remove(PathBuf, io::Error),
}

impl fmt::Display for LockError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Acquired(at) => write!(to, "the deploy already owns lock {}", at.display()),
      Self::Create(at, error) => {
        write!(to, "could not create deploy lock {}: {error}", at.display())
      }
      Self::Read(error) => write!(to, "could not read the deploy lock owner: {error}"),
      Self::Write(error) => write!(to, "could not write the deploy lock owner: {error}"),
      Self::Remove(at, error) => {
        write!(to, "could not remove deploy lock {}: {error}", at.display())
      }
    }
  }
}

impl std::error::Error for LockError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      Self::Acquired(_) => None,
      Self::Create(_, error) | Self::Remove(_, error) => Some(error),
      Self::Read(error) => Some(error),
      Self::Write(error) => Some(error),
    }
  }
}

impl Locking {
  fn take(&mut self, at: &Path) -> Result<DeployingEvent, LockError> {
    match &self.held {
      Some(held) => return Err(LockError::Acquired(held.clone())),
      None => {}
    }

    match std::fs::create_dir(at) {
      Ok(()) => self.held = Some(at.to_path_buf()),
      Err(error) => {
        return match ::core::primitive::bool::then_some(
          error.kind() == io::ErrorKind::AlreadyExists,
          (),
        ) {
          ::core::option::Option::Some(()) => Ok(DeployingEvent::Busy),
          ::core::option::Option::None => Err(LockError::Create(at.to_path_buf(), error)),
        };
      }
    }

    let host = here()?;
    let Ok(since) = today();
    let pid = std::process::id().to_string();

    for (called, said) in [
      (LockField::Pid, pid.as_str()),
      (LockField::Host, host.as_str()),
      (LockField::Since, since.as_str()),
    ] {
      wrote(at, Line { called, said })?;
    }

    Ok(DeployingEvent::Took)
  }
}

impl Interpreter for Locking {
  type Event = DeployingEvent;
  type Effect = DeployingEffect;
  type Error = LockError;

  fn interpret(
    &mut self,
    acts: &DeployingEffect,
    _clock: &Clock,
  ) -> Result<Vec<Event<DeployingEvent>>, LockError> {
    match acts {
      DeployingEffect::Take(at) => self.take(at).map(|event| vec![Event::Custom(event)]),

      DeployingEffect::Read(at) => {
        let holding = read_holder(at)?;

        Ok(vec![Event::Custom(DeployingEvent::Holder(holding))])
      }

      DeployingEffect::Free(at) => {
        remove_lock(at)?;

        Ok(Vec::new())
      }
    }
  }

  fn finish(&mut self) -> Result<(), LockError> {
    match self.held.take() {
      Some(at) => remove_lock(&at),
      None => Ok(()),
    }
  }
}

impl Drop for Locking {
  fn drop(&mut self) {
    match self.finish() {
      Ok(()) => {}
      Err(error) => eprintln!("console-deploy: {error}"),
    }
  }
}

fn remove_lock(at: &Path) -> Result<(), LockError> {
  match std::fs::remove_dir_all(at) {
    Ok(()) => Ok(()),
    Err(error) => {
      match ::core::primitive::bool::then_some(error.kind() == io::ErrorKind::NotFound, ()) {
        ::core::option::Option::Some(()) => Ok(()),
        ::core::option::Option::None => Err(LockError::Remove(at.to_path_buf(), error)),
      }
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LockField {
  Pid,
  Host,
  Since,
}

impl AsRef<Path> for LockField {
  fn as_ref(&self) -> &Path {
    Path::new(match self {
      LockField::Pid => "pid",
      LockField::Host => "on",
      LockField::Since => "since",
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Line<'a> {
  called: LockField,
  said: &'a str,
}

fn wrote(at: &Path, line: Line<'_>) -> Result<(), LockError> {
  let Line { called, said } = line;
  let path = at.join(called);

  console_core_atomic_writes::whole(&path, format!("{said}\n").as_bytes())
    .map_err(LockError::Write)
}

fn read_field(at: &Path, called: LockField) -> Result<String, LockError> {
  read_trimmed(&at.join(called))
}

fn read_trimmed(at: &Path) -> Result<String, LockError> {
  console_core_atomic_writes::text_or_empty(at)
    .map(|said| said.trim().to_string())
    .map_err(LockError::Read)
}

fn read_holder(at: &Path) -> Result<Holder, LockError> {
  let pid = read_field(at, LockField::Pid)?;
  let on = read_field(at, LockField::Host)?;
  let since = read_field(at, LockField::Since)?;
  let here = here()?;

  Ok(Holder {
    alive: match ::core::primitive::bool::then_some(
      std::fs::metadata(format!("/proc/{pid}")).is_ok_and(|metadata| metadata.is_dir()),
      (),
    ) {
      ::core::option::Option::Some(()) => Alive::Yes,
      ::core::option::Option::None => Alive::No,
    },
    on,
    since,
    here,
    pid,
  })
}

fn here() -> Result<String, LockError> {
  read_trimmed(Path::new(NAMED))
}

fn today() -> Result<String, Never> {
  let Ok(asking) = Program::Date.arguments(&[]);

  Ok(
    match console_core_external_programs::capture_output(&asking) {
      Ok(said) => said.trim().to_string(),
      Err(_unprinted) => String::new(),
    },
  )
}

#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  allow(
    explicit044_no_ambient_value,
    reason = "the first thing this does, before a relative path has meant anything: what follows is git and cargo, which are run inside a tree rather than handed one, and `console_repository` is what says which tree that is"
  )
)]
fn main() -> ExitCode {
  let host = match device() {
    Ok(host) => host,
    Err(fault) => {
      eprintln!("console-deploy: {fault}");

      return ExitCode::FAILURE;
    }
  };
  let words: Vec<String> = std::env::args().skip(1).collect();

  let line = match read(&deploying::COMMAND, &words) {
    Ok(line) => line,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };
  let Ok(deployment) = Deployment::of(&host, &line);

  match console_repository::root() {
    Ok(root) => match std::env::set_current_dir(&root) {
      Ok(()) => {}
      Err(fault) => {
        eprintln!("console-deploy: {}: {fault}", root.display());

        return ExitCode::FAILURE;
      }
    },
    Err(fault) => {
      eprintln!("console-deploy: {fault}");

      return ExitCode::FAILURE;
    }
  }

  let Ok(asked) = console_awake::taking_on(&host, console_awake::InhibitReason::FromDeploying);
  let _kept_up = match asked {
    console_awake::InhibitResult::Acquired(staying) => Some(staying),
    console_awake::InhibitResult::Failed(said) => {
      eprintln!("console-deploy: {said}");

      None
    }
  };

  let mut locking = Locking { held: None };

  let Ok(how) = console_operating_system_linux::program_runtime::run::<Deploy, Locking>(
    "console-deploy",
    &deployment,
    &mut locking,
  );

  how
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn lock_contention_and_filesystem_failure_are_distinct() -> Result<(), LockError> {
    let mut locking = Locking { held: None };
    let package = Path::new(env!("CARGO_MANIFEST_DIR"));
    let blocked = package.join("Cargo.toml").join(deploying::LOCK);

    let locking_take = locking.take(package)?;
    assert_eq!(locking_take, DeployingEvent::Busy);
    assert!(
      matches!(locking.take(&blocked), Err(LockError::Create(at, error)) if at == blocked && error.kind() == io::ErrorKind::NotADirectory)
    );
    assert!(locking.held.is_none());

    Ok(())
  }
}
