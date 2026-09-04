//! What the machine has been like to wait for.
//!
//!     console-timings              everything the store holds
//!     console-timings --last 50    the last fifty waits
//!     console-timings --raw        the lines themselves, for something else to read
//!
//! The store is a line per thing waited for and it is meant to be read by
//! anything -- `jq`, a spreadsheet, a script somebody writes once. This is the
//! reading that should not have to be written twice: which surface is slow,
//! which stretch of it is the slow one, and whether the worst of them is worth
//! chasing or is one opening from a boot.

use std::process::ExitCode;

use console_timings::{line, summary, where_};

const USAGE: &str = "usage: console-timings [--last N] [--raw] [--file PATH]";

fn main() -> ExitCode {
    // Die the way every other program on a pipe dies. Rust turns the broken
    // pipe off at startup and turns a closed reader into an error on every
    // write, so `console-timings | head` ended in a panic and a backtrace
    // about stdout rather than in nothing at all -- which is what `head`
    // closing its end has always meant.
    //
    // SAFETY: one call that sets a disposition and touches nothing else.
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL) };

    let asked: Vec<String> = std::env::args().skip(1).collect();
    let mut last: Option<usize> = None;
    let mut raw = false;
    let mut at = where_();
    let mut words = asked.iter();

    while let Some(word) = words.next() {
        match word.as_str() {
            // A number that is missing and a number that is not a number are
            // two different mistakes, and telling somebody which one they made
            // is the whole of what a usage line is for.
            "--last" => match words.next().map(|many| many.parse::<usize>()) {
                Some(Ok(many)) => last = Some(many),
                Some(Err(fault)) => {
                    eprintln!("console-timings: --last: {fault}");
                    eprintln!("{USAGE}");
                    return ExitCode::FAILURE;
                }
                None => {
                    eprintln!("{USAGE}");
                    return ExitCode::FAILURE;
                }
            },
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

    let Ok(said) = std::fs::read_to_string(&at) else {
        // Nothing has been waited for yet, which is not a failure. It is what
        // a machine that has just been deployed to looks like.
        println!("nothing waited for yet: {}", at.display());
        return ExitCode::SUCCESS;
    };

    let mut lines: Vec<&str> = said.lines().collect();

    if let Some(many) = last {
        lines = lines.split_off(lines.len().saturating_sub(many));
    }

    if raw {
        for line in &lines {
            println!("{line}");
        }

        return ExitCode::SUCCESS;
    }

    let entries: Vec<line::Entry> = lines.iter().filter_map(|said| line::read(said)).collect();

    match entries.len() {
        0 => {
            println!("nothing readable in {}", at.display());
            return ExitCode::SUCCESS;
        }
        1.. => {}
    }

    for about in summary::about(&entries) {
        print!("{}", summary::told(&about));
    }

    ExitCode::SUCCESS
}
