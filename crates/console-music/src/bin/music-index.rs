//! What every song says about itself, written down.
//!
//!     music-index
//!
//! Off the panel, and not in it. Reading one file is an ffprobe and reading a
//! library is nine hundred of them, which is minutes: a card that waited for it
//! would stop answering the buttons for all of them. So the panel starts this,
//! goes on drawing, and reads what it leaves behind.
//!
//! It reads what no one has read yet and nothing else, so the minutes are spent
//! once. Run again after a song has been fetched, it is one file and it is over
//! before the panel has finished drawing.

use std::path::Path;

use console_core_never::Never;
use console_music::library::{self, folder};
use console_music::looking::{self, Song};
use console_music_player::tags;

const NOW_AND_THEN: u32 = 50;

fn main() {
  let Ok(cache) = console_core_places::Base::Cache.user();

  let cache_home = match cache {
    Some(home) => home,

    None => {
      eprintln!(
        "music-index: no HOME, so there is nowhere to keep what is known about the songs"
      );

      return;
    }
  };

  let Ok(at) = looking::at(&cache_home);
  let Ok(known) = looking::load(&at);

  let Ok(music) = folder();

  let Ok(mut songs) = looking::songs(&music, &library::things, &known);

  let mut read: u32 = 0;

  #[cfg_attr(
    dylint_lib = "explicit031_no_walking_by_count",
    allow(
      explicit031_no_walking_by_count,
      reason = "the list is written out from inside the loop as it goes, and one song is put back into it each time round, so the walk cannot be holding a borrow of it"
    )
  )]
  for one in 0..songs.len() {
    let song = match songs.get(one) {
      Some(song) => song,
      None => continue,
    };

    match ::core::primitive::bool::then_some(song.read, ()) {
      ::core::option::Option::Some(()) => continue,
      ::core::option::Option::None => {}
    }

    let Ok(metadata) = tags::of(&song.path);

    let indexed = Song {
      tags: metadata,
      read: true,
      ..song.clone()
    };

    let slot = match songs.get_mut(one) {
      Some(slot) => slot,
      None => continue,
    };

    *slot = indexed;
    read = read.saturating_add(1);

    match read.wrapping_rem(NOW_AND_THEN) {
      0 => {
        let Ok(()) = wrote(&at, &songs);
      }
      _ => {}
    }
  }

  let Ok(()) = wrote(&at, &songs);
}

fn wrote(at: &Path, songs: &[Song]) -> Result<(), Never> {
  let written = looking::serialize(songs)?;

  match console_core_atomic_writes::whole_with_folders(at, written.as_bytes()) {
    Ok(()) => {}
    Err(fault) => {
      eprintln!(
        "music-index: {}: writing what was read about the songs: {fault}",
        at.display()
      );

      return Ok(());
    }
  }

  Ok(())
}
