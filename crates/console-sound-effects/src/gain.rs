//! Read the gain applied by the playback graph before the listener's volume.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_waiting::clock::{Clock, LiveClock};

const DIRECTORY: &str = "console-playback-gain";
const PLUGIN: &str = "console-playback-gain.so";
const PAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/console-playback-gain.so"));
const INTERVAL: Duration = Duration::from_millis(250);
const EXPIRES: Duration = Duration::from_millis(750);

fn directory() -> io::Result<PathBuf> {
  let Ok(configured) = console_core_places::runtime();
  let runtime = configured.ok_or_else(|| io::Error::from(ErrorKind::NotFound))?;

  Ok(runtime.join(DIRECTORY))
}

pub fn prepare() -> io::Result<()> {
  let directory = directory()?;
  let plugin = directory.join(PLUGIN);
  console_core_atomic_writes::whole_with_folders(&plugin, PAGE).map_err(io::Error::other)
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Sample {
  sequence: u32,
  tenths_db: i32,
}

impl Sample {
  fn read(bytes: &[u8]) -> Result<Option<Self>, Never> {
    let [a, b, c, d, e, f, g, h] = match <[u8; 8]>::try_from(bytes) {
      Ok(record) => record,
      Err(_incomplete) => return Ok(None),
    };
    let sequence = u32::from_le_bytes([e, f, g, h]);
    let multiplier = f32::from_le_bytes([a, b, c, d]);

    match (sequence != 0 && multiplier.is_finite() && multiplier > 0.0).then_some(()) {
      Some(()) => {
        let decibels = 200.0 * f64::from(multiplier).log10();
        let Ok(tenths_db) = console_core_number_conversion::toward_zero_i32(decibels.round());

        Ok(Some(Self {
          sequence,
          tenths_db,
        }))
      }
      None => Ok(None),
    }
  }
}

struct Observation {
  node: u32,
  file: OsString,
  sample: Sample,
}

enum Meter {
  WaitingForChange(Sample),
  Live { sample: Sample, changed: Duration },
}

impl Meter {
  fn observe(self, sample: Sample, now: Duration) -> Result<Self, Never> {
    let before = match &self {
      Self::WaitingForChange(before) | Self::Live { sample: before, .. } => before,
    };

    Ok(match (before.sequence != sample.sequence).then_some(()) {
      Some(()) => Self::Live {
        sample,
        changed: now,
      },
      None => self,
    })
  }

  fn at(&self, now: Duration) -> Result<Option<(Duration, i32)>, Never> {
    Ok(match *self {
      Self::WaitingForChange(_) => None,
      Self::Live { sample, changed } => {
        (now.saturating_sub(changed) < EXPIRES).then_some((changed, sample.tenths_db))
      }
    })
  }
}

#[derive(Default)]
struct Display {
  meters: BTreeMap<OsString, Meter>,
  shown: Option<BTreeMap<u32, i32>>,
}

enum Effect {
  Redraw(BTreeMap<u32, i32>),
}

impl Display {
  fn observe(&mut self, frame: Vec<Observation>, now: Duration) -> Result<Option<Effect>, Never> {
    let mut next = BTreeMap::new();
    let mut fresh = BTreeMap::new();

    for Observation { node, file, sample } in frame {
      let meter = match self.meters.remove(&file) {
        Some(previous) => {
          let Ok(meter) = previous.observe(sample, now);
          meter
        }
        None => Meter::WaitingForChange(sample),
      };

      match meter.at(now) {
        Ok(Some(reading)) => {
          let newest = fresh.entry(node).or_insert(reading);

          match (newest.0 < reading.0).then_some(()) {
            Some(()) => *newest = reading,
            None => {}
          }
        }
        Ok(None) => {}
      }

      let _previous = next.insert(file, meter);
    }

    self.meters = next;
    let gains = fresh
      .into_iter()
      .map(|(node, (_, gain))| (node, gain))
      .collect();

    Ok(match (self.shown.as_ref() == Some(&gains)).then_some(()) {
      Some(()) => None,
      None => {
        self.shown = Some(gains.clone());
        Some(Effect::Redraw(gains))
      }
    })
  }
}

fn observed(directory: &Path) -> io::Result<Vec<Observation>> {
  let entries = fs::read_dir(directory)?;
  let mut frame = Vec::new();

  for item in entries {
    let entry = item?;
    let file = entry.file_name();
    let id = file
      .to_str()
      .and_then(|name| name.strip_prefix("meter-"))
      .and_then(|name| name.split_once('-'));
    let node = match id.map(|(word, _suffix)| word.parse::<u32>()) {
      Some(Ok(node)) => node,
      Some(Err(_invalid_meter_name)) => continue,
      None => continue,
    };
    let bytes = match console_core_atomic_writes::read_prefix(&entry.path(), 8) {
      Ok(bytes) => bytes,
      Err(fault) => match (fault.kind() == ErrorKind::NotFound).then_some(()) {
        Some(()) => continue,
        None => return Err(fault),
      },
    };
    let Ok(reading) = Sample::read(&bytes);

    match reading {
      Some(sample) => frame.push(Observation { node, file, sample }),
      None => {}
    }
  }

  Ok(frame)
}

fn render(effect: Effect, output: &mut impl Write) -> io::Result<()> {
  let Effect::Redraw(gains) = effect;

  match gains.is_empty().then_some(()) {
    Some(()) => writeln!(output, "Waiting for active gain meters")?,
    None => {
      for (node, gain) in gains {
        writeln!(output, "Node {node:<5} {:+5.1} dB", f64::from(gain) / 10.0)?;
      }
    }
  }

  output.flush()
}

pub fn watch(output: &mut impl Write) -> io::Result<()> {
  let at = directory()?;
  let Ok(clock) = LiveClock::started();
  let watching = iterate(
    (Display::default(), clock, output),
    |(mut display, mut timer, stream)| {
      let reading = refresh(&mut display, &at, &timer, stream);

      Ok(match reading {
        Ok(()) => {
          let Ok(()) = timer.pause(INTERVAL);
          Step::Again((display, timer, stream))
        }
        Err(fault) => Step::Halt(Err(fault)),
      })
    },
  )
  .map_err(io::Error::other)?;

  watching
}

fn refresh(
  display: &mut Display,
  at: &Path,
  clock: &impl Clock,
  output: &mut impl Write,
) -> io::Result<()> {
  let frame = observed(at)?;
  let Ok(now) = clock.elapsed();
  let Ok(effect) = display.observe(frame, now);

  match effect {
    Some(redraw) => render(redraw, output),
    None => Ok(()),
  }
}
