//! Every kind of Ogg stream the player opens, demuxed and held to what this crate answered the
//! day it was brought under the workspace's rules.
//!
//! The fixtures are streams ffmpeg and flac wrote for a synthetic tone: Vorbis, Opus and Ogg FLAC
//! alone, two of them multiplexed, two chained one after the other, one long enough that a seek
//! has to bisect, and one with a byte flipped so a page fails its checksum. Each transcript is
//! the tracks the reader found, the tags it read, every packet it handed back (timestamp,
//! duration, trim, and the checksum of what it held) and where a handful of seeks landed, once
//! with gapless playback asked for and once without, and once more over a source that cannot seek.
//!
//! The damaged transcript was recorded from this reader. Its recovery is also checked against
//! the clean stream ending before the damaged page, without a transcript as the answer.

use console_core_audio_decoding_interfaces::formats::{
  FormatOptions, FormatReader, GaplessPlayback, SeekMode, SeekTo, Track,
};
use console_core_audio_decoding_interfaces::units::Time;
use console_core_audio_decoding_ogg_container::OggReader;
use console_core_byte_streams::{MediaSourceStream, MediaSourceStreamOptions, ReadOnlySource};
use console_core_checksums::crc32;
use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use std::cmp::Ordering;
use std::fmt::{Display, Write};
use std::io::Cursor;

#[derive(Debug)]
struct Fault(String);

impl<Cause: Display> From<Cause> for Fault {
  fn from(cause: Cause) -> Self {
    Fault(cause.to_string())
  }
}

struct Case {
  name: &'static str,
  file: &'static [u8],
  golden: &'static str,
}

#[derive(Clone, Copy)]
enum Source {
  Seekable,
  Forward,
}

fn open(file: &[u8], source: Source, gapless: GaplessPlayback) -> Result<OggReader, Fault> {
  let options = FormatOptions {
    gapless,
    ..FormatOptions::default()
  };
  let Ok(stream) = match source {
    Source::Seekable => MediaSourceStream::new(
      Box::new(Cursor::new(file.to_vec())),
      MediaSourceStreamOptions::default(),
    ),
    Source::Forward => {
      let Ok(forward) = ReadOnlySource::new(Cursor::new(file.to_vec()));

      MediaSourceStream::new(Box::new(forward), MediaSourceStreamOptions::default())
    }
  };

  let reader = OggReader::try_new(stream, &options)?;

  Ok(reader)
}

fn describe(track: &Track, into: &mut String) -> Result<(), Fault> {
  let mut parameters = track.codec_params.clone();
  let extra = parameters.extra_data.take();

  let extra_checksum = match &extra {
    Some(data) => {
      let Ok(checksum) = crc32::of(data);
      let Ok(length) = fitted::<_, u64>(data.len());

      Some((length, checksum))
    }
    None => None,
  };

  writeln!(
    into,
    "track {:#x} {:?} extra {:?}",
    track.id, parameters, extra_checksum
  )?;

  Ok(())
}

struct Reading {
  reader: OggReader,
  lines: Vec<String>,
}

fn read_step(mut reading: Reading) -> Result<Step<Reading, (Reading, String)>, Never> {
  match reading.reader.next_packet() {
    Ok(packet) => {
      let Ok(value_1) = packet.buf();

      let Ok(checksum) = crc32::of(value_1);

      let Ok(value_2) = packet.track_id();
      let Ok(value_3) = packet.ts();
      let Ok(value_4) = packet.dur();
      let Ok(value_5) = packet.trim_start();
      let Ok(value_6) = packet.trim_end();
      let Ok(value_7) = packet.buf();

      reading.lines.push(format!(
        "packet {:#x} ts {} dur {} trim {} {} len {} crc {:#x}",
        value_2,
        value_3,
        value_4,
        value_5,
        value_6,
        value_7.len(),
        checksum
      ));

      Ok(Step::Again(reading))
    }
    Err(fault) => Ok(Step::Halt((reading, fault.to_string()))),
  }
}

type Tagging = (OggReader, Vec<String>);

