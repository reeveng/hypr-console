//! A transient user service opens browser windows outside the OBS monitor's sandbox.

mod holds;
mod obs;

use std::io;
use std::path::{Path, PathBuf};
use std::process::{ExitCode, Stdio};

use console_core_atomic_writes::{gone, whole_with_folders};
use console_core_external_programs::Program;
use console_core_state_machine::Never;
use console_default_applications::browsers;
use console_operating_system_linux::program_runtime::{self, Clock, Interpreter};
use console_program_contract::Event;
use console_stream_privacy::{DesktopEffect, Obs, Output, StreamPrivacy};

use holds::{Holds, Protection};
use obs::Logs;

fn main() -> ExitCode {
  let Ok(runtime) = console_core_places::application_runtime();

  let runtime_dir = match runtime {
    Some(dir) => dir,
    None => {
      console_core_logging::error!("XDG_RUNTIME_DIR is required for the OBS status applet");
      return ExitCode::FAILURE;
    }
  };

  let mut desktop = Desktop {
    logs: Logs::default(),
    uid: rustix::process::getuid().as_raw(),
    holds: Holds::default(),
    path: runtime_dir.join("bar-applets/obs.txt"),
    published: None,
    observing_error: None,
    notification_error: None,
    screen_error: None,
    publishing_error: None,
  };
  let Ok(code) = program_runtime::run::<StreamPrivacy, _>("stream-privacy", &(), &mut desktop);

  code
}

struct Desktop {
  logs: Logs,
  uid: u32,
  holds: Holds,
  path: PathBuf,
  published: Option<&'static str>,
  observing_error: Option<String>,
  notification_error: Option<String>,
  screen_error: Option<String>,
  publishing_error: Option<String>,
}

impl Interpreter for Desktop {
  type Event = Obs;
  type Effect = DesktopEffect;
  type Error = Never;

  fn interpret(&mut self, effect: &DesktopEffect, _: &Clock) -> Result<Vec<Event<Obs>>, Never> {
    match *effect {
      DesktopEffect::Observe => {
        let observed = self.logs.read(Path::new("/proc"), self.uid);
        let Ok(()) = report("read OBS state", &mut self.observing_error, &observed);
        let obs = match observed {
          Ok(obs) => obs,
          Err(_reported) => Obs::UNREADABLE,
        };

        return Ok(vec![Event::Custom(obs)]);
      }
      DesktopEffect::SwitchBrowser => match open_private_window() {
        Ok(()) => {}
        Err(fault) => console_core_logging::warn!("could not switch browser context: {fault}"),
      },
      DesktopEffect::HoldNotifications => {
        let held = self.holds.notifications();
        let Ok(()) = report("silence notifications", &mut self.notification_error, &held);
      }
      DesktopEffect::HoldScreen => {
        let held = self.holds.screen();
        let Ok(()) = report("inhibit screen idle", &mut self.screen_error, &held);
      }
      DesktopEffect::Release => {
        let Ok(()) = self.release();
      }
      DesktopEffect::Publish(obs) => {
        let Ok(()) = self.publish(obs);
      }
    }

    Ok(Vec::new())
  }

  fn finish(&mut self) -> Result<(), Never> {
    let Ok(()) = self.release();
    self.publish(Output::Closed)
  }
}

impl Desktop {
  fn release(&mut self) -> Result<(), Never> {
    self.holds = Holds::default();
    self.notification_error = None;
    self.screen_error = None;

    Ok(())
  }

  fn publish(&mut self, streaming: Output) -> Result<(), Never> {
    let state = match streaming {
      Output::Closed => "",
      Output::Idle => "idle",
      Output::Unreadable => "unavailable",
      Output::Active => {
        let Ok(protection) = self.holds.protection();

        match protection {
          Protection::Held => "live",
          Protection::Limited => "limited",
        }
      }
    };

    match { ::core::primitive::bool::then_some(self.published == Some(state), ()) } {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let result = match state {
      "" => gone(&self.path).map_err(|fault| fault.to_string()),
      _ => whole_with_folders(&self.path, state.as_bytes()).map_err(|fault| fault.to_string()),
    };
    let Ok(()) = report("publish OBS status", &mut self.publishing_error, &result);

    match result {
      Ok(()) => {
        self.published = Some(state);
        Ok(())
      }
      Err(_retry_on_next_observation) => Ok(()),
    }
  }
}

fn open_private_window() -> io::Result<()> {
  let arguments = browsers::private_window()?;
  let Ok(mut service) = Program::SystemdRun.command();
  let status = service
    .args([
      "--user",
      "--quiet",
      "--collect",
      "--expand-environment=no",
      "--service-type=exec",
      "--",
    ])
    .args(arguments)
    .stdin(Stdio::null())
    .status()?;

  match status.success().then_some(()) {
    Some(()) => Ok(()),
    None => Err(io::Error::other(format!(
      "browser launch exited with {status}"
    ))),
  }
}

fn report<T, E: std::fmt::Display>(
  action: &str,
  previous: &mut Option<String>,
  result: &Result<T, E>,
) -> Result<(), Never> {
  let current = match result {
    Ok(_value) => None,
    Err(fault) => Some(fault.to_string()),
  };

  match (current != *previous).then_some(()) {
    Some(()) => match current.as_deref() {
      Some(fault) => console_core_logging::warn!("could not {action}: {fault}"),
      None => {}
    },
    None => return Ok(()),
  }

  *previous = current;
  Ok(())
}
