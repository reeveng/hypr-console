//! Local audio comparisons and review decisions. Findings refer to playable paths.
//! The scanner compares decoded audio and titles without modifying the library.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};
use console_core_places::Base;
use console_music_player::{Seconds, decoding};
use serde::{Deserialize, Serialize};

use crate::looking::{FileRevision, Song};

const FNV_START: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_STEP: u64 = 0x100_0000_01b3;
const MAX_WINDOWS: u32 = 240;
const MAX_BLOCKS: u32 = 2_000_000;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(
  unknown_lints,
  reason = "Explicit Rust checks this type before cfg_attr is expanded"
)]
#[expect(
  explicit064_no_unvalidated_input,
  reason = "the waveform cache is decoded by read_json after its 16 MiB file limit"
)]
pub struct Audio {
  hash: u64,
  millis: u64,
  windows: Vec<Window>,
}

impl Audio {
  pub(crate) fn checksum(&self) -> Result<u64, Never> {
    Ok(self.hash)
  }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Window {
  power: SignalPower,
  motion: RelativeMotion,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(transparent, deny_unknown_fields)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this newtype holds a normalized mean square sample amplitude"
  )
)]
struct SignalPower(f64);

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(transparent, deny_unknown_fields)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this newtype holds a dimensionless ratio of two squared sample sums"
  )
)]
struct RelativeMotion(f64);

#[derive(Default)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this newtype holds a normalized decoded sample amplitude"
  )
)]
struct SampleAmplitude(f64);

#[derive(Default)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this newtype sums squared normalized sample amplitudes"
  )
)]
struct SquaredSampleSum(f64);

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(
  unknown_lints,
  reason = "Explicit Rust checks this type before cfg_attr is expanded"
)]
#[expect(
  explicit064_no_unvalidated_input,
  reason = "the waveform cache is decoded by read_json after its 16 MiB file limit"
)]
struct Cached {
  path: String,
  revision: FileRevision,
  audio: Option<Audio>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub enum Evidence {
  Title,
  Waveform,
  IdenticalAudio,
}

impl Evidence {
  pub fn label(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Title => "Similar title · listen to compare",
      Self::Waveform => "Similar waveform · listen to compare",
      Self::IdenticalAudio => "Identical decoded audio",
    })
  }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[allow(
  unknown_lints,
  reason = "Explicit Rust checks this type before cfg_attr is expanded"
)]
#[expect(
  explicit064_no_unvalidated_input,
  reason = "findings are decoded by read_json after its 16 MiB file limit"
)]
pub struct Finding {
  pub tracks: Vec<FoundTrack>,
  pub evidence: Evidence,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[allow(
  unknown_lints,
  reason = "Explicit Rust checks this type before cfg_attr is expanded"
)]
#[expect(
  explicit064_no_unvalidated_input,
  reason = "recording paths enter through the bounded cache or command decoders"
)]
pub struct FoundTrack {
  pub path: String,
  pub revision: Option<FileRevision>,
}

#[derive(Default)]
struct Capture {
  hash: Option<u64>,
  windows: Vec<Window>,
  frames: u64,
  total_micros: u64,
  in_window: u32,
  power: SquaredSampleSum,
  motion: SquaredSampleSum,
  previous: SampleAmplitude,
  blocks: u32,
}

