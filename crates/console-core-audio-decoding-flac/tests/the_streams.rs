//! Streams `flac` itself wrote, read back through the demuxer and the decoder.
//!
//! The encoder's own settings are what make each file worth having: every
//! channel assignment, wasted bits, constant, verbatim, fixed and linear
//! subframes up to the deepest order, both residual codings, an odd sample
//! rate and an odd channel count. What the decoder answers is held to the MD5
//! the encoder stored in each stream, so a file is right when the decoder
//! says so rather than when it matches a number somebody pasted. The packets,
//! the metadata and the seeks are the demuxer's half, and what they answer is
//! the answer the code gave before it was held to the workspace's rules.

use std::io::Cursor;

use console_core_audio_decoding_flac::{FlacDecoder, FlacReader};
use console_core_audio_decoding_interfaces::audio::{AudioBufferRef, Signal};
use console_core_audio_decoding_interfaces::codecs::{
  Decoder, DecoderOptions, PacketIntegrity, VerificationMode, VerificationOutcome,
};
use console_core_audio_decoding_interfaces::errors::{Error, SeekErrorKind};
use console_core_audio_decoding_interfaces::formats::{
  FormatOptions, FormatReader, Packet, SeekMode, SeekTo,
};
use console_core_byte_streams::{MediaSourceStream, MediaSourceStreamOptions, ReadOnlySource};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

type Failure = Box<dyn std::error::Error>;

const STREAMS: [(&str, &[u8]); 10] = [
  ("stereo16", include_bytes!("streams/stereo16.flac")),
  ("stereo24", include_bytes!("streams/stereo24.flac")),
  ("mono8", include_bytes!("streams/mono8.flac")),
  ("parts16", include_bytes!("streams/parts16.flac")),
  ("odd16", include_bytes!("streams/odd16.flac")),
  ("kilo16", include_bytes!("streams/kilo16.flac")),
  ("deep16", include_bytes!("streams/deep16.flac")),
  ("sides16", include_bytes!("streams/sides16.flac")),
  ("left_side", include_bytes!("streams/left_side.flac")),
  ("right_side", include_bytes!("streams/right_side.flac")),
];

fn stream(name: &str) -> Result<&'static [u8], Failure> {
  let found = STREAMS.iter().find(|(known, _)| *known == name);

  match found {
    Some((_, bytes)) => Ok(bytes),
    None => Err(Failure::from(format!("no stream called {name}"))),
  }
}

fn source(bytes: Vec<u8>) -> Result<MediaSourceStream, Never> {
  MediaSourceStream::new(
    Box::new(Cursor::new(bytes)),
    MediaSourceStreamOptions::default(),
  )
}

fn reader(bytes: Vec<u8>) -> Result<FlacReader, Failure> {
  let Ok(media) = source(bytes);
  let opened = FlacReader::try_new(media, &FormatOptions::default())?;

  Ok(opened)
}

fn count<Counted>(items: &[Counted]) -> Result<u32, Never> {
  fitted::<_, u32>(items.len())
}

struct Packets {
  format: FlacReader,
}

impl Iterator for Packets {
  type Item = Result<Packet, Error>;

  fn next(&mut self) -> Option<Self::Item> {
    match self.format.next_packet() {
      Ok(packet) => Some(Ok(packet)),
      Err(Error::IoError(_ended)) => None,
      Err(other) => Some(Err(other)),
    }
  }
}

#[derive(Debug, Default, PartialEq)]
struct Decoded {
  packets: u32,
  refusals: Vec<String>,
  frames: u64,
  first_sample: Option<i32>,
  last_sample: Option<i32>,
  sum: i64,
  verification: VerificationOutcome,
}

impl Decoded {
  fn take(&mut self, frames: u32, samples: &[i32]) -> Result<(), Never> {
    self.frames = self.frames.saturating_add(u64::from(frames));
    self.sum = samples.iter().fold(self.sum, |sum, sample| {
      sum.saturating_add(i64::from(*sample))
    });

    match (self.first_sample, samples.first()) {
      (None, Some(first)) => self.first_sample = Some(*first),
      (Some(_), _) | (None, None) => (),
    }

    match samples.last() {
      Some(last) => self.last_sample = Some(*last),
      None => (),
    }

    Ok(())
  }

  fn refused(&self) -> Result<u32, Never> {
    count(&self.refusals)
  }
}

