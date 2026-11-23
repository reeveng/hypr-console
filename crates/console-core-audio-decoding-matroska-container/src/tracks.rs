//! A track element made into the `Track` the demuxer hands out.

use std::collections::HashMap;

use console_core_audio_decoding_interfaces::audio::Layout;
use console_core_audio_decoding_interfaces::codecs::{
  CODEC_TYPE_FLAC, CODEC_TYPE_VORBIS, CodecParameters, CodecType,
};
use console_core_audio_decoding_interfaces::formats::Track;
use console_core_audio_decoding_interfaces::sample::SampleFormat;
use console_core_audio_decoding_interfaces::units::TimeBase;
use console_core_never::Never;
use console_core_number_conversion::{fitted, toward_zero_u64};

use crate::codecs::codec_id_to_type;
use crate::extra::{flac_extra_data, vorbis_extra_data};
use crate::fault::{DecodeError, Invalid};
use crate::segment::{AudioElement, InfoElement, TrackElement};

const NANOSECONDS: u32 = 1_000_000_000;

pub(crate) fn time_base(info: &InfoElement) -> Result<TimeBase, DecodeError> {
  let scale = u32::try_from(info.timestamp_scale)
    .map_err(|_too_wide| DecodeError::Invalid(Invalid::TimestampScale))?;

  match scale {
    0 => Err(DecodeError::Invalid(Invalid::TimestampScale)),
    numerator => TimeBase::new(numerator, NANOSECONDS).map_err(DecodeError::from),
  }
}

pub(crate) fn default_durations(
  elements: &[TrackElement],
  info: &InfoElement,
) -> Result<HashMap<u32, u64>, DecodeError> {
  let mut durations = HashMap::new();

  for element in elements {
    let id = track_id(element)?;
    let ticks = element
      .default_duration
      .and_then(|nanoseconds| nanoseconds.checked_div(info.timestamp_scale));

    match ticks {
      Some(ticks_2) => {
        durations.insert(id, ticks_2);
      }
      None => {}
    }
  }

  Ok(durations)
}

pub(crate) fn build_tracks(
  elements: Vec<TrackElement>,
  info: &InfoElement,
  time_base: TimeBase,
) -> Result<Vec<Track>, DecodeError> {
  elements
    .into_iter()
    .map(|element| build_track(element, info, time_base))
    .collect()
}

fn track_id(element: &TrackElement) -> Result<u32, DecodeError> {
  u32::try_from(element.number).map_err(|_too_wide| DecodeError::Invalid(Invalid::TrackNumber))
}

fn build_track(
  element: TrackElement,
  info: &InfoElement,
  time_base: TimeBase,
) -> Result<Track, DecodeError> {
  let id = track_id(&element)?;
  let Ok(codec) = codec_id_to_type(&element);
  let Ok(mut parameters) = CodecParameters::new();

  let Ok(_configured_with_time_base) = parameters.with_time_base(time_base);

  match info.duration {
    Some(duration) => {
      let Ok(frames) = toward_zero_u64(duration);

      let Ok(_configured_with_n_frames) = parameters.with_n_frames(frames);
    }
    None => {}
  }

  match &element.audio {
    Some(audio) => describe_audio(&mut parameters, &element, audio, codec)?,
    None => {}
  }

  Ok(Track {
    id,
    codec_params: parameters,
    language: element.language,
  })
}

fn layout(channels: u64) -> Result<Option<Layout>, Never> {
  Ok(match channels {
    1 => Some(Layout::Mono),
    2 => Some(Layout::Stereo),
    3 => Some(Layout::TwoPointOne),
    6 => Some(Layout::FivePointOne),
    _ => None,
  })
}

fn sample_format(bit_depth: Option<u64>) -> Result<Option<SampleFormat>, Never> {
  Ok(match bit_depth {
    Some(8) => Some(SampleFormat::S8),
    Some(16) => Some(SampleFormat::S16),
    Some(24) => Some(SampleFormat::S24),
    Some(32) => Some(SampleFormat::S32),
    Some(_) | None => None,
  })
}

fn describe_audio(
  parameters: &mut CodecParameters,
  element: &TrackElement,
  audio: &AudioElement,
  codec: Option<CodecType>,
) -> Result<(), DecodeError> {
  let Ok(format) = sample_format(audio.bit_depth);
  let Ok(channel_layout) = layout(audio.channels);

  let Ok(_configured_with_sample_rate) = parameters.with_sample_rate(audio.sample_rate_hz);

  match format {
    Some(format_2) => {
      let Ok(_configured_with_sample_format) = parameters.with_sample_format(format_2);
    }
    None => {}
  }

  match audio.bit_depth {
    Some(bits) => {
      let Ok(bits_2) = fitted::<u64, u32>(bits);

      let Ok(_configured_with_bits_per_sample) = parameters.with_bits_per_sample(bits_2);
    }
    None => {}
  }

  match channel_layout {
    Some(channel_layout_2) => {
      let Ok(_configured_with_channel_layout) = parameters.with_channel_layout(channel_layout_2);
    }
    None => {}
  }

  match codec {
    Some(codec_2) => describe_codec(parameters, element, codec_2),
    None => Ok(()),
  }
}

fn describe_codec(
  parameters: &mut CodecParameters,
  element: &TrackElement,
  codec: CodecType,
) -> Result<(), DecodeError> {
  let Ok(_configured_for_codec) = parameters.for_codec(codec);

  match &element.codec_private {
    Some(private) => {
      let extra = match codec {
        CODEC_TYPE_VORBIS => vorbis_extra_data(private)?,
        CODEC_TYPE_FLAC => flac_extra_data(private)?,
        _ => private.clone(),
      };

      let Ok(_configured_with_extra_data) = parameters.with_extra_data(extra);

      Ok(())
    }
    None => Ok(()),
  }
}
