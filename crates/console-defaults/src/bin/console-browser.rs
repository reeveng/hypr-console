//! The browser this desktop means, opened on whatever it opens on.
//!
//! Bound to View on the front of the machine. Nothing here writes down which
//! browser that is: `xdg-settings` is asked, so the button follows the browser
//! chosen on the settings panel's Defaults tab. A name kept here would be a
//! second answer, and the two would part company the day either of them moved.

use std::path::PathBuf;
use std::process::Command;

use console_defaults::browsers::{applications, asking};

/// Where a link goes when nothing on the machine claims to be the browser.
///
/// Still a browser: `xdg-open` on a web address is answered by whatever the
/// desktop does have, and a button that opens nothing at all is a button that
/// looks broken.
const ANYWHERE: &str = "https://duckduckgo.com";

/// The .desktop file the machine says is the browser, if it is really there.
///
/// Both halves matter. `xdg-settings` can name a file that has been uninstalled
/// since it was chosen, and launching that is an error nobody sees, because
/// this runs from a button press with no terminal under it.
pub fn found(desktop: &str, among: &[PathBuf]) -> Option<PathBuf> {
    if desktop.is_empty() {
        return None;
    }
    among.iter().map(|at| at.join(desktop)).find(|at| at.is_file())
}

fn chosen() -> String {
    let asked = asking();
    let Ok(said) = Command::new(asked[0]).args(&asked[1..]).output() else {
        return String::new();
    };
    String::from_utf8_lossy(&said.stdout).trim().to_string()
}

fn main() {
    let argv = match found(&chosen(), &applications()) {
        Some(at) => vec!["gio".to_string(), "launch".to_string(), at.display().to_string()],
        None => vec!["xdg-open".to_string(), ANYWHERE.to_string()],
    };
    let _ = Command::new(&argv[0]).args(&argv[1..]).status();
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
            Some(here.join("librewolf.desktop"))
        );
        let _ = std::fs::remove_dir_all(&here);
    }

    /// xdg-settings goes on naming a browser that has been uninstalled since it
    /// was chosen, and launching that is an error nobody sees: this runs from a
    /// button press, with no terminal under it.
    #[test]
    fn a_browser_the_machine_names_and_does_not_have_is_not_opened() {
        let here = somewhere("gone");
        assert_eq!(found("librewolf.desktop", std::slice::from_ref(&here)), None);
        let _ = std::fs::remove_dir_all(&here);
    }

    #[test]
    fn a_machine_that_names_no_browser_at_all_opens_the_fallback() {
        assert_eq!(found("", &[PathBuf::from("/usr/share/applications")]), None);
    }

    /// The first directory that has it wins, which is the order the menu looks
    /// in too: a browser installed for this person stands in front of one
    /// installed for everybody.
    #[test]
    fn the_persons_own_copy_is_found_before_the_machines() {
        let mine = somewhere("mine");
        let everyones = somewhere("everyones");
        std::fs::write(mine.join("firefox.desktop"), "[Desktop Entry]\n").expect("mine");
        std::fs::write(everyones.join("firefox.desktop"), "[Desktop Entry]\n").expect("theirs");
        assert_eq!(
            found("firefox.desktop", &[mine.clone(), everyones.clone()]),
            Some(mine.join("firefox.desktop"))
        );
        let _ = std::fs::remove_dir_all(&mine);
        let _ = std::fs::remove_dir_all(&everyones);
    }
}
