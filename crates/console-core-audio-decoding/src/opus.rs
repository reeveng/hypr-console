//! The packet registry adapter for the standalone Opus decoder.

use console_core_audio_decoding_interfaces::audio::{
  AsAudioBufferRef, AudioBuffer, AudioBufferRef, Signal, SignalSpec,
};
use console_core_audio_decoding_interfaces::codecs::{
  CODEC_TYPE_OPUS, CodecDescriptor, CodecParameters, Decoder, DecoderOptions, FinalizeResult,
  StreamStart,
};
use console_core_audio_decoding_interfaces::errors::{Error, Result};
use console_core_audio_decoding_interfaces::formats::Packet;
use console_core_audio_decoding_interfaces::support_codec;
use console_core_never::Never;
use core::num::NonZeroU32;

const SAMPLE_RATE: u32 = 48_000;
const MAX_PACKET_SAMPLES: u64 = 5_760;

pub(super) struct OpusDecoder {
  params: CodecParameters,
  decoder: console_core_audio_decoding_opus::Decoder,
  output: AudioBuffer<f32>,
}

fn decode_error(fault: console_core_audio_decoding_opus::Error) -> Error {
  let Ok(description) = fault.description();
  Error::DecodeError(description)
}

impl Decoder for OpusDecoder {
  fn try_new(params: &CodecParameters, options: &DecoderOptions) -> Result<Self> {
    match params.codec {
      CODEC_TYPE_OPUS => {}
      _unsupported => return Err(Error::Unsupported("Opus: wrong codec identifier")),
    }
    let head = params
      .extra_data
      .as_deref()
      .ok_or(Error::DecodeError("missing OpusHead"))?;
    let mut decoder =
      console_core_audio_decoding_opus::Decoder::new(head).map_err(decode_error)?;
    match options.start {
      StreamStart::Beginning => {
        let Ok(()) = decoder.trim_pre_skip();
      }
      StreamStart::AfterSeek => {}
    }
    let Ok(channels) = decoder.channel_count();
    let count = NonZeroU32::new(channels).ok_or(Error::DecodeError("Opus has no channels"))?;
    let Ok(spec) = SignalSpec::new_discrete(SAMPLE_RATE, count);
    let output = AudioBuffer::new(MAX_PACKET_SAMPLES, spec)?;
    Ok(Self {
      params: params.clone(),
      decoder,
      output,
    })
  }

  fn supported_codecs() -> core::result::Result<&'static [CodecDescriptor], Never> {
    Ok(&[support_codec!(CODEC_TYPE_OPUS, "opus", "Opus")])
  }

  fn reset(&mut self) -> core::result::Result<(), Never> {
    let Ok(()) = self.decoder.reset();
    self.output.clear()
  }

  fn codec_params(&self) -> core::result::Result<&CodecParameters, Never> {
    Ok(&self.params)
  }

  fn decode(&mut self, packet: &Packet) -> Result<AudioBufferRef<'_>> {
    let Ok(()) = self.output.clear();
    let Ok(bytes) = packet.buf();
    let channels = self.decoder.decode(Some(bytes)).map_err(decode_error)?;
    let first = channels
      .first()
      .ok_or(Error::DecodeError("Opus has no channels"))?;
    let Ok(frames) = console_core_number_conversion::fitted::<_, u32>(first.len());
    self.output.render_reserved(Some(frames))?;

    for (index, samples) in (0_u32..).zip(channels) {
      let Ok(destination) = self.output.chan_mut(index);
      for (target, sample) in destination.iter_mut().zip(samples) {
        *target = sample;
      }
    }

    let Ok(view) = self.output.as_audio_buffer_ref();
    Ok(view)
  }

  fn finalize(&mut self) -> core::result::Result<FinalizeResult, Never> {
    Ok(FinalizeResult::default())
  }

  fn last_decoded(&self) -> core::result::Result<AudioBufferRef<'_>, Never> {
    self.output.as_audio_buffer_ref()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn registered_opus_obeys_start_and_seek_pre_skip() -> Result<()> {
    let head = [
      b'O', b'p', b'u', b's', b'H', b'e', b'a', b'd', 1, 2, 120, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    let Ok(mut params) = CodecParameters::new();
    let Ok(_) = params.for_codec(CODEC_TYPE_OPUS);
    let Ok(_) = params.with_extra_data(Box::from(head.as_slice()));
    let Ok(registry) = crate::codec_registry();
    let mut beginning = registry.make(&params, &DecoderOptions::default())?;
    let mut sought = registry.make(
      &params,
      &DecoderOptions {
        start: StreamStart::AfterSeek,
        ..DecoderOptions::default()
      },
    )?;
    let Ok(packet) = Packet::new_from_slice(0, 0, 960, &[]);

    let start = beginning.decode(&packet)?;
    let Ok(start_frames) = start.frames();
    let end = sought.decode(&packet)?;
    let Ok(seek_frames) = end.frames();
    let Ok(spec) = end.spec();
    let Ok(channels) = spec.channel_count();
    assert_eq!(channels, 2);
    assert_eq!(seek_frames, 960);
    assert_eq!(start_frames, seek_frames - 120);

    let Ok(()) = beginning.reset();
    let reset = beginning.decode(&packet)?;
    let Ok(reset_frames) = reset.frames();
    assert_eq!(reset_frames, seek_frames);
    Ok(())
  }

  #[test]
  fn registered_opus_preserves_discrete_channel_count() -> Result<()> {
    let mut head = vec![
      b'O', b'p', b'u', b's', b'H', b'e', b'a', b'd', 1, 27, 0, 0, 0, 0, 0, 0, 0, 0, 255, 1, 0,
    ];
    head.extend([0; 27]);
    let Ok(mut params) = CodecParameters::new();
    let Ok(_) = params.for_codec(CODEC_TYPE_OPUS);
    let Ok(_) = params.with_extra_data(head.into_boxed_slice());
    let Ok(registry) = crate::codec_registry();
    let mut decoder = registry.make(&params, &DecoderOptions::default())?;
    let Ok(packet) = Packet::new_from_slice(0, 0, 960, &[]);
    let output = decoder.decode(&packet)?;
    let Ok(spec) = output.spec();
    let Ok(count) = spec.channel_count();
    let Ok(frames) = output.frames();
    let mut interleaved = crate::core::audio::SampleBuffer::<f32>::new(u64::from(frames), *spec)?;
    interleaved.copy_interleaved_ref(output)?;
    let Ok(samples) = interleaved.samples();

    assert_eq!(count, 27);
    assert_eq!(frames, 960);
    assert_eq!(samples.len(), 27 * 960);
    Ok(())
  }
}
