//! The browser this desktop means, opened on whatever it opens on.
//!
//! Bound to View on the front of the machine. Nothing here writes down which
//! browser that is: `xdg-settings` is asked, so the button follows the browser
//! chosen on the settings panel's Defaults tab. A name kept here would be a
//! second answer, and the two would part company the day either of them moved.

use std::path::PathBuf;
use std::process::Command;

use console_default_applications::browsers::get_default_command;
use console_core_external_programs::Program;
use console_core_never::Never;

const ANYWHERE: &str = "https://duckduckgo.com";

pub fn find_desktop_file(desktop: &str, among: &[PathBuf]) -> Result<Option<PathBuf>, Never> {
    match desktop.is_empty() {
        true => return Ok(None),
        false => {}
    }

    Ok(among.iter().map(|at| at.join(desktop)).find(|at| at.is_file()))
}

fn current_browser() -> Result<String, Never> {
    let asked = get_default_command()?;

    Ok(match console_core_external_programs::capture_output(&asked) {
        Ok(said) => said.trim().to_string(),
        Err(_unprinted) => String::new(),
    })
}

fn main() {
    let Ok(chosen) = current_browser();
    let Ok(among) = console_core_places::applications();
    let Ok(found) = find_desktop_file(&chosen, &among);

    let Ok(arguments) = match found {
        Some(at) => Program::Gio.arguments(&["launch", &at.display().to_string()]),
        None => Program::XdgOpen.arguments(&[ANYWHERE]),
    };

    match arguments.split_first() {
        Some((program, rest)) => {
            let _ = Command::new(program).args(rest).status();
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_core_atomic_writes::whole;
    use std::error::Error;

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

        let _ = std::fs::remove_dir_all(&here);

        Ok(())
    }

    #[test]
    fn a_browser_the_machine_names_and_does_not_have_is_not_opened() -> Result<(), Box<dyn Error>> {
        let here = somewhere("gone")?;

        assert_eq!(find_desktop_file("librewolf.desktop", std::slice::from_ref(&here)), Ok(None));

        let _ = std::fs::remove_dir_all(&here);

        Ok(())
    }

    #[test]
    fn a_machine_that_names_no_browser_at_all_opens_the_fallback() {
        assert_eq!(find_desktop_file("", &[PathBuf::from("/usr/share/applications")]), Ok(None));
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

        let _ = std::fs::remove_dir_all(&mine);
        let _ = std::fs::remove_dir_all(&everyones);

        Ok(())
    }
}
