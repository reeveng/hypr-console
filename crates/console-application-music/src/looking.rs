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
use std::fmt;
use std::path::{Path, PathBuf};

use console_core_atomic_writes::{MAX_TEXT_BYTES, Stored};
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::fixed_index;

use serde::de::{IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};

use crate::library::{Thing, visible_name};
use console_music_player::tags::{Album, Tags};

const MAX_RECORDS: u32 = 65_536;
const MAX_FIELD_BYTES: u32 = 64 * 1024;
pub(crate) const CATALOG_FILE: &str = "songs.json";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Song {
  pub path: PathBuf,
  pub name: String,
  pub tags: Tags,
  tag_state: TagState,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TagState {
  #[default]
  Unobserved,
  Unread(FileRevision),
  Cached(FileRevision),
  LegacyCache,
}

#[derive(Clone, Copy, Debug, Deserialize, serde::Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FileRevision {
  device: u64,
  inode: u64,
  bytes: u64,
  changed: i64,
  nanos: i64,
}

impl FileRevision {
  pub fn of(path: &Path) -> Result<Option<Self>, Never> {
    use std::os::unix::fs::MetadataExt;

    Ok(match path.metadata() {
      Ok(info) => Some(Self {
        device: info.dev(),
        inode: info.ino(),
        bytes: info.len(),
        changed: info.ctime(),
        nanos: info.ctime_nsec(),
      }),
      Err(_unavailable) => None,
    })
  }

  pub fn file(self) -> Result<console_music_player::loudness::FileId, Never> {
    Ok(console_music_player::loudness::FileId {
      device: self.device,
      inode: self.inode,
    })
  }
}

impl Song {
  pub fn tag_state(&self) -> Result<TagState, Never> {
    Ok(self.tag_state)
  }

  pub fn revision(&self) -> Result<Option<FileRevision>, Never> {
    Ok(match self.tag_state {
      TagState::Unobserved | TagState::LegacyCache => None,
      TagState::Unread(revision) | TagState::Cached(revision) => Some(revision),
    })
  }

  pub fn record_tags(&mut self, tags: Tags, observed: Option<FileRevision>) -> Result<(), Never> {
    let Ok(expected) = self.revision();
    let verified = match observed.filter(|revision| Some(*revision) == expected) {
      Some(revision) => TagState::Cached(revision),
      None => match expected {
        Some(revision) => TagState::Unread(revision),
        None => TagState::Unobserved,
      },
    };
    self.tags = tags;
    self.tag_state = verified;

    Ok(())
  }

  pub fn of(path: &Path) -> Result<Self, Never> {
    let name = match path.file_name() {
      Some(name) => visible_name(name)?,
      None => String::new(),
    };

    let named = console_core_file_names::title(&name)?;

    Ok(Song {
      name: named,
      path: path.to_path_buf(),
      ..Song::default()
    })
  }

  pub fn says(&self) -> Result<&str, Never> {
    Ok(
      match ::core::primitive::bool::then_some(self.tags.title.is_empty(), ()) {
        ::core::option::Option::Some(()) => &self.name,
        ::core::option::Option::None => &self.tags.title,
      },
    )
  }

  pub fn aside(&self, folder: &Path) -> Result<String, Never> {
    match ::core::primitive::bool::then_some(self.tags.artist.is_empty(), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Ok(self.tags.artist.clone()),
    }

    let within = self.path.parent().and_then(|at| {
      let within = match at.strip_prefix(folder) {
        Ok(within) => within,
        Err(_outside_the_folder) => return None,
      };

      Some(within)
    });

    match within {
      Some(relative) => visible_name(relative.as_os_str()),
      None => Ok(String::new()),
    }
  }
}

