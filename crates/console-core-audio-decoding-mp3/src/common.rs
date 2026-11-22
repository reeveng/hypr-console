//! MPEG audio decoder configuration derived from frame headers.

use console_core_audio_decoding_interfaces::audio::{Channels, Layout, SignalSpec};
use console_core_audio_decoding_interfaces::codecs::{
  CODEC_TYPE_MP1, CODEC_TYPE_MP2, CODEC_TYPE_MP3, CodecType,
};
use console_core_audio_decoding_interfaces::errors::{Result, end_of_stream_error};
use console_core_iteration::Endless;
pub use console_core_mpeg_audio_frames::header::{
  ChannelMode, FrameHeader, Layer, Protection, Rate, Switch, Version,
};
use console_core_never::Never;

use crate::layer3::tables::{LONG_BANDS, MIXED_BANDS, MIXED_SWITCH_POINTS, SHORT_BANDS};

pub type Infallible<T> = std::result::Result<T, Never>;

pub fn finished<T>(ended: std::result::Result<Result<T>, Endless>) -> Result<T> {
  match ended {
    Ok(ended_2) => ended_2,
    Err(Endless) => end_of_stream_error(),
  }
}

#[derive(Copy, Clone, Debug)]
pub struct Bands {
  pub long: &'static [u32; 23],
  pub short: &'static [u32; 40],
  pub mixed: &'static [u32],
  pub mixed_switch: u32,
}

pub fn codec(layer: Layer) -> Infallible<CodecType> {
  Ok(match layer {
    Layer::One => CODEC_TYPE_MP1,
    Layer::Two => CODEC_TYPE_MP2,
    Layer::Three => CODEC_TYPE_MP3,
  })
}

pub fn bands(rate: Rate) -> Infallible<Bands> {
  let [
    long44,
    long48,
    long32,
    long22,
    long24,
    long16,
    long11,
    long12,
    long8,
  ] = &LONG_BANDS;
  let [
    short44,
    short48,
    short32,
    short22,
    short24,
    short16,
    short11,
    short12,
    short8,
  ] = &SHORT_BANDS;
  let [
    mixed44,
    mixed48,
    mixed32,
    mixed22,
    mixed24,
    mixed16,
    mixed11,
    mixed12,
    mixed8,
  ] = MIXED_BANDS;
  let [
    switch44,
    switch48,
    switch32,
    switch22,
    switch24,
    switch16,
    switch11,
    switch12,
    switch8,
  ] = MIXED_SWITCH_POINTS;

  Ok(match rate {
    Rate::Hz44100 => Bands {
      long: long44,
      short: short44,
      mixed: mixed44,
      mixed_switch: switch44,
    },
    Rate::Hz48000 => Bands {
      long: long48,
      short: short48,
      mixed: mixed48,
      mixed_switch: switch48,
    },
    Rate::Hz32000 => Bands {
      long: long32,
      short: short32,
      mixed: mixed32,
      mixed_switch: switch32,
    },
    Rate::Hz22050 => Bands {
      long: long22,
      short: short22,
      mixed: mixed22,
      mixed_switch: switch22,
    },
    Rate::Hz24000 => Bands {
      long: long24,
      short: short24,
      mixed: mixed24,
      mixed_switch: switch24,
    },
    Rate::Hz16000 => Bands {
      long: long16,
      short: short16,
      mixed: mixed16,
      mixed_switch: switch16,
    },
    Rate::Hz11025 => Bands {
      long: long11,
      short: short11,
      mixed: mixed11,
      mixed_switch: switch11,
    },
    Rate::Hz12000 => Bands {
      long: long12,
      short: short12,
      mixed: mixed12,
      mixed_switch: switch12,
    },
    Rate::Hz8000 => Bands {
      long: long8,
      short: short8,
      mixed: mixed8,
      mixed_switch: switch8,
    },
  })
}

pub fn channel_set(mode: ChannelMode) -> Infallible<Channels> {
  Ok(match mode {
    ChannelMode::Mono => Channels::FRONT_LEFT,
    ChannelMode::DualMono | ChannelMode::Stereo | ChannelMode::JointStereo { .. } => {
      Channels::FRONT_LEFT | Channels::FRONT_RIGHT
    }
  })
}

pub fn spec(header: &FrameHeader) -> Infallible<SignalSpec> {
  let layout = match header.channel_mode {
    ChannelMode::Mono => Layout::Mono,
    ChannelMode::DualMono | ChannelMode::Stereo | ChannelMode::JointStereo { .. } => {
      Layout::Stereo
    }
  };

  let Ok(value_1) = SignalSpec::new_with_layout(header.sample_rate, layout);

  Ok(value_1)
}
