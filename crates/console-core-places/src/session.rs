//! Launch arguments published by an open Console app for the session saver.
//! The process start time prevents stale files from following a reused PID.

use std::io;
use std::path::{Path, PathBuf};

use console_core_atomic_writes::{Stored, Unwritten};
use console_core_never::Never;

const PROGRAMS: [(&str, &str); 12] = [
  ("books", "console-application-books"),
  ("calculator", "console-application-calculator"),
  ("console-browser-window", "console-application-browser"),
  ("console-camera", "console-application-camera"),
  ("console-clipboard", "console-application-clipboard"),
  ("console-task-manager", "console-application-task-manager"),
  ("downloads", "console-application-downloads"),
  ("files", "console-application-files"),
  ("habits", "console-application-habits"),
  ("music", "console-application-music"),
  ("notes", "console-application-notes"),
  ("viewer", "console-application-media-viewer"),
];

pub fn application(executable: &str) -> Result<Option<&'static str>, Never> {
  Ok(
    PROGRAMS
      .iter()
      .find_map(|&(id, program)| (executable == id || executable == program).then_some(id)),
  )
}

pub fn renamed(executable: &str) -> Result<Option<&'static str>, Never> {
  Ok(
    PROGRAMS
      .iter()
      .find_map(|&(id, program)| (executable == id).then_some(program)),
  )
}

struct ProcessFilename(String);

impl From<u32> for ProcessFilename {
  fn from(pid: u32) -> Self {
    Self(pid.to_string())
  }
}

impl AsRef<Path> for ProcessFilename {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

fn at(pid: u32) -> Result<Option<PathBuf>, Never> {
  let Ok(runtime) = super::runtime();
  let filename = ProcessFilename::from(pid);

  Ok(runtime.map(|root| root.join("console").join("session-apps").join(&filename)))
}

fn start(pid: u32) -> io::Result<Option<String>> {
  let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;

  Ok(
    stat
      .rsplit_once(')')
      .and_then(|(_, fields)| fields.split_whitespace().nth(19))
      .map(str::to_owned),
  )
}

fn process_record(pid: u32) -> Result<Option<(PathBuf, String)>, Never> {
  let Ok(record) = at(pid);

  Ok(match (record, start(pid)) {
    (Some(path), Ok(Some(started))) => Some((path, started)),
    (None, _) | (Some(_), Ok(None)) => None,
    (Some(_), Err(_unreadable_process_stat)) => None,
  })
}

pub fn publish(application: &str, arguments: &[&str]) -> Result<(), Unwritten> {
  let Ok(canonical_application) = self::application(application);
  let name = match canonical_application {
    Some(known) => known,
    None => return Ok(()),
  };

  let executable = match std::env::current_exe() {
    Ok(executable) => executable,
    Err(_unavailable) => return Ok(()),
  };
  let executable_name = executable
    .file_name()
    .and_then(|filename| filename.to_str());

  match executable_name.and_then(|binary| {
    let Ok(known) = self::application(binary);
    known.filter(|candidate| *candidate == name)
  }) {
    Some(_) => {}
    None => return Ok(()),
  }

  let Ok(active) = process_record(std::process::id());
  let (record, started) = match active {
    Some(active_record) => active_record,
    None => return Ok(()),
  };

  match arguments.iter().find(|argument| argument.contains('\0')) {
    Some(_) => return Ok(()),
    None => {}
  }

  let contents = std::iter::once(started.as_str())
    .chain(std::iter::once(name))
    .chain(arguments.iter().copied())
    .collect::<Vec<_>>()
    .join("\0");
  console_core_atomic_writes::whole_with_folders(&record, contents.as_bytes())
}

pub fn arguments(pid: u32, application: &str) -> Result<Option<Vec<String>>, Never> {
  let Ok(active) = process_record(pid);
  let (record, started) = match active {
    Some(active_record) => active_record,
    None => return Ok(None),
  };

  let Ok(stored) = console_core_atomic_writes::read(&record);
  let held = match stored {
    Stored::Text(text) => text,
    Stored::Absent | Stored::Failed(_) => return Ok(None),
  };

  let mut said = held.split('\0');
  let actual = (said.next(), said.next());
  let expected = (Some(started.as_str()), Some(application));

  Ok(match (actual == expected).then_some(()) {
    Some(()) => Some(said.map(str::to_owned).collect()),
    None => None,
  })
}

pub fn forget() -> Result<(), Never> {
  let Ok(record) = at(std::process::id());

  match record {
    Some(at) => {
      let _gone = std::fs::remove_file(at);
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}
