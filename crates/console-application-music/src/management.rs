//! Navigation and requests for catalog work. Jobs carry the identity of their completion.

use std::path::PathBuf;

use console_core_never::Never;
use console_music_player::selection::Visibility;
use console_program_contract::ExitStatus;

use crate::albums::Key;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Management {
  pub album: Option<Key>,
  pub scan: Status,
  pub cover: Status,
  pub saving: Status,
  pub search: Status,
  turn: u64,
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
  Refresh,
  Cover(crate::album_art::Lookup),
  PlayAlbum(AlbumStart),
  SearchSong(crate::suggestions::Search),
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
    query: crate::suggestions::Search,
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
      Request::Refresh => match self.scan {
        Status::Pending(_) => Vec::new(),
        Status::Idle | Status::Finished(_) => {
          let Ok(id) = self.job(Job::Scan);

          vec![Work::Refresh { id }, Work::Redraw]
        }
      },
      Request::Cover(album) => match self.cover {
        Status::Pending(_) => Vec::new(),
        Status::Idle | Status::Finished(_) => {
          let Ok(id) = self.job(Job::Cover);

          vec![Work::Cover { id, album }, Work::Redraw]
        }
      },
      Request::PlayAlbum(paths) => vec![Work::PlayAlbum(paths)],
      Request::SearchSong(query) => match self.search {
        Status::Pending(_) => Vec::new(),
        Status::Idle | Status::Finished(_) => {
          let Ok(id) = self.job(Job::Search);

          vec![Work::SearchSong { id, query }, Work::Redraw]
        }
      },
      Request::ConfirmSong(choice) => match self.saving {
        Status::Pending(_) => Vec::new(),
        Status::Idle | Status::Finished(_) => {
          let Ok(id) = self.job(Job::Selection);

          vec![Work::ConfirmSong { id, choice }]
        }
      },
      Request::Select(changes) => match self.saving {
        Status::Pending(_) => Vec::new(),
        Status::Idle | Status::Finished(_) => {
          let Ok(id) = self.job(Job::Selection);

          vec![Work::Select { id, changes }]
        }
      },
      Request::Finished { id, job, status } => {
        let Ok(pending) = self.status(job);

        match pending {
          Status::Pending(expected) => match (*expected == id).then_some(()) {
            Some(()) => {
              *pending = Status::Finished(status);
              vec![Work::Redraw]
            }
            None => Vec::new(),
          },
          Status::Idle | Status::Finished(_) => Vec::new(),
        }
      }
    };

    Ok((self, work))
  }

  fn job(&mut self, job: Job) -> Result<u64, Never> {
    self.turn = self.turn.saturating_add(1);
    let id = self.turn;
    let Ok(status) = self.status(job);
    *status = Status::Pending(id);

    Ok(id)
  }

  fn status(&mut self, job: Job) -> Result<&mut Status, Never> {
    Ok(match job {
      Job::Scan => &mut self.scan,
      Job::Cover => &mut self.cover,
      Job::Selection => &mut self.saving,
      Job::Search => &mut self.search,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_stale_reply_does_not_finish_another_job() {
    let Ok((state, first)) = Management::default().update(Request::Refresh);
    assert_eq!(first.first(), Some(&Work::Refresh { id: 1 }));
    let Ok((state, second)) = state.update(Request::Refresh);
    assert!(second.is_empty());
    let Ok((state, reply)) = state.update(Request::Finished {
      id: 0,
      job: Job::Scan,
      status: ExitStatus::Success,
    });
    assert_eq!(state.scan, Status::Pending(1));
    assert!(reply.is_empty());
  }
}
