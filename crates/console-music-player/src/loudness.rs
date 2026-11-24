//! Read a file's loudness before starting its audio stream.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::Metadata;
use std::io::{self, ErrorKind};
use std::os::fd::OwnedFd;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_core_places::Base;
use console_operating_system_linux::program_lifetime::{TimedReadPipe, alongside};

mod index;

const TARGET_LUFS: f64 = -36.4;
const CEILING_DBTP: f64 = -22.0;
const MAX_BOOST_DB: f64 = 27.6;
const MAX_CACHE_RECORD: u32 = 24_576;
const MAX_REPORT: u64 = 64 * 1024;
const MAX_MOMENT: i32 = 300;
const MIN_MOMENT: i32 = -700;

pub enum Analysis {
  Indexed(f64),
  Unavailable,
  Interrupted,
}

#[derive(Clone, Debug, PartialEq)]
struct Measurement {
  lufs: Lufs,
  peak_dbtp: TruePeakDb,
  moments: BTreeMap<i32, u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "LUFS is a logarithmic level relative to full scale"
  )
)]
struct Lufs(f64);

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "true peak is measured in dBTP"
  )
)]
struct TruePeakDb(f64);

impl Measurement {
  fn gain(&self) -> Result<f64, Never> {
    let correction = match (self.lufs.0 <= -70.0).then_some(()) {
      Some(()) => 0.0,
      None => TARGET_LUFS - self.lufs.0,
    };
    let decibels = correction
      .min(CEILING_DBTP - self.peak_dbtp.0)
      .min(MAX_BOOST_DB);
    Ok(10.0_f64.powf(decibels / 20.0))
  }

  fn valid(self) -> Result<Option<Self>, Never> {
    Ok(
      (self.lufs.0.is_finite()
        && (self.peak_dbtp.0.is_finite()
          || <::core::primitive::f64 as ::core::cmp::PartialOrd>::partial_cmp(
            &self.peak_dbtp.0,
            &f64::NEG_INFINITY,
          ) == ::core::option::Option::Some(::core::cmp::Ordering::Equal)))
      .then_some(self),
    )
  }
}

pub(crate) enum Request {
  Continue,
  Cancel,
}

fn directory() -> Result<Option<std::path::PathBuf>, Never> {
  let Ok(root) = Base::Cache.ours();

  Ok(root.map(|at| at.join("audio-loudness-v2")))
}

pub struct BackgroundScan {
  _guard: std::fs::File,
}

impl BackgroundScan {
  pub fn start() -> io::Result<Self> {
    let Ok(configured) = directory();
    let cache =
      configured.ok_or_else(|| io::Error::other("audio analysis cache is unavailable"))?;
    std::fs::create_dir_all(&cache)?;
    let guard = console_core_atomic_writes::open_lock_file(&cache.join("scan.lock"))?;
    guard.try_lock().map_err(io::Error::from)?;

    Ok(Self { _guard: guard })
  }
}

fn scan_running(cache: &Path) -> Result<ScanActivity, Never> {
  let file = match std::fs::File::open(cache.join("scan.lock")) {
    Ok(file) => file,
    Err(fault) => match (fault.kind() == ErrorKind::NotFound).then_some(()) {
      Some(()) => return Ok(ScanActivity::Idle),
      None => {
        console_core_logging::debug!("could not check background audio analysis: {fault}");

        return Ok(ScanActivity::Idle);
      }
    },
  };

  Ok(match file.try_lock_shared() {
    Ok(()) => ScanActivity::Idle,
    Err(std::fs::TryLockError::WouldBlock) => ScanActivity::Running,
    Err(std::fs::TryLockError::Error(fault)) => {
      console_core_logging::debug!("could not check background audio analysis: {fault}");

      ScanActivity::Idle
    }
  })
}

enum ScanActivity {
  Idle,
  Running,
}

pub fn index_in_background(song: &Path) -> io::Result<()> {
  let Ok(cache) = directory();
  let _measured = index::read(song, cache.as_deref(), index::Work::Measure, || {
    Request::Continue
  })?;

  Ok(())
}

pub fn prune_removed() -> io::Result<()> {
  let Ok(cache) = directory();

  match cache {
    Some(root) => index::prune_removed(&root),
    None => Ok(()),
  }
}

