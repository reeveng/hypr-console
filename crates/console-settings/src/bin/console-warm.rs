//! Warm colours on the screen, and the memory of whether they are wanted.
//!
//!     console-warm            turn them on, or off again
//!     console-warm get        which way the switch is standing
//!     console-warm again      tell the daemon what was already decided
//!
//! `hyprsunset` holds the colour and forgets it when it stops, so `again` is
//! what the unit runs after starting it: a machine that was warm last night is
//! warm when it comes back, without anybody pressing anything.
//!
//! What warm is, and where the answer lives, is `console_settings::warm`.

use std::process::{Command, ExitCode};

use console_settings::warm::{Warmth, at};

fn main() -> ExitCode {
    let word = std::env::args().nth(1).unwrap_or_default();
    let Ok(home) = std::env::var("HOME") else {
        eprintln!("console-warm: no HOME, so there is nobody to remember for");
        return ExitCode::FAILURE;
    };
    let at = at(&home);
    let standing = Warmth::read(&std::fs::read_to_string(&at).unwrap_or_default());

    let wanted = match word.as_str() {
        "get" => {
            println!("{}", standing.written().trim());
            return ExitCode::SUCCESS;
        }
        // Said again rather than decided again. This runs when the daemon has
        // just started and is therefore wearing nothing, which is not the same
        // as somebody having asked for nothing.
        "again" => standing,
        "" => standing.other(),
        _ => {
            eprintln!("usage: console-warm [get|again]");
            return ExitCode::from(2);
        }
    };

    // The daemon is told first, and what it took is what gets written down.
    //
    // The other way round -- remember, then tell -- was written first and the
    // device showed why it is wrong within a minute: hyprsunset was not
    // listening, the file said warm, and the switch on the panel drew "warm"
    // over a screen that was daylight. A switch that lies about the machine is
    // worse than one that did not work, because the second is obvious and the
    // first sends somebody looking at the screen instead of at the daemon.
    //
    // Tried more than once, and only for `again`, which the unit runs the
    // moment hyprsunset is started: a daemon that has been started is not yet
    // a daemon with a socket to be told anything on. Three seconds is far less
    // than a person would wait before deciding the switch is broken, and a
    // press by hand never reaches it because by then the daemon has been up as
    // long as the desktop has.
    let tries = match word.as_str() {
        "again" => 30,
        _ => 1,
    };
    let told = wanted.told();
    let mut took = false;
    for _ in 0..tries {
        took = Command::new(&told[0]).args(&told[1..]).status().is_ok_and(|how| how.success());
        if took {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    if !took {
        eprintln!(
            "console-warm: hyprsunset did not take it, so the screen is still {}",
            standing.written().trim()
        );
        return ExitCode::FAILURE;
    }

    // Nothing to write for `again`: it said what was already decided, and
    // writing it back would be the program agreeing with itself.
    if word == "again" {
        return ExitCode::SUCCESS;
    }
    if let Some(holding) = at.parent()
        && (std::fs::create_dir_all(holding).is_err()
            || std::fs::write(&at, wanted.written()).is_err())
    {
        // The screen is warm and the file does not know. Said rather than
        // swallowed, because the next restart will put it back the way the
        // file remembers and somebody should know why.
        eprintln!("console-warm: the screen took it, but {} could not be written", at.display());
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
