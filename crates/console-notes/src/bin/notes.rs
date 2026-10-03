//! The notes, asked for. What it draws and why is `console_notes::card`'s own
//! head.

use console_panel::card::{Panel, opened};

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    let Ok(()) = opened(&asked, Panel { who: console_notes::WHO, door: console_notes::door, card: console_notes::card });
}
