//! A song read into samples here, rather than by an ffmpeg at the far end of a
//! pipe.
//!
//! The readers and decoders are symphonia's, carried in `crates/` as
//! `console-core-audio-decoding` and its family: every container the library
//! and the films hold, and every codec in them but one. Opus is the one, and
//! it is most of the library, because every download is kept as Opus. The
//! containers find it and hand its packets over; the decoder that hears them
//! is filmcraft's clean-room one, `console-core-audio-decoding-opus`, and the
//! two meet here because a packet is the same packet whoever hears it.
//!
//! What comes out is the song at its own rate and with its own channels, and
//! the sink is told those rather than told forty-eight thousand and two.
//! PipeWire resamples and downmixes whatever it is handed already; ffmpeg was
//! asked to do it first only because a pipe of raw samples has no header to
//! say what is in it, and a sink started after the first block is decoded can
//! be told. The rate and the channels are the first block's rather than what
//! the container declares, because some containers declare nothing until a
//! frame is read.
//!
//! A start part of the way into a song is a seek to the packet at or before
//! the second asked for, and what that packet holds before the second is
//! decoded and dropped rather than played. Decoding it keeps the decoder's
//! state warm, which MP3 needs and every codec tolerates; playing it would
//! start a resumed song a fraction of a second behind where it was paused.
//!
//! A packet that will not decode is stepped over, the way a player meets a
//! scratch. A container that will not give the next packet is the end of the
//! song, said on the way out, because a song that cannot go on is one the
//! playlist should move past rather than one the player should sit on.

use console_core_audio_decoding::core::audio::{AudioBufferRef, SampleBuffer};
use console_core_audio_decoding::core::codecs::{
  CODEC_TYPE_NULL, CODEC_TYPE_OPUS, CodecParameters, Decoder, DecoderOptions,
};
use console_core_audio_decoding::core::errors::Error as Fault;
use console_core_audio_decoding::core::formats::{
  FormatOptions, FormatReader, Packet, SeekMode, SeekTo,
};
use console_core_audio_decoding::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use console_core_audio_decoding::core::meta::MetadataOptions;
use console_core_audio_decoding::core::probe::Hint;
use console_core_audio_decoding::core::units::{Time, TimeBase};
use console_core_audio_decoding::{codec_registry, format_probe};
use console_core_audio_decoding_opus as opus;
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::{Float, fitted, index, toward_zero_u64};
use std::fmt;
use std::fs::File;
use std::io::ErrorKind;
use std::path::Path;

const OPUS_RATE: u32 = 48_000;

#[derive(Debug)]
pub enum Undecoded {
  Unopened(std::io::Error),
  Unrecognized(Fault),
  Silent,
  Headless,
  Unsupported(Fault),
  Unheard(opus::Error),
  Unseekable(Fault),
}

impl fmt::Display for Undecoded {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Undecoded::Unopened(fault) => write!(formatter, "opening it: {fault}"),
      Undecoded::Unrecognized(fault) => {
        write!(formatter, "it is no container this player reads: {fault}")
      }
      Undecoded::Silent => write!(formatter, "it holds no sound"),
      Undecoded::Headless => write!(
        formatter,
        "its Opus carries no header to say how it was made"
      ),
      Undecoded::Unsupported(fault) => write!(
        formatter,
        "its sound is in no codec this player can hear: {fault}"
      ),
      Undecoded::Unheard(fault) => write!(formatter, "its Opus header: {fault}"),
      Undecoded::Unseekable(fault) => write!(formatter, "going part of the way in: {fault}"),
    }
  }
}

impl std::error::Error for Undecoded {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Format {
  pub rate: u32,
  pub channels: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
  pub samples: Vec<f32>,
  pub format: Format,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Start {
  AtTheBeginning,
  At(f64),
}

enum Voice {
  Native(Box<dyn Decoder>),
  Opus(opus::Decoder),
}

enum Audio {
  Block(Block),
  Empty,
  Skipped(PacketFault),
  End,
}

enum PacketFault {
  Native(Fault),
  Opus(opus::Error),
}

impl fmt::Display for PacketFault {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Native(fault) => fmt::Display::fmt(fault, formatter),
      Self::Opus(fault) => fmt::Display::fmt(fault, formatter),
    }
  }
}

struct SkippedPackets {
  count: u64,
  first: PacketFault,
}

pub struct Decoding {
  reader: Box<dyn FormatReader>,
  voice: Voice,
  track: u32,
  base: Option<TimeBase>,
  start: Start,
  skipped: Option<SkippedPackets>,
}

