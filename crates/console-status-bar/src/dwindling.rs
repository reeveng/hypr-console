//! Watching the battery, which is the one reading that moves while no one is
//! pressing anything.  udev says when a supply changed and `watch::tick` sits
//! under that as the net; the reading either of them wakes is taken here, for
//! the icon this bar draws, and it is the only reading on the machine.
//! Anything else wanting to know how full the battery is would be a second
//! program on a second clock, and two clocks reading one battery is two
//! machines' worth of opinions about when it crossed something.  So the
//! crossing is noticed here, where the reading already happens. What a crossing
//! is is `console_battery`, which can be asked without a
//! machine; what is done about one is `console-battery`, which is a program of
//! its own because the protection steps can stop the machine and a bar module is
//! not a thing that should be holding still for that.

use std::process::{Command, Stdio};
use std::time::Duration;

use console_battery::{Action, Cable, Charge, Levels, alert};
use console_core_never::Never;
use console_notifications::saying::StatePath;
use console_program_lifetime::{Detached, Still, let_go};

pub const SAID: &str = "battery-said";

pub const DOES: &str = "console-battery";

const NEAR_EMPTY: i32 = 5;
const QUICK_POLL: Duration = Duration::from_secs(2);

#[derive(Debug, Default)]
pub struct Watching {
  doing: Option<(Action, Detached)>,
  reading: Charge,
}

impl Watching {
  pub fn record(&mut self, said: &str) -> Result<(), Never> {
    let Ok(completed) = self.reap();
    let reading = Charge::of(said)?;
    self.reading = reading;

    let charge = match reading.percent {
      Some(charge) => charge,
      None => return Ok(()),
    };

    let Ok(kept) = StatePath::new(SAID);
    let Ok(read) = kept.read();

    let told = match (completed, read) {
      (Some(action), _) => Some(action),
      (None, Some(number)) => Action::of_number(number)?,
      (None, None) => None,
    };

    let levels = Levels::here()?;
    let said_2 = alert(levels, charge, reading.filling, told)?;

    let number = match said_2.told {
      Some(action) => action.number()?,
      None => 0,
    };

    match (
      said_2.act,
      &self.doing,
      (read != Some(number)).then_some(()),
    ) {
      (Some(action), None, _) => self.start(action, number, &kept),
      (Some(_), Some(_), _) | (None, _, None) => Ok(()),
      (None, _, Some(())) => kept.write(number),
    }
  }

  fn start(&mut self, action: Action, number: u32, kept: &StatePath) -> Result<(), Never> {
    let Ok(started) = run(action);

    match started {
      Some(child) => {
        self.doing = Some((action, child));

        match action {
          Action::Emergency => Ok(()),
          Action::Configured(_level) => kept.write(number),
        }
      }
      None => Ok(()),
    }
  }

  pub fn quick_poll(&self) -> Result<Option<Duration>, Never> {
    let Ok(cable) = self.reading.filling.cable();

    Ok(match cable {
      Cable::Connected => None,
      Cable::Disconnected => self
        .reading
        .percent
        .filter(|percent| *percent <= NEAR_EMPTY)
        .map(|_| QUICK_POLL),
    })
  }

  fn reap(&mut self) -> Result<Option<Action>, Never> {
    let (action, mut doing) = match self.doing.take() {
      Some(running) => running,
      None => return Ok(None),
    };

    let Ok(still) = doing.still();

    Ok(match (still, action) {
      (Still::Running, _) => {
        self.doing = Some((action, doing));

        None
      }
      (Still::Ended, Action::Emergency) => match doing.wait() {
        Ok(status) => status.success().then_some(action),
        Err(fault) => {
          console_core_logging::warn!("could not collect the emergency battery action: {fault}");
          None
        }
      },
      (Still::Ended, _) => None,
    })
  }
}

fn run(action: Action) -> Result<Option<Detached>, Never> {
  let word = action.word()?;
  let mut starting = Command::new(DOES);

  starting
    .arg(word)
    .stdout(Stdio::null())
    .stderr(Stdio::null());

  Ok(match let_go(&mut starting) {
    Ok(child) => Some(child),
    Err(fault) => {
      console_core_logging::warn!("{DOES} would not start for {word}: {fault}");
      None
    }
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_machine_with_no_battery_is_left_alone() {
    let mut watching = Watching::default();
    let Ok(()) = watching.record("");
    assert!(watching.doing.is_none());
  }

  #[test]
  fn every_step_is_asked_for_by_its_own_word() {
    for step in console_battery::EVERY {
      let Ok(word) = Action::Configured(step).word();

      assert!(!word.is_empty());
    }

    assert_eq!(Action::Emergency.word(), Ok("emergency"));
    assert_eq!(DOES, "console-battery");
  }

  #[test]
  fn only_an_unplugged_battery_near_empty_is_polled_faster() {
    for (said, expected) in [
      ("", None),
      ("6 unplugged Discharging", None),
      ("5 unplugged Discharging", Some(QUICK_POLL)),
      ("1 plugged Not charging", None),
    ] {
      let Ok(reading) = Charge::of(said);
      let watching = Watching {
        reading,
        ..Watching::default()
      };

      assert_eq!(watching.quick_poll(), Ok(expected));
    }
  }
}
