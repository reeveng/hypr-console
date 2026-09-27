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

use console_core_iteration::{Endless, Step};
use console_core_never::Never;
use console_response_times::measuring::{self, Measuring};
use console_resource_usage::{Moment, usage_between, append, of, sample, summarize, where_};

const USAGE: &str = "usage: console-resource-usage [note] [--file PATH]";

const NOTE: &str = "note";

fn main() -> ExitCode {
    let asked: Vec<String> = std::env::args().skip(1).collect();
    let Ok(at) = where_();
    let reading = Reading { words: asked.iter(), note: Note::No, at };
    let read = console_core_iteration::iterate(reading, |mut reading| {
        Ok(match reading.words.next() {
            None => Step::Halt(Some(reading)),
            Some(word) => match heard(&mut reading, word) {
                Ok(Recognized::Understood) => Step::Again(reading),
                Ok(Recognized::Not) => Step::Halt(None),
            },
        })
    });

    let Reading { note, at, .. } = match read {
        Ok(Some(reading)) => reading,
        Ok(None) | Err(Endless) => {
            eprintln!("{USAGE}");

            return ExitCode::FAILURE;
        }
    };

    let at = match at {
        Some(at) => at,
        None => {
            eprintln!("console-resource-usage: nothing says where a reading would be written");

            return ExitCode::FAILURE;
        }
    };

    match note {
        Note::Yes => {
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
        Note::No => {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Note {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Recognized {
    Understood,
    Not,
}

struct Reading<'a> {
    words: std::slice::Iter<'a, String>,
    note: Note,
    at: Option<PathBuf>,
}

fn heard(reading: &mut Reading<'_>, word: &str) -> Result<Recognized, Never> {
    match word {
        NOTE => reading.note = Note::Yes,
        "--file" => match reading.words.next() {
            Some(said) => reading.at = Some(PathBuf::from(said)),
            None => return Ok(Recognized::Not),
        },
        _unknown => return Ok(Recognized::Not),
    }

    Ok(Recognized::Understood)
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
