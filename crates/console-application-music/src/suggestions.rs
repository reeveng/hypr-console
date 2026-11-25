//! Suggested library metadata is reviewed before it is stored as a local overlay.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use console_core_never::Never;
use console_core_places::Base;
use console_music_player::tags::Album;
use console_waiting::{Schedule, clock::LiveClock, until_some_handed_on_result};
use serde::{Deserialize, Serialize};

use crate::looking::{FileRevision, Song};
use crate::review::{Audio, FoundTrack};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
#[allow(
  unknown_lints,
  reason = "Explicit Rust checks this type before cfg_attr is expanded"
)]
#[expect(
  explicit064_no_unvalidated_input,
  reason = "metadata caches are decoded by read_json after its 16 MiB file limit"
)]
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
#[serde(deny_unknown_fields)]
pub struct Proposal {
  pub file: FoundTrack,
  pub tags: Tags,
  pub source: Source,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[allow(
  unknown_lints,
  reason = "Explicit Rust checks this type before cfg_attr is expanded"
)]
#[expect(
  explicit064_no_unvalidated_input,
  reason = "music-index decodes the argument with an explicit 128 KiB limit"
)]
pub struct Search {
  pub file: FoundTrack,
  title: String,
  artist: String,
}

pub fn missing(song: &Song) -> Result<Option<()>, Never> {
  Ok((song.tags.title.trim().is_empty() || song.tags.artist.trim().is_empty()).then_some(()))
}

fn words(text: &str) -> Result<String, Never> {
  Ok(
    text
      .split_whitespace()
      .collect::<Vec<_>>()
      .join(" ")
      .to_lowercase(),
  )
}

pub fn query(song: &Song) -> Result<Search, Never> {
  let Ok(path) = crate::review::key(&song.path);
  let Ok(revision) = song.revision();
  let separated = song
    .name
    .split_once(" - ")
    .or_else(|| song.name.split_once(" — "));
  let (artist, title) = match separated {
    Some((artist, title)) => (artist, title),
    None => ("", song.name.as_str()),
  };
  let artist_3 = match song.tags.artist.trim() {
    "" => artist.trim(),
    artist_2 => artist_2,
  };
  let title_3 = match song.tags.title.trim() {
    "" => title.trim(),
    title_2 => title_2,
  };

  Ok(Search {
    file: FoundTrack { path, revision },
    title: title_3.to_owned(),
    artist: artist_3.to_owned(),
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

  for (song, audio_2) in songs.iter().zip(audio) {
    match missing(song) {
      Ok(Some(())) => continue,
      Ok(None) => {}
    }

    let Ok(tags) = tagged(song);
    let Ok(artist) = words(&tags.artist);
    let Ok(title) = words(&tags.title);
    by_name
      .entry((artist, title))
      .or_default()
      .insert(tags.clone());

    match audio_2 {
      Some(audio_3) => {
        let Ok(key) = audio_3.checksum();
        let _known = by_audio.entry(key).or_default().insert(tags);
      }
      None => {}
    }
  }

  let mut suggestions = Vec::new();

  for (song, audio_4) in songs.iter().zip(audio) {
    match missing(song) {
      Ok(Some(())) => {}
      Ok(None) => continue,
    }

    let Ok(query) = query(song);
    let wave = audio_4.as_ref().and_then(|audio_5| {
      let Ok(key) = audio_5.checksum();

      by_audio.get(&key)
    });
    let (candidates, source) = match wave {
      Some(candidates) => (Some(candidates), Source::Audio),
      None => {
        let Ok(artist) = words(&query.artist);
        let Ok(title) = words(&query.title);

        (by_name.get(&(artist, title)), Source::Filename)
      }
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

  let proposals = match cache {
    Some(root) => {
      let Ok(proposals) =
        crate::review::read_json(&root.join("music").join("metadata-suggestions-v1.json"));

      proposals
    }
    None => None,
  };

  Ok(match proposals {
    Some(proposals_2) => proposals_2,
    None => Vec::new(),
  })
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
        Ok(Some(saved)) => match saved
          .iter()
          .all(|proposal| proposal.file.path == *path)
          .then_some(())
        {
          Some(()) => crate::review::remove(&at)?,
          None => {}
        },
        Ok(None) => {}
      },
      None => {}
    }

    match accepted {
      Some(at) => match crate::review::read_json::<Proposal>(&at) {
        Ok(Some(saved)) => match (saved.file.path == *path).then_some(()) {
          Some(()) => crate::review::remove(&at)?,
          None => {}
        },
        Ok(None) => {}
      },
      None => {}
    }
  }

  let at = cache.join("metadata-suggestions-v1.json");

  match crate::review::read_json::<Vec<Proposal>>(&at) {
    Ok(Some(mut proposals)) => {
      proposals.retain(|proposal| !removed.contains(&proposal.file.path));
      crate::review::write_json(&at, &proposals)
    }
    Ok(None) => Ok(()),
  }
}

pub fn online_results(file: &FoundTrack) -> Result<Option<Vec<Proposal>>, Never> {
  let Ok(path) = stored_path(&file.path, Store::Online);
  let candidates = match path {
    Some(at) => crate::review::read_json::<Vec<Proposal>>(&at)?,
    None => None,
  };

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
    let stored = match location {
      Some(at) => crate::review::read_json::<Proposal>(&at)?,
      None => None,
    };
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
#[allow(
  unknown_lints,
  reason = "Explicit Rust checks this type before cfg_attr is expanded"
)]
#[expect(
  explicit064_no_unvalidated_input,
  reason = "MusicBrainz includes response counts and offsets, and curl bounds the response body"
)]
struct Response {
  recordings: Vec<Recording>,
}

