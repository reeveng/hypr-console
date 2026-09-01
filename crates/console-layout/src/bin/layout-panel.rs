//! The buttons, and where they are on this device.
//!
//!     layout-panel            open it
//!     layout-panel --first    open it because nobody has answered yet
//!
//! One row per thing the desktop does, what plays it now beside it, and A on a
//! row asks for the button by putting a card up and waiting for a press. The
//! card is a program of its own, because what tells the controller daemon to
//! make the front of the machine inert while the question is on screen is the
//! card's own layer being there.
//!
//! What is here is the machine. `console_layout::rows` is the screen, and can
//! be asked without one.

use std::sync::Arc;

use console_layout::rows::{PUT_BACK_SURE, PUT_BACK_YES, Part, TABS, parts, question, rows};
use console_layout::table;
use console_pad::jobs::Jobs;
use console_panel::page::{Does, Page, Row, Rows};
use console_panel::{chooser, panel};

/// The card that asks, and the job it is asking about.
fn asks_for(part: &Part) -> Does {
    let slug = part.slug.clone();
    let asked = question(part);
    Does::and_stay(move |showing| {
        // Said before the card goes up rather than after, because the card is
        // what the buttons are for from the moment it is drawn: whoever is
        // holding this has to know what is being asked before it is inert.
        showing.note(&asked);
        showing.later(vec!["console-asking".to_string(), slug.clone()]);
    })
}

/// Undo the lot, and put every job back where this desktop has it.
///
/// An empty file is the whole of it: what is in the file is only what somebody
/// moved, so a file with nothing in it is a machine with nothing moved.
///
/// Asked first. This is the first row on the page, so it is the row the
/// highlight opens on, and it is the one row here that undoes work rather than
/// doing any: a press that lands on it by being a press too many should not
/// quietly throw away every button somebody has moved.
fn puts_it_all_back() -> Does {
    Does::and_stay(|showing| {
        showing.sure(PUT_BACK_SURE, "", &[PUT_BACK_YES], Arc::new(|showing, _| {
            if let Err(fault) = table::write(&Jobs::none()) {
                showing.note(&fault);
            }
        }));
    })
}

fn buttons_tab() -> Vec<Row> {
    rows(&parts(&table::table(), &table::front()), asks_for, puts_it_all_back())
}

fn pages() -> Vec<Page> {
    vec![Page::new(TABS[0], Rows::asked(buttons_tab))]
}

fn main() {
    if !chooser::alone("buttons", chooser::Again::Closes) {
        return;
    }
    // Opened by an apply, on a device nobody has answered for yet. Answering
    // is putting the screen up rather than moving anything: somebody who looks
    // at their buttons and finds them all where they should be has answered
    // the question, and a machine that raised this after every apply until a
    // button moved would be a machine that had not listened.
    if std::env::args().any(|word| word == "--first") && !table::at().exists() {
        let _ = table::write(&Jobs::none());
    }
    panel::show(Arc::new(pages), 0, None);
}
