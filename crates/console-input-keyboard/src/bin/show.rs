//! Raise the on-screen keyboard, and leave it alone if it is already up.
//!
//! Not a toggle, on purpose: a program asking for a keyboard on somebody's
//! behalf must not take one away. `console_input_keyboard::asked` says why at
//! length.

use std::process::ExitCode;

use console_input_keyboard::asked::Show;
use console_program_contract::Argv;
use console_program_runtime::Nothing;

fn main() -> ExitCode {
    let Ok(code) =
        console_program_runtime::run::<Show, Nothing>("keyboard-show", &Argv::default(), &mut Nothing);

    code
}
