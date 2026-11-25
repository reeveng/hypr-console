//! Album covers are cached locally. Online lookup is an explicit user action.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, SystemTime};

use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_places::Base;
use console_core_state_machine::{
  EffectDependencies, Service, SingleEffectMachine, SingleTransition,
};
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
const COVER_NAMES: [&str; 4] = ["cover.jpg", "cover.png", "folder.jpg", "front.jpg"];

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
#[allow(
  unknown_lints,
  reason = "Explicit Rust checks this type before cfg_attr is expanded"
)]
#[expect(
  explicit064_no_unvalidated_input,
  reason = "music-index decodes the argument with an explicit 128 KiB limit"
)]
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

  #[cfg_attr(
    dylint_lib = "explicit086_no_climbing_join",
    expect(
      explicit086_no_climbing_join,
      reason = "the filename is a locally formatted 16-digit hash with a fixed extension"
    )
  )]
  let destination = cache.map(|root| {
    root
      .join("music")
      .join("album-art-v2")
      .join(format!("{hash:016x}.jpg"))
  });

  Ok(destination)
}

pub fn cached(key: &Key) -> Result<Option<PathBuf>, Never> {
  let Ok(candidate) = location(key);

  Ok(candidate.filter(|path| matches!(cached_cover(path), Ok(Some(())))))
}

fn cached_cover(path: &Path) -> Result<Option<()>, Never> {
  match console_core_atomic_writes::read_prefix(path, 0) {
    Ok(_) => Ok(Some(())),
    Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
      Some(()) => Ok(None),
      None => {
        console_core_logging::debug!("could not open cached cover {}: {fault}", path.display());

        Ok(None)
      }
    },
  }
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
      let image_3 = match image {
        Some(image_2) => image_2,
        None => return Ok(Cover::Unavailable),
      };
      console_core_atomic_writes::whole_with_folders(destination, &image_3)
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

  match size.filter(|size_2| size_2.width.max(size_2.height) <= 8192) {
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
  let Ok(size_3) = pixels.size();
  let Ok(quality) = console_core_jpeg_files::Quality::percent(QUALITY);
  let jpeg = console_core_jpeg_files::encoded(
    &console_core_jpeg_files::Picture {
      size: size_3,
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
  let Ok(location) = location(&album.key);
  let destination = match location {
    Some(destination) => match cached_cover(&destination) {
      Ok(Some(())) => return Ok(()),
      Ok(None) => destination,
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
        .find_map(|parent| {
          let Ok(cover) = sidecar(parent);

          cover
        })
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

fn sidecar(parent: &Path) -> Result<Option<Vec<u8>>, Never> {
  for name in COVER_NAMES {
    #[cfg_attr(
      dylint_lib = "explicit086_no_climbing_join",
      expect(
        explicit086_no_climbing_join,
        reason = "COVER_NAMES holds four literal file basenames"
      )
    )]
    let candidate = parent.join(name);
    let bytes =
      match console_core_atomic_writes::read_prefix(&candidate, MAX_ART.saturating_add(1)) {
        Ok(bytes) => bytes,
        Err(fault) => {
          match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
            Some(()) => {}
            None => console_core_logging::debug!(
              "could not read cover {}: {fault}",
              candidate.display()
            ),
          }

          continue;
        }
      };
    let Ok(valid) = is_cover(&bytes);

    match valid {
      Some(()) => return Ok(Some(bytes)),
      None => {}
    }
  }

  Ok(None)
}

#[derive(Deserialize)]
#[allow(
  unknown_lints,
  reason = "Explicit Rust checks this type before cfg_attr is expanded"
)]
#[expect(
  explicit064_no_unvalidated_input,
  reason = "MusicBrainz includes response metadata outside the fields used here, and curl bounds the response body"
)]
struct Search {
  #[serde(rename = "release-groups")]
  groups: Vec<Group>,
}

