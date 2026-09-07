//! The two kinds of thing this fetches, and where a search is kept.
//!
//! The panel holds no results. `download-find` runs off it, writes what came
//! back here, and the panel draws again when that ends and reads whatever is
//! there. So the slow half is a program with a name rather than a thread inside
//! a card, and a search survives the tab being walked away from.

use console_core_never::Never;
use std::path::{Path, PathBuf};

const SOUND_TAB: &str = "Audio";
const FILM_TAB: &str = "Video";

pub const TABS: [&str; 2] = [SOUND_TAB, FILM_TAB];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Sound,
    Film,
}

impl Kind {
    pub const BOTH: [Kind; 2] = [Kind::Sound, Kind::Film];

    pub fn word(self) -> Result<&'static str, Never> {
        Ok(match self {
            Kind::Sound => "audio",
            Kind::Film => "video",
        })
    }

    pub fn flag(self) -> Result<&'static str, Never> {
        Ok(match self {
            Kind::Sound => "--audio",
            Kind::Film => "--video",
        })
    }

    pub fn tab(self) -> Result<&'static str, Never> {
        Ok(match self {
            Kind::Sound => SOUND_TAB,
            Kind::Film => FILM_TAB,
        })
    }

    pub fn other(self) -> Result<Kind, Never> {
        Ok(match self {
            Kind::Sound => Kind::Film,
            Kind::Film => Kind::Sound,
        })
    }

    pub fn read(said: &str) -> Result<Option<Kind>, Never> {
        let said = said.trim().trim_start_matches('-');
        Ok(Kind::BOTH.into_iter().find(|kind| kind.word() == Ok(said)))
    }
}

pub fn folder(cache: &Path) -> Result<PathBuf, Never> {
    Ok(cache.join("console").join("download"))
}

pub fn found_at(cache: &Path, kind: Kind) -> Result<PathBuf, Never> {
    let Ok(folder) = folder(cache);
    let Ok(word) = kind.word();

    Ok(folder.join(format!("{word}.json")))
}

pub fn pictures(cache: &Path) -> Result<PathBuf, Never> {
    let Ok(folder) = folder(cache);

    Ok(folder.join("pictures"))
}

pub const SIDE: &str = "128";

pub fn picture_of(cache: &Path, id: &str) -> Result<Option<PathBuf>, Never> {
    let Ok(named) = named(id);
    let Ok(pictures) = pictures(cache);

    Ok(named.map(|id| pictures.join(format!("{id}.jpg"))))
}

pub fn named(id: &str) -> Result<Option<String>, Never> {
    let plain = |letter: char| letter.is_ascii_alphanumeric() || letter == '-' || letter == '_';

    Ok(match !id.is_empty() && id.chars().all(plain) {
        true => Some(id.to_string()),
        false => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache() -> PathBuf {
        Path::new("/home/ada/.cache").to_path_buf()
    }

    #[test]
    fn a_kind_is_read_from_the_word_one_program_hands_the_next() {
        assert_eq!(Kind::read("--audio"), Ok(Some(Kind::Sound)));
        assert_eq!(Kind::read("video"), Ok(Some(Kind::Film)));
        assert_eq!(Kind::read("--pictures"), Ok(None));
    }

    #[test]
    fn each_tab_keeps_what_it_found_apart_from_the_other() {
        let Ok(sound) = found_at(&cache(), Kind::Sound);
        let Ok(film) = found_at(&cache(), Kind::Film);
        let Ok(folder) = folder(&cache());

        assert_ne!(sound, film);
        assert!(sound.starts_with(folder));
        assert_eq!(Kind::Sound.other(), Ok(Kind::Film));
    }

    #[test]
    fn an_id_that_could_leave_the_folder_is_no_id_at_all() {
        assert_eq!(named("qU9mHegkTc4"), Ok(Some("qU9mHegkTc4".to_string())));
        assert_eq!(named("../../.bashrc"), Ok(None));
        assert_eq!(named("a/b"), Ok(None));
        assert_eq!(named(""), Ok(None));
        assert_eq!(picture_of(&cache(), "../evil"), Ok(None));
    }
}
