//! What a typed word finds in the music library.
//!
//! The Music tab is a folder read one folder at a time, which is the right way
//! to walk a library somebody knows and the wrong way to find one song in nine
//! hundred. So the line at the top of it is not a filter on what is in front of
//! you: it looks at everything under the music folder, and at what each of
//! those files says about itself as well as at what it is called.
//!
//! What it is called is free and what it says is not: reading one file takes an
//! ffprobe, and reading the library takes minutes. So the two are separate. The
//! walk happens here, on every letter, and it is fast; the reading happens once
//! in `music-index` and is written down beside the cache, and a song nobody has
//! read yet is still found by its name.
//!
//! The order is the whole point of the thing. A word is looked for in the
//! song's name first, then in whose it is, then in everything else it says, and
//! within each of those the more of it the word was the higher it stands. Typing
//! "nujabes" puts the song called that above the songs by him, and both above
//! the one that merely mentions him in a description.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::library::{self, Thing};
use crate::tags::Tags;

/// How many songs one walk is worth, and how many folders it is worth reading.
///
/// A library is a folder of songs and a few folders of albums, and both ends
/// are here for the same reason the file panel has them: a folder that is a
/// link to the one above it is walked for as long as the panel is open.
const ENOUGH: usize = 4000;
const FAR: usize = 400;

/// One song, and whatever is known about it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Song {
    pub path: PathBuf,
    /// What the file is called, as a title. What there is to go on before
    /// anything has opened it.
    pub name: String,
    pub tags: Tags,
    /// Whether the file has been read.
    ///
    /// Not the same as having said something. A great many songs here are
    /// somebody's download with no tags in it at all, and a song that was
    /// opened and said nothing has to be told apart from one nobody has opened
    /// yet -- otherwise the library is read again, every time, for ever.
    pub read: bool,
}

impl Song {
    /// A song nobody has read yet.
    pub fn of(path: &Path) -> Self {
        let name = path.file_name().map(|name| name.to_string_lossy().to_string());
        Song {
            name: library::named(&name.unwrap_or_default()),
            path: path.to_path_buf(),
            ..Song::default()
        }
    }

    /// What the row says: what the song calls itself, or what the file is
    /// called when it calls itself nothing.
    pub fn says(&self) -> &str {
        match self.tags.title.is_empty() {
            true => &self.name,
            false => &self.tags.title,
        }
    }

    /// What is said beside it: whose it is, or, failing that, where it is.
    ///
    /// One or the other rather than both. Three songs of the same name are a
    /// list nobody can choose from, and either answer settles it.
    pub fn aside(&self, folder: &Path) -> String {
        if !self.tags.artist.is_empty() {
            return self.tags.artist.clone();
        }

        let within = self.path.parent().and_then(|at| {
            let Ok(within) = at.strip_prefix(folder) else { return None };

            Some(within)
        });

        match within {
            Some(within) if !within.as_os_str().is_empty() => within.display().to_string(),
            _ => String::new(),
        }
    }
}

// ------------------------------------------------------------- what there is

/// Every song under a folder, the nearest first.
///
/// The reading is handed in rather than done here, so the walk can be asked
/// without a disk to ask it of.
pub fn under(folder: &Path, read: &dyn Fn(&Path) -> Vec<Thing>) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    let mut waiting = VecDeque::from([folder.to_path_buf()]);
    let mut read_so_far = 0;

    // A folder at a time, nearest first, so a walk that is cut short is cut
    // off the far end of the library rather than out of the middle of it.
    while let Some(at) = waiting.pop_front() {
        if found.len() >= ENOUGH || read_so_far >= FAR {
            break;
        }

        read_so_far += 1;

        for thing in read(&at) {
            match thing.folder {
                true => waiting.push_back(thing.path),
                false => found.push(thing.path),
            }
        }
    }

    found
}

/// Every song under a folder, carrying what has already been read about it.
///
/// The walk decides what exists and the index only says what those files said,
/// so a song fetched a minute ago is in the list at once and a song deleted a
/// minute ago is out of it, whatever the index still holds.
pub fn songs(folder: &Path, read: &dyn Fn(&Path) -> Vec<Thing>, known: &[Song]) -> Vec<Song> {
    let known: HashMap<&Path, &Song> =
        known.iter().map(|song| (song.path.as_path(), song)).collect();
    under(folder, read)
        .into_iter()
        .map(|path| match known.get(path.as_path()) {
            Some(song) => (*song).clone(),
            None => Song::of(&path),
        })
        .collect()
}

/// How many of them nobody has read yet.
pub fn unread(songs: &[Song]) -> usize {
    songs.iter().filter(|song| !song.read).count()
}

// -------------------------------------------------------- what was found out

/// Where what the songs say is written down.
pub fn at(cache: &Path) -> PathBuf {
    cache.join("console").join("music").join("songs.json")
}