#[derive(Deserialize)]
#[allow(
  unknown_lints,
  reason = "Explicit Rust checks this type before cfg_attr is expanded"
)]
#[expect(
  explicit064_no_unvalidated_input,
  reason = "MusicBrainz adds unrelated release-group fields, and curl bounds the response body"
)]
struct Group {
  id: String,
  title: String,
  #[serde(rename = "artist-credit")]
  credit: Vec<Credit>,
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

fn normalized(said: &str) -> Result<String, Never> {
  Ok(
    said
      .split_whitespace()
      .collect::<Vec<_>>()
      .join(" ")
      .to_lowercase(),
  )
}

struct NamedRelease<'a> {
  artist: &'a str,
  title: &'a str,
}

fn match_group(search: Search, wanted: NamedRelease<'_>) -> Result<Option<String>, Never> {
  let mut matches = search.groups.into_iter().filter_map(|group| {
    let author: String = group
      .credit
      .into_iter()
      .flat_map(|credit| [credit.name, credit.joinphrase])
      .collect();
    let Ok(id) = release_id(&group.id);
    let Ok(artist) = normalized(&author);
    let Ok(title) = normalized(&group.title);

    match (id, artist == wanted.artist && title == wanted.title) {
      (Some(_valid), true) => Some(group.id),
      (None, _) | (_, false) => None,
    }
  });

  Ok(match (matches.next(), matches.next()) {
    (Some(id), None) => Some(id),
    _ambiguous_or_missing => None,
  })
}

pub(crate) enum RequestSlot {
  Granted(std::fs::File),
  Busy,
  CoolingDown,
}

pub(crate) fn requests(cache: &Path, observed: SystemTime) -> io::Result<RequestSlot> {
  use std::fs::TryLockError;

  std::fs::create_dir_all(cache)?;
  let lock = console_core_atomic_writes::open_lock_file(&cache.join("musicbrainz-rate"))?;

  match lock.try_lock() {
    Ok(()) => {}
    Err(TryLockError::WouldBlock) => return Ok(RequestSlot::Busy),
    Err(TryLockError::Error(fault)) => return Err(fault),
  }

  let stamp = cache.join("musicbrainz-last-request");
  let previous = last_request(&lock, &stamp)?;

  match previous.filter(|last| {
    observed
      .duration_since(*last)
      .is_ok_and(|age| age < Duration::from_secs(1))
  }) {
    Some(_) => return Ok(RequestSlot::CoolingDown),
    None => {}
  }

  console_core_atomic_writes::publish(&stamp, b"1").map_err(io::Error::other)?;

  Ok(RequestSlot::Granted(lock))
}

fn last_request(lock: &std::fs::File, stamp: &Path) -> io::Result<Option<SystemTime>> {
  let legacy = lock.metadata()?;
  let legacy_time = match legacy.len() {
    0 => None,
    _ => {
      let modified = legacy.modified()?;

      Some(modified)
    }
  };
  let stamp_time = match std::fs::metadata(stamp) {
    Ok(info) => {
      let modified = info.modified()?;

      Some(modified)
    }
    Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
      Some(()) => None,
      None => return Err(fault),
    },
  };

  Ok(legacy_time.max(stamp_time))
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

  let (body, code) = reply
    .stdout
    .split_at_checked(reply.stdout.len().saturating_sub(3))
    .ok_or_else(|| io::Error::other("album artwork request returned an invalid response"))?;
  let Ok(body_bytes) = console_core_number_conversion::fixed_index(body.len());

  match (body_bytes <= u64::from(MAX_ART)).then_some(()) {
    Some(()) => {}
    None => {
      return Err(io::Error::other(
        "album artwork response exceeded its size limit",
      ));
    }
  }

  match (reply.status.success(), code) {
    (true, b"200") => Ok(Some(body.to_vec())),
    (_, b"404") => Ok(None),
    _ => Err(io::Error::other(format!(
      "album artwork request ended with {}",
      reply.status
    ))),
  }
}

