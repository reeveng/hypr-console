//! The forecast, asked for. What it holds and why it is drawn this way is
//! `console_forecast::card`'s own head. This is the program the launcher
//! runs: it takes the screen, asks the host to draw the card on it, and
//! holds the screen until the card is gone.

use console_panel::card::{Panel, opened};

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    let Ok(()) = opened(&asked, Panel { who: console_forecast::WHO, door: console_forecast::door, card: console_forecast::card });
}
