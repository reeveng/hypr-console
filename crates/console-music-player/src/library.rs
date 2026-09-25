//! What counts as a song, and what songs are under a folder.
//!
//! The list of kinds was `console_music::library`'s and both crates need it
//! now: the panel to decide what to draw a row for, the player to decide what
//! goes in a playlist when a song is opened. It is one list here and the panel
//! reads it from here, because a kind that plays and cannot be listed -- or is
//! listed and will not play -- is the same disagreement in two directions.
//!
//! Which folder is the music folder is here for the same reason. It used to be
//! read out of `~/.config/kew/kewrc`, because kew had to be told; the player is
//! this tree's now and is handed the folder on its command line, so what is
//! read is a file of this desktop's own holding one path, and the migration
//! that swept the fork carried the old answer into it. `named` is how a file in
//! it is said: the title a download kept, without the id it came with.
//!
//! The walk is the player's own and is deliberately the plain one: everything
//! under the folder, in the order a sorted walk gives, with no idea of folders
//! or of what a person was looking at. What the panel walks for browsing is a
//! different question with a different answer and it stays where it is.

use console_core_never::Never;
use std::fs::read_dir;
use std::path::{Path, PathBuf};

const MUSIC: &str = "Music";

const MINE: &str = "music";

pub const KINDS: [&str; 9] =
    ["aac", "flac", "m4a", "mp3", "ogg", "opus", "wav", "webm", "wma"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Playable {
    Yes,
    No,
}

pub fn playable(at: &Path) -> Result<Playable, Never> {
    let kind = match at.extension().and_then(|kind| kind.to_str()) {
        Some(kind) => kind.to_lowercase(),
        None => return Ok(Playable::No),
    };

    Ok(match KINDS.contains(&kind.as_str()) {
        true => Playable::Yes,
        false => Playable::No,
    })
}

pub fn songs_under(folder: &Path) -> Result<Vec<PathBuf>, Never> {
    let mut found: Vec<PathBuf> = Vec::new();
    let mut walking = vec![folder.to_path_buf()];

    while let Some(at) = walking.pop() {
        let held = match read_dir(&at) {
            Ok(held) => held,
            Err(_not_a_folder_we_can_read) => continue,
        };

        for thing in held.flatten() {
            let at = thing.path();

            match at.is_dir() {
                true => walking.push(at),
                false => {
                    let Ok(playable) = playable(&at);

                    match playable {
                        Playable::Yes => found.push(at),
                        Playable::No => {},
                    }
                },
            }
        }
    }

    found.sort();

    Ok(found)
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
            eprintln!("console-music-player: no HOME; the music folder is looked for from here");

            PathBuf::new()
        }
    };

    Ok(match said {
        Some(said) => PathBuf::from(said.replace('~', &home.to_string_lossy())),
        None => home.join(MUSIC),
    })
}

pub fn told() -> Result<Option<PathBuf>, Never> {
    let ours = console_core_places::Base::Configuration.ours()?;

    Ok(ours.map(|ours| ours.join(MINE)))
}

fn ours() -> Result<Option<String>, Never> {
    let Ok(at) = told();

    let at = match at {
        Some(at) => at,
        None => return Ok(None),
    };

    let said = match std::fs::read_to_string(at) {
        Ok(said) => said,
        Err(_no_one_has_said_where) => return Ok(None),
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

pub fn named(filename: &str) -> Result<String, Never> {
    let name = filename.rsplit_once('.').map_or(filename, |(stem, _)| stem);

    Ok(match name.rsplit_once(" [") {
        Some((title, tail)) => match tail.ends_with(']') {
            true => title.trim().to_string(),
            false => name.trim().to_string(),
        },
        None => name.trim().to_string(),
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
    fn no_home_is_no_home_of_someone_elses() {
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
}
