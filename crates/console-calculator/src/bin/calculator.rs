//! The calculator, asked for. What it draws and why is
//! `console_calculator::card`'s own head.

use console_panel::card::{Panel, opened};

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    let Ok(()) = opened(&asked, Panel { who: console_calculator::WHO, door: console_calculator::door, card: console_calculator::card });
}
