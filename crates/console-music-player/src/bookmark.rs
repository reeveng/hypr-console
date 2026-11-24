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
//! The playhead is refreshed during playback and when the session asks for a
//! final checkpoint. The queue, shuffle order and repeat mode are written
//! separately when they change so saving the playhead stays small.
//!
//! A song that has been moved or deleted since is not put back and is not a
//! fault: the note is what was true, and the library is what is true now.

use std::path::{Path, PathBuf};

use crate::playlist::{Order, Over, SavedQueue};
use console_core_atomic_writes::Stored;
use console_core_never::Never;
use console_core_places::Base;

pub const NOTE: &str = "music-player.playing";

const QUEUE: &str = "music-player.queue";

pub const UNDER: &str = "playing";

pub const SONG: &str = "song";

pub const AT: &str = "at";

#[derive(Debug, Clone, PartialEq)]
pub struct Bookmark {
  pub song: PathBuf,
  pub at: crate::Seconds,
}

pub fn where_() -> Result<Option<PathBuf>, Never> {
  let ours = Base::State.ours()?;

  Ok(ours.map(|ours_2| ours_2.join(NOTE)))
}

pub fn serialize(song: &Path, crate::Seconds(at): crate::Seconds) -> Result<String, Never> {
  let Ok(path) = console_core_file_urls::url(song);
  Ok(format!("[{UNDER}]\nsong-uri={path}\n{AT}={at}\n"))
}

pub fn of(said: &str) -> Result<Option<Bookmark>, Never> {
  let fields = console_core_ini_files::fields(said, console_core_ini_files::Under(UNDER))?;

  let song = match fields.get("song-uri") {
    Some(uri) => {
      let Ok(path) = console_core_file_urls::path(uri);

      match path {
        Some(matched_song) => matched_song,
        None => return Ok(None),
      }
    }
    None => match fields.get(SONG) {
      Some(path) => PathBuf::from(path),
      None => return Ok(None),
    },
  };

  match ::core::primitive::bool::then_some(song.as_os_str().is_empty(), ()) {
    ::core::option::Option::Some(()) => return Ok(None),
    ::core::option::Option::None => {}
  }

  let at = match fields.get(AT).map(|at| at.parse::<f64>()) {
    Some(Ok(at)) => match (at.is_finite() && at >= 0.0).then_some(()) {
      Some(()) => at,
      None => 0.0,
    },
    Some(Err(_that_is_not_a_number)) => 0.0,
    None => 0.0,
  };

  Ok(Some(Bookmark {
    song,
    at: crate::Seconds(at),
  }))
}

pub fn write(song: &Path, at: crate::Seconds) -> Result<(), Never> {
  let Ok(where_) = where_();

  let at_ = match where_ {
    Some(at_) => at_,
    None => return Ok(()),
  };

  let Ok(said) = serialize(song, at);

  match console_core_atomic_writes::whole_with_folders(&at_, said.as_bytes()) {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!("music-player: {}: {fault}", at_.display()),
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
      console_core_logging::warn!("music-player: {}: {fault}", at.display());

      return Ok(None);
    }
  };

  of(&said)
}

#[must_use]
fn encode_queue(queue: &SavedQueue) -> Result<String, Never> {
  use std::fmt::Write;
  let shuffle = match { ::core::primitive::bool::then_some(queue.ordered == Order::Any, ()) } {
    ::core::option::Option::Some(()) => "shuffle",
    ::core::option::Option::None => "listed",
  };
  let repeat = match queue.over {
    Over::On => "none",
    Over::Again => "track",
    Over::Round => "queue",
  };
  let mut text = format!("{shuffle} {repeat}\n");
  let mut ranks = vec![0_u32; queue.songs.len()];

  for (rank, song) in (0_u32..).zip(&queue.order) {
    let Ok(song_2) = console_core_number_conversion::index(*song);

    {
      match ranks.get_mut(song_2) {
        Some(slot) => {
          *slot = rank;
        }
        ::core::option::Option::None => {}
      };
    }
  }

  for (path, rank) in queue.songs.iter().zip(ranks) {
    let Ok(url) = console_core_file_urls::url(path);
    let _written = writeln!(text, "{rank} {url}");
  }

  Ok(text)
}

