//! Navigation and requests for catalog work. Jobs carry the identity of their completion.

use std::collections::BTreeMap;
use std::path::PathBuf;

use console_core_never::Never;
use console_music_player::selection::Visibility;
use console_panel::page::PagePosition;
use console_program_contract::ExitStatus;

use crate::albums::Key;
use crate::review::FoundTrack;
use crate::suggestions::Search;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Management {
  pub album: Option<Key>,
  pub scan: Status,
  pub cover: Status,
  pub saving: Status,
  pub metadata: MetadataView,
  metadata_position: Option<PagePosition>,
  detail_positions: BTreeMap<(MetadataDetail, String), PagePosition>,
  searches: BTreeMap<String, SearchJob>,
  turn: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum MetadataView {
  #[default]
  Recordings,
  Recording(FoundTrack),
  MusicBrainz(FoundTrack),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum MetadataDetail {
  Recording,
  MusicBrainz,
}

impl MetadataView {
  fn detail(&self) -> Result<Option<(MetadataDetail, String)>, Never> {
    Ok(match self {
      Self::Recordings => None,
      Self::Recording(file) => Some((MetadataDetail::Recording, file.path.clone())),
      Self::MusicBrainz(file) => Some((MetadataDetail::MusicBrainz, file.path.clone())),
    })
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SearchJob {
  Pending { id: u64, file: FoundTrack },
  Failed(FoundTrack),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lookup {
  Ready,
  Pending,
  Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cached {
  Yes,
  No,
}

enum Completion {
  Applied,
  Stale,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Status {
  #[default]
  Idle,
  Pending(u64),
  Finished(ExitStatus),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Job {
  Scan,
  Cover,
  Selection,
  Search,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlbumStart {
  pub album: Key,
  pub track: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Request {
  Browse(Option<Key>),
  Opened,
  Refresh,
  Cover(crate::album_art::Lookup),
  PlayAlbum(AlbumStart),
  OpenMetadata(FoundTrack),
  OpenMusicBrainz {
    query: Search,
    cached: Cached,
  },
  MetadataBack,
  ViewedMetadata(PagePosition),
  SearchSong(Search),
  ConfirmSong(crate::suggestions::Proposal),
  Select(Vec<(PathBuf, Visibility)>),
  Finished {
    id: u64,
    job: Job,
    status: ExitStatus,
  },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Work {
  Refresh {
    id: u64,
  },
  Cover {
    id: u64,
    album: crate::album_art::Lookup,
  },
  Select {
    id: u64,
    changes: Vec<(PathBuf, Visibility)>,
  },
  PlayAlbum(AlbumStart),
  SearchSong {
    id: u64,
    query: Search,
  },
  ConfirmSong {
    id: u64,
    choice: crate::suggestions::Proposal,
  },
  Redraw,
  FocusFirst,
}

impl Management {
  pub fn update(mut self, request: Request) -> Result<(Self, Vec<Work>), Never> {
    let work = match request {
      Request::Browse(album) => {
        self.album = album;
        vec![Work::FocusFirst]
      }
      Request::Opened => match self.scan {
        Status::Idle => {
          let Ok(id) = self.next_id();
          self.scan = Status::Pending(id);

          vec![Work::Refresh { id }]
        }
        Status::Pending(_) | Status::Finished(_) => Vec::new(),
      },
      Request::Refresh => match self.scan {
        Status::Pending(_) => Vec::new(),
        Status::Idle | Status::Finished(_) => {
          let Ok(id) = self.next_id();
          self.scan = Status::Pending(id);

          vec![Work::Refresh { id }, Work::Redraw]
        }
      },
      Request::Cover(album) => match self.cover {
        Status::Pending(_) => Vec::new(),
        Status::Idle | Status::Finished(_) => {
          let Ok(id) = self.next_id();
          self.cover = Status::Pending(id);

          vec![Work::Cover { id, album }, Work::Redraw]
        }
      },
      Request::PlayAlbum(paths) => vec![Work::PlayAlbum(paths)],
      Request::OpenMetadata(file) => {
        let Ok(work) = self.open_metadata(file);
        work
      }
      Request::OpenMusicBrainz { query, cached } => {
        let Ok(work) = self.open_musicbrainz(query, cached);
        work
      }
      Request::MetadataBack => {
        let Ok(work) = self.metadata_back();
        work
      }
      Request::ViewedMetadata(position) => {
        let Ok(viewing) = self.metadata.detail();

        match viewing {
          None => self.metadata_position = Some(position),
          Some(detail) => {
            let _ = self.detail_positions.insert(detail, position);
          }
        }

        Vec::new()
      }
      Request::SearchSong(query) => {
        let Ok(work) = self.search(query);
        work.into_iter().collect()
      }
      Request::ConfirmSong(choice) => match self.saving {
        Status::Pending(_) => Vec::new(),
        Status::Idle | Status::Finished(_) => {
          let Ok(id) = self.next_id();
          self.saving = Status::Pending(id);

          vec![Work::ConfirmSong { id, choice }]
        }
      },
      Request::Select(changes) => match self.saving {
        Status::Pending(_) => Vec::new(),
        Status::Idle | Status::Finished(_) => {
          let Ok(id) = self.next_id();
          self.saving = Status::Pending(id);

          vec![Work::Select { id, changes }]
        }
      },
      Request::Finished { id, job, status } => {
        let Ok(completion) = match job {
          Job::Scan => self.scan.finished(id, status),
          Job::Cover => self.cover.finished(id, status),
          Job::Selection => self.saving.finished(id, status),
          Job::Search => self.search_finished(id, status),
        };

        match completion {
          Completion::Applied => vec![Work::Redraw],
          Completion::Stale => Vec::new(),
        }
      }
    };

    Ok((self, work))
  }

  fn next_id(&mut self) -> Result<u64, Never> {
    self.turn = self.turn.saturating_add(1);

    Ok(self.turn)
  }

  pub fn metadata_position(&self) -> Result<Option<PagePosition>, Never> {
    let Ok(viewing) = self.metadata.detail();

    Ok(match viewing {
      None => self.metadata_position.clone(),
      Some(detail) => self.detail_positions.get(&detail).cloned(),
    })
  }

  pub fn lookup(&self, target: &FoundTrack) -> Result<Lookup, Never> {
    Ok(match self.searches.get(&target.path) {
      Some(SearchJob::Pending { file, .. }) => match (file == target).then_some(()) {
        Some(()) => Lookup::Pending,
        None => Lookup::Ready,
      },
      Some(SearchJob::Failed(file)) => match (file == target).then_some(()) {
        Some(()) => Lookup::Failed,
        None => Lookup::Ready,
      },
      None => Lookup::Ready,
    })
  }

  fn open_metadata(&mut self, file: FoundTrack) -> Result<Vec<Work>, Never> {
    match self.metadata {
      MetadataView::Recordings => {
        let scroll = self
          .metadata_position
          .as_ref()
          .map_or(0, |position| position.scroll);
        self.metadata_position = Some(PagePosition {
          key: Some(file.path.clone()),
          scroll,
        });
        self.metadata = MetadataView::Recording(file);

        Ok(vec![Work::FocusFirst])
      }
      MetadataView::Recording(_) | MetadataView::MusicBrainz(_) => Ok(Vec::new()),
    }
  }

  fn open_musicbrainz(&mut self, query: Search, cached: Cached) -> Result<Vec<Work>, Never> {
    match &self.metadata {
      MetadataView::Recording(file) => match (*file == query.file).then_some(()) {
        Some(()) => self.metadata = MetadataView::MusicBrainz(query.file.clone()),
        None => return Ok(Vec::new()),
      },
      MetadataView::Recordings | MetadataView::MusicBrainz(_) => return Ok(Vec::new()),
    }

    let mut work = vec![Work::FocusFirst];
    let Ok(lookup) = self.lookup(&query.file);

    match (cached, lookup) {
      (Cached::No, Lookup::Ready) => {
        let Ok(started) = self.search(query);
        work.extend(started);
      }
      (Cached::Yes, _) | (Cached::No, Lookup::Pending | Lookup::Failed) => {}
    }

    Ok(work)
  }

  fn metadata_back(&mut self) -> Result<Vec<Work>, Never> {
    self.metadata = match &self.metadata {
      MetadataView::Recordings => return Ok(Vec::new()),
      MetadataView::Recording(_) => MetadataView::Recordings,
      MetadataView::MusicBrainz(file) => MetadataView::Recording(file.clone()),
    };

    Ok(vec![Work::FocusFirst])
  }

  fn search(&mut self, query: Search) -> Result<Option<Work>, Never> {
    let Ok(lookup) = self.lookup(&query.file);

    match (&self.metadata, lookup) {
      (MetadataView::MusicBrainz(file), Lookup::Ready | Lookup::Failed) => {
        match (*file == query.file).then_some(()) {
          Some(()) => {}
          None => return Ok(None),
        }
      }
      (_, Lookup::Pending) | (MetadataView::Recordings | MetadataView::Recording(_), _) => {
        return Ok(None);
      }
    }

    let Ok(id) = self.next_id();
    let _prior = self.searches.insert(
      query.file.path.clone(),
      SearchJob::Pending {
        id,
        file: query.file.clone(),
      },
    );

    Ok(Some(Work::SearchSong { id, query }))
  }

  fn search_finished(&mut self, id: u64, status: ExitStatus) -> Result<Completion, Never> {
    let finished = self.searches.values().find_map(|search| match search {
      SearchJob::Pending { id: expected, file } => (*expected == id).then_some(file),
      SearchJob::Failed(_) => None,
    });
    let file = match finished {
      Some(found) => found.clone(),
      None => return Ok(Completion::Stale),
    };

    match status {
      ExitStatus::Success => {
        let _prior = self.searches.remove(&file.path);
      }
      ExitStatus::Failure(_) | ExitStatus::Interrupt => {
        let _prior = self
          .searches
          .insert(file.path.clone(), SearchJob::Failed(file));
      }
    }

    Ok(Completion::Applied)
  }
}

impl Status {
  fn finished(&mut self, id: u64, status: ExitStatus) -> Result<Completion, Never> {
    Ok(match self {
      Self::Pending(expected) => match (*expected == id).then_some(()) {
        Some(()) => {
          *self = Self::Finished(status);
          Completion::Applied
        }
        None => Completion::Stale,
      },
      Self::Idle | Self::Finished(_) => Completion::Stale,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn query(name: &str) -> Result<Search, Never> {
    let path = PathBuf::from(format!("/music/{name}.flac"));
    let Ok(mut song) = crate::looking::Song::of(&path);
    song.name = name.to_owned();

    crate::suggestions::query(&song)
  }

  #[test]
  fn jobs_for_distinct_recordings_can_finish_out_of_order() {
    let Ok(first) = query("First");
    let Ok(second) = query("Second");
    let Ok((first_recording, _focus)) =
      Management::default().update(Request::OpenMetadata(first.file.clone()));
    let Ok((first_search, first_work)) = first_recording.update(Request::OpenMusicBrainz {
      query: first.clone(),
      cached: Cached::No,
    });
    assert!(first_work.contains(&Work::SearchSong {
      id: 1,
      query: first.clone()
    }));
    let Ok((back_to_first, _focus)) = first_search.update(Request::MetadataBack);
    let Ok((overview, _focus)) = back_to_first.update(Request::MetadataBack);
    assert_eq!(overview.lookup(&first.file), Ok(Lookup::Pending));
    assert_eq!(overview.lookup(&second.file), Ok(Lookup::Ready));

    let Ok((second_recording, _focus)) =
      overview.update(Request::OpenMetadata(second.file.clone()));
    let Ok((both_searches, second_work)) = second_recording.update(Request::OpenMusicBrainz {
      query: second.clone(),
      cached: Cached::No,
    });
    assert!(second_work.contains(&Work::SearchSong {
      id: 2,
      query: second.clone()
    }));
    let Ok((already_pending, duplicate)) =
      both_searches.update(Request::SearchSong(second.clone()));
    assert!(duplicate.is_empty());

    let Ok((second_finished, _redraw)) = already_pending.update(Request::Finished {
      id: 2,
      job: Job::Search,
      status: ExitStatus::Failure(None),
    });
    let Ok((both_finished, _redraw)) = second_finished.update(Request::Finished {
      id: 1,
      job: Job::Search,
      status: ExitStatus::Success,
    });
    assert_eq!(
      both_finished.metadata,
      MetadataView::MusicBrainz(second.file.clone())
    );
    assert_eq!(both_finished.lookup(&first.file), Ok(Lookup::Ready));
    assert_eq!(both_finished.lookup(&second.file), Ok(Lookup::Failed));
  }

  #[test]
  fn backing_out_of_cached_results_restores_the_overview_position() {
    let Ok(song) = query("Example");
    let position = PagePosition {
      key: Some(song.file.path.clone()),
      scroll: 120,
    };
    let Ok((overview, _)) =
      Management::default().update(Request::ViewedMetadata(position.clone()));
    let Ok((recording, _)) = overview.update(Request::OpenMetadata(song.file.clone()));
    let detail_position = PagePosition {
      key: None,
      scroll: 42,
    };
    let Ok((positioned, _)) = recording.update(Request::ViewedMetadata(detail_position.clone()));
    let Ok((results, work)) = positioned.update(Request::OpenMusicBrainz {
      query: song,
      cached: Cached::Yes,
    });
    assert_eq!(work, [Work::FocusFirst]);
    let Ok((same_results, _)) = results.update(Request::ViewedMetadata(PagePosition {
      key: None,
      scroll: 0,
    }));
    let Ok((to_recording, _)) = same_results.update(Request::MetadataBack);
    assert_eq!(to_recording.metadata_position(), Ok(Some(detail_position)));
    let Ok((to_overview, _)) = to_recording.update(Request::MetadataBack);

    assert_eq!(to_overview.metadata_position(), Ok(Some(position)));
  }

  #[test]
  fn the_first_song_list_starts_one_scan_and_manual_refresh_can_retry() {
    let Ok((scanning, first)) = Management::default().update(Request::Opened);
    assert_eq!(first, [Work::Refresh { id: 1 }]);
    let Ok((still_scanning, opened_again)) = scanning.update(Request::Opened);
    assert!(opened_again.is_empty());
    let Ok((finished, _redraw)) = still_scanning.update(Request::Finished {
      id: 1,
      job: Job::Scan,
      status: ExitStatus::Failure(None),
    });
    let Ok((still_finished, reopened)) = finished.update(Request::Opened);
    assert!(reopened.is_empty());
    let Ok((_, retried)) = still_finished.update(Request::Refresh);
    assert!(retried.contains(&Work::Refresh { id: 2 }));
  }

  #[test]
  fn a_stale_reply_does_not_finish_another_job() {
    let Ok((state, first)) = Management::default().update(Request::Refresh);
    assert_eq!(first.first(), Some(&Work::Refresh { id: 1 }));
    let Ok((state_2, second)) = state.update(Request::Refresh);
    assert!(second.is_empty());
    let Ok((state_3, reply)) = state_2.update(Request::Finished {
      id: 0,
      job: Job::Scan,
      status: ExitStatus::Success,
    });
    assert_eq!(state_3.scan, Status::Pending(1));
    assert!(reply.is_empty());
  }
}
