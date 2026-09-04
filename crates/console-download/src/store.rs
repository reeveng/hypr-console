//! The two kinds of thing this fetches, and where a search is kept.
//!
//! The panel holds no results. `download-find` runs off it, writes what came
//! back here, and the panel draws again when that ends and reads whatever is
//! there. So the slow half is a program with a name rather than a thread inside
//! a card, and a search survives the tab being walked away from.

use std::path::{Path, PathBuf};

/// What each tab says on the strip.
pub const TABS: [&str; 2] = ["Audio", "Video"];

/// What is being fetched, which is the whole of the difference between the two
/// tabs.
///
/// One search answers both: the same thing on the same site is a song or a
/// film depending only on what is asked for out of it, which is why these are
/// two tabs of one panel rather than two programs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    /// The sound of it, into the folder the music player reads.
    Sound,
    /// The whole of it, into Videos.
    Film,
}

impl Kind {
    /// Both of them, in the order the shoulders walk them in.
    pub const BOTH: [Kind; 2] = [Kind::Sound, Kind::Film];

    /// The word it is written down under, in the cache and on a command line.
    pub fn word(self) -> &'static str {
        match self {
            Kind::Sound => "audio",
            Kind::Film => "video",
        }
    }

    /// How one program here names it to the next.
    pub fn flag(self) -> &'static str {
        match self {
            Kind::Sound => "--audio",
            Kind::Film => "--video",
        }
    }

    /// The tab it is drawn on.
    pub fn tab(self) -> &'static str {
        match self {
            Kind::Sound => TABS[0],
            Kind::Film => TABS[1],
        }
    }

    /// The other one, which is what Y over a row offers.
    pub fn other(self) -> Kind {
        match self {
            Kind::Sound => Kind::Film,
            Kind::Film => Kind::Sound,
        }
    }

    /// Which kind a word on a command line means, with or without its dashes.
    pub fn read(said: &str) -> Option<Kind> {
        let said = said.trim().trim_start_matches('-');
        Kind::BOTH.into_iter().find(|kind| kind.word() == said)
    }
}

/// Where this panel keeps what it has been told.
pub fn folder(cache: &Path) -> PathBuf {
    cache.join("console").join("download")
}

/// What one tab's last search came to.
pub fn found_at(cache: &Path, kind: Kind) -> PathBuf {
    folder(cache).join(format!("{}.json", kind.word()))
}

/// Where the pictures a search fetched are kept.
///
/// Not the store the desktop shares. That one is named for the address of a
/// file on this machine, and these are pictures of things that are not on this
/// machine and mostly never will be.
pub fn pictures(cache: &Path) -> PathBuf {
    folder(cache).join("pictures")
}

/// How big a picture is kept, on its longest side.
///
/// A row draws one 32 points across and scales down to it. Kept larger than
/// that so a row that grows one day does not send every picture back to the
/// site, and nothing like the 360 across the site offers.
pub const SIDE: &str = "128";

/// The picture of one thing, by the name the site knows it as.
pub fn picture_of(cache: &Path, id: &str) -> Option<PathBuf> {
    named(id).map(|id| pictures(cache).join(format!("{id}.jpg")))
}

/// An id, if it is one that can safely be a filename.
///
/// A site's id for a thing arrives over the network and is written into a path
/// here, and a path built out of somebody else's answer is a path that can
/// leave the folder it was meant to be in. Everything a site has ever called a
/// video is letters, digits, a dash or an underscore; anything else is refused
/// rather than cleaned up, because a cleaned-up id is one two things can share.
pub fn named(id: &str) -> Option<String> {
    let plain = |letter: char| letter.is_ascii_alphanumeric() || letter == '-' || letter == '_';

    match !id.is_empty() && id.chars().all(plain) {
        true => Some(id.to_string()),
        false => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache() -> PathBuf {
        Path::new("/home/ada/.cache").to_path_buf()
    }

    #[test]
    fn a_kind_is_read_from_the_word_one_program_hands_the_next() {
        assert_eq!(Kind::read("--audio"), Some(Kind::Sound));
        assert_eq!(Kind::read("video"), Some(Kind::Film));
        assert_eq!(Kind::read("--pictures"), None);
    }

    #[test]
    fn each_tab_keeps_what_it_found_apart_from_the_other() {
        let sound = found_at(&cache(), Kind::Sound);
        let film = found_at(&cache(), Kind::Film);
        assert_ne!(sound, film);
        assert!(sound.starts_with(folder(&cache())));
        assert_eq!(Kind::Sound.other(), Kind::Film);
    }

    /// The id comes off the network and becomes a filename, which is the shape
    /// of every path that ever walked out of the folder it was meant for.
    #[test]
    fn an_id_that_could_leave_the_folder_is_no_id_at_all() {
        assert_eq!(named("qU9mHegkTc4"), Some("qU9mHegkTc4".to_string()));
        assert_eq!(named("../../.bashrc"), None);
        assert_eq!(named("a/b"), None);
        assert_eq!(named(""), None);
        assert_eq!(picture_of(&cache(), "../evil"), None);
    }
}