pub fn opened(song: &Path, crate::Seconds(from): crate::Seconds) -> Result<Decoding, Undecoded> {
  let file = File::open(song).map_err(Undecoded::Unopened)?;
  let Ok(source) = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
  let Ok(mut hint) = Hint::new();

  match song.extension().and_then(|extension| extension.to_str()) {
    Some(extension) => {
      let Ok(_configured_extension) = hint.with_extension(extension);
    }
    None => {}
  }

  let Ok(probe) = format_probe();
  let probed = probe
    .format(
      &hint,
      source,
      &FormatOptions::default(),
      &MetadataOptions::default(),
    )
    .map_err(Undecoded::Unrecognized)?;
  let mut reader = probed.format;
  let Ok(tracks) = reader.tracks();
  let track = tracks
    .iter()
    .find(|track| track.codec_params.codec != CODEC_TYPE_NULL);
  let (id, params) = track
    .map(|track_2| (track_2.id, track_2.codec_params.clone()))
    .ok_or(Undecoded::Silent)?;
  let start = match ::core::primitive::bool::then_some(from > 0.0, ()) {
    ::core::option::Option::Some(()) => Start::At(from),
    ::core::option::Option::None => Start::AtTheBeginning,
  };
  let voice = voice(&params, start)?;

  sought(reader.as_mut(), id, start).map_err(Undecoded::Unseekable)?;

  Ok(Decoding {
    reader,
    voice,
    track: id,
    base: params.time_base,
    start,
    skipped: None,
  })
}

fn voice(params: &CodecParameters, start: Start) -> Result<Voice, Undecoded> {
  match ::core::primitive::bool::then_some(params.codec == CODEC_TYPE_OPUS, ()) {
    ::core::option::Option::Some(()) => {
      let head = params.extra_data.as_deref().ok_or(Undecoded::Headless)?;
      let mut decoder = opus::Decoder::new(head).map_err(Undecoded::Unheard)?;

      match start {
        Start::AtTheBeginning => {
          let Ok(()) = decoder.trim_pre_skip();
        }
        Start::At(_) => {}
      }

      Ok(Voice::Opus(decoder))
    }
    ::core::option::Option::None => {
      let Ok(registry) = codec_registry();
      let decoder = registry
        .make(params, &DecoderOptions::default())
        .map_err(Undecoded::Unsupported)?;

      Ok(Voice::Native(decoder))
    }
  }
}

fn sought(reader: &mut dyn FormatReader, track: u32, start: Start) -> Result<(), Fault> {
  match start {
    Start::AtTheBeginning => Ok(()),
    Start::At(second) => {
      let to = SeekTo::Time {
        time: Time::from(second),
        track_id: Some(track),
      };
      let _where_it_landed_is_read_off_each_packet = reader.seek(SeekMode::Accurate, to)?;

      Ok(())
    }
  }
}

impl Decoding {
  pub fn decoded(&mut self) -> Result<Option<Block>, Never> {
    let found = console_core_iteration::iterate(self, |decoding| {
      let Ok(heard) = decoding.heard();

      Ok(match heard {
        Audio::Block(block) => Step::Halt(Some(block)),
        Audio::Empty => Step::Again(decoding),
        Audio::Skipped(fault) => {
          match &mut decoding.skipped {
            Some(skipped) => skipped.count = skipped.count.saturating_add(1),
            None => {
              decoding.skipped = Some(SkippedPackets {
                count: 1,
                first: fault,
              })
            }
          }

          Step::Again(decoding)
        }
        Audio::End => Step::Halt(None),
      })
    });

    Ok(match found {
      Ok(block) => block,
      Err(_endless) => None,
    })
  }

  fn heard(&mut self) -> Result<Audio, Never> {
    let packet = match self.reader.next_packet() {
      Ok(packet) => packet,
      Err(fault) => return ended(&fault),
    };

    let Ok(track) = packet.track_id();

    match ::core::primitive::bool::then_some(track == self.track, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Ok(Audio::Empty),
    }

    let Ok(voiced) = voiced(&mut self.voice, &packet);

    let block = match voiced {
      Audio::Block(block) => block,
      interrupted @ (Audio::Empty | Audio::Skipped(_) | Audio::End) => return Ok(interrupted),
    };

    let Ok(kept) = self.kept(&packet, block);

    Ok(kept)
  }

