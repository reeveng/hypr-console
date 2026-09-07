//! Whether the screen follows the clock, and the curve it follows.
//!
//!     console-warm            follow the clock, or stop following it
//!     console-warm get        which way the switch is standing
//!     console-warm wanted     nothing said; the exit code is the answer
//!     console-warm curve      print the config the daemon reads
//!
//! The colour itself is `hyprsunset`'s, out of a config written from
//! `console_settings::warm`. Nothing here sends it a temperature: the whole
//! curve is in the file it reads at startup, and a temperature sent afterwards
//! is undone by the next profile. So the switch is the daemon running or not
//! running, which `console-warm.service` asks about in `ExecCondition=` and
//! this restarts when the answer changes.
//!
//! `wanted` is that question and nothing else, so it is the exit code rather
//! than a word: systemd reads the code, and a program run before every start of
//! a unit should print nothing into the journal for the ordinary case.
//!
//! `curve` is how `files/home/@user@/.config/hypr/hyprsunset.conf` is made. It
//! is not something the device runs; it is run here, into the tree, and a test
//! holds the file to it.

use std::process::ExitCode;

use console_core_external_programs::Program;
use console_settings::warm::{Wanted, Warmth, at, config};

const UNIT: &str = "console-warm.service";

fn main() -> ExitCode {
    let word = std::env::args().nth(1).unwrap_or_default();

    match word == "curve" {
        true => {
            let Ok(config) = config();

            print!("{config}");
            return ExitCode::SUCCESS;
        }
        false => {},
    }

    let Ok(home) = std::env::var("HOME") else {
        eprintln!("console-warm: no HOME, so there is nobody to remember for");
        return ExitCode::FAILURE;
    };

    let Ok(at) = at(&home);
    let said = match std::fs::read_to_string(&at) {
        Ok(said) => said,

        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => String::new(),

        Err(fault) => {
            eprintln!("console-warm: {}: {fault}", at.display());
            String::new()
        }
    };

    let Ok(standing) = Warmth::read(&said);

    let wanted = match word.as_str() {
        "get" => {
            let Ok(written) = standing.written();

            println!("{}", written.trim());
            return ExitCode::SUCCESS;
        }
        "wanted" => {
            let Ok(wanted) = standing.wanted();

            return match wanted {
                Wanted::Running => ExitCode::SUCCESS,
                Wanted::Off => ExitCode::FAILURE,
            };
        }
        "" => {
            let Ok(other) = standing.other();

            other
        }
        _ => {
            eprintln!("usage: console-warm [get|wanted|curve]");
            return ExitCode::from(2);
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

            match std::fs::write(&at, written) {
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
