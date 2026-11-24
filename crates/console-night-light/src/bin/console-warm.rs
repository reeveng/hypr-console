//! Control Console's Night Light and run its session daemon.

use std::fmt;
use std::ops::RangeInclusive;
use std::process::ExitCode;

use console_core_arguments::{Flag, Operands, Presence, Subcommand, Takes, read_with};
use console_core_color::temperature::{MAX_KELVIN, MIN_KELVIN};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_words::Words;
use console_night_light::{
  self as warm, NightShift, Standing, Switched, Temperature, at, configuration,
};

mod clock;
mod display;

use display::{GammaPercent, Look};

const UNIT: &str = "console-warm.service";

const TEMPERATURE: Flag = Flag {
  spelling: "--temperature",
  takes: Takes::Value("KELVIN"),
  about: "use a fixed temperature in the daemon",
};
const GAMMA: Flag = Flag {
  spelling: "--gamma",
  takes: Takes::Value("PERCENT"),
  about: "scale output brightness in the daemon, from 0 to 200",
};
const IDENTITY: Flag = Flag {
  spelling: "--identity",
  takes: Takes::None,
  about: "use neutral colors in the daemon",
};

const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
  name: "console-warm",
  about: "set Night Light or toggle it when called without a command",
  flags: &[TEMPERATURE, GAMMA, IDENTITY],
  operands: Operands::None,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
enum Report {
  #[words(word = "get", about = "which way the switch is standing")]
  Get,
  #[words(
    word = "switched-on",
    about = "nothing said; the exit code is the answer"
  )]
  SwitchedOn,
  #[words(word = "curve", about = "print the fallback UTC schedule")]
  Curve,
  #[words(word = "on", about = "keep the display warm")]
  On,
  #[words(word = "scheduled", about = "follow the night light schedule")]
  Scheduled,
  #[words(word = "off", about = "turn night light off")]
  Off,
  #[words(word = "daemon", about = "run the service's night light daemon")]
  Daemon,
}

impl Subcommand for Report {
  fn variants() -> Result<impl Iterator<Item = Self>, Never> {
    Ok(Report::VARIANTS.iter().copied())
  }

  fn spelling(self) -> Result<&'static str, Never> {
    self.word()
  }

  fn about(self) -> Result<&'static str, Never> {
    Report::about(self)
  }
}

fn curve() -> Result<ExitCode, Never> {
  let Ok(configuration) = configuration();

  print!("{configuration}");

  Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();

  let line = match read_with::<Report, String>(&COMMAND, &words) {
    Ok(line) => line,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };

  let Ok(asked) = line.subcommand();
  let Ok(temperature) = line.value(TEMPERATURE);
  let Ok(gamma) = line.value(GAMMA);
  let Ok(identity) = line.presence(IDENTITY);

  match (asked, temperature, gamma, identity) {
    (Some(Report::Daemon), _, _, _) | (_, None, None, Presence::Absent) => {}
    _ => {
      eprintln!("console-warm: display flags require the daemon command");
      return ExitCode::FAILURE;
    }
  }

  match asked {
    Some(Report::Curve) => {
      let Ok(printed) = curve();

      return printed;
    }
    Some(
      Report::Get
      | Report::SwitchedOn
      | Report::On
      | Report::Scheduled
      | Report::Off
      | Report::Daemon,
    )
    | None => {}
  }

  let Ok(said) = console_core_places::home();

  let home = match said {
    Some(home) => home,
    None => {
      eprintln!("console-warm: no HOME, so there is no one to remember for");

      return ExitCode::FAILURE;
    }
  };

  let Ok(at) = at(&home);
  let Ok(held) = warm::load(&home);

  let standing = match held {
    Standing::Loaded(warmth) => warmth,

    Standing::Invalid(fault) => {
      console_core_logging::warn!("{}: {fault}", at.display());

      NightShift::Scheduled
    }
  };

  let wanted = match asked {
    Some(Report::Get) => {
      let Ok(written) = standing.written();

      println!("{}", written.trim());
      return ExitCode::SUCCESS;
    }
    Some(Report::SwitchedOn) => {
      let Ok(switched) = standing.switched();

      return match switched {
        Switched::On => ExitCode::SUCCESS,
        Switched::Off => ExitCode::FAILURE,
      };
    }
    Some(Report::Curve) => {
      let Ok(printed) = curve();

      return printed;
    }
    Some(Report::Daemon) => {
      return match daemon(standing, temperature, gamma, identity) {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
          eprintln!("console-warm: {fault}");
          ExitCode::FAILURE
        }
      };
    }
    Some(Report::On) => NightShift::On,
    Some(Report::Scheduled) => NightShift::Scheduled,
    Some(Report::Off) => NightShift::Off,
    None => {
      let Ok(other) = standing.other();

      other
    }
  };

  match at.parent() {
    Some(holding) => {
      match std::fs::create_dir_all(holding) {
        Ok(()) => {}
        Err(fault) => {
          eprintln!(
            "console-warm: {}: {fault}, so nothing was changed",
            holding.display()
          );

          return ExitCode::FAILURE;
        }
      }

      let Ok(written) = wanted.written();

      match console_core_atomic_writes::whole(&at, written.as_bytes()) {
        Ok(()) => {}
        Err(fault) => {
          eprintln!(
            "console-warm: {}: {fault}, so nothing was changed",
            at.display()
          );

          return ExitCode::FAILURE;
        }
      }
    }
    None => {}
  }

  let Ok(mut asking) = Program::Systemctl.command();
  let done = asking.args(["--user", "restart", UNIT]).status();

  match done.map(|status| status.code()) {
    Ok(Some(0)) => {}
    Err(fault) => {
      eprintln!("console-warm: restarting {UNIT} failed: {fault}");
      return ExitCode::FAILURE;
    }
    Ok(_) => {
      eprintln!(
        "console-warm: {UNIT} would not restart, so the screen is still what it was. \
                 It is written down, and the next start of the desktop will wear it."
      );

      return ExitCode::FAILURE;
    }
  }

  ExitCode::SUCCESS
}