#[must_use]
fn decode_queue(text: &str) -> Result<Option<SavedQueue>, Never> {
  let mut lines = text.lines();
  let (shuffle, repeat) = match lines.next().and_then(|header| header.split_once(' ')) {
    Some(modes) => modes,
    None => return Ok(None),
  };
  let ordered = match shuffle {
    "shuffle" => Order::Any,
    "listed" => Order::AsListed,
    _ => return Ok(None),
  };
  let over = match repeat {
    "none" => Over::On,
    "track" => Over::Again,
    "queue" => Over::Round,
    _ => return Ok(None),
  };
  let mut songs = Vec::new();
  let mut order = Vec::new();

  for (index, line) in (0_u32..).zip(lines) {
    let (rank, url) = match line.split_once(' ') {
      Some(parts) => parts,
      None => return Ok(None),
    };
    let Ok(path) = console_core_file_urls::path(url);

    match (path, rank.parse::<u32>()) {
      (Some(song), Ok(position)) => {
        songs.push(song);
        order.push((position, index));
      }
      (None, _) => return Ok(None),
      (Some(_), Err(_invalid_rank)) => return Ok(None),
    }
  }

  order.sort_unstable_by_key(|(rank, _)| *rank);

  match {
    ::core::primitive::bool::then_some(
      !order
        .iter()
        .zip(0_u32..)
        .all(|((rank, _), index)| *rank == index),
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  Ok(Some(SavedQueue {
    songs,
    order: order.into_iter().map(|(_, index)| index).collect(),
    ordered,
    over,
  }))
}

#[must_use]
pub(crate) fn read_queue() -> Result<Option<SavedQueue>, Never> {
  let Ok(state) = Base::State.ours();
  let path = match state {
    Some(root) => root.join(QUEUE),
    None => return Ok(None),
  };
  let Ok(stored) = console_core_atomic_writes::read(&path);

  match stored {
    Stored::Text(text) => decode_queue(&text),
    Stored::Absent | Stored::Failed(_) => Ok(None),
  }
}

pub(crate) enum QueueSaved {
  Yes,
  No,
}

pub(crate) fn write_queue(queue: &SavedQueue) -> Result<QueueSaved, Never> {
  let Ok(state) = Base::State.ours();
  let path = match state {
    Some(root) => root.join(QUEUE),
    None => return Ok(QueueSaved::Yes),
  };
  let Ok(encoded) = encode_queue(queue);

  Ok(
    match console_core_atomic_writes::whole_with_folders(&path, encoded.as_bytes()) {
      Ok(()) => QueueSaved::Yes,
      Err(fault) => {
        console_core_logging::warn!("music-player: saving the queue: {fault}");

        QueueSaved::No
      }
    },
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_shuffled_queue_keeps_its_order_and_paths() {
    let queue = SavedQueue {
      songs: vec![
        PathBuf::from("/Music/one\nSong.flac"),
        PathBuf::from("/Music/two song.flac"),
      ],
      order: vec![1, 0],
      over: Over::Again,
      ordered: Order::Any,
    };
    let Ok(encoded) = encode_queue(&queue);

    assert_eq!(decode_queue(&encoded), Ok(Some(queue)));
  }

  #[test]
  fn what_is_written_is_what_is_read_back() {
    let song = Path::new("/home/someone/Music/a song.flac");
    let Ok(said) = serialize(song, crate::Seconds(61.5));

    assert_eq!(
      of(&said),
      Ok(Some(Bookmark {
        song: song.to_path_buf(),
        at: crate::Seconds(61.5)
      }))
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

    assert_eq!(
      kept,
      Ok(Some(Bookmark {
        song: PathBuf::from("/a.flac"),
        at: crate::Seconds(0.0)
      }))
    );
  }

  #[test]
  fn a_song_with_no_offset_beside_it_starts_where_songs_start() {
    let kept = of("[playing]\nsong=/a.flac\n");

    assert_eq!(
      kept,
      Ok(Some(Bookmark {
        song: PathBuf::from("/a.flac"),
        at: crate::Seconds(0.0)
      }))
    );
  }
}
