//! The browser this desktop means, opened on whatever it opens on.
//!
//! Bound to View on the front of the machine. Nothing here writes down which
//! browser that is: `xdg-settings` is asked, so the button follows the browser
//! chosen on the settings panel's Defaults tab. A name kept here would be a
//! second answer, and the two would part company the day either of them moved.

use std::ffi::OsStr;
use std::process::ExitCode;

use console_applications::launch::{self, find_desktop_file};
use console_default_applications::{browsers, opening};

const ANYWHERE: &str = "https://duckduckgo.com";

fn main() -> ExitCode {
  let found = match browsers::current() {
    Ok(chosen) => {
      let Ok(among) = console_core_places::applications();
      let Ok(desktop_file) = find_desktop_file(&chosen, &among);
      desktop_file
    }
    Err(fault) => {
      eprintln!("console-browser: could not query the default browser: {fault}");

      None
    }
  };

  let launched = match found {
    Some(at) => {
      let Ok(mut command) = launch::entry(&at);

      command.status()
    }
    None => {
      let Ok(mut command) = opening::command(OsStr::new(ANYWHERE));

      command.status()
    }
  };

  match launched {
    Ok(status) => match ::core::primitive::bool::then_some(status.success(), ()) {
      ::core::option::Option::Some(()) => ExitCode::SUCCESS,
      ::core::option::Option::None => {
        eprintln!("console-browser: launch failed ({status})");

        ExitCode::FAILURE
      }
    },
    Err(fault) => {
      eprintln!("console-browser: could not launch the browser: {fault}");

      ExitCode::FAILURE
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_core_atomic_writes::whole;
  use std::error::Error;
  use std::path::PathBuf;

  fn somewhere(named: &str) -> Result<PathBuf, console_core_temporary_directories::Unmade> {
    console_core_temporary_directories::fresh(&format!("browser-{named}"))
  }

  #[test]
  fn the_browser_the_machine_names_is_the_one_that_is_opened() -> Result<(), Box<dyn Error>> {
    let here = somewhere("named")?;

    whole(&here.join("librewolf.desktop"), b"[Desktop Entry]\n")?;

    assert_eq!(
      find_desktop_file("librewolf.desktop", std::slice::from_ref(&here)),
      Ok(Some(here.join("librewolf.desktop")))
    );

    std::fs::remove_dir_all(&here)?;

    Ok(())
  }

  #[test]
  fn a_browser_the_machine_names_and_does_not_have_is_not_opened() -> Result<(), Box<dyn Error>> {
    let here = somewhere("gone")?;

    assert_eq!(
      find_desktop_file("librewolf.desktop", std::slice::from_ref(&here)),
      Ok(None)
    );

    std::fs::remove_dir_all(&here)?;

    Ok(())
  }

  #[test]
  fn a_machine_that_names_no_browser_at_all_opens_the_fallback() {
    assert_eq!(
      find_desktop_file("", &[PathBuf::from("/usr/share/applications")]),
      Ok(None)
    );
  }

  #[test]
  fn the_persons_own_copy_is_found_before_the_machines() -> Result<(), Box<dyn Error>> {
    let mine = somewhere("mine")?;
    let everyones = somewhere("everyones")?;

    whole(&mine.join("firefox.desktop"), b"[Desktop Entry]\n")?;
    whole(&everyones.join("firefox.desktop"), b"[Desktop Entry]\n")?;

    assert_eq!(
      find_desktop_file("firefox.desktop", &[mine.clone(), everyones.clone()]),
      Ok(Some(mine.join("firefox.desktop")))
    );

    std::fs::remove_dir_all(&mine)?;
    std::fs::remove_dir_all(&everyones)?;

    Ok(())
  }
}
