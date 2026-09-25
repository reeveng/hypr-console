//! A card that is open when something changes in a folder she keeps things in.
//!
//! The music, the films, the files and the downloads are cards whose rows are
//! a folder read again each time they are asked for, so what a change needs
//! from them is only to be asked. The pool watches every folder
//! `console_core_places::kept` names -- Music, Videos, Books and the rest --
//! and this listens for the whole life of the card and wakes the rows at each
//! change. A song fetched, a screenshot taken, a book unzipped or a film thrown
//! away by some other program all arrive the same way, because it is the
//! kernel saying so rather than whichever program did it.
//!
//! It does not ask which folder changed: which folder a card shows is the
//! card's business and changes as someone walks it, and a folder read that
//! finds nothing new costs less than the arithmetic that would have avoided it.
//!
//! It is started where every card is drawn rather than by the cards that care,
//! because a card may be drawn by the host that stays up between openings as
//! well as by its own program. Started once per process, so a host that opens
//! the music fifty times holds one listener rather than fifty, and every card
//! it draws is asked again -- which for a card with no folder in it is one
//! reading of rows nobody sees change.

use std::path::{Path, PathBuf};
use std::sync::Once;

use console_core_never::Never;
use console_events::subscription::{self, Received};
use console_program_contract::Topic;

use crate::frames::{self, Notice};

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "whether this process is already listening, which is a fact about the process: the host draws each card from a call that knows nothing of the cards before it, so there is nobody to hand it to"
    )
)]
static FOLLOWING: Once = Once::new();

pub fn follow() -> Result<(), Never> {
    FOLLOWING.call_once(|| {
        let Ok(kept) = console_core_places::kept();
        let Ok(folders) = watched(&kept);
        let Ok(listening) = subscription::connect(&folders);
        let Ok(()) = following(listening);
    });

    Ok(())
}

pub fn follow_at(socket: &Path, kept: &[PathBuf]) -> Result<(), Never> {
    let Ok(folders) = watched(kept);
    let Ok(listening) = subscription::connect_at(socket, &folders);

    following(listening)
}

fn watched(kept: &[PathBuf]) -> Result<Vec<Topic>, Never> {
    Ok(kept.iter().cloned().map(Topic::Path).collect())
}

fn following(listening: subscription::Subscriber) -> Result<(), Never> {
    console_program_lifetime::threads::let_go(std::thread::spawn(move || {
        let Ok(arriving) = listening.received();

        for received in arriving {
            match received {
                Received::Event(_changed) => {
                    let Ok(()) = frames::tell(Notice::Rows);
                },
                Received::Connected => {},
            }
        }
    }))
}
