//! Take what was changed on the device back into this checkout.
//!
//! The device is named by `CONSOLE_HOST`, which `console-device-name` reads and
//! nothing else does. `pulling` may not read it because it is a
//! `console_program_contract::Program` and a value arriving through the side of
//! one is a value no transcript can put a different answer in -- which is the
//! same reason `console-deploy` and `console-migrate` read it here too.

use std::process::ExitCode;

use console_device_name as naming;
use console_program_contract::Arguments;
use console_program_runtime::Pure;
use console_device::pulling::Pull;

fn main() -> ExitCode {
    let host = match naming::device() {
        Ok(host) => host,
        Err(fault) => {
            eprintln!("{fault}");

            return ExitCode::FAILURE;
        }
    };

    let arguments: Vec<&str> = match host.is_empty() {
        true => Vec::new(),
        false => vec![host.as_str()],
    };

    let Ok(arguments) = Arguments::of(&arguments);
    let Ok(code) = console_program_runtime::run::<Pull, Pure>("console-pull", &arguments, &mut Pure);

    code
}
