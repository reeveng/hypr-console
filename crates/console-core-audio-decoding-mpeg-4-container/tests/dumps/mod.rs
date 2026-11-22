use std::fmt::{self, Write};
use std::io::Cursor;

use console_core_audio_decoding_interfaces::formats::{
  FormatOptions, FormatReader, SeekMode, SeekTo,
};
use console_core_audio_decoding_interfaces::meta::MetadataOptions;
use console_core_audio_decoding_interfaces::probe::{Hint, Probe};
use console_core_audio_decoding_interfaces::units::Time;
use console_core_audio_decoding_mpeg_4_container::IsoMp4Reader;
use console_core_byte_streams::{MediaSourceStream, MediaSourceStreamOptions, ReadOnlySource};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

#[derive(Debug)]
pub enum Failure {
  Writing(fmt::Error),
  NeverEnded,
  NoPrefix,
}

impl fmt::Display for Failure {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Writing(error) => write!(formatter, "the report could not be written: {error}"),
      Self::NeverEnded => formatter.write_str("the stream never ended"),
      Self::NoPrefix => formatter.write_str("no prefix"),
    }
  }
}

impl std::error::Error for Failure {}

impl From<fmt::Error> for Failure {
  fn from(error: fmt::Error) -> Self {
    Self::Writing(error)
  }
}

const PACKET_LIMIT: u32 = 4096;

const SEEK_STAMPS: [u64; 5] = [0, 1000, 20_000, 44_100, 4_000_000];

const SEEK_SECONDS: [u64; 3] = [0, 1, 100];

pub enum Source {
  Seekable,
  Forward,
}

fn probe() -> Result<Probe, Never> {
  let mut probe = Probe::default();
  let Ok(()) = probe.register_all::<IsoMp4Reader>();

  Ok(probe)
}

fn stream(bytes: &[u8], source: &Source) -> Result<MediaSourceStream, Never> {
  let options = MediaSourceStreamOptions::default();
  let cursor = Cursor::new(bytes.to_vec());

  match source {
    Source::Seekable => MediaSourceStream::new(Box::new(cursor), options),
    Source::Forward => {
      let Ok(forward) = ReadOnlySource::new(cursor);

      MediaSourceStream::new(Box::new(forward), options)
    }
  }
}

fn open(
  bytes: &[u8],
  source: &Source,
) -> Result<Box<dyn FormatReader>, console_core_audio_decoding_interfaces::errors::Error> {
  let Ok(probe) = probe();
  let Ok(stream) = stream(bytes, source);
  let Ok(value_1) = Hint::new();

  let found = probe.format(
    &value_1,
    stream,
    &FormatOptions::default(),
    &MetadataOptions::default(),
  )?;
  Ok(found.format)
}

fn weigh(bytes: &[u8]) -> Result<u64, Never> {
  Ok(bytes.iter().fold(7, |held, byte| {
    held.wrapping_mul(31).wrapping_add(u64::from(*byte))
  }))
}

struct Weight<'a>(&'a [u8]);

impl fmt::Display for Weight<'_> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    let Ok(weight) = weigh(self.0);
    write!(formatter, "{weight}")
  }
}

fn describe(reader: &mut dyn FormatReader, out: &mut String) -> Result<(), Failure> {
  let Ok(value_2) = reader.tracks();

  for track in value_2 {
    writeln!(
      out,
      "track {} {:?} {:?}",
      track.id, track.codec_params, track.language
    )?;
  }

  let Ok(value_3) = reader.cues();

  writeln!(out, "cues {}", value_3.len())?;
  let Ok(metadata) = reader.metadata();
  let Ok(current) = metadata.current();
  writeln!(out, "revisions current={}", current.is_some())?;

  match current {
    Some(revision) => {
      let Ok(value_4) = revision.tags();

      for tag in value_4 {
        writeln!(out, "tag {:?} {} = {}", tag.std_key, tag.key, tag.value)?;
      }

      let Ok(value_5) = revision.visuals();

      for visual in value_5 {
        writeln!(
          out,
          "visual {} {:?} {:?} {:?} {:?} {} {}",
          visual.media_type,
          visual.dimensions,
          visual.bits_per_pixel,
          visual.color_mode,
          visual.usage,
          visual.data.len(),
          Weight(&visual.data)
        )?;
      }
    }
    None => writeln!(out, "no metadata")?,
  }

  Ok(())
}

const HEAD: u32 = 3;

