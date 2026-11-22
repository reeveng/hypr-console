//! What a codec number means: which sample type it fills, in which byte order,
//! and how far its coded bits sit below the top of that type.
//!
//! A buffer is built for exactly one codec, so the variant that holds it is
//! also the one that reads it. Decoding a packet never asks which of two
//! things it was handed, because the answer is the variant.

use std::io;
use std::num::NonZeroU32;

use console_core_audio_decoding_interfaces::audio::{
  AsAudioBufferRef, AudioBuffer, AudioBufferRef, Signal, SignalSpec,
};
use console_core_audio_decoding_interfaces::codecs::{
  CODEC_TYPE_PCM_ALAW, CODEC_TYPE_PCM_F32BE, CODEC_TYPE_PCM_F32LE, CODEC_TYPE_PCM_F64BE,
  CODEC_TYPE_PCM_F64LE, CODEC_TYPE_PCM_MULAW, CODEC_TYPE_PCM_S8, CODEC_TYPE_PCM_S16BE,
  CODEC_TYPE_PCM_S16LE, CODEC_TYPE_PCM_S24BE, CODEC_TYPE_PCM_S24LE, CODEC_TYPE_PCM_S32BE,
  CODEC_TYPE_PCM_S32LE, CODEC_TYPE_PCM_U8, CODEC_TYPE_PCM_U16BE, CODEC_TYPE_PCM_U16LE,
  CODEC_TYPE_PCM_U24BE, CODEC_TYPE_PCM_U24LE, CODEC_TYPE_PCM_U32BE, CODEC_TYPE_PCM_U32LE,
  CodecType,
};
use console_core_audio_decoding_interfaces::errors::{
  Error, Result, decode_error, unsupported_error,
};
use console_core_audio_decoding_interfaces::units::Duration;
use console_core_audio_samples::conv::IntoSample;
use console_core_audio_samples::sample::{Sample, i24, u24};
use console_core_byte_streams::{BufReader, FiniteStream, ReadBytes};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::law::{alaw_to_linear, mulaw_to_linear};

type Infallible<T> = std::result::Result<T, Never>;

#[derive(Clone, Copy)]
struct Bits(u32);

#[derive(Clone, Copy)]
pub enum Order {
  Little,
  Big,
}

impl Order {
  fn read<R, T>(
    self,
    reader: &mut R,
    little: fn(&mut R) -> io::Result<T>,
    big: fn(&mut R) -> io::Result<T>,
  ) -> io::Result<T> {
    match self {
      Order::Little => little(reader),
      Order::Big => big(reader),
    }
  }
}

#[derive(Clone, Copy)]
pub enum Wire {
  U8,
  U16(Order),
  U24(Order),
  U32(Order),
  S8,
  S16(Order),
  S24(Order),
  S32(Order),
  F32(Order),
  F64(Order),
  ALaw,
  MuLaw,
}

impl Wire {
  pub fn of(codec: CodecType) -> Result<Wire> {
    Ok(match codec {
      CODEC_TYPE_PCM_S32LE => Wire::S32(Order::Little),
      CODEC_TYPE_PCM_S32BE => Wire::S32(Order::Big),
      CODEC_TYPE_PCM_S24LE => Wire::S24(Order::Little),
      CODEC_TYPE_PCM_S24BE => Wire::S24(Order::Big),
      CODEC_TYPE_PCM_S16LE => Wire::S16(Order::Little),
      CODEC_TYPE_PCM_S16BE => Wire::S16(Order::Big),
      CODEC_TYPE_PCM_S8 => Wire::S8,
      CODEC_TYPE_PCM_U32LE => Wire::U32(Order::Little),
      CODEC_TYPE_PCM_U32BE => Wire::U32(Order::Big),
      CODEC_TYPE_PCM_U24LE => Wire::U24(Order::Little),
      CODEC_TYPE_PCM_U24BE => Wire::U24(Order::Big),
      CODEC_TYPE_PCM_U16LE => Wire::U16(Order::Little),
      CODEC_TYPE_PCM_U16BE => Wire::U16(Order::Big),
      CODEC_TYPE_PCM_U8 => Wire::U8,
      CODEC_TYPE_PCM_F32LE => Wire::F32(Order::Little),
      CODEC_TYPE_PCM_F32BE => Wire::F32(Order::Big),
      CODEC_TYPE_PCM_F64LE => Wire::F64(Order::Little),
      CODEC_TYPE_PCM_F64BE => Wire::F64(Order::Big),
      CODEC_TYPE_PCM_ALAW => Wire::ALaw,
      CODEC_TYPE_PCM_MULAW => Wire::MuLaw,
      _ => return unsupported_error("pcm: invalid codec type"),
    })
  }
}

pub struct Lane<S: Sample> {
  buffer: AudioBuffer<S>,
  order: Order,
  shift: u32,
}

struct WireBytes(u32);

impl<S: Sample> Lane<S> {
  fn made(frames: Duration, spec: SignalSpec, order: Order, shift: u32) -> Result<Lane<S>> {
    AudioBuffer::new(frames, spec).map(|buffer| Lane {
      buffer,
      order,
      shift,
    })
  }

