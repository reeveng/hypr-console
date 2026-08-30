//! What is in the music folder.

use std::path::{Path, PathBuf};

/// What kew will play, which is what is worth listing.
pub const KINDS: [&str; 9] =
    ["aac", "flac", "m4a", "mp3", "ogg", "opus", "wav", "webm", "wma"];

/// One thing to choose: a folder of songs, or a song.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Thing {
    pub name: String,
    pub path: PathBuf,
    pub folder: bool,
}

/// Where the music is.
///
/// kew's own setting first, so that the panel and the player never disagree
/// about which folder they are talking about.
pub fn folder() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    said_by_kew()
        .map(|said| PathBuf::from(said.replace('~', &home)))
        .unwrap_or_else(|| PathBuf::from(home).join("Music"))
}

fn said_by_kew() -> Option<String> {
    let config = gtk4::glib::user_config_dir().join("kew/kewrc");
    let said = std::fs::read_to_string(config).ok()?;
    path_in(&said)
}

/// The music path out of kew's settings file.
pub fn path_in(kewrc: &str) -> Option<String> {
    kewrc
        .lines()
        .find_map(|line| line.trim().strip_prefix("path="))
        .map(|said| said.trim().to_string())
        .filter(|said| !said.is_empty())
}

/// Tell kew where the music is, if nothing has told it yet.
///
/// kew asks this once, on its first run, by printing the question and reading
/// the answer off the terminal. Started by a panel it has no terminal: it says
/// "Error reading input" into nothing and stops before it has played a note,
/// so every press of A does nothing and says nothing about why. The panel
/// already knows the answer -- it is the folder it is listing -- so it writes
/// it down before kew is ever asked to play.
pub fn tell_kew(folder: &Path) {
    let config = gtk4::glib::user_config_dir().join("kew/kewrc");
    let said = std::fs::read_to_string(&config).unwrap_or_default();
    let Some(writing) = with_path(&said, &folder.to_string_lossy()) else { return };
    let _ = std::fs::create_dir_all(config.parent().unwrap_or(&config));
    let _ = std::fs::write(&config, writing);
}

/// kew's settings file with the music folder named in it, or nothing to do.
///
/// The line is written where it already is rather than added at the end,
/// because the file kew makes on its first run has `path=` in it and empty,
/// and a settings file that says the same thing twice is one nobody can read.
pub fn with_path(kewrc: &str, folder: &str) -> Option<String> {
    if path_in(kewrc).is_some() {
        return None;
    }
    let told = format!("path={folder}");
    let mut lines: Vec<String> = kewrc.lines().map(|line| line.to_string()).collect();
    match lines.iter().position(|line| line.trim().starts_with("path=")) {
        Some(at) => lines[at] = told,
        None => lines.push(told),
    }
    Some(lines.join("\n") + "\n")
}

/// What is in a folder, folders first and each in name order.
pub fn things(folder: &Path) -> Vec<Thing> {
    let Ok(reading) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut things: Vec<Thing> = reading
        .flatten()
        .filter_map(|entry| about(&entry.path()))
        .collect();

    things.sort_by_key(|thing| (!thing.folder, thing.name.to_lowercase()));
    things
}

/// What one path in a music folder is, if it is anything.
fn about(path: &Path) -> Option<Thing> {
    let name = path.file_name()?.to_string_lossy().to_string();

    if name.starts_with('.') {
        return None;
    }
    match path.is_dir() {
        true => Some(Thing { name, path: path.to_path_buf(), folder: true }),
        false => playable(path).then(|| Thing {
            name: named(&name),
            path: path.to_path_buf(),
            folder: false,
        }),
    }
}

/// Whether kew would play this file.
pub fn playable(path: &Path) -> bool {
    let Some(kind) = path.extension() else { return false };
    KINDS.contains(&kind.to_string_lossy().to_lowercase().as_str())
}

/// A filename, as a title.
///
/// The extension goes, and so does the YouTube id a download leaves in square
/// brackets at the end. Both of them are the file's business rather than the
/// song's.
pub fn named(filename: &str) -> String {
    let name = filename.rsplit_once('.').map_or(filename, |(stem, _)| stem);
    match name.rsplit_once(" [") {
        Some((title, tail)) if tail.ends_with(']') => title.trim().to_string(),
        _ => name.trim().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kews_own_path_is_the_one_used() {
        assert_eq!(path_in("path=~/Music\nvolume=50\n"), Some("~/Music".to_string()));
        assert_eq!(path_in("volume=50\n"), None);
    }

    /// The empty one kew writes on its first run is the one that matters: left
    /// as it is, kew stops to ask a question nobody can answer.
    #[test]
    fn a_settings_file_that_names_no_folder_is_given_one() {
        let said = with_path("[miscellaneous]\n\npath=\n\nvolume=50\n", "/home/ada/Music");
        assert_eq!(said.as_deref(), Some("[miscellaneous]\n\npath=/home/ada/Music\n\nvolume=50\n"));
        assert_eq!(with_path("", "/home/ada/Music").as_deref(), Some("path=/home/ada/Music\n"));
    }

    /// A person who has said where their music is has said it, and a panel
    /// that writes over that is a panel that moves their library.
    #[test]
    fn a_settings_file_that_names_one_is_left_alone() {
        assert_eq!(with_path("path=~/Songs\n", "/home/ada/Music"), None);
    }

    #[test]
    fn a_download_keeps_its_title_and_loses_its_id() {
        assert_eq!(named("505 [qU9mHegkTc4].opus"), "505");
        assert_eq!(named("227.Pink + White.flac"), "227.Pink + White");
    }

    #[test]
    fn only_what_the_player_plays_is_listed() {
        assert!(playable(Path::new("/a/b.OPUS")));
        assert!(playable(Path::new("/a/b.mp3")));
        assert!(!playable(Path::new("/a/cover.jpg")));
        assert!(!playable(Path::new("/a/notes")));
    }
}
