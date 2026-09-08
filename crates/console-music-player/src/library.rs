//! What counts as a song, and what songs are under a folder.
//!
//! The list of kinds was `console_music_panel::library`'s and both crates need it
//! now: the panel to decide what to draw a row for, the player to decide what
//! goes in a playlist when a song is opened. It is one list here and the panel
//! reads it from here, because a kind that plays and cannot be listed -- or is
//! listed and will not play -- is the same disagreement in two directions.
//!
//! The walk is the player's own and is deliberately the plain one: everything
//! under the folder, in the order a sorted walk gives, with no idea of folders
//! or of what a person was looking at. What the panel walks for browsing is a
//! different question with a different answer and it stays where it is.

use console_core_never::Never;
use std::fs::read_dir;
use std::path::{Path, PathBuf};

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
