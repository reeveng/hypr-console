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

use console_bar_modules::notices::{Waiting, notices};
use console_bar_modules::reading::line;
use console_bar_modules::watch::{BELL, watching_notices};
use console_external_programs::Program;
use console_notifications::reading::{held_back, read};
use console_panel::door::{Up, is_open};
use console_panel::running::said;

const SETTLE: Duration = Duration::from_millis(120);

const PANEL: &str = "notices-panel";

fn main() {
    let Ok(heard) = watching_notices();
    let mut last = String::new();
    let mut open = Up::NotThere;
    let mut quiet = false;

    loop {
        let Ok(listed) = said(Program::Makoctl, &["list", "-j"]);
        let Ok(mode) = said(Program::Makoctl, &["mode"]);

        let Ok(held) = read(&listed);
        let Ok(back) = held_back(&mode);
        let Ok(waiting) = Waiting::of(&held, back);

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
                std::thread::sleep(SETTLE);

                while let Ok(()) = heard.try_recv() {}
            }
            Err(RecvTimeoutError::Timeout) => (),
            Err(RecvTimeoutError::Disconnected) => std::thread::sleep(BELL),
        }
    }
}
