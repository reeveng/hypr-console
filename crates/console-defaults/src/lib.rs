//! What this desktop opens things with.
//!
//! Which browser a link means, which engine a question is asked of, and where
//! the battery starts saying something. All three used to be written into a
//! program. A setting nobody can reach is a setting somebody has to be asked
//! to change, and there is nobody to ask on a machine with one person on it.
//!
//! The browser is xdg-settings', because every program on the machine asks
//! that and a second copy here would be a second answer. The other two have no
//! such place, so there is a file, and this is what reads and writes it.

pub mod battery;
pub mod browsers;
pub mod engines;
pub mod policies;

use std::path::PathBuf;

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".to_string()))
}

/// The file the choices that are nobody else's live in.
pub fn where_() -> PathBuf {
    home().join(".config/console/defaults")
}

/// What a settings file says, as a value for each key.
///
/// Written by hand as readily as by the panel, so a line that is not a setting
/// is passed over rather than argued with.
pub fn read(said: &str) -> Vec<(String, String)> {
    said.lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .filter(|(key, _)| !key.is_empty())
        .collect()
}

/// The same, with one key set to something and the rest left where they are.
pub fn written(said: &str, key: &str, value: &str) -> String {
    let mut settings = read(said);
    match settings.iter_mut().find(|(named, _)| named == key) {
        Some(found) => found.1 = value.to_string(),
        None => settings.push((key.to_string(), value.to_string())),
    }
    settings.sort_by(|one, two| one.0.cmp(&two.0));
    settings.iter().map(|(key, value)| format!("{key}={value}\n")).collect()
}

/// What one key says, out of the file as it stands.
pub fn setting(key: &str) -> Option<String> {
    let said = std::fs::read_to_string(where_()).ok()?;
    read(&said).into_iter().find(|(named, _)| named == key).map(|(_, value)| value)
}

/// Set one key, leaving the file's other lines alone.
pub fn set(key: &str, value: &str) {
    let at = where_();
    if let Some(parent) = at.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let said = std::fs::read_to_string(&at).unwrap_or_default();
    let _ = std::fs::write(&at, written(&said, key, value));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_setting_is_a_key_and_a_value() {
        assert_eq!(read("search=startpage\n"), [("search".to_string(), "startpage".to_string())]);
    }

    #[test]
    fn a_file_written_by_hand_is_read_the_same() {
        let said = "# which engine\n  search = startpage  \n\nnonsense\n";
        assert_eq!(read(said), [("search".to_string(), "startpage".to_string())]);
    }

    #[test]
    fn setting_one_leaves_the_others_where_they_were() {
        let said = "browser=librewolf.desktop\nsearch=duckduckgo\n";
        assert_eq!(written(said, "search", "startpage"), "browser=librewolf.desktop\nsearch=startpage\n");
    }

    #[test]
    fn setting_one_that_was_never_there_writes_it() {
        assert_eq!(written("", "search", "wikipedia"), "search=wikipedia\n");
    }
}
