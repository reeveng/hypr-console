//! What a laptop needs before it can pretend to be a handheld.
//!
//!     sudo cargo run --bin allow-uinput
//!  `console_input_gamepad::allowing` decides all of it. What is here is the
//! user to name, which is the one the sudo came from rather than root.

use std::process::ExitCode;

use console_input_gamepad::allowing::Allow;
use console_core_never::Never;
use console_program_contract::Arguments;
use console_program_runtime::Pure;

fn main() -> ExitCode {
    let Ok(uid) = whoami();
    let Ok(whom) = sudoer();
    let mut arguments = vec![uid];

    match whom {
        Some(whom) => arguments.extend(["--for".to_string(), whom]),
        None => {},
    }

    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let Ok(arguments) = Arguments::of(&arguments);
    let Ok(how) = console_program_runtime::run::<Allow, Pure>("allow-uinput", &arguments, &mut Pure);

    how
}

fn whoami() -> Result<String, Never> {
    let uid = rustix::process::getuid().as_raw();

    Ok(uid.to_string())
}

#[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
        explicit026_env_read_once,
        reason = "SUDO_USER and USER are how a program run under sudo finds out whose desktop it is fixing, and this binary is the only one here that runs that way"
    )
)]
fn sudoer() -> Result<Option<String>, Never> {
    match std::env::var("SUDO_USER") {
        Ok(whom) => match whom.is_empty() {
            true => {},
            false => return Ok(Some(whom)),
        },
        Err(_unset) => {},
    }

    Ok(match std::env::var("USER") {
        Ok(whom) => match !whom.is_empty() && whom != "root" {
            true => Some(whom),
            false => None,
        },
        Err(_unset) => None,
    })
}
