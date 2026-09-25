//! The files, as an app.
//!
//! What it holds and why it is drawn this way is `console_files::card`'s own head.
//! This is the program someone types and the desktop opens, and it is an app
//! rather than a panel: it draws its own card across the whole screen, it is
//! not one of the pickers that take turns, and a panel opened over it goes
//! away again to find the folder still where it was left. Asked for a second
//! time with nothing new, it is already up and nothing starts; asked for a
//! folder, the one up stands down for it.

use console_core_never::Never;
use console_panel::picker::{self, Alone};
use console_panel::surface;

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    let Ok(()) = opened(&asked);
}

fn opened(asked: &[String]) -> Result<(), Never> {
    let Ok(alone) = picker::alone_as(console_files::WHO, asked);

    match alone {
        Alone::No => Ok(()),
        Alone::Yes => {
            let Ok(card) = console_files::card(asked);

            surface::app(console_files::WHO, card)
        },
    }
}
