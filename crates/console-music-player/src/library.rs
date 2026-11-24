//! What counts as a song, and what songs are under a folder.
//!
//! The list of kinds was `console_application_music::library`'s and both crates need it
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
use std::ffi::OsString;
use std::path::{Path, PathBuf};

const MUSIC: &str = "Music";

const MINE: &str = "music";

pub const KINDS: [&str; 10] = [
  "aac", "flac", "m4a", "mp3", "oga", "ogg", "opus", "wav", "webm", "wma",
];

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

  Ok(
    match ::core::primitive::bool::then_some(KINDS.contains(&kind.as_str()), ()) {
      ::core::option::Option::Some(()) => Playable::Yes,
      ::core::option::Option::None => Playable::No,
    },
  )
}

pub fn songs_under(folder: &Path) -> Result<Vec<PathBuf>, Never> {
  let Ok(listing) = console_core_directory_listing::files(folder);

  Ok(
    listing
      .into_iter()
      .filter_map(|at| {
        let Ok(playable) = playable(&at);

        match playable {
          Playable::No => None,
          Playable::Yes => match std::fs::metadata(&at) {
            Ok(metadata) => metadata.is_file().then_some(at),
            Err(fault) => {
              console_core_logging::debug!("could not read song {}: {fault}", at.display());

              None
            }
          },
        }
      })
      .collect(),
  )
}

pub fn folder() -> Result<PathBuf, Never> {
  let Ok(home) = console_core_places::home();
  let Ok(said) = ours();

  folder_under(home.as_deref(), said)
}

pub fn folder_under(home: Option<&Path>, said: Option<String>) -> Result<PathBuf, Never> {
  let home_3 = match home {
    Some(home_2) => home_2.to_path_buf(),

    None => {
      console_core_logging::warn!("no HOME, so the music folder is looked for from here");

      PathBuf::new()
    }
  };

  Ok(match said {
    Some(said_2) => {
      let mut parts = said_2.split('~');
      let prefix = match parts.next() {
        Some(prefix) => prefix,
        None => "",
      };
      let mut named = OsString::from(prefix);

      for part in parts {
        named.push(&home_3);
        named.push(part);
      }

      PathBuf::from(named)
    }
    None => home_3.join(MUSIC),
  })
}

pub fn config_path() -> Result<Option<PathBuf>, Never> {
  let ours = console_core_places::Base::Configuration.ours()?;

  Ok(ours.map(|ours_2| ours_2.join(MINE)))
}

fn ours() -> Result<Option<String>, Never> {
  let Ok(at) = config_path();

  let at_3 = match at {
    Some(at_2) => at_2,
    None => return Ok(None),
  };

  let said = match std::fs::read_to_string(at_3) {
    Ok(said) => said,
    Err(_no_one_has_said_where) => return Ok(None),
  };

  said_in(&said)
}

pub fn said_in(file: &str) -> Result<Option<String>, Never> {
  Ok(
    file
      .lines()
      .map(str::trim)
      .find(|line| !line.is_empty() && !line.starts_with('#'))
      .map(str::to_string),
  )
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
    assert_eq!(
      folder_under(Some(&hers), None),
      Ok(PathBuf::from("/home/ada/Music"))
    );
  }

  #[test]
  fn no_home_is_no_home_of_someone_elses() {
    let Ok(folder) = folder_under(None, None);

    assert_eq!(folder, PathBuf::from("Music"));
    assert!(
      !folder.starts_with("/root"),
      "an absent home is absent, not root's"
    );
  }

  #[test]
  fn the_folder_this_desktop_was_told_is_the_one_used() {
    assert_eq!(said_in("~/Songs\n"), Ok(Some("~/Songs".to_string())));
    assert_eq!(
      said_in("# where the music is\n/mnt/music\n"),
      Ok(Some("/mnt/music".to_string()))
    );
  }

  #[test]
  fn a_file_that_names_no_folder_leaves_the_ordinary_one() {
    assert_eq!(said_in(""), Ok(None));
    assert_eq!(said_in("\n\n# nothing but a note\n"), Ok(None));

    let Ok(folder) = folder_under(Some(Path::new("/home/ada")), None);

    assert_eq!(folder, PathBuf::from("/home/ada/Music"));
  }

  #[test]
  fn a_download_keeps_its_title_and_loses_its_id() {}
}