pub fn under(
  folder: &Path,
  read: &dyn Fn(&Path) -> Result<Vec<Thing>, Never>,
) -> Result<Vec<PathBuf>, Never> {
  let first = (Vec::new(), VecDeque::from([folder.to_path_buf()]));

  let walked = console_core_iteration::iterate(
    first,
    |(mut found, mut waiting): (Vec<PathBuf>, VecDeque<PathBuf>)| {
      let at = match waiting.pop_front() {
        Some(at) => at,
        None => return Ok(Step::Halt(found)),
      };

      let Ok(things) = read(&at);

      for thing in things {
        match ::core::primitive::bool::then_some(thing.folder, ()) {
          ::core::option::Option::Some(()) => waiting.push_back(thing.path),
          ::core::option::Option::None => found.push(thing.path),
        }
      }

      Ok(Step::Again((found, waiting)))
    },
  );

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
  let known_2: HashMap<&Path, &Song> = known
    .iter()
    .map(|song| (song.path.as_path(), song))
    .collect();

  let under = under(folder, read)?;

  let mut songs: Vec<Song> = Vec::new();

  for path in under {
    let revision = FileRevision::of(&path)?;
    let song = match known_2
      .get(path.as_path())
      .filter(|song| song.revision() == Ok(revision))
    {
      Some(song) => (*song).clone(),
      None => {
        let song = Song::of(&path)?;

        let tag_state = match revision {
          Some(revision_2) => TagState::Unread(revision_2),
          None => TagState::Unobserved,
        };

        Song { tag_state, ..song }
      }
    };

    songs.push(song);
  }

  Ok(songs)
}

pub fn unread(songs: &[Song]) -> Result<u32, Never> {
  let ::core::result::Result::Ok(unread) = ::console_core_number_conversion::fixed_index(
    songs
      .iter()
      .filter(|song| matches!(song.tag_state, TagState::Unobserved | TagState::Unread(_)))
      .count(),
  );

  console_core_number_conversion::fitted(match ::console_core_number_conversion::index(unread) {
    ::core::result::Result::Ok(native_position) => native_position,
  })
}

pub fn at(cache: &Path) -> Result<PathBuf, Never> {
  Ok(cache.join("console").join("music").join(CATALOG_FILE))
}

pub fn lock_catalog(cache: &Path) -> std::io::Result<std::fs::File> {
  std::fs::create_dir_all(cache)?;
  let gate = console_core_atomic_writes::open_lock_file(&cache.join("catalog.lock"))?;
  gate.lock()?;

  Ok(gate)
}

pub fn load(at: &Path) -> Result<Vec<Song>, Never> {
  let held = console_core_atomic_writes::read(at)?;

  match held {
    Stored::Text(text) => parse(&text),
    Stored::Absent => Ok(Vec::new()),
    Stored::Failed(fault) => {
      console_core_logging::warn!(
        "music-index: {}: reading the song index: {fault}",
        at.display()
      );

      Ok(Vec::new())
    }
  }
}

pub fn serialize(songs: &[Song]) -> Result<String, Never> {
  serialize_records(songs)
}

pub fn serialize_records<'a>(songs: impl IntoIterator<Item = &'a Song>) -> Result<String, Never> {
  let held: Vec<Value> = songs
    .into_iter()
    .filter(|song| matches!(song.tag_state, TagState::Cached(_) | TagState::LegacyCache))
    .map(|song| {
      let Ok(revision) = song.revision();
      let (path_key, path) = match song.path.to_str() {
        Some(text) => ("path", text.to_owned()),
        None => {
          let Ok(url) = console_core_file_urls::url(&song.path);

          ("path_url", url)
        }
      };

      json!({
          (path_key): path,
          "title": song.tags.title,
          "artist": song.tags.artist,
          "rest": song.tags.rest,
          "album": song.tags.album.title,
          "album_artist": song.tags.album.artist,
          "release_id": song.tags.album.release_id,
          "disc": song.tags.album.disc,
          "track": song.tags.album.track,
          "revision": revision,
      })
    })
    .collect();

  Ok(match serde_json::to_string(&json!({ "songs": held })) {
    Ok(written) => written,

    Err(fault) => {
      console_core_logging::warn!(
        "music-index: writing down what was read about the songs: {fault}"
      );
      String::new()
    }
  })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SongIndex {
  songs: IndexSongs,
}

struct IndexSongs(Vec<IndexedSong>);

impl<'de> Deserialize<'de> for IndexSongs {
  fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
    decoder.deserialize_seq(SongsVisitor)
  }
}