pub fn prune_old_revisions(
  removed: &BTreeSet<FileId>,
  retained: &BTreeSet<FileId>,
) -> io::Result<()> {
  let Ok(cache) = directory();

  match cache {
    Some(root) => index::prune_old_revisions(&root, removed, retained),
    None => Ok(()),
  }
}

pub(crate) fn before_playback(
  song: &Path,
  wanted: impl Fn() -> Request,
) -> Result<Analysis, Never> {
  let Ok(cache) = directory();
  let measured = match cache.as_deref() {
    Some(root) => {
      let Ok(activity) = scan_running(root);

      match activity {
        ScanActivity::Idle => index::read(song, Some(root), index::Work::Measure, wanted),
        ScanActivity::Running => index::read(song, Some(root), index::Work::Cached, wanted),
      }
    }
    None => index::read(song, None, index::Work::Measure, wanted),
  };

  Ok(match measured {
    Ok(Some(measurement)) => {
      let Ok(gain) = measurement.gain();
      Analysis::Indexed(gain)
    }
    Ok(None) => Analysis::Unavailable,
    Err(fault) => match (fault.kind() == ErrorKind::Interrupted).then_some(()) {
      Some(()) => Analysis::Interrupted,
      None => {
        console_core_logging::warn!("could not index loudness for {}: {fault}", song.display());
        Analysis::Unavailable
      }
    },
  })
}

#[derive(PartialEq, Eq)]
struct Revision(String);