#[derive(Deserialize)]
#[allow(
  unknown_lints,
  reason = "Explicit Rust checks this type before cfg_attr is expanded"
)]
#[expect(
  explicit064_no_unvalidated_input,
  reason = "MusicBrainz includes recording metadata outside the fields used here, and curl bounds the response body"
)]
struct Recording {
  title: String,
  score: u32,
  #[serde(default, rename = "artist-credit")]
  artist: Vec<Credit>,
  #[serde(default)]
  releases: Vec<Release>,
}

#[derive(Deserialize)]
#[allow(
  unknown_lints,
  reason = "Explicit Rust checks this type before cfg_attr is expanded"
)]
#[expect(
  explicit064_no_unvalidated_input,
  reason = "MusicBrainz includes artist metadata outside the credit text used here, and curl bounds the response body"
)]
struct Credit {
  name: String,
  #[serde(default)]
  joinphrase: String,
}

#[derive(Deserialize)]
#[allow(
  unknown_lints,
  reason = "Explicit Rust checks this type before cfg_attr is expanded"
)]
#[expect(
  explicit064_no_unvalidated_input,
  reason = "MusicBrainz includes release metadata outside the title used here, and curl bounds the response body"
)]
struct Release {
  title: String,
}

fn candidate(recording: Recording, search: &Search) -> Result<Option<Tags>, Never> {
  let artist: String = recording
    .artist
    .into_iter()
    .flat_map(|credit| [credit.name, credit.joinphrase])
    .collect();
  let Ok(title) = words(&recording.title);
  let Ok(search_title) = words(&search.title);
  let Ok(known_artist) = words(&artist);
  let Ok(search_artist) = words(&search.artist);
  let possible = recording.score >= 90
    && title == search_title
    && !artist.is_empty()
    && (search.artist.is_empty() || known_artist == search_artist);

  match possible.then_some(()) {
    Some(()) => {}
    None => return Ok(None),
  }

  let releases: BTreeSet<_> = recording
    .releases
    .into_iter()
    .map(|release| release.title)
    .collect();
  let album = match releases.len() {
    1 => match releases.into_iter().next() {
      Some(title_2) => title_2,
      None => String::new(),
    },
    _ => String::new(),
  };

  Ok(Some(Tags {
    title: recording.title,
    artist,
    album,
    album_artist: String::new(),
    release: String::new(),
    disc: None,
    track: None,
  }))
}

fn proposals(response: Response, search: &Search) -> Result<Vec<Proposal>, Never> {
  let mut seen = BTreeSet::new();
  let mut proposals = Vec::new();

  for recording in response.recordings {
    let Ok(candidate) = candidate(recording, search);
    let tags = match candidate {
      Some(tags) => tags,
      None => continue,
    };

    match seen.insert(tags.clone()).then_some(()) {
      Some(()) => proposals.push(Proposal {
        file: search.file.clone(),
        tags,
        source: Source::MusicBrainz,
      }),
      None => continue,
    }

    match proposals.len() {
      5 => break,
      _ => {}
    }
  }

  Ok(proposals)
}

fn request_slot(
  cache: &Path,
  mut observe: impl FnMut() -> SystemTime,
) -> io::Result<std::fs::File> {
  let Ok(mut clock) = LiveClock::started();
  let Ok(patience) = Schedule::asking_every(Duration::from_secs(45), Duration::from_millis(150));
  let available = until_some_handed_on_result(&mut clock, patience, &mut observe, |now| {
    use crate::album_art::RequestSlot;

    let slot = crate::album_art::requests(cache, now())?;

    Ok::<_, io::Error>(match slot {
      RequestSlot::Granted(lock) => Some(lock),
      RequestSlot::Busy | RequestSlot::CoolingDown => None,
    })
  })?;

  available.ok_or_else(|| io::Error::other("MusicBrainz request limit did not clear in time"))
}

pub fn search(search: &Search, observe: impl FnMut() -> SystemTime) -> io::Result<()> {
  let Ok(destination) = stored_path(&search.file.path, Store::Online);
  let at = destination.ok_or_else(|| io::Error::other("metadata cache is unavailable"))?;
  let Ok(cache) = Base::Cache.ours();
  let root = cache.ok_or_else(|| io::Error::other("music cache is unavailable"))?;
  let _guard = request_slot(&root.join("music").join("album-art-v2"), observe)?;
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
  let proposals: Vec<_> = match response {
    Some(bytes) => {
      let response_2: Response = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
      let Ok(found) = proposals(response_2, search);

      found
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
  fn a_following_lookup_waits_for_the_shared_rate_limit() -> Result<(), Box<dyn std::error::Error>>
  {
    let cache = console_core_temporary_directories::fresh("musicbrainz-search-permits")?;
    let first = request_slot(&cache, SystemTime::now)?;
    drop(first);

    let next = request_slot(&cache, SystemTime::now)?;
    drop(next);

    Ok(())
  }

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
    assert_eq!(
      candidate(entry("Other Artist", "Fixture Song"), &search),
      Ok(None)
    );
    assert_eq!(
      candidate(entry("Fixture Artist", "Other Song"), &search),
      Ok(None)
    );
    let Ok(selected) = candidate(entry("Fixture Artist", "Fixture Song"), &search);
    assert_eq!(selected.map(|tag| tag.album), Some(String::new()));
  }
}
