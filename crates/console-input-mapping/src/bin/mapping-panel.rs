//! The button layout, asked for.  What it holds and why it is drawn this way is
//! `console_input_mapping::card`'s own head. This is the program someone types
//! and the desktop opens: it takes the screen, asks the host to draw the card
//! on it, and holds the screen until the card is gone.

use std::process::ExitCode;

use console_panel::card::{Panel, opened};

fn main() -> ExitCode {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    let Ok(code) = opened(&asked, Panel { who: console_input_mapping::WHO, command: console_input_mapping::COMMAND, door: console_input_mapping::door, card: console_input_mapping::card });

    code
}
