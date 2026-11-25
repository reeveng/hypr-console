//! The panel that was up when the machine went down, so it comes back up.
//!
//! A restart that nobody asked for -- the battery running out, the button held,
//! a deploy restarting the host -- should put back what was on the screen, the
//! way a book comes back at its page. So the host writes which panel it put up
//! and what it was asked with, every time it puts one up, and takes it away
//! when a person puts the panel away: B, a row that closes it, or its own
//! button pressed a second time. Restart and Shut Down are rows that close the
//! panel as they run, so what they leave behind is nothing, and the machine
//! that comes back from them comes back to the desktop rather than to Settings.
//!
//! What is not a person putting it away is left alone. The host being asked to
//! stop and the panel's program being ended are what a machine going down looks
//! like from here, and forgetting then is forgetting the one time it mattered.
//!
//! Put back is the panel's own program started again with the same arguments,
//! which is every road in: it takes the lock, asks the host, and the host draws
//! it as though it had been pressed.
//!
//! The live record is published in the session directory. A single worker
//! copies the newest value to persistent storage so opening or closing never
//! waits for a disk flush. The picker that dismisses a panel from another
//! process uses the same persistence lock and reads the newest record after
//! taking it.

use std::io;
use std::path::PathBuf;

use console_concurrency::{Latest, WorkerStopped};
use console_core_atomic_writes::{Stored, open_lock_file, publish};
use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_operating_system_linux::program_runtime::commands::{Linux, Timing};
use console_program_contract::{Command, Executable};
use console_program_runtime::commands::{self as execution_commands, Launch, Streams};
use rustix::fs::{FlockOperation, flock};

use crate::notes::{self, Note};

pub const NOTE: Note<'static> = Note {
  program: "panels",
  called: "left-open",
};

const BETWEEN: char = '\0';

const LIVE_NOTE: &str = "left-open";
const SAVE_LOCK: &str = "left-open.lock";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftOpen {
  pub who: String,
  pub arguments: Vec<String>,
}

pub fn serialize(open: &LeftOpen) -> Result<String, Never> {
  let words: Vec<&str> = std::iter::once(open.who.as_str())
    .chain(open.arguments.iter().map(String::as_str))
    .collect();

  Ok(words.join(&BETWEEN.to_string()))
}

pub fn read(said: &str) -> Result<Option<LeftOpen>, Never> {
  let mut words = said.split(BETWEEN);

  Ok(match words.next() {
    Some("") | None => None,
    Some(who) => Some(LeftOpen {
      who: who.to_string(),
      arguments: words.map(str::to_string).collect(),
    }),
  })
}

struct LiveNote {
  file: PathBuf,
  lock: PathBuf,
}

impl LiveNote {
  fn find() -> Result<Option<Self>, Never> {
    let Ok(runtime) = console_core_places::application_runtime();

    Ok(runtime.map(|directory| Self {
      file: directory.join(LIVE_NOTE),
      lock: directory.join(SAVE_LOCK),
    }))
  }

  fn read(&self) -> Result<Option<String>, Never> {
    let Ok(record) = console_core_atomic_writes::read(&self.file);

    Ok(match record {
      Stored::Text(said) => Some(said),
      Stored::Absent => None,
      Stored::Failed(fault) => {
        console_core_logging::warn!("console-panel: reading the live panel record: {fault}");
        None
      }
    })
  }

  fn publish(&self, record: &str) -> Result<(), Never> {
    match publish(&self.file, record.as_bytes()) {
      Ok(()) => {}
      Err(fault) => {
        console_core_logging::warn!("console-panel: publishing the panel record: {fault}")
      }
    }

    Ok(())
  }

  fn save(&self, store: impl FnOnce(&str) -> Result<(), Never>) -> io::Result<()> {
    let lock = open_lock_file(&self.lock)?;
    flock(&lock, FlockOperation::LockExclusive)?;

    match self.read() {
      Ok(Some(latest)) => {
        let Ok(()) = store(&latest);
      }
      Ok(None) => {}
    }

    Ok(())
  }

  fn save_reported(&self, store: impl FnOnce(&str) -> Result<(), Never>) -> Result<(), Never> {
    match self.save(store) {
      Ok(()) => {}
      Err(fault) => {
        console_core_logging::warn!("console-panel: saving the panel record: {fault}")
      }
    }

    Ok(())
  }
}

pub struct Journal {
  live: LiveNote,
  worker: Latest,
}