fn daemon(
  standing: NightShift,
  kelvin: Option<&str>,
  gamma: Option<&str>,
  identity: Presence,
) -> Result<(), Box<dyn std::error::Error>> {
  let look = match (kelvin, identity, standing) {
    (Some(_), Presence::Present, _) => {
      return Err(Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "choose either --temperature or --identity",
      )));
    }
    (Some(value), Presence::Absent, _) => {
      let fixed = bounded_value(value, TEMPERATURE, MIN_KELVIN..=MAX_KELVIN)?;

      Look::Fixed(Temperature::Kelvin(fixed))
    }
    (None, Presence::Present, _) => Look::Fixed(Temperature::Neutral),
    (None, Presence::Absent, NightShift::On) => Look::Fixed(Temperature::Kelvin(warm::WARM)),
    (None, Presence::Absent, NightShift::Scheduled) => Look::Scheduled,
    (None, Presence::Absent, NightShift::Off) => return Ok(()),
  };
  let percent = match gamma {
    Some(value) => bounded_value(value, GAMMA, 0..=200)?,
    None => 100,
  };

  display::run(look, GammaPercent(percent))
}

#[derive(Debug)]
struct InvalidValue {
  flag: Flag,
  allowed: RangeInclusive<u32>,
}

impl fmt::Display for InvalidValue {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      out,
      "{} expects a whole number between {} and {}",
      self.flag.spelling,
      self.allowed.start(),
      self.allowed.end(),
    )
  }
}

impl std::error::Error for InvalidValue {}

fn bounded_value(
  text: &str,
  flag: Flag,
  allowed: RangeInclusive<u32>,
) -> Result<u32, InvalidValue> {
  let value = match text.parse::<u32>() {
    Ok(parsed) => parsed,
    Err(_not_an_unsigned_integer) => return Err(InvalidValue { flag, allowed }),
  };

  match allowed.contains(&value).then_some(value) {
    Some(valid) => Ok(valid),
    None => Err(InvalidValue { flag, allowed }),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_core_arguments::{Reason, ValidationError};

  #[test]
  fn no_word_flips_the_switch_and_it_refuses_a_word_it_does_not_know()
  -> Result<(), ValidationError> {
    let switch = read_with::<Report, &str>(&COMMAND, &[])?;
    let curve = read_with::<Report, &str>(&COMMAND, &["curve"])?;
    let unknown = read_with::<Report, &str>(&COMMAND, &["warmer"]);

    assert_eq!(switch.subcommand(), Ok(None));
    assert_eq!(curve.subcommand(), Ok(Some(Report::Curve)));
    assert_eq!(
      unknown.map_err(|refusal| refusal.reason),
      Err(Reason::NoSuchSubcommand("warmer".to_string()))
    );

    Ok(())
  }
}
