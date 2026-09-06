//! The music, asked for.
//!
//! What it holds and why it is drawn this way is `console_music::card`'s own head.
//! This is the program somebody types and the desktop opens: it takes the
//! screen, asks the host to draw the card on it, and holds the screen until
//! the card is gone.

use console_never::Never;
use console_panel::chooser::{self, Alone};
use console_panel::held::{self, Drawn};
use console_panel::panel;

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    let Ok(()) = opened(&asked);
}

fn opened(asked: &[String]) -> Result<(), Never> {
    let Ok(door) = console_music::door(asked);
    let Ok(alone) = chooser::alone(&door.name, door.again);

    match alone {
        Alone::No => return Ok(()),
        Alone::Yes => {},
    }

    let Ok(drawn) = held::stood_in(console_music::WHO, asked);

    match drawn {
        Drawn::ByTheHost => Ok(()),
        Drawn::Here => {
            let Ok(card) = console_music::card(asked);

            panel::drawn_here(console_music::WHO, card)
        },
    }
}
