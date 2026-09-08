//! The picture on the front of a song, put where a panel can open it.
//!
//! MPRIS says where a cover is and never what it is, so something has to write
//! the picture out as a file. kew kept one path and rewrote it in place, which
//! is the arrangement that cannot be told apart from nothing happening: a panel
//! handed the same path twice has no way to know the picture behind it changed,
//! and draws the one it already read. So the name carries the turn it was
//! written on, and the one before it is taken away as the next is made.
//!
//! The picture is copied rather than converted. Whatever stream is stapled to
//! the song comes out as it went in, and the name it lands under says jpg
//! whether or not it is one, because everything on this desktop that opens a
//! picture reads what is inside the file rather than the end of its name.
//!
//! Where they go is `console_core_places::Base` rather than the toolkit's own
//! answer. glib is in the room for the bus, and `glib::user_cache_dir` would
//! have been the shortest line here, but it answers something for a machine
//! with no `HOME` and this desktop would rather write no picture than write one
//! where nobody asked for it.

use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_places::Base;
use std::fs::{create_dir_all, read_dir, remove_file};
use std::path::{Path, PathBuf};

fn where_covers_go() -> Result<Option<PathBuf>, Never> {
    let ours = Base::Cache.ours()?;

    Ok(ours.map(|ours| ours.join("covers")))
}

pub fn of(song: &Path, turn: u64) -> Result<Option<PathBuf>, Never> {
    let covers = where_covers_go()?;

    let folder = match covers {
        Some(folder) => folder,
        None => {
            eprintln!("music-player: nobody's home, so the cover is not written anywhere");

            return Ok(None);
        },
    };

    match create_dir_all(&folder) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("music-player: {}: {fault}", folder.display());

            return Ok(None);
        },
    }

    let at = folder.join(format!("cover-{turn}.jpg"));
    let Ok(mut asking) = Program::Ffmpeg.command();

    asking
        .args(["-y", "-v", "quiet", "-i"])
        .arg(song)
        .args(["-an", "-c:v", "copy", "-frames:v", "1"])
        .arg(&at);

    let done = match asking.status() {
        Ok(done) => done,
        Err(fault) => {
            eprintln!("music-player: {}: asking for the cover: {fault}", song.display());

            return Ok(None);
        },
    };

    let Ok(()) = swept(&folder, &at);

    Ok(match done.success() && at.is_file() {
        true => Some(at),
        false => None,
    })
}

fn swept(folder: &Path, keeping: &Path) -> Result<(), Never> {
    let held = match read_dir(folder) {
        Ok(held) => held,
        Err(_nothing_to_sweep) => return Ok(()),
    };

    for found in held.flatten() {
        let at = found.path();

        match at == keeping {
            true => continue,
            false => {},
        }

        match remove_file(&at) {
            Ok(()) => {},
            Err(fault) => eprintln!("music-player: {}: {fault}", at.display()),
        }
    }

    Ok(())
}
