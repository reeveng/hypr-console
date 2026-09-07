//! What the machine has been like to wait for.
//!
//!     console-response-times              the last few thousand waits
//!     console-response-times --last 50    the last fifty
//!     console-response-times --all        every one the store holds
//!     console-response-times --raw        the lines themselves, for something else to read
//!
//! The store is a line per thing waited for and it is meant to be read by
//! anything -- `jq`, a spreadsheet, a script somebody writes once. This is the
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

use console_core_never::Never;
use console_response_times::{line, summary, where_};

const USAGE: &str = "usage: console-response-times [--last N] [--all] [--raw] [--file PATH]";

const WINDOW: usize = 20_000;

fn main() -> ExitCode {
    // SAFETY: one call that sets a disposition and touches nothing else.
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL) };

    let asked: Vec<String> = std::env::args().skip(1).collect();
    let mut window = Some(WINDOW);
    let mut raw = false;
    let Ok(mut at) = where_();

    let mut words = asked.iter();

    while let Some(word) = words.next() {
        match word.as_str() {
            "--last" => match words.next().map(|many| many.parse::<usize>()) {
                Some(Ok(many)) => window = Some(many),
                Some(Err(fault)) => {
                    eprintln!("console-response-times: --last: {fault}");
                    eprintln!("{USAGE}");
                    return ExitCode::FAILURE;
                }
                None => {
                    eprintln!("{USAGE}");
                    return ExitCode::FAILURE;
                }
            },
            "--all" => window = None,
            "--file" => match words.next() {
                Some(path) => at = path.into(),
                None => {
                    eprintln!("{USAGE}");
                    return ExitCode::FAILURE;
                }
            },
            "--raw" => raw = true,
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::FAILURE;
            }
        }
    }

    let Ok(store) = File::open(&at) else {
        println!("nothing waited for yet: {}", at.display());
        return ExitCode::SUCCESS;
    };

    let Ok((kept, whole)) = held(store, window);

    match raw {
        true => {
            for said in &kept {
                println!("{said}");
            }

            return ExitCode::SUCCESS;
        }
        false => {}
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

    match whole > kept.len() {
        true => println!("the last {} waits of {whole}. --all reads the rest\n", kept.len()),
        false => {}
    }

    let Ok(gathered) = summary::about(&entries);

    for about in gathered {
        let Ok(said) = summary::told(&about);

        print!("{said}");
    }

    ExitCode::SUCCESS
}

fn held(store: File, window: Option<usize>) -> Result<(VecDeque<String>, usize), Never> {
    let mut kept: VecDeque<String> = VecDeque::new();
    let mut whole: usize = 0;

    for said in BufReader::new(store).lines() {
        let Ok(said) = said else { continue };

        whole = whole.saturating_add(1);
        kept.push_back(said);

        match window {
            Some(many) => match kept.len() > many {
                true => {
                    let _ = kept.pop_front();
                }
                false => {}
            },
            None => {}
        }
    }

    Ok((kept, whole))
}
