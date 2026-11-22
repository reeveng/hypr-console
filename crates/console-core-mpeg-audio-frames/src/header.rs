//! Finding a frame header in a stream, and reading what it says.
//!
//! The four bytes of a header begin with eleven set bits that random data
//! shows up with often, so a word that looks synced is first held to the
//! fields that cannot take every value, and only then read.
//! `parse_frame_header` takes a word whose sync marker has already been checked.

use console_core_audio_decoding_interfaces::errors::{
  Error, Result, decode_error, unsupported_error,
};
use console_core_number_conversion::index;

use crate::Infallible;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Version {
  Mpeg2p5,
  Mpeg2,
  Mpeg1,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Layer {
  One,
  Two,
  Three,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Rate {
  Hz44100,
  Hz48000,
  Hz32000,
  Hz22050,
  Hz24000,
  Hz16000,
  Hz11025,
  Hz12000,
  Hz8000,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Switch {
  On,
  Off,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ChannelMode {
  Mono,
  DualMono,
  Stereo,
  JointStereo { mid_side: Switch, intensity: Switch },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Protection {
  Crc,
  None,
}

#[derive(Debug)]
pub struct FrameHeader {
  pub version: Version,
  pub layer: Layer,
  pub bitrate: u32,
  pub sample_rate: u32,
  pub rate: Rate,
  pub channel_mode: ChannelMode,
  pub channels: u32,
  pub protection: Protection,
  pub frame_size: u32,
  pub granules: u32,
  pub duration: u64,
  pub side_info_len: u32,
  pub intensity_bound: u32,
}

pub const MPEG_HEADER_LEN: u32 = 4;

const SYNC_MASK: u32 = 0xffe0_0000;

const BIT_RATES_MPEG1_L1: [u32; 15] = [
  0, 32_000, 64_000, 96_000, 128_000, 160_000, 192_000, 224_000, 256_000, 288_000, 320_000,
  352_000, 384_000, 416_000, 448_000,
];

const BIT_RATES_MPEG1_L2: [u32; 15] = [
  0, 32_000, 48_000, 56_000, 64_000, 80_000, 96_000, 112_000, 128_000, 160_000, 192_000, 224_000,
  256_000, 320_000, 384_000,
];

const BIT_RATES_MPEG2_L1: [u32; 15] = [
  0, 32_000, 48_000, 56_000, 64_000, 80_000, 96_000, 112_000, 128_000, 144_000, 160_000, 176_000,
  192_000, 224_000, 256_000,
];

const BIT_RATES_MPEG1_L3: [u32; 15] = [
  0, 32_000, 40_000, 48_000, 56_000, 64_000, 80_000, 96_000, 112_000, 128_000, 160_000, 192_000,
  224_000, 256_000, 320_000,
];

const BIT_RATES_MPEG2_L23: [u32; 15] = [
  0, 8_000, 16_000, 24_000, 32_000, 40_000, 48_000, 56_000, 64_000, 80_000, 96_000, 112_000,
  128_000, 144_000, 160_000,
];

pub fn synced(word: u32) -> Infallible<Option<u32>> {
  let version = word.wrapping_shr(19) & 0x3;
  let layer = word.wrapping_shr(17) & 0x3;
  let bitrate = word.wrapping_shr(12) & 0xf;
  let sample_rate = word.wrapping_shr(10) & 0x3;

  Ok(
    match (word & SYNC_MASK, version, layer, bitrate, sample_rate) {
      (SYNC_MASK, 0 | 2 | 3, 1..=3, 0..=14, 0..=2) => Some(word),
      _invalid => None,
    },
  )
}

fn version(header: u32) -> Result<Version> {
  match header.wrapping_shr(19) & 0x3 {
    0b00 => Ok(Version::Mpeg2p5),
    0b10 => Ok(Version::Mpeg2),
    0b11 => Ok(Version::Mpeg1),
    _ => decode_error("mpa: invalid MPEG version"),
  }
}

fn layer(header: u32) -> Result<Layer> {
  match header.wrapping_shr(17) & 0x3 {
    0b01 => Ok(Layer::Three),
    0b10 => Ok(Layer::Two),
    0b11 => Ok(Layer::One),
    _ => decode_error("mpa: invalid MPEG layer"),
  }
}

fn bitrate(header: u32, version: Version, layer: Layer) -> Result<u32> {
  let code = header.wrapping_shr(12) & 0xf;
  let Ok(position) = index(code);

  let table = match (version, layer) {
    (Version::Mpeg1, Layer::One) => &BIT_RATES_MPEG1_L1,
    (Version::Mpeg1, Layer::Two) => &BIT_RATES_MPEG1_L2,
    (Version::Mpeg1, Layer::Three) => &BIT_RATES_MPEG1_L3,
    (Version::Mpeg2 | Version::Mpeg2p5, Layer::One) => &BIT_RATES_MPEG2_L1,
    (Version::Mpeg2 | Version::Mpeg2p5, Layer::Two | Layer::Three) => &BIT_RATES_MPEG2_L23,
  };

  match (code, table.get(position)) {
    (0b0000, _) => unsupported_error("mpa: free bit-rate is not supported"),
    (0b1111, _) => decode_error("mpa: invalid bit-rate"),
    (_, Some(&rate)) => Ok(rate),
    (_, None) => decode_error("mpa: invalid bit-rate"),
  }
}

fn hertz(rate: Rate) -> Infallible<u32> {
  Ok(match rate {
    Rate::Hz44100 => 44_100,
    Rate::Hz48000 => 48_000,
    Rate::Hz32000 => 32_000,
    Rate::Hz22050 => 22_050,
    Rate::Hz24000 => 24_000,
    Rate::Hz16000 => 16_000,
    Rate::Hz11025 => 11_025,
    Rate::Hz12000 => 12_000,
    Rate::Hz8000 => 8_000,
  })
}

fn rate(header: u32, version: Version) -> Result<Rate> {
  match (header.wrapping_shr(10) & 0x3, version) {
    (0b00, Version::Mpeg1) => Ok(Rate::Hz44100),
    (0b01, Version::Mpeg1) => Ok(Rate::Hz48000),
    (0b10, Version::Mpeg1) => Ok(Rate::Hz32000),
    (0b00, Version::Mpeg2) => Ok(Rate::Hz22050),
    (0b01, Version::Mpeg2) => Ok(Rate::Hz24000),
    (0b10, Version::Mpeg2) => Ok(Rate::Hz16000),
    (0b00, Version::Mpeg2p5) => Ok(Rate::Hz11025),
    (0b01, Version::Mpeg2p5) => Ok(Rate::Hz12000),
    (0b10, Version::Mpeg2p5) => Ok(Rate::Hz8000),
    _ => decode_error("mpa: invalid sample rate"),
  }
}

fn channel_mode(header: u32) -> Result<ChannelMode> {
  let switch = |bit: u32| match header & bit {
    0 => Switch::Off,
    _ => Switch::On,
  };

  match header.wrapping_shr(6) & 0x3 {
    0b00 => Ok(ChannelMode::Stereo),
    0b10 => Ok(ChannelMode::DualMono),
    0b11 => Ok(ChannelMode::Mono),
    0b01 => Ok(ChannelMode::JointStereo {
      mid_side: switch(0x20),
      intensity: switch(0x10),
    }),
    _ => decode_error("mpa: invalid channel mode"),
  }
}

pub fn parse_frame_header(header: u32) -> Result<FrameHeader> {
  let layer = layer(header)?;
  let version = version(header)?;
  let bitrate = bitrate(header, version, layer)?;
  let rate = rate(header, version)?;
  let channel_mode = channel_mode(header)?;
  let Ok(sample_rate) = hertz(rate);

  let protection = match header & 0x1_0000 {
    0 => Protection::Crc,
    _ => Protection::None,
  };

  let (factor, granules, duration, slot_size): (u32, u32, u64, u32) = match (layer, version) {
    (Layer::One, _) => (12, 1, 384, 4),
    (Layer::Two, _) => (144, 1, 1152, 1),
    (Layer::Three, Version::Mpeg1) => (144, 2, 1152, 1),
    (Layer::Three, Version::Mpeg2 | Version::Mpeg2p5) => (72, 1, 576, 1),
  };

  match (layer, channel_mode, bitrate) {
    (Layer::Two, ChannelMode::Mono, 224_000 | 256_000 | 320_000 | 384_000) => {
      return decode_error("mpa: invalid Layer 2 bitrate for mono channel mode");
    }
    (
      Layer::Two,
      ChannelMode::DualMono | ChannelMode::Stereo | ChannelMode::JointStereo { .. },
      32_000 | 48_000 | 56_000 | 80_000,
    ) => {
      return decode_error("mpa: invalid Layer 2 bitrate for non-mono channel mode");
    }
    (Layer::One | Layer::Two | Layer::Three, _, _) => {}
  }

  let padding = header.wrapping_shr(9) & 0x1;

  let slots = match factor.wrapping_mul(bitrate).checked_div(sample_rate) {
    Some(slots) => slots.wrapping_add(padding).wrapping_mul(slot_size),
    None => return decode_error("mpa: invalid sample rate"),
  };

  let (channels, side_info_len) = match (version, channel_mode) {
    (Version::Mpeg1, ChannelMode::Mono) => (1, 17),
    (
      Version::Mpeg1,
      ChannelMode::DualMono | ChannelMode::Stereo | ChannelMode::JointStereo { .. },
    ) => (2, 32),
    (Version::Mpeg2 | Version::Mpeg2p5, ChannelMode::Mono) => (1, 9),
    (
      Version::Mpeg2 | Version::Mpeg2p5,
      ChannelMode::DualMono | ChannelMode::Stereo | ChannelMode::JointStereo { .. },
    ) => (2, 17),
  };

  Ok(FrameHeader {
    version,
    layer,
    bitrate,
    sample_rate,
    rate,
    channel_mode,
    channels,
    protection,
    frame_size: slots.saturating_sub(MPEG_HEADER_LEN),
    granules,
    duration,
    side_info_len,
    intensity_bound: (header.wrapping_shr(4) & 0x3)
      .saturating_add(1)
      .saturating_mul(4),
  })
}

pub fn parse_candidate(word: u32) -> Result<Option<FrameHeader>> {
  match parse_frame_header(word) {
    Ok(header) => Ok(Some(header)),
    Err(Error::DecodeError(_) | Error::Unsupported(_)) => Ok(None),
    Err(
      fault @ (Error::IoError(_)
      | Error::SeekError(_)
      | Error::LimitError(_)
      | Error::ResetRequired),
    ) => Err(fault),
  }
}
