//! What a decoder is told about its stream before the first packet: the
//! sampling rate and the number of channels.
//!
//! In an MP4 file they come in an AudioSpecificConfig (ISO/IEC 14496-3, 1.6.2),
//! and in an ADTS stream they were already read out of the header of the first
//! frame. This decoder takes the low-complexity profile with one or two
//! channels and a frame of 1024 samples, which is what both of those say for
//! nearly every file there is, and says what it was asked for when it is not.
//! The configuration may also say the stream carries spectral band replication
//! or parametric stereo; the core is decoded as it is and what rides on top of
//! it is not asked for.

use console_core_audio_decoding_interfaces::audio::Channels;
use console_core_audio_decoding_interfaces::codecs::CodecParameters;
use console_core_audio_decoding_interfaces::errors::{Result, unsupported_error};
use console_core_byte_streams::{BitReaderLtr, FlagBit, ReadBitsLtr};

use crate::lane::{Constraint, Lane, Sure};

pub(crate) const AAC_SAMPLE_RATES: [u32; 16] = [
  96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350, 0, 0,
  0,
];

const AAC_CHANNELS: [u32; 8] = [0, 1, 2, 3, 4, 5, 6, 8];

#[derive(Clone, Copy, PartialEq, Eq)]
enum ObjectType {
  LowComplexity,
  Sbr,
  ParametricStereo,
  Other,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FrameLength {
  Long,
  Short,
}

pub(crate) struct Configuration {
  pub(crate) sample_rate: u32,
  pub(crate) channels: u32,
}

impl ObjectType {
  fn read<B: ReadBitsLtr>(bs: &mut B) -> Result<Self> {
    let index = bs.read_bits_leq32(5)?;

    match index {
      2 => Ok(ObjectType::LowComplexity),
      5 => Ok(ObjectType::Sbr),
      29 => Ok(ObjectType::ParametricStereo),
      31 => {
        let _extended = bs.read_bits_leq32(6)?;

        Ok(ObjectType::Other)
      }
      _ => Ok(ObjectType::Other),
    }
  }
}

fn read_sampling_frequency<B: ReadBitsLtr>(bs: &mut B) -> Result<u32> {
  let index = bs.read_bits_leq32(4)?;

  match index {
    15 => {
      let _explicit = bs.read_bits_leq32(20)?;

      Ok(0)
    }
    _ => AAC_SAMPLE_RATES.at(index),
  }
}

fn read_channel_config<B: ReadBitsLtr>(bs: &mut B) -> Result<u32> {
  let index = bs.read_bits_leq32(4)?;

  match index {
    0..=7 => AAC_CHANNELS.at(index),
    8.. => Ok(index),
  }
}

pub(crate) fn map_channels(channels: u32) -> Sure<Option<Channels>> {
  Ok(match channels {
    1 => Some(Channels::FRONT_LEFT),
    2 => Some(Channels::FRONT_LEFT | Channels::FRONT_RIGHT),
    3 => Some(Channels::FRONT_CENTRE | Channels::FRONT_LEFT | Channels::FRONT_RIGHT),
    4 => Some(
      Channels::FRONT_CENTRE
        | Channels::FRONT_LEFT
        | Channels::FRONT_RIGHT
        | Channels::REAR_CENTRE,
    ),
    5 => Some(
      Channels::FRONT_CENTRE
        | Channels::FRONT_LEFT
        | Channels::FRONT_RIGHT
        | Channels::SIDE_LEFT
        | Channels::SIDE_RIGHT,
    ),
    6 => Some(
      Channels::FRONT_CENTRE
        | Channels::FRONT_LEFT
        | Channels::FRONT_RIGHT
        | Channels::SIDE_LEFT
        | Channels::SIDE_RIGHT
        | Channels::LFE1,
    ),
    8 => Some(
      Channels::FRONT_CENTRE
        | Channels::FRONT_LEFT
        | Channels::FRONT_RIGHT
        | Channels::SIDE_LEFT
        | Channels::SIDE_RIGHT
        | Channels::FRONT_LEFT_WIDE
        | Channels::FRONT_RIGHT_WIDE
        | Channels::LFE1,
    ),
    _ => None,
  })
}

impl Configuration {
  pub(crate) fn from_audio_specific_config(bytes: &[u8]) -> Result<Self> {
    Constraint::from(bytes.len() >= 2).require()?;

    let Ok(mut bs) = BitReaderLtr::new(bytes);
    let first = ObjectType::read(&mut bs)?;
    let sample_rate = read_sampling_frequency(&mut bs)?;

    Constraint::from(sample_rate > 0).require()?;

    let channels = read_channel_config(&mut bs)?;
    let profile = match first {
      ObjectType::Sbr | ObjectType::ParametricStereo => {
        let _extension_rate = read_sampling_frequency(&mut bs)?;

        ObjectType::read(&mut bs)?
      }
      ObjectType::LowComplexity | ObjectType::Other => first,
    };

    match profile {
      ObjectType::LowComplexity => {}
      ObjectType::Sbr | ObjectType::ParametricStereo | ObjectType::Other => {
        return unsupported_error("aac: object type");
      }
    }

    let short_frame = bs.read_flag()?;
    let frame = match short_frame {
      FlagBit::One => FrameLength::Short,
      FlagBit::Zero => FrameLength::Long,
    };

    let depends_on_core = bs.read_flag()?;

    match depends_on_core {
      FlagBit::One => {
        let _delay = bs.read_bits_leq32(14)?;
      }
      FlagBit::Zero => {}
    }

    let extension_flag = bs.read_flag()?;

    match channels {
      0 => return unsupported_error("aac: program config element"),
      _ => {}
    }

    match extension_flag {
      FlagBit::One => {
        let version3_extensions = bs.read_flag()?;

        match version3_extensions {
          FlagBit::One => return unsupported_error("aac: version3 extensions"),
          FlagBit::Zero => {}
        }
      }
      FlagBit::Zero => {}
    }

    Configuration {
      sample_rate,
      channels,
    }
    .supported(frame)
  }

  pub(crate) fn from_parameters(params: &CodecParameters) -> Result<Self> {
    let sample_rate = match params.sample_rate {
      Some(rate) => rate,
      None => return unsupported_error("aac: sample rate is required"),
    };
    let Ok(channels) = match (params.channels, params.channel_layout) {
      (Some(channels), _) => channels.count(),
      (None, Some(layout)) => {
        let Ok(channels) = layout.into_channels();

        channels.count()
      }
      (None, None) => return unsupported_error("aac: channels or channel layout is required"),
    };

    match channels {
      0 => unsupported_error("aac: channels or channel layout is required"),
      _ => Configuration {
        sample_rate,
        channels,
      }
      .supported(FrameLength::Long),
    }
  }

  fn supported(self, frame: FrameLength) -> Result<Self> {
    match (frame, self.channels) {
      (FrameLength::Long, 0..=2) => Ok(self),
      (FrameLength::Short, _) | (FrameLength::Long, 3..) => {
        unsupported_error("aac: aac too complex")
      }
    }
  }
}
