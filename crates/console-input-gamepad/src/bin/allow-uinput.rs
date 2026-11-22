//! What a laptop needs before it can pretend to be a handheld.
//!
//!     sudo cargo run --bin allow-uinput
//!  `console_input_gamepad::allowing` decides all of it. What is here is the
//! user to name, which is the one the sudo came from rather than root.

use std::process::ExitCode;

use console_core_never::Never;
use console_input_gamepad::allowing::{Allow, Allowing, Whoever};
use console_operating_system_linux::program_runtime::Pure;

fn main() -> ExitCode {
  let Ok(uid) = whoami();
  let Ok(whom) = sudoer();
  let Ok(whoever) = Whoever::of(Some(&uid));
  let opening = Allowing::Opening { whoever, whom };
  let Ok(how) = console_operating_system_linux::program_runtime::run::<Allow, Pure>(
    "allow-uinput",
    &opening,
    &mut Pure,
  );

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
  let from_sudo = match std::env::var("SUDO_USER") {
    Ok(whom) => bool::then_some(!whom.is_empty(), whom),
    Err(std::env::VarError::NotPresent | std::env::VarError::NotUnicode(_)) => None,
  };

  match from_sudo {
    Some(whom) => return Ok(Some(whom)),
    None => {}
  }

  Ok(match std::env::var("USER") {
    Ok(whom) => bool::then_some(!whom.is_empty() && whom != "root", whom),
    Err(std::env::VarError::NotPresent | std::env::VarError::NotUnicode(_)) => None,
  })
}
