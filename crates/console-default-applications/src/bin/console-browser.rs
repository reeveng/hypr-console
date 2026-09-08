//! The browser this desktop means, opened on whatever it opens on.
//!
//! Bound to View on the front of the machine. Nothing here writes down which
//! browser that is: `xdg-settings` is asked, so the button follows the browser
//! chosen on the settings panel's Defaults tab. A name kept here would be a
//! second answer, and the two would part company the day either of them moved.

use std::path::PathBuf;
use std::process::Command;

use console_default_applications::browsers::asking;
use console_core_external_programs::Program;
use console_core_never::Never;

const ANYWHERE: &str = "https://duckduckgo.com";

pub fn found(desktop: &str, among: &[PathBuf]) -> Result<Option<PathBuf>, Never> {
    match desktop.is_empty() {
        true => return Ok(None),
        false => {}
    }

    Ok(among.iter().map(|at| at.join(desktop)).find(|at| at.is_file()))
}

fn chosen() -> Result<String, Never> {
    let asked = asking()?;

    let (program, rest) = match asked.split_first() {
        Some((program, rest)) => (program, rest),
        None => return Ok(String::new()),
    };

    let said = match Command::new(program).args(rest).output() {
        Ok(said) => said,
        Err(_fault) => return Ok(String::new()),
    };

    Ok(String::from_utf8_lossy(&said.stdout).trim().to_string())
}

fn main() {
    let Ok(chosen) = chosen();
    let Ok(among) = console_core_places::applications();
    let Ok(found) = found(&chosen, &among);

    let Ok(argv) = match found {
        Some(at) => Program::Gio.argv(&["launch", &at.display().to_string()]),
        None => Program::XdgOpen.argv(&[ANYWHERE]),
    };

    match argv.split_first() {
        Some((program, rest)) => {
            let _ = Command::new(program).args(rest).status();
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn somewhere(named: &str) -> PathBuf {
        let here = std::env::temp_dir().join(format!("console-browser-{named}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&here);
        std::fs::create_dir_all(&here).expect("somewhere to look");
        here
    }

    #[test]
    fn the_browser_the_machine_names_is_the_one_that_is_opened() {
        let here = somewhere("named");

        std::fs::write(here.join("librewolf.desktop"), "[Desktop Entry]\n").expect("a browser");

        assert_eq!(
            found("librewolf.desktop", std::slice::from_ref(&here)),
            Ok(Some(here.join("librewolf.desktop")))
        );

        let _ = std::fs::remove_dir_all(&here);
    }

    #[test]
    fn a_browser_the_machine_names_and_does_not_have_is_not_opened() {
        let here = somewhere("gone");
        assert_eq!(found("librewolf.desktop", std::slice::from_ref(&here)), Ok(None));
        let _ = std::fs::remove_dir_all(&here);
    }

    #[test]
    fn a_machine_that_names_no_browser_at_all_opens_the_fallback() {
        assert_eq!(found("", &[PathBuf::from("/usr/share/applications")]), Ok(None));
    }

    #[test]
    fn the_persons_own_copy_is_found_before_the_machines() {
        let mine = somewhere("mine");
        let everyones = somewhere("everyones");
        std::fs::write(mine.join("firefox.desktop"), "[Desktop Entry]\n").expect("mine");
        std::fs::write(everyones.join("firefox.desktop"), "[Desktop Entry]\n").expect("theirs");
        assert_eq!(
            found("firefox.desktop", &[mine.clone(), everyones.clone()]),
            Ok(Some(mine.join("firefox.desktop")))
        );
        let _ = std::fs::remove_dir_all(&mine);
        let _ = std::fs::remove_dir_all(&everyones);
    }
}
