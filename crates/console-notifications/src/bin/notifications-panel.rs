//! What the desktop has said, asked for.
//!
//! What it holds and why it is drawn this way is `console_notifications::card`'s own head.
//! This is the program someone types and the desktop opens: it takes the
//! screen, asks the host to draw the card on it, and holds the screen until
//! the card is gone.

use console_panel::card::{Panel, opened};

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    let Ok(()) = opened(&asked, Panel { who: console_notifications::WHO, door: console_notifications::door, card: console_notifications::card });
}
