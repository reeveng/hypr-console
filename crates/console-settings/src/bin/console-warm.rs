//! Whether the screen follows the clock, and the curve it follows.
//!
//!     console-warm            follow the clock, or stop following it
//!     console-warm get        which way the switch is standing
//!     console-warm switched-on   nothing said; the exit code is the answer
//!     console-warm curve      print the config the daemon reads
//!
//! The color itself is `hyprsunset`'s, out of a config written from
//! `console_settings::warm`. Nothing here sends it a temperature: the whole
//! curve is in the file it reads at startup, and a temperature sent afterwards
//! is undone by the next profile. So the switch is the daemon running or not
//! running, which `console-warm.service` asks about in `ExecCondition=` and
//! this restarts when the answer changes.
//!
//! `switched-on` is that question and nothing else, so it is the exit code rather
//! than a word: systemd reads the code, and a program run before every start of
//! a unit should print nothing into the journal for the ordinary case.
//!
//! `curve` is how `files/home/@user@/.config/console/hypr/hyprsunset.conf` is made. It
//! is not something the device runs; it is run here, into the tree, and a test
//! holds the file to it.

use std::process::ExitCode;

use console_core_arguments::{Operands, Subcommand, read_with};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_words::Words;
use console_settings::warm::{self, Standing, Switched, NightShift, at, configuration};

const UNIT: &str = "console-warm.service";

const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
    name: "console-warm",
    about: "whether the screen follows the clock, and the curve it follows; with nothing, follow it or stop following it",
    flags: &[],
    operands: Operands::None,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
enum Report {
    #[words(word = "get", about = "which way the switch is standing")]
    Get,
    #[words(word = "switched-on", about = "nothing said; the exit code is the answer")]
    SwitchedOn,
    #[words(word = "curve", about = "print the config the daemon reads")]
    Curve,
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

    let asked = match read_with::<Report, String>(&COMMAND, &words) {
        Ok(line) => {
            let Ok(asked) = line.subcommand();

            asked
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
        Some(Report::Get | Report::SwitchedOn) | None => {},
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
        None => {
            let Ok(other) = standing.other();

            other
        }
    };

    match at.parent() {
        Some(holding) => {
            match std::fs::create_dir_all(holding) {
                Ok(()) => {},
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
                Ok(()) => {},
                Err(fault) => {
                    eprintln!("console-warm: {}: {fault}, so nothing was changed", at.display());

                    return ExitCode::FAILURE;
                }
            }
        }
        None => {},
    }

    let Ok(mut asking) = Program::Systemctl.command();
    let done = asking.args(["--user", "restart", UNIT]).status();

    match done.is_ok_and(|how| how.success()) {
        true => {},
        false => {
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
    fn no_word_flips_the_switch_and_it_refuses_a_word_it_does_not_know() -> Result<(), ValidationError> {
        let switch = read_with::<Report, &str>(&COMMAND, &[])?;
        let curve = read_with::<Report, &str>(&COMMAND, &["curve"])?;
        let unknown = read_with::<Report, &str>(&COMMAND, &["warmer"]);

        assert_eq!(switch.subcommand(), Ok(None));
        assert_eq!(curve.subcommand(), Ok(Some(Report::Curve)));
        assert_eq!(unknown.map_err(|refusal| refusal.reason), Err(Reason::NoSuchSubcommand("warmer".to_string())));

        Ok(())
    }
}