impl Capture {
  fn add(&mut self, block: decoding::Block) -> Result<(), Never> {
    let rate = block.format.rate.max(1);
    let Ok(channels) = index(block.format.channels);
    let target = match rate.checked_div(4) {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 1,
    }
    .max(1);
    let mut hash = match self.hash {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => FNV_START,
    };

    for frame in block.samples.chunks_exact(channels.max(1)) {
      let mut sum = 0.0;

      for sample in frame {
        sum += f64::from(*sample);
        hash = (hash ^ u64::from(sample.to_bits())).wrapping_mul(FNV_STEP);
      }

      let Ok(windows) = console_core_number_conversion::fitted::<_, u32>(self.windows.len());

      match (windows < MAX_WINDOWS).then_some(()) {
        Some(()) => {
          let mono = sum / f64::from(block.format.channels.max(1));
          self.power.0 += mono * mono;
          self.motion.0 += (mono - self.previous.0).powi(2);
          self.previous = SampleAmplitude(mono);
          self.in_window = self.in_window.saturating_add(1);

          match (self.in_window >= target).then_some(()) {
            Some(()) => {
              let count = f64::from(self.in_window);
              self.windows.push(Window {
                power: SignalPower(self.power.0 / count),
                motion: RelativeMotion(self.motion.0 / self.power.0.max(1e-12)),
              });
              self.in_window = 0;
              self.power = SquaredSampleSum::default();
              self.motion = SquaredSampleSum::default();
            }
            None => {}
          }
        }
        None => {}
      }
    }

    self.hash = Some(hash);
    let Ok(samples) = console_core_number_conversion::fixed_index(block.samples.len());
    let frames = match samples.checked_div(u64::from(block.format.channels.max(1))) {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 0,
    };
    self.frames = self.frames.saturating_add(frames);
    self.total_micros = self.total_micros.saturating_add(
      match frames
        .saturating_mul(1_000_000)
        .checked_div(u64::from(rate))
      {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0,
      },
    );
    self.blocks = self.blocks.saturating_add(1);

    Ok(())
  }

  fn finished(self) -> Result<Option<Audio>, Never> {
    Ok(self.hash.filter(|_| self.frames > 0).map(|hash| Audio {
      hash,
      millis: match self.total_micros.checked_div(1_000) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0,
      },
      windows: self.windows,
    }))
  }
}

fn measure(path: &Path) -> Result<Option<Audio>, decoding::Undecoded> {
  let decoding = decoding::opened(path, Seconds(0.0))?;
  let captured = iterate(
    (decoding, Capture::default()),
    |(mut decoding_2, mut state)| {
      let Ok(block) = decoding_2.decoded();

      match (block, state.blocks < MAX_BLOCKS) {
        (None, _) => {
          let Ok(answer) = state.finished();

          Ok(Step::Halt(answer))
        }
        (Some(block_2), true) => {
          let Ok(()) = state.add(block_2);

          Ok(Step::Again((decoding_2, state)))
        }
        (Some(_), false) => Ok(Step::Halt(None)),
      }
    },
  );

  Ok(match captured {
    Ok(audio) => audio,
    Err(_exhausted) => None,
  })
}

pub fn key(path: &Path) -> Result<String, Never> {
  console_core_file_urls::url(path)
}

pub(crate) fn hash_key(key: &str) -> Result<u64, Never> {
  Ok(key.bytes().fold(FNV_START, |sum, byte| {
    (sum ^ u64::from(byte)).wrapping_mul(FNV_STEP)
  }))
}

fn cache_path(cache: &Path, key: &str) -> Result<PathBuf, Never> {
  let Ok(hash) = hash_key(key);

  #[cfg_attr(
    dylint_lib = "explicit086_no_climbing_join",
    expect(
      explicit086_no_climbing_join,
      reason = "the filename is a locally formatted 16-digit hash with a fixed extension"
    )
  )]
  Ok(cache.join("waveforms-v1").join(format!("{hash:016x}.json")))
}

pub fn audio_for(cache: &Path, song: &Song) -> Result<Option<Audio>, Never> {
  let Ok(revision) = song.revision();
  let expected = match revision {
    Some(revision_2) => revision_2,
    None => return Ok(None),
  };
  let Ok(path) = key(&song.path);
  let Ok(at) = cache_path(cache, &path);

  let Ok(stored) = read_json::<Cached>(&at);

  match stored {
    Some(cached) => match (cached.path == path && cached.revision == expected).then_some(()) {
      Some(()) => return Ok(cached.audio),
      None => {}
    },
    None => {}
  }

  let audio = match measure(&song.path) {
    Ok(audio) => audio,
    Err(fault) => {
      console_core_logging::debug!("cannot compare audio in {}: {fault}", song.path.display());

      None
    }
  };
  let Ok(after) = FileRevision::of(&song.path);

  match (after == Some(expected)).then_some(()) {
    Some(()) => {
      let saved = Cached {
        path,
        revision: expected,
        audio,
      };
      let Ok(()) = record(&at, &saved);

      Ok(saved.audio)
    }
    None => Ok(None),
  }
}

