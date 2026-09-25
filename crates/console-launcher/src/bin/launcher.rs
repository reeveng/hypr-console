//! The menu, asked for.
//!
//! What it is and why it is ours is `console_launcher`'s own head. This is the
//! program someone types, the bar clicks, the paddle opens and the compositor
//! binds a key to: it takes the screen, asks the host to draw the menu on it,
//! and holds the screen until the menu is gone.

use console_panel::card::{Panel, opened};

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    let Ok(()) = opened(&asked, Panel { who: console_launcher::WHO, door: console_launcher::door, card: console_launcher::card });
}
