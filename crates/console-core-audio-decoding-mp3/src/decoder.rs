//! The decoder a packet of the stream is handed to.
//!
//! A packet is one whole frame, header and all, and its length has to be the
//! one its header says; the first one decides what the output buffer is, and a
//! later one that says a different rate or channel layout is refused rather
//! than converted. A packet that fails clears the buffer, so what is handed
//! back is never half of a frame.

use console_core_audio_decoding_interfaces::audio::{
  AsAudioBufferRef, AudioBuffer, AudioBufferRef, Signal,
};
use console_core_audio_decoding_interfaces::codecs::{
  CODEC_TYPE_MP1, CODEC_TYPE_MP2, CODEC_TYPE_MP3, CodecDescriptor, CodecParameters, CodecType,
  Decoder, DecoderOptions, FinalizeResult,
};
use console_core_audio_decoding_interfaces::errors::{Result, decode_error, unsupported_error};
use console_core_audio_decoding_interfaces::formats::Packet;
use console_core_audio_decoding_interfaces::io::FiniteStream;
use console_core_audio_decoding_interfaces::support_codec;
use console_core_byte_streams::BufReader;

use crate::common::{FrameHeader, Infallible, Layer, spec};
use crate::header::read_frame_header;
use crate::layer1::Layer1;
use crate::layer2::Layer2;
use crate::layer3::Layer3;

const FRAME_SAMPLES: u64 = 1152;

enum Layers {
  One(Layer1),
  Two(Layer2),
  Three(Box<Layer3>),
}

pub struct MpaDecoder {
  params: CodecParameters,
  layers: Layers,
  buf: AudioBuffer<f32>,
}

fn layers_for(codec: CodecType) -> Result<Layers> {
  match codec {
    CODEC_TYPE_MP1 => {
      let Ok(layer) = Layer1::new();

      Ok(Layers::One(layer))
    }
    CODEC_TYPE_MP2 => {
      let Ok(layer) = Layer2::new();

      Ok(Layers::Two(layer))
    }
    CODEC_TYPE_MP3 => {
      let layer = Layer3::new()?;

      Ok(Layers::Three(Box::new(layer)))
    }
    _ => unsupported_error("mpa: invalid codec type"),
  }
}

fn hand_over(
  layers: &mut Layers,
  reader: &mut BufReader<'_>,
  header: &FrameHeader,
  out: &mut AudioBuffer<f32>,
) -> Result<()> {
  match (layers, header.layer) {
    (Layers::One(layer), Layer::One) => layer.decode(reader, header, out),
    (Layers::Two(layer), Layer::Two) => layer.decode(reader, header, out),
    (Layers::Three(layer), Layer::Three) => layer.decode(reader, header, out),
    (Layers::One(_), Layer::Two | Layer::Three)
    | (Layers::Two(_), Layer::One | Layer::Three)
    | (Layers::Three(_), Layer::One | Layer::Two) => {
      decode_error("mpa: invalid mpeg audio layer")
    }
  }
}

fn forget(layers: &mut Layers) -> Infallible<()> {
  match layers {
    Layers::One(layer) => layer.reset(),
    Layers::Two(layer) => layer.reset(),
    Layers::Three(layer) => layer.reset(),
  }
}

impl MpaDecoder {
  fn decode_inner(&mut self, packet: &Packet) -> Result<()> {
    let Ok(mut reader) = packet.as_buf_reader();
    let header = read_frame_header(&mut reader)?;

    match ::core::primitive::bool::then_some(u64::from(header.frame_size) == reader.bytes_available(), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return decode_error("mpa: invalid packet length"),
    }

    let Ok(frame_spec) = spec(&header);
    let Ok(current_spec) = self.buf.spec();
    let Ok(capacity) = self.buf.capacity();
    let same = current_spec == &frame_spec;

    match (capacity, same) {
      (0, _) => {
        let buf = AudioBuffer::new(FRAME_SAMPLES, frame_spec)?;

        self.buf = buf;
      }
      (_, true) => {}
      (_, false) => return decode_error("mpa: invalid audio buffer signal spec for packet"),
    }

    let Ok(()) = self.buf.clear();

    hand_over(&mut self.layers, &mut reader, &header, &mut self.buf)?;

    let Ok(value_1) = packet.trim_start();
    let Ok(value_2) = packet.trim_end();

    let Ok(()) = self.buf.trim(value_1, value_2);

    Ok(())
  }
}

impl Decoder for MpaDecoder {
  fn try_new(params: &CodecParameters, _: &DecoderOptions) -> Result<Self> {
    let layers = layers_for(params.codec)?;

    let Ok(value_3) = AudioBuffer::unused();

    Ok(MpaDecoder {
      params: params.clone(),
      layers,
      buf: value_3,
    })
  }

  fn supported_codecs()
  -> std::result::Result<&'static [CodecDescriptor], console_core_never::Never> {
    Ok(&[
      support_codec!(CODEC_TYPE_MP1, "mp1", "MPEG Audio Layer 1"),
      support_codec!(CODEC_TYPE_MP2, "mp2", "MPEG Audio Layer 2"),
      support_codec!(CODEC_TYPE_MP3, "mp3", "MPEG Audio Layer 3"),
    ])
  }

  fn codec_params(&self) -> std::result::Result<&CodecParameters, console_core_never::Never> {
    Ok(&self.params)
  }

  fn reset(&mut self) -> std::result::Result<(), console_core_never::Never> {
    let Ok(()) = forget(&mut self.layers);

    Ok(())
  }

  fn decode(&mut self, packet: &Packet) -> Result<AudioBufferRef<'_>> {
    match self.decode_inner(packet) {
      Ok(()) => {
        let Ok(value_4) = self.buf.as_audio_buffer_ref();

        Ok(value_4)
      }
      Err(fault) => {
        let Ok(()) = self.buf.clear();

        Err(fault)
      }
    }
  }

  fn finalize(&mut self) -> std::result::Result<FinalizeResult, console_core_never::Never> {
    Ok(FinalizeResult::default())
  }

  fn last_decoded(&self) -> std::result::Result<AudioBufferRef<'_>, console_core_never::Never> {
    let Ok(value_5) = self.buf.as_audio_buffer_ref();

    Ok(value_5)
  }
}
