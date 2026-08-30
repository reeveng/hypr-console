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

use gtk4::glib;
use console_music::library::{self, folder};
use console_music::looking::{self, Song};
use console_music::tags;

/// How often what has been read so far is written down.
///
/// A library read for three minutes and written at the end is a library read
/// from the top again after anything at all interrupts it.
const NOW_AND_THEN: usize = 50;

fn main() {
    let cache = glib::user_cache_dir();
    let at = looking::at(&cache);
    let known = looking::kept(&std::fs::read_to_string(&at).unwrap_or_default());
    // The walk says what there is and the file only says what those songs
    // said, so this is also what forgets a song that has been deleted.
    let mut songs = looking::songs(&folder(), &library::things, &known);
    let mut read = 0;

    for one in 0..songs.len() {
        if songs[one].read {
            continue;
        }
        let said = tags::of(&songs[one].path);
        songs[one] = Song { tags: said, read: true, ..songs[one].clone() };
        read += 1;

        if read % NOW_AND_THEN == 0 {
            wrote(&at, &songs);
        }
    }
    // Written even when nothing was read, so a song deleted since the last
    // reading leaves the file it was written into.
    wrote(&at, &songs);
}

/// What has been read, where the panel looks for it.
///
/// Beside itself and drawn over, because the panel reads this file on every
/// letter typed into the line and half a file is a library that says nothing.
fn wrote(at: &Path, songs: &[Song]) {
    let Some(folder) = at.parent() else { return };
    let _ = std::fs::create_dir_all(folder);
    let part = at.with_extension("part");

    if std::fs::write(&part, looking::written(songs)).is_ok() {
        let _ = std::fs::rename(&part, at);
    }
}