/// The songs that have been read, as the file holds them.
///
/// Only the ones that have been read. An unread song is the absence of a line
/// here rather than a line saying it is missing, which is also what makes the
/// file stand for the work already done.
pub fn written(songs: &[Song]) -> String {
    let held: Vec<Value> = songs
        .iter()
        .filter(|song| song.read)
        .map(|song| {
            json!({
                "path": song.path.to_string_lossy(),
                "title": song.tags.title,
                "artist": song.tags.artist,
                "rest": song.tags.rest,
            })
        })
        .collect();

    match serde_json::to_string(&json!({ "songs": held })) {
        Ok(written) => written,

        Err(fault) => {
            eprintln!("music-index: writing down what was read about the songs: {fault}");
            String::new()
        }
    }
}

/// What was written down, read back.
pub fn kept(said: &str) -> Vec<Song> {
    let held: Value = match serde_json::from_str(said) {
        Ok(held) => held,

        // Not json is a file from an older version of this, or one written
        // half way. The walk rebuilds it either way, and this is read on every
        // draw of the panel, so it is not worth a line each time.
        Err(_) => Value::Null,
    };

    let Some(songs) = held.get("songs").and_then(Value::as_array) else {
        return Vec::new();
    };

    songs.iter().filter_map(one).collect()
}

/// One line of it, if it is enough of one to be a song.
fn one(held: &Value) -> Option<Song> {
    let said = |key: &str| {
        held.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
    };
    let path = said("path");

    if path.is_empty() {
        return None;
    }

    Some(Song {
        read: true,
        tags: Tags { title: said("title"), artist: said("artist"), rest: said("rest") },
        ..Song::of(Path::new(&path))
    })
}

// ------------------------------------------------------------- what is found

/// Which of the three things a word was found in, the closest first.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum In {
    /// The song itself: what it calls itself, or what the file is called.
    Song,
    Artist,
    /// Everything else it says about itself.
    Else,
}

/// How much of that thing the word turned out to be, the most first.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum How {
    /// The whole of it.
    Whole,
    /// The start of it.
    Start,
    /// The start of a word in it, which is how a surname is typed.
    Word,
    /// Somewhere in it.
    Anywhere,
}

/// How much of one thing the typed word was, if it was in it at all.
pub fn how(said: &str, wanted: &str) -> Option<How> {
    let said = said.trim().to_lowercase();
    let wanted = wanted.trim().to_lowercase();

    if said.is_empty() || wanted.is_empty() {
        return None;
    }

    if said == wanted {
        return Some(How::Whole);
    }

    if said.starts_with(&wanted) {
        return Some(How::Start);
    }

    let at_a_word = said.match_indices(&wanted).any(|(at, _)| {
        said[..at].chars().next_back().is_some_and(|before| !before.is_alphanumeric())
    });

    match at_a_word {
        true => Some(How::Word),
        false => said.contains(&wanted).then_some(How::Anywhere),
    }
}

/// Where the word stands in one song, if it is anywhere in it.
///
/// The best of the three, and the three are ranked before the amounts are: a
/// word that is merely somewhere in a song's name still beats one that is the
/// whole of an artist's, because what was typed was almost always a song.
pub fn rank(song: &Song, word: &str) -> Option<(In, How)> {
    let itself = [how(song.says(), word), how(&song.name, word)].into_iter().flatten().min();

    [
        (In::Song, itself),
        (In::Artist, how(&song.tags.artist, word)),
        (In::Else, how(&song.tags.rest, word)),
    ]
    .into_iter()
    .filter_map(|(what, how)| how.map(|how| (what, how)))
    .min()
}

/// The songs a word answers for, the closest first.
///
/// Sorted rather than reordered: songs that stand equally well stay in the
/// order the walk found them, which is the order the folder is in.
pub fn ranked<'a>(songs: &'a [Song], word: &str) -> Vec<&'a Song> {
    let mut found: Vec<((In, How), &Song)> =
        songs.iter().filter_map(|song| rank(song, word).map(|rank| (rank, song))).collect();

    found.sort_by_key(|(rank, _)| *rank);
    found.into_iter().map(|(_, song)| song).collect()
}

#[cfg(test)]
mod tests {
    use crate::tags::Said;
    use super::*;

    /// A library with no disk under it: a folder, and what is in it.
    fn tree(at: &Path) -> Vec<Thing> {
        let of = |folders: &[&str], songs: &[&str]| {
            let thing = |name: &str, folder: bool| Thing {
                name: match folder {
                    true => name.to_string(),
                    false => library::named(name),
                },
                path: at.join(name),
                folder,
            };
            let mut things: Vec<Thing> =
                folders.iter().map(|name| thing(name, true)).collect();
            things.extend(songs.iter().map(|name| thing(name, false)));
            things
        };
        match at.to_string_lossy().as_ref() {
            "/music" => of(&["Nujabes"], &["505 [qU9mHegkTc4].opus"]),
            "/music/Nujabes" => of(&[], &["aruarian dance.mp3"]),
            _ => Vec::new(),
        }
    }

    fn a_song(name: &str, title: &str, artist: &str, rest: &str) -> Song {
        Song {
            read: true,
            tags: Tags {
                title: title.to_string(),
                artist: artist.to_string(),
                rest: rest.to_string(),
            },
            ..Song::of(Path::new(name))
        }
    }

