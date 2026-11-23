//! Unix executable lookup by path and permission bits.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use console_core_never::Never;

use super::path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Installed {
  Yes,
  No,
}

impl From<bool> for Installed {
  fn from(found: bool) -> Self {
    match bool::then_some(found, ()) {
      Some(()) => Self::Yes,
      None => Self::No,
    }
  }
}

struct ProgramName<'a>(&'a str);

struct InvalidProgramName;

impl<'a> ProgramName<'a> {
  fn new(program: &'a str) -> Result<Self, InvalidProgramName> {
    match (program, program.split_once(['/', '\0'])) {
      ("" | "." | "..", _) | (_, Some(_)) => Err(InvalidProgramName),
      (_name, None) => Ok(Self(program)),
    }
  }
}

impl AsRef<Path> for ProgramName<'_> {
  fn as_ref(&self) -> &Path {
    Path::new(self.0)
  }
}

pub fn installed(program: &str) -> Result<Installed, Never> {
  let Ok(path) = path();

  match (program.split_once('/'), path) {
    (Some(_), _) => runnable(Path::new(program)),
    (None, None) => Ok(Installed::No),
    (None, Some(search)) => {
      let name = match ProgramName::new(program) {
        Ok(name) => name,
        Err(_not_a_filename) => return Ok(Installed::No),
      };

      let found = search.split(':').filter(|at| !at.is_empty()).any(|at| {
        let Ok(runnable) = runnable(&Path::new(at).join(&name));

        runnable == Installed::Yes
      });

      Ok(Installed::from(found))
    }
  }
}

const RUNNABLE: u32 = 0o111;

fn runnable(at: &Path) -> Result<Installed, Never> {
  let about = match std::fs::metadata(at) {
    Ok(about) => about,
    Err(_nothing_there) => return Ok(Installed::No),
  };

  Ok(Installed::from(
    about.is_file() && about.permissions().mode() & RUNNABLE != 0,
  ))
}