fn route(
  key: &Key,
  cache: &Path,
  observe: &mut impl FnMut() -> SystemTime,
) -> io::Result<Option<String>> {
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

      let slot = requests(cache, observe())?;

      let _guard = match slot {
        RequestSlot::Granted(lock) => lock,
        RequestSlot::Busy | RequestSlot::CoolingDown => {
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
        Some(bytes_2) => serde_json::from_slice(&bytes_2).map_err(io::Error::other)?,
        None => return Ok(None),
      };
      let Ok(group) = match_group(search, NamedRelease { artist, title });

      match group {
        Some(id) => ("release-group", id),
        None => return Ok(None),
      }
    }
  };

  Ok(Some(format!("https://coverartarchive.org/{kind}/{id}/")))
}

#[derive(Deserialize)]
#[allow(
  unknown_lints,
  reason = "Explicit Rust checks this type before cfg_attr is expanded"
)]
#[expect(
  explicit064_no_unvalidated_input,
  reason = "Cover Art Archive includes release metadata outside the image list, and curl bounds the response body"
)]
struct Archive {
  images: Vec<Image>,
}

#[derive(Deserialize)]
#[allow(
  unknown_lints,
  reason = "Explicit Rust checks this type before cfg_attr is expanded"
)]
#[expect(
  explicit064_no_unvalidated_input,
  reason = "Cover Art Archive includes image metadata outside these fields, and curl bounds the response body"
)]
struct Image {
  front: ImageSide,
  approved: Approval,
  image: String,
  #[serde(default)]
  thumbnails: BTreeMap<String, String>,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(from = "bool")]
enum ImageSide {
  Front,
  Other,
}

impl From<bool> for ImageSide {
  fn from(value: bool) -> Self {
    match value {
      true => Self::Front,
      false => Self::Other,
    }
  }
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(from = "bool")]
enum Approval {
  Approved,
  Unapproved,
}

impl From<bool> for Approval {
  fn from(value: bool) -> Self {
    match value {
      true => Self::Approved,
      false => Self::Unapproved,
    }
  }
}

fn archive_cover(bytes: &[u8]) -> io::Result<Option<String>> {
  let archive: Archive = serde_json::from_slice(bytes).map_err(io::Error::other)?;
  let front = archive
    .images
    .iter()
    .find(|image| image.front == ImageSide::Front && image.approved == Approval::Approved);
  let name = match front {
    Some(image) => match image
      .thumbnails
      .get("250")
      .or_else(|| image.thumbnails.get("small"))
    {
      Some(thumbnail) => thumbnail,
      None => &image.image,
    },
    None => return Ok(None),
  };
  let site = name
    .strip_prefix("http://")
    .or_else(|| name.strip_prefix("https://"));
  let url = match site.and_then(|site_2| site_2.split_once('/')) {
    Some(("coverartarchive.org" | "archive.org", _)) => site,
    Some((host, _)) => site.filter(|_| host.ends_with(".archive.org")),
    None => None,
  };

  Ok(url.map(|url_2| format!("https://{url}", url = url_2)))
}

fn official_cover(
  key: &Key,
  cache: &Path,
  observe: &mut impl FnMut() -> SystemTime,
) -> io::Result<Option<Vec<u8>>> {
  let route = route(key, cache, observe)?;
  let route_3 = match route {
    Some(route_2) => route_2,
    None => return Ok(None),
  };
  let listing = curl(&[&route_3])?;
  let listing_2 = match listing {
    Some(bytes) => bytes,
    None => return Ok(None),
  };
  let url = archive_cover(&listing_2)?;

  match url {
    Some(url_2) => curl(&[&url_2]),
    None => Ok(None),
  }
}

#[derive(Deserialize)]
#[allow(
  unknown_lints,
  reason = "Explicit Rust checks this type before cfg_attr is expanded"
)]
#[expect(
  explicit064_no_unvalidated_input,
  reason = "yt-dlp returns extra video metadata, and fallback_cover bounds the reply before decoding"
)]
struct Video {
  id: String,
  title: String,
  channel: Option<String>,
  uploader: Option<String>,
}

