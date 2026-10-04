//! Move a device that is still called legion over to the console names.
//!
//! The device is named by `CONSOLE_HOST` and read here, because reading the
//! environment is the one thing `console_device::migrating` may not do. The
//! top of the tree is found here for the same reason: what is pushed is this
//! checkout's history, and `git` run from three directories down would push
//! someone else's.

use std::process::ExitCode;

use console_core_arguments::read;
use console_device::migrating::{COMMAND, Migrate, Migration};
use console_device_name::device;
use console_program_runtime::Pure;

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "the first thing this does, before a relative path has meant anything: what follows is git and cargo, which are run inside a tree rather than handed one, and `console_repository` is what says which tree that is"
    )
)]
fn main() -> ExitCode {
    let host = match device() {
        Ok(host) => host,
        Err(fault) => {
            eprintln!("console-migrate: {fault}");

            return ExitCode::FAILURE;
        }
    };
    let words: Vec<String> = std::env::args().skip(1).collect();

    let line = match read(&COMMAND, &words) {
        Ok(line) => line,
        Err(refusal) => {
            let Ok(code) = refusal.print();

            return ExitCode::from(code);
        }
    };
    let Ok(migration) = Migration::of(&host, &line);

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

    let Ok(how) = console_program_runtime::run::<Migrate, Pure>(
        "console-migrate",
        &migration,
        &mut Pure,
    );

    how
}
