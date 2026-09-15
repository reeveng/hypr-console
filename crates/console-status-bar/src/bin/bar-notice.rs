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

use console_status_bar::notices::{Waiting, notices};
use console_status_bar::reading::line;
use console_status_bar::watch::{BELL, watching_notices};
use console_notifications::serving::held;
use console_panel::door::{Up, is_open};

const SETTLE: Duration = Duration::from_millis(120);

const PANEL: &str = "notifications-panel";

fn main() {
    let Ok(heard) = watching_notices();
    let mut last = String::new();
    let mut open = Up::NotThere;
    let mut quiet = false;

    loop {
        let Ok(whole) = held();
        let Ok(waiting) = Waiting::of(&whole.waiting, whole.quiet);

        match is_open(PANEL) {
            Ok(up) => {
                open = up;
                quiet = false;
            },
            Err(why) => {
                match quiet {
                    true => {}
                    false => {
                        eprintln!(
                            "bar-notice: the compositor would not say what is up: {why}"
                        );
                        quiet = true;
                    }
                }
            },
        }

        let Ok(says) = notices(waiting);
        let Ok(said) = line(&says, open);

        match said == last {
            true => {}
            false => {
                println!("{said}");
                let _ = std::io::stdout().flush();
                last = said;
            }
        }

        match heard.recv_timeout(BELL) {
            Ok(()) => {
                #[cfg_attr(
                    dylint_lib = "explicit021_no_sleeping",
                    allow(
                        explicit021_no_sleeping,
                        reason = "a burst of notices arrives as many words about one change; this is the window they are gathered in, and the thing being waited for is the burst stopping"
                    )
                )]
                std::thread::sleep(SETTLE);

                while let Ok(()) = heard.try_recv() {}
            }
            Err(RecvTimeoutError::Timeout) => (),
            #[cfg_attr(
                dylint_lib = "explicit021_no_sleeping",
                allow(
                    explicit021_no_sleeping,
                    reason = "nothing is left to say when a notice arrives, so there is no longer a thing to wait for; the bar falls back to a cadence rather than spinning on a dead channel"
                )
            )]
            Err(RecvTimeoutError::Disconnected) => std::thread::sleep(BELL),
        }
    }
}
