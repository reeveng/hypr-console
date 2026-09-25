//! The music, asked for.
//!
//! What it holds and why it is drawn this way is `console_music::card`'s own head.
//! This is the program someone types and the desktop opens: it takes the screen,
//! asks the host to draw the card on it, and holds the screen until the card is
//! gone.

use console_panel::card::{Panel, opened};

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    let Ok(()) = opened(&asked, Panel { who: console_music::WHO, door: console_music::door, card: console_music::card });
}