  fn shifted(
    frames: Duration,
    spec: SignalSpec,
    order: Order,
    width: Bits,
    coded: NonZeroU32,
  ) -> Result<Lane<S>> {
    match width.0.checked_sub(coded.get()) {
      Some(shift) => Lane::made(frames, spec, order, shift),
      None => decode_error("pcm: coded bits per sample is greater than the sample format"),
    }
  }

  fn explicit(
    frames: Duration,
    spec: SignalSpec,
    order: Order,
    width: Bits,
    coded: Option<NonZeroU32>,
  ) -> Result<Lane<S>> {
    match coded {
      Some(bits) => Lane::shifted(frames, spec, order, width, bits),
      None => unsupported_error("pcm: unknown bits per (coded) sample"),
    }
  }

  fn implicit(
    frames: Duration,
    spec: SignalSpec,
    order: Order,
    width: Bits,
    coded: Option<NonZeroU32>,
  ) -> Result<Lane<S>> {
    match coded {
      Some(bits) => Lane::shifted(frames, spec, order, width, bits),
      None => Lane::made(frames, spec, order, 0),
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit047_no_captured_permission",
    allow(
      explicit047_no_captured_permission,
      reason = "the buffer's fill takes one closure of a frame and nothing else, so the packet being read has to be captured by it"
    )
  )]
  fn fill_from(
    &mut self,
    reader: &mut BufReader<'_>,
    width: WireBytes,
    read: impl Fn(&mut BufReader<'_>, Order, u32) -> io::Result<S>,
  ) -> Result<()> {
    let order = self.order;
    let shift = self.shift;
    let Ok(spec) = self.buffer.spec();
    let Ok(channels) = spec.channels.count();
    let frame_bytes = u64::from(channels).saturating_mul(u64::from(width.0));
    let available = reader.bytes_available();
    let whole = available
      .checked_div(frame_bytes)
      .ok_or(Error::DecodeError("pcm: frame width is zero"))?;
    let remainder = available
      .checked_rem(frame_bytes)
      .ok_or(Error::DecodeError("pcm: frame width is zero"))?;
    let Ok(frames) = fitted::<_, u32>(whole.saturating_add(u64::from(remainder != 0)));
    let Ok(()) = self.buffer.clear();

    self.buffer.render(Some(frames), |planes, frame| {
      let Ok(at) = index(frame);

      let Ok(output) = planes.planes();

      for plane in output.iter_mut() {
        let sample = read(reader, order, shift).map_err(Error::IoError)?;

        match plane.get_mut(at) {
          Some(slot) => *slot = sample,
          None => return decode_error("pcm: frame is outside the audio buffer"),
        }
      }

      Ok(())
    })
  }
}

pub enum Pcm {
  U8(Lane<u8>),
  U16(Lane<u16>),
  U24(Lane<u24>),
  U32(Lane<u32>),
  S8(Lane<i8>),
  S16(Lane<i16>),
  S24(Lane<i24>),
  S32(Lane<i32>),
  F32(Lane<f32>),
  F64(Lane<f64>),
  ALaw(Lane<i16>),
  MuLaw(Lane<i16>),
}

impl Pcm {
  pub fn new(
    wire: Wire,
    frames: Duration,
    spec: SignalSpec,
    coded: Option<NonZeroU32>,
  ) -> Result<Pcm> {
    match wire {
      Wire::U8 => Lane::explicit(frames, spec, Order::Little, Bits(8), coded).map(Pcm::U8),
      Wire::U16(order) => Lane::explicit(frames, spec, order, Bits(16), coded).map(Pcm::U16),
      Wire::U24(order) => Lane::explicit(frames, spec, order, Bits(24), coded).map(Pcm::U24),
      Wire::U32(order) => Lane::explicit(frames, spec, order, Bits(32), coded).map(Pcm::U32),
      Wire::S8 => Lane::explicit(frames, spec, Order::Little, Bits(8), coded).map(Pcm::S8),
      Wire::S16(order) => Lane::explicit(frames, spec, order, Bits(16), coded).map(Pcm::S16),
      Wire::S24(order) => Lane::explicit(frames, spec, order, Bits(24), coded).map(Pcm::S24),
      Wire::S32(order) => Lane::explicit(frames, spec, order, Bits(32), coded).map(Pcm::S32),
      Wire::F32(order) => Lane::implicit(frames, spec, order, Bits(32), coded).map(Pcm::F32),
      Wire::F64(order) => Lane::implicit(frames, spec, order, Bits(64), coded).map(Pcm::F64),
      Wire::ALaw => Lane::implicit(frames, spec, Order::Little, Bits(16), coded).map(Pcm::ALaw),
      Wire::MuLaw => Lane::implicit(frames, spec, Order::Little, Bits(16), coded).map(Pcm::MuLaw),
    }
  }

