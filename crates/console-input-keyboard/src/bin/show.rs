//! Raise the on-screen keyboard, and leave it alone if it is already up.
//!
//! Not a toggle, on purpose: a program asking for a keyboard on someone's
//! behalf must not take one away. `console_input_keyboard::remote` says why at
//! length.

use std::process::ExitCode;

use console_input_keyboard::remote::{Show, Sending};
use console_program_contract::Arguments;

fn main() -> ExitCode {
    let Ok(code) =
        console_program_runtime::run::<Show, Sending>("keyboard-show", &Arguments::default(), &mut Sending);

    code
}
