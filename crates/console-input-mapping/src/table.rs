//! The table on this device, and where it is.
//!
//! Two programs write it -- the screen and the card it raises -- so where it
//! is and what is in it are here once rather than in both.
//!
//! Writing the file is the whole of a move now. It used to be half: a button's
//! meaning lived in an InputPlumber profile under `/etc`, so saying a job had
//! moved meant asking root to write the profiles again, and the screen carried
//! the one line in this repository that crossed into `/etc` on someone's say
//! so. The profile no longer decides what a button means -- it says only what
//! each button is -- and the daemon reads this file itself. There is nothing
//! left to make true.

use std::collections::btree_map::Entry;
use std::path::{Path, PathBuf};
use std::process::Command;

use console_core_atomic_writes::Stored;
use console_core_never::Never;
use console_input_bindings::moved::{Tasks, path_in};
use console_input_controller::actions::{MappedAction, Table, special_workspace_job};
use console_input_gamepad::front::{DEVICES, Front, Read, capabilities_command};

use crate::Unmapped;

pub fn at() -> Result<Option<PathBuf>, Never> {
  let Ok(home) = console_core_places::home();

  let home_3 = match home {
    Some(home_2) => home_2,
    None => return Ok(None),
  };

  let Ok(at) = path_in(&home_3);

  Ok(Some(at))
}

pub fn read() -> Result<Tasks, Never> {
  let Ok(table_path) = at();

  let at = match table_path {
    Some(path) => path,
    None => return Ok(Tasks::default()),
  };

  Ok(match read_at(&at) {
    Ok(jobs) => jobs,
    Err(fault) => {
      console_core_logging::warn!("{fault}");
      Tasks::default()
    }
  })
}

pub fn special_workspaces() -> Result<Vec<String>, Never> {
  let Ok(jobs) = read();
  let Ok(table) = Table::of(&jobs);
  let Ok(every) = table.every();

  Ok(every
    .filter_map(|(job, _)| match &job.action {
      MappedAction::SpecialWorkspace(name) => Some(name.clone()),
      MappedAction::Builtin(_) => None,
    })
    .collect())
}

fn read_at(path: &Path) -> Result<Tasks, Unmapped> {
  let Ok(stored) = console_core_atomic_writes::read(path);

  match stored {
    Stored::Text(contents) => {
      Tasks::read(&contents).map_err(|fault| Unmapped::Decoding(path.to_path_buf(), fault))
    }
    Stored::Absent => Ok(Tasks::default()),
    Stored::Failed(fault) => Err(Unmapped::Reading(path.to_path_buf(), fault)),
  }
}

pub enum Change<T> {
  Save(T),
  Keep(T),
}

pub fn register_special_workspace(name: &str) -> Result<(), Unmapped> {
  let Ok(job) = special_workspace_job(name);
  let registration = job.ok_or(Unmapped::InvalidWorkspaceName);
  let slug = registration?;

  change(move |jobs| {
    Ok(match jobs.moved.entry(slug) {
      Entry::Occupied(_) => Change::Keep(()),
      Entry::Vacant(entry) => {
        let _bindings = entry.insert(Vec::new());
        Change::Save(())
      }
    })
  })
}

pub fn change<T>(
  apply: impl FnOnce(&mut Tasks) -> Result<Change<T>, Never>,
) -> Result<T, Unmapped> {
  let Ok(path) = at();
  let target = match path {
    Some(target) => target,
    None => return Err(Unmapped::NoOnes),
  };

  change_at(&target, apply)
}

fn change_at<T>(
  target: &Path,
  apply: impl FnOnce(&mut Tasks) -> Result<Change<T>, Never>,
) -> Result<T, Unmapped> {
  let lock = lock_at(target);
  let _held = lock?;
  let current = read_at(target);
  let mut jobs = current?;
  let Ok(change) = apply(&mut jobs);

  match change {
    Change::Save(value) => {
      let Ok(contents) = jobs.serialize();
      let saved = store(target, contents.as_bytes());
      saved?;
      Ok(value)
    }
    Change::Keep(value) => Ok(value),
  }
}

pub(crate) fn write_contents(path: &Path, contents: &[u8]) -> Result<(), Unmapped> {
  let lock = lock_at(path);
  let _held = lock?;

  store(path, contents)
}

fn lock_at(path: &Path) -> Result<std::fs::File, Unmapped> {
  match path.parent() {
    Some(holding) => std::fs::create_dir_all(holding)
      .map_err(|fault| Unmapped::Holding(holding.to_path_buf(), fault))?,
    None => {}
  }

  let lock = path.with_extension("lock");
  let opened = console_core_atomic_writes::open_lock_file(&lock)
    .map_err(|fault| Unmapped::Locking(lock.clone(), fault));
  let file = opened?;
  let locked = file
    .try_lock()
    .map_err(|fault| Unmapped::Locking(lock, std::io::Error::from(fault)));
  locked?;
  Ok(file)
}

fn store(at: &Path, contents: &[u8]) -> Result<(), Unmapped> {
  console_core_atomic_writes::whole(at, contents).map_err(Unmapped::Writing)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn overlapping_edits_do_not_overwrite_the_shortcut_table()
  -> Result<(), Box<dyn std::error::Error>> {
    let fresh = console_core_temporary_directories::fresh("mapping-lock");
    let root = fresh?;
    let file = root.join("buttons.toml");
    let locked = lock_at(&file);
    let guard = locked?;

    assert!(matches!(
      change_at(&file, |_| Ok(Change::Save(()))),
      Err(Unmapped::Locking(_, fault)) if fault.kind() == std::io::ErrorKind::WouldBlock
    ));
    drop(guard);

    let saved = change_at(&file, |jobs| {
      jobs
        .moved
        .insert("special-workspace-notes".to_owned(), Vec::new());
      Ok(Change::Save(()))
    });
    saved?;
    let read = read_at(&file);
    let jobs = read?;

    assert!(jobs.moved.contains_key("special-workspace-notes"));
    let removed = std::fs::remove_dir_all(root);
    removed?;
    Ok(())
  }
}

pub fn table() -> Result<Table, Never> {
  let Ok(jobs) = read();

  Table::of(&jobs)
}

pub fn front() -> Result<Front, Never> {
  let Ok(asking) = capabilities_command();
  let Ok(asked) = run_output(&asking);
  let Ok(devices) = devices();

  Front::of(Read {
    said: &asked,
    devices: &devices,
  })
}

fn devices() -> Result<String, Never> {
  Ok(match std::fs::read_to_string(DEVICES) {
    Ok(said) => said,

    Err(fault) => {
      eprintln!("{DEVICES}: reading what this machine's inputs are: {fault}");
      String::new()
    }
  })
}

pub fn run_output(arguments: &[&str]) -> Result<String, Never> {
  let (program, rest) = match arguments.split_first() {
    Some((program, rest)) => (program, rest),
    None => return Ok(String::new()),
  };

  Ok(match Command::new(program).args(rest).output() {
    Ok(done) => match std::str::from_utf8(&done.stdout) {
      Ok(text) => text.trim().to_string(),
      Err(fault) => {
        eprintln!(
          "{}: command output is not UTF-8: {fault}",
          arguments.join(" ")
        );

        String::new()
      }
    },

    Err(fault) => {
      eprintln!("{}: {fault}", arguments.join(" "));
      String::new()
    }
  })
}
