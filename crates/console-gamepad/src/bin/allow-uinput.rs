//! What a laptop needs before it can pretend to be a handheld.
//!
//!     sudo cargo run --bin allow-uinput
//!
//! `console_gamepad::allowing` decides all of it. What is here is the user to
//! name, which is the one the sudo came from rather than root.

use std::process::ExitCode;

use console_gamepad::allowing::Allow;
use console_never::Never;
use console_program_contract::Argv;
use console_program_runtime::Nothing;

fn main() -> ExitCode {
    let Ok(uid) = whoami();
    let Ok(whom) = sudoer();
    let mut argv = vec![uid];

    match whom {
        Some(whom) => argv.extend(["--for".to_string(), whom]),
        None => {},
    }

    let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
    let Ok(argv) = Argv::of(&argv);
    let Ok(how) = console_program_runtime::run::<Allow, Nothing>("allow-uinput", &argv, &mut Nothing);

    how
}

fn whoami() -> Result<String, Never> {
    // SAFETY: getuid reads this process's own real user id and touches nothing else.
    let uid = unsafe { libc::getuid() };

    Ok(uid.to_string())
}

fn sudoer() -> Result<Option<String>, Never> {
    match std::env::var("SUDO_USER") {
        Ok(whom) if !whom.is_empty() => return Ok(Some(whom)),
        Ok(_) | Err(_) => {},
    }

    Ok(match std::env::var("USER") {
        Ok(whom) if !whom.is_empty() && whom != "root" => Some(whom),
        Ok(_) | Err(_) => None,
    })
}