fn matches_album(video: &Video, lookup: &Lookup) -> Result<Option<()>, Never> {
  let Ok(artist) = normalized(&lookup.artist);
  let Ok(album) = normalized(&lookup.title);
  let credit = video
    .channel
    .as_deref()
    .or(video.uploader.as_deref())
    .map(normalized);
  let credit_3 = match credit {
    Some(Ok(credit_2)) => credit_2,
    None => String::new(),
  };
  let topic = format!("{artist} - topic");
  let Ok(title) = normalized(&video.title);
  let trimmed = title
    .strip_suffix(" (full album)")
    .or_else(|| title.strip_suffix(" [full album]"));
  let title_2 = match trimmed {
    Some(trimmed_2) => trimmed_2,
    None => title.as_str(),
  };
  let expected = [format!("{artist} - {album}"), format!("{artist} – {album}")];

  Ok(
    ((credit_3 == artist || credit_3 == topic)
      && (title_2 == album || expected.iter().any(|title_match| title_2 == title_match)))
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

  let Ok(output_bytes) = console_core_number_conversion::fixed_index(output.stdout.len());

  match (output_bytes <= u64::from(MAX_ART)).then_some(()) {
    Some(()) => {}
    None => {
      return Err(io::Error::other(
        "artwork search response exceeded its size limit",
      ));
    }
  }

  let video = output
    .stdout
    .split(|byte| *byte == b'\n')
    .filter(|line| !line.is_empty())
    .filter_map(|line| match serde_json::from_slice::<Video>(line) {
      Ok(video) => Some(video),
      Err(fault) => {
        console_core_logging::debug!("could not decode an artwork search reply: {fault}");

        None
      }
    })
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
    Some(video_2) => curl(&[&format!(
      "https://i.ytimg.com/vi/{}/hqdefault.jpg",
      video_2.id
    )]),
    None => Ok(None),
  }
}

enum FetchState {
  CheckingArchive,
  SavingArchive,
  CheckingFallback,
  SavingFallback,
  Done(io::Result<Cover>),
}

enum FetchEffect {
  FindArchive,
  FindFallback,
  Save(Vec<u8>),
}

impl EffectDependencies for FetchEffect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::FindArchive => &[
        Service::FileSystem,
        Service::Network,
        Service::Process,
        Service::Clock,
      ],
      Self::FindFallback => &[Service::Network, Service::Process],
      Self::Save(_) => &[Service::FileSystem],
    })
  }
}

enum FetchReply {
  Archive(Option<Vec<u8>>),
  Fallback(io::Result<Option<Vec<u8>>>),
  Saved(io::Result<Cover>),
}

struct Fetch;

type FetchTurn = SingleTransition<FetchState, FetchEffect>;

impl SingleEffectMachine for Fetch {
  type Input = ();
  type State = FetchState;
  type Request = FetchReply;
  type Effect = FetchEffect;

  fn initialize(_input: &(), _previous: Option<FetchState>) -> Result<FetchTurn, Never> {
    Ok(FetchTurn {
      state: FetchState::CheckingArchive,
      effect: Some(FetchEffect::FindArchive),
    })
  }

