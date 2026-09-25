//! The music, as an app.
//!
//! What it holds and why it is drawn this way is `console_music::card`'s own
//! head. This is the library someone opens from the home screen and the menu,
//! drawn by this process across the whole screen; the song on now is also the
//! panel the bar opens, which is `music-panel`.

use console_core_never::Never;
use console_panel::picker::{self, Alone};
use console_panel::surface;

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    let Ok(()) = opened(&asked);
}

fn opened(asked: &[String]) -> Result<(), Never> {
    let Ok(alone) = picker::alone_as(console_music::APP, asked);

    match alone {
        Alone::No => Ok(()),
        Alone::Yes => {
            let Ok(card) = console_music::library(asked);

            surface::app(console_music::APP, card)
        },
    }
}
