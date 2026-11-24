//! What is in the music folder, as the panel browses it.
//!
//! Which folder that is, and what counts as a song, are not decided here. The
//! player owns both, because a kind that plays and cannot be listed -- or is
//! listed and will not play -- is one disagreement wearing two faces, and a
//! download saving a song asks the same folder without drawing a panel.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use console_core_never::Never;

pub use console_music_player::library::{
  KINDS, Playable, config_path, folder, folder_under, playable, said_in,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Thing {
  pub name: String,
  pub path: PathBuf,
  pub folder: bool,
}

pub fn things(folder: &Path) -> Result<Vec<Thing>, Never> {
  let reading = match std::fs::read_dir(folder) {
    Ok(reading) => reading,
    Err(_unreadable) => return Ok(Vec::new()),
  };

  let mut things: Vec<Thing> = Vec::new();

  for entry in reading.flatten() {
    let about = about(&entry.path())?;

    match about {
      Some(thing) => things.push(thing),
      None => {}
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

  match bool::then_some(called.as_encoded_bytes().starts_with(b"."), ()) {
    ::core::option::Option::Some(()) => return Ok(None),
    ::core::option::Option::None => {}
  }

  let name = visible_name(called)?;
  let playable = playable(path)?;
  let folder = match std::fs::symlink_metadata(path) {
    Ok(metadata) => metadata.is_dir(),
    Err(_unavailable) => return Ok(None),
  };

  match bool::then_some(folder, ()) {
    ::core::option::Option::Some(()) => Ok(Some(Thing {
      name,
      path: path.to_path_buf(),
      folder: true,
    })),
    ::core::option::Option::None => match playable {
      Playable::Yes => {
        match path.is_file().then_some(()) {
          Some(()) => {}
          None => return Ok(None),
        }
        let named = console_core_file_names::title(&name)?;

        Ok(Some(Thing {
          name: named,
          path: path.to_path_buf(),
          folder: false,
        }))
      }
      Playable::No => Ok(None),
    },
  }
}

pub(crate) fn visible_name(name: &OsStr) -> Result<String, Never> {
  match name.to_str() {
    Some(text) => Ok(text.to_owned()),
    None => console_core_file_urls::escaped(Path::new(name)),
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
  AFolder,
  ASong,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn only_what_the_player_plays_is_listed() {
    assert_eq!(playable(Path::new("/a/b.OPUS")), Ok(Playable::Yes));
    assert_eq!(playable(Path::new("/a/b.mp3")), Ok(Playable::Yes));
    assert_eq!(playable(Path::new("/a/cover.jpg")), Ok(Playable::No));
    assert_eq!(playable(Path::new("/a/notes")), Ok(Playable::No));
  }
}
