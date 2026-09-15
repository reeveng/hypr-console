//! Take what was changed on the device back into this checkout.
//!
//! The device is named by `CONSOLE_HOST`, which `naming` reads and nothing else
//! does. `console_repository::pulling` may not read it because it is a
//! `console_program_contract::Program` and a value arriving through the side of
//! one is a value no transcript can put a different answer in -- and this bin
//! lives beside `naming` rather than beside `pulling` for the same reason
//! `console-deploy` and `console-migrate` do.

use std::process::ExitCode;

use console_device::naming;
use console_program_contract::Argv;
use console_program_runtime::Nothing;
use console_repository::pulling::Pull;

fn main() -> ExitCode {
    let host = match naming::device() {
        Ok(host) => host,
        Err(fault) => {
            eprintln!("{fault}");

            return ExitCode::FAILURE;
        }
    };

    let argv: Vec<&str> = match host.is_empty() {
        true => Vec::new(),
        false => vec![host.as_str()],
    };

    let Ok(argv) = Argv::of(&argv);
    let Ok(code) = console_program_runtime::run::<Pull, Nothing>("console-pull", &argv, &mut Nothing);

    code
}
