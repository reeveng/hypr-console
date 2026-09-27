//! What a typed word finds in the music library.
//!
//! The Music tab is a folder read one folder at a time, which is the right way
//! to walk a library someone knows and the wrong way to find one song in nine
//! hundred. So the line at the top of it is not a filter on what is in front of
//! you: it looks at everything under the music folder, and at what each of
//! those files says about itself as well as at what it is called.
//!
//! What it is called is free and what it says is not: reading one file takes an
//! ffprobe, and reading the library takes minutes. So the two are separate. The
//! walk happens here, on every letter, and it is fast; the reading happens once
//! in `music-index` and is written down beside the cache, and a song no one has
//! read yet is still found by its name.
//!
//! The order is the whole point of the thing. A word is looked for in the
//! song's name first, then in whose it is, then in everything else it says, and
//! within each of those the more of it the word was the higher it stands. Typing
//! "nujabes" puts the song called that above the songs by him, and both above
//! the one that merely mentions him in a description.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use console_core_iteration::Step;
use console_core_never::Never;

use serde_json::{Value, json};

use crate::library::Thing;
use crate::tags::Tags;

const ENOUGH: u32 = 4000;
const FAR: u32 = 400;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Song {
    pub path: PathBuf,
    pub name: String,
    pub tags: Tags,
    pub read: bool,
}

impl Song {
    pub fn of(path: &Path) -> Result<Self, Never> {
        let name = match path.file_name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => String::new(),
        };

        let named = console_core_file_names::title(&name)?;

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
            let within = match at.strip_prefix(folder) {
                Ok(within) => within,
                Err(_outside_the_folder) => return None,
            };

            Some(within)
        });

        Ok(match within {
            Some(within) => match within.as_os_str().is_empty() {
                true => String::new(),
                false => within.display().to_string(),
            },
            None => String::new(),
        })
    }
}

