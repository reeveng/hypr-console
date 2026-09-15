//! The clock in the middle of the bar.
//!
//!     bar-clock
//!
//! A line whenever the words change, which is what a waybar custom module
//! reads. What decides the words and what decides when they change is
//! `console_status_bar::clock`, which the bar draws out of directly; this is
//! that module with a `println!` under it, for as long as waybar is the thing
//! drawing.

use std::io::Write;
use std::process::ExitCode;
use std::time::Duration;

use console_status_bar::clock::{now, standing};
use console_waiting::{Patience, Seen, until};

const ASKING_EVERY: Duration = Duration::from_millis(500);

const LONGEST: Duration = Duration::from_secs(60);

fn main() -> ExitCode {
    let Ok(patience) = Patience::asking_every(LONGEST, ASKING_EVERY);
    let Ok(mut was) = standing();
    let mut last = String::new();

    loop {
        let Ok(says) = now();

        match says == last {
            true => {},
            false => {
                println!("{says}");

                let _ = std::io::stdout().flush();

                last = says;
            }
        }

        let Ok(_) = until(patience, || {
            let Ok(now) = standing();

            Ok(match now == was {
                true => Seen::NotYet,
                false => Seen::Yes,
            })
        });

        let Ok(now) = standing();

        was = now;
    }
}
