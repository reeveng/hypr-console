//! Starting a desktop application with a file. Its Exec field codes specify
//! where the file argument goes. The panel launcher lets the app keep running
//! after Files closes.

use std::path::Path;

use console_applications::launch;
use console_core_never::Never;
use console_default_applications::editors::{self, Application, Support};

use crate::paths;

pub fn programs(kind: &str) -> Result<Vec<Application>, Never> {
  match ::core::primitive::bool::then_some(kind.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      return Ok(Vec::new());
    }
    ::core::option::Option::None => {}
  }

  let installed = editors::installed()?;
  let mut found: Vec<Application> = installed
    .into_iter()
    .filter(|app| {
      let Ok(support) = app.opens(kind);

      support == Support::Yes
    })
    .collect();

  found.sort_by(|one, two| one.says.cmp(&two.says));
  found.dedup_by(|one, two| one.says == two.says);

  Ok(found)
}

pub fn started(at: &Path, path: &Path) -> Result<(), Never> {
  let file = match paths::text(path) {
    Ok(file) => file,
    Err(fault) => {
      console_core_logging::warn!("{fault}");

      return Ok(());
    }
  };
  let arguments = match launch::for_file(at, file) {
    Ok(arguments) => arguments,
    Err(fault) => {
      console_core_logging::warn!("{}: {fault}", at.display());

      return Ok(());
    }
  };

  console_panel::running::left_running(&arguments)
}
