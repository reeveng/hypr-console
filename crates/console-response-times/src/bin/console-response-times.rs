//! What the machine has been like to wait for.
//!
//!     console-response-times              the last few thousand waits
//!     console-response-times --last 50    the last fifty
//!     console-response-times --all        every one the store holds
//!     console-response-times --raw        the lines themselves, for something else to read
//!
//! The store is a line per thing waited for and it is meant to be read by
//! anything -- `jq`, a spreadsheet, a script someone writes once. This is the
//! reading that should not have to be written twice: which surface is slow,
//! which stretch of it is the slow one, and whether the worst of them is worth
//! chasing or is one opening from a boot.
//!
//! It reads a line at a time and holds a window rather than a file, because the
//! store is kept now: it is allowed to grow to ten gigabytes, and a program
//! that answers *how has this week been* by loading the year into memory is a
//! program that stops answering. `WINDOW` is the default and `--all` is there
//! for whoever wants the year and has the memory for it.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::process::ExitCode;

use console_core_iteration::{Endless, Step};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_response_times::{line, summary, where_};

const USAGE: &str = "usage: console-response-times [--last N] [--all] [--raw] [--file PATH]";

const WINDOW: u32 = 20_000;

fn main() -> ExitCode {
    match console_signals::restore_default(console_signals::Signal::PIPE) {
        Ok(()) => {},
        Err(fault) => eprintln!("console-response-times: a closed pipe will be an error rather than an ending: {fault}"),
    }

    let asked: Vec<String> = std::env::args().skip(1).collect();
    let Ok(at) = where_();
    let reading = Reading { words: asked.iter(), window: Some(WINDOW), raw: Raw::No, at };
    let read = console_core_iteration::iterate(reading, |mut reading| {
        Ok(match reading.words.next() {
            None => Step::Halt(Ok(reading)),
            Some(word) => match heard(&mut reading, word) {
                Ok(()) => Step::Again(reading),
                Err(refused) => Step::Halt(Err(refused)),
            },
        })
    });

    let Reading { window, raw, at, .. } = match read {
        Ok(Ok(reading)) => reading,
        Ok(Err(Rejected::Last(fault))) => {
            eprintln!("console-response-times: --last: {fault}");
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
        Ok(Err(Rejected::Usage)) | Err(Endless) => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    let at = match at {
        Some(at) => at,
        None => {
            eprintln!("console-response-times: nothing says where a wait would be written");

            return ExitCode::FAILURE;
        }
    };

    let store = match File::open(&at) {
        Ok(store) => store,
        Err(_unreadable) => {
            println!("nothing waited for yet: {}", at.display());
            return ExitCode::SUCCESS;
        }
    };

    let Ok((kept, whole)) = read_lines(store, window);

    match raw {
        Raw::Yes => {
            for said in &kept {
                println!("{said}");
            }

            return ExitCode::SUCCESS;
        }
        Raw::No => {}
    }

    let entries: Vec<line::Entry> = kept
        .iter()
        .filter_map(|said| {
            let Ok(entry) = line::read(said);

            entry
        })
        .collect();

    match entries.len() {
        0 => {
            println!("nothing readable in {}", at.display());
            return ExitCode::SUCCESS;
        }
        1.. => {}
    }

    let Ok(shown) = fitted::<_, u64>(kept.len());

    match whole > shown {
        true => println!("the last {} waits of {whole}. --all reads the rest\n", kept.len()),
        false => {}
    }

    let Ok(gathered) = summary::about(&entries);

    for about in gathered {
        let Ok(said) = summary::summarize(&about);

        print!("{said}");
    }

    ExitCode::SUCCESS
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Raw {
    Yes,
    No,
}

enum Rejected {
    Last(std::num::ParseIntError),
    Usage,
}

struct Reading<'a> {
    words: std::slice::Iter<'a, String>,
    window: Option<u32>,
    raw: Raw,
    at: Option<std::path::PathBuf>,
}

fn heard(reading: &mut Reading<'_>, word: &str) -> Result<(), Rejected> {
    match word {
        "--last" => match reading.words.next().map(|many| many.parse::<u32>()) {
            Some(Ok(many)) => reading.window = Some(many),
            Some(Err(fault)) => return Err(Rejected::Last(fault)),
            None => return Err(Rejected::Usage),
        },
        "--all" => reading.window = None,
        "--file" => match reading.words.next() {
            Some(path) => reading.at = Some(std::path::PathBuf::from(path)),
            None => return Err(Rejected::Usage),
        },
        "--raw" => reading.raw = Raw::Yes,
        _ => return Err(Rejected::Usage),
    }

    Ok(())
}

fn read_lines(store: File, window: Option<u32>) -> Result<(VecDeque<String>, u64), Never> {
    let mut kept: VecDeque<String> = VecDeque::new();
    let mut whole: u64 = 0;

    for said in BufReader::new(store).lines() {
        let said = match said {
            Ok(said) => said,
            Err(_the_read_failed) => continue,
        };

        whole = whole.saturating_add(1);
        kept.push_back(said);

        match window {
            Some(many) => {
                let Ok(many) = index(many);

                match kept.len() > many {
                    true => {
                        let _ = kept.pop_front();
                    }
                    false => {}
                }
            }
            None => {}
        }
    }

    Ok((kept, whole))
}