fn decode(bytes: Vec<u8>) -> Result<Decoded, Failure> {
  let format = reader(bytes)?;
  let Ok(value_1) = format.tracks();

  let track = value_1.first().ok_or("no track")?;
  let params = track.codec_params.clone();
  let options = DecoderOptions {
    verification: VerificationMode::Verify,
  };
  let mut decoder = FlacDecoder::try_new(&params, &options)?;
  let mut survey = Decoded::default();

  for packet in (Packets { format }) {
    let packet_2 = packet?;

    survey.packets = survey.packets.saturating_add(1);

    match decoder.decode(&packet_2) {
      Ok(AudioBufferRef::S32(buffer)) => {
        let Ok(planes) = buffer.planes();
        let Ok(channels) = planes.planes();
        let planar: Vec<i32> = channels
          .iter()
          .flat_map(|plane| plane.iter().copied())
          .collect();
        let Ok(value_2) = buffer.frames();

        let Ok(()) = survey.take(value_2, &planar);
      }
      Ok(_) => return Err(Failure::from("not 32 bit")),
      Err(refusal) => survey.refusals.push(refusal.to_string()),
    }
  }

  let Ok(value_3) = decoder.finalize();

  survey.verification = value_3.verification;

  Ok(survey)
}

#[test]
fn every_stream_decodes_to_the_md5_the_encoder_stored() -> Result<(), Failure> {
  for (name, bytes) in STREAMS {
    let survey = decode(bytes.to_vec())?;
    let format = reader(bytes.to_vec())?;
    let Ok(value_4) = format.tracks();

    let track = value_4.first().ok_or("no track")?;

    assert_eq!(survey.verification, VerificationOutcome::Passed, "{name}");
    assert_eq!(survey.refusals, Vec::<String>::new(), "{name}");
    assert_eq!(Some(survey.frames), track.codec_params.n_frames, "{name}");
  }

  Ok(())
}

#[test]
fn what_each_stream_decodes_to_is_what_it_decoded_to_before() -> Result<(), Failure> {
  let mut answers = Vec::new();

  for (name, bytes) in STREAMS {
    let survey = decode(bytes.to_vec())?;
    let first = survey.first_sample.ok_or("no samples")?;
    let last = survey.last_sample.ok_or("no samples")?;

    answers.push((name, survey.packets, survey.frames, first, last, survey.sum));
  }

  assert_eq!(answers, GOLDEN_SAMPLES);

  Ok(())
}

const GOLDEN_SAMPLES: [(&str, u32, u64, i32, i32, i64); 10] = [
  ("stereo16", 7, 7000, -131072, -578551808, 16277307392),
  ("stereo24", 2, 6000, -8314368, -719955456, 155327433728),
  ("mono8", 15, 3000, 0, 0, 167772160),
  ("parts16", 6, 6000, 536870912, 180355072, 2238153555968),
  ("odd16", 5, 2500, 2949120, 1236074496, 6653673472),
  ("kilo16", 10, 2500, 3932160, 768802816, 743505920),
  ("deep16", 2, 6000, -4456448, -836894720, 19662241792),
  ("sides16", 12, 6000, 8716288, 644153344, 62160240640),
  ("left_side", 13, 6000, 8716288, 644153344, 62160240640),
  ("right_side", 13, 6000, 8716288, 644153344, 62160240640),
];

#[test]
fn a_stream_that_was_cut_still_gives_the_packets_before_the_cut() -> Result<(), Failure> {
  let whole = stream("stereo16")?;
  let kept = whole
    .get(..whole.len().saturating_sub(5000))
    .ok_or("short")?;
  let survey = decode(kept.to_vec())?;
  let Ok(refused) = survey.refused();

  assert_eq!(
    (survey.packets, refused, survey.frames, survey.verification),
    GOLDEN_CUT
  );

  Ok(())
}

const GOLDEN_CUT: (u32, u32, u64, VerificationOutcome) =
  (4, 0, 4608, VerificationOutcome::Failed);

#[test]
fn a_stream_with_a_byte_flipped_resynchronizes_on_the_next_frame() -> Result<(), Failure> {
  let whole = stream("stereo16")?;
  let mut damaged = whole.to_vec();
  let audio_start = 36862u32;
  let Ok(length) = count(&damaged);
  let at = length
    .saturating_sub(audio_start)
    .wrapping_div(2)
    .saturating_add(audio_start);

  for place in [at, at.saturating_add(3000)] {
    let Ok(position) = index(place);
    let byte = damaged.get_mut(position).ok_or("past the end")?;

    *byte = !*byte;
  }

  let survey = decode(damaged)?;
  let Ok(refused) = survey.refused();

  assert_eq!(
    (survey.packets, refused, survey.frames, survey.verification),
    GOLDEN_DAMAGED
  );

  Ok(())
}