pub fn under(
    folder: &Path,
    read: &dyn Fn(&Path) -> Result<Vec<Thing>, Never>,
) -> Result<Vec<PathBuf>, Never> {
    let first = (Vec::new(), VecDeque::from([folder.to_path_buf()]), 0_u32);

    let walked = console_core_iteration::iterate(first, |(mut found, mut waiting, read_so_far): (Vec<PathBuf>, VecDeque<PathBuf>, u32)| {
        let at = match waiting.pop_front() {
            Some(at) => at,
            None => return Ok(Step::Halt(found)),
        };

        let Ok(many) = console_core_number_conversion::fitted::<_, u32>(found.len());

        match many >= ENOUGH || read_so_far >= FAR {
            true => return Ok(Step::Halt(found)),
            false => {},
        }

        let Ok(things) = read(&at);

        for thing in things {
            match thing.folder {
                true => waiting.push_back(thing.path),
                false => found.push(thing.path),
            }
        }

        Ok(Step::Again((found, waiting, read_so_far.saturating_add(1))))
    });

    Ok(match walked {
        Ok(found) => found,
        Err(_endless) => Vec::new(),
    })
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

pub fn unread(songs: &[Song]) -> Result<u32, Never> {
    console_core_number_conversion::fitted(songs.iter().filter(|song| !song.read).count())
}

pub fn at(cache: &Path) -> Result<PathBuf, Never> {
    Ok(cache.join("console").join("music").join("songs.json"))
}

pub fn serialize(songs: &[Song]) -> Result<String, Never> {
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

pub fn parse(said: &str) -> Result<Vec<Song>, Never> {
    let held: Value = match serde_json::from_str(said) {
        Ok(held) => held,

        Err(_not_json) => Value::Null,
    };

    let songs = match held.get("songs").and_then(Value::as_array) {
        Some(songs) => songs,
        None => return Ok(Vec::new()),
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
    let said = |key: &str| match held.get(key).and_then(Value::as_str) {
        Some(said) => said.to_string(),
        None => String::new(),
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
pub enum MatchedIn {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wanted<'a>(pub &'a str);

pub fn how(said: &str, wanted: Wanted<'_>) -> Result<Option<How>, Never> {
    let said = said.trim().to_lowercase();
    let wanted = wanted.0.trim().to_lowercase();

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

pub fn rank(song: &Song, word: &str) -> Result<Option<(MatchedIn, How)>, Never> {
    let says = song.says()?;
    let wanted = Wanted(word);
    let title = how(says, wanted)?;
    let name = how(&song.name, wanted)?;
    let itself = [title, name].into_iter().flatten().min();
    let artist = how(&song.tags.artist, wanted)?;
    let rest = how(&song.tags.rest, wanted)?;

    Ok([(MatchedIn::Song, itself), (MatchedIn::Artist, artist), (MatchedIn::Else, rest)]
        .into_iter()
        .filter_map(|(what, how)| how.map(|how| (what, how)))
        .min())
}

pub fn ranked<'a>(songs: &'a [Song], word: &str) -> Result<Vec<&'a Song>, Never> {
    let mut found: Vec<((MatchedIn, How), &Song)> = Vec::new();

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
    use crate::tags::Tagged;
    use super::*;

    fn tree(at: &Path) -> Result<Vec<Thing>, Never> {
        let of = |folders: &[&str], songs: &[&str]| {
            let thing = |name: &str, folder: bool| Thing {
                name: match folder {
                    true => name.to_string(),
                    false => {
                        let Ok(named) = console_core_file_names::title(name);

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

    fn a_song(name: &str, tags: Tags) -> Result<Song, Never> {
        let Ok(song) = Song::of(Path::new(name));

        Ok(Song { read: true, tags, ..song })
    }

    fn library() -> Result<Vec<Song>, Never> {
        [
            ("/music/505.opus", Tags { title: "505".to_string(), artist: "Arctic Monkeys".to_string(), rest: "Favourite Worst Nightmare".to_string() }),
            ("/music/Aruarian Dance.mp3", Tags { title: String::new(), artist: "Nujabes".to_string(), rest: "Samurai Champloo".to_string() }),
            ("/music/Nujabes Tribute.opus", Tags { title: "Nujabes Tribute".to_string(), artist: "Someone".to_string(), rest: String::new() }),
            ("/music/Luv Sic.opus", Tags { title: "Luv (sic) Part 3".to_string(), artist: "Shing02".to_string(), rest: "by Nujabes".to_string() }),
        ]
        .into_iter()
        .map(|(name, tags)| a_song(name, tags))
        .collect()
    }

    fn titles(found: &[&Song]) -> Result<Vec<String>, Never> {
        Ok(found
            .iter()
            .map(|song| {
                let Ok(says) = song.says();

                says.to_string()
            })
            .collect())
    }

    fn search<'a>(songs: &'a [Song], word: &str) -> Result<Vec<&'a Song>, Never> {
        ranked(songs, word)
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
    fn a_song_no_one_has_read_is_still_a_song() -> Result<(), &'static str> {
        let Ok(known) = a_song("/music/505 [qU9mHegkTc4].opus", Tags { title: "505".to_string(), artist: "Arctic Monkeys".to_string(), rest: String::new() });
        let known = vec![known];

        let Ok(songs) = songs(Path::new("/music"), &tree, &known);
        let (first, second) = match songs.as_slice() {
            [first, second] => (first, second),
            _other => return Err("the tree holds two songs"),
        };

        assert_eq!(first.says(), Ok("505"));
        assert!(first.read);
        assert_eq!(second.says(), Ok("aruarian dance"));
        assert!(!second.read);
        assert_eq!(unread(&songs), Ok(1));

        Ok(())
    }

    #[test]
    fn a_song_that_was_read_and_said_nothing_counts_as_read() -> Result<(), &'static str> {
        let Ok(quiet) = a_song("/music/quiet.opus", Tags { title: String::new(), artist: String::new(), rest: String::new() });
        let Ok(written) = serialize(&[quiet]);

        let Ok(songs) = parse(&written);
        let song = songs.first().ok_or("the song written down was not read back")?;

        assert_eq!(unread(&songs), Ok(0));
        assert_eq!(song.tags.tagged(), Ok(Tagged::None));

        Ok(())
    }

    #[test]
    fn what_was_written_down_is_what_is_read_back() {
        let Ok(library) = library();
        let Ok(written) = serialize(&library);

        let Ok(songs) = parse(&written);

        let Ok(nothing) = parse("");

        let Ok(rubbish) = parse("not json");

        assert_eq!(songs, library);
        assert!(nothing.is_empty());
        assert!(rubbish.is_empty());
    }

    #[test]
    fn the_whole_of_a_name_beats_the_start_of_one_and_the_start_beats_the_middle() {
        assert_eq!(how("505", Wanted("505")), Ok(Some(How::Whole)));
        assert_eq!(how("505 Live", Wanted("505")), Ok(Some(How::Start)));
        assert_eq!(how("Live at 505", Wanted("505")), Ok(Some(How::Word)));
        assert_eq!(how("Live at 1505", Wanted("505")), Ok(Some(How::Anywhere)));
        assert_eq!(how("Live", Wanted("505")), Ok(None));
        assert_eq!(how("505", Wanted("  ")), Ok(None));
    }

    #[test]
    fn the_case_it_was_typed_in_does_not_matter() {
        assert_eq!(how("Arctic Monkeys", Wanted("ARCTIC")), Ok(Some(How::Start)));
        let Ok(library) = library();
        let Ok(found) = search(&library, "ARCTIC");

        assert_eq!(titles(&found), Ok(vec!["505".to_string()]));
    }

    #[test]
    fn the_song_ranks_above_the_artist_and_the_artist_above_the_rest() -> Result<(), &'static str> {
        let Ok(library) = library();
        let Ok(found) = search(&library, "nujabes");
        let (song, artist, rest) = match found.as_slice() {
            [song, artist, rest] => (*song, *artist, *rest),
            _other => return Err("three songs are found"),
        };

        assert_eq!(
            titles(&found),
            Ok(vec!["Nujabes Tribute".to_string(), "Aruarian Dance".to_string(), "Luv (sic) Part 3".to_string()])
        );
        assert_eq!(rank(song, "nujabes"), Ok(Some((MatchedIn::Song, How::Start))));
        assert_eq!(rank(artist, "nujabes"), Ok(Some((MatchedIn::Artist, How::Whole))));
        assert_eq!(rank(rest, "nujabes"), Ok(Some((MatchedIn::Else, How::Word))));

        Ok(())
    }

    #[test]
    fn a_song_with_no_title_is_found_by_the_name_of_the_file() {
        let Ok(library) = library();
        let Ok(found) = search(&library, "aruarian");

        assert_eq!(titles(&found), Ok(vec!["Aruarian Dance".to_string()]));
    }

    #[test]
    fn a_word_the_library_says_nothing_about_finds_nothing() {
        let Ok(library) = library();
        let Ok(kangaroo) = search(&library, "kangaroo");
        let Ok(nothing) = search(&library, "");

        assert!(kangaroo.is_empty());
        assert!(nothing.is_empty());
    }

    #[test]
    fn a_row_says_whose_the_song_is_or_where_it_is() -> Result<(), &'static str> {
        let Ok(songs) = songs(Path::new("/music"), &tree, &[]);
        let (loose, filed) = match songs.as_slice() {
            [loose, filed] => (loose, filed),
            _other => return Err("the tree holds two songs"),
        };
        let Ok(library) = library();
        let known = library.first().ok_or("the library is empty")?;

        assert_eq!(filed.aside(Path::new("/music")), Ok("Nujabes".to_string()));
        assert_eq!(loose.aside(Path::new("/music")), Ok(String::new()));
        assert_eq!(known.aside(Path::new("/music")), Ok("Arctic Monkeys".to_string()));

        Ok(())
    }
}
