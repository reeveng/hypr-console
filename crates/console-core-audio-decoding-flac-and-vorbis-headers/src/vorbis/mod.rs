//! The channels Vorbis assigns to each count of them, in its mapping 0.

use console_core_audio_decoding_interfaces::audio::Channels;
use console_core_audio_decoding_interfaces::errors::{Result, unsupported_error};

pub fn vorbis_channels_to_channels(num_channels: u8) -> Result<Channels> {
  let channels = match num_channels {
    1 => Channels::FRONT_LEFT,
    2 => Channels::FRONT_LEFT | Channels::FRONT_RIGHT,
    3 => Channels::FRONT_LEFT | Channels::FRONT_CENTRE | Channels::FRONT_RIGHT,
    4 => {
      Channels::FRONT_LEFT | Channels::FRONT_RIGHT | Channels::REAR_LEFT | Channels::REAR_RIGHT
    }
    5 => {
      Channels::FRONT_LEFT
        | Channels::FRONT_CENTRE
        | Channels::FRONT_RIGHT
        | Channels::REAR_LEFT
        | Channels::REAR_RIGHT
    }
    6 => {
      Channels::FRONT_LEFT
        | Channels::FRONT_CENTRE
        | Channels::FRONT_RIGHT
        | Channels::REAR_LEFT
        | Channels::REAR_RIGHT
        | Channels::LFE1
    }
    7 => {
      Channels::FRONT_LEFT
        | Channels::FRONT_CENTRE
        | Channels::FRONT_RIGHT
        | Channels::SIDE_LEFT
        | Channels::SIDE_RIGHT
        | Channels::REAR_CENTRE
        | Channels::LFE1
    }
    8 => {
      Channels::FRONT_LEFT
        | Channels::FRONT_CENTRE
        | Channels::FRONT_RIGHT
        | Channels::SIDE_LEFT
        | Channels::SIDE_RIGHT
        | Channels::REAR_LEFT
        | Channels::REAR_RIGHT
        | Channels::LFE1
    }
    _ => return unsupported_error("vorbis: unknown channel map (fix me)"),
  };

  Ok(channels)
}
