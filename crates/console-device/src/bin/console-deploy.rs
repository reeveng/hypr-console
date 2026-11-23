//! Put this checkout on the device and bring the machine to match it.
//!
//! Everything decided is in `console_device::deploying`. What is here is the
//! half that touches this machine: the environment the device is named in, the
//! top of the tree, and the lock -- which is a `mkdir` and four small files,
//! and is the one effect no other program on this device will ever want.
//!
//! The lock is released by dropping it. A deploy that ends any way the loop
//! can see ends by returning through here, and one killed harder than that
//! leaves a directory the next run takes over because the pid in it is gone.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console_core_arguments::read;
use console_core_external_programs::Program as ExternalProgram;
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

impl Interpreter for Locking {
  type Event = DeployingEvent;
  type Effect = DeployingEffect;

  fn interpret(&mut self, acts: &DeployingEffect, _clock: &Clock) -> Vec<Event<DeployingEvent>> {
    match acts {
      DeployingEffect::Take(at) => match std::fs::create_dir(at) {
        Ok(()) => {
          self.held = Some(at.clone());

          vec![Event::Custom(DeployingEvent::Took)]
        }
        Err(_already_held) => vec![Event::Custom(DeployingEvent::Busy)],
      },

      DeployingEffect::Mine(at) => {
        let Ok(here) = here();
        let Ok(today) = today();
        let pid = std::process::id().to_string();
        let Ok(()) = wrote(
          at,
          Line {
            called: LockField::Pid,
            said: &pid,
          },
        );
        let Ok(()) = wrote(
          at,
          Line {
            called: LockField::Host,
            said: &here,
          },
        );
        let Ok(()) = wrote(
          at,
          Line {
            called: LockField::Since,
            said: &today,
          },
        );

        Vec::new()
      }

      DeployingEffect::Read(at) => {
        let Ok(holding) = read_holder(at);

        vec![Event::Custom(DeployingEvent::Holder(holding))]
      }

      DeployingEffect::Free(at) => {
        let Ok(()) = remove_lock(at);

        Vec::new()
      }
    }
  }
}

impl Drop for Locking {
  fn drop(&mut self) {
    match &self.held {
      Some(at) => {
        let Ok(()) = remove_lock(at);
      }
      None => {}
    }
  }
}

fn remove_lock(at: &Path) -> Result<(), Never> {
  match std::fs::remove_dir_all(at) {
    Ok(()) => {}
    Err(fault) => {
      match ::core::primitive::bool::then_some(fault.kind() == std::io::ErrorKind::NotFound, ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => {
          eprintln!("console-deploy: removing lock {}: {fault}", at.display())
        }
      }
    }
  }

  Ok(())
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

fn wrote(at: &Path, line: Line<'_>) -> Result<(), Never> {
  let Line { called, said } = line;
  let path = at.join(called);

  match console_core_atomic_writes::whole(&path, format!("{said}\n").as_bytes()) {
    Ok(()) => {}
    Err(fault) => eprintln!(
      "console-deploy: writing lock field {}: {fault}",
      path.display()
    ),
  }

  Ok(())
}

fn read_field(at: &Path, called: LockField) -> Result<String, Never> {
  read_trimmed(&at.join(called))
}

fn read_trimmed(at: &Path) -> Result<String, Never> {
  Ok(match console_core_atomic_writes::text_or_empty(at) {
    Ok(said) => said.trim().to_string(),
    Err(fault) => {
      eprintln!("console-deploy: {fault}");

      String::new()
    }
  })
}

fn read_holder(at: &Path) -> Result<Holder, Never> {
  let Ok(pid) = read_field(at, LockField::Pid);
  let Ok(on) = read_field(at, LockField::Host);
  let Ok(since) = read_field(at, LockField::Since);
  let Ok(here) = here();

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

fn here() -> Result<String, Never> {
  read_trimmed(Path::new(NAMED))
}

fn today() -> Result<String, Never> {
  let Ok(asking) = ExternalProgram::Date.arguments(&[]);

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
