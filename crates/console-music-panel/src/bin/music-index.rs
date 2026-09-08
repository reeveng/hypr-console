//! What every song says about itself, written down.
//!
//!     music-index
//!
//! Off the panel, and not in it. Reading one file is an ffprobe and reading a
//! library is nine hundred of them, which is minutes: a card that waited for it
//! would stop answering the buttons for all of them. So the panel starts this,
//! goes on drawing, and reads what it leaves behind.
//!
//! It reads what nobody has read yet and nothing else, so the minutes are spent
//! once. Run again after a song has been fetched, it is one file and it is over
//! before the panel has finished drawing.

use std::path::Path;

use console_core_atomic_writes::Held;
use console_core_never::Never;
use gtk4::glib;
use console_music_panel::library::{self, folder};
use console_music_panel::looking::{self, Song};
use console_music_panel::tags;

const NOW_AND_THEN: usize = 50;

fn main() {
    let cache = glib::user_cache_dir();

    let Ok(at) = looking::at(&cache);

    let Ok(held) = console_core_atomic_writes::read(&at);

    let said = match held {
        Held::Said(said) => said,

        Held::Nothing => String::new(),

        Held::Unreadable(fault) => {
            eprintln!("music-index: {}: reading what is known about the songs: {fault}", at.display());
            String::new()
        }
    };

    let Ok(known) = looking::kept(&said);

    let Ok(music) = folder();

    let Ok(mut songs) = looking::songs(&music, &library::things, &known);

    let mut read: usize = 0;

    for one in 0..songs.len() {
        let song = match songs.get(one) {
            Some(song) => song,
            None => continue,
        };

        match song.read {
            true => continue,
            false => {},
        }

        let Ok(said) = tags::of(&song.path);

        let song = Song { tags: said, read: true, ..song.clone() };

        let slot = match songs.get_mut(one) {
            Some(slot) => slot,
            None => continue,
        };

        *slot = song;
        read = read.saturating_add(1);

        match read.wrapping_rem(NOW_AND_THEN) {
            0 => {
                let Ok(()) = wrote(&at, &songs);
            },
            _ => {},
        }
    }

    let Ok(()) = wrote(&at, &songs);
}

fn wrote(at: &Path, songs: &[Song]) -> Result<(), Never> {
    let folder = match at.parent() {
        Some(folder) => folder,
        None => return Ok(()),
    };

    let _ = std::fs::create_dir_all(folder);

    let part = at.with_extension("part");

    let written = looking::written(songs)?;

    match std::fs::write(&part, written) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!(
                "music-index: {}: writing what was read about the songs: {fault}",
                part.display()
            );

            return Ok(());
        }
    }

    let _ = std::fs::rename(&part, at);

    Ok(())
}
