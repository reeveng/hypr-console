//! The on-screen keyboard, started with the palette in its hands.
//!
//! It exists so the unit file can be about starting a program. What colour
//! each thing is is `console_keyboard`, which can be asked without a keyboard
//! to start.
//!
//! This becomes wvkbd rather than starting it. `osk` toggles the keyboard by
//! sending SIGRTMIN to a process called `wvkbd-mobintl`, and the unit tracks a
//! main PID. A parent left sitting over it would be the process both of those
//! were aiming at, and neither would reach the keyboard.

use std::os::unix::process::CommandExt;
use std::process::Command;

use console_colour::spent::{SPENT, read};
use console_keyboard::{argv, missing};

fn main() -> std::process::ExitCode {
    let at = std::path::Path::new("/").join(SPENT);
    let held = match std::fs::read_to_string(&at) {
        Ok(held) => held,
        Err(why) => {
            eprintln!("osk-start: no palette at {}: {why}", at.display());
            String::new()
        }
    };
    let palette = read(&held);

    // Said rather than refused. A keyboard in the wrong colours is worse than
    // one in the right colours and better than no keyboard at all, and this is
    // the only surface on the machine a person cannot type without.
    let missing = missing(&palette);
    if !missing.is_empty() {
        eprintln!(
            "osk-start: the palette has no {}, so the keyboard keeps whatever colour wvkbd was \
             compiled with for those",
            missing.join(", ")
        );
    }

    let rest: Vec<String> = std::env::args().skip(1).collect();
    let argv = argv(&palette, &rest);
    let why = Command::new(&argv[0]).args(&argv[1..]).exec();
    eprintln!("osk-start: no keyboard to start at {}: {why}", argv[0]);
    std::process::ExitCode::FAILURE
}