pub(crate) fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>, Never> {
  let bytes = match console_core_atomic_writes::read_bytes(path) {
    Ok(bytes) => bytes,
    Err(fault) => {
      match (fault.kind() == ErrorKind::NotFound).then_some(()) {
        Some(()) => {}
        None => console_core_logging::warn!("could not open {}: {fault}", path.display()),
      }

      return Ok(None);
    }
  };

  match serde_json::from_slice(&bytes) {
    Ok(value) => Ok(Some(value)),
    Err(fault) => {
      console_core_logging::warn!("could not read {}: {fault}", path.display());

      Ok(None)
    }
  }
}

pub(crate) fn record<T: Serialize>(at: &Path, data: &T) -> Result<(), Never> {
  match write_json(at, data) {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!("could not save {}: {fault}", at.display()),
  }

  Ok(())
}

pub(crate) fn write_json<T: Serialize>(at: &Path, data: &T) -> io::Result<()> {
  let bytes = serde_json::to_vec(data).map_err(io::Error::other)?;
  console_core_atomic_writes::whole_with_folders(at, &bytes).map_err(io::Error::other)
}

pub(crate) fn remove(at: &Path) -> io::Result<()> {
  match std::fs::remove_file(at) {
    Ok(()) => Ok(()),
    Err(fault) => match (fault.kind() == ErrorKind::NotFound).then_some(()) {
      Some(()) => Ok(()),
      None => Err(fault),
    },
  }
}

pub(crate) fn forget(cache: &Path, removed: &BTreeSet<String>) -> io::Result<()> {
  for path in removed {
    let Ok(at) = cache_path(cache, path);

    let Ok(stored) = read_json::<Cached>(&at);

    match stored {
      Some(saved) => match (saved.path == *path).then_some(()) {
        Some(()) => remove(&at)?,
        None => {}
      },
      None => {}
    }
  }

  let at = cache.join(FINDINGS_FILE);

  let Ok(stored) = read_json::<Vec<Finding>>(&at);

  match stored {
    Some(mut findings) => {
      for finding in &mut findings {
        finding
          .tracks
          .retain(|track| !removed.contains(&track.path));
      }

      findings.retain(|finding| finding.tracks.len() >= 2);
      write_json(&at, &findings)
    }
    None => Ok(()),
  }
}

pub(crate) const FINDINGS_FILE: &str = "findings-v2.json";

pub fn findings() -> Result<Option<Vec<Finding>>, Never> {
  let Ok(cache) = Base::Cache.ours();

  match cache {
    Some(root) => read_json(&root.join("music").join(FINDINGS_FILE)),
    None => Ok(None),
  }
}

pub fn save(cache: &Path, findings: &[Finding]) -> io::Result<()> {
  write_json(&cache.join(FINDINGS_FILE), &findings)
}

fn normalized(said: &str) -> Result<String, Never> {
  Ok(
    said
      .chars()
      .filter(|word| word.is_alphanumeric())
      .flat_map(char::to_lowercase)
      .collect(),
  )
}

fn base_title(raw: &str) -> Result<String, Never> {
  let parts = raw
    .rsplit_once(" (")
    .or_else(|| raw.rsplit_once(" ["))
    .or_else(|| raw.rsplit_once(" - "));
  let base = match parts {
    Some((title, qualifier)) => {
      let suffix = qualifier.to_lowercase();

      match [
        "live",
        "remaster",
        "edit",
        "version",
        "mix",
        "demo",
        "acoustic",
        "instrumental",
      ]
      .iter()
      .any(|mark| suffix.contains(mark))
      .then_some(())
      {
        Some(()) => title,
        None => raw,
      }
    }
    None => raw,
  };

  normalized(base)
}

