//! Desktop actions for saved browser downloads.

use std::io;
use std::path::{Path, PathBuf};

pub(crate) use console_browser_downloads::{Id, Staging, folder, locate, stored};

pub(crate) fn open_in_desktop(path: PathBuf) -> io::Result<()> {
  let Ok(mut command) = console_default_applications::opening::command(path.as_os_str());
  let child = console_program_lifetime::let_go(&mut command)?;
  let Ok(()) =
    console_operating_system_linux::program_runtime::commands::reap_in_background(child);
  Ok(())
}

pub(crate) fn reveal(root: &Path, id: &Id) -> io::Result<()> {
  let path = locate(root, id)?;

  match console_default_applications::opening::show_item(&path, std::time::Instant::now) {
    Ok(()) => Ok(()),
    Err(fault) => {
      console_core_logging::debug!("selecting download in the file manager: {fault}");
      open_in_desktop(root.join(id))
    }
  }
}

pub(crate) fn trash(root: &Path, id: &Id) -> io::Result<()> {
  let target = console_browser_downloads::trash_target(root, id)?;

  console_gio::trash(&target)
}
