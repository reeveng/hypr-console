//! The settings, asked for.
//!
//!     settings-panel
//!     settings-panel Sound
//!
//! What is on each tab and why they are apart is `console_settings::card`'s own
//! head. This is the program the bar's icons click and the Legion right button
//! opens: it takes the screen, asks the host to draw the settings on it, and
//! holds the screen until they are gone.

use console_never::Never;
use console_panel::chooser::{self, Alone};
use console_panel::held::{self, Drawn};
use console_panel::panel;

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    let Ok(()) = opened(&asked);
}

fn opened(asked: &[String]) -> Result<(), Never> {
    let Ok(door) = console_settings::door(asked);
    let Ok(alone) = chooser::alone(&door.name, door.again);

    match alone {
        Alone::No => return Ok(()),
        Alone::Yes => {},
    }

    let Ok(drawn) = held::stood_in(console_settings::WHO, asked);

    match drawn {
        Drawn::ByTheHost => Ok(()),
        Drawn::Here => {
            let Ok(card) = console_settings::card(asked);

            panel::drawn_here(console_settings::WHO, card)
        },
    }
}
