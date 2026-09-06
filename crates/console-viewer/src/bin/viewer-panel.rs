//! A picture or a film, asked for.
//!
//!     viewer-panel FILE-OR-FOLDER
//!
//! What it shows and how it is driven is `console_viewer::card`'s own head.
//! This is the program the files panel opens and a `.desktop` file names.
//!
//! It is the one panel that can answer a press by not opening at all, because
//! what it was handed may be neither a picture nor a film -- and that has to be
//! decided here rather than in the host, before the screen is taken. Whoever is
//! looking gets the folder read twice for it, once to find out there is
//! something to show and once to show it, which is the cheapest of the three
//! panels to open and the rarest.

use console_never::Never;
use console_panel::chooser::{self, Alone};
use console_panel::held::{self, Drawn};
use console_panel::panel;
use console_viewer::card::Worth;

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    let Ok(()) = opened(&asked);
}

fn opened(asked: &[String]) -> Result<(), Never> {
    let Ok(worth) = console_viewer::card::worth_opening(asked);

    match worth {
        Worth::Nothing => return Ok(()),
        Worth::Opening => {},
    }

    let Ok(door) = console_viewer::door(asked);
    let Ok(alone) = chooser::alone(&door.name, door.again);

    match alone {
        Alone::No => return Ok(()),
        Alone::Yes => {},
    }

    let Ok(drawn) = held::stood_in(console_viewer::WHO, asked);

    match drawn {
        Drawn::ByTheHost => Ok(()),
        Drawn::Here => {
            let Ok(card) = console_viewer::card(asked);

            panel::drawn_here(console_viewer::WHO, card)
        },
    }
}
