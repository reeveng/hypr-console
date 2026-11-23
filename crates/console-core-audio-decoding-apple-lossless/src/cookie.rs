//! The "magic cookie": the codec-specific configuration a container carries
//! beside an ALAC stream, and the channel layout it names or implies.
//!
//! The reserved and rate fields that nothing here uses are read past rather
//! than kept, so what the struct holds is what the decoder asks of it.

use core::cmp::Ordering;

use console_core_audio_decoding_interfaces::audio::Channels;
use console_core_byte_streams::{FiniteStream, ReadBytes};
use console_core_never::Never;

use crate::fault::DecodeError;

pub(crate) const MAX_FRAME_LENGTH: u32 = 1 << 20;

const SHORT: u64 = 24;

const LONG: u64 = 48;

const LAYOUT_INFO_SIZE: u32 = 24;

const LAYOUT_INFO_ID: [u8; 4] = *b"chan";

const LAYOUT_INFO_VERSION: u32 = 0;

const SUPPORTED_VERSION: u8 = 0;

const MONO: u32 = 0x64_0001;
const STEREO: u32 = 0x65_0002;
const MPEG_3_0_B: u32 = 0x71_0003;
const MPEG_4_0_B: u32 = 0x74_0004;
const MPEG_5_0_D: u32 = 0x78_0005;
const MPEG_5_1_D: u32 = 0x7c_0006;
const AAC_6_1: u32 = 0x8e_0007;
const MPEG_7_1_B: u32 = 0x7f_0008;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChannelLayout {
  Mono,
  Stereo,
  Mpeg3p0B,
  Mpeg4p0B,
  Mpeg5p0D,
  Mpeg5p1D,
  Aac6p1,
  Mpeg7p1B,
}

impl ChannelLayout {
  fn named(tag: u32) -> Result<ChannelLayout, DecodeError> {
    match tag {
      MONO => Ok(ChannelLayout::Mono),
      STEREO => Ok(ChannelLayout::Stereo),
      MPEG_3_0_B => Ok(ChannelLayout::Mpeg3p0B),
      MPEG_4_0_B => Ok(ChannelLayout::Mpeg4p0B),
      MPEG_5_0_D => Ok(ChannelLayout::Mpeg5p0D),
      MPEG_5_1_D => Ok(ChannelLayout::Mpeg5p1D),
      AAC_6_1 => Ok(ChannelLayout::Aac6p1),
      MPEG_7_1_B => Ok(ChannelLayout::Mpeg7p1B),
      _ => Err(DecodeError::LayoutTag),
    }
  }

  fn implied(channels: u8) -> Result<ChannelLayout, DecodeError> {
    match channels {
      1 => Ok(ChannelLayout::Mono),
      2 => Ok(ChannelLayout::Stereo),
      3 => Ok(ChannelLayout::Mpeg3p0B),
      4 => Ok(ChannelLayout::Mpeg4p0B),
      5 => Ok(ChannelLayout::Mpeg5p0D),
      6 => Ok(ChannelLayout::Mpeg5p1D),
      7 => Ok(ChannelLayout::Aac6p1),
      8 => Ok(ChannelLayout::Mpeg7p1B),
      _ => Err(DecodeError::ChannelCount),
    }
  }

  pub(crate) fn channel_map(self) -> Result<&'static [u32], Never> {
    Ok(match self {
      ChannelLayout::Mono => &[0],
      ChannelLayout::Stereo => &[0, 1],
      ChannelLayout::Mpeg3p0B => &[2, 0, 1],
      ChannelLayout::Mpeg4p0B => &[2, 0, 1, 3],
      ChannelLayout::Mpeg5p0D => &[2, 0, 1, 3, 4],
      ChannelLayout::Mpeg5p1D => &[2, 0, 1, 4, 5, 3],
      ChannelLayout::Aac6p1 => &[2, 0, 1, 5, 6, 4, 3],
      ChannelLayout::Mpeg7p1B => &[2, 4, 5, 0, 1, 6, 7, 3],
    })
  }

  pub(crate) fn channels(self) -> Result<Channels, Never> {
    Ok(match self {
      ChannelLayout::Mono => Channels::FRONT_LEFT,
      ChannelLayout::Stereo => Channels::FRONT_LEFT | Channels::FRONT_RIGHT,
      ChannelLayout::Mpeg3p0B => {
        Channels::FRONT_CENTRE | Channels::FRONT_LEFT | Channels::FRONT_RIGHT
      }
      ChannelLayout::Mpeg4p0B => {
        Channels::FRONT_CENTRE
          | Channels::FRONT_LEFT
          | Channels::FRONT_RIGHT
          | Channels::REAR_CENTRE
      }
      ChannelLayout::Mpeg5p0D => {
        Channels::FRONT_CENTRE
          | Channels::FRONT_LEFT
          | Channels::FRONT_RIGHT
          | Channels::SIDE_LEFT
          | Channels::SIDE_RIGHT
      }
      ChannelLayout::Mpeg5p1D => {
        Channels::FRONT_CENTRE
          | Channels::FRONT_LEFT
          | Channels::FRONT_RIGHT
          | Channels::SIDE_LEFT
          | Channels::SIDE_RIGHT
          | Channels::LFE1
      }
      ChannelLayout::Aac6p1 => {
        Channels::FRONT_CENTRE
          | Channels::FRONT_LEFT
          | Channels::FRONT_RIGHT
          | Channels::SIDE_LEFT
          | Channels::SIDE_RIGHT
          | Channels::REAR_CENTRE
          | Channels::LFE1
      }
      ChannelLayout::Mpeg7p1B => {
        Channels::FRONT_CENTRE
          | Channels::FRONT_LEFT_CENTRE
          | Channels::FRONT_RIGHT_CENTRE
          | Channels::FRONT_LEFT
          | Channels::FRONT_RIGHT
          | Channels::SIDE_LEFT
          | Channels::SIDE_RIGHT
          | Channels::LFE1
      }
    })
  }
}

