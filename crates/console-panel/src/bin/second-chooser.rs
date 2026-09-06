//! A chooser a test can be the second of.
//!
//! The lock is between processes, so nothing inside one process proves anything
//! about it. This is the other process. It takes the screen and keeps it, or it
//! asks for the screen once and says what it was told.
//!
//! It says "held" on the way in rather than being given a moment, because
//! waiting for a line is waiting for the lock and waiting for a moment is a test
//! that fails on a busy machine.

use console_never::Never;
use console_panel::chooser::{Again, Alone, alone, drawn, gone};

const KEPT: std::time::Duration = std::time::Duration::from_secs(30);

const DRAWING: std::time::Duration = std::time::Duration::from_millis(200);

const GOING: std::time::Duration = std::time::Duration::from_millis(300);

fn said(so: Alone) -> Result<&'static str, Never> {
    Ok(match so {
        Alone::Yes => "yes",
        Alone::No => "no",
    })
}

fn took(name: &str) -> Result<(), Never> {
    let Ok(alone) = alone(name, Again::Closes);

    assert_eq!(alone, Alone::Yes, "something was already holding it");

    Ok(())
}

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();
    let name = asked.get(1).cloned().unwrap_or_default();

    match asked.first().map(String::as_str) {
        Some("hold") => {
            let Ok(()) = took(&name);
            let Ok(()) = drawn();

            println!("held");
            std::thread::sleep(KEPT);
        }
        Some("coming") => {
            let Ok(()) = took(&name);

            println!("held");
            std::thread::sleep(DRAWING);

            let Ok(()) = drawn();

            std::thread::sleep(KEPT);
        }
        Some("going") => {
            let Ok(()) = took(&name);
            let Ok(()) = drawn();
            let Ok(()) = gone();

            println!("held");
            std::thread::sleep(GOING);
        }
        Some("stuck") => {
            let Ok(()) = took(&name);

            println!("held");
            std::thread::sleep(KEPT);
        }
        Some("twice") => {
            let Ok(one) = alone(&name, Again::Closes);
            let Ok(other) = alone(&name, Again::Closes);
            let Ok(one) = said(one);
            let Ok(other) = said(other);

            println!("{one} {other}");
        }
        Some(_) | None => {
            let Ok(so) = alone(&name, Again::Closes);
            let Ok(so) = said(so);

            println!("{so}");
        }
    }
}
