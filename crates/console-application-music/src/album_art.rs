//! Album covers are cached locally. Online lookup is an explicit user action.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, SystemTime};

use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_places::Base;
use serde::{Deserialize, Serialize};

use crate::albums::{Album, Key, release_id};

const AGENT: &str = concat!(
  "Console-Music-Library/",
  env!("CARGO_PKG_VERSION"),
  " (Music Library desktop application)"
);
const MAX_ART: u32 = 8 * 1024 * 1024;
const SIDE: u32 = 256;
const QUALITY: u8 = 58;

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct Lookup {
  pub album: Key,
  pub artist: String,
  pub title: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cover {
  Saved,
  Unavailable,
}

fn location(key: &Key) -> Result<Option<PathBuf>, Never> {
  let Ok(cache) = Base::Cache.ours();
  let bytes = match serde_json::to_vec(key) {
    Ok(bytes) => bytes,
    Err(fault) => {
      console_core_logging::warn!("could not encode the album identity: {fault}");

      return Ok(None);
    }
  };
  let hash = bytes
    .into_iter()
    .fold(0xcbf2_9ce4_8422_2325_u64, |sum, byte| {
      (sum ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
    });

  Ok(cache.map(|root| {
    root
      .join("music")
      .join("album-art-v2")
      .join(format!("{hash:016x}.jpg"))
  }))
}

pub fn cached(key: &Key) -> Result<Option<PathBuf>, Never> {
  let candidate = location(key)?;

  Ok(candidate.filter(|path| path.is_file()))
}

pub(crate) fn forget(key: &Key) -> io::Result<()> {
  let Ok(candidate) = location(key);

  match candidate {
    Some(path) => crate::review::remove(&path),
    None => Ok(()),
  }
}

fn is_cover(bytes: &[u8]) -> Result<Option<()>, Never> {
  Ok(
    (bytes.starts_with(b"\xff\xd8\xff")
      || bytes.starts_with(b"\x89PNG\r\n\x1a\n")
      || (bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP")))
    .then_some(()),
  )
}

fn save(destination: &Path, bytes: &[u8]) -> io::Result<Cover> {
  let Ok(valid) = is_cover(bytes);
  let Ok(size) = console_core_number_conversion::fitted::<_, u32>(bytes.len());

  match (valid, size <= MAX_ART) {
    (Some(()), true) => {
      let parent = destination
        .parent()
        .ok_or_else(|| io::Error::other("album artwork has no cache directory"))?;
      let image = thumbnail(parent, bytes)?;
      let image = match image {
        Some(image) => image,
        None => return Ok(Cover::Unavailable),
      };
      console_core_atomic_writes::whole_with_folders(destination, &image)
        .map_err(io::Error::other)?;

      Ok(Cover::Saved)
    }
    (None, _) | (_, false) => Ok(Cover::Unavailable),
  }
}

struct Scratch(PathBuf);

impl Drop for Scratch {
  fn drop(&mut self) {
    match std::fs::remove_dir_all(&self.0) {
      Ok(()) => {}
      Err(fault) => console_core_logging::debug!("clearing cover staging directory: {fault}"),
    }
  }
}

fn thumbnail(cache: &Path, bytes: &[u8]) -> io::Result<Option<Vec<u8>>> {
  std::fs::create_dir_all(cache)?;
  let at = console_core_temporary_directories::private_under(cache, "cover")
    .map_err(io::Error::other)?;
  let staged = Scratch(at);
  let path = staged.0.join("image");
  console_core_atomic_writes::whole(&path, bytes).map_err(io::Error::other)?;
  let size = console_pictures::measure(&path).map_err(io::Error::other)?;
  match size.filter(|size| size.width.max(size.height) <= 8192) {
    Some(_) => {}
    None => return Ok(None),
  }

  let decoded = console_pictures::decoded(
    &path,
    console_core_size::Size {
      width: SIDE,
      height: SIDE,
    },
  )
  .map_err(io::Error::other)?;
  let pixels = match decoded {
    Some(pixels) => pixels,
    None => return Ok(None),
  };
  let Ok(size) = pixels.size();
  let Ok(quality) = console_core_jpeg_files::Quality::percent(QUALITY);
  let jpeg = console_core_jpeg_files::encoded(
    &console_core_jpeg_files::Picture {
      size,
      bytes: pixels.bytes,
    },
    quality,
  )
  .map_err(io::Error::other)?;

  Ok(Some(jpeg))
}

pub fn local(albums: &[Album<'_>]) -> Result<(), Never> {
  let mut folders = BTreeMap::new();

  for album in albums {
    for parent in album.tracks.iter().filter_map(|song| song.path.parent()) {
      let owner = folders.entry(parent).or_insert(Some(&album.key));
      match (*owner).filter(|key| *key == &album.key) {
        Some(_) => {}
        None => *owner = None,
      }
    }
  }

  for album in albums {
    let Ok(()) = local_album(album, &folders);
  }

  Ok(())
}

fn local_album(album: &Album<'_>, folders: &BTreeMap<&Path, Option<&Key>>) -> Result<(), Never> {
  let destination = match location(&album.key)? {
    Some(destination) => match destination.is_file().then_some(()) {
      Some(()) => return Ok(()),
      None => destination,
    },
    None => return Ok(()),
  };

  let cover = album
    .tracks
    .iter()
    .find_map(|song| {
      let Ok(picture) = console_music_player::art::picture(&song.path);

      picture
    })
    .or_else(|| {
      album
        .tracks
        .iter()
        .filter_map(|song| song.path.parent())
        .filter(|parent| folders.get(parent).copied() == Some(Some(&album.key)))
        .find_map(sidecar)
    });

  match cover {
    Some(bytes) => match save(&destination, &bytes) {
      Ok(Cover::Saved | Cover::Unavailable) => {}
      Err(fault) => console_core_logging::warn!("could not save local cover: {fault}"),
    },
    None => {}
  }

  Ok(())
}

fn sidecar(parent: &Path) -> Option<Vec<u8>> {
  ["cover.jpg", "cover.png", "folder.jpg", "front.jpg"]
    .into_iter()
    .find_map(|name| {
      let bytes =
        console_core_atomic_writes::read_prefix(&parent.join(name), MAX_ART.saturating_add(1))
          .ok()?;
      let Ok(valid) = is_cover(&bytes);

      valid.map(|()| bytes)
    })
}

#[derive(Deserialize)]
struct Search {
  #[serde(rename = "release-groups")]
  groups: Vec<Group>,
}

#[derive(Deserialize)]
struct Group {
  id: String,
  title: String,
  #[serde(rename = "artist-credit")]
  credit: Vec<Credit>,
}

#[derive(Deserialize)]
struct Credit {
  name: String,
  #[serde(default)]
  joinphrase: String,
}

fn normalized(said: &str) -> String {
  said
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
    .to_lowercase()
}

fn match_group(search: Search, artist: &str, title: &str) -> Result<Option<String>, Never> {
  let mut matches = search.groups.into_iter().filter_map(|group| {
    let author: String = group
      .credit
      .into_iter()
      .flat_map(|credit| [credit.name, credit.joinphrase])
      .collect();
    let Ok(id) = release_id(&group.id);

    match (
      id,
      normalized(&author) == artist && normalized(&group.title) == title,
    ) {
      (Some(_valid), true) => Some(group.id),
      (None, _) | (_, false) => None,
    }
  });

  Ok(match (matches.next(), matches.next()) {
    (Some(id), None) => Some(id),
    _ambiguous_or_missing => None,
  })
}

pub(crate) fn requests(cache: &Path, observed: SystemTime) -> io::Result<Option<std::fs::File>> {
  use std::fs::TryLockError;

  std::fs::create_dir_all(cache)?;
  let mut lock = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(cache.join("musicbrainz-rate"))?;

  match lock.try_lock() {
    Ok(()) => {}
    Err(TryLockError::WouldBlock) => return Ok(None),
    Err(TryLockError::Error(fault)) => return Err(fault),
  }

  let info = lock.metadata()?;
  let last = info.modified()?;
  let recent = observed
    .duration_since(last)
    .is_ok_and(|age| age < Duration::from_secs(1));

  match (info.len() > 0 && recent).then_some(()) {
    Some(()) => Ok(None),
    None => {
      lock.set_len(0)?;
      use std::io::Write;
      lock.write_all(b"1")?;

      Ok(Some(lock))
    }
  }
}

pub(crate) fn curl(arguments: &[&str]) -> io::Result<Option<Vec<u8>>> {
  let Ok(mut command) = Program::Curl.command();
  let reply = command
    .args([
      "--disable",
      "--silent",
      "--show-error",
      "--fail",
      "--location",
      "--max-time",
      "20",
      "--max-filesize",
      "8388608",
      "--proto",
      "=https",
      "--proto-redir",
      "=https",
      "--user-agent",
      AGENT,
      "--write-out",
      "%{response_code}",
    ])
    .args(arguments)
    .stdin(Stdio::null())
    .output()?;

  let (body, code) = reply.stdout.split_at(reply.stdout.len().saturating_sub(3));

  match (reply.status.success(), code) {
    (true, b"200") => Ok(Some(body.to_vec())),
    (_, b"404") => Ok(None),
    _ => Err(io::Error::other(format!(
      "album artwork request ended with {}",
      reply.status
    ))),
  }
}

fn route(key: &Key, cache: &Path) -> io::Result<Option<String>> {
  let (kind, id) = match key {
    Key::Release(id) => {
      let Ok(valid) = release_id(id);

      match valid {
        Some(_) => ("release", id.clone()),
        None => return Ok(None),
      }
    }
    Key::Named { artist, title } => {
      match artist.is_empty().then_some(()) {
        Some(()) => return Ok(None),
        None => {}
      }

      let lock = requests(cache, SystemTime::now())?;

      let _guard = match lock {
        Some(lock) => lock,
        None => {
          return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "MusicBrainz is busy, try again in a moment",
          ));
        }
      };
      let quoted = |text: &str| text.replace('\\', "\\\\").replace('"', "\\\"");
      let query = format!(
        "query=releasegroup:\"{}\" AND artist:\"{}\"",
        quoted(title),
        quoted(artist)
      );
      let bytes = curl(&[
        "--get",
        "--data-urlencode",
        &query,
        "--data-urlencode",
        "fmt=json",
        "--data-urlencode",
        "limit=25",
        "https://musicbrainz.org/ws/2/release-group/",
      ])?;
      let search = match bytes {
        Some(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other)?,
        None => return Ok(None),
      };
      let Ok(group) = match_group(search, artist, title);

      match group {
        Some(id) => ("release-group", id),
        None => return Ok(None),
      }
    }
  };

  Ok(Some(format!("https://coverartarchive.org/{kind}/{id}/")))
}

#[derive(Deserialize)]
struct Archive {
  images: Vec<Image>,
}

#[derive(Deserialize)]
struct Image {
  front: bool,
  approved: bool,
  image: String,
  #[serde(default)]
  thumbnails: BTreeMap<String, String>,
}

fn archive_cover(bytes: &[u8]) -> io::Result<Option<String>> {
  let archive: Archive = serde_json::from_slice(bytes).map_err(io::Error::other)?;
  let front = archive
    .images
    .iter()
    .find(|image| image.front && image.approved);
  let name = match front {
    Some(image) => image
      .thumbnails
      .get("250")
      .or_else(|| image.thumbnails.get("small"))
      .unwrap_or(&image.image),
    None => return Ok(None),
  };
  let site = name
    .strip_prefix("http://")
    .or_else(|| name.strip_prefix("https://"));
  let url = match site.and_then(|site| site.split_once('/')) {
    Some(("coverartarchive.org" | "archive.org", _)) => site,
    Some((host, _)) => site.filter(|_| host.ends_with(".archive.org")),
    None => None,
  };

  Ok(url.map(|url| format!("https://{url}")))
}

fn official_cover(key: &Key, cache: &Path) -> io::Result<Option<Vec<u8>>> {
  let route = route(key, cache)?;
  let route = match route {
    Some(route) => route,
    None => return Ok(None),
  };
  let listing = curl(&[&route])?;
  let listing = match listing {
    Some(bytes) => bytes,
    None => return Ok(None),
  };
  let url = archive_cover(&listing)?;

  match url {
    Some(url) => curl(&[&url]),
    None => Ok(None),
  }
}

#[derive(Deserialize)]
struct Video {
  id: String,
  title: String,
  channel: Option<String>,
  uploader: Option<String>,
}

fn matches_album(video: &Video, lookup: &Lookup) -> Result<Option<()>, Never> {
  let artist = normalized(&lookup.artist);
  let album = normalized(&lookup.title);
  let credit = video
    .channel
    .as_deref()
    .or(video.uploader.as_deref())
    .map(normalized)
    .unwrap_or_default();
  let topic = format!("{artist} - topic");
  let title = normalized(&video.title);
  let title = title
    .strip_suffix(" (full album)")
    .or_else(|| title.strip_suffix(" [full album]"))
    .unwrap_or(&title);
  let expected = [format!("{artist} - {album}"), format!("{artist} – {album}")];

  Ok(
    ((credit == artist || credit == topic)
      && (title == album || expected.iter().any(|title_match| title == title_match)))
    .then_some(()),
  )
}

fn fallback_cover(lookup: &Lookup) -> io::Result<Option<Vec<u8>>> {
  match (lookup.artist.trim().is_empty() || lookup.title.trim().is_empty()).then_some(()) {
    Some(()) => return Ok(None),
    None => {}
  }

  let search = format!("ytsearch5:{} {} album", lookup.artist, lookup.title);
  let Ok(mut program) = Program::YtDlp.command();
  let output = program
    .args([
      "--ignore-config",
      "--no-cache-dir",
      "--socket-timeout",
      "8",
      "--retries",
      "0",
      "--extractor-retries",
      "0",
      "--simulate",
      "--flat-playlist",
      "--dump-json",
      "--",
    ])
    .arg(search)
    .stdin(Stdio::null())
    .output()?;
  match output.status.success().then_some(()) {
    Some(()) => {}
    None => return Err(io::Error::other("YouTube artwork search failed")),
  }

  let video = output
    .stdout
    .split(|byte| *byte == b'\n')
    .filter_map(|line| serde_json::from_slice::<Video>(line).ok())
    .find(|video| {
      let Ok(matched) = matches_album(video, lookup);
      let Ok(length) = console_core_number_conversion::fixed_index(video.id.len());

      matched.is_some()
        && length == 11
        && video
          .id
          .bytes()
          .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    });

  match video {
    Some(video) => curl(&[&format!(
      "https://i.ytimg.com/vi/{}/hqdefault.jpg",
      video.id
    )]),
    None => Ok(None),
  }
}

pub fn fetch(lookup: &Lookup) -> io::Result<Cover> {
  let Ok(destination) = location(&lookup.album);
  let destination =
    destination.ok_or_else(|| io::Error::other("album art cache is unavailable"))?;
  let cache = destination
    .parent()
    .ok_or_else(|| io::Error::other("album art cache has no parent"))?;

  match official_cover(&lookup.album, cache) {
    Ok(Some(bytes)) => {
      let cover = save(&destination, &bytes)?;
      match cover {
        Cover::Saved => return Ok(cover),
        Cover::Unavailable => {}
      }
    }
    Ok(None) => {}
    Err(fault) => {
      console_core_logging::debug!("could not get Cover Art Archive artwork: {fault}")
    }
  }

  let cover = fallback_cover(lookup)?;
  match cover {
    Some(bytes) => save(&destination, &bytes),
    None => Ok(Cover::Unavailable),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn video_art_requires_release_and_channel_to_agree() {
    let lookup = Lookup {
      album: Key::Named {
        artist: "fixture artist".to_owned(),
        title: "fixture album".to_owned(),
      },
      artist: "Fixture Artist".to_owned(),
      title: "Fixture Album".to_owned(),
    };
    let mut video = Video {
      id: "public-test".to_owned(),
      title: "Fixture Artist - Fixture Album (Full Album)".to_owned(),
      channel: Some("Fixture Artist - Topic".to_owned()),
      uploader: None,
    };
    assert_eq!(matches_album(&video, &lookup), Ok(Some(())));
    video.channel = Some("Unrelated music channel".to_owned());
    assert_eq!(matches_album(&video, &lookup), Ok(None));
    video.channel = Some("Fixture Artist".to_owned());
    video.title = "Different Album".to_owned();
    assert_eq!(matches_album(&video, &lookup), Ok(None));
  }
}
