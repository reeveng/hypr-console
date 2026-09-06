//! Take what was changed on the device back into this checkout.
//!
//! The device is named by `CONSOLE_HOST` and read here, because reading the
//! environment is the one thing `console_repository::pulling` may not do.

use std::process::ExitCode;

use console_program_contract::Argv;
use console_program_runtime::Nothing;
use console_repository::pulling::{HOST, Pull};

fn main() -> ExitCode {
    let host = std::env::var(HOST);
    let argv: Vec<&str> = match &host {
        Ok(host) => vec![host.as_str()],
        Err(_) => Vec::new(),
    };

    let Ok(argv) = Argv::of(&argv);
    let Ok(code) = console_program_runtime::run::<Pull, Nothing>("console-pull", &argv, &mut Nothing);

    code
}
