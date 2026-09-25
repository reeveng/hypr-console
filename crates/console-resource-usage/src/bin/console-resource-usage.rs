//! What the machine has been spending, and what spent it.
//!
//!     console-resource-usage             the readings so far, added up
//!     console-resource-usage note        take one reading and keep it
//!     console-resource-usage --file PATH somewhere other than the store
//!
//! `note` is what the timer runs and no one types; everything else is the
//! reading. The store is a line per reading and is meant to be read by anything
//! -- `jq`, a spreadsheet, whatever someone writes once -- and this is the
//! reading that should not have to be written twice: how much power went
//! through the machine while it was awake, how much battery that came to, and
//! which programs were holding the CPU while it happened.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use console_core_never::Never;
use console_resource_usage::{Moment, between, kept, of, taken, told, where_};

const USAGE: &str = "usage: console-resource-usage [note] [--file PATH]";

const NOTE: &str = "note";

fn main() -> ExitCode {
    let asked: Vec<String> = std::env::args().skip(1).collect();
    let mut note = false;
    let Ok(mut at) = where_();

    let mut words = asked.iter();

    while let Some(word) = words.next() {
        match word.as_str() {
            NOTE => note = true,
            "--file" => match words.next() {
                Some(said) => at = Some(PathBuf::from(said)),
                None => {
                    eprintln!("{USAGE}");

                    return ExitCode::FAILURE;
                }
            },
            _unknown => {
                eprintln!("{USAGE}");

                return ExitCode::FAILURE;
            }
        }
    }

    let at = match at {
        Some(at) => at,
        None => {
            eprintln!("console-resource-usage: nothing says where a reading would be written");

            return ExitCode::FAILURE;
        }
    };

    match note {
        true => {
            let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
                Ok(now) => now.as_secs(),
                Err(fault) => {
                    eprintln!("console-resource-usage: the clock is before the epoch: {fault}");

                    return ExitCode::FAILURE;
                }
            };
            let Ok(moment) = taken(now);

            match kept(&at, &moment) {
                Ok(()) => ExitCode::SUCCESS,
                Err(fault) => {
                    eprintln!("console-resource-usage: {fault}");

                    ExitCode::FAILURE
                }
            }
        }
        false => {
            let store = match File::open(&at) {
                Ok(store) => store,
                Err(_nothing_kept_yet) => {
                    println!("nothing measured yet: {}", at.display());

                    return ExitCode::SUCCESS;
                }
            };
            let Ok(moments) = held(store);
            let Ok(used) = between(&moments);

            match used {
                Some(used) => {
                    let Ok(said) = told(&used);

                    print!("{said}");

                    ExitCode::SUCCESS
                }
                None => {
                    println!("one reading is not a measurement: {}", at.display());

                    ExitCode::SUCCESS
                }
            }
        }
    }
}

fn held(store: File) -> Result<Vec<Moment>, Never> {
    let mut kept: Vec<Moment> = Vec::new();

    for said in BufReader::new(store).lines() {
        let said = match said {
            Ok(said) => said,
            Err(_unreadable) => continue,
        };
        let Ok(moment) = of(&said);

        match moment {
            Some(moment) => kept.push(moment),
            None => {}
        }
    }

    Ok(kept)
}
