//! The pool stood up on a socket in a temporary directory, with a source a
//! test speaks through.
//!
//! The serving callback owns the channel it hands to the test when it
//! subscribes. Each integration binary uses the part of this fixture it needs.

use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::time::{Duration, Instant};

use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_events::serving;
use console_events::subscription::Received;
use console_program_contract::{Change, EventGroup};
use console_program_lifetime::threads;
use console_waiting::{Outcome, Ready, Schedule};

pub type Failure = Box<dyn std::error::Error>;

pub const BEFORE_LONG: Duration = Duration::from_secs(5);

const EVERY_SO_OFTEN: Duration = Duration::from_millis(5);

pub fn socket(named: &str) -> Result<PathBuf, Failure> {
  let folder = console_core_temporary_directories::fresh(&format!("events-{named}"))?;

  Ok(folder.join("pool.sock"))
}

pub fn serve_at(at: &Path) -> Result<Receiver<SyncSender<Change>>, Failure> {
  let (handing, handed) = sync_channel(1);

  let serving = at.to_path_buf();
  let Ok(()) = threads::let_go(std::thread::spawn(move || {
    let _the_test_reads_whether_it_came_up = serving::serve(&serving, |event_group, say| {
      console_events::sources::handed_to(Some(&handing), &EventGroup::Sound, event_group, say)
    });
  }));

  up(at)?;

  Ok(handed)
}

pub fn up(at: &Path) -> Result<(), Failure> {
  let Ok(patience) = Schedule::asking_every(BEFORE_LONG, EVERY_SO_OFTEN);
  let Ok(came_up) = console_waiting::until(patience, || {
    Ok(match std::fs::symlink_metadata(at) {
      Ok(details) => match ::core::primitive::bool::then_some(details.file_type().is_socket(), ()) {
        ::core::option::Option::Some(()) => Ready::Yes,
        ::core::option::Option::None => Ready::NotYet,
      },
      Err(_not_ready) => Ready::NotYet,
    })
  });

  match came_up {
    Outcome::Happened => Ok(()),
    Outcome::RanOut => Err(Failure::from(format!(
      "nothing came up at {}",
      at.display()
    ))),
  }
}

pub fn change(text: &str) -> Result<Change, Never> {
  Ok(Change {
    event_group: EventGroup::Sound,
    text: text.to_string(),
  })
}

pub fn before_long(heard: &Receiver<Received>) -> Result<Option<Change>, Never> {
  heard_before(heard, BEFORE_LONG, |received| match received {
    Received::Event(change) => Some(change),
    Received::Connected => None,
  })
}

pub fn heard_before<T>(
  heard: &Receiver<Received>,
  patience: Duration,
  accept: impl Fn(Received) -> Option<T>,
) -> Result<Option<T>, Never> {
  let began = Instant::now();
  let waited = iterate((), |()| {
    let left = patience.saturating_sub(began.elapsed());

    match ::core::primitive::bool::then_some(left.is_zero(), ()) {
      ::core::option::Option::Some(()) => return Ok(Step::Halt(None)),
      ::core::option::Option::None => {}
    }

    Ok(match heard.recv_timeout(left) {
      Ok(received) => match accept(received) {
        Some(accepted) => Step::Halt(Some(accepted)),
        None => Step::Again(()),
      },
      Err(_nothing_before_the_deadline) => Step::Halt(None),
    })
  });

  Ok(match waited {
    Ok(accepted) => accepted,
    Err(Endless) => None,
  })
}