    fn library() -> Vec<Song> {
        vec![
            a_song("/music/505.opus", "505", "Arctic Monkeys", "Favourite Worst Nightmare"),
            a_song("/music/Aruarian Dance.mp3", "", "Nujabes", "Samurai Champloo"),
            a_song("/music/Nujabes Tribute.opus", "Nujabes Tribute", "Someone", ""),
            a_song("/music/Luv Sic.opus", "Luv (sic) Part 3", "Shing02", "by Nujabes"),
        ]
    }

    fn said(found: &[&Song]) -> Vec<String> {
        found.iter().map(|song| song.says().to_string()).collect()
    }

    #[test]
    fn a_word_reaches_the_songs_in_the_folders_under_this_one() {
        let found = under(Path::new("/music"), &tree);
        assert_eq!(
            found,
            [
                PathBuf::from("/music/505 [qU9mHegkTc4].opus"),
                PathBuf::from("/music/Nujabes/aruarian dance.mp3"),
            ],
            "the folders under it are walked into, and the nearest comes first"
        );
    }

    /// What the walk finds is what there is. The index only says what those
    /// files said when somebody read them.
    #[test]
    fn a_song_nobody_has_read_is_still_a_song() {
        let known = vec![a_song("/music/505 [qU9mHegkTc4].opus", "505", "Arctic Monkeys", "")];
        let songs = songs(Path::new("/music"), &tree, &known);
        assert_eq!(songs.len(), 2);
        assert_eq!(songs[0].says(), "505");
        assert!(songs[0].read);
        assert_eq!(songs[1].says(), "aruarian dance");
        assert!(!songs[1].read);
        assert_eq!(unread(&songs), 1);
    }

    /// Which is what stops the library being read again every time the tab is
    /// opened: most of these files have no tags at all, and a song that was
    /// read and said nothing must not look like one nobody has opened.
    #[test]
    fn a_song_that_was_read_and_said_nothing_counts_as_read() {
        let songs = kept(&written(&[a_song("/music/quiet.opus", "", "", "")]));
        assert_eq!(unread(&songs), 0);
        assert_eq!(songs[0].tags.anything(), Said::Nothing);
    }

    #[test]
    fn what_was_written_down_is_what_is_read_back() {
        let songs = kept(&written(&library()));
        assert_eq!(songs, library());
        assert!(kept("").is_empty());
        assert!(kept("not json").is_empty());
    }

    #[test]
    fn the_whole_of_a_name_beats_the_start_of_one_and_the_start_beats_the_middle() {
        assert_eq!(how("505", "505"), Some(How::Whole));
        assert_eq!(how("505 Live", "505"), Some(How::Start));
        assert_eq!(how("Live at 505", "505"), Some(How::Word));
        assert_eq!(how("Live at 1505", "505"), Some(How::Anywhere));
        assert_eq!(how("Live", "505"), None);
        assert_eq!(how("505", "  "), None);
    }

    #[test]
    fn the_case_it_was_typed_in_does_not_matter() {
        assert_eq!(how("Arctic Monkeys", "ARCTIC"), Some(How::Start));
        assert_eq!(said(&ranked(&library(), "ARCTIC")), ["505"]);
    }

    /// What was typed was almost always a song, so a song called it comes
    /// before the songs by whoever is called it, and both come before the one
    /// that merely mentions them.
    #[test]
    fn the_song_ranks_above_the_artist_and_the_artist_above_the_rest() {
        let library = library();
        let found = ranked(&library, "nujabes");
        assert_eq!(said(&found), ["Nujabes Tribute", "Aruarian Dance", "Luv (sic) Part 3"]);
        assert_eq!(rank(found[0], "nujabes"), Some((In::Song, How::Start)));
        assert_eq!(rank(found[1], "nujabes"), Some((In::Artist, How::Whole)));
        assert_eq!(rank(found[2], "nujabes"), Some((In::Else, How::Word)));
    }

    /// A song with no title of its own is found by what the file is called,
    /// which is what nearly everything in this library has instead of tags.
    #[test]
    fn a_song_with_no_title_is_found_by_the_name_of_the_file() {
        assert_eq!(said(&ranked(&library(), "aruarian")), ["Aruarian Dance"]);
    }

    #[test]
    fn a_word_the_library_says_nothing_about_finds_nothing() {
        assert!(ranked(&library(), "kangaroo").is_empty());
        assert!(ranked(&library(), "").is_empty());
    }

    /// Whose it is, or where it is: either settles which of three songs of the
    /// same name a row is about.
    #[test]
    fn a_row_says_whose_the_song_is_or_where_it_is() {
        let songs = songs(Path::new("/music"), &tree, &[]);
        assert_eq!(songs[1].aside(Path::new("/music")), "Nujabes");
        assert_eq!(songs[0].aside(Path::new("/music")), "");
        assert_eq!(library()[0].aside(Path::new("/music")), "Arctic Monkeys");
    }
}