fn packets(reader: &mut dyn FormatReader, out: &mut String) -> Result<(), Failure> {
  let mut count: u64 = 0;
  let mut folded: u64 = 0;

  for _ in 0..PACKET_LIMIT {
    match reader.next_packet() {
      Ok(packet) => {
        let Ok(value_6) = packet.track_id();
        let Ok(value_7) = packet.ts();
        let Ok(value_8) = packet.dur();
        let Ok(value_9) = packet.buf();
        let Ok(value_10) = packet.buf();

        let line = format!(
          "packet track={} ts={} dur={} len={} sum={}",
          value_6,
          value_7,
          value_8,
          value_9.len(),
          Weight(value_10)
        );
        let Ok(weight) = weigh(line.as_bytes());
        folded = folded.wrapping_mul(1_000_003).wrapping_add(weight);
        count = count.wrapping_add(1);

        match ::core::primitive::bool::then_some(count <= u64::from(HEAD), ()) {
          ::core::option::Option::Some(()) => writeln!(out, "{line}")?,
          ::core::option::Option::None => {}
        }
      }
      Err(error) => {
        writeln!(out, "{count} packets, fold {folded}, ended: {error}")?;
        return Ok(());
      }
    }
  }

  Err(Failure::NeverEnded)
}

fn then(reader: &mut dyn FormatReader, out: &mut String) -> Result<(), Failure> {
  match reader.next_packet() {
    Ok(packet) => {
      let Ok(value_11) = packet.track_id();
      let Ok(value_12) = packet.ts();
      let Ok(value_13) = packet.buf();
      let Ok(value_14) = packet.buf();

      writeln!(
        out,
        "  then track={} ts={} len={} sum={}",
        value_11,
        value_12,
        value_13.len(),
        Weight(value_14)
      )?
    }
    Err(error) => writeln!(out, "  then failed: {error}")?,
  }

  Ok(())
}

fn seeks(reader: &mut dyn FormatReader, out: &mut String) -> Result<(), Failure> {
  let Ok(value_15) = reader.tracks();

  let track_ids: Vec<u32> = value_15.iter().map(|track| track.id).collect();

  for track_id in track_ids {
    for ts in SEEK_STAMPS {
      let result = reader.seek(SeekMode::Accurate, SeekTo::TimeStamp { ts, track_id });

      match result {
        Ok(to) => writeln!(out, "seek ts={ts} track={track_id} -> {to:?}")?,
        Err(error) => writeln!(out, "seek ts={ts} track={track_id} failed: {error}")?,
      }

      then(reader, out)?;
    }
  }

  for seconds in SEEK_SECONDS {
    let Ok(time) = Time::new(seconds, 0.25);

    match reader.seek(
      SeekMode::Accurate,
      SeekTo::Time {
        time,
        track_id: None,
      },
    ) {
      Ok(to) => writeln!(out, "seek time={seconds} -> {to:?}")?,
      Err(error) => writeln!(out, "seek time={seconds} failed: {error}")?,
    }

    then(reader, out)?;
  }

  Ok(())
}

pub fn whole(bytes: &[u8], source: &Source) -> Result<String, Failure> {
  let mut out = String::new();

  match open(bytes, source) {
    Ok(mut reader) => {
      describe(reader.as_mut(), &mut out)?;
      packets(reader.as_mut(), &mut out)?;
      seeks(reader.as_mut(), &mut out)?;
    }
    Err(error) => writeln!(out, "refused: {error}")?,
  }

  match open(bytes, source) {
    Ok(mut reader) => {
      writeln!(out, "-- seeking before reading")?;
      seeks(reader.as_mut(), &mut out)?;
    }
    Err(error) => writeln!(out, "refused again: {error}")?,
  }

  Ok(out)
}

pub fn shortened(bytes: &[u8], step: u32, source: &Source) -> Result<String, Failure> {
  let mut out = String::new();
  let Ok(total): Result<u32, Never> = fitted(bytes.len());

  for length in 0..total {
    match length.checked_rem(step) {
      Some(0) => {
        let Ok(end) = index(length);
        let prefix = bytes.get(..end).ok_or(Failure::NoPrefix)?;
        prefix_line(prefix, length, source, &mut out)?;
      }
      Some(_) | None => {}
    }
  }

  Ok(out)
}

fn prefix_line(
  prefix: &[u8],
  length: u32,
  source: &Source,
  out: &mut String,
) -> Result<(), Failure> {
  match open(prefix, source) {
    Ok(mut reader) => {
      let mut tail = String::new();
      packets(reader.as_mut(), &mut tail)?;

      let last = match tail.lines().last() {
        Some(line) => line,
        None => "",
      };

      writeln!(
        out,
        "{length}: opened, {} lines, {}",
        tail.lines().count(),
        last
      )?;
    }
    Err(error) => writeln!(out, "{length}: refused: {error}")?,
  }

  Ok(())
}
