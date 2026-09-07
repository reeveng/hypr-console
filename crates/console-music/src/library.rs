//! What is in the music folder.

use console_core_never::Never;
use std::path::{Path, PathBuf};

pub const KINDS: [&str; 9] =
    ["aac", "flac", "m4a", "mp3", "ogg", "opus", "wav", "webm", "wma"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Thing {
    pub name: String,
    pub path: PathBuf,
    pub folder: bool,
}

pub fn folder() -> Result<PathBuf, Never> {
    let home = match std::env::var("HOME") {
        Ok(home) => home,

        Err(fault) => {
            eprintln!("console-music: HOME: {fault}; the music folder is looked for from here");
            String::new()
        }
    };

    let said = said_by_kew()?;

    Ok(said
        .map(|said| PathBuf::from(said.replace('~', &home)))
        .unwrap_or_else(|| PathBuf::from(home).join("Music")))
}

fn said_by_kew() -> Result<Option<String>, Never> {
    let config = gtk4::glib::user_config_dir().join("kew/kewrc");

    let Ok(said) = std::fs::read_to_string(config) else { return Ok(None) };

    path_in(&said)
}

pub fn path_in(kewrc: &str) -> Result<Option<String>, Never> {
    Ok(kewrc
        .lines()
        .find_map(|line| line.trim().strip_prefix("path="))
        .map(|said| said.trim().to_string())
        .filter(|said| !said.is_empty()))
}

pub fn tell_kew(folder: &Path) -> Result<(), Never> {
    let config = gtk4::glib::user_config_dir().join("kew/kewrc");
    let said = match std::fs::read_to_string(&config) {
        Ok(said) => said,

        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => String::new(),

        Err(fault) => {
            eprintln!("console-music: {}: reading kew's settings: {fault}", config.display());
            String::new()
        }
    };

    let with_path = with_path(&said, &folder.to_string_lossy())?;

    let Some(writing) = with_path else { return Ok(()) };

    let _ = std::fs::create_dir_all(config.parent().unwrap_or(&config));
    let _ = std::fs::write(&config, writing);

    Ok(())
}

pub fn with_path(kewrc: &str, folder: &str) -> Result<Option<String>, Never> {
    let already = path_in(kewrc)?;

    match already.is_some() {
        true => return Ok(None),
        false => {},
    }

    let told = format!("path={folder}");
    let mut lines: Vec<String> = kewrc.lines().map(|line| line.to_string()).collect();

    match lines.iter().position(|line| line.trim().starts_with("path=")) {
        Some(at) => match lines.get_mut(at) {
            Some(line) => *line = told,
            None => lines.push(told),
        },
        None => lines.push(told),
    }

    Ok(Some(lines.join("\n") + "\n"))
}

pub fn things(folder: &Path) -> Result<Vec<Thing>, Never> {
    let Ok(reading) = std::fs::read_dir(folder) else { return Ok(Vec::new()) };

    let mut things: Vec<Thing> = Vec::new();

    for entry in reading.flatten() {
        let about = about(&entry.path())?;

        match about {
            Some(thing) => things.push(thing),
            None => {},
        }
    }

    things.sort_by_key(|thing| (!thing.folder, thing.name.to_lowercase()));

    Ok(things)
}

fn about(path: &Path) -> Result<Option<Thing>, Never> {
    let Some(called) = path.file_name() else { return Ok(None) };

    let name = called.to_string_lossy().to_string();

    match name.starts_with('.') {
        true => return Ok(None),
        false => {},
    }

    let playable = playable(path)?;

    match path.is_dir() {
        true => Ok(Some(Thing { name, path: path.to_path_buf(), folder: true })),
        false => match playable {
            Playable::Yes => {
                let named = named(&name)?;

                Ok(Some(Thing { name: named, path: path.to_path_buf(), folder: false }))
            }
            Playable::No => Ok(None),
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Playable {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    AFolder,
    ASong,
}

pub fn playable(path: &Path) -> Result<Playable, Never> {
    let Some(kind) = path.extension() else { return Ok(Playable::No) };

    Ok(match KINDS.contains(&kind.to_string_lossy().to_lowercase().as_str()) {
        true => Playable::Yes,
        false => Playable::No,
    })
}

pub fn named(filename: &str) -> Result<String, Never> {
    let name = filename.rsplit_once('.').map_or(filename, |(stem, _)| stem);

    Ok(match name.rsplit_once(" [") {
        Some((title, tail)) if tail.ends_with(']') => title.trim().to_string(),
        Some(_) | None => name.trim().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kews_own_path_is_the_one_used() {
        assert_eq!(path_in("path=~/Music\nvolume=50\n"), Ok(Some("~/Music".to_string())));
        assert_eq!(path_in("volume=50\n"), Ok(None));
    }

    #[test]
    fn a_settings_file_that_names_no_folder_is_given_one() {
        let Ok(said) = with_path("[miscellaneous]\n\npath=\n\nvolume=50\n", "/home/ada/Music");

        let Ok(empty) = with_path("", "/home/ada/Music");

        assert_eq!(said.as_deref(), Some("[miscellaneous]\n\npath=/home/ada/Music\n\nvolume=50\n"));
        assert_eq!(empty.as_deref(), Some("path=/home/ada/Music\n"));
    }

    #[test]
    fn a_settings_file_that_names_one_is_left_alone() {
        assert_eq!(with_path("path=~/Songs\n", "/home/ada/Music"), Ok(None));
    }

    #[test]
    fn a_download_keeps_its_title_and_loses_its_id() {
        assert_eq!(named("505 [qU9mHegkTc4].opus"), Ok("505".to_string()));
        assert_eq!(named("227.Pink + White.flac"), Ok("227.Pink + White".to_string()));
    }

    #[test]
    fn only_what_the_player_plays_is_listed() {
        assert_eq!(playable(Path::new("/a/b.OPUS")), Ok(Playable::Yes));
        assert_eq!(playable(Path::new("/a/b.mp3")), Ok(Playable::Yes));
        assert_eq!(playable(Path::new("/a/cover.jpg")), Ok(Playable::No));
        assert_eq!(playable(Path::new("/a/notes")), Ok(Playable::No));
    }
}
