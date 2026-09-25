//! Show or hide the on-screen keyboard. Bound to X on the controller.
//!
//! Everything it decides is in `console_input_keyboard::remote`, where it can be
//! asked the same question twice. What is here is a real process to signal.

use std::process::ExitCode;

use console_input_keyboard::remote::Toggle;
use console_program_contract::Arguments;
use console_program_runtime::Pure;

fn main() -> ExitCode {
    let Ok(code) = console_program_runtime::run::<Toggle, Pure>(
        "keyboard-toggle",
        &Arguments::default(),
        &mut Pure,
    );

    code
}
