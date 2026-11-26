//! Raise the on-screen keyboard, and leave it alone if it is already up.
//!
//! Not a toggle, on purpose: a program asking for a keyboard on someone's
//! behalf must not take one away. `console_input_keyboard::remote` says why at
//! length.

use std::process::ExitCode;

use console_core_internal_programs::KEYBOARD_AUTOMATIC_SHOW;
use console_default_applications::keyboard::{self, Automatic};
use console_input_keyboard::remote::{Sending, Show};

fn main() -> ExitCode {
  let asked = std::env::args().nth(1);
  let Ok(automatic) = keyboard::automatic();

  match (
    asked.as_deref() == Some(KEYBOARD_AUTOMATIC_SHOW.spelling),
    automatic,
  ) {
    (true, Automatic::Off) => {
      return ExitCode::SUCCESS;
    }
    (false, _) | (true, Automatic::On) => {}
  }

  let Ok(code) = console_operating_system_linux::program_runtime::run::<Show, Sending>(
    "keyboard-show",
    &(),
    &mut Sending,
  );

  code
}
