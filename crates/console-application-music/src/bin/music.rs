//! The music, as an app.
//!
//! What it holds and why it is drawn this way is `console_application_music::card`'s own
//! head. This is the library someone opens from the home screen and the menu,
//! drawn by this process as a window on a workspace of its own; the song on now
//! is also the panel the bar opens, which is `music-panel`.

use std::process::ExitCode;

use console_core_arguments::{Command, Operands};
use console_panel::card::{App, open_app};

const COMMAND: Command = Command {
  name: env!("CARGO_BIN_NAME"),
  about: "the music, as an app",
  flags: &[],
  operands: Operands::None,
};

fn main() -> ExitCode {
  let asked: Vec<String> = std::env::args().skip(1).collect();

  let Ok(code) = open_app(
    &asked,
    App {
      who: console_application_music::APP,
      command: COMMAND,
      card: console_application_music::library,
    },
  );

  code
}
