//! Streams of every layer decoded end to end, and the samples they come out as.
//!
//! `goldens/streams.txt` was written by upstream's own decoder, symphonia
//! 0.5.5, before any of this crate was rewritten, so a change that moves one
//! sample, one timestamp or one parameter is a change that fails here. The
//! layer three streams cover each version of the format (one, two and two and
//! a half), mono and stereo, joint stereo, a protected stream, a stream with
//! no tag at the front and one with a variable rate. The layer two streams,
//! made by ffmpeg and twolame, cover mono, stereo, dual channel, joint
//! stereo, a protected stream, two rates and the half-rate tables. No
//! encoder on the machine writes layer one, so those streams were written
//! by a short script that quantizes a few tones with the allocation and
//! scale factors the standard describes; upstream decoded them all the same,
//! except the mono one, which it refuses. The mono samples are held to
//! FFmpeg n9.0.2's `mp1float` decode in `goldens/layer1-mono.f32le`, clipped
//! to the decoder's sample range, within half a 16-bit PCM step. The `intensity` streams are the
//! joint stereo ones with the mode extension of every header rewritten to say
//! intensity coding, which is not what was encoded and so is read as noise,
//! but noise that goes through the whole of the stereo decoding. Each is read
//! from the start and from a seek, since it is the reader that decides which
//! earlier frames a frame needs.

use std::error::Error;
use std::io::Cursor;

use console_core_audio_decoding_interfaces::audio::{AudioBufferRef, Signal};
use console_core_audio_decoding_interfaces::codecs::{Decoder, DecoderOptions};
use console_core_audio_decoding_interfaces::formats::{
  FormatOptions, FormatReader, GaplessPlayback, Packet, SeekMode, SeekTo,
};
use console_core_audio_decoding_interfaces::io::{MediaSourceStream, MediaSourceStreamOptions};
use console_core_audio_decoding_mp3::{MpaDecoder, MpaReader};
use console_core_checksums::crc32;
use console_core_iteration::{Step, iterate};
use console_core_number_conversion::{fitted, index};

type Failure = Box<dyn Error>;

struct Decoded {
  sample_rate: Option<u32>,
  channels: u32,
  frames: Option<u64>,
  delay: Option<u32>,
  padding: Option<u32>,
  packets: u32,
  refused: u32,
  bytes: u64,
  crc: u32,
}

impl std::fmt::Debug for Decoded {
  fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    out
      .debug_struct("Decoded")
      .field("sample_rate", &self.sample_rate)
      .field("channels", &self.channels)
      .field("frames", &self.frames)
      .field("delay", &self.delay)
      .field("padding", &self.padding)
      .field("packets", &self.packets)
      .field("refused", &self.refused)
      .field("bytes", &self.bytes)
      .field("crc", &self.crc)
      .finish()
  }
}

struct Sought {
  required: u64,
  actual: u64,
  decoded: Decoded,
}

impl std::fmt::Debug for Sought {
  fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    out
      .debug_struct("Sought")
      .field("required", &self.required)
      .field("actual", &self.actual)
      .field("decoded", &self.decoded)
      .finish()
  }
}

#[derive(Default)]
struct Drained {
  bytes: Vec<u8>,
  packets: u32,
  refused: u32,
  channels: u32,
}

enum Outcome {
  Decoded,
  Rejected,
}

fn refused<T>(what: &str) -> Result<T, Failure> {
  Err(Box::<dyn Error>::from(what))
}

fn reader(file: &[u8], gapless: GaplessPlayback) -> Result<MpaReader, Failure> {
  let Ok(source) = MediaSourceStream::new(
    Box::new(Cursor::new(file.to_vec())),
    MediaSourceStreamOptions::default(),
  );

  let options = FormatOptions {
    gapless,
    ..FormatOptions::default()
  };

  MpaReader::try_new(source, &options).map_err(Failure::from)
}