fn tags_step((mut reader, mut lines): Tagging) -> Result<Step<Tagging, Tagging>, Never> {
  let Ok(mut metadata) = reader.metadata();
  let Ok(revision) = metadata.pop();

  match revision {
    Some(revision_2) => {
      let Ok(value_8) = revision_2.tags();

      for tag in value_8 {
        lines.push(format!("tag {tag}"));
      }

      let Ok(value_9) = revision_2.visuals();

      lines.push(format!("visuals {}", value_9.len()));

      Ok(Step::Again((reader, lines)))
    }
    None => Ok(Step::Halt((reader, lines))),
  }
}

fn transcript(file: &[u8], source: Source, gapless: GaplessPlayback) -> Result<String, Fault> {
  let mut text = String::new();
  let reader = open(file, source, gapless)?;

  let Ok(value_10) = reader.tracks();

  for track in value_10 {
    describe(track, &mut text)?;
  }

  let (reader_2, tagged) = iterate((reader, Vec::new()), tags_step)?;

  for line in &tagged {
    writeln!(text, "{line}")?;
  }

  let (read, ending) = iterate(
    Reading {
      reader: reader_2,
      lines: Vec::new(),
    },
    read_step,
  )?;

  writeln!(text, "ended after {} packets: {}", read.lines.len(), ending)?;

  for line in &read.lines {
    writeln!(text, "{line}")?;
  }

  Ok(text)
}

fn seeks(file: &[u8], gapless: GaplessPlayback) -> Result<String, Fault> {
  let mut text = String::new();
  let mut reader = open(file, Source::Seekable, gapless)?;
  let Ok(value_11) = reader.tracks();

  let ids: Vec<u32> = value_11.iter().map(|track| track.id).collect();

  for id in &ids {
    for ts in [
      0u64,
      1,
      700,
      4096,
      10_000,
      22_050,
      100_000,
      400_000,
      1_000_000_000,
    ] {
      let found = reader.seek(SeekMode::Accurate, SeekTo::TimeStamp { ts, track_id: *id });
      let next = reader.next_packet().map(|packet| {
        let Ok(value_12) = packet.ts();
        let Ok(value_13) = packet.dur();

        (value_12, value_13)
      });

      writeln!(text, "seek {id:#x} {ts} -> {found:?} then {next:?}")?;
    }
  }

  for seconds in [0.0f64, 0.25, 0.5, 3.5, 9.0] {
    let found = reader.seek(
      SeekMode::Coarse,
      SeekTo::Time {
        time: Time::from(seconds),
        track_id: None,
      },
    );

    writeln!(text, "seek {seconds} s -> {found:?}")?;
  }

  let missing = reader.seek(
    SeekMode::Accurate,
    SeekTo::TimeStamp {
      ts: 0,
      track_id: 0xdead,
    },
  );

  writeln!(text, "seek on no track -> {missing:?}")?;

  Ok(text)
}

fn everything(name: &str, file: &[u8]) -> Result<String, Fault> {
  let mut all = format!("== {name}\n");

  for gapless in [GaplessPlayback::Disabled, GaplessPlayback::Enabled] {
    writeln!(all, "-- gapless {gapless:?}")?;
    let whole = transcript(file, Source::Seekable, gapless)?;
    let sought = seeks(file, gapless)?;

    all.push_str(&whole);
    all.push_str(&sought);
  }

  writeln!(all, "-- forward only")?;
  let forward = transcript(file, Source::Forward, GaplessPlayback::Disabled)?;

  all.push_str(&forward);

  Ok(all)
}

fn held(case: Case) -> Result<(), Fault> {
  let found = everything(case.name, case.file)?;

  match found.as_str().cmp(case.golden) {
    Ordering::Equal => Ok(()),
    Ordering::Less | Ordering::Greater => Err(Fault(format!("{} changed:\n{found}", case.name))),
  }
}

#[test]
fn a_vorbis_stream_is_demuxed() -> Result<(), Fault> {
  held(Case {
    name: "vorbis",
    file: include_bytes!("fixtures/vorbis-stereo.ogg"),
    golden: include_str!("goldens/vorbis.txt"),
  })
}

#[test]
fn an_opus_stream_is_demuxed() -> Result<(), Fault> {
  held(Case {
    name: "opus",
    file: include_bytes!("fixtures/opus-mono.opus"),
    golden: include_str!("goldens/opus.txt"),
  })
}

