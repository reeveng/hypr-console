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

use console_never::Never;

use serde_json::{Value, json};

use crate::library::{self, Thing};
use crate::tags::Tags;

const ENOUGH: usize = 4000;
const FAR: usize = 400;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Song {
    pub path: PathBuf,
    pub name: String,
    pub tags: Tags,
    pub read: bool,
}

impl Song {
    pub fn of(path: &Path) -> Result<Self, Never> {
        let name = path.file_name().map(|name| name.to_string_lossy().to_string());
        let named = library::named(&name.unwrap_or_default())?;

        Ok(Song { name: named, path: path.to_path_buf(), ..Song::default() })
    }

    pub fn says(&self) -> Result<&str, Never> {
        Ok(match self.tags.title.is_empty() {
            true => &self.name,
            false => &self.tags.title,
        })
    }

    pub fn aside(&self, folder: &Path) -> Result<String, Never> {
        match self.tags.artist.is_empty() {
            true => {},
            false => return Ok(self.tags.artist.clone()),
        }

        let within = self.path.parent().and_then(|at| {
            let Ok(within) = at.strip_prefix(folder) else { return None };

            Some(within)
        });

        Ok(match within {
            Some(within) if !within.as_os_str().is_empty() => within.display().to_string(),
            Some(_) | None => String::new(),
        })
    }
}

pub fn under(
    folder: &Path,
    read: &dyn Fn(&Path) -> Result<Vec<Thing>, Never>,
) -> Result<Vec<PathBuf>, Never> {
    let mut found: Vec<PathBuf> = Vec::new();
    let mut waiting = VecDeque::from([folder.to_path_buf()]);
    let mut read_so_far: usize = 0;

    while let Some(at) = waiting.pop_front() {
        match found.len() >= ENOUGH || read_so_far >= FAR {
            true => break,
            false => {},
        }

        read_so_far = read_so_far.saturating_add(1);

        let things = read(&at)?;

        for thing in things {
            match thing.folder {
                true => waiting.push_back(thing.path),
                false => found.push(thing.path),
            }
        }
    }

    Ok(found)
}

pub fn songs(
    folder: &Path,
    read: &dyn Fn(&Path) -> Result<Vec<Thing>, Never>,
    known: &[Song],
) -> Result<Vec<Song>, Never> {
    let known: HashMap<&Path, &Song> =
        known.iter().map(|song| (song.path.as_path(), song)).collect();

    let under = under(folder, read)?;

    let mut songs: Vec<Song> = Vec::new();

    for path in under {
        let song = match known.get(path.as_path()) {
            Some(song) => (*song).clone(),
            None => Song::of(&path)?,
        };

        songs.push(song);
    }

    Ok(songs)
}

pub fn unread(songs: &[Song]) -> Result<usize, Never> {
    Ok(songs.iter().filter(|song| !song.read).count())
}

pub fn at(cache: &Path) -> Result<PathBuf, Never> {
    Ok(cache.join("console").join("music").join("songs.json"))
}

pub fn written(songs: &[Song]) -> Result<String, Never> {
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

    Ok(match serde_json::to_string(&json!({ "songs": held })) {
        Ok(written) => written,

        Err(fault) => {
            eprintln!("music-index: writing down what was read about the songs: {fault}");
            String::new()
        }
    })
}

pub fn kept(said: &str) -> Result<Vec<Song>, Never> {
    let held: Value = match serde_json::from_str(said) {
        Ok(held) => held,

        Err(_) => Value::Null,
    };

    let Some(songs) = held.get("songs").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };

    let mut kept: Vec<Song> = Vec::new();

    for held in songs {
        let one = one(held)?;

        match one {
            Some(song) => kept.push(song),
            None => {},
        }
    }

    Ok(kept)
}

