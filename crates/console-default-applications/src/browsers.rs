//! Which browser a link means.
//!
//! Not written down here. xdg-settings is what every program on the machine
//! asks, from the menu to whatever a game puts on the screen, so this says
//! which browsers there are to choose between and lets that answer which one
//! is chosen. A copy kept alongside it would be a second answer, and the two
//! would part company the day either of them moved.

use std::path::PathBuf;

use console_core_external_programs::Program;
use console_core_never::Never;

pub struct Browser {
    pub key: &'static str,
    pub says: &'static str,
    pub desktop: &'static str,
}

pub const EVERY: [Browser; 3] = [
    Browser { key: "chromium", says: "Chromium", desktop: "chromium.desktop" },
    Browser { key: "firefox", says: "Firefox", desktop: "firefox.desktop" },
    Browser { key: "librewolf", says: "LibreWolf", desktop: "librewolf.desktop" },
];

pub fn here(among: &[PathBuf]) -> Result<Vec<&'static Browser>, Never> {
    Ok(EVERY
        .iter()
        .filter(|browser| among.iter().any(|at| at.join(browser.desktop).exists()))
        .collect())
}

pub fn asking() -> Result<[&'static str; 3], Never> {
    let Ok(settings) = Program::XdgSettings.name();

    Ok([settings, "get", "default-web-browser"])
}

pub fn telling(desktop: &str) -> Result<Vec<String>, Never> {
    Program::XdgSettings.argv(&["set", "default-web-browser", desktop])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_what_is_installed_is_offered() {
        let at = std::env::temp_dir().join("console-defaults-browsers");
        let _ = std::fs::remove_dir_all(&at);
        std::fs::create_dir_all(&at).expect("somewhere to look");
        std::fs::write(at.join("librewolf.desktop"), "[Desktop Entry]").expect("a browser");
        let among = vec![at.clone(), PathBuf::from("/nowhere")];
        let Ok(found) = here(&among);
        let says: Vec<&str> = found.iter().map(|browser| browser.says).collect();

        assert_eq!(says, ["LibreWolf"]);
        let _ = std::fs::remove_dir_all(&at);
    }

    #[test]
    fn nothing_installed_is_offered_as_nothing() {
        let Ok(found) = here(&[PathBuf::from("/nowhere")]);

        assert!(found.is_empty());
    }

    #[test]
    fn a_browser_is_set_by_the_name_it_is_read_by() {
        let Ok(told) = telling("librewolf.desktop");

        assert_eq!(told.last().expect("a name"), "librewolf.desktop");
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