struct SongsVisitor;

impl<'de> Visitor<'de> for SongsVisitor {
  type Value = IndexSongs;

  fn expecting(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(to, "at most {MAX_RECORDS} song records")
  }

  fn visit_seq<A: SeqAccess<'de>>(self, mut list: A) -> Result<Self::Value, A::Error> {
    let mut songs = Vec::new();

    for _ in 0..MAX_RECORDS {
      let next = list.next_element()?;

      match next {
        Some(song) => songs.push(song),
        None => return Ok(IndexSongs(songs)),
      }
    }

    let extra = list.next_element::<IgnoredAny>()?;

    match extra {
      Some(_) => Err(serde::de::Error::custom("song index has too many records")),
      None => Ok(IndexSongs(songs)),
    }
  }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum IndexedSong {
  Song(SongRecord),
  Other(IgnoredAny),
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SongRecord {
  path: IndexText,
  title: IndexText,
  artist: IndexText,
  rest: IndexText,
  path_url: IndexText,
  album: IndexText,
  album_artist: IndexText,
  release_id: IndexText,
  disc: Option<u32>,
  track: Option<u32>,
  revision: Option<FileRevision>,
}

#[derive(Default)]
struct IndexText(String);

impl<'de> Deserialize<'de> for IndexText {
  fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
    let value = Value::deserialize(decoder)?;

    let text = match value {
      Value::String(text) => text,
      Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => {
        return Ok(Self::default());
      }
    };
    let Ok(bytes) = fixed_index(text.len());

    match bool::then_some(bytes <= u64::from(MAX_FIELD_BYTES), ()) {
      Some(()) => Ok(Self(text)),
      None => Ok(Self::default()),
    }
  }
}

