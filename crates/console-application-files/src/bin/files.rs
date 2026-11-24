//! The files, as an app.
//!
//! What it holds and why it is drawn this way is `console_application_files::card`'s own head.
//! This is the program someone types and the desktop opens, and it is an app
//! rather than a panel: it draws its own card as a window on a workspace of its
//! own, it is not one of the pickers that take turns, and a panel opened over
//! it goes away again to find the folder still where it was left. Asked for a
//! second time with nothing new, its window is brought forward and nothing
//! starts; asked for a folder, the one up stands down for it.

use std::process::ExitCode;

use console_panel::card::{App, open_app};

fn main() -> ExitCode {
  let asked: Vec<String> = std::env::args().skip(1).collect();

  let Ok(code) = open_app(
    &asked,
    App {
      who: console_application_files::WHO,
      command: console_application_files::COMMAND,
      card: console_application_files::card,
    },
  );

  let Ok(()) = console_core_places::session::forget();
  code
}
