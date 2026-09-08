//! What is in the music folder, and which folder that is.
//!
//! Where the music lives used to be read out of `~/.config/kew/kewrc`, and
//! written there too, because kew had to be told and a person who had already
//! told kew should not have to say it twice. The player is this tree's now and
//! is handed the folder on its command line, so there is nobody left to tell:
//! what is read here is a file of this desktop's own holding one path, and the
//! migration that swept the fork carried the old answer into it.
//!
//! What counts as a song is not decided here any more either. The player owns
//! that list, because a kind that plays and cannot be listed -- or is listed
//! and will not play -- is one disagreement wearing two faces.

use console_core_never::Never;
use std::path::{Path, PathBuf};

pub use console_music_player::library::{KINDS, Playable, playable};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Thing {
    pub name: String,
    pub path: PathBuf,
    pub folder: bool,
}

pub fn folder() -> Result<PathBuf, Never> {
    let Ok(home) = console_core_places::home();
    let Ok(said) = ours();

    folder_under(home.as_deref(), said)
}

pub fn folder_under(home: Option<&Path>, said: Option<String>) -> Result<PathBuf, Never> {
    let home = match home {
        Some(home) => home.to_path_buf(),

        None => {
            eprintln!("console-music-panel: no HOME; the music folder is looked for from here");

            PathBuf::new()
        }
    };

    Ok(said
        .map(|said| PathBuf::from(said.replace('~', &home.to_string_lossy())))
        .unwrap_or_else(|| home.join("Music")))
}

pub fn told() -> Result<PathBuf, Never> {
    Ok(gtk4::glib::user_config_dir().join("console/music"))
}

fn ours() -> Result<Option<String>, Never> {
    let Ok(at) = told();

    let said = match std::fs::read_to_string(at) {
        Ok(said) => said,
        Err(_nobody_has_said_where) => return Ok(None),
    };

    said_in(&said)
}

pub fn said_in(file: &str) -> Result<Option<String>, Never> {
    Ok(file
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string))
}

pub fn things(folder: &Path) -> Result<Vec<Thing>, Never> {
    let reading = match std::fs::read_dir(folder) {
        Ok(reading) => reading,
        Err(_fault) => return Ok(Vec::new()),
    };

    let mut things: Vec<Thing> = Vec::new();

    for entry in reading.flatten() {
        let about = about(&entry.path())?;

        match about {
            Some(thing) => things.push(thing),
            None => {},
        }
    }

    things.sort_by_key(|thing| (!thing.folder, thing.name.to_lowercase()));

    Ok(things)
}

fn about(path: &Path) -> Result<Option<Thing>, Never> {
    let called = match path.file_name() {
        Some(called) => called,
        None => return Ok(None),
    };

    let name = called.to_string_lossy().to_string();

    match name.starts_with('.') {
        true => return Ok(None),
        false => {},
    }

    let playable = playable(path)?;

    match path.is_dir() {
        true => Ok(Some(Thing { name, path: path.to_path_buf(), folder: true })),
        false => match playable {
            Playable::Yes => {
                let named = named(&name)?;

                Ok(Some(Thing { name: named, path: path.to_path_buf(), folder: false }))
            }
            Playable::No => Ok(None),
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    AFolder,
    ASong,
}

pub fn named(filename: &str) -> Result<String, Never> {
    let name = filename.rsplit_once('.').map_or(filename, |(stem, _)| stem);

    Ok(match name.rsplit_once(" [") {
        Some((title, tail)) if tail.ends_with(']') => title.trim().to_string(),
        Some(_) | None => name.trim().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_folder_is_under_the_home_it_belongs_to() {
        let hers = PathBuf::from("/home/ada");

        assert_eq!(
            folder_under(Some(&hers), Some("~/Songs".to_string())),
            Ok(PathBuf::from("/home/ada/Songs"))
        );
        assert_eq!(folder_under(Some(&hers), None), Ok(PathBuf::from("/home/ada/Music")));
    }

    #[test]
    fn no_home_is_no_home_of_somebody_elses() {
        let Ok(folder) = folder_under(None, None);

        assert_eq!(folder, PathBuf::from("Music"));
        assert!(!folder.starts_with("/root"), "an absent home is absent, not root's");
    }

    #[test]
    fn the_folder_this_desktop_was_told_is_the_one_used() {
        assert_eq!(said_in("~/Songs\n"), Ok(Some("~/Songs".to_string())));
        assert_eq!(said_in("# where the music is\n/mnt/music\n"), Ok(Some("/mnt/music".to_string())));
    }

    #[test]
    fn a_file_that_names_no_folder_leaves_the_ordinary_one() {
        assert_eq!(said_in(""), Ok(None));
        assert_eq!(said_in("\n\n# nothing but a note\n"), Ok(None));

        let Ok(folder) = folder_under(Some(Path::new("/home/ada")), None);

        assert_eq!(folder, PathBuf::from("/home/ada/Music"));
    }

    #[test]
    fn a_download_keeps_its_title_and_loses_its_id() {
        assert_eq!(named("505 [qU9mHegkTc4].opus"), Ok("505".to_string()));
        assert_eq!(named("227.Pink + White.flac"), Ok("227.Pink + White".to_string()));
    }

    #[test]
    fn only_what_the_player_plays_is_listed() {
        assert_eq!(playable(Path::new("/a/b.OPUS")), Ok(Playable::Yes));
        assert_eq!(playable(Path::new("/a/b.mp3")), Ok(Playable::Yes));
        assert_eq!(playable(Path::new("/a/cover.jpg")), Ok(Playable::No));
        assert_eq!(playable(Path::new("/a/notes")), Ok(Playable::No));
    }
}