  fn kept(&self, packet: &Packet, block: Block) -> Result<Audio, Never> {
    let until = match (self.start, self.base) {
      (Start::At(second), Some(_)) => second,
      (Start::At(_), None) | (Start::AtTheBeginning, _) => return Ok(Audio::Block(block)),
    };
    let Ok(stamp) = packet.ts();
    let Ok(at) = base_seconds(self.base, stamp);
    let Ok(dropped) = toward_zero_u64((until - at) * f64::from(block.format.rate));
    let Ok(from) = index(dropped.saturating_mul(u64::from(block.format.channels)));

    Ok(match block.samples.get(from..) {
      Some([]) | None => Audio::Empty,
      Some(kept) => Audio::Block(Block {
        samples: kept.to_vec(),
        ..block
      }),
    })
  }
}

fn base_seconds(base: Option<TimeBase>, ts: u64) -> Result<f64, Never> {
  let time = match base {
    Some(base_2) => base_2.calc_time(ts),
    None => Time::new(0, 0.0),
  };
  let Ok(time_2) = time;
  let Ok(seconds) = time_2.seconds.float();

  Ok(seconds + f64::from(time_2.frac))
}

fn ended(fault: &Fault) -> Result<Audio, Never> {
  match fault {
    Fault::IoError(fault_2) => match ::core::primitive::bool::then_some(fault_2.kind() == ErrorKind::UnexpectedEof, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => console_core_logging::warn!("the song stops short: {fault}", fault = fault_2),
    },
    Fault::DecodeError(_)
    | Fault::SeekError(_)
    | Fault::Unsupported(_)
    | Fault::LimitError(_)
    | Fault::ResetRequired => {
      console_core_logging::warn!("the song stops short: {fault}");
    }
  }

  Ok(Audio::End)
}

fn voiced(voice: &mut Voice, packet: &Packet) -> Result<Audio, Never> {
  match voice {
    Voice::Opus(decoder) => {
      let Ok(bytes) = packet.buf();

      match decoder.decode(Some(bytes)) {
        Ok(planar) => {
          let Ok(samples) = interleaved(&planar);
          let Ok(channels) = fitted::<_, u32>(planar.len());

          Ok(Audio::Block(Block {
            samples,
            format: Format {
              rate: OPUS_RATE,
              channels,
            },
          }))
        }
        Err(fault) => Ok(Audio::Skipped(PacketFault::Opus(fault))),
      }
    }
    Voice::Native(decoder) => match decoder.decode(packet).and_then(native_audio) {
      Ok(audio) => Ok(audio),
      Err(fault) => refused(fault),
    },
  }
}

fn native_audio(decoded: AudioBufferRef<'_>) -> Result<Audio, Fault> {
  let Ok(spec) = decoded.spec();
  let spec_2 = *spec;
  let Ok(frames) = decoded.capacity();
  let mut buffer = SampleBuffer::<f32>::new(u64::from(frames), spec_2)?;

  buffer.copy_interleaved_ref(decoded)?;

  let Ok(samples) = buffer.samples();
  let Ok(channels) = spec_2.channels.count();

  Ok(Audio::Block(Block {
    samples: samples.to_vec(),
    format: Format {
      rate: spec_2.rate,
      channels,
    },
  }))
}

fn refused(fault: Fault) -> Result<Audio, Never> {
  match fault {
    Fault::DecodeError(_) | Fault::IoError(_) => Ok(Audio::Skipped(PacketFault::Native(fault))),
    Fault::SeekError(_) | Fault::Unsupported(_) | Fault::LimitError(_) | Fault::ResetRequired => {
      ended(&fault)
    }
  }
}

impl Drop for Decoding {
  fn drop(&mut self) {
    match &self.skipped {
      Some(skipped) => console_core_logging::warn!(
        "skipped {} audio packets during playback. First failure was {}",
        skipped.count,
        skipped.first
      ),
      None => {}
    }
  }
}

