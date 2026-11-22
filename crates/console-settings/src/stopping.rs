//! What a battery running out puts on the screen, and how the machine stops.
//! `console_battery` decides when to warn or stop. The configurable stop gives
//! the user time to plug in, then hibernates if the machine has a resume device
//! or shuts down cleanly. At the final safeguard, the machine acts without a
//! countdown. Suspend can hold the session in RAM while there is power left.

use std::time::Duration;

use console_battery::Action::{Configured, Emergency};
use console_battery::{Action, Step};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_notifications::saying::{Content, Notification};

pub const GRACE: Duration = Duration::from_secs(15);

pub const LOOKING: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stop {
  Hibernate,
  Suspend,
  PowerOff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resume<'a>(pub &'a str);

impl Stop {
  pub fn of(state: &str, resume: Resume<'_>) -> Result<Self, Never> {
    let can = state.split_whitespace().any(|word| word == "disk");
    let somewhere = !matches!(resume.0.trim(), "" | "0:0");

    Ok(match ::core::primitive::bool::then_some(can && somewhere, ()) {
      ::core::option::Option::Some(()) => Stop::Hibernate,
      ::core::option::Option::None => Stop::PowerOff,
    })
  }

  pub fn arguments(self) -> Result<Vec<String>, Never> {
    let what = match self {
      Stop::Hibernate => "hibernate",
      Stop::Suspend => "suspend",
      Stop::PowerOff => "poweroff",
    };

    Program::Systemctl.arguments(&[what])
  }

  pub fn emergency_of(state: &str, resume: Resume<'_>) -> Result<Self, Never> {
    let Ok(stop) = Self::of(state, resume);
    let can_suspend = state
      .split_whitespace()
      .any(|word| matches!(word, "mem" | "standby" | "freeze"));

    Ok(match (stop, can_suspend) {
      (Stop::PowerOff, true) => Stop::Suspend,
      _ => stop,
    })
  }

  pub fn attempts(self, action: Action) -> Result<&'static [Self], Never> {
    Ok(match (self, action) {
      (Stop::Hibernate, Emergency) => &[Stop::Hibernate, Stop::Suspend, Stop::PowerOff],
      (Stop::Hibernate, _) => &[Stop::Hibernate, Stop::PowerOff],
      (Stop::Suspend, _) => &[Stop::Suspend, Stop::PowerOff],
      (Stop::PowerOff, _) => &[Stop::PowerOff],
    })
  }
}

pub const STATE: &str = "/sys/power/state";
pub const RESUME: &str = "/sys/power/resume";

pub fn stop(action: Action) -> Result<Stop, Never> {
  let said = |at: &str| match std::fs::read_to_string(at) {
    Ok(said) => said,
    Err(_unreadable) => String::new(),
  };

  let state = said(STATE);
  let resume = said(RESUME);

  match action {
    Emergency => Stop::emergency_of(&state, Resume(&resume)),
    Configured(_) => Stop::of(&state, Resume(&resume)),
  }
}

pub fn card(action: Action, charge: i32, stop: Stop) -> Result<Notification, Never> {
  let left = format!("{charge}% left.");

  match action {
    Configured(Step::Low) => {
      let Ok(notification) = Notification::new(Content {
        summary: "Low Battery",
        body: &left,
      });
      let Ok(notification_2) = notification.lasting(6000);

      notification_2.valued(i64::from(charge))
    }
    Configured(Step::Lower) => {
      let Ok(notification) = Notification::new(Content {
        summary: "Very Low Battery",
        body: &format!("{left} Plug in soon."),
      });
      let Ok(notification_3) = notification.urgent();
      let Ok(notification_4) = notification_3.staying();

      notification_4.valued(i64::from(charge))
    }
    Configured(Step::Protect) => stopping(charge, stop),
    Emergency => {
      let action_2 = match stop {
        Stop::Hibernate => "Hibernating now.",
        Stop::Suspend => "Sleeping now. Plug in to keep your session.",
        Stop::PowerOff => "Shutting down now. Unsaved work will be lost.",
      };
      let Ok(notification) = Notification::new(Content {
        summary: "Critical Battery",
        body: &format!("{left} {action}", action = action_2),
      });
      let Ok(urgent) = notification.urgent();
      let Ok(staying) = urgent.staying();

      staying.valued(i64::from(charge))
    }
  }
}

