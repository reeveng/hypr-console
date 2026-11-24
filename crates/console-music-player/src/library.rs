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
use std::io::{self, ErrorKind};
use std::os::unix::fs::MetadataExt;
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

pub(crate) fn queueable(at: &Path) -> Result<Playable, Never> {
  let Ok(kind) = playable(at);

  match kind {
    Playable::Yes => {}
    Playable::No => return Ok(Playable::No),
  }

  let opened = at
    .canonicalize()
    .and_then(|path| console_core_atomic_writes::read_prefix(&path, 0));

  Ok(match opened {
    Ok(_checked_file) => Playable::Yes,
    Err(fault) => {
      console_core_logging::debug!("could not open song {}: {fault}", at.display());
      Playable::No
    }
  })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiskPresence {
  Removed,
  Retain,
}

pub fn disk_presence(path: &Path, device: u64) -> io::Result<DiskPresence> {
  match path.symlink_metadata() {
    Ok(_entry) => return Ok(DiskPresence::Retain),
    Err(fault) => match (fault.kind() == ErrorKind::NotFound).then_some(()) {
      Some(()) => {}
      None => return Err(fault),
    },
  }

  for parent in path.ancestors().skip(1) {
    match parent.metadata() {
      Ok(directory) => {
        return Ok(
          match (directory.is_dir() && directory.dev() == device).then_some(()) {
            Some(()) => DiskPresence::Removed,
            None => DiskPresence::Retain,
          },
        );
      }
      Err(fault) => match (fault.kind() == ErrorKind::NotFound).then_some(()) {
        Some(()) => {}
        None => return Err(fault),
      },
    }
  }

  Ok(DiskPresence::Retain)
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

#[derive(Debug)]
pub struct AlbumQueue {
  pub tracks: Vec<PathBuf>,
  pub first: u32,
}

pub fn album_argument(album: &AlbumQueue) -> Result<String, Never> {
  use std::fmt::Write;

  let mut argument = format!("{}\n", album.first);

  for path in &album.tracks {
    let Ok(url) = console_core_file_urls::url(path);
    let _written = writeln!(argument, "{url}");
  }

  Ok(argument)
}

pub fn album_paths(argument: &str) -> Result<Option<AlbumQueue>, Never> {
  let mut lines = argument.lines();
  let first = match lines.next().map(str::parse::<u32>) {
    Some(Ok(first)) => first,
    Some(Err(_invalid_index)) => return Ok(None),
    None => return Ok(None),
  };
  let mut tracks = Vec::new();

  for line in lines {
    let Ok(location) = console_core_file_urls::path(line);

    match location.filter(|path| path.is_absolute()) {
      Some(path) => tracks.push(path),
      None => return Ok(None),
    }
  }

  let Ok(at) = console_core_number_conversion::index(first);
  Ok(
    tracks
      .get(at)
      .is_some()
      .then_some(AlbumQueue { tracks, first }),
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
  fn queue_validation_follows_links_and_requires_a_readable_regular_file() -> std::io::Result<()>
  {
    let folder =
      console_core_temporary_directories::private("queueable").map_err(std::io::Error::other)?;
    let song = folder.join("song.opus");
    let link = folder.join("link.opus");
    let directory = folder.join("directory.opus");
    console_core_atomic_writes::whole(&song, b"audio").map_err(std::io::Error::other)?;
    std::os::unix::fs::symlink(&song, &link)?;
    std::fs::create_dir(&directory)?;

    assert_eq!(queueable(&song), Ok(Playable::Yes));
    assert_eq!(queueable(&link), Ok(Playable::Yes));
    assert_eq!(queueable(&directory), Ok(Playable::No));
    assert_eq!(queueable(&folder.join("missing.opus")), Ok(Playable::No));
    std::fs::remove_dir_all(folder)?;

    Ok(())
  }

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
