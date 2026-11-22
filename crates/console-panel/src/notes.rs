//! Where a panel keeps what it remembers about itself between openings.
//!
//! Under the state directory rather than the cache, because these are things
//! the desktop remembers about itself rather than things it can work out
//! again. One file each, named for the panel and for the note, so a panel that
//! learns to remember something new does not have to be taught where.
//!
//! None of it is required to work. A file that cannot be read or written is a
//! panel that opens the way it did before there was one.

use console_core_never::Never;
use console_core_places::{APPLICATION, Base};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Note<'a> {
  pub program: &'a str,
  pub called: &'a str,
}

pub(crate) struct NoteFile<'a>(&'a OsStr);

impl<'a> NoteFile<'a> {
  pub(crate) fn new(name: &'a str) -> Result<Option<Self>, Never> {
    let path = Path::new(name);

    Ok(
      path
        .file_name()
        .filter(|component| *component == path.as_os_str())
        .map(Self),
    )
  }
}

impl AsRef<Path> for NoteFile<'_> {
  fn as_ref(&self) -> &Path {
    Path::new(self.0)
  }
}

pub fn beside(note: Note<'_>) -> Result<Option<PathBuf>, Never> {
  let state = Base::State.user()?;

  under(state.as_deref(), note)
}

fn under(state: Option<&Path>, note: Note<'_>) -> Result<Option<PathBuf>, Never> {
  let Note { program, called } = note;

  let name = format!("{program}.{called}");
  let Ok(file) = NoteFile::new(&name);

  Ok(
    state
      .zip(file)
      .map(|(base, leaf)| base.join(APPLICATION).join("panel").join(leaf)),
  )
}

pub fn write(note: Note<'_>, said: &str) -> Result<(), Never> {
  let Ok(beside) = beside(note);

  let path = match beside {
    Some(path) => path,
    None => return Ok(()),
  };

  let holding = match path.parent() {
    Some(holding) => holding,
    None => return Ok(()),
  };

  match std::fs::create_dir_all(holding) {
    Ok(_) => {}
    Err(fault) => {
      console_core_logging::warn!(
        "console: {}: keeping a panel's note: {fault}",
        holding.display()
      );

      return Ok(());
    }
  }

  match console_core_atomic_writes::whole(&path, said.as_bytes()) {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!(
      "console: {}: keeping a panel's note: {fault}",
      path.display()
    ),
  }

  Ok(())
}

pub fn read(note: Note<'_>) -> Result<Option<String>, Never> {
  let Ok(beside) = beside(note);

  let path = match beside {
    Some(path) => path,
    None => return Ok(None),
  };

  let said = match std::fs::read_to_string(path) {
    Ok(said) => said,
    Err(_unreadable) => return Ok(None),
  };

  Ok(Some(said))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_note_is_named_for_the_panel_and_for_itself() {
    assert_eq!(
      under(
        Some(Path::new("/home/console-test/.local/state")),
        Note {
          program: "settings-panel",
          called: "tab"
        }
      ),
      Ok(Some(PathBuf::from(
        "/home/console-test/.local/state/console/panel/settings-panel.tab"
      )))
    );
  }

  #[test]
  fn a_note_name_cannot_leave_its_directory() {
    let base = Some(Path::new("/home/console-test/.local/state"));

    for program in ["/elsewhere", "../parent", "also/subdirectory"] {
      assert_eq!(
        under(
          base,
          Note {
            program,
            called: "tab"
          }
        ),
        Ok(None)
      );
    }
  }

  #[test]
  fn nothing_named_is_nothing_remembered() {
    assert_eq!(
      under(
        None,
        Note {
          program: "settings-panel",
          called: "tab"
        }
      ),
      Ok(None)
    );
  }
}