  pub fn fill(&mut self, reader: &mut BufReader<'_>) -> Result<()> {
    match self {
      Pcm::U8(lane) => lane.fill_from(reader, WireBytes(1), |source, _, shift| {
        source.read_u8().map(|sample| sample.wrapping_shl(shift))
      }),
      Pcm::U16(lane) => lane.fill_from(reader, WireBytes(2), |source, order, shift| {
        order
          .read(source, BufReader::read_u16, BufReader::read_be_u16)
          .map(|sample| sample.wrapping_shl(shift))
      }),
      Pcm::U24(lane) => lane.fill_from(reader, WireBytes(3), |source, order, shift| {
        order
          .read(source, BufReader::read_u24, BufReader::read_be_u24)
          .map(|sample| sample.wrapping_shl(8).wrapping_shl(shift).into_sample())
      }),
      Pcm::U32(lane) => lane.fill_from(reader, WireBytes(4), |source, order, shift| {
        order
          .read(source, BufReader::read_u32, BufReader::read_be_u32)
          .map(|sample| sample.wrapping_shl(shift))
      }),
      Pcm::S8(lane) => lane.fill_from(reader, WireBytes(1), |source, _, shift| {
        source.read_i8().map(|sample| sample.wrapping_shl(shift))
      }),
      Pcm::S16(lane) => lane.fill_from(reader, WireBytes(2), |source, order, shift| {
        order
          .read(source, BufReader::read_i16, BufReader::read_be_i16)
          .map(|sample| sample.wrapping_shl(shift))
      }),
      Pcm::S24(lane) => lane.fill_from(reader, WireBytes(3), |source, order, shift| {
        order
          .read(source, BufReader::read_i24, BufReader::read_be_i24)
          .map(|sample| sample.wrapping_shl(8).wrapping_shl(shift).into_sample())
      }),
      Pcm::S32(lane) => lane.fill_from(reader, WireBytes(4), |source, order, shift| {
        order
          .read(source, BufReader::read_i32, BufReader::read_be_i32)
          .map(|sample| sample.wrapping_shl(shift))
      }),
      Pcm::F32(lane) => lane.fill_from(reader, WireBytes(4), |source, order, _| {
        order.read(source, BufReader::read_f32, BufReader::read_be_f32)
      }),
      Pcm::F64(lane) => lane.fill_from(reader, WireBytes(8), |source, order, _| {
        order.read(source, BufReader::read_f64, BufReader::read_be_f64)
      }),
      Pcm::ALaw(lane) => lane.fill_from(reader, WireBytes(1), |source, _, _| {
        source.read_u8().map(|code| {
          let Ok(linear) = alaw_to_linear(code);

          linear
        })
      }),
      Pcm::MuLaw(lane) => lane.fill_from(reader, WireBytes(1), |source, _, _| {
        source.read_u8().map(|code| {
          let Ok(linear) = mulaw_to_linear(code);

          linear
        })
      }),
    }
  }

  pub fn clear(&mut self) -> Infallible<()> {
    let Ok(()) = match self {
      Pcm::U8(lane) => lane.buffer.clear(),
      Pcm::U16(lane) => lane.buffer.clear(),
      Pcm::U24(lane) => lane.buffer.clear(),
      Pcm::U32(lane) => lane.buffer.clear(),
      Pcm::S8(lane) => lane.buffer.clear(),
      Pcm::S16(lane) | Pcm::ALaw(lane) | Pcm::MuLaw(lane) => lane.buffer.clear(),
      Pcm::S24(lane) => lane.buffer.clear(),
      Pcm::S32(lane) => lane.buffer.clear(),
      Pcm::F32(lane) => lane.buffer.clear(),
      Pcm::F64(lane) => lane.buffer.clear(),
    };

    Ok(())
  }

  pub fn decoded(&self) -> Infallible<AudioBufferRef<'_>> {
    match self {
      Pcm::U8(lane) => {
        let Ok(value_1) = lane.buffer.as_audio_buffer_ref();

        Ok(value_1)
      }
      Pcm::U16(lane) => {
        let Ok(value_2) = lane.buffer.as_audio_buffer_ref();

        Ok(value_2)
      }
      Pcm::U24(lane) => {
        let Ok(value_3) = lane.buffer.as_audio_buffer_ref();

        Ok(value_3)
      }
      Pcm::U32(lane) => {
        let Ok(value_4) = lane.buffer.as_audio_buffer_ref();

        Ok(value_4)
      }
      Pcm::S8(lane) => {
        let Ok(value_5) = lane.buffer.as_audio_buffer_ref();

        Ok(value_5)
      }
      Pcm::S16(lane) | Pcm::ALaw(lane) | Pcm::MuLaw(lane) => {
        let Ok(value_6) = lane.buffer.as_audio_buffer_ref();

        Ok(value_6)
      }
      Pcm::S24(lane) => {
        let Ok(value_7) = lane.buffer.as_audio_buffer_ref();

        Ok(value_7)
      }
      Pcm::S32(lane) => {
        let Ok(value_8) = lane.buffer.as_audio_buffer_ref();

        Ok(value_8)
      }
      Pcm::F32(lane) => {
        let Ok(value_9) = lane.buffer.as_audio_buffer_ref();

        Ok(value_9)
      }
      Pcm::F64(lane) => {
        let Ok(value_10) = lane.buffer.as_audio_buffer_ref();

        Ok(value_10)
      }
    }
  }
}
