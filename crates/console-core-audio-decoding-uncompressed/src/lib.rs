//! The decoder for every raw and logarithmic PCM codec the containers name.
//!
//! It came out of Symphonia's `symphonia-codec-pcm` and keeps its behaviour:
//! an integer codec may code fewer bits than the sample type that holds it
//! and is shifted up to fill it, floating point and G.711 codecs have a width
//! that is implicit in the codec, and a packet that cannot be read whole
//! leaves the buffer empty rather than half written. What the codec number
//! means is `pcm`, and the G.711 arithmetic is `law`.

mod law;
mod pcm;

use std::num::NonZeroU32;

use console_core_audio_decoding_interfaces::audio::{AudioBufferRef, SignalSpec};
use console_core_audio_decoding_interfaces::codecs::{
  CODEC_TYPE_PCM_ALAW, CODEC_TYPE_PCM_F32BE, CODEC_TYPE_PCM_F32LE, CODEC_TYPE_PCM_F64BE,
  CODEC_TYPE_PCM_F64LE, CODEC_TYPE_PCM_MULAW, CODEC_TYPE_PCM_S8, CODEC_TYPE_PCM_S16BE,
  CODEC_TYPE_PCM_S16LE, CODEC_TYPE_PCM_S24BE, CODEC_TYPE_PCM_S24LE, CODEC_TYPE_PCM_S32BE,
  CODEC_TYPE_PCM_S32LE, CODEC_TYPE_PCM_U8, CODEC_TYPE_PCM_U16BE, CODEC_TYPE_PCM_U16LE,
  CODEC_TYPE_PCM_U24BE, CODEC_TYPE_PCM_U24LE, CODEC_TYPE_PCM_U32BE, CODEC_TYPE_PCM_U32LE,
  CodecDescriptor, CodecParameters, Decoder, DecoderOptions, FinalizeResult,
};
use console_core_audio_decoding_interfaces::errors::{Result, unsupported_error};
use console_core_audio_decoding_interfaces::formats::Packet;
use console_core_audio_decoding_interfaces::support_codec;

use crate::pcm::{Pcm, Wire};

pub struct PcmDecoder {
  params: CodecParameters,
  pcm: Pcm,
}

fn signal_spec(params: &CodecParameters, rate: u32) -> Result<SignalSpec> {
  match (params.channels, params.channel_layout) {
    (Some(channels), _) => match channels.count() {
      Ok(0) => unsupported_error("pcm: number of channels cannot be 0"),
      Ok(_) => {
        let Ok(value_1) = SignalSpec::new(rate, channels);

        Ok(value_1)
      }
    },
    (None, Some(layout)) => {
      let Ok(value_2) = SignalSpec::new_with_layout(rate, layout);

      Ok(value_2)
    }
    (None, None) => unsupported_error("pcm: channels or channel_layout is required"),
  }
}

fn tuned(wire: Wire, params: &CodecParameters) -> Result<PcmDecoder> {
  let frames = match params.max_frames_per_packet {
    Some(frames) => frames,
    None => return unsupported_error("pcm: maximum frames per packet is required"),
  };

  let rate = match params.sample_rate {
    Some(rate) => rate,
    None => return unsupported_error("pcm: sample rate is required"),
  };

  let spec = signal_spec(params, rate)?;
  let coded = params
    .bits_per_coded_sample
    .or(params.bits_per_sample)
    .and_then(NonZeroU32::new);
  let pcm = Pcm::new(wire, frames, spec, coded)?;

  Ok(PcmDecoder {
    params: params.clone(),
    pcm,
  })
}

