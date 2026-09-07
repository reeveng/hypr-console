//! The card of what else can be done with one square of the home screen.
//!
//! Y on a square opens it, which is what Y is everywhere on this desktop: the
//! thing you are standing on, and under it the things that can be done to it.
//! The home screen has two -- move this one somewhere else, and take it off --
//! and until this card existed neither was on a button. Moving was a hold on
//! A, which is a press somebody has to be told about before they can make it,
//! and taking off was not on the home screen at all: it was a row on a card
//! listing every application on the machine, reached from the same Y.
//!
//! That list is the menu now, where it always was. This is only about the
//! square Y was pressed on.
//!
//! ## It holds no file and knows no square
//!
//! Both rows are a word said back through `console_onscreen::homeward`, which is
//! the door everything says everything to the home screen through. So the card
//! does not read what is on the home screen, does not write it, and is not
//! told which square is meant: the home screen is the one holding a highlight,
//! and it has not moved while this was over it.
//!
//! The word is said on the way out rather than from the row, and that is the
//! whole reason `main` is shaped the way it is. The home screen puts its
//! highlight away whenever anything opens in front of it and takes it back
//! when that closes; a word said while this card was still up would be a
//! square picked up and then dropped by the card going away.

use std::sync::{Arc, OnceLock};

use console_core_never::Never;
use console_onscreen::Said;
use console_panel::page::{Does, Page, Row, Rows};
use console_panel::{chooser, panel};

const MOVE: &str = "Move it";
const OFF: &str = "Take it off the home screen";

const THEN: &str = "then A puts it down";

fn rows(chosen: &Arc<OnceLock<Said>>) -> Result<Vec<Row>, Never> {
    let moving = Arc::clone(chosen);
    let taking = Arc::clone(chosen);

    let Ok(carries) = Does::call(move |_| {
        let _ = moving.set(Said::Carry);

        true
    });
    let Ok(takes) = Does::call(move |_| {
        let _ = taking.set(Said::Off);

        true
    });
    let Ok(moves) = Row::new(MOVE, THEN, carries);
    let Ok(off) = Row::new(OFF, "", takes);

    Ok(vec![moves, off])
}

fn main() {
    let Some(name) = std::env::args().nth(1).filter(|name| !name.is_empty()) else {
        eprintln!("home-square: nothing was named, so there is no square this is about");

        return;
    };

    let Ok(alone) = chooser::alone("home-square", chooser::Again::Closes);

    match alone {
        chooser::Alone::No => return,
        chooser::Alone::Yes => {},
    }

    let chosen: Arc<OnceLock<Said>> = Arc::new(OnceLock::new());
    let building = Arc::clone(&chosen);

    let Ok(()) = panel::show(
        Arc::new(move || {
            let Ok(rows) = rows(&building);
            let Ok(page) = Page::new(&name, Rows::Fixed(rows));

            vec![page]
        }),
        0,
        None,
    );

    let Some(said) = chosen.get() else { return };

    match console_onscreen::telling(*said) {
        Ok(()) => {},
        Err(fault) => {
            let Ok(word) = said.word();

            eprintln!("home-square: the home screen was not told {word}: {fault}");
        }
    }
}
