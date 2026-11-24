//! Suggested library metadata is reviewed before it is stored as a local overlay.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use console_core_never::Never;
use console_core_places::Base;
use console_music_player::tags::Album;
use serde::{Deserialize, Serialize};

use crate::looking::{FileRevision, Song};
use crate::review::{Audio, FoundTrack};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tags {
  pub artist: String,
  pub title: String,
  pub album: String,
  album_artist: String,
  release: String,
  disc: Option<u32>,
  track: Option<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Source {
  Audio,
  Filename,
  MusicBrainz,
}

impl Source {
  pub fn label(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Audio => "Same decoded audio",
      Self::Filename => "Filename matches a local track",
      Self::MusicBrainz => "MusicBrainz candidate",
    })
  }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Proposal {
  pub file: FoundTrack,
  pub tags: Tags,
  pub source: Source,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Search {
  pub file: FoundTrack,
  title: String,
  artist: String,
}

pub fn missing(song: &Song) -> Result<Option<()>, Never> {
  Ok((song.tags.title.trim().is_empty() || song.tags.artist.trim().is_empty()).then_some(()))
}

fn words(text: &str) -> String {
  text
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
    .to_lowercase()
}

pub fn query(song: &Song) -> Result<Search, Never> {
  let Ok(path) = crate::review::key(&song.path);
  let separated = song
    .name
    .split_once(" - ")
    .or_else(|| song.name.split_once(" — "));
  let (artist, title) = match separated {
    Some((artist, title)) => (artist, title),
    None => ("", song.name.as_str()),
  };
  let artist = match song.tags.artist.trim() {
    "" => artist.trim(),
    artist => artist,
  };
  let title = match song.tags.title.trim() {
    "" => title.trim(),
    title => title,
  };

  Ok(Search {
    file: FoundTrack {
      path,
      revision: song.revision,
    },
    title: title.to_owned(),
    artist: artist.to_owned(),
  })
}

fn tagged(song: &Song) -> Result<Tags, Never> {
  Ok(Tags {
    artist: song.tags.artist.clone(),
    title: song.tags.title.clone(),
    album: song.tags.album.title.clone(),
    album_artist: song.tags.album.artist.clone(),
    release: song.tags.album.release_id.clone(),
    disc: song.tags.album.disc,
    track: song.tags.album.track,
  })
}

pub fn local(cache: &Path, songs: &[Song], audio: &[Option<Audio>]) -> Result<(), Never> {
  let mut by_audio = BTreeMap::<u64, BTreeSet<Tags>>::new();
  let mut by_name = BTreeMap::<(String, String), BTreeSet<Tags>>::new();

  for (song, audio) in songs.iter().zip(audio) {
    match missing(song) {
      Ok(Some(())) => continue,
      Ok(None) => {}
    }
    let Ok(tags) = tagged(song);
    by_name
      .entry((words(&tags.artist), words(&tags.title)))
      .or_default()
      .insert(tags.clone());

    match audio {
      Some(audio) => {
        let Ok(key) = audio.checksum();
        let _known = by_audio.entry(key).or_default().insert(tags);
      }
      None => {}
    }
  }

  let mut suggestions = Vec::new();
  for (song, audio) in songs.iter().zip(audio) {
    match missing(song) {
      Ok(Some(())) => {}
      Ok(None) => continue,
    }
    let Ok(query) = query(song);
    let wave = audio.as_ref().and_then(|audio| {
      let Ok(key) = audio.checksum();

      by_audio.get(&key)
    });
    let (candidates, source) = match wave {
      Some(candidates) => (Some(candidates), Source::Audio),
      None => (
        by_name.get(&(words(&query.artist), words(&query.title))),
        Source::Filename,
      ),
    };

    for tags in candidates.into_iter().flatten().take(4) {
      suggestions.push(Proposal {
        file: query.file.clone(),
        tags: tags.clone(),
        source,
      });
    }
  }

  let Ok(()) = crate::review::record(&cache.join("metadata-suggestions-v1.json"), &suggestions);

  Ok(())
}

pub fn local_results() -> Result<Vec<Proposal>, Never> {
  let Ok(cache) = Base::Cache.ours();

  Ok(
    cache
      .and_then(|root| {
        crate::review::read_json(&root.join("music").join("metadata-suggestions-v1.json"))
      })
      .unwrap_or_default(),
  )
}

enum Store {
  Online,
  Accepted,
}

struct RecordName(String);

impl AsRef<Path> for RecordName {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

fn stored_path(path: &str, store: Store) -> Result<Option<PathBuf>, Never> {
  let base = match store {
    Store::Online => Base::Cache,
    Store::Accepted => Base::State,
  };
  let Ok(root) = base.ours();
  let Ok(key) = crate::review::hash_key(path);
  let name = RecordName(format!("{key:016x}.json"));

  Ok(root.map(|home| {
    let music = home.join("music");

    match store {
      Store::Online => music.join("metadata-search-v1").join(name),
      Store::Accepted => music.join("accepted-metadata-v1").join(name),
    }
  }))
}

fn lock(cache: &Path) -> io::Result<std::fs::File> {
  std::fs::create_dir_all(cache)?;
  let guard = console_core_atomic_writes::open_lock_file(&cache.join("metadata.lock"))?;
  guard.lock()?;

  Ok(guard)
}

pub(crate) fn forget(cache: &Path, removed: &BTreeSet<String>) -> io::Result<()> {
  let _guard = lock(cache)?;

  for path in removed {
    let Ok(online) = stored_path(path, Store::Online);
    let Ok(accepted) = stored_path(path, Store::Accepted);

    match online {
      Some(at) => match crate::review::read_json::<Vec<Proposal>>(&at) {
        Some(saved) => match saved
          .iter()
          .all(|proposal| proposal.file.path == *path)
          .then_some(())
        {
          Some(()) => crate::review::remove(&at)?,
          None => {}
        },
        None => {}
      },
      None => {}
    }

    match accepted {
      Some(at) => match crate::review::read_json::<Proposal>(&at) {
        Some(saved) => match (saved.file.path == *path).then_some(()) {
          Some(()) => crate::review::remove(&at)?,
          None => {}
        },
        None => {}
      },
      None => {}
    }
  }

  let at = cache.join("metadata-suggestions-v1.json");

  match crate::review::read_json::<Vec<Proposal>>(&at) {
    Some(mut proposals) => {
      proposals.retain(|proposal| !removed.contains(&proposal.file.path));
      crate::review::write_json(&at, &proposals)
    }
    None => Ok(()),
  }
}

pub fn online_results(file: &FoundTrack) -> Result<Option<Vec<Proposal>>, Never> {
  let Ok(path) = stored_path(&file.path, Store::Online);
  let candidates = path.and_then(|at| crate::review::read_json::<Vec<Proposal>>(&at));

  Ok(candidates.map(|found| {
    found
      .into_iter()
      .filter(|item| item.file == *file)
      .collect()
  }))
}

pub fn apply(choice: &Proposal) -> io::Result<()> {
  let Ok(cache) = Base::Cache.ours();
  let _guard = match cache {
    Some(root) => {
      let opened = lock(&root.join("music"))?;
      Some(opened)
    }
    None => None,
  };
  validate(&choice.file)?;
  let Ok(at) = stored_path(&choice.file.path, Store::Accepted);
  let destination = at.ok_or_else(|| io::Error::other("metadata state directory unavailable"))?;
  let bytes = serde_json::to_vec(choice).map_err(io::Error::other)?;

  console_core_atomic_writes::whole_with_folders(&destination, &bytes).map_err(io::Error::other)
}

fn validate(file: &FoundTrack) -> io::Result<()> {
  let Ok(decoded) = console_core_file_urls::path(&file.path);
  let path = decoded.ok_or_else(|| io::Error::other("invalid recording path"))?;
  let Ok(now) = FileRevision::of(&path);

  match (now.is_some() && now == file.revision).then_some(()) {
    Some(()) => Ok(()),
    None => Err(io::Error::other("recording changed before review")),
  }
}

pub fn apply_saved(songs: &mut [Song]) -> Result<(), Never> {
  for song in songs {
    match missing(song) {
      Ok(Some(())) => {}
      Ok(None) => continue,
    }

    let Ok(query) = query(song);
    let Ok(location) = stored_path(&query.file.path, Store::Accepted);
    let stored = location.and_then(|at| crate::review::read_json::<Proposal>(&at));
    let tags = match stored {
      Some(proposal) => match (proposal.file == query.file).then_some(()) {
        Some(()) => proposal.tags,
        None => continue,
      },
      None => continue,
    };
    song.tags.title = tags.title;
    song.tags.artist = tags.artist;

    match (song.tags.album.title.is_empty() && !tags.album.is_empty()).then_some(()) {
      Some(()) => {
        song.tags.album = Album {
          title: tags.album,
          artist: tags.album_artist,
          release_id: tags.release,
          disc: tags.disc,
          track: tags.track,
        };
      }
      None => {}
    }
  }

  Ok(())
}

#[derive(Deserialize)]
struct Response {
  recordings: Vec<Recording>,
}

#[derive(Deserialize)]
struct Recording {
  title: String,
  score: u32,
  #[serde(default, rename = "artist-credit")]
  artist: Vec<Credit>,
  #[serde(default)]
  releases: Vec<Release>,
}

#[derive(Deserialize)]
struct Credit {
  name: String,
  #[serde(default)]
  joinphrase: String,
}

#[derive(Deserialize)]
struct Release {
  title: String,
}

fn candidate(recording: Recording, search: &Search) -> Option<Tags> {
  let artist: String = recording
    .artist
    .into_iter()
    .flat_map(|credit| [credit.name, credit.joinphrase])
    .collect();
  let possible = recording.score >= 90
    && words(&recording.title) == words(&search.title)
    && !artist.is_empty()
    && (search.artist.is_empty() || words(&artist) == words(&search.artist));
  let _matched = possible.then_some(())?;
  let releases: BTreeSet<_> = recording
    .releases
    .into_iter()
    .map(|release| release.title)
    .collect();
  let album = match releases.len() {
    1 => releases.into_iter().next().unwrap_or_default(),
    _ => String::new(),
  };

  Some(Tags {
    title: recording.title,
    artist,
    album,
    album_artist: String::new(),
    release: String::new(),
    disc: None,
    track: None,
  })
}

pub fn search(search: &Search) -> io::Result<()> {
  let Ok(at) = stored_path(&search.file.path, Store::Online);
  let at = at.ok_or_else(|| io::Error::other("metadata cache is unavailable"))?;
  let Ok(root) = Base::Cache.ours();
  let root = root.ok_or_else(|| io::Error::other("music cache is unavailable"))?;
  let request_slot = crate::album_art::requests(
    &root.join("music").join("album-art-v2"),
    std::time::SystemTime::now(),
  )?;
  let _guard =
    request_slot.ok_or_else(|| io::Error::other("MusicBrainz is busy, try again in a moment"))?;
  let quote = |value: &str| value.replace('\\', "\\\\").replace('"', "\\\"");
  let title = quote(&search.title);
  let artist = quote(&search.artist);
  let query = match artist.is_empty().then_some(()) {
    Some(()) => format!("query=recording:\"{title}\""),
    None => format!("query=recording:\"{title}\" AND artist:\"{artist}\""),
  };
  let response = crate::album_art::curl(&[
    "--get",
    "--data-urlencode",
    &query,
    "--data-urlencode",
    "fmt=json",
    "--data-urlencode",
    "limit=20",
    "https://musicbrainz.org/ws/2/recording/",
  ])?;
  let mut seen = BTreeSet::new();
  let proposals: Vec<_> = match response {
    Some(bytes) => {
      let response: Response = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
      response
        .recordings
        .into_iter()
        .filter_map(|recording| candidate(recording, search))
        .filter(|tags| seen.insert(tags.clone()))
        .take(5)
        .map(|tags| Proposal {
          file: search.file.clone(),
          tags,
          source: Source::MusicBrainz,
        })
        .collect()
    }
    None => Vec::new(),
  };
  let bytes = serde_json::to_vec(&proposals).map_err(io::Error::other)?;
  let _metadata = lock(&root.join("music"))?;
  validate(&search.file)?;

  console_core_atomic_writes::whole_with_folders(&at, &bytes).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn online_candidates_require_title_and_known_artist_to_match() {
    let Ok(song) = Song::of(Path::new("/music/Fixture Artist - Fixture Song.flac"));
    let Ok(search) = query(&song);
    let entry = |artist: &str, title: &str| Recording {
      title: title.to_owned(),
      score: 100,
      artist: vec![Credit {
        name: artist.to_owned(),
        joinphrase: String::new(),
      }],
      releases: vec![
        Release {
          title: "First Album".to_owned(),
        },
        Release {
          title: "Second Album".to_owned(),
        },
      ],
    };
    assert!(candidate(entry("Other Artist", "Fixture Song"), &search).is_none());
    assert!(candidate(entry("Fixture Artist", "Other Song"), &search).is_none());
    let selected = candidate(entry("Fixture Artist", "Fixture Song"), &search);
    assert_eq!(selected.map(|tag| tag.album), Some(String::new()));
  }
}
