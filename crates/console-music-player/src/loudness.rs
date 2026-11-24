//! Read a file's loudness before starting its audio stream.

use std::fs::Metadata;
use std::io::{self, ErrorKind};
use std::os::fd::OwnedFd;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use console_core_external_programs::Program;
use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_core_places::Base;
use console_operating_system_linux::program_lifetime::{TimedReadPipe, alongside};

const TARGET_LUFS: f64 = -36.4;
const CEILING_DBTP: f64 = -22.0;
const MAX_BOOST_DB: f64 = 27.6;
const MAX_REPORT: u64 = 64 * 1024;

pub enum Analysis {
  Indexed(f64),
  Unavailable,
  Interrupted,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Measurement {
  lufs: Lufs,
  peak_dbtp: TruePeakDb,
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
  fn gain(self) -> Result<f64, Never> {
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

pub(crate) fn before_playback(
  song: &Path,
  wanted: impl Fn() -> Request,
) -> Result<Analysis, Never> {
  let Ok(cache) = Base::Cache.ours();
  let cache_2 = cache.map(|directory| directory.join("audio-loudness-v1"));

  Ok(match indexed(song, cache_2.as_deref(), wanted) {
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

struct CacheFilename(String);

impl From<&Metadata> for CacheFilename {
  fn from(file: &Metadata) -> Self {
    Self(format!("{}-{}", file.dev(), file.ino()))
  }
}

impl AsRef<Path> for CacheFilename {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

fn indexed(
  song: &Path,
  cache: Option<&Path>,
  wanted: impl Fn() -> Request,
) -> io::Result<Option<Measurement>> {
  let song_2 = song.canonicalize()?;
  let before = song_2.metadata()?;

  match before.is_file().then_some(()) {
    Some(()) => {}
    None => return Ok(None),
  }

  let Ok(stamp) = revision(&before);
  let entry = cache.map(|directory| directory.join(CacheFilename::from(&before)));

  match entry.as_deref() {
    Some(path) => {
      let Ok(saved) = cached_file(path, &stamp);

      match saved {
        Some(measured) => return Ok(Some(measured)),
        None => {}
      }
    }
    None => {}
  }

  let measured = scan(&song_2, wanted)?;
  let after = song_2.metadata()?;
  let Ok(updated) = revision(&after);

  match ((before.dev(), before.ino(), &stamp) == (after.dev(), after.ino(), &updated))
    .then_some(())
  {
    Some(()) => {}
    None => return Ok(None),
  }

  match entry {
    Some(path) => {
      let record = format!(
        "{}\n{} {}\n",
        stamp.0, measured.lufs.0, measured.peak_dbtp.0
      );

      match console_core_atomic_writes::whole_with_folders(&path, record.as_bytes()) {
        Ok(()) => {}
        Err(fault) => {
          console_core_logging::warn!("could not save the audio loudness index: {fault}");
        }
      }
    }
    None => {}
  }

  Ok(Some(measured))
}

fn cached_file(path: &Path, stamp: &Revision) -> Result<Option<Measurement>, Never> {
  let bytes = match console_core_atomic_writes::read_prefix(path, 512) {
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
  let mut words = levels.split_whitespace();
  let (loudness, peak) = match (words.next(), words.next()) {
    (Some(loudness), Some(peak)) => (loudness, peak),
    _ => return Ok(None),
  };
  let (lufs, peak_dbtp) = match (loudness.parse(), peak.parse()) {
    (Ok(lufs), Ok(peak_dbtp)) => (lufs, peak_dbtp),
    _ => return Ok(None),
  };

  Measurement {
    lufs: Lufs(lufs),
    peak_dbtp: TruePeakDb(peak_dbtp),
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

fn scan(song: &Path, wanted: impl Fn() -> Request) -> io::Result<Measurement> {
  match wanted() {
    Request::Continue => {}
    Request::Cancel => return Err(io::Error::from(ErrorKind::Interrupted)),
  }

  let Ok(mut asking) = Program::Ffmpeg.command();
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
      "ebur128=peak=true:framelog=verbose",
      "-f",
      "null",
      "-",
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::piped());
  let mut measuring = alongside(&mut asking)?;
  let Ok(stderr) = measuring.take_stderr();
  let stderr_2 = stderr.ok_or_else(|| io::Error::other("missing loudness measurement pipe"))?;
  let pipe = TimedReadPipe::new(OwnedFd::from(stderr_2))?;
  let report = read_report(&pipe, wanted)?;
  let status = measuring.wait()?;

  match status.success().then_some(()) {
    Some(()) => parse_report(&report),
    None => Err(io::Error::other("audio analysis did not finish")),
  }
}

fn read_report(pipe: &TimedReadPipe, wanted: impl Fn() -> Request) -> io::Result<Vec<u8>> {
  let reading = iterate((Vec::new(), [0; 4096]), |(mut report, mut buffer)| {
    match wanted() {
      Request::Continue => {}
      Request::Cancel => return Ok(Step::Halt(Err(io::Error::from(ErrorKind::Interrupted)))),
    }

    Ok(
      match pipe.read_some(&mut buffer, Duration::from_millis(100)) {
        Ok(0) => Step::Halt(Ok(report)),
        Ok(count) => {
          let Ok(already_read) = console_core_number_conversion::fixed_index(report.len());
          let bounded = already_read.saturating_add(u64::from(count)) <= MAX_REPORT;

          match bounded.then_some(()) {
            Some(()) => {}
            None => {
              return Ok(Step::Halt(Err(io::Error::other(
                "audio analysis report exceeded 64 KiB",
              ))));
            }
          }

          let Ok(end) = console_core_number_conversion::index(count);

          match buffer.get(..end) {
            Some(bytes) => {
              report.extend_from_slice(bytes);

              Step::Again((report, buffer))
            }
            None => Step::Halt(Err(io::Error::other(
              "audio analysis pipe returned an invalid byte count",
            ))),
          }
        }
        Err(fault) => match matches!(
          fault.kind(),
          ErrorKind::Interrupted | ErrorKind::WouldBlock | ErrorKind::TimedOut
        )
        .then_some(())
        {
          Some(()) => Step::Again((report, buffer)),
          None => Step::Halt(Err(fault)),
        },
      },
    )
  })
  .map_err(io::Error::other)?;

  reading
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_cached_measurement_is_used_for_the_same_file_revision() {
    let record = "12 34 56 78 9\n-30.4 -15.9\n";
    assert_eq!(
      cached(record, &Revision(String::from("12 34 56 78 9"))),
      Ok(Some(Measurement {
        lufs: Lufs(-30.4),
        peak_dbtp: TruePeakDb(-15.9)
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
        peak_dbtp: TruePeakDb(f64::NEG_INFINITY)
      }
      .gain(),
      Ok(1.0)
    );
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
    let indexed_bass = indexed(&song, Some(&cache), || Request::Continue)?;
    let bass = indexed_bass.ok_or("missing bass measurement")?;
    let cached = indexed(&song, Some(&cache), || Request::Cancel)?;
    assert_eq!(
      cached,
      Some(bass),
      "opening the same file should reuse its measurement"
    );

    write_tone(2_000.0)?;
    let stopped = indexed(&song, Some(&cache), || Request::Cancel);
    assert_eq!(
      stopped.map_err(|fault| fault.kind()),
      Err(ErrorKind::Interrupted)
    );
    let indexed_treble = indexed(&song, Some(&cache), || Request::Continue)?;
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