#[test]
fn an_ogg_flac_stream_is_demuxed() -> Result<(), Fault> {
  held(Case {
    name: "flac",
    file: include_bytes!("fixtures/flac-mono.oga"),
    golden: include_str!("goldens/flac.txt"),
  })
}

#[test]
fn two_multiplexed_streams_are_demuxed() -> Result<(), Fault> {
  held(Case {
    name: "muxed",
    file: include_bytes!("fixtures/muxed.ogg"),
    golden: include_str!("goldens/muxed.txt"),
  })
}

#[test]
fn two_chained_streams_are_demuxed() -> Result<(), Fault> {
  held(Case {
    name: "chained",
    file: include_bytes!("fixtures/chained.ogg"),
    golden: include_str!("goldens/chained.txt"),
  })
}

#[test]
fn a_long_stream_is_sought_by_bisection() -> Result<(), Fault> {
  held(Case {
    name: "long",
    file: include_bytes!("fixtures/long-vorbis.ogg"),
    golden: include_str!("goldens/long.txt"),
  })
}

#[test]
fn a_damaged_page_is_skipped() -> Result<(), Fault> {
  held(Case {
    name: "damaged",
    file: include_bytes!("fixtures/damaged.ogg"),
    golden: include_str!("goldens/damaged.txt"),
  })
}

#[test]
fn a_bad_checksum_loses_only_the_last_page() -> Result<(), Fault> {
  let clean = include_bytes!("fixtures/vorbis-stereo.ogg");
  let before_last_page = clean
    .windows(4)
    .rposition(|window| window == b"OggS")
    .and_then(|at| clean.get(..at))
    .ok_or("the fixture has no last page")?;

  for source in [Source::Seekable, Source::Forward] {
    let clean_reader = open(before_last_page, source, GaplessPlayback::Disabled)?;
    let damaged_reader = open(
      include_bytes!("fixtures/damaged.ogg"),
      source,
      GaplessPlayback::Disabled,
    )?;
    let (expected, _clean_end) = iterate(
      Reading {
        reader: clean_reader,
        lines: Vec::new(),
      },
      read_step,
    )?;
    let (damaged, _damaged_end) = iterate(
      Reading {
        reader: damaged_reader,
        lines: Vec::new(),
      },
      read_step,
    )?;

    assert!(!expected.lines.is_empty());
    assert_eq!(damaged.lines, expected.lines);
  }

  Ok(())
}

#[test]
fn a_file_that_is_not_ogg_is_refused() -> Result<(), Fault> {
  match open(
    b"RIFF....WAVEfmt ",
    Source::Seekable,
    GaplessPlayback::Disabled,
  ) {
    Ok(_reader) => Err(Fault(String::from("a wave file was taken for ogg"))),
    Err(_refused) => Ok(()),
  }
}

fn counting_step(
  (mut reader, count): (OggReader, u32),
) -> Result<Step<(OggReader, u32), u32>, Never> {
  match reader.next_packet() {
    Ok(_packet) => Ok(Step::Again((reader, count.saturating_add(1)))),
    Err(_ended) => Ok(Step::Halt(count)),
  }
}

#[test]
fn a_stream_cut_short_is_refused_or_ends() -> Result<(), Fault> {
  let file = include_bytes!("fixtures/vorbis-stereo.ogg");
  let mut found = String::new();

  for length in [0u32, 10, 27, 58, 500, 2000, 4000] {
    let Ok(end) = index(length);
    let cut = match file.get(..end) {
      Some(cut) => cut,
      None => return Err(Fault(String::from("a cut past the end"))),
    };

    match open(cut, Source::Seekable, GaplessPlayback::Disabled) {
      Ok(reader) => {
        let count = iterate((reader, 0u32), counting_step)?;

        writeln!(found, "{length}: opened, {count} packets")?;
      }
      Err(fault) => writeln!(found, "{length}: {}", fault.0)?,
    }
  }

  match found.as_str().cmp(include_str!("goldens/cut.txt")) {
    Ordering::Equal => Ok(()),
    Ordering::Less | Ordering::Greater => Err(Fault(format!("cuts changed:\n{found}"))),
  }
}
