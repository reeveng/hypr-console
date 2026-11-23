//! What happens when the battery gets low. The configurable alerts can play
//! the warning cue when Sound Effects and notifications are enabled.
//!
//! ```text
//! console-battery low      say it is getting low
//! console-battery lower    say it is getting really low
//! console-battery protect  say the machine is stopping, wait, and stop it
//! console-battery emergency  put the machine to sleep at 1% or less while unplugged
//! ```
//! `console-bar` dispatches the command when a reading reaches a threshold.
//! `console_battery` decides when a threshold is reached and
//! `console_settings::stopping` sets the available power actions and messages.

use std::process::{Command, ExitCode};

use console_battery::Action::{Configured, Emergency};
use console_battery::{Action, Cable, Charge, Step, charge};
use console_core_arguments::{Operands, read_with};
use console_core_never::Never;
use console_notifications::reading::DoNotDisturb;
use console_notifications::saying::{Content, StatePath, journal, raise_kept};
use console_notifications::serving::load_inbox;
use console_settings::stopping::{GRACE, LOOKING, Stop, card, for_the_journal, saved, stop};
use console_sound_effects::catalogue::Sound;
use console_sound_effects::{Degree, SoundEffects};
use console_waiting::{Schedule, until_some};

const NOTHING_SAID: i32 = 0;
const EMERGENCY_CANCELED: u8 = 3;

const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
  name: "console-battery",
  about: "what happens when the battery gets low",
  flags: &[],
  operands: Operands::None,
};

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();
  let asked =
    read_with::<Action, String>(&COMMAND, &words).and_then(|line| line.require_subcommand());

  let action = match asked {
    Ok(action) => action,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };

  let Ok(said) = charge();
  let Ok(now) = Charge::of(&said);
  let Ok(emergency_percent) = now.emergency();

  match (action, emergency_percent) {
    (Configured(Step::Protect) | Emergency, Some(percent)) => {
      let Ok(exit) = emergency(percent);

      return exit;
    }
    (Emergency, None) => return ExitCode::from(EMERGENCY_CANCELED),
    _ => {}
  }

  let percent = match now.percent {
    Some(percent) => percent,
    None => {
      eprintln!("no battery on this machine to say anything about");

      return ExitCode::FAILURE;
    }
  };

  let Ok(stop) = stop(action);
  let Ok(()) = announce(action, percent, stop);
  let Ok(()) = play_warning();

  match action {
    Configured(Step::Protect) => {
      let Ok(gone) = stopping(percent, stop);

      gone
    }
    Configured(Step::Low | Step::Lower) | Emergency => ExitCode::SUCCESS,
  }
}

fn announce(action: Action, percent: i32, stop: Stop) -> Result<(), Never> {
  let Ok(card) = card(action, percent, stop);
  let Ok(kept) = StatePath::new("battery");

  raise_kept(card, &kept)
}

fn play_warning() -> Result<(), Never> {
  let Ok(effects) = SoundEffects::current();

  match effects {
    SoundEffects::Off => return Ok(()),
    SoundEffects::On => {}
  }

  let Ok(inbox) = load_inbox();

  match inbox.do_not_disturb {
    DoNotDisturb::On => return Ok(()),
    DoNotDisturb::Off => {}
  }

  let Ok(cue) = Sound::Warning.cue();

  match console_sound_effects::play(&cue, Degree(0)) {
    Ok(()) => {}
    Err(fault) => eprintln!("console-battery: {fault}"),
  }

  Ok(())
}

fn stopping(percent: i32, stop: Stop) -> Result<ExitCode, Never> {
  let Ok(patience) = Schedule::asking_every(GRACE, LOOKING);
  let Ok(changed) = until_some(patience, power_change);

  match changed {
    Some(PowerChange::PluggedIn(percent_2)) => plugged_in(percent_2, Configured(Step::Protect)),
    Some(PowerChange::Emergency(percent_3)) => emergency(percent_3),
    None => {
      let Ok(result) = request_stop(Configured(Step::Protect), percent, stop);

      match result {
        Protection::Finished(code) => Ok(code),
        Protection::Escalated(remaining) => emergency(remaining),
      }
    }
  }
}

