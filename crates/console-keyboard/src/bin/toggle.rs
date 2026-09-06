//! Show or hide the on-screen keyboard. Bound to X on the controller.
//!
//! Everything it decides is in `console_keyboard::asked`, where it can be
//! asked the same question twice. What is here is a real process to signal.

use std::process::ExitCode;

use console_keyboard::asked::Toggle;
use console_program_contract::Argv;
use console_program_runtime::Nothing;

fn main() -> ExitCode {
    let Ok(code) = console_program_runtime::run::<Toggle, Nothing>(
        "keyboard-toggle",
        &Argv::default(),
        &mut Nothing,
    );

    code
}
