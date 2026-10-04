//! The calculator, asked for. What it draws and why is
//! `console_calculator::card`'s own head.

use std::process::ExitCode;

use console_panel::card::{Panel, opened};

fn main() -> ExitCode {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    let Ok(code) = opened(&asked, Panel { who: console_calculator::WHO, command: console_calculator::COMMAND, door: console_calculator::door, card: console_calculator::card });

    code
}