fn stopping(charge: i32, stop: Stop) -> Result<Notification, Never> {
  let seconds = GRACE.as_secs();
  let (summary, body) = match stop {
    Stop::Hibernate => (
      format!("Going to sleep in {seconds} seconds"),
      format!("{charge}% left. Your work is kept. Plug in to keep going."),
    ),
    Stop::Suspend => (
      format!("Going to sleep in {seconds} seconds"),
      format!("{charge}% left. Plug in to keep your session."),
    ),
    Stop::PowerOff => (
      format!("Shutting down in {seconds} seconds"),
      format!("{charge}% left. Unsaved work is lost. Plug in to keep going."),
    ),
  };
  let Ok(notification) = Notification::new(Content {
    summary: &summary,
    body: &body,
  });
  let Ok(notification_2) = notification.urgent();
  let Ok(notification_3) = notification_2.staying();

  notification_3.valued(i64::from(charge))
}

pub fn saved() -> Result<Notification, Never> {
  let Ok(notification) = Notification::new(Content {
    summary: "Charging",
    body: "Nothing was turned off.",
  });

  notification.lasting(4000)
}

pub fn for_the_journal(charge: i32, stop: Stop) -> Result<String, Never> {
  let doing = match stop {
    Stop::Hibernate => "hibernating",
    Stop::Suspend => "suspending",
    Stop::PowerOff => "shutting down",
  };

  Ok(format!("battery at {charge}%: {doing} before it runs out"))
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_core_arguments::Subcommand;

  #[test]
  fn a_kernel_that_can_and_a_machine_with_nowhere_to_write_cannot_hibernate() {
    assert_eq!(
      Stop::of("freeze mem disk", Resume("0:0")),
      Ok(Stop::PowerOff)
    );
    assert_eq!(
      Stop::of("freeze mem disk", Resume("259:2")),
      Ok(Stop::Hibernate)
    );
    assert_eq!(Stop::of("freeze mem", Resume("259:2")), Ok(Stop::PowerOff));
    assert_eq!(Stop::of("", Resume("")), Ok(Stop::PowerOff));
  }

  #[test]
  fn emergency_sleep_prefers_hibernation_then_suspend_then_shutdown() {
    assert_eq!(
      Stop::emergency_of("freeze mem disk", Resume("259:2")),
      Ok(Stop::Hibernate)
    );
    assert_eq!(
      Stop::emergency_of("freeze mem disk", Resume("0:0")),
      Ok(Stop::Suspend)
    );
    assert_eq!(
      Stop::emergency_of("disk", Resume("0:0")),
      Ok(Stop::PowerOff)
    );
    assert_eq!(
      Stop::Hibernate.attempts(Emergency),
      Ok([Stop::Hibernate, Stop::Suspend, Stop::PowerOff].as_slice())
    );
    assert_eq!(
      Stop::Suspend.attempts(Emergency),
      Ok([Stop::Suspend, Stop::PowerOff].as_slice())
    );
    assert_eq!(
      Stop::Hibernate.attempts(Configured(Step::Protect)),
      Ok([Stop::Hibernate, Stop::PowerOff].as_slice())
    );
  }

  #[test]
  fn the_card_promises_only_what_the_machine_can_do() {
    let Ok(stopping) = card(Configured(Step::Protect), 5, Stop::PowerOff);

    assert!(
      stopping.summary.contains("Shutting down"),
      "{}",
      stopping.summary
    );
    assert!(stopping.body.contains("is lost"), "{}", stopping.body);

    let Ok(saving) = card(Configured(Step::Protect), 5, Stop::Hibernate);

    assert!(saving.summary.contains("sleep"), "{}", saving.summary);
    assert!(saving.body.contains("is kept"), "{}", saving.body);
  }

  #[test]
  fn the_card_says_how_long_there_is() {
    let Ok(said) = card(Configured(Step::Protect), 4, Stop::PowerOff);
    let seconds = GRACE.as_secs().to_string();

    assert!(said.summary.contains(&seconds) || said.body.contains(&seconds));
  }

  #[test]
  fn only_the_first_card_takes_itself_away() {
    use console_notifications::saying::Expiry;

    let Ok(low) = card(Configured(Step::Low), 25, Stop::PowerOff);
    let Ok(lower) = card(Configured(Step::Lower), 10, Stop::PowerOff);
    let Ok(protect) = card(Configured(Step::Protect), 5, Stop::PowerOff);

    assert_eq!(low.expiry, Expiry::Milliseconds(6000));
    assert_eq!(lower.expiry, Expiry::Stays);
    assert_eq!(protect.expiry, Expiry::Stays);
  }

  #[test]
  fn every_card_draws_what_is_left() {
    let Ok(actions) = Action::variants();

    for action in actions {
      let Ok(said) = card(action, 7, Stop::PowerOff);

      assert_eq!(said.value, Some(7));
    }
  }

  #[test]
  fn the_journal_says_why_the_machine_stopped() {
    let Ok(said) = for_the_journal(4, Stop::PowerOff);

    assert!(
      said.contains("4%") && said.contains("shutting down"),
      "{said}"
    );
  }
}