fn revision(file: &Metadata) -> Result<Revision, Never> {
  Ok(Revision(format!(
    "{} {} {} {} {}",
    file.len(),
    file.mtime(),
    file.mtime_nsec(),
    file.ctime(),
    file.ctime_nsec()
  )))
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct FileId {
  pub device: u64,
  pub inode: u64,
}

impl From<&Metadata> for FileId {
  fn from(file: &Metadata) -> Self {
    Self {
      device: file.dev(),
      inode: file.ino(),
    }
  }
}

impl FileId {
  fn name(self) -> Result<InodeCacheName, Never> {
    Ok(InodeCacheName(format!("{}-{}", self.device, self.inode)))
  }
}

struct InodeCacheName(String);

impl AsRef<Path> for InodeCacheName {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

fn cached_file(path: &Path, stamp: &Revision) -> Result<Option<Measurement>, Never> {
  let bytes = match console_core_atomic_writes::read_prefix(path, MAX_CACHE_RECORD) {
    Ok(bytes) => bytes,
    Err(_cache_unavailable) => return Ok(None),
  };

  match std::str::from_utf8(&bytes) {
    Ok(record) => cached(record, stamp),
    Err(_invalid_cache) => Ok(None),
  }
}

fn cached(record: &str, stamp: &Revision) -> Result<Option<Measurement>, Never> {
  let levels = match record
    .strip_prefix(&stamp.0)
    .and_then(|rest| rest.strip_prefix('\n'))
  {
    Some(levels) => levels,
    None => return Ok(None),
  };
  let mut lines = levels.lines();
  let first = match lines.next() {
    Some(first) => first,
    None => return Ok(None),
  };
  let mut words = first.split_whitespace();
  let (loudness, peak) = match (words.next(), words.next()) {
    (Some(loudness), Some(peak)) => (loudness, peak),
    _ => return Ok(None),
  };
  let (lufs, peak_dbtp) = match (loudness.parse(), peak.parse()) {
    (Ok(lufs), Ok(peak_dbtp)) => (lufs, peak_dbtp),
    _ => return Ok(None),
  };

  let mut moments = BTreeMap::new();

  for line in lines {
    let (bucket, samples) = match line.split_once(' ') {
      Some(fields) => fields,
      None => return Ok(None),
    };

    match (bucket.parse::<i32>(), samples.parse::<u32>()) {
      (Ok(bin @ MIN_MOMENT..=MAX_MOMENT), Ok(count)) => {
        let _previous = moments.insert(bin, count);
      }
      (Ok(_out_of_range), Ok(_)) => return Ok(None),
      (Err(_invalid_bucket), _) => return Ok(None),
      (Ok(_), Err(_invalid_count)) => return Ok(None),
    }
  }

  Measurement {
    lufs: Lufs(lufs),
    peak_dbtp: TruePeakDb(peak_dbtp),
    moments,
  }
  .valid()
}

fn reading(summary: &str) -> Result<Option<Measurement>, Never> {
  let value = |label: &str| {
    let find_map = summary
      .lines()
      .find_map(|line| line.trim_start().strip_prefix(label))?;
    let next = find_map.split_whitespace().next()?;

    match next.parse() {
      ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
      ::core::result::Result::Err(_invalid_float) => ::core::option::Option::None,
    }
  };
  let (lufs, peak_dbtp) = match (value("I:"), value("Peak:")) {
    (Some(lufs), Some(peak)) => (lufs, peak),
    _ => return Ok(None),
  };

  Measurement {
    lufs: Lufs(lufs),
    peak_dbtp: TruePeakDb(peak_dbtp),
    moments: BTreeMap::new(),
  }
  .valid()
}

fn parse_report(report: &[u8]) -> io::Result<Measurement> {
  const SUMMARY: &[u8] = b"] Summary:";
  let missing = || io::Error::other("missing loudness and true-peak values");
  let tail = report
    .array_windows::<{ SUMMARY.len() }>()
    .rposition(|bytes| bytes == SUMMARY)
    .and_then(|at| report.get(at.saturating_add(SUMMARY.len())..))
    .ok_or_else(missing)?;
  let summary = std::str::from_utf8(tail).map_err(io::Error::other)?;
  let Ok(measurement) = reading(summary);

  measurement.ok_or_else(missing)
}

fn scan(
  asking: &mut Command,
  song: &Path,
  wanted: impl Fn() -> Request,
) -> io::Result<Measurement> {
  match wanted() {
    Request::Continue => {}
    Request::Cancel => return Err(io::Error::from(ErrorKind::Interrupted)),
  }

  asking
    .env("LC_ALL", "C")
    .args([
      "-hide_banner",
      "-nostats",
      "-nostdin",
      "-loglevel",
      "info",
      "-protocol_whitelist",
      "file",
      "-i",
    ])
    .arg(song)
    .args([
      "-map",
      "0:a:0",
      "-af",
      "ebur128=peak=true",
      "-f",
      "null",
      "-",
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::piped());
  let mut measuring = alongside(asking)?;
  let Ok(stderr) = measuring.take_stderr();
  let stderr_2 = stderr.ok_or_else(|| io::Error::other("missing loudness measurement pipe"))?;
  let pipe = TimedReadPipe::new(OwnedFd::from(stderr_2))?;
  let report = read_report(&pipe, wanted)?;
  let status = measuring.wait()?;

  match status.success().then_some(()) {
    Some(()) => {
      let mut measurement = parse_report(&report.summary)?;
      measurement.moments = report.moments;

      Ok(measurement)
    }
    None => Err(io::Error::other("audio analysis did not finish")),
  }
}

#[derive(Default)]
struct Report {
  summary: Vec<u8>,
  line: Vec<u8>,
  moments: BTreeMap<i32, u32>,
}

impl Report {
  fn take(&mut self, bytes: &[u8]) -> io::Result<()> {
    for piece in bytes.split_inclusive(|byte| *byte == b'\n') {
      let Ok(ongoing) = console_core_number_conversion::fixed_index(self.line.len());
      let Ok(more) = console_core_number_conversion::fixed_index(piece.len());

      match (ongoing.saturating_add(more) <= MAX_REPORT).then_some(()) {
        Some(()) => self.line.extend_from_slice(piece),
        None => return Err(io::Error::other("audio analysis line exceeded 64 KiB")),
      }

      match (piece.last() == Some(&b'\n')).then_some(()) {
        Some(()) => self.complete()?,
        None => {}
      }
    }

    Ok(())
  }

  fn complete(&mut self) -> io::Result<()> {
    let frame = self.line.starts_with(b"[Parsed_ebur128_");
    let moment = match (frame.then_some(()), std::str::from_utf8(&self.line)) {
      (Some(()), Ok(line)) => line.split_once(" M:"),
      (Some(()), Err(_non_utf8_diagnostic)) => None,
      (None, _) => None,
    };

    match moment {
      Some((_, after)) => {
        let level = match after.split_whitespace().next().map(str::parse::<f64>) {
          Some(Ok(heard)) => Some(heard),
          Some(Err(_invalid_momentary_level)) => None,
          None => None,
        };

        match level.filter(|said| (-70.0..=30.0).contains(said)) {
          Some(said) => {
            let Ok(bin) = console_core_number_conversion::toward_zero_i32((said * 10.0).round());
            let count = self.moments.entry(bin).or_default();
            *count = count.saturating_add(1);
          }
          None => {}
        }
      }
      None => {
        let Ok(existing) = console_core_number_conversion::fixed_index(self.summary.len());
        let Ok(extra) = console_core_number_conversion::fixed_index(self.line.len());

        match (existing.saturating_add(extra) <= MAX_REPORT).then_some(()) {
          Some(()) => self.summary.extend_from_slice(&self.line),
          None => return Err(io::Error::other("audio analysis summary exceeded 64 KiB")),
        }
      }
    }

    self.line.clear();
    Ok(())
  }
}

fn read_report(pipe: &TimedReadPipe, wanted: impl Fn() -> Request) -> io::Result<Report> {
  let mut report = read_pipe(pipe, wanted, Report::default(), Report::take)?;
  report.complete()?;

  Ok(report)
}

fn read_pipe<State>(
  pipe: &TimedReadPipe,
  wanted: impl Fn() -> Request,
  initial: State,
  take: impl Fn(&mut State, &[u8]) -> io::Result<()>,
) -> io::Result<State> {
  let reading = iterate((initial, [0; 4096]), |(mut state, mut buffer)| {
    match wanted() {
      Request::Continue => {}
      Request::Cancel => return Ok(Step::Halt(Err(io::Error::from(ErrorKind::Interrupted)))),
    }

    Ok(
      match pipe.read_some(&mut buffer, Duration::from_millis(100)) {
        Ok(0) => Step::Halt(Ok(state)),
        Ok(count) => {
          let Ok(end) = console_core_number_conversion::index(count);
          let parsed = match buffer.get(..end) {
            Some(bytes) => take(&mut state, bytes),
            None => Err(io::Error::other(
              "audio analysis pipe returned an invalid byte count",
            )),
          };

          match parsed {
            Ok(()) => Step::Again((state, buffer)),
            Err(fault) => Step::Halt(Err(fault)),
          }
        }
        Err(fault) => match matches!(
          fault.kind(),
          ErrorKind::Interrupted | ErrorKind::WouldBlock | ErrorKind::TimedOut
        )
        .then_some(())
        {
          Some(()) => Step::Again((state, buffer)),
          None => Step::Halt(Err(fault)),
        },
      },
    )
  })
  .map_err(io::Error::other)?;

  reading
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "album gain is a dimensionless amplitude multiplier"
  )
)]
pub struct AlbumGain(pub(crate) f64);

pub fn album_gain(tracks: &[std::path::PathBuf]) -> Result<Option<AlbumGain>, Never> {
  let Ok(configured) = directory();
  let cache = match configured {
    Some(root) => root,
    None => return Ok(None),
  };
  let mut moments = BTreeMap::<i32, u32>::new();
  let mut peak = f64::NEG_INFINITY;

  for track in tracks {
    let measured = match index::read(track, Some(&cache), index::Work::Reuse, || {
      Request::Continue
    }) {
      Ok(Some(measured)) => measured,
      Ok(None) => return Ok(None),
      Err(fault) => {
        console_core_logging::debug!(
          "could not look up album loudness for {}: {fault}",
          track.display()
        );
        return Ok(None);
      }
    };
    peak = peak.max(measured.peak_dbtp.0);

    for (bin, count) in measured.moments {
      let total = moments.entry(bin).or_default();
      *total = total.saturating_add(count);
    }
  }

  let Ok(lufs) = gated(&moments);
  let measurement = Measurement {
    lufs,
    peak_dbtp: TruePeakDb(peak),
    moments,
  };
  let Ok(gain) = measurement.gain();

  Ok(Some(AlbumGain(gain)))
}

fn gated(moments: &BTreeMap<i32, u32>) -> Result<Lufs, Never> {
  let mean = |floor: f64| {
    let (power, count) = moments
      .iter()
      .filter_map(|(bin, count)| {
        let lufs = f64::from(*bin) / 10.0;

        (lufs >= floor).then_some((10.0_f64.powf(lufs / 10.0), f64::from(*count)))
      })
      .fold((0.0, 0.0), |(power, count), (energy, many)| {
        (power + energy * many, count + many)
      });

    10.0 * (power / count.max(1.0)).log10()
  };

  Ok(Lufs(mean((mean(-70.0) - 10.0).max(-70.0)).max(-70.0)))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn album_loudness_uses_combined_energy_and_relative_gate() {
    let histogram = BTreeMap::from([(-650, 4), (-320, 10), (-300, 40)]);
    let Ok(Lufs(level)) = gated(&histogram);
    let expected =
      10.0 * ((10.0_f64.powf(-3.2) * 10.0 + 10.0_f64.powf(-3.0) * 40.0) / 50.0).log10();
    assert!((level - expected).abs() < 1e-9);
  }

  #[test]
  fn a_cached_measurement_is_used_for_the_same_file_revision() {
    let record = "12 34 56 78 9\n-30.4 -15.9\n";
    assert_eq!(
      cached(record, &Revision(String::from("12 34 56 78 9"))),
      Ok(Some(Measurement {
        lufs: Lufs(-30.4),
        peak_dbtp: TruePeakDb(-15.9),
        moments: BTreeMap::new(),
      }))
    );
    assert_eq!(
      cached(record, &Revision(String::from("12 34 56 78 10"))),
      Ok(None)
    );
  }

  #[test]
  fn true_peaks_bound_the_gain_and_silence_stays_at_unity() -> io::Result<()> {
    let report = b"name-\xff.wav\n[Parsed_ebur128_0 @ 0x0] Summary:\n  Integrated loudness:\n    I: -60.0 LUFS\n  True peak:\n    Peak: -20.0 dBFS\n";
    let measured = parse_report(report)?;
    let Ok(gain) = measured.gain();
    assert!((gain - 10.0_f64.powf(-2.0 / 20.0)).abs() < 1e-10);
    assert_eq!(
      Measurement {
        lufs: Lufs(-70.0),
        peak_dbtp: TruePeakDb(f64::NEG_INFINITY),
        moments: BTreeMap::new(),
      }
      .gain(),
      Ok(1.0)
    );
    Ok(())
  }

  #[test]
  #[ignore = "uses the desktop's FFmpeg analyser"]
  fn retagging_or_moving_audio_reuses_analysis_but_replacing_audio_does_not() -> io::Result<()> {
    let directory = console_core_temporary_directories::private("retag-measurement")
      .map_err(io::Error::other)?;
    let cache = directory.join("index");
    let song = directory.join("song.opus");
    let retagged = directory.join("retagged.opus");
    let original = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/a-tone.opus");
    std::fs::copy(original, &song)?;

    let index = |path: &Path, work| index::read(path, Some(&cache), work, || Request::Continue);
    let measured = index(&song, index::Work::Measure)?;
    assert!(measured.is_some());
    let Ok(mut command) = console_core_external_programs::Program::Ffmpeg.command();
    let status = command
      .args(["-nostdin", "-v", "error", "-i"])
      .arg(&song)
      .args([
        "-map",
        "0:a:0",
        "-c",
        "copy",
        "-metadata",
        "title=New title",
      ])
      .arg(&retagged)
      .status()?;
    assert!(status.success());
    std::fs::rename(&retagged, &song)?;
    let first_lookup = index(&song, index::Work::Cached)?;
    assert_eq!(first_lookup, None);
    let retagged_measurement = index(&song, index::Work::Reuse)?;
    assert_eq!(retagged_measurement, measured);
    let revision_lookup = index(&song, index::Work::Cached)?;
    assert_eq!(revision_lookup, measured);
    std::fs::rename(&song, &retagged)?;
    let moved_measurement = index(&retagged, index::Work::Reuse)?;
    assert_eq!(moved_measurement, measured);

    let Ok(mut reencode) = console_core_external_programs::Program::Ffmpeg.command();
    let reencoded = reencode
      .args(["-nostdin", "-v", "error", "-i"])
      .arg(&retagged)
      .args(["-af", "volume=0.25", "-c:a", "libopus"])
      .arg(&song)
      .status()?;
    assert!(reencoded.success());
    let changed_measurement = index(&song, index::Work::Reuse)?;
    assert_eq!(changed_measurement, None);
    std::fs::remove_dir_all(directory)?;

    Ok(())
  }

  #[test]
  #[ignore = "uses the desktop's FFmpeg analyser"]
  fn removal_keeps_shared_analysis_until_the_last_copy_is_gone() -> io::Result<()> {
    let directory =
      console_core_temporary_directories::private("removed-audio").map_err(io::Error::other)?;
    let cache = directory.join("index");
    let first = directory.join("first.opus");
    let second = directory.join("second.opus");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/a-tone.opus");
    std::fs::copy(&fixture, &first)?;
    std::fs::copy(&fixture, &second)?;

    let measured = index::read(&first, Some(&cache), index::Work::Measure, || {
      Request::Continue
    })?;
    assert!(measured.is_some());
    let copy = index::read(&second, Some(&cache), index::Work::Reuse, || {
      Request::Continue
    })?;
    assert_eq!(copy, measured);

    std::fs::remove_file(&first)?;
    index::prune_removed(&cache)?;
    std::fs::copy(&fixture, &first)?;
    let reused = index::read(&first, Some(&cache), index::Work::Reuse, || {
      Request::Continue
    })?;
    assert_eq!(reused, measured);

    std::fs::remove_file(&first)?;
    std::fs::remove_file(&second)?;
    index::prune_removed(&cache)?;
    std::fs::copy(&fixture, &first)?;
    let forgotten = index::read(&first, Some(&cache), index::Work::Reuse, || {
      Request::Continue
    })?;
    assert_eq!(forgotten, None);
    std::fs::remove_dir_all(directory)?;

    Ok(())
  }

  #[test]
  #[ignore = "uses the desktop's FFmpeg analyser"]
  fn equal_pcm_levels_can_index_at_different_loudness() -> Result<(), Box<dyn std::error::Error>>
  {
    let directory = console_core_temporary_directories::fresh("indexed-audio")?;
    let cache = directory.join("index");
    let song = directory.join("test.wav");
    let write_tone = |frequency: f64| -> Result<(), Box<dyn std::error::Error>> {
      let samples: Vec<_> = (0..16_000)
        .map(|frame| {
          let radians = f64::from(frame) * frequency * std::f64::consts::TAU / 8_000.0;
          let Ok(sample) =
            console_core_number_conversion::toward_zero_i32(8192.0 * radians.sin());
          let Ok(sample_2) = console_core_number_conversion::fitted::<_, i16>(sample);
          sample_2
        })
        .collect();
      let Ok(wave) = crate::tags::tests::a_wave(&samples);
      console_core_atomic_writes::whole_with_folders(&song, &wave)?;
      Ok(())
    };

    write_tone(125.0)?;
    let indexed =
      |wanted: fn() -> Request| index::read(&song, Some(&cache), index::Work::Measure, wanted);
    let indexed_bass = indexed(|| Request::Continue)?;
    let bass = indexed_bass.ok_or("missing bass measurement")?;
    assert!(!bass.moments.is_empty());
    let cached = indexed(|| Request::Cancel)?;
    assert_eq!(
      cached.as_ref(),
      Some(&bass),
      "opening the same file should reuse its measurement"
    );

    write_tone(2_000.0)?;
    let stopped = indexed(|| Request::Cancel);
    assert_eq!(
      stopped.map_err(|fault| fault.kind()),
      Err(ErrorKind::Interrupted)
    );
    let indexed_treble = indexed(|| Request::Continue)?;
    let treble = indexed_treble.ok_or("missing treble measurement")?;
    assert!((bass.peak_dbtp.0 - treble.peak_dbtp.0).abs() < 1.0);
    assert!(
      treble.lufs.0 > bass.lufs.0 + 2.0,
      "bass {bass:?}, treble {treble:?}"
    );
    eprintln!(
      "Equal-amplitude tones measured {:.1} LUFS at 125 Hz and {:.1} LUFS at 2 kHz",
      bass.lufs.0, treble.lufs.0
    );
    Ok(())
  }
}
