//! The menu, asked for.
//!
//! What it is and why it is ours is `console_launcher`'s own head. This is the
//! program somebody types, the bar clicks, the paddle opens and the compositor
//! binds a key to: it takes the screen, asks the host to draw the menu on it,
//! and holds the screen until the menu is gone.

use console_never::Never;
use console_panel::chooser::{self, Alone};
use console_panel::held::{self, Drawn};
use console_panel::panel;

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    let Ok(()) = opened(&asked);
}

fn opened(asked: &[String]) -> Result<(), Never> {
    let Ok(door) = console_launcher::door(asked);
    let Ok(alone) = chooser::alone(&door.name, door.again);

    match alone {
        Alone::No => return Ok(()),
        Alone::Yes => {},
    }

    let Ok(drawn) = held::stood_in(console_launcher::WHO, asked);

    match drawn {
        Drawn::ByTheHost => Ok(()),
        Drawn::Here => {
            let Ok(card) = console_launcher::card(asked);

            panel::drawn_here(console_launcher::WHO, card)
        },
    }
}
