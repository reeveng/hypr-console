//! The files, as an app.
//!
//! What it holds and why it is drawn this way is `console_files::card`'s own head.
//! This is the program someone types and the desktop opens, and it is an app
//! rather than a panel: it draws its own card as a window on a workspace of its
//! own, it is not one of the pickers that take turns, and a panel opened over
//! it goes away again to find the folder still where it was left. Asked for a
//! second time with nothing new, its window is brought forward and nothing
//! starts; asked for a folder, the one up stands down for it.

use std::process::ExitCode;

use console_core_arguments::{Command, Operands};
use console_panel::card::{App, open_app};

const COMMAND: Command = Command {
    name: console_files::WHO,
    about: "the files, as an app",
    flags: &[],
    operands: Operands::Optional("FOLDER"),
};

fn main() -> ExitCode {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    let Ok(code) = open_app(&asked, App { who: console_files::WHO, command: COMMAND, card: console_files::card });

    code
}
