//! The Opus mapping: the first packet is `OpusHead` with a pre-skip and a channel mapping, the
//! second is `OpusTags`, and every packet after is audio whose table-of-contents byte says how
//! long it plays.

use std::cmp::Ordering;

use console_core_audio_decoding_interfaces::audio::Channels;
use console_core_audio_decoding_interfaces::codecs::{CODEC_TYPE_OPUS, CodecParameters};
use console_core_audio_decoding_interfaces::errors::Result;
use console_core_audio_decoding_interfaces::meta::MetadataBuilder;
use console_core_audio_decoding_interfaces::units::TimeBase;
use console_core_audio_decoding_tags::vorbis;
use console_core_byte_streams::{BufReader, ReadBytes};
use console_core_number_conversion::{fitted, index};

use super::MapResult;
use crate::stepping::Certain;

const OGG_OPUS_MIN_IDENTIFICATION_PACKET_SIZE: u32 = 19;

const OGG_OPUS_MAGIC_SIGNATURE: [u8; 8] = *b"OpusHead";

const OGG_OPUS_COMMENT_SIGNATURE: [u8; 8] = *b"OpusTags";

const OGG_OPUS_MAPPING_VERSION_MAX: u8 = 0x0f;

const FRAME_DURATIONS: [u64; 32] = [
  480, 960, 1920, 2880, 480, 960, 1920, 2880, 480, 960, 1920, 2880, 480, 960, 480, 960, 120, 240,
  480, 960, 120, 240, 480, 960, 120, 240, 480, 960, 120, 240, 480, 960,
];

enum Expecting {
  Comment,
  Audio,
}

pub struct OpusMapper {
  pub(super) codec_params: CodecParameters,
  expecting: Expecting,
}

fn vorbis_order(channel_count: u8) -> Certain<Option<Channels>> {
  Ok(match channel_count {
    1 => Some(Channels::FRONT_LEFT),
    2 => Some(Channels::FRONT_LEFT | Channels::FRONT_RIGHT),
    3 => Some(Channels::FRONT_LEFT | Channels::FRONT_CENTRE | Channels::FRONT_RIGHT),
    4 => Some(
      Channels::FRONT_LEFT | Channels::FRONT_RIGHT | Channels::REAR_LEFT | Channels::REAR_RIGHT,
    ),
    5 => Some(
      Channels::FRONT_LEFT
        | Channels::FRONT_CENTRE
        | Channels::FRONT_RIGHT
        | Channels::REAR_LEFT
        | Channels::REAR_RIGHT,
    ),
    6 => Some(
      Channels::FRONT_LEFT
        | Channels::FRONT_CENTRE
        | Channels::FRONT_RIGHT
        | Channels::REAR_LEFT
        | Channels::REAR_RIGHT
        | Channels::LFE1,
    ),
    7 => Some(
      Channels::FRONT_LEFT
        | Channels::FRONT_CENTRE
        | Channels::FRONT_RIGHT
        | Channels::SIDE_LEFT
        | Channels::SIDE_RIGHT
        | Channels::REAR_CENTRE
        | Channels::LFE1,
    ),
    8 => Some(
      Channels::FRONT_LEFT
        | Channels::FRONT_CENTRE
        | Channels::FRONT_RIGHT
        | Channels::SIDE_LEFT
        | Channels::SIDE_RIGHT
        | Channels::REAR_LEFT
        | Channels::REAR_RIGHT
        | Channels::LFE1,
    ),
    _unmapped => None,
  })
}

pub fn detect(buf: &[u8]) -> Result<Option<OpusMapper>> {
  let Ok(len) = fitted::<_, u32>(buf.len());

  match len.cmp(&OGG_OPUS_MIN_IDENTIFICATION_PACKET_SIZE) {
    Ordering::Less => return Ok(None),
    Ordering::Equal | Ordering::Greater => {}
  }

  let Ok(mut reader) = BufReader::new(buf);
  let mut identification = [0u8; 19];

  reader.read_buf_exact(&mut identification)?;

  let [
    m0,
    m1,
    m2,
    m3,
    m4,
    m5,
    m6,
    m7,
    version,
    channel_count,
    pre_skip_low,
    pre_skip_high,
    _rate0,
    _rate1,
    _rate2,
    _rate3,
    _gain_low,
    _gain_high,
    channel_mapping,
  ] = identification;

  match [m0, m1, m2, m3, m4, m5, m6, m7] {
    OGG_OPUS_MAGIC_SIGNATURE => {}
    _mismatch => return Ok(None),
  }

  match version.cmp(&OGG_OPUS_MAPPING_VERSION_MAX) {
    Ordering::Greater => return Ok(None),
    Ordering::Less | Ordering::Equal => {}
  }

  let found = match (channel_mapping, channel_count) {
    (0, 1) => Some(Channels::FRONT_LEFT),
    (0, 2) => Some(Channels::FRONT_LEFT | Channels::FRONT_RIGHT),
    (1, count) => {
      let Ok(ordered) = vorbis_order(count);

      ordered
    }
    (_reserved, _count) => None,
  };

  let channels = match found {
    Some(channels) => channels,
    None => return Ok(None),
  };

  let pre_skip = u16::from_le_bytes([pre_skip_low, pre_skip_high]);
  let time_base = TimeBase::new(1, 48_000)?;
  let Ok(mut codec_params) = CodecParameters::new();

  let Ok(_configured_for_codec) = codec_params.for_codec(CODEC_TYPE_OPUS);
  let Ok(_configured_with_delay) = codec_params.with_delay(u32::from(pre_skip));
  let Ok(_configured_with_sample_rate) = codec_params.with_sample_rate(48_000);
  let Ok(_configured_with_time_base) = codec_params.with_time_base(time_base);
  let Ok(_configured_with_channels) = codec_params.with_channels(channels);
  let Ok(_configured_with_extra_data) = codec_params.with_extra_data(Box::from(buf));

  Ok(Some(OpusMapper {
    codec_params,
    expecting: Expecting::Comment,
  }))
}

pub fn packet_duration(packet: &[u8]) -> Certain<u64> {
  let (toc, following) = match packet {
    [] => return Ok(0),
    [toc] => (*toc, None),
    [toc, following, ..] => (*toc, Some(*following)),
  };

  let Ok(configuration) = index(toc.wrapping_shr(3));

  let frame_duration = match FRAME_DURATIONS.get(configuration) {
    Some(duration) => *duration,
    None => return Ok(0),
  };

  let frames = match toc & 0b11 {
    0 => 1,
    1 | 2 => 2,
    _counted => match following {
      Some(byte) => u64::from(byte & 0b11111),
      None => return Ok(0),
    },
  };

  Ok(frame_duration.saturating_mul(frames))
}

impl OpusMapper {
  pub fn map_packet(&mut self, packet: &[u8]) -> Result<MapResult> {
    match self.expecting {
      Expecting::Audio => {
        let Ok(dur) = packet_duration(packet);

        Ok(MapResult::Audio { dur })
      }
      Expecting::Comment => {
        let Ok(mut reader) = BufReader::new(packet);
        let mut signature = [0u8; 8];

        reader.read_buf_exact(&mut signature)?;

        match signature {
          OGG_OPUS_COMMENT_SIGNATURE => {
            let Ok(mut builder) = MetadataBuilder::new();

            vorbis::read_comment_no_framing(&mut reader, &mut builder)?;

            self.expecting = Expecting::Audio;

            let Ok(value_1) = builder.metadata();

            Ok(MapResult::Metadata(value_1))
          }
          _mismatch => Ok(MapResult::Skipped),
        }
      }
    }
  }
}