impl PcmDecoder {
  const SUPPORTED_CODECS: &[CodecDescriptor] = &[
    support_codec!(
      CODEC_TYPE_PCM_S32LE,
      "pcm_s32le",
      "PCM Signed 32-bit Little-Endian Interleaved"
    ),
    support_codec!(
      CODEC_TYPE_PCM_S32BE,
      "pcm_s32be",
      "PCM Signed 32-bit Big-Endian Interleaved"
    ),
    support_codec!(
      CODEC_TYPE_PCM_S24LE,
      "pcm_s24le",
      "PCM Signed 24-bit Little-Endian Interleaved"
    ),
    support_codec!(
      CODEC_TYPE_PCM_S24BE,
      "pcm_s24be",
      "PCM Signed 24-bit Big-Endian Interleaved"
    ),
    support_codec!(
      CODEC_TYPE_PCM_S16LE,
      "pcm_s16le",
      "PCM Signed 16-bit Little-Endian Interleaved"
    ),
    support_codec!(
      CODEC_TYPE_PCM_S16BE,
      "pcm_s16be",
      "PCM Signed 16-bit Big-Endian Interleaved"
    ),
    support_codec!(CODEC_TYPE_PCM_S8, "pcm_s8", "PCM Signed 8-bit Interleaved"),
    support_codec!(
      CODEC_TYPE_PCM_U32LE,
      "pcm_u32le",
      "PCM Unsigned 32-bit Little-Endian Interleaved"
    ),
    support_codec!(
      CODEC_TYPE_PCM_U32BE,
      "pcm_u32be",
      "PCM Unsigned 32-bit Big-Endian Interleaved"
    ),
    support_codec!(
      CODEC_TYPE_PCM_U24LE,
      "pcm_u24le",
      "PCM Unsigned 24-bit Little-Endian Interleaved"
    ),
    support_codec!(
      CODEC_TYPE_PCM_U24BE,
      "pcm_u24be",
      "PCM Unsigned 24-bit Big-Endian Interleaved"
    ),
    support_codec!(
      CODEC_TYPE_PCM_U16LE,
      "pcm_u16le",
      "PCM Unsigned 16-bit Little-Endian Interleaved"
    ),
    support_codec!(
      CODEC_TYPE_PCM_U16BE,
      "pcm_u16be",
      "PCM Unsigned 16-bit Big-Endian Interleaved"
    ),
    support_codec!(
      CODEC_TYPE_PCM_U8,
      "pcm_u8",
      "PCM Unsigned 8-bit Interleaved"
    ),
    support_codec!(
      CODEC_TYPE_PCM_F32LE,
      "pcm_f32le",
      "PCM 32-bit Little-Endian Floating Point Interleaved"
    ),
    support_codec!(
      CODEC_TYPE_PCM_F32BE,
      "pcm_f32be",
      "PCM 32-bit Big-Endian Floating Point Interleaved"
    ),
    support_codec!(
      CODEC_TYPE_PCM_F64LE,
      "pcm_f64le",
      "PCM 64-bit Little-Endian Floating Point Interleaved"
    ),
    support_codec!(
      CODEC_TYPE_PCM_F64BE,
      "pcm_f64be",
      "PCM 64-bit Big-Endian Floating Point Interleaved"
    ),
    support_codec!(CODEC_TYPE_PCM_ALAW, "pcm_alaw", "PCM A-law"),
    support_codec!(CODEC_TYPE_PCM_MULAW, "pcm_mulaw", "PCM Mu-law"),
  ];
}

impl Decoder for PcmDecoder {
  fn try_new(params: &CodecParameters, _options: &DecoderOptions) -> Result<Self> {
    let wire = Wire::of(params.codec)?;

    tuned(wire, params)
  }

  fn supported_codecs()
  -> std::result::Result<&'static [CodecDescriptor], console_core_never::Never> {
    Ok(Self::SUPPORTED_CODECS)
  }

  fn reset(&mut self) -> std::result::Result<(), console_core_never::Never> {
    Ok(())
  }

  fn codec_params(&self) -> std::result::Result<&CodecParameters, console_core_never::Never> {
    Ok(&self.params)
  }

  fn decode(&mut self, packet: &Packet) -> Result<AudioBufferRef<'_>> {
    let Ok(mut reader) = packet.as_buf_reader();

    match self.pcm.fill(&mut reader) {
      Ok(()) => {
        let Ok(decoded) = self.pcm.decoded();

        Ok(decoded)
      }
      Err(fault) => {
        let Ok(()) = self.pcm.clear();

        Err(fault)
      }
    }
  }

  fn finalize(&mut self) -> std::result::Result<FinalizeResult, console_core_never::Never> {
    Ok(FinalizeResult::default())
  }

  fn last_decoded(&self) -> std::result::Result<AudioBufferRef<'_>, console_core_never::Never> {
    let Ok(decoded) = self.pcm.decoded();

    Ok(decoded)
  }
}
