//! Which browser a link means.
//!
//! Not written down here. xdg-settings is what every program on the machine
//! asks, from the menu to whatever a game puts on the screen, so this says
//! which browsers there are to choose between and lets that answer which one
//! is chosen. A copy kept alongside it would be a second answer, and the two
//! would part company the day either of them moved.

use std::io;
use std::path::PathBuf;

use console_applications::launch;
use console_core_external_programs::{Program, Unprinted, capture_output};
use console_core_never::Never;

pub use console_applications::launch::find_desktop_file;

const DEFAULT_BROWSER: &str = "default-web-browser";
const PRIVATE_WINDOW_ACTIONS: &[&str] = &[
  "new-private-window",
  "new-incognito-window",
  "new-inprivate-window",
  "private-window",
  "incognito-window",
  "NewPrivateWindow",
  "NewIncognitoWindow",
  "NewInPrivateWindow",
  "PrivateBrowsing",
  "Incognito",
];

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

pub fn current() -> Result<String, Unprinted> {
  let Ok(settings) = Program::XdgSettings.name();

  capture_output(&[settings, "get", DEFAULT_BROWSER]).map(|said| said.trim().to_owned())
}

pub fn private_window() -> io::Result<Vec<String>> {
  let desktop = current().map_err(io::Error::other)?;
  let Ok(roots) = console_core_places::applications();

  private_window_for(&desktop, &roots)
}

fn private_window_for(desktop: &str, roots: &[PathBuf]) -> io::Result<Vec<String>> {
  let Ok(found) = find_desktop_file(desktop, roots);
  let entry = found.ok_or_else(|| {
    io::Error::new(
      io::ErrorKind::NotFound,
      format!("configured browser desktop entry {desktop:?} was not found"),
    )
  })?;
  let action = launch::action(&entry, PRIVATE_WINDOW_ACTIONS)?;

  action.ok_or_else(|| {
    io::Error::new(
      io::ErrorKind::Unsupported,
      format!("{desktop} does not provide a private-window action"),
    )
  })
}

fn set_default_command(desktop: &str) -> Result<Vec<String>, Never> {
  Program::XdgSettings.arguments(&["set", DEFAULT_BROWSER, desktop])
}

pub fn set_default(desktop: &str) -> Result<(), Unprinted> {
  let Ok(arguments) = set_default_command(desktop);

  capture_output(&arguments).map(|_| ())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::error::Error;
  use std::path::PathBuf;

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
  fn the_configured_browser_supplies_its_own_private_window_command() -> Result<(), Box<dyn Error>>
  {
    const DESKTOP: &str = "example-browser.desktop";
    let at = console_core_temporary_directories::fresh("defaults-private-window")?;
    let entry = at.join(DESKTOP);
    let roots = std::slice::from_ref(&at);

    console_core_atomic_writes::whole(
      &entry,
      b"\
[Desktop Entry]
Exec=example-browser %u
Actions=new-window;NewIncognitoWindow;
[Desktop Action new-window]
Exec=example-browser --new-window %u
[Desktop Action NewIncognitoWindow]
Exec=\"/opt/Example Browser/browser\" --incognito %U
",
    )?;

    let arguments = private_window_for(DESKTOP, roots)?;
    assert_eq!(arguments, ["/opt/Example Browser/browser", "--incognito"]);

    console_core_atomic_writes::whole(&entry, b"[Desktop Entry]\nExec=example-browser %u\n")?;
    let missing = private_window_for(DESKTOP, roots).expect_err("ordinary browser launch");
    assert_eq!(missing.kind(), io::ErrorKind::Unsupported);

    std::fs::remove_dir_all(&at)?;

    Ok(())
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
