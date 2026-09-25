//! The song that was playing, kept, so a player that starts has one.
//!
//! A music player that comes up with nothing in it asks a person to go and find
//! something before it will make a sound, every time, including the times they
//! only wanted to carry on with what they were already listening to. What was
//! playing is the one thing the machine knows and the person would have to
//! remember, so it is written down: the song, and how far into it the last
//! press left off.
//!
//! **Nothing plays by itself.** What is put back is a song loaded and paused at
//! its offset, never a sound out of a device someone has just switched on. So
//! the panel comes up with a card on it and the play button does what it says,
//! which is the whole of what "there is always a song" has to mean on a machine
//! that lives in a bag.
//!
//! **A file rather than a store with a schema.** This is two facts, written at
//! a press and read once at start; a database would be a dependency, a lock and
//! a migration in exchange for nothing, and the file is readable by whoever is
//! wondering what the player thinks it is holding. It is under the state
//! directory rather than the cache for the reason `console-response-times` is:
//! the machine cannot work this out again, and clearing a cache should not cost
//! someone the thing they were in the middle of.
//!
//! **When it is written is what makes the offset worth anything.** Every press
//! that changes what is playing writes it, and so does a song ending, because
//! those are the moments the answer changed. Nothing is written while a song
//! merely plays: a position that walked to disk once a second would be a write
//! a second for the length of an album, and what it would buy is the last few
//! seconds of a song after a power cut. The song survives that anyway. The
//! offset is as good as the last press, which is what someone pausing and
//! coming back to it tomorrow is asking for.
//!
//! A song that has been moved or deleted since is not put back and is not a
//! fault: the note is what was true, and the library is what is true now.

use std::path::{Path, PathBuf};

use console_core_atomic_writes::Stored;
use console_core_never::Never;
use console_core_places::Base;

pub const NOTE: &str = "music-player.playing";

pub const UNDER: &str = "playing";

pub const SONG: &str = "song";

pub const AT: &str = "at";

#[derive(Debug, Clone, PartialEq)]
pub struct Bookmark {
    pub song: PathBuf,
    pub at: f64,
}

pub fn where_() -> Result<Option<PathBuf>, Never> {
    let ours = Base::State.ours()?;

    Ok(ours.map(|ours| ours.join(NOTE)))
}

pub fn said(song: &Path, at: f64) -> Result<String, Never> {
    Ok(format!("[{UNDER}]\n{SONG}={}\n{AT}={at}\n", song.display()))
}

pub fn of(said: &str) -> Result<Option<Bookmark>, Never> {
    let fields = console_core_ini_files::fields(said, console_core_ini_files::Under(UNDER))?;

    let song = match fields.get(SONG) {
        Some(song) => PathBuf::from(song),
        None => return Ok(None),
    };

    match song.as_os_str().is_empty() {
        true => return Ok(None),
        false => {},
    }

    let at = match fields.get(AT).map(|at| at.parse()) {
        Some(Ok(at)) => at,
        Some(Err(_that_is_not_a_number)) => 0.0,
        None => 0.0,
    };

    Ok(Some(Bookmark { song, at }))
}

pub fn write(song: &Path, at: f64) -> Result<(), Never> {
    let Ok(where_) = where_();

    let at_ = match where_ {
        Some(at_) => at_,
        None => return Ok(()),
    };

    let Ok(said) = said(song, at);

    match console_core_atomic_writes::whole(&at_, said.as_bytes()) {
        Ok(()) => {},
        Err(fault) => eprintln!("music-player: {}: {fault}", at_.display()),
    }

    Ok(())
}

pub fn read() -> Result<Option<Bookmark>, Never> {
    let Ok(where_) = where_();

    let at = match where_ {
        Some(at) => at,
        None => return Ok(None),
    };

    let Ok(held) = console_core_atomic_writes::read(&at);

    let said = match held {
        Stored::Text(said) => said,
        Stored::Absent => return Ok(None),
        Stored::Failed(fault) => {
            eprintln!("music-player: {}: {fault}", at.display());

            return Ok(None);
        }
    };

    of(&said)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_written_is_what_is_read_back() {
        let song = Path::new("/home/someone/Music/a song.flac");
        let Ok(said) = said(song, 61.5);

        assert_eq!(
            of(&said),
            Ok(Some(Bookmark { song: song.to_path_buf(), at: 61.5 }))
        );
    }

    #[test]
    fn a_note_with_nothing_in_it_is_a_player_that_has_never_run() {
        assert_eq!(of(""), Ok(None));
        assert_eq!(of("[playing]\n"), Ok(None));
        assert_eq!(of("[playing]\nsong=\n"), Ok(None));
    }

    #[test]
    fn an_offset_no_one_can_read_is_the_start_of_the_song_and_not_a_lost_song() {
        let kept = of("[playing]\nsong=/a.flac\nat=the middle\n");

        assert_eq!(kept, Ok(Some(Bookmark { song: PathBuf::from("/a.flac"), at: 0.0 })));
    }

    #[test]
    fn a_song_with_no_offset_beside_it_starts_where_songs_start() {
        let kept = of("[playing]\nsong=/a.flac\n");

        assert_eq!(kept, Ok(Some(Bookmark { song: PathBuf::from("/a.flac"), at: 0.0 })));
    }
}
