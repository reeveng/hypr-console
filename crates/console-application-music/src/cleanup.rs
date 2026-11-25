//! Reconcile generated library records with the files observed on disk.

use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use console_core_never::Never;
use console_music_player::library::{DiskPresence, disk_presence};
use console_music_player::{loudness, selection::Selection};

use crate::{album_art, albums, looking::Song, review, suggestions};

pub struct Reconciliation<'a> {
  removed: Vec<&'a Song>,
  unavailable: Vec<&'a Song>,
}

impl<'a> Reconciliation<'a> {
  pub fn observe(previous: &'a [Song], current: &[Song]) -> Result<Self, Never> {
    let seen: BTreeSet<_> = current.iter().map(|song| song.path.as_path()).collect();
    let mut removed = Vec::new();
    let mut unavailable = Vec::new();

    for song in previous
      .iter()
      .filter(|song| !seen.contains(song.path.as_path()))
    {
      let Ok(presence) = presence(song);

      match presence {
        DiskPresence::Removed => removed.push(song),
        DiskPresence::Retain => unavailable.push(song),
      }
    }

    Ok(Self {
      removed,
      unavailable,
    })
  }

  pub fn checkpoint<'b>(
    &'b self,
    songs: &'b [Song],
  ) -> Result<impl Iterator<Item = &'b Song>, Never> {
    Ok(
      songs
        .iter()
        .chain(self.unavailable.iter().chain(&self.removed).copied()),
    )
  }

  pub fn catalog<'b>(
    &'b self,
    songs: &'b [Song],
  ) -> Result<impl Iterator<Item = &'b Song>, Never> {
    Ok(songs.iter().chain(self.unavailable.iter().copied()))
  }

  pub fn remove_tracks(&self, cache: &Path, current: &[Song]) -> io::Result<()> {
    let paths: BTreeSet<_> = self
      .removed
      .iter()
      .map(|song| song.path.as_path())
      .collect();

    match paths.is_empty().then_some(()) {
      Some(()) => return Ok(()),
      None => {}
    }

    let keys = paths
      .iter()
      .map(|path| {
        let Ok(key) = review::key(path);
        key
      })
      .collect();
    let file_id = |song: &Song| {
      let Ok(file) = original_file(song);
      file
    };
    let removed = self.removed.iter().copied().filter_map(file_id).collect();
    let retained = current
      .iter()
      .chain(self.unavailable.iter().copied())
      .filter_map(file_id)
      .collect();

    review::forget(cache, &keys)?;
    suggestions::forget(cache, &keys)?;
    Selection::forget(&paths)?;
    loudness::prune_old_revisions(&removed, &retained)
  }

  pub fn remove_covers(&self, previous: &[Song], songs: &[Song]) -> io::Result<()> {
    let removed: BTreeSet<_> = self
      .removed
      .iter()
      .map(|song| song.path.as_path())
      .collect();

    match removed.is_empty().then_some(()) {
      Some(()) => return Ok(()),
      None => {}
    }

    let Ok(current) = albums::groups(songs);
    let retained: BTreeSet<_> = current.into_iter().map(|album| album.key).collect();
    let Ok(old) = albums::groups(previous);

    for album in old {
      let gone = !retained.contains(&album.key)
        && album
          .tracks
          .iter()
          .all(|song| removed.contains(song.path.as_path()));

      match gone.then_some(()) {
        Some(()) => album_art::forget(&album.key)?,
        None => {}
      }
    }

    Ok(())
  }
}

#[must_use]
fn original_file(song: &Song) -> Result<Option<loudness::FileId>, Never> {
  let Ok(revision) = song.revision();

  Ok(revision.map(|revision_2| {
    let Ok(file) = revision_2.file();

    file
  }))
}

fn presence(song: &Song) -> Result<DiskPresence, Never> {
  let Ok(original) = original_file(song);
  let file = match original {
    Some(file) => file,
    None => return Ok(DiskPresence::Retain),
  };

  Ok(match disk_presence(&song.path, file.device) {
    Ok(known) => known,
    Err(fault) => {
      console_core_logging::debug!(
        "could not verify {} for library cleanup: {fault}",
        song.path.display()
      );
      DiskPresence::Retain
    }
  })
}