fn plugged_in(percent: i32, action: Action) -> Result<ExitCode, Never> {
  let Ok(()) = journal(&format!(
    "battery at {percent}%: the cable went in, so nothing was stopped"
  ));
  let Ok(saved) = saved();
  let Ok(kept) = StatePath::new("battery");
  let Ok(()) = raise_kept(saved, &kept);

  Ok(match action {
    Emergency => ExitCode::from(EMERGENCY_CANCELED),
    Configured(_) => ExitCode::SUCCESS,
  })
}

fn emergency(percent: i32) -> Result<ExitCode, Never> {
  let Ok(stop) = stop(Emergency);
  let Ok(()) = announce(Emergency, percent, stop);

  let Ok(result) = request_stop(Emergency, percent, stop);
  Ok(match result {
    Protection::Finished(code) => code,
    Protection::Escalated(_) => ExitCode::FAILURE,
  })
}

enum Protection {
  Finished(ExitCode),
  Escalated(i32),
}

fn request_stop(action: Action, percent: i32, stop: Stop) -> Result<Protection, Never> {
  let Ok(attempts) = stop.attempts(action);

  for &doing in attempts {
    let Ok(changed) = power_change();

    match (action, changed) {
      (_, Some(PowerChange::PluggedIn(percent_2))) => {
        let Ok(code) = plugged_in(percent_2, action);
        return Ok(Protection::Finished(code));
      }
      (Configured(Step::Protect), Some(PowerChange::Emergency(percent_3))) => {
        return Ok(Protection::Escalated(percent_3));
      }
      _ => {}
    }

    let Ok(arguments) = doing.arguments();

    let (program, rest) = match arguments.split_first() {
      Some((program, rest)) => (program, rest),
      None => {
        eprintln!("nothing to run: the way to stop the machine named no program");

        continue;
      }
    };

    #[cfg_attr(
      dylint_lib = "explicit029_no_asking_per_item",
      expect(
        explicit029_no_asking_per_item,
        reason = "each shutdown fallback may stop the machine; only try the next one if this one failed and power has not returned"
      )
    )]
    match Command::new(program).args(rest).status() {
      Ok(how) => match ::core::primitive::bool::then_some(how.success(), ()) {
        ::core::option::Option::Some(()) => {
          let Ok(said) = for_the_journal(percent, doing);
          let Ok(()) = journal(&said);

          return Ok(Protection::Finished(ExitCode::SUCCESS));
        }
        ::core::option::Option::None => eprintln!("{} said {how}", arguments.join(" ")),
      },
      Err(fault) => eprintln!("no {program} to run: {fault}"),
    }
  }

  let said = "Battery nearly empty. Plug it in.";
  let Ok(notification) = console_notifications::saying::Notification::new(Content {
    summary: "Battery protection failed",
    body: said,
  });
  let Ok(notification_2) = notification.urgent();
  let Ok(notification_3) = notification_2.staying();
  let Ok(kept) = StatePath::new("battery");

  let Ok(()) = journal(&format!(
    "battery at {percent}%: nothing would stop the machine"
  ));
  let Ok(()) = raise_kept(notification_3, &kept);

  Ok(Protection::Finished(ExitCode::FAILURE))
}

enum PowerChange {
  PluggedIn(i32),
  Emergency(i32),
}

fn power_change() -> Result<Option<PowerChange>, Never> {
  let said = charge()?;
  let now = Charge::of(&said)?;
  let Ok(cable) = now.filling.cable();

  Ok(match cable {
    Cable::Connected => Some(PowerChange::PluggedIn(match now.percent {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => NOTHING_SAID,
    })),
    Cable::Disconnected => {
      let checked_value = now.emergency()?;
      checked_value
    }
    .map(PowerChange::Emergency),
  })
}
