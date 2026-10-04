//! What the machine has been spending, and what spent it.
//!
//!     console-resource-usage             the readings so far, added up
//!     console-resource-usage note        take one reading and keep it
//!     console-resource-usage --file PATH somewhere other than the store
//!
//! `note` is what the timer runs and no one types, and it keeps nothing while
//! the `measuring` setting is off; everything else is the reading. The store is
//! a line per reading and is meant to be read by anything -- `jq`, a
//! spreadsheet, whatever someone writes once -- and this is the reading that
//! should not have to be written twice: how much power went through the machine
//! while it was awake, how much battery that came to, and which programs were
//! holding the CPU while it happened.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use console_core_arguments::{Command, Flag, Operands, Subcommand, Takes, read_with};
use console_core_never::Never;
use console_core_words::Words;
use console_response_times::measuring::{self, Measuring};
use console_resource_usage::{Moment, usage_between, append, of, sample, summarize, where_};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
enum Taking {
    #[words(word = "note", about = "take one reading and keep it, which is what the timer runs")]
    Note,
}

impl Subcommand for Taking {
    fn variants() -> Result<impl Iterator<Item = Self>, Never> {
        Ok(Taking::VARIANTS.iter().copied())
    }

    fn spelling(self) -> Result<&'static str, Never> {
        self.word()
    }

    fn about(self) -> Result<&'static str, Never> {
        Taking::about(self)
    }
}

const FILE: Flag = Flag {
    spelling: "--file",
    takes: Takes::Value("PATH"),
    about: "read and keep the readings somewhere other than the store",
};

const COMMAND: Command = Command {
    name: "console-resource-usage",
    about: "what the machine has been spending, and what spent it",
    flags: &[FILE],
    operands: Operands::None,
};

fn main() -> ExitCode {
    let asked: Vec<String> = std::env::args().skip(1).collect();
    let line = match read_with::<Taking, String>(&COMMAND, &asked) {
        Ok(line) => line,
        Err(refusal) => {
            let Ok(code) = refusal.print();

            return ExitCode::from(code);
        }
    };
    let Ok(taking) = line.subcommand();
    let Ok(file) = line.value(FILE);
    let Ok(store) = where_();
    let at = match file {
        Some(file) => Some(PathBuf::from(file)),
        None => store,
    };

    let at = match at {
        Some(at) => at,
        None => {
            eprintln!("console-resource-usage: nothing says where a reading would be written");

            return ExitCode::FAILURE;
        }
    };

    match taking {
        Some(Taking::Note) => {
            let Ok(chosen) = measuring::current();

            match chosen {
                Measuring::On => {}
                Measuring::Off => return ExitCode::SUCCESS,
            }

            let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
                Ok(now) => now.as_secs(),
                Err(fault) => {
                    eprintln!("console-resource-usage: the clock is before the epoch: {fault}");

                    return ExitCode::FAILURE;
                }
            };
            let Ok(moment) = sample(now);

            match append(&at, &moment) {
                Ok(()) => ExitCode::SUCCESS,
                Err(fault) => {
                    eprintln!("console-resource-usage: {fault}");

                    ExitCode::FAILURE
                }
            }
        }
        None => {
            let store = match File::open(&at) {
                Ok(store) => store,
                Err(_nothing_kept_yet) => {
                    println!("nothing measured yet: {}", at.display());

                    return ExitCode::SUCCESS;
                }
            };
            let Ok(moments) = read_moments(store);
            let Ok(used) = usage_between(&moments);

            match used {
                Some(used) => {
                    let Ok(said) = summarize(&used);

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

fn read_moments(store: File) -> Result<Vec<Moment>, Never> {
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
