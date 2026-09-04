//! What the desktop has said and nobody has cleared.
//!
//!     bar-notice
//!
//! A line of JSON whenever the answer changes, which is what a waybar custom
//! module reads. It takes no argument: there is one bell and it counts one
//! thing.

use std::io::Write;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

use console_bar::notices::{Waiting, notices};
use console_bar::reading::line;
use console_bar::watch::{BELL, watching_notices};
use console_notices::reading::{held_back, read};
use console_panel::door::{Up, is_open};
use console_panel::running::said;

/// How long a wake is left to settle before the count is taken again.
///
/// One notification is a call, a reply and a signal or two, and each of them
/// is a line off the monitor. Counted on every one, mako is asked four times
/// for an answer that did not change for three of them, and asking is a
/// subprocess. Waiting a moment and then taking whatever else arrived in it
/// makes that one question.
const SETTLE: Duration = Duration::from_millis(120);

/// The panel the bell opens, as the compositor lists it.
const PANEL: &str = "notices-panel";

fn main() {
    let heard = watching_notices();
    let mut last = String::new();
    let mut open = Up::NotThere;
    let mut quiet = false;

    loop {
        let waiting = Waiting::of(
            &read(&said(&["makoctl", "list", "-j"])),
            held_back(&said(&["makoctl", "mode"])),
        );

        // Lit while what it opens is in front, the same as every other icon on
        // this bar that opens something. It is one control for opening and
        // closing, so it has to say which a tap will do.
        //
        // A compositor that will not answer is not news about the panel, so
        // the icon holds the reading it has. Said once rather than once a
        // pass, which would be a journal nobody can read.
        match is_open(PANEL) {
            Ok(up) => {
                open = up;
                quiet = false;
            },
            Err(why) => {
                if !quiet {
                    eprintln!("bar-notice: the compositor would not say what is up: {why}");
                    quiet = true;
                }
            },
        }

        let said = line(&notices(waiting), open);

        if said != last {
            println!("{said}");
            let _ = std::io::stdout().flush();
            last = said;
        }

        match heard.recv_timeout(BELL) {
            Ok(()) => {
                std::thread::sleep(SETTLE);

                while let Ok(()) = heard.try_recv() {}
            }
            Err(RecvTimeoutError::Timeout) => (),
            // The monitor has gone. The tick is all that is left, and without
            // this the receiver answers at once and spins.
            Err(RecvTimeoutError::Disconnected) => std::thread::sleep(BELL),
        }
    }
}
