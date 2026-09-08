//! Asking ffprobe what a song says about itself, the way a panel asks.
//!
//! What an answer *means* is `console_music_player::tags`, because the player
//! has to say the same things on the bus and one file read two ways drifts.
//! What is left here is the asking, which is a panel's own business: it goes
//! through `console_panel::running`, and that is what takes the press stamp
//! back off, so a library of a thousand songs is not a thousand presses nobody
//! made.

use console_core_external_programs::Program;
use console_core_never::Never;
use console_panel::running::said;
use std::path::Path;

pub use console_music_player::tags::{AS_MUCH, BETWEEN, Said, Tags, asking, every, read};

pub fn of(path: &Path) -> Result<Tags, Never> {
    let argv = asking(path)?;
    let words: Vec<&str> = argv.iter().map(String::as_str).collect();

    let Ok(said) = said(Program::Ffprobe, &words);

    read(&said)
}
