//! The notes, asked for. What it draws and why is `console_notes::card`'s own
//! head.

use std::process::ExitCode;

use console_panel::card::{Panel, opened};

fn main() -> ExitCode {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    let Ok(code) = opened(&asked, Panel { who: console_notes::WHO, command: console_notes::COMMAND, door: console_notes::door, card: console_notes::card });

    code
}
