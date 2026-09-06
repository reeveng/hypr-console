//! Move a device that is still called legion over to the console names.
//!
//! The device is named by `CONSOLE_HOST` and read here, because reading the
//! environment is the one thing `console_device::migrating` may not do. The
//! top of the tree is found here for the same reason: what is pushed is this
//! checkout's history, and `git` run from three directories down would push
//! somebody else's.

use std::process::ExitCode;

use console_device::migrating::Migrate;
use console_device::naming::device;
use console_program_contract::Argv;
use console_program_runtime::Nothing;

fn main() -> ExitCode {
    let host = match device() {
        Ok(host) => host,
        Err(fault) => {
            eprintln!("console-migrate: {fault}");

            return ExitCode::FAILURE;
        }
    };
    let mut words = vec![host];

    words.extend(std::env::args().skip(1));

    let given: Vec<&str> = words.iter().map(String::as_str).collect();

    match console_repository::root() {
        Ok(root) => match std::env::set_current_dir(&root) {
            Ok(()) => {},
            Err(fault) => {
                eprintln!("console-migrate: {}: {fault}", root.display());

                return ExitCode::FAILURE;
            }
        },
        Err(fault) => {
            eprintln!("console-migrate: {fault}");

            return ExitCode::FAILURE;
        }
    }

    let Ok(argv) = Argv::of(&given);
    let Ok(how) = console_program_runtime::run::<Migrate, Nothing>(
        "console-migrate",
        &argv,
        &mut Nothing,
    );

    how
}