fn take(
  decoder: &mut MpaDecoder,
  packet: &Packet,
  drained: &mut Drained,
) -> Result<Outcome, Failure> {
  match decoder.decode(packet) {
    Ok(AudioBufferRef::F32(buffer)) => {
      let Ok(value_1) = buffer.spec();

      let Ok(value_5) = value_1.channels.count();

      drained.channels = value_5;

      let lanes: Vec<&[f32]> = (0..drained.channels)
        .map(|channel| {
          let Ok(value_2) = buffer.chan(channel);

          value_2
        })
        .collect();

      let Ok(value_3) = buffer.frames();

      for at in 0..value_3 {
        let Ok(at_2) = index(at);

        drained.bytes.extend(
          lanes
            .iter()
            .filter_map(|lane| lane.get(at_2))
            .flat_map(|sample| sample.to_le_bytes()),
        );
      }

      drained.packets = drained.packets.saturating_add(1);

      Ok(Outcome::Decoded)
    }
    Ok(_) => refused("the decoder did not make f32"),
    Err(_declined) => {
      drained.refused = drained.refused.saturating_add(1);

      Ok(Outcome::Rejected)
    }
  }
}

fn drain(reader: &mut MpaReader) -> Result<Decoded, Failure> {
  let Ok(value_4) = reader.tracks();

  let parameters = match value_4.first() {
    Some(track) => track.codec_params.clone(),
    None => return refused("the stream has no track"),
  };

  let decoder = MpaDecoder::try_new(&parameters, &DecoderOptions::default());
  let decoder_2 = decoder?;

  let ended = iterate(
    (&mut *reader, decoder_2, Drained::default()),
    |(reader_2, mut decoder_3, mut drained)| {
      Ok(match reader_2.next_packet() {
        Err(_ended) => Step::Halt(Ok(drained)),
        Ok(packet) => match take(&mut decoder_3, &packet, &mut drained) {
          Ok(Outcome::Decoded | Outcome::Rejected) => Step::Again((reader_2, decoder_3, drained)),
          Err(fault) => Step::Halt(Err(fault)),
        },
      })
    },
  );

  let drained = match ended {
    Ok(ended_2) => ended_2?,
    Err(endless) => return Err(Box::new(endless)),
  };

  let Ok(crc) = crc32::of(&drained.bytes);
  let Ok(bytes) = fitted::<_, u64>(drained.bytes.len());

  Ok(Decoded {
    sample_rate: parameters.sample_rate,
    channels: drained.channels,
    frames: parameters.n_frames,
    delay: parameters.delay,
    padding: parameters.padding,
    packets: drained.packets,
    refused: drained.refused,
    bytes,
    crc,
  })
}

fn seek(file: &[u8], mode: SeekMode, ts: u64) -> Result<String, Failure> {
  let mut reader = reader(file, GaplessPlayback::Enabled)?;

  match reader.seek(mode, SeekTo::TimeStamp { ts, track_id: 0 }) {
    Ok(sought) => {
      let decoded = drain(&mut reader)?;

      Ok(format!(
        "{:?}",
        Sought {
          required: sought.required_ts,
          actual: sought.actual_ts,
          decoded
        }
      ))
    }
    Err(_refused) => Ok("SeekRefused".to_string()),
  }
}

fn lines(name: &str, file: &[u8], ts: u64) -> Result<Vec<String>, Failure> {
  let mut whole = reader(file, GaplessPlayback::Enabled)?;
  let gapless = drain(&mut whole)?;

  let mut untrimmed = reader(file, GaplessPlayback::Disabled)?;
  let raw = drain(&mut untrimmed)?;

  let accurate = seek(file, SeekMode::Accurate, ts)?;
  let coarse = seek(file, SeekMode::Coarse, ts)?;

  Ok(vec![
    format!("{name} gapless {gapless:?}"),
    format!("{name} raw {raw:?}"),
    format!("{name} accurate {accurate}"),
    format!("{name} coarse {coarse}"),
  ])
}

fn expected(name: &str) -> Result<Vec<&'static str>, Failure> {
  let prefix = format!("{name} ");

  Ok(
    include_str!("goldens/streams.txt")
      .lines()
      .filter(|line| line.starts_with(&prefix))
      .collect(),
  )
}

