//! Which browser a link means.
//!
//! Not written down here. xdg-settings is what every program on the machine
//! asks, from the menu to whatever a game puts on the screen, so this says
//! which browsers there are to choose between and lets that answer which one
//! is chosen. A copy kept alongside it would be a second answer, and the two
//! would part company the day either of them moved.

use std::path::{Path, PathBuf};

use console_core_external_programs::Program;
use console_core_never::Never;

pub struct Browser {
  pub key: &'static str,
  pub says: &'static str,
  pub desktop: &'static str,
}

pub const EVERY: [Browser; 3] = [
  Browser {
    key: "chromium",
    says: "Chromium",
    desktop: "chromium.desktop",
  },
  Browser {
    key: "firefox",
    says: "Firefox",
    desktop: "firefox.desktop",
  },
  Browser {
    key: "librewolf",
    says: "LibreWolf",
    desktop: "librewolf.desktop",
  },
];

pub fn here(among: &[PathBuf]) -> Result<Vec<&'static Browser>, Never> {
  Ok(
    EVERY
      .iter()
      .filter(|browser| {
        let Ok(found) = find_desktop_file(browser.desktop, among);

        found.is_some()
      })
      .collect(),
  )
}

struct DesktopFile<'a>(&'a Path);

impl AsRef<Path> for DesktopFile<'_> {
  fn as_ref(&self) -> &Path {
    self.0
  }
}

pub fn find_desktop_file(desktop: &str, among: &[PathBuf]) -> Result<Option<PathBuf>, Never> {
  let path = Path::new(desktop);
  let named = match ::core::primitive::bool::then_some(path.file_name() == Some(path.as_os_str()), ()) {
    ::core::option::Option::Some(()) => DesktopFile(path),
    ::core::option::Option::None => return Ok(None),
  };

  Ok(among.iter().map(|at| at.join(&named)).find(|candidate| {
    match std::fs::metadata(candidate) {
      Ok(metadata) => metadata.is_file(),
      Err(_desktop_file_metadata_unavailable) => false,
    }
  }))
}

pub fn get_default_command() -> Result<[&'static str; 3], Never> {
  let Ok(settings) = Program::XdgSettings.name();

  Ok([settings, "get", "default-web-browser"])
}

pub fn set_default_command(desktop: &str) -> Result<Vec<String>, Never> {
  Program::XdgSettings.arguments(&["set", "default-web-browser", desktop])
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::error::Error;

  #[test]
  fn only_what_is_installed_is_offered() -> Result<(), Box<dyn Error>> {
    let at = console_core_temporary_directories::fresh("defaults-browsers")?;

    console_core_atomic_writes::whole(&at.join("librewolf.desktop"), b"[Desktop Entry]")?;

    let among = vec![at.clone(), PathBuf::from("/nowhere")];
    let Ok(found) = here(&among);
    let says: Vec<&str> = found.iter().map(|browser| browser.says).collect();

    assert_eq!(says, ["LibreWolf"]);

    std::fs::remove_dir_all(&at)?;

    Ok(())
  }

  #[test]
  fn nothing_installed_is_offered_as_nothing() {
    let Ok(found) = here(&[PathBuf::from("/nowhere")]);

    assert!(found.is_empty());
  }

  #[test]
  fn a_browser_is_set_by_the_name_it_is_read_by() {
    let Ok(told) = set_default_command("librewolf.desktop");

    assert_eq!(told.last().map(String::as_str), Some("librewolf.desktop"));
  }

  #[test]
  fn every_browser_is_named_once_and_in_order() {
    let keys: Vec<&str> = EVERY.iter().map(|browser| browser.key).collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(keys, sorted, "the browsers are out of order or named twice");
  }
}
