//! A chooser a test can be the second of.
//!
//! The lock is between processes, so nothing inside one process proves anything
//! about it. This is the other process. It takes the screen and keeps it, or it
//! asks for the screen once and says what it was told.
//!
//! It says "held" on the way in rather than being given a moment, because
//! waiting for a line is waiting for the lock and waiting for a moment is a test
//! that fails on a busy machine.

use console_panel::chooser::{Again, Alone, alone, drawn, gone};

/// Long enough that the test decides when it ends, and short enough that one
/// left behind is gone before anybody notices.
const KEPT: std::time::Duration = std::time::Duration::from_secs(30);

/// How long the one on its way takes to get there.
const DRAWING: std::time::Duration = std::time::Duration::from_millis(200);

/// How long the last of one takes, once its window has gone.
const GOING: std::time::Duration = std::time::Duration::from_millis(300);

fn said(so: Alone) -> &'static str {
    match so {
        Alone::Yes => "yes",
        Alone::No => "no",
    }
}

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();
    let name = asked.get(1).cloned().unwrap_or_default();

    match asked.first().map(String::as_str) {
        Some("hold") => {
            assert_eq!(alone(&name, Again::Closes), Alone::Yes, "something was already holding it");
            drawn();
            println!("held");
            std::thread::sleep(KEPT);
        }
        // Holding the screen without having drawn on it yet, which is every
        // chooser between the button and the first frame.
        Some("coming") => {
            assert_eq!(alone(&name, Again::Closes), Alone::Yes, "something was already holding it");
            println!("held");
            std::thread::sleep(DRAWING);
            drawn();
            std::thread::sleep(KEPT);
        }
        // One whose window has gone and which is still holding the lock,
        // which is every chooser for the last of its life.
        Some("going") => {
            assert_eq!(alone(&name, Again::Closes), Alone::Yes, "something was already holding it");
            drawn();
            gone();
            println!("held");
            std::thread::sleep(GOING);
        }
        // One that took the screen and never draws on it. Nothing should be
        // able to shut the screen for the rest of the session.
        Some("stuck") => {
            assert_eq!(alone(&name, Again::Closes), Alone::Yes, "something was already holding it");
            println!("held");
            std::thread::sleep(KEPT);
        }
        // Asking is not taking, so the one holding it may ask again.
        Some("twice") => println!(
            "{} {}",
            said(alone(&name, Again::Closes)),
            said(alone(&name, Again::Closes))
        ),
        _ => println!("{}", said(alone(&name, Again::Closes))),
    }
}