fn same_waveform(a: &Audio, b: &Audio) -> Result<Option<Evidence>, Never> {
  match (a.hash == b.hash && a.millis.abs_diff(b.millis) <= 5).then_some(()) {
    Some(()) => return Ok(Some(Evidence::IdenticalAudio)),
    None => {}
  }

  match (a.millis.abs_diff(b.millis) <= 1_500).then_some(()) {
    Some(()) => {}
    None => return Ok(None),
  }

  for shift in 0..=4 {
    let Ok(forward) = resembles(a.windows.iter().skip(shift).zip(&b.windows));
    let Ok(back) = resembles(a.windows.iter().zip(b.windows.iter().skip(shift)));

    match (forward, back) {
      (Resemblance::Match, _) | (_, Resemblance::Match) => return Ok(Some(Evidence::Waveform)),
      (Resemblance::Difference, Resemblance::Difference) => {}
    }
  }

  Ok(None)
}

enum Resemblance {
  Match,
  Difference,
}

fn resembles<'a>(
  windows: impl Iterator<Item = (&'a Window, &'a Window)>,
) -> Result<Resemblance, Never> {
  let mut used = 0_u32;
  let mut total = 0.0;
  let mut squares = 0.0;
  let mut shape = 0.0;
  let mut variation = 0.0;
  let mut level_sum = 0.0;

  for (a, b) in windows {
    match (a.power.0 > 1e-8 && b.power.0 > 1e-8).then_some(()) {
      Some(()) => {
        let level = a.power.0.log10();
        let difference = level - b.power.0.log10();
        used = used.saturating_add(1);
        total += difference;
        squares += difference * difference;
        level_sum += level;
        variation += level * level;
        shape += (a.motion.0 - b.motion.0).abs();
      }
      None => {}
    }
  }

  let n = f64::from(used.max(1));

  let matched = used >= 32
    && (variation / n - (level_sum / n).powi(2)) >= 0.025
    && (squares / n - (total / n).powi(2)) < 0.012
    && shape / n < 0.065;

  Ok(match matched.then_some(()) {
    Some(()) => Resemblance::Match,
    None => Resemblance::Difference,
  })
}

pub fn compare(songs: &[Song], audio: &[Option<Audio>]) -> Result<Vec<Finding>, Never> {
  let mut titles: BTreeMap<String, Vec<u64>> = BTreeMap::new();
  let mut lengths: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
  let mut edges: Vec<Vec<(u64, Evidence)>> = vec![Vec::new(); songs.len()];

  for (at, song) in songs.iter().enumerate() {
    let Ok(title) = song.says();
    let Ok(base) = base_title(title);
    let Ok(artist) = normalized(&song.tags.artist);
    let mut similar = BTreeMap::new();
    let Ok(track) = fixed_index(at);

    match base.chars().nth(3) {
      Some(_) => {
        let titled = titles.entry(format!("{artist}/{base}")).or_default();
        similar.extend(titled.iter().map(|other| (*other, Evidence::Title)));
        titled.push(track);
      }
      None => {}
    }

    match audio.get(at).and_then(Option::as_ref) {
      Some(captured) => {
        let second = match captured.millis.checked_div(1_000) {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => 0,
        };
        let nearby = lengths.range(second.saturating_sub(2)..=second.saturating_add(2));
        let Ok(()) = link_waveforms(
          &mut similar,
          audio,
          captured,
          nearby.flat_map(|(_, items)| items),
        );
        lengths.entry(second).or_default().push(track);
      }
      None => {}
    }

    for (other, evidence) in similar {
      match edges.get_mut(at) {
        Some(current) => current.push((other, evidence)),
        None => {}
      }

      let Ok(neighbor) = index(other);

      match edges.get_mut(neighbor) {
        Some(neighbor_2) => neighbor_2.push((track, evidence)),
        None => {}
      }
    }
  }

  groups(songs, &edges)
}

