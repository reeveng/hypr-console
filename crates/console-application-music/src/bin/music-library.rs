//! The management app shares playback with Console Music and the Now Playing panel.

use std::process::ExitCode;

use console_core_arguments::{Command, Operands};
use console_panel::card::{App, open_app};

const COMMAND: Command = Command {
  name: env!("CARGO_BIN_NAME"),
  about: "browse albums and review recordings in Music Library",
  flags: &[],
  operands: Operands::None,
};

fn main() -> ExitCode {
  let asked: Vec<String> = std::env::args().skip(1).collect();
  let Ok(code) = open_app(
    &asked,
    App {
      who: "music-library",
      command: COMMAND,
      card: console_application_music::card::management,
    },
  );

  code
}