fn check(name: &str, file: &[u8], ts: u64) -> Result<(), Failure> {
  let produced = lines(name, file, ts)?;
  let wanted = expected(name)?;

  assert_eq!(
    produced.iter().map(String::as_str).collect::<Vec<_>>(),
    wanted
  );

  Ok(())
}

#[test]
fn a_joint_stereo_stream_comes_out_as_it_always_did() -> Result<(), Failure> {
  check("joint", include_bytes!("fixtures/joint.mp3"), 30_000)
}

#[test]
fn a_protected_stream_with_no_tag_comes_out_as_it_always_did() -> Result<(), Failure> {
  check(
    "crc_plain",
    include_bytes!("fixtures/crc_plain.mp3"),
    30_000,
  )
}

#[test]
fn a_mono_stream_at_half_the_rate_comes_out_as_it_always_did() -> Result<(), Failure> {
  check(
    "mono_mpeg2",
    include_bytes!("fixtures/mono_mpeg2.mp3"),
    30_000,
  )
}

#[test]
fn a_stream_at_a_quarter_of_the_rate_comes_out_as_it_always_did() -> Result<(), Failure> {
  check("mpeg25", include_bytes!("fixtures/mpeg25.mp3"), 6_000)
}

#[test]
fn a_variable_rate_stream_comes_out_as_it_always_did() -> Result<(), Failure> {
  check("vbr", include_bytes!("fixtures/vbr.mp3"), 30_000)
}

#[test]
fn a_stream_at_forty_eight_thousand_comes_out_as_it_always_did() -> Result<(), Failure> {
  check(
    "stereo_48k",
    include_bytes!("fixtures/stereo_48k.mp3"),
    30_000,
  )
}

#[test]
fn a_joint_stereo_stream_at_half_the_rate_comes_out_as_it_always_did() -> Result<(), Failure> {
  check(
    "joint_mpeg2",
    include_bytes!("fixtures/joint_mpeg2.mp3"),
    30_000,
  )
}

#[test]
fn intensity_coding_read_in_a_stream_that_was_not_comes_out_as_it_always_did()
-> Result<(), Failure> {
  check(
    "intensity",
    include_bytes!("fixtures/intensity.mp3"),
    30_000,
  )
}

#[test]
fn intensity_with_mid_side_comes_out_as_it_always_did() -> Result<(), Failure> {
  check(
    "intensity_ms",
    include_bytes!("fixtures/intensity_ms.mp3"),
    30_000,
  )
}

#[test]
fn intensity_at_half_the_rate_comes_out_as_it_always_did() -> Result<(), Failure> {
  check(
    "intensity_mpeg2",
    include_bytes!("fixtures/intensity_mpeg2.mp3"),
    30_000,
  )
}

#[test]
fn intensity_alone_at_half_the_rate_comes_out_as_it_always_did() -> Result<(), Failure> {
  check(
    "intensity_mpeg2_only",
    include_bytes!("fixtures/intensity_mpeg2_only.mp3"),
    30_000,
  )
}

#[test]
fn a_layer_two_stereo_stream_comes_out_as_it_always_did() -> Result<(), Failure> {
  check(
    "l2_stereo",
    include_bytes!("fixtures/l2_stereo.mp2"),
    30_000,
  )
}

#[test]
fn a_layer_two_mono_stream_comes_out_as_it_always_did() -> Result<(), Failure> {
  check("l2_mono", include_bytes!("fixtures/l2_mono.mp2"), 30_000)
}

#[test]
fn a_layer_two_stream_at_forty_eight_thousand_comes_out_as_it_always_did() -> Result<(), Failure>
{
  check("l2_48k", include_bytes!("fixtures/l2_48k.mp2"), 30_000)
}

#[test]
fn a_layer_two_stream_at_half_the_rate_comes_out_as_it_always_did() -> Result<(), Failure> {
  check("l2_mpeg2", include_bytes!("fixtures/l2_mpeg2.mp2"), 30_000)
}

