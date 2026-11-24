use std::error::Error;
use std::path::Path;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

use console_core_external_programs::Program;
use console_core_iteration::{Endless, Step, iterate};
use console_core_state_machine::{EffectDependencies, Machine, Never, Queue, Service};
use console_lock_screen::LockScreen;
use console_lock_screen::idle::{self, Delay};
use console_login_window::stored_pattern::{self, StoredPattern};
use console_program_contract::EventGroup;

pub fn write(base: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
  let config = read(base)?;
  console_core_atomic_writes::whole(output, config.as_bytes())?;
  Ok(())
}

enum LockAvailability {
  Available,
  Unavailable,
}

fn read(base: &Path) -> Result<String, std::io::Error> {
  let Ok(enabled) = console_lock_screen::current();
  let Ok(home) = console_core_places::home();
  let availability = match (enabled, home) {
    (LockScreen::On, Some(directory)) => match stored_pattern::stored(&directory) {
      Ok(StoredPattern::Absent) => LockAvailability::Unavailable,
      Ok(StoredPattern::Hash(_)) => LockAvailability::Available,
      Err(fault) => {
        console_core_logging::warn!("reading the pattern for the idle lock: {fault}");
        LockAvailability::Available
      }
    },
    (LockScreen::Off, _) | (LockScreen::On, None) => LockAvailability::Unavailable,
  };
  let Ok(delay) = idle::effective_delay();
  let Ok(suspend) = idle::on_suspend();
  let base_2 = std::fs::read_to_string(base)?;

  Ok({
    let Ok(explicit_value) = configuration(base_2, availability, delay, suspend);
    explicit_value
  })
}

pub fn watch_power(base: &Path, output: &Path) -> Result<(), Never> {
  const EVENT_WAIT: Duration = Duration::from_secs(30);
  let Ok(subscription) = console_events::subscription::connect(&[EventGroup::Battery]);
  let Ok(events) = subscription.received();

  let finished = iterate(events, |pending| {
    match pending.recv_timeout(EVENT_WAIT) {
      Ok(_event) => match refresh(base, output) {
        Ok(()) => {}
        Err(fault) => {
          console_core_logging::warn!(
            "applying the lock delay for the current power source: {fault}"
          );
        }
      },
      Err(RecvTimeoutError::Timeout) => return Ok(Step::Again(pending)),
      Err(RecvTimeoutError::Disconnected) => return Ok(Step::Halt(())),
    }

    Ok(Step::Again(pending))
  });

  match finished {
    Ok(()) | Err(Endless) => Ok(()),
  }
}

fn refresh(base: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
  let current = match std::fs::read_to_string(output) {
    Ok(current) => current,
    Err(fault) => match (fault.kind() == std::io::ErrorKind::NotFound).then_some(()) {
      Some(()) => return Ok(()),
      None => return Err(Box::from(fault)),
    },
  };
  let wanted = read(base)?;
  apply(Observation::Configuration { current, wanted })
}

pub fn request_lock(seconds: u32) -> Result<(), Box<dyn Error>> {
  let Ok(delay) = idle::effective_delay();
  apply(Observation::TimedOut { seconds, delay })
}

enum Observation {
  Configuration { current: String, wanted: String },
  TimedOut { seconds: u32, delay: Delay },
}

#[derive(Debug, PartialEq, Eq)]
enum IdleEffect {
  Restart,
  Lock,
}

impl EffectDependencies for IdleEffect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Restart => &[Service::Process],
      Self::Lock => &[Service::Session],
    })
  }
}

struct Idle;

impl Machine for Idle {
  type Input = ();
  type State = ();
  type Request = Observation;
  type Effect = IdleEffect;

  fn initialize(_: &(), _: Option<()>, _: &mut Queue<IdleEffect>) -> Result<(), Never> {
    Ok(())
  }

  fn handle(_: (), observed: Observation, effects: &mut Queue<IdleEffect>) -> Result<(), Never> {
    match observed {
      Observation::Configuration { current, wanted } => match (current != wanted).then_some(()) {
        Some(()) => effects.offer(IdleEffect::Restart),
        None => Ok(()),
      },
      Observation::TimedOut { seconds, delay } => {
        let Ok(configured) = delay.seconds();

        match (configured == Some(seconds)).then_some(()) {
          Some(()) => effects.offer(IdleEffect::Lock),
          None => Ok(()),
        }
      }
    }
  }
}

