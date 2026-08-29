//! Which browser a link means.
//!
//! Not written down here. xdg-settings is what every program on the machine
//! asks, from the menu to whatever a game puts on the screen, so this says
//! which browsers there are to choose between and lets that answer which one
//! is chosen. A copy kept alongside it would be a second answer, and the two
//! would part company the day either of them moved.

use std::path::{Path, PathBuf};

/// One browser: what it is called here, on screen, and in a .desktop file.
pub struct Browser {
    pub key: &'static str,
    pub says: &'static str,
    pub desktop: &'static str,
}

/// The browsers offered, in the order they are drawn.
pub const EVERY: [Browser; 3] = [
    Browser { key: "chromium", says: "Chromium", desktop: "chromium.desktop" },
    Browser { key: "firefox", says: "Firefox", desktop: "firefox.desktop" },
    Browser { key: "librewolf", says: "LibreWolf", desktop: "librewolf.desktop" },
];

/// Where a .desktop file is looked for, which is where the menu looks too.
pub fn applications() -> Vec<PathBuf> {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".to_string()));
    let dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());
    let mut every = vec![home.join(".local/share/applications")];
    every.extend(dirs.split(':').filter(|at| !at.is_empty()).map(|at| Path::new(at).join("applications")));
    every
}

/// The browsers actually on this machine.
///
/// A row for one that is not installed is a row that changes the setting and
/// then opens nothing, which reads as a link that has stopped working rather
/// than a browser that was never there.
pub fn here(among: &[PathBuf]) -> Vec<&'static Browser> {
    EVERY.iter().filter(|browser| among.iter().any(|at| at.join(browser.desktop).exists())).collect()
}

/// What xdg-settings was asked, and what it is told.
pub fn asking() -> [&'static str; 3] {
    ["xdg-settings", "get", "default-web-browser"]
}

pub fn telling(desktop: &str) -> Vec<String> {
    ["xdg-settings", "set", "default-web-browser", desktop]
        .iter()
        .map(|word| (*word).to_string())
        .collect()
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
        let says: Vec<&str> = here(&among).iter().map(|browser| browser.says).collect();
        assert_eq!(says, ["LibreWolf"]);
        let _ = std::fs::remove_dir_all(&at);
    }

    #[test]
    fn nothing_installed_is_offered_as_nothing() {
        assert!(here(&[PathBuf::from("/nowhere")]).is_empty());
    }

    #[test]
    fn a_browser_is_set_by_the_name_it_is_read_by() {
        assert_eq!(telling("librewolf.desktop").last().expect("a name"), "librewolf.desktop");
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