#[test]
fn a_layer_two_joint_stereo_stream_comes_out_as_it_always_did() -> Result<(), Failure> {
  check("l2_joint", include_bytes!("fixtures/l2_joint.mp2"), 30_000)
}

#[test]
fn a_protected_layer_two_stream_comes_out_as_it_always_did() -> Result<(), Failure> {
  check("l2_crc", include_bytes!("fixtures/l2_crc.mp2"), 30_000)
}

#[test]
fn a_layer_two_dual_channel_stream_comes_out_as_it_always_did() -> Result<(), Failure> {
  check("l2_dual", include_bytes!("fixtures/l2_dual.mp2"), 30_000)
}

#[test]
fn a_layer_one_stereo_stream_comes_out_as_it_always_did() -> Result<(), Failure> {
  check(
    "l1_stereo",
    include_bytes!("fixtures/l1_stereo.mp1"),
    10_000,
  )
}

#[test]
fn a_protected_layer_one_joint_stereo_stream_comes_out_as_it_always_did() -> Result<(), Failure> {
  check(
    "l1_joint_crc",
    include_bytes!("fixtures/l1_joint_crc.mp1"),
    10_000,
  )
}

#[test]
fn a_layer_one_mono_stream_decodes_every_frame_as_one_channel() -> Result<(), Failure> {
  let mut whole = reader(
    include_bytes!("fixtures/l1_mono.mp1"),
    GaplessPlayback::Enabled,
  )?;
  let decoded = drain(&mut whole)?;

  assert_eq!(
    (
      decoded.channels,
      decoded.packets,
      decoded.refused,
      decoded.bytes
    ),
    (1, 60, 0, 92_160)
  );

  let Ok(silence) = crc32::of(&[0u8; 92_160]);

  assert_ne!(decoded.crc, silence);

  Ok(())
}

#[test]
fn the_constructed_layer_one_mono_stream_matches_an_independent_decoder() -> Result<(), Failure> {
  let file = include_bytes!("fixtures/l1_mono.mp1");
  let reference = include_bytes!("goldens/layer1-mono.f32le");
  let mut whole = reader(file, GaplessPlayback::Disabled)?;
  let Ok(tracks) = whole.tracks();
  let track = tracks.first().ok_or("the stream has no track")?;
  let mut decoder = MpaDecoder::try_new(&track.codec_params, &DecoderOptions::default())?;
  let mut drained = Drained::default();
  let Ok(limit) = fitted::<_, u64>(file.len());

  for _packet in 0..limit {
    let packet = match whole.next_packet() {
      Ok(packet) => packet,
      Err(console_core_audio_decoding_interfaces::errors::Error::IoError(fault)) => {
        match ::core::primitive::bool::then_some(
          fault.kind() == std::io::ErrorKind::UnexpectedEof,
          (),
        ) {
          ::core::option::Option::Some(()) => break,
          ::core::option::Option::None => return Err(Failure::from(fault)),
        }
      }
      Err(fault) => return Err(Failure::from(fault)),
    };
    let _each_packet_is_recorded = take(&mut decoder, &packet, &mut drained)?;
  }

  assert_eq!(drained.bytes.len(), reference.len());

  let (actual, rest) = drained.bytes.as_chunks::<4>();
  let (expected, _) = reference.as_chunks::<4>();

  assert!(rest.is_empty());
  assert!(
    actual
      .iter()
      .all(|sample| f32::from_le_bytes(*sample).is_finite())
  );

  let difference = actual
    .iter()
    .zip(expected)
    .fold(0.0_f32, |maximum, (sample, expected_2)| {
      let expected_3 = f32::from_le_bytes(*expected_2).clamp(-1.0, 1.0);

      maximum.max((f32::from_le_bytes(*sample) - expected_3).abs())
    });

  assert!(
    difference < 1.0 / 65_536.0,
    "largest sample difference: {difference}"
  );

  Ok(())
}