const GOLDEN_DAMAGED: (u32, u32, u64, VerificationOutcome) =
  (5, 0, 4696, VerificationOutcome::Failed);

#[test]
fn the_packets_are_cut_where_the_encoder_cut_them() -> Result<(), Failure> {
  let mut digest = Vec::new();

  for (name, bytes) in STREAMS {
    let format = reader(bytes.to_vec())?;
    let mut cuts = Vec::new();

    for packet in (Packets { format }) {
      let packet_2 = packet?;
      let Ok(value_5) = packet_2.buf();

      let Ok(length) = count(value_5);

      let Ok(value_6) = packet_2.ts();
      let Ok(value_7) = packet_2.dur();

      cuts.push((value_6, value_7, length));
    }

    let Ok(packets) = count(&cuts);
    let bytes_cut = cuts
      .iter()
      .fold(0u32, |sum, (_, _, length)| sum.saturating_add(*length));

    digest.push((name, packets, cuts.last().copied(), bytes_cut));
  }

  assert_eq!(digest, GOLDEN_PACKETS);

  Ok(())
}

type Cut = (&'static str, u32, Option<(u64, u64, u32)>, u32);

const GOLDEN_PACKETS: [Cut; 10] = [
  ("stereo16", 7, Some((6912, 88, 240)), 16635),
  ("stereo24", 2, Some((4096, 1904, 8220)), 25830),
  ("mono8", 15, Some((2800, 200, 173)), 2578),
  ("parts16", 6, Some((5000, 1000, 1487)), 8500),
  ("odd16", 5, Some((2304, 196, 799)), 9382),
  ("kilo16", 10, Some((2304, 196, 284)), 3509),
  ("deep16", 2, Some((3000, 3000, 6742)), 13483),
  ("sides16", 12, Some((5500, 500, 838)), 12760),
  ("left_side", 13, Some((5760, 240, 706)), 16758),
  ("right_side", 13, Some((5760, 240, 497)), 14268),
];

#[test]
fn the_tracks_say_what_the_stream_info_says() -> Result<(), Failure> {
  let mut answers = Vec::new();

  for (name, bytes) in STREAMS {
    let format = reader(bytes.to_vec())?;
    let Ok(value_8) = format.tracks();

    let Ok(tracks) = count(value_8);
    let Ok(value_9) = format.tracks();

    let track = value_9.first().ok_or("no track")?;
    let params = &track.codec_params;

    answers.push((
      name,
      tracks,
      params.sample_rate,
      params.bits_per_sample,
      params.channels.map(|channels| {
        let Ok(count) = channels.count();

        count
      }),
      params.n_frames,
      params.max_frames_per_packet,
      params.packet_data_integrity,
    ));
  }

  assert_eq!(answers, GOLDEN_TRACKS);

  Ok(())
}

type Described = (
  &'static str,
  u32,
  Option<u32>,
  Option<u32>,
  Option<u32>,
  Option<u64>,
  Option<u64>,
  PacketIntegrity,
);

const GOLDEN_TRACKS: [Described; 10] = [
  (
    "stereo16",
    1,
    Some(44100),
    Some(16),
    Some(2),
    Some(7000),
    None,
    PacketIntegrity::Guaranteed,
  ),
  (
    "stereo24",
    1,
    Some(96000),
    Some(24),
    Some(2),
    Some(6000),
    None,
    PacketIntegrity::Guaranteed,
  ),
  (
    "mono8",
    1,
    Some(8000),
    Some(8),
    Some(1),
    Some(3000),
    None,
    PacketIntegrity::Guaranteed,
  ),
  (
    "parts16",
    1,
    Some(32000),
    Some(16),
    Some(2),
    Some(6000),
    None,
    PacketIntegrity::Guaranteed,
  ),
  (
    "odd16",
    1,
    Some(12345),
    Some(16),
    Some(3),
    Some(2500),
    None,
    PacketIntegrity::Guaranteed,
  ),
  (
    "kilo16",
    1,
    Some(7000),
    Some(16),
    Some(1),
    Some(2500),
    None,
    PacketIntegrity::Guaranteed,
  ),
  (
    "deep16",
    1,
    Some(22050),
    Some(16),
    Some(2),
    Some(6000),
    None,
    PacketIntegrity::Guaranteed,
  ),
  (
    "sides16",
    1,
    Some(48000),
    Some(16),
    Some(2),
    Some(6000),
    None,
    PacketIntegrity::Guaranteed,
  ),
  (
    "left_side",
    1,
    Some(48000),
    Some(16),
    Some(2),
    Some(6000),
    None,
    PacketIntegrity::Guaranteed,
  ),
  (
    "right_side",
    1,
    Some(48000),
    Some(16),
    Some(2),
    Some(6000),
    None,
    PacketIntegrity::Guaranteed,
  ),
];

#[test]
fn the_tags_the_picture_and_the_cues_are_read_from_their_blocks() -> Result<(), Failure> {
  let whole = stream("stereo16")?;
  let mut format = reader(whole.to_vec())?;
  let Ok(metadata) = format.metadata();
  let Ok(current) = metadata.current();
  let revision = current.ok_or("no metadata")?;

  let Ok(value_10) = revision.tags();

  let tags: Vec<(String, String)> = value_10
    .iter()
    .map(|tag| (tag.key.clone(), tag.value.to_string()))
    .collect();
  let Ok(value_11) = revision.visuals();

  let visuals: Vec<(String, u32)> = value_11
    .iter()
    .map(|visual| {
      let Ok(length) = count(&visual.data);

      (visual.media_type.clone(), length)
    })
    .collect();
  let Ok(value_12) = format.cues();

  let cues: Vec<(u32, u64, u32)> = value_12
    .iter()
    .map(|cue| {
      let Ok(points) = count(&cue.points);

      (cue.index, cue.start_ts, points)
    })
    .collect();

  let Ok(expected) = expected_metadata();

  assert_eq!((tags, visuals, cues), expected);

  Ok(())
}

type Metadata = (
  Vec<(String, String)>,
  Vec<(String, u32)>,
  Vec<(u32, u64, u32)>,
);

fn expected_metadata() -> Result<Metadata, Never> {
  Ok((
    vec![
      ("TITLE".to_owned(), "A Song".to_owned()),
      ("ARTIST".to_owned(), "Nobody".to_owned()),
    ],
    vec![("image/png".to_owned(), 94)],
    vec![(1, 0, 1), (2, 5880, 1), (255, 7000, 0)],
  ))
}

#[test]
fn a_seek_lands_on_the_packet_that_holds_the_timestamp() -> Result<(), Failure> {
  let mut answers = Vec::new();

  for (name, bytes) in STREAMS {
    let mut format = reader(bytes.to_vec())?;
    let Ok(value_13) = format.tracks();

    let track = value_13.first().ok_or("no track")?;
    let frames = track.codec_params.n_frames.ok_or("no length")?;

    for ts in [
      0,
      1,
      frames.wrapping_div(3),
      frames.wrapping_div(2),
      frames.saturating_sub(1),
    ] {
      let seeked = format.seek(SeekMode::Accurate, SeekTo::TimeStamp { ts, track_id: 0 })?;
      let packet = format.next_packet()?;

      let Ok(value_14) = packet.ts();
      let Ok(value_15) = packet.dur();

      answers.push((name, ts, seeked.actual_ts, value_14, value_15));
    }

    let past = format.seek(
      SeekMode::Accurate,
      SeekTo::TimeStamp {
        ts: frames.saturating_add(1),
        track_id: 0,
      },
    );

    let refused = match past {
      Err(Error::SeekError(SeekErrorKind::OutOfRange)) => 1,
      Err(other) => return Err(Failure::from(other)),
      Ok(_) => 0,
    };

    answers.push((name, frames, refused, 0, 0));
  }

  assert_eq!(answers, GOLDEN_SEEKS);

  Ok(())
}

const GOLDEN_SEEKS: [(&str, u64, u64, u64, u64); 60] = [
  ("stereo16", 0, 0, 0, 1152),
  ("stereo16", 1, 0, 0, 1152),
  ("stereo16", 2333, 2304, 2304, 1152),
  ("stereo16", 3500, 3456, 3456, 1152),
  ("stereo16", 6999, 6912, 6912, 88),
  ("stereo16", 7000, 1, 0, 0),
  ("stereo24", 0, 0, 0, 4096),
  ("stereo24", 1, 0, 0, 4096),
  ("stereo24", 2000, 0, 0, 4096),
  ("stereo24", 3000, 0, 0, 4096),
  ("stereo24", 5999, 4096, 4096, 1904),
  ("stereo24", 6000, 1, 0, 0),
  ("mono8", 0, 0, 0, 200),
  ("mono8", 1, 0, 0, 200),
  ("mono8", 1000, 1000, 1000, 200),
  ("mono8", 1500, 1400, 1400, 200),
  ("mono8", 2999, 2800, 2800, 200),
  ("mono8", 3000, 1, 0, 0),
  ("parts16", 0, 0, 0, 1000),
  ("parts16", 1, 0, 0, 1000),
  ("parts16", 2000, 2000, 2000, 1000),
  ("parts16", 3000, 3000, 3000, 1000),
  ("parts16", 5999, 5000, 5000, 1000),
  ("parts16", 6000, 1, 0, 0),
  ("odd16", 0, 0, 0, 576),
  ("odd16", 1, 0, 0, 576),
  ("odd16", 833, 576, 576, 576),
  ("odd16", 1250, 1152, 1152, 576),
  ("odd16", 2499, 2304, 2304, 196),
  ("odd16", 2500, 1, 0, 0),
  ("kilo16", 0, 0, 0, 256),
  ("kilo16", 1, 0, 0, 256),
  ("kilo16", 833, 768, 768, 256),
  ("kilo16", 1250, 1024, 1024, 256),
  ("kilo16", 2499, 2304, 2304, 196),
  ("kilo16", 2500, 1, 0, 0),
  ("deep16", 0, 0, 0, 3000),
  ("deep16", 1, 0, 0, 3000),
  ("deep16", 2000, 0, 0, 3000),
  ("deep16", 3000, 3000, 3000, 3000),
  ("deep16", 5999, 3000, 3000, 3000),
  ("deep16", 6000, 1, 0, 0),
  ("sides16", 0, 0, 0, 500),
  ("sides16", 1, 0, 0, 500),
  ("sides16", 2000, 2000, 2000, 500),
  ("sides16", 3000, 3000, 3000, 500),
  ("sides16", 5999, 5500, 5500, 500),
  ("sides16", 6000, 1, 0, 0),
  ("left_side", 0, 0, 0, 480),
  ("left_side", 1, 0, 0, 480),
  ("left_side", 2000, 1920, 1920, 480),
  ("left_side", 3000, 2880, 2880, 480),
  ("left_side", 5999, 5760, 5760, 240),
  ("left_side", 6000, 1, 0, 0),
  ("right_side", 0, 0, 0, 480),
  ("right_side", 1, 0, 0, 480),
  ("right_side", 2000, 1920, 1920, 480),
  ("right_side", 3000, 2880, 2880, 480),
  ("right_side", 5999, 5760, 5760, 240),
  ("right_side", 6000, 1, 0, 0),
];

#[test]
fn a_stream_that_cannot_seek_can_still_be_read_forward_to_a_timestamp() -> Result<(), Failure> {
  let whole = stream("stereo16")?;
  let bytes = whole.to_vec();
  let Ok(forward_only) = ReadOnlySource::new(Cursor::new(bytes));
  let Ok(source) =
    MediaSourceStream::new(Box::new(forward_only), MediaSourceStreamOptions::default());
  let mut format = FlacReader::try_new(source, &FormatOptions::default())?;
  let forward = format.seek(
    SeekMode::Accurate,
    SeekTo::TimeStamp {
      ts: 3000,
      track_id: 0,
    },
  )?;
  let packet = format.next_packet()?;
  let backward = format.seek(
    SeekMode::Accurate,
    SeekTo::TimeStamp {
      ts: 10,
      track_id: 0,
    },
  );
  let Ok(value_16) = packet.ts();

  assert_eq!((forward.actual_ts, value_16), GOLDEN_FORWARD);
  assert!(matches!(
    backward,
    Err(Error::SeekError(SeekErrorKind::ForwardOnly))
  ));

  Ok(())
}

const GOLDEN_FORWARD: (u64, u64) = (2304, 2304);

#[test]
fn something_that_is_not_flac_is_refused() {
  let Ok(media) = source(b"RIFF....WAVEfmt ".to_vec());
  let refused = FlacReader::try_new(media, &FormatOptions::default());

  assert!(matches!(refused, Err(Error::Unsupported(_))));
}