impl Journal {
  pub fn start() -> io::Result<Self> {
    let Ok(found) = LiveNote::find();
    let live =
      found.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "panel session directory"))?;
    let journal = Self::with_note(live, |record| notes::write(NOTE, record))?;
    let Ok(()) = journal.wake();

    Ok(journal)
  }

  fn with_note(
    live: LiveNote,
    mut store: impl FnMut(&str) -> Result<(), Never> + Send + 'static,
  ) -> io::Result<Self> {
    let saving = LiveNote {
      file: live.file.clone(),
      lock: live.lock.clone(),
    };
    let worker = Latest::start("panel journal".to_owned(), move || {
      saving.save_reported(&mut store)
    })?;

    Ok(Self { live, worker })
  }

  pub fn remember(&self, open: Option<&LeftOpen>) -> Result<(), Never> {
    let Ok(record) = record(open);
    let Ok(()) = self.live.publish(&record);

    self.wake()
  }

  fn wake(&self) -> Result<(), Never> {
    match self.worker.schedule() {
      Ok(()) => {}
      Err(WorkerStopped) => {
        console_core_logging::warn!("console-panel: the panel record worker has stopped");
      }
    }

    Ok(())
  }

  pub fn close(self) -> Result<(), Never> {
    match self.worker.close() {
      Ok(()) => {}
      Err(_panicked) => {
        console_core_logging::warn!("console-panel: the panel record worker stopped unexpectedly")
      }
    }

    Ok(())
  }
}

fn record(open: Option<&LeftOpen>) -> Result<String, Never> {
  match open {
    Some(value) => serialize(value),
    None => Ok(String::new()),
  }
}

pub fn remember_directly(open: Option<&LeftOpen>, published: impl FnOnce()) -> Result<(), Never> {
  let Ok(record) = record(open);
  let Ok(found) = LiveNote::find();

  match found {
    Some(live) => {
      let Ok(()) = live.publish(&record);
      published();

      live.save_reported(|latest| notes::write(NOTE, latest))
    }
    None => {
      published();

      notes::write(NOTE, &record)
    }
  }
}

pub fn load() -> Result<Option<LeftOpen>, Never> {
  let Ok(found) = LiveNote::find();

  match found {
    Some(live) => match live.read() {
      Ok(Some(record)) => return read(&record),
      Ok(None) => {}
    },
    None => {}
  }

  let Ok(said) = notes::read(NOTE);

  match said {
    Some(said_2) => read(&said_2),
    None => Ok(None),
  }
}

pub fn started(program: InternalProgram, arguments: Vec<String>) -> Result<(), Never> {
  let command = Command {
    program: Executable::Internal(program),
    arguments,
  };
  let Ok(mut platform) = Linux::new(Timing::Background);

  match execution_commands::spawn(&mut platform, &command, Streams::QUIET, Launch::Application) {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!("console-panels: putting a panel back: {fault}"),
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::mpsc::sync_channel;
  use std::time::Duration;

  #[test]
  fn a_panel_and_its_arguments_come_back_as_they_went_in() {
    let open = LeftOpen {
      who: "settings-panel".to_string(),
      arguments: vec!["Sound".to_string(), "two words".to_string(), String::new()],
    };
    let Ok(said) = serialize(&open);

    assert_eq!(read(&said), Ok(Some(open)));
  }

  #[test]
  fn a_panel_put_away_leaves_nothing_to_put_back() {
    assert_eq!(read(""), Ok(None));
  }

  #[test]
  fn a_slow_save_does_not_delay_dismissal_or_put_back_an_older_panel()
  -> Result<(), Box<dyn std::error::Error>> {
    let here = console_core_temporary_directories::private("left-open")?;
    let live = LiveNote {
      file: here.join(LIVE_NOTE),
      lock: here.join(SAVE_LOCK),
    };
    let (started, saving) = sync_channel(1);
    let (release, allowed) = sync_channel(1);
    let (saved, observed) = sync_channel(2);
    let deadline = Duration::from_secs(2);
    let journal = Journal::with_note(live, move |record| {
      match (record == "first").then_some(()) {
        Some(()) => {
          assert_eq!(started.try_send(()), Ok(()));
          assert_eq!(allowed.recv_timeout(deadline), Ok(()));
        }
        None => {}
      }

      assert_eq!(saved.try_send(record.to_owned()), Ok(()));
      Ok(())
    })?;
    let first = LeftOpen {
      who: "first".to_owned(),
      arguments: Vec::new(),
    };
    let second = LeftOpen {
      who: "second".to_owned(),
      arguments: Vec::new(),
    };
    let Ok(()) = journal.remember(Some(&first));
    saving.recv_timeout(deadline)?;
    let Ok(()) = journal.remember(Some(&second));
    let Ok(()) = journal.remember(None);

    assert_eq!(
      journal
        .live
        .read()
        .map(|current| current.as_deref() == Some("")),
      Ok(true)
    );

    release.try_send(())?;
    let saved_first = observed.recv_timeout(deadline)?;
    let saved_last = observed.recv_timeout(deadline)?;
    assert_eq!(saved_first, "first");
    assert_eq!(saved_last, "");
    let Ok(()) = journal.close();

    Ok(())
  }
}
