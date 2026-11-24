//! Whether the screen follows the clock, and the curve it follows.
//!
//!     console-warm on|scheduled|off
//!                             choose when the display is warm
//!     console-warm get        which way the switch is standing
//!     console-warm switched-on   nothing said; the exit code is the answer
//!     console-warm curve      print the config the daemon reads
//!
//! The service asks whether Night Light is enabled and uses this command to
//! launch hyprsunset in the saved mode. The scheduled mode reads the shipped
//! curve. The always-on mode sets a constant color temperature.
//!
//! `curve` prints the config from which the repo's scheduled profile is made.

use std::os::unix::process::CommandExt;
use std::process::ExitCode;

use console_core_arguments::{Flag, Operands, Subcommand, Takes, read_with};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_words::Words;
use console_settings::warm::{self, NightShift, Standing, Switched, at, configuration};

const UNIT: &str = "console-warm.service";

const ENGINE: Flag = Flag {
  spelling: "--engine",
  takes: Takes::Value("PATH"),
  about: "the night light daemon to run from the service",
};

const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
  name: "console-warm",
  about: "whether the screen follows the clock, and the curve it follows; with nothing, follow it or stop following it",
  flags: &[ENGINE],
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
  #[words(word = "curve", about = "print the config the daemon reads")]
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

  let (asked, engine) = match read_with::<Report, String>(&COMMAND, &words) {
    Ok(line) => {
      let Ok(asked) = line.subcommand();
      let Ok(engine) = line.value(ENGINE);

      (asked, engine.map(str::to_owned))
    }
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };

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
      eprintln!("console-warm: {}: {fault}", at.display());

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
      let engine_path = match engine {
        Some(path) => path,
        None => {
          eprintln!("console-warm: --engine is required for the service");
          return ExitCode::FAILURE;
        }
      };

      let mut command = std::process::Command::new(engine_path);

      match standing {
        NightShift::On => {
          command.args([
            "--config",
            "/dev/null",
            "--temperature",
            &warm::WARM.to_string(),
          ]);
        }
        NightShift::Scheduled => {}
        NightShift::Off => return ExitCode::SUCCESS,
      }

      let fault = command.exec();
      eprintln!("console-warm: starting the night light daemon: {fault}");
      return ExitCode::FAILURE;
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