fn interleaved(planar: &[Vec<f32>]) -> Result<Vec<f32>, Never> {
  let first = match planar.first() {
    Some(first) => first,
    None => return Ok(Vec::new()),
  };
  let Ok(frames) = fitted::<_, u32>(first.len());

  Ok(
    (0..frames)
      .flat_map(|frame| {
        let Ok(at) = index(frame);

        planar
          .iter()
          .filter_map(move |channel| channel.get(at).copied())
      })
      .collect(),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  const TONE_PEAK: f32 = 0.092;

  fn every(decoding: &mut Decoding) -> Result<Vec<Block>, Never> {
    let gathered =
      console_core_iteration::iterate((decoding, Vec::new()), |(decoding_2, mut blocks)| {
        let Ok(next) = decoding_2.decoded();

        Ok(match next {
          Some(block) => {
            blocks.push(block);

            Step::Again((decoding_2, blocks))
          }
          None => Step::Halt(blocks),
        })
      });

    Ok(match gathered {
      Ok(blocks) => blocks,
      Err(_endless) => Vec::new(),
    })
  }

  fn frames(blocks: &[Block]) -> Result<u64, Never> {
    let samples: u64 = blocks
      .iter()
      .map(|block| {
        let Ok(samples) = fitted::<_, u64>(block.samples.len());

        samples
      })
      .sum();
    let channels = blocks.first().map(|block| u64::from(block.format.channels));

    Ok(
      match channels.and_then(|channels_2| samples.checked_div(channels_2)) {
        Some(frames) => frames,
        None => 0,
      },
    )
  }

  fn peak(blocks: &[Block]) -> Result<f32, Never> {
    Ok(
      blocks
        .iter()
        .flat_map(|block| block.samples.iter())
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs())),
    )
  }

  fn a_rising_wave() -> Result<Vec<u8>, Never> {
    let rising: Vec<i16> = (0_i16..8_000).collect();

    crate::tags::tests::a_wave(&rising)
  }

  fn written(
    named: &str,
    bytes: &[u8],
  ) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let folder = console_core_temporary_directories::fresh(named)?;
    let sample = folder.join("sample");
    let at = match Path::new(named).extension() {
      Some(extension) => sample.with_extension(extension),
      None => sample,
    };

    console_core_atomic_writes::whole(&at, bytes)?;

    Ok(at)
  }

  #[test]
  fn an_opus_song_is_heard_at_forty_eight_thousand_in_stereo_for_as_long_as_it_lasts()
  -> Result<(), Box<dyn std::error::Error>> {
    let song = written(
      "music-player-a-tone.opus",
      include_bytes!("../fixtures/a-tone.opus"),
    )?;
    let mut decoding = opened(&song, crate::Seconds(0.0))?;
    let Ok(blocks) = every(&mut decoding);
    let Ok(heard) = frames(&blocks);
    let Ok(peak) = peak(&blocks);
    let first = blocks.first().ok_or("no sound came out of the song")?;

    assert_eq!(
      first.format,
      Format {
        rate: 48_000,
        channels: 2
      }
    );
    assert!(
      heard.abs_diff(24_000) < 1_000,
      "half a second came out as {heard} frames"
    );
    assert!(
      (peak - TONE_PEAK).abs() < 0.005,
      "the tone peaks at {peak} rather than at -20.7 dB"
    );

    Ok(())
  }

  #[test]
  fn an_opus_film_track_is_heard_out_of_matroska_the_same_way()
  -> Result<(), Box<dyn std::error::Error>> {
    let film = written(
      "music-player-a-tone.webm",
      include_bytes!("../fixtures/a-tone.webm"),
    )?;
    let mut decoding = opened(&film, crate::Seconds(0.0))?;
    let Ok(blocks) = every(&mut decoding);
    let Ok(heard) = frames(&blocks);
    let Ok(peak) = peak(&blocks);

    assert!(
      heard.abs_diff(24_000) < 1_000,
      "half a second came out as {heard} frames"
    );
    assert!(
      (peak - TONE_PEAK).abs() < 0.005,
      "the tone peaks at {peak} rather than at -20.7 dB"
    );

    Ok(())
  }

  #[test]
  fn a_start_part_of_the_way_in_begins_on_the_sample_it_was_asked_for()
  -> Result<(), Box<dyn std::error::Error>> {
    let Ok(rising) = a_rising_wave();
    let song = written("music-player-rising.wav", &rising)?;
    let mut decoding = opened(&song, crate::Seconds(0.5))?;
    let Ok(blocks) = every(&mut decoding);
    let Ok(heard) = frames(&blocks);
    let first = blocks
      .first()
      .and_then(|block| block.samples.first())
      .ok_or("nothing after half a second")?;
    let expected = 4_000.0 / 32_768.0;

    assert_eq!(heard, 4_000);
    assert!(
      (first - expected).abs() < 2.0 / 32_768.0,
      "began on {first}, which is not the sample half a second in"
    );

    Ok(())
  }

  #[test]
  fn what_is_not_a_song_is_said_so() -> Result<(), Box<dyn std::error::Error>> {
    let song = written("music-player-not-a-song.opus", b"<html>not found</html>")?;

    assert!(matches!(
      opened(&song, crate::Seconds(0.0)),
      Err(Undecoded::Unrecognized(_))
    ));

    Ok(())
  }
}