fn one(held: &Value) -> Result<Option<Song>, Never> {
    let said = |key: &str| {
        held.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
    };
    let path = said("path");

    match path.is_empty() {
        true => return Ok(None),
        false => {},
    }

    let song = Song::of(Path::new(&path))?;

    Ok(Some(Song {
        read: true,
        tags: Tags { title: said("title"), artist: said("artist"), rest: said("rest") },
        ..song
    }))
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum In {
    Song,
    Artist,
    Else,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum How {
    Whole,
    Start,
    Word,
    Anywhere,
}

pub fn how(said: &str, wanted: &str) -> Result<Option<How>, Never> {
    let said = said.trim().to_lowercase();
    let wanted = wanted.trim().to_lowercase();

    match said.is_empty() || wanted.is_empty() {
        true => return Ok(None),
        false => {},
    }

    match said == wanted {
        true => return Ok(Some(How::Whole)),
        false => {},
    }

    match said.starts_with(&wanted) {
        true => return Ok(Some(How::Start)),
        false => {},
    }

    let at_a_word = said.match_indices(&wanted).any(|(at, _)| {
        said.get(..at)
            .and_then(|before| before.chars().next_back())
            .is_some_and(|before| !before.is_alphanumeric())
    });

    Ok(match at_a_word {
        true => Some(How::Word),
        false => said.contains(&wanted).then_some(How::Anywhere),
    })
}

pub fn rank(song: &Song, word: &str) -> Result<Option<(In, How)>, Never> {
    let says = song.says()?;
    let title = how(says, word)?;
    let name = how(&song.name, word)?;
    let itself = [title, name].into_iter().flatten().min();
    let artist = how(&song.tags.artist, word)?;
    let rest = how(&song.tags.rest, word)?;

    Ok([(In::Song, itself), (In::Artist, artist), (In::Else, rest)]
        .into_iter()
        .filter_map(|(what, how)| how.map(|how| (what, how)))
        .min())
}

pub fn ranked<'a>(songs: &'a [Song], word: &str) -> Result<Vec<&'a Song>, Never> {
    let mut found: Vec<((In, How), &Song)> = Vec::new();

    for song in songs {
        let rank = rank(song, word)?;

        match rank {
            Some(rank) => found.push((rank, song)),
            None => {},
        }
    }

    found.sort_by_key(|(rank, _)| *rank);

    Ok(found.into_iter().map(|(_, song)| song).collect())
}

#[cfg(test)]
mod tests {
    use crate::tags::Said;
    use super::*;

    fn tree(at: &Path) -> Result<Vec<Thing>, Never> {
        let of = |folders: &[&str], songs: &[&str]| {
            let thing = |name: &str, folder: bool| Thing {
                name: match folder {
                    true => name.to_string(),
                    false => {
                        let Ok(named) = library::named(name);

                        named
                    }
                },
                path: at.join(name),
                folder,
            };
            let mut things: Vec<Thing> =
                folders.iter().map(|name| thing(name, true)).collect();
            things.extend(songs.iter().map(|name| thing(name, false)));
            things
        };
        Ok(match at.to_string_lossy().as_ref() {
            "/music" => of(&["Nujabes"], &["505 [qU9mHegkTc4].opus"]),
            "/music/Nujabes" => of(&[], &["aruarian dance.mp3"]),
            _ => Vec::new(),
        })
    }

    fn a_song(name: &str, title: &str, artist: &str, rest: &str) -> Song {
        let Ok(song) = Song::of(Path::new(name));

        Song {
            read: true,
            tags: Tags {
                title: title.to_string(),
                artist: artist.to_string(),
                rest: rest.to_string(),
            },
            ..song
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
        found
            .iter()
            .map(|song| {
                let Ok(says) = song.says();

                says.to_string()
            })
            .collect()
    }

    fn found<'a>(songs: &'a [Song], word: &str) -> Vec<&'a Song> {
        let Ok(found) = ranked(songs, word);

