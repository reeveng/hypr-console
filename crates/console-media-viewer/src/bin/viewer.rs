//! A picture or a film, asked for.
//!
//!     viewer FILE-OR-FOLDER
//!  What it shows and how it is driven is `console_media_viewer::card`'s own
//! head. This is the program the files open and a `.desktop` file names, and it
//! is an app, drawn by this process across the whole screen. It can answer a
//! press by not opening at all, because the file it was handed may be neither
//! a picture nor a film -- and that has to be decided before the screen is
//! taken. Whoever is looking gets the folder read twice for it, once to find
//! out there is something to show and once to show it.
//!
//! A folder is not one of those and never was. An empty one opens and says it
//! is empty, because the entry that opens the pictures folder is on the home
//! screen, and a press there that draws nothing cannot be told from a crash.

use console_core_never::Never;
use console_panel::picker::{self, Alone};
use console_panel::surface;
use console_media_viewer::card::Worth;

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    let Ok(()) = opened(&asked);
}

fn opened(asked: &[String]) -> Result<(), Never> {
    let Ok(worth) = console_media_viewer::card::worth_opening(asked);

    match worth {
        Worth::None => return Ok(()),
        Worth::Opening => {},
    }

    let Ok(alone) = picker::alone_as(console_media_viewer::WHO, asked);

    match alone {
        Alone::No => Ok(()),
        Alone::Yes => {
            let Ok(card) = console_media_viewer::card(asked);

            surface::app(console_media_viewer::WHO, card)
        },
    }
}
