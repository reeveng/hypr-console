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

use console_core_arguments::{Command, CommandLine, Flag, NoSubcommand, Operands, Presence, Reason, Takes, ValidationError};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_response_times::{line, summary, where_};

const LAST: Flag = Flag { spelling: "--last", takes: Takes::Value("N"), about: "the last N waits rather than the last few thousand" };

const ALL: Flag = Flag { spelling: "--all", takes: Takes::None, about: "every wait the store holds" };

const RAW: Flag = Flag { spelling: "--raw", takes: Takes::None, about: "the lines themselves, for something else to read" };

const FILE: Flag = Flag { spelling: "--file", takes: Takes::Value("PATH"), about: "read the waits somewhere other than the store" };

const COMMAND: Command = Command {
    name: "console-response-times",
    about: "what the machine has been like to wait for",
    flags: &[LAST, ALL, RAW, FILE],
    operands: Operands::None,
};

const WINDOW: u32 = 20_000;

fn main() -> ExitCode {
    match console_signals::restore_default(console_signals::Signal::PIPE) {
        Ok(()) => {},
        Err(fault) => eprintln!("console-response-times: a closed pipe will be an error rather than an ending: {fault}"),
    }

    let asked: Vec<String> = std::env::args().skip(1).collect();
    let line = match console_core_arguments::read(&COMMAND, &asked) {
        Ok(line) => line,
        Err(refusal) => {
            let Ok(code) = refusal.print();

            return ExitCode::from(code);
        }
    };
    let window = match window(&line) {
        Ok(window) => window,
        Err(refusal) => {
            let Ok(code) = refusal.print();

            return ExitCode::from(code);
        }
    };
    let Ok(raw) = line.presence(RAW);
    let Ok(file) = line.value(FILE);
    let Ok(store) = where_();
    let at = match file {
        Some(file) => Some(std::path::PathBuf::from(file)),
        None => store,
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
        Presence::Present => {
            for said in &kept {
                println!("{said}");
            }

            return ExitCode::SUCCESS;
        }
        Presence::Absent => {}
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

fn window(line: &CommandLine<NoSubcommand>) -> Result<Option<u32>, ValidationError> {
    let last = line.parsed::<u32>(LAST)?;
    let Ok(all) = line.presence(ALL);

    match (last, all) {
        (Some(many), Presence::Absent) => Ok(Some(many)),
        (None, Presence::Absent) => Ok(Some(WINDOW)),
        (None, Presence::Present) => Ok(None),
        (Some(_many), Presence::Present) => {
            let Ok(refusal) = line.refusal(Reason::ExtraArgument(ALL.spelling.to_string()));

            Err(refusal)
        }
    }
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