pub fn parse(said: &str) -> Result<Vec<Song>, Never> {
  let Ok(bytes) = fixed_index(said.len());

  match bool::then_some(bytes <= u64::from(MAX_TEXT_BYTES), ()) {
    Some(()) => {}
    None => return Ok(Vec::new()),
  }

  let index = match serde_json::from_str::<SongIndex>(said) {
    Ok(index) => index,
    Err(_unreadable) => return Ok(Vec::new()),
  };

  let mut kept = Vec::new();

  for record in index.songs.0 {
    let recorded = match record {
      IndexedSong::Song(recorded) => recorded,
      IndexedSong::Other(_ignored) => continue,
    };
    let location = match recorded.path.0.as_str() {
      "" => console_core_file_urls::path(&recorded.path_url.0)?,
      text => Some(PathBuf::from(text)),
    };

    let at = match location {
      Some(path) => path,
      None => continue,
    };

    let song = Song::of(&at)?;
    let tag_state = match recorded.revision {
      Some(revision) => TagState::Cached(revision),
      None => TagState::LegacyCache,
    };
    kept.push(Song {
      tag_state,
      tags: Tags {
        title: recorded.title.0,
        artist: recorded.artist.0,
        rest: recorded.rest.0,
        album: Album {
          title: recorded.album.0,
          artist: recorded.album_artist.0,
          release_id: recorded.release_id.0,
          disc: recorded.disc,
          track: recorded.track,
        },
      },
      ..song
    });
  }

  Ok(kept)
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
  let said_2 = said.trim().to_lowercase();
  let wanted_2 = wanted.0.trim().to_lowercase();

  match ::core::primitive::bool::then_some(said_2.is_empty() || wanted_2.is_empty(), ()) {
    ::core::option::Option::Some(()) => return Ok(None),
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(said_2 == wanted_2, ()) {
    ::core::option::Option::Some(()) => return Ok(Some(How::Whole)),
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(said_2.starts_with(&wanted_2), ()) {
    ::core::option::Option::Some(()) => return Ok(Some(How::Start)),
    ::core::option::Option::None => {}
  }

  let at_a_word = said_2.match_indices(&wanted_2).any(|(at, _)| {
    said_2
      .get(..at)
      .and_then(|before| before.chars().next_back())
      .is_some_and(|before| !before.is_alphanumeric())
  });

  Ok(match ::core::primitive::bool::then_some(at_a_word, ()) {
    ::core::option::Option::Some(()) => Some(How::Word),
    ::core::option::Option::None => said_2.contains(&wanted_2).then_some(How::Anywhere),
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

  Ok(
    [
      (MatchedIn::Song, itself),
      (MatchedIn::Artist, artist),
      (MatchedIn::Else, rest),
    ]
    .into_iter()
    .filter_map(|(what, how)| how.map(|how_2| (what, how_2)))
    .min(),
  )
}

pub fn ranked<'a>(songs: &'a [Song], word: &str) -> Result<Vec<&'a Song>, Never> {
  let mut found: Vec<((MatchedIn, How), &Song)> = Vec::new();

  for song in songs {
    let rank = rank(song, word)?;

    match rank {
      Some(rank_2) => found.push((rank_2, song)),
      None => {}
    }
  }

  found.sort_by_key(|(rank, _)| *rank);

  Ok(found.into_iter().map(|(_, song)| song).collect())
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_music_player::tags::Tagged;

  fn tree(at: &Path) -> Result<Vec<Thing>, Never> {
    let of = |folders: &[&str], songs: &[&str]| {
      let thing = |path: &str, folder: bool| {
        let located = PathBuf::from(path);
        let name = match located.file_name().and_then(std::ffi::OsStr::to_str) {
          Some(text) => text,
          None => "",
        };

        Thing {
          name: match bool::then_some(folder, ()) {
            Some(()) => name.to_string(),
            None => {
              let Ok(named) = console_core_file_names::title(name);

              named
            }
          },
          path: located,
          folder,
        }
      };
      let mut things: Vec<Thing> = folders.iter().map(|name| thing(name, true)).collect();
      things.extend(songs.iter().map(|name| thing(name, false)));
      things
    };
    Ok(match at.to_str() {
      Some("/music") => of(&["/music/Nujabes"], &["/music/505 [qU9mHegkTc4].opus"]),
      Some("/music/Nujabes") => of(&[], &["/music/Nujabes/aruarian dance.mp3"]),
      Some(_) | None => Vec::new(),
    })
  }

  fn a_song(name: &str, tags: Tags) -> Result<Song, Never> {
    let Ok(song) = Song::of(Path::new(name));

    Ok(Song {
      tag_state: TagState::LegacyCache,
      tags,
      ..song
    })
  }

  fn library() -> Result<Vec<Song>, Never> {
    [
      (
        "/music/505.opus",
        Tags {
          title: "505".to_string(),
          artist: "Arctic Monkeys".to_string(),
          rest: "Favourite Worst Nightmare".to_string(),
          ..Tags::default()
        },
      ),
      (
        "/music/Aruarian Dance.mp3",
        Tags {
          title: String::new(),
          artist: "Nujabes".to_string(),
          rest: "Samurai Champloo".to_string(),
          ..Tags::default()
        },
      ),
      (
        "/music/Nujabes Tribute.opus",
        Tags {
          title: "Nujabes Tribute".to_string(),
          artist: "Someone".to_string(),
          rest: String::new(),
          ..Tags::default()
        },
      ),
      (
        "/music/Luv Sic.opus",
        Tags {
          title: "Luv (sic) Part 3".to_string(),
          artist: "Shing02".to_string(),
          rest: "by Nujabes".to_string(),
          ..Tags::default()
        },
      ),
    ]
    .into_iter()
    .map(|(name, tags)| a_song(name, tags))
    .collect()
  }

  fn titles(found: &[&Song]) -> Result<Vec<String>, Never> {
    Ok(
      found
        .iter()
        .map(|song| {
          let Ok(says) = song.says();

          says.to_string()
        })
        .collect(),
    )
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
    let Ok(known) = a_song(
      "/music/505 [qU9mHegkTc4].opus",
      Tags {
        title: "505".to_string(),
        artist: "Arctic Monkeys".to_string(),
        rest: String::new(),
        ..Tags::default()
      },
    );
    let known_2 = vec![known];

    let Ok(songs) = songs(Path::new("/music"), &tree, &known_2);
    let (first, second) = match songs.as_slice() {
      [first, second] => (first, second),
      _other => return Err("the tree holds two songs"),
    };

    assert_eq!(first.says(), Ok("505"));
    assert_eq!(first.tag_state(), Ok(TagState::LegacyCache));
    assert_eq!(second.says(), Ok("aruarian dance"));
    assert_eq!(second.tag_state(), Ok(TagState::Unobserved));
    assert_eq!(unread(&songs), Ok(1));

    Ok(())
  }

  #[test]
  fn a_song_changed_during_the_read_keeps_its_original_revision_and_stays_unread() {
    let before = FileRevision {
      device: 1,
      inode: 2,
      bytes: 3,
      changed: 4,
      nanos: 5,
    };
    let changed = FileRevision { bytes: 4, ..before };
    let mut song = Song {
      tag_state: TagState::Unread(before),
      ..Song::default()
    };

    let Ok(()) = song.record_tags(Tags::default(), Some(changed));
    assert_eq!(song.tag_state(), Ok(TagState::Unread(before)));

    let Ok(()) = song.record_tags(Tags::default(), Some(before));
    assert_eq!(song.tag_state(), Ok(TagState::Cached(before)));
  }

  #[test]
  fn a_song_that_was_read_and_said_nothing_counts_as_read() -> Result<(), &'static str> {
    let Ok(quiet) = a_song(
      "/music/quiet.opus",
      Tags {
        title: String::new(),
        artist: String::new(),
        rest: String::new(),
        ..Tags::default()
      },
    );
    let Ok(written) = serialize(&[quiet]);

    let Ok(songs) = parse(&written);
    let song = songs
      .first()
      .ok_or("the song written down was not read back")?;

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
  fn the_song_index_preserves_file_name_bytes_and_recovers_other_records() {
    use std::os::unix::ffi::OsStrExt;

    let path = Path::new(std::ffi::OsStr::from_bytes(b"/music/a\xff.opus"));
    let Ok(song) = Song::of(path);
    let record = Song {
      tag_state: TagState::LegacyCache,
      ..song
    };
    let Ok(written) = serialize(std::slice::from_ref(&record));

    assert_eq!(parse(&written), Ok(vec![record]));

    let Ok(from_old_index) = parse(
      r#"{"songs":[null,{"path":"/music/b.opus","title":false},{"path":"/music/c.opus","unknown":true}]}"#,
    );
    let paths: Vec<_> = from_old_index
      .iter()
      .map(|song_2| song_2.path.as_path())
      .collect();

    assert_eq!(paths, [Path::new("/music/b.opus")]);
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
    assert_eq!(
      how("Arctic Monkeys", Wanted("ARCTIC")),
      Ok(Some(How::Start))
    );
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
      Ok(vec![
        "Nujabes Tribute".to_string(),
        "Aruarian Dance".to_string(),
        "Luv (sic) Part 3".to_string()
      ])
    );
    assert_eq!(
      rank(song, "nujabes"),
      Ok(Some((MatchedIn::Song, How::Start)))
    );
    assert_eq!(
      rank(artist, "nujabes"),
      Ok(Some((MatchedIn::Artist, How::Whole)))
    );
    assert_eq!(
      rank(rest, "nujabes"),
      Ok(Some((MatchedIn::Else, How::Word)))
    );

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
    assert_eq!(
      known.aside(Path::new("/music")),
      Ok("Arctic Monkeys".to_string())
    );

    Ok(())
  }
}