fn link_waveforms<'a>(
  similar: &mut BTreeMap<u64, Evidence>,
  audio: &[Option<Audio>],
  captured: &Audio,
  nearby: impl Iterator<Item = &'a u64>,
) -> Result<(), Never> {
  for other in nearby {
    let Ok(at) = index(*other);
    let prior = match audio.get(at).and_then(Option::as_ref) {
      Some(prior) => prior,
      None => continue,
    };
    let Ok(wave) = same_waveform(captured, prior);

    match wave {
      Some(evidence) => {
        let _replaced = similar.insert(*other, evidence);
      }
      None => {}
    }
  }

  Ok(())
}

fn groups(songs: &[Song], edges: &[Vec<(u64, Evidence)>]) -> Result<Vec<Finding>, Never> {
  let mut seen = BTreeSet::new();
  let mut findings = Vec::new();

  for (at, links) in edges.iter().enumerate() {
    let Ok(track) = fixed_index(at);

    match (!links.is_empty() && seen.insert(track)).then_some(()) {
      Some(()) => {}
      None => continue,
    }

    let start = (
      VecDeque::from([track]),
      Vec::new(),
      Evidence::IdenticalAudio,
      seen,
    );
    let grouped = iterate(
      start,
      |(mut waiting, mut tracks, mut evidence, mut seen_2)| {
        let next = match waiting.pop_front() {
          Some(item) => item,
          None => return Ok(Step::Halt((Finding { tracks, evidence }, seen_2))),
        };
        let Ok(item) = index(next);

        match songs.get(item) {
          Some(song) => {
            let Ok(path) = key(&song.path);
            let Ok(revision) = song.revision();
            tracks.push(FoundTrack { path, revision });
          }
          None => {}
        }

        for (other, why) in edges.get(item).into_iter().flatten() {
          evidence = evidence.min(*why);

          match seen_2.insert(*other).then_some(()) {
            Some(()) => waiting.push_back(*other),
            None => {}
          }
        }

        Ok(Step::Again((waiting, tracks, evidence, seen_2)))
      },
    );

    match grouped {
      Ok((finding, visited)) => {
        findings.push(finding);
        seen = visited;
      }
      Err(_exhausted) => return Ok(findings),
    }
  }

  Ok(findings)
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_music_player::tags::Tags;

  struct Title<'a>(&'a str);

  fn song(name: &str, title: Title<'_>) -> Result<Song, Never> {
    let path = PathBuf::from(format!("/music/{name}.flac"));
    let Ok(mut song) = Song::of(&path);
    song.tags = Tags {
      title: title.0.to_owned(),
      artist: "Performer".to_owned(),
      ..Tags::default()
    };

    Ok(song)
  }

  #[test]
  fn renamed_copies_and_potential_recording_versions_are_reviewed() {
    let Ok(one) = song("track", Title("Example"));
    let Ok(two) = song("renamed", Title("Another Name"));
    let Ok(three) = song("live", Title("Example (Live)"));
    let audio = Audio {
      hash: 78,
      millis: 10_000,
      windows: Vec::new(),
    };
    let files = [one, two, three];
    let Ok(found) = compare(&files, &[Some(audio.clone()), Some(audio), None]);

    match found.as_slice() {
      [group] => {
        assert_eq!(group.tracks.len(), 3);
        assert_eq!(group.evidence, Evidence::Title);
      }
      _ => assert_eq!(found.len(), 1),
    }

    let audio_2 = Audio {
      hash: 82,
      millis: 10_000,
      windows: Vec::new(),
    };
    let exact = [files.first(), files.get(1)]
      .into_iter()
      .flatten()
      .cloned()
      .collect::<Vec<_>>();
    let Ok(only_copies) = compare(&exact, &[Some(audio_2.clone()), Some(audio_2)]);
    let evidence = only_copies.first().map(|group| group.evidence);
    assert_eq!(evidence, Some(Evidence::IdenticalAudio));
  }
}
