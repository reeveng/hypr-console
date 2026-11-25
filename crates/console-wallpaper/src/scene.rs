//! Executable Wayland wallpapers registered in a user's Console configuration.

use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use console_core_never::Never;
use console_core_places::Base;

use crate::choose::Wanted;
use crate::place::Name;

pub(crate) const DIRECTORY: &str = "wallpaper-scenes";

fn directory() -> Result<Option<PathBuf>, Never> {
  let Ok(config) = Base::Configuration.ours();

  Ok(config.map(|at| at.join(DIRECTORY)))
}

enum Execution {
  Executable,
  Unavailable,
}

fn executable(path: &Path) -> Result<Execution, Never> {
  Ok(match path.metadata() {
    Ok(metadata) => {
      match (metadata.is_file() && metadata.permissions().mode() & 0o111 != 0).then_some(()) {
        Some(()) => Execution::Executable,
        None => Execution::Unavailable,
      }
    }
    Err(_unavailable) => Execution::Unavailable,
  })
}

pub fn selected(wanted: &Wanted) -> Result<Option<PathBuf>, Never> {
  let Ok(selection) = wanted.scene();
  let Ok(folder) = directory();

  let (name, at) = match (selection, folder) {
    (Some(name), Some(at)) => (name, at),
    (None, _) | (Some(_), None) => return Ok(None),
  };
  let checked = match Name::new(name) {
    Ok(checked) => checked,
    Err(_invalid_name) => return Ok(None),
  };
  let path = at.join(checked);
  let Ok(execution) = executable(&path);

  Ok(match execution {
    Execution::Executable => Some(path),
    Execution::Unavailable => None,
  })
}

pub fn every() -> Result<Vec<String>, Never> {
  let Ok(folder) = directory();
  let at = match folder {
    Some(at) => at,
    None => return Ok(Vec::new()),
  };
  let entries = match std::fs::read_dir(&at) {
    Ok(entries) => entries,
    Err(fault) => {
      match (fault.kind() == std::io::ErrorKind::NotFound).then_some(()) {
        Some(()) => {}
        None => console_core_logging::warn!("{}: {fault}", at.display()),
      }

      return Ok(Vec::new());
    }
  };

  let mut names: Vec<_> = entries
    .filter_map(|received| match received {
      Ok(entry) => {
        let path = entry.path();
        let Ok(execution) = executable(&path);

        match execution {
          Execution::Executable => {
            path
              .file_name()
              .and_then(OsStr::to_str)
              .and_then(|name| match Name::new(name) {
                Ok(_checked) => Some(name.to_owned()),
                Err(_invalid_name) => None,
              })
          }
          Execution::Unavailable => None,
        }
      }
      Err(fault) => {
        console_core_logging::warn!("{}: {fault}", at.display());

        None
      }
    })
    .collect();
  names.sort();

  Ok(names)
}