        found
    }

    #[test]
    fn a_word_reaches_the_songs_in_the_folders_under_this_one() {
        let Ok(found) = under(Path::new("/music"), &tree);

        assert_eq!(
            found,
            [
                PathBuf::from("/music/505 [qU9mHegkTc4].opus"),
                PathBuf::from("/music/Nujabes/aruarian dance.mp3"),
            ],
            "the folders under it are walked into, and the nearest comes first"
        );
    }

    #[test]
    fn a_song_nobody_has_read_is_still_a_song() {
        let known = vec![a_song("/music/505 [qU9mHegkTc4].opus", "505", "Arctic Monkeys", "")];

        let Ok(songs) = songs(Path::new("/music"), &tree, &known);

        assert_eq!(songs.len(), 2);
        assert_eq!(songs[0].says(), Ok("505"));
        assert!(songs[0].read);
        assert_eq!(songs[1].says(), Ok("aruarian dance"));
        assert!(!songs[1].read);
        assert_eq!(unread(&songs), Ok(1));
    }

    #[test]
    fn a_song_that_was_read_and_said_nothing_counts_as_read() {
        let Ok(written) = written(&[a_song("/music/quiet.opus", "", "", "")]);

        let Ok(songs) = kept(&written);

        assert_eq!(unread(&songs), Ok(0));
        assert_eq!(songs[0].tags.anything(), Ok(Said::Nothing));
    }

    #[test]
    fn what_was_written_down_is_what_is_read_back() {
        let Ok(written) = written(&library());

        let Ok(songs) = kept(&written);

        let Ok(nothing) = kept("");

        let Ok(rubbish) = kept("not json");

        assert_eq!(songs, library());
        assert!(nothing.is_empty());
        assert!(rubbish.is_empty());
    }

    #[test]
    fn the_whole_of_a_name_beats_the_start_of_one_and_the_start_beats_the_middle() {
        assert_eq!(how("505", "505"), Ok(Some(How::Whole)));
        assert_eq!(how("505 Live", "505"), Ok(Some(How::Start)));
        assert_eq!(how("Live at 505", "505"), Ok(Some(How::Word)));
        assert_eq!(how("Live at 1505", "505"), Ok(Some(How::Anywhere)));
        assert_eq!(how("Live", "505"), Ok(None));
        assert_eq!(how("505", "  "), Ok(None));
    }

    #[test]
    fn the_case_it_was_typed_in_does_not_matter() {
        assert_eq!(how("Arctic Monkeys", "ARCTIC"), Ok(Some(How::Start)));
        assert_eq!(said(&found(&library(), "ARCTIC")), ["505"]);
    }

    #[test]
    fn the_song_ranks_above_the_artist_and_the_artist_above_the_rest() {
        let library = library();
        let found = found(&library, "nujabes");

        assert_eq!(said(&found), ["Nujabes Tribute", "Aruarian Dance", "Luv (sic) Part 3"]);
        assert_eq!(rank(found[0], "nujabes"), Ok(Some((In::Song, How::Start))));
        assert_eq!(rank(found[1], "nujabes"), Ok(Some((In::Artist, How::Whole))));
        assert_eq!(rank(found[2], "nujabes"), Ok(Some((In::Else, How::Word))));
    }

    #[test]
    fn a_song_with_no_title_is_found_by_the_name_of_the_file() {
        assert_eq!(said(&found(&library(), "aruarian")), ["Aruarian Dance"]);
    }

    #[test]
    fn a_word_the_library_says_nothing_about_finds_nothing() {
        assert!(found(&library(), "kangaroo").is_empty());
        assert!(found(&library(), "").is_empty());
    }

    #[test]
    fn a_row_says_whose_the_song_is_or_where_it_is() {
        let Ok(songs) = songs(Path::new("/music"), &tree, &[]);

        assert_eq!(songs[1].aside(Path::new("/music")), Ok("Nujabes".to_string()));
        assert_eq!(songs[0].aside(Path::new("/music")), Ok(String::new()));
        assert_eq!(library()[0].aside(Path::new("/music")), Ok("Arctic Monkeys".to_string()));
    }
}