#[derive(Debug)]
pub(crate) struct MagicCookie {
  pub(crate) frame_length: u32,
  pub(crate) bit_depth: u8,
  pub(crate) pb: u8,
  pub(crate) mb: u8,
  pub(crate) kb: u8,
  pub(crate) num_channels: u8,
  pub(crate) sample_rate: u32,
  pub(crate) channel_layout: ChannelLayout,
}

struct Fixed {
  frame_length: u32,
  version: u8,
  bit_depth: u8,
  pb: u8,
  mb: u8,
  kb: u8,
  num_channels: u8,
  sample_rate: u32,
}

impl Fixed {
  fn read<B: ReadBytes>(reader: &mut B) -> Result<Fixed, DecodeError> {
    let frame_length = reader.read_be_u32()?;
    let version = reader.read_u8()?;
    let bit_depth = reader.read_u8()?;
    let pb = reader.read_u8()?;
    let mb = reader.read_u8()?;
    let kb = reader.read_u8()?;
    let num_channels = reader.read_u8()?;
    let _max_run = reader.read_be_u16()?;
    let _max_frame_bytes = reader.read_be_u32()?;
    let _avg_bit_rate = reader.read_be_u32()?;
    let sample_rate = reader.read_be_u32()?;

    Ok(Fixed {
      frame_length,
      version,
      bit_depth,
      pb,
      mb,
      kb,
      num_channels,
      sample_rate,
    })
  }
}

fn explicit_layout<B: ReadBytes>(
  reader: &mut B,
  num_channels: u8,
) -> Result<ChannelLayout, DecodeError> {
  let size = reader.read_be_u32()?;

  match size {
    LAYOUT_INFO_SIZE => {}
    _ => return Err(DecodeError::LayoutSize),
  }

  let id = reader.read_quad_bytes()?;

  match id {
    LAYOUT_INFO_ID => {}
    _unexpected_id => return Err(DecodeError::LayoutId),
  }

  let version = reader.read_be_u32()?;

  match version {
    LAYOUT_INFO_VERSION => {}
    _ => return Err(DecodeError::LayoutVersion),
  }

  let tag = reader.read_be_u32()?;
  let layout = ChannelLayout::named(tag)?;
  let Ok(implied) = layout.channels();

  let Ok(value_1) = implied.count();

  match u32::from(num_channels).cmp(&value_1) {
    Ordering::Equal => {}
    Ordering::Less | Ordering::Greater => return Err(DecodeError::LayoutChannels),
  }

  let first_reserved = reader.read_be_u32()?;
  let second_reserved = reader.read_be_u32()?;

  match (first_reserved, second_reserved) {
    (0, 0) => Ok(layout),
    _ => Err(DecodeError::LayoutReserved),
  }
}

impl MagicCookie {
  pub(crate) fn try_read<B: ReadBytes + FiniteStream>(
    reader: &mut B,
  ) -> Result<MagicCookie, DecodeError> {
    let length = reader.byte_len();

    match length {
      SHORT | LONG => {}
      _ => return Err(DecodeError::CookieSize),
    }

    let fixed = Fixed::read(reader)?;

    match fixed.version {
      SUPPORTED_VERSION => {}
      _ => return Err(DecodeError::Version),
    }

    match fixed.bit_depth {
      0..=32 => {}
      _ => return Err(DecodeError::BitDepth),
    }

    match fixed.frame_length {
      0..=MAX_FRAME_LENGTH => {}
      _ => return Err(DecodeError::FrameLength),
    }

    let implied = ChannelLayout::implied(fixed.num_channels)?;

    let named = match length {
      LONG => explicit_layout(reader, fixed.num_channels),
      _ => Ok(implied),
    };
    let channel_layout = named?;

    Ok(MagicCookie {
      frame_length: fixed.frame_length,
      bit_depth: fixed.bit_depth,
      pb: fixed.pb,
      mb: fixed.mb,
      kb: fixed.kb,
      num_channels: fixed.num_channels,
      sample_rate: fixed.sample_rate,
      channel_layout,
    })
  }
}
