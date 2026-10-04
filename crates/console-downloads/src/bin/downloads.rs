//! The downloads, as an app.
//!
//! What it holds and why it is drawn this way is `console_downloads::card`'s
//! own head. This is the program someone types and the desktop opens, and the
//! book store the library's last cover leads to. It is an app rather than a
//! panel: drawn by this process as a window on a workspace of its own, and
//! still there with its search and what it found when a panel opened over it
//! goes.

use std::process::ExitCode;

use console_core_arguments::{Command, Operands};
use console_panel::card::{App, open_app};

const COMMAND: Command = Command {
    name: console_downloads::WHO,
    about: "the downloads, as an app",
    flags: &[],
    operands: Operands::Optional("TAB"),
};

fn main() -> ExitCode {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    let Ok(code) = open_app(&asked, App { who: console_downloads::WHO, command: COMMAND, card: console_downloads::card });

    code
}
