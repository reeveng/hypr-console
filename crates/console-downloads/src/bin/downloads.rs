//! The downloads, as an app.
//!
//! What it holds and why it is drawn this way is `console_downloads::card`'s
//! own head. This is the program someone types and the desktop opens, and the
//! book store the library's last cover leads to. It is an app rather than a
//! panel: drawn by this process across the whole screen, and still there with
//! its search and what it found when a panel opened over it goes.

use console_core_never::Never;
use console_panel::picker::{self, Alone};
use console_panel::surface;

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    let Ok(()) = opened(&asked);
}

fn opened(asked: &[String]) -> Result<(), Never> {
    let Ok(alone) = picker::alone_as(console_downloads::WHO, asked);

    match alone {
        Alone::No => Ok(()),
        Alone::Yes => {
            let Ok(card) = console_downloads::card(asked);

            surface::app(console_downloads::WHO, card)
        },
    }
}