  fn handle(state: FetchState, request: FetchReply) -> Result<FetchTurn, Never> {
    let (state_2, effect) = match (state, request) {
      (FetchState::CheckingArchive, FetchReply::Archive(Some(bytes))) => {
        (FetchState::SavingArchive, Some(FetchEffect::Save(bytes)))
      }
      (FetchState::CheckingArchive, FetchReply::Archive(None))
      | (FetchState::SavingArchive, FetchReply::Saved(Ok(Cover::Unavailable))) => (
        FetchState::CheckingFallback,
        Some(FetchEffect::FindFallback),
      ),
      (FetchState::CheckingFallback, FetchReply::Fallback(Ok(Some(bytes)))) => {
        (FetchState::SavingFallback, Some(FetchEffect::Save(bytes)))
      }
      (FetchState::CheckingFallback, FetchReply::Fallback(Ok(None))) => {
        (FetchState::Done(Ok(Cover::Unavailable)), None)
      }
      (FetchState::CheckingFallback, FetchReply::Fallback(Err(fault))) => {
        (FetchState::Done(Err(fault)), None)
      }
      (FetchState::SavingArchive, FetchReply::Saved(result))
      | (FetchState::SavingFallback, FetchReply::Saved(result)) => {
        (FetchState::Done(result), None)
      }
      (unchanged, _) => (unchanged, None),
    };

    Ok(FetchTurn {
      state: state_2,
      effect,
    })
  }
}

pub fn fetch(lookup: &Lookup, mut observe: impl FnMut() -> SystemTime) -> io::Result<Cover> {
  let Ok(destination) = location(&lookup.album);
  let destination_2 =
    destination.ok_or_else(|| io::Error::other("album art cache is unavailable"))?;
  let cache = destination_2
    .parent()
    .ok_or_else(|| io::Error::other("album art cache has no parent"))?;

  let Ok(start) = Fetch::initialize(&(), None);
  let settled = Fetch::run_effects(start, 4, &mut observe, |observe_2, effect| match effect {
    FetchEffect::FindArchive => {
      let bytes = match official_cover(&lookup.album, cache, observe_2) {
        Ok(bytes) => bytes,
        Err(fault) => {
          console_core_logging::debug!("could not get Cover Art Archive artwork: {fault}");

          None
        }
      };

      FetchReply::Archive(bytes)
    }
    FetchEffect::FindFallback => FetchReply::Fallback(fallback_cover(lookup)),
    FetchEffect::Save(bytes) => FetchReply::Saved(save(&destination_2, &bytes)),
  });

  match settled {
    Ok(FetchState::Done(result)) => result,
    Ok(
      FetchState::CheckingArchive
      | FetchState::SavingArchive
      | FetchState::CheckingFallback
      | FetchState::SavingFallback,
    )
    | Err(_) => Err(io::Error::other("album artwork lookup did not finish")),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn unusable_archive_art_falls_back_and_saves_the_second_image() {
    let Ok(start) = Fetch::initialize(&(), None);
    let mut attempted = Vec::new();
    let result = Fetch::run_effects(start, 4, &mut attempted, |attempts, effect| match effect {
      FetchEffect::FindArchive => FetchReply::Archive(Some(vec![1])),
      FetchEffect::FindFallback => FetchReply::Fallback(Ok(Some(vec![2]))),
      FetchEffect::Save(bytes) => {
        let outcome = match bytes.as_slice() {
          [2] => Cover::Saved,
          _ => Cover::Unavailable,
        };
        attempts.push(bytes);

        FetchReply::Saved(Ok(outcome))
      }
    });

    assert_eq!(attempted, [vec![1], vec![2]]);
    assert!(matches!(result, Ok(FetchState::Done(Ok(Cover::Saved)))));
  }

  #[test]
  fn saving_failure_is_returned_before_fallback() {
    let Ok(start) = Fetch::initialize(&(), None);
    let mut fallback = Vec::new();
    let result = Fetch::run_effects(start, 4, &mut fallback, |attempts, effect| match effect {
      FetchEffect::FindArchive => FetchReply::Archive(Some(vec![1])),
      FetchEffect::FindFallback => {
        attempts.push(());

        FetchReply::Fallback(Ok(None))
      }
      FetchEffect::Save(_) => {
        FetchReply::Saved(Err(io::Error::from(io::ErrorKind::PermissionDenied)))
      }
    });

    assert!(fallback.is_empty());
    assert!(
      matches!(result, Ok(FetchState::Done(Err(fault))) if fault.kind() == io::ErrorKind::PermissionDenied)
    );
  }

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