fn apply(observed: Observation) -> Result<(), Box<dyn Error>> {
  let Ok(turn) = Idle::transition((), observed);

  for effect in turn.effects {
    match effect {
      IdleEffect::Restart => run(
        Program::Systemctl,
        &["--user", "try-restart", idle::SERVICE],
      )?,
      IdleEffect::Lock => run(Program::Loginctl, &["lock-session"])?,
    }
  }

  Ok(())
}

fn run(program: Program, arguments: &[&str]) -> Result<(), Box<dyn Error>> {
  let Ok(mut command) = program.command();
  let status = command.args(arguments).status()?;

  match { ::core::primitive::bool::then_some(status.success(), ()) } {
    ::core::option::Option::Some(()) => Ok(()),
    ::core::option::Option::None => {
      let Ok(name) = program.name();
      Err(Box::from(format!("{name} exited with {status}")))
    }
  }
}

#[must_use]
fn configuration(
  mut base: String,
  availability: LockAvailability,
  delay: Delay,
  suspend: LockScreen,
) -> core::result::Result<String, console_core_never::Never> {
  match availability {
    LockAvailability::Available => {}
    LockAvailability::Unavailable => return Ok(base),
  }

  match suspend {
    LockScreen::On => {
      base
        .push_str("\ngeneral {\n    before_sleep_cmd = $start_lock\n    inhibit_sleep = 3\n}\n");
    }
    LockScreen::Off => {}
  }

  let Ok(timeout) = delay.seconds();

  match timeout {
    Some(seconds) => {
      base.push_str(&format!(
        "\nlistener {{\n    timeout = {seconds}\n    on-timeout = $request_lock {seconds}\n}}\n"
      ));
    }
    ::core::option::Option::None => {}
  }

  Ok(base)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_default_locks_after_five_minutes_and_waits_for_a_lock_before_sleep() {
    let Ok(config) = configuration(
      String::new(),
      LockAvailability::Available,
      Delay::default(),
      LockScreen::On,
    );
    assert!(config.contains("timeout = 300\n    on-timeout = $request_lock 300"));
    assert!(config.contains("before_sleep_cmd = $start_lock\n    inhibit_sleep = 3"));
  }

  #[test]
  fn custom_idle_and_suspend_lock_can_be_configured_independently()
  -> Result<(), idle::InvalidDelay> {
    let delay = Delay::custom("74s")?;
    let Ok(idle_only) = configuration(
      String::new(),
      LockAvailability::Available,
      delay,
      LockScreen::Off,
    );
    assert!(idle_only.contains("timeout = 74\n    on-timeout = $request_lock 74"));
    assert!(!idle_only.contains("before_sleep_cmd"));

    let Ok(suspend_only) = configuration(
      String::new(),
      LockAvailability::Available,
      Delay::NEVER,
      LockScreen::On,
    );
    assert!(!suspend_only.contains("listener"));
    assert!(suspend_only.contains("before_sleep_cmd"));
    Ok(())
  }

  #[test]
  fn without_a_lock_the_base_configuration_still_runs() {
    let base = "general {\n    lock_cmd = $start_lock\n    inhibit_sleep = 0\n}\n";
    assert_eq!(
      ({
        let Ok(explicit_value) = configuration(
          base.to_owned(),
          LockAvailability::Unavailable,
          Delay::default(),
          LockScreen::On,
        );
        explicit_value
      }),
      base
    );
  }

  #[test]
  fn only_a_current_timeout_locks_the_session() {
    for (delay, expected) in [
      (Delay::default(), vec![IdleEffect::Lock]),
      (Delay::NEVER, vec![]),
    ] {
      let Ok(turn) = Idle::transition(
        (),
        Observation::TimedOut {
          seconds: 300,
          delay,
        },
      );
      assert_eq!(turn.effects, expected);
    }
  }

  #[test]
  fn only_a_changed_idle_configuration_restarts_the_service() {
    for (wanted, expected) in [("old", vec![]), ("new", vec![IdleEffect::Restart])] {
      let Ok(turn) = Idle::transition(
        (),
        Observation::Configuration {
          current: "old".to_owned(),
          wanted: wanted.to_owned(),
        },
      );
      assert_eq!(turn.effects, expected);
    }
  }
}
