//! The FLAC decoder: a packet is one frame, and the frame is a header, a subframe for each
//! channel, and a CRC the demuxer has already checked.
//!
//! A subframe is one of four things (constant, verbatim, a fixed polynomial predictor, a
//! quantized linear one), each followed by a Rice-coded residual that the predictor is added
//! back onto. The fixed predictors are the linear predictor with coefficients the format
//! fixes and no shift, so there is one predictor here rather than five; orders one to four
//! are the rows of `FIXED_COEFFICIENTS`. Everything is decoded as 32 bit samples whatever the
//! stream's depth, shifted up so the sample sits at the top of the word, which is what the
//! rest of the player reads.

use console_core_audio_decoding_flac_and_vorbis_headers::flac::metadata::StreamInfo;
use console_core_audio_decoding_interfaces::audio::{
  AsAudioBufferRef, AudioBuffer, AudioBufferRef,
};
use console_core_audio_decoding_interfaces::audio::{Signal, SignalSpec};
use console_core_audio_decoding_interfaces::codecs::{
  CODEC_TYPE_FLAC, CodecDescriptor, CodecParameters, Decoder, DecoderOptions, FinalizeResult,
  VerificationCheck, VerificationMode, VerificationOutcome,
};
use console_core_audio_decoding_interfaces::errors::{Result, decode_error, unsupported_error};
use console_core_audio_decoding_interfaces::formats::Packet;
use console_core_audio_decoding_interfaces::support_codec;
use console_core_audio_decoding_interfaces::units::TimeBase;
use console_core_byte_streams::{BitReaderLtr, BufReader, FlagBit, ReadBitsLtr};
use console_core_never::Never;
use std::cell::Cell;
use std::cmp::Ordering;

use console_core_number_conversion::{fitted, index};

use super::frame::{ChannelAssignment, read_frame_header, sync_frame};
use super::validate::Validator;

const FIXED_COEFFICIENTS: [&[i32]; 5] = [&[], &[1], &[-1, 2], &[1, -3, 3], &[-1, 4, -6, 4]];

const MAX_LINEAR_ORDER: u32 = 32;

fn decorrelate_left_side(left: &[i32], side: &mut [i32]) -> std::result::Result<(), Never> {
  for (s, l) in side.iter_mut().zip(left) {
    *s = l.wrapping_sub(*s);
  }

  Ok(())
}

fn decorrelate_mid_side(mid: &mut [i32], side: &mut [i32]) -> std::result::Result<(), Never> {
  for (m, s) in mid.iter_mut().zip(side) {
    let doubled = m.wrapping_shl(1) | (*s & 1);
    let difference = *s;

    *m = doubled.wrapping_add(difference).wrapping_shr(1);
    *s = doubled.wrapping_sub(difference).wrapping_shr(1);
  }

  Ok(())
}

fn decorrelate_right_side(right: &[i32], side: &mut [i32]) -> std::result::Result<(), Never> {
  for (s, r) in side.iter_mut().zip(right) {
    *s = s.wrapping_add(*r);
  }

  Ok(())
}

pub struct FlacDecoder {
  params: CodecParameters,
  validator: Option<Validator>,
  buf: AudioBuffer<i32>,
}

fn read_subframes<B: ReadBitsLtr>(
  bs: &mut B,
  assignment: ChannelAssignment,
  bits_per_sample: u32,
  buf: &mut AudioBuffer<i32>,
) -> Result<()> {
  let wider = bits_per_sample.saturating_add(1);

  match assignment {
    ChannelAssignment::Independent(channels) => {
      for channel in 0..channels {
        let Ok(value_1) = buf.chan_mut(channel);

        read_subframe(bs, bits_per_sample, value_1)?;
      }
    }
    ChannelAssignment::LeftSide => {
      let Ok((left, side)) = buf.chan_pair_mut(0, 1);

      read_subframe(bs, bits_per_sample, left)?;
      read_subframe(bs, wider, side)?;

      let Ok(()) = decorrelate_left_side(left, side);
    }
    ChannelAssignment::MidSide => {
      let Ok((mid, side)) = buf.chan_pair_mut(0, 1);

      read_subframe(bs, bits_per_sample, mid)?;
      read_subframe(bs, wider, side)?;

      let Ok(()) = decorrelate_mid_side(mid, side);
    }
    ChannelAssignment::RightSide => {
      let Ok((side, right)) = buf.chan_pair_mut(0, 1);

      read_subframe(bs, wider, side)?;
      read_subframe(bs, bits_per_sample, right)?;

      let Ok(()) = decorrelate_right_side(right, side);
    }
  }

  Ok(())
}

impl FlacDecoder {
  fn decode_inner(&mut self, packet: &Packet) -> Result<()> {
    let Ok(mut reader) = packet.as_buf_reader();

    let sync = sync_frame(&mut reader)?;
    let header = read_frame_header(&mut reader, sync)?;

    let bits_per_sample = match header.bits_per_sample.or(self.params.bits_per_sample) {
      Some(bits) => bits,
      None => return decode_error("flac: bits per sample not provided"),
    };

    let block = u32::from(header.block_num_samples);

    let Ok(()) = self.buf.clear();
    self.buf.render_reserved(Some(block))?;

    let Ok(available) = reader.read_buf_bytes_available_ref();
    let Ok(mut bs) = BitReaderLtr::new(available);

    read_subframes(
      &mut bs,
      header.channel_assignment,
      bits_per_sample,
      &mut self.buf,
    )?;

    match self.validator.as_mut() {
      Some(validator) => {
        validator.update(&self.buf, bits_per_sample)?;
      }
      None => (),
    }

    let shift = 32u32.saturating_sub(bits_per_sample);

    let Ok(()) = self.buf.transform(|sample| sample.wrapping_shl(shift));

    Ok(())
  }
}

impl Decoder for FlacDecoder {
  fn try_new(params: &CodecParameters, options: &DecoderOptions) -> Result<Self> {
    match params.codec {
      CODEC_TYPE_FLAC => (),
      _other => return unsupported_error("flac: invalid codec type"),
    }

    let extra_data = match params.extra_data.as_ref() {
      Some(buf) => buf,
      None => return unsupported_error("flac: missing extra data"),
    };

    let Ok(mut extra) = BufReader::new(extra_data);
    let info = StreamInfo::read(&mut extra)?;

    let mut params_2 = params.clone();

    let time_base = TimeBase::new(1, info.sample_rate)?;

    let Ok(_configured_with_sample_rate) = params_2.with_sample_rate(info.sample_rate);
    let Ok(_configured_with_time_base) = params_2.with_time_base(time_base);
    let Ok(_configured_with_bits_per_sample) =
      params_2.with_bits_per_sample(info.bits_per_sample);
    let Ok(_configured_with_max_frames_per_packet) =
      params_2.with_max_frames_per_packet(u64::from(info.block_len_max));
    let Ok(_configured_with_channels) = params_2.with_channels(info.channels);

    match info.md5 {
      Some(md5) => {
        let Ok(_configured_with_verification_code) =
          params_2.with_verification_code(VerificationCheck::Md5(md5));
      }
      None => (),
    }

    match info.n_samples {
      Some(n_frames) => {
        let Ok(_configured_with_n_frames) = params_2.with_n_frames(n_frames);
      }
      None => (),
    }

    let Ok(spec) = SignalSpec::new(info.sample_rate, info.channels);
    let buf = AudioBuffer::new(u64::from(info.block_len_max), spec)?;

    let validator = match options.verification {
      VerificationMode::Verify => Some(Validator::default()),
      VerificationMode::Skip => None,
    };

    Ok(FlacDecoder {
      params: params_2,
      validator,
      buf,
    })
  }

  fn supported_codecs()
  -> std::result::Result<&'static [CodecDescriptor], console_core_never::Never> {
    Ok(&[support_codec!(
      CODEC_TYPE_FLAC,
      "flac",
      "Free Lossless Audio Codec"
    )])
  }

  fn reset(&mut self) -> std::result::Result<(), console_core_never::Never> {
    Ok(())
  }

  fn codec_params(&self) -> std::result::Result<&CodecParameters, console_core_never::Never> {
    Ok(&self.params)
  }

  fn decode(&mut self, packet: &Packet) -> Result<AudioBufferRef<'_>> {
    match self.decode_inner(packet) {
      Ok(()) => {
        let Ok(value_2) = self.buf.as_audio_buffer_ref();

        Ok(value_2)
      }
      Err(fault) => {
        let Ok(()) = self.buf.clear();

        Err(fault)
      }
    }
  }

  fn finalize(&mut self) -> std::result::Result<FinalizeResult, console_core_never::Never> {
    let verification = match (self.validator.as_ref(), self.params.verification_check) {
      (Some(validator), Some(VerificationCheck::Md5(expected))) => {
        let Ok(decoded) = validator.md5();

        VerificationOutcome::from(decoded == expected)
      }
      (
        Some(_),
        Some(
          VerificationCheck::Crc8(_)
          | VerificationCheck::Crc16(_)
          | VerificationCheck::Crc32(_)
          | VerificationCheck::Other(_),
        ),
      )
      | (Some(_), None)
      | (None, _) => VerificationOutcome::NotRun,
    };

    Ok(FinalizeResult { verification })
  }

  fn last_decoded(&self) -> std::result::Result<AudioBufferRef<'_>, console_core_never::Never> {
    let Ok(value_3) = self.buf.as_audio_buffer_ref();

    Ok(value_3)
  }
}

#[derive(Debug, Clone, Copy)]
enum SubFrameType {
  Constant,
  Verbatim,
  FixedLinear(Order),
  Linear(Order),
}

#[derive(Debug, Clone, Copy)]
struct Order(u32);

struct RiceCoding {
  width: u32,
  escape: u32,
}

fn subframe_type(encoded: u32) -> Result<SubFrameType> {
  match encoded {
    0x00 => Ok(SubFrameType::Constant),
    0x01 => Ok(SubFrameType::Verbatim),
    0x08..=0x0c => Ok(SubFrameType::FixedLinear(Order(encoded & 0x07))),
    0x0d..=0x0f => decode_error("flac: fixed predictor orders of greater than 4 are invalid"),
    0x20..=0x3f => Ok(SubFrameType::Linear(Order(
      (encoded & 0x1f).saturating_add(1),
    ))),
    _ => decode_error("flac: subframe type set to reserved value"),
  }
}

fn read_subframe<B: ReadBitsLtr>(bs: &mut B, frame_bps: u32, buf: &mut [i32]) -> Result<()> {
  let padding = bs.read_flag()?;

  match padding {
    FlagBit::One => return decode_error("flac: subframe padding is not 0"),
    FlagBit::Zero => (),
  }

  let encoded = bs.read_bits_leq32(6)?;
  let kind = subframe_type(encoded)?;

  let wasted = bs.read_flag()?;

  let dropped_bps = match wasted {
    FlagBit::One => {
      let zeros = bs.read_unary_zeros()?;

      zeros.saturating_add(1)
    }
    FlagBit::Zero => 0,
  };

  let bps = match frame_bps.checked_sub(dropped_bps) {
    Some(bps) => bps,
    None => return decode_error("flac: wasted bits exceed the sample width"),
  };

  match kind {
    SubFrameType::Constant => {
      decode_constant(bs, bps, buf)?;
    }
    SubFrameType::Verbatim => {
      decode_verbatim(bs, bps, buf)?;
    }
    SubFrameType::FixedLinear(order) => {
      decode_fixed_linear(bs, bps, order, buf)?;
    }
    SubFrameType::Linear(order) => {
      decode_linear(bs, bps, order, buf)?;
    }
  }

  for sample in buf.iter_mut() {
    *sample = sample.wrapping_shl(dropped_bps);
  }

  Ok(())
}

fn decode_constant<B: ReadBitsLtr>(bs: &mut B, bps: u32, buf: &mut [i32]) -> Result<()> {
  let const_sample = bs.read_bits_leq32_signed(bps)?;

  for sample in buf.iter_mut() {
    *sample = const_sample;
  }

  Ok(())
}

fn decode_verbatim<B: ReadBitsLtr>(bs: &mut B, bps: u32, buf: &mut [i32]) -> Result<()> {
  for sample in buf.iter_mut() {
    let value = bs.read_bits_leq32_signed(bps)?;

    *sample = value;
  }

  Ok(())
}

fn warm_up(order: Order, buf: &mut [i32]) -> Result<&mut [i32]> {
  let Ok(warm) = index(order.0);

  match buf.get_mut(..warm) {
    Some(warm_up) => Ok(warm_up),
    None => decode_error("flac: predictor order exceeds the block"),
  }
}

fn decode_fixed_linear<B: ReadBitsLtr>(
  bs: &mut B,
  bps: u32,
  order: Order,
  buf: &mut [i32],
) -> Result<()> {
  let warm = warm_up(order, buf)?;

  decode_verbatim(bs, bps, warm)?;
  decode_residual(bs, order.0, buf)?;

  let Ok(at) = index(order.0);

  match FIXED_COEFFICIENTS.get(at) {
    Some(coefficients) => {
      let Ok(()) = predict(coefficients, 0, buf);

      Ok(())
    }
    None => decode_error("flac: fixed predictor orders of greater than 4 are invalid"),
  }
}

fn read_coefficients<B: ReadBitsLtr>(
  bs: &mut B,
  order: Order,
  precision: u32,
) -> Result<Vec<i32>> {
  let mut coefficients = Vec::new();

  for _ in 0..order.0 {
    let coefficient = bs.read_bits_leq32_signed(precision)?;

    coefficients.push(coefficient);
  }

  coefficients.reverse();

  Ok(coefficients)
}

fn decode_linear<B: ReadBitsLtr>(
  bs: &mut B,
  bps: u32,
  order: Order,
  buf: &mut [i32],
) -> Result<()> {
  match order.0 {
    0..=MAX_LINEAR_ORDER => (),
    _out_of_range => return decode_error("flac: linear predictor order is out of range"),
  }

  let warm = warm_up(order, buf)?;

  decode_verbatim(bs, bps, warm)?;

  let encoded_precision = bs.read_bits_leq32(4)?;
  let precision = encoded_precision.saturating_add(1);

  match precision {
    1..=15 => (),
    _ => return decode_error("flac: qlp precision set to reserved value"),
  }

  let shift = bs.read_bits_leq32_signed(5)?;

  let shift_3 = match u32::try_from(shift) {
    Ok(shift_2) => shift_2,
    Err(_negative) => return unsupported_error("flac: lpc shifts less than 0 are not supported"),
  };

  let coefficients = read_coefficients(bs, order, precision)?;

  decode_residual(bs, order.0, buf)?;

  let Ok(()) = predict(&coefficients, shift_3, buf);

  Ok(())
}

fn rice_parameter_width(method: u32) -> Result<RiceCoding> {
  match method {
    0x0 => Ok(RiceCoding {
      width: 4,
      escape: 0xf,
    }),
    0x1 => Ok(RiceCoding {
      width: 5,
      escape: 0x1f,
    }),
    _ => decode_error("flac: residual method set to reserved value"),
  }
}

fn decode_residual<B: ReadBitsLtr>(
  bs: &mut B,
  n_prelude_samples: u32,
  buf: &mut [i32],
) -> Result<()> {
  let method = bs.read_bits_leq32(2)?;
  let coding = rice_parameter_width(method)?;

  let order = bs.read_bits_leq32(4)?;

  let Ok(length) = fitted::<_, u32>(buf.len());
  let n_partitions = 1u32.wrapping_shl(order);
  let n_partition_samples = length.wrapping_shr(order);

  match n_prelude_samples.cmp(&n_partition_samples) {
    Ordering::Greater => {
      return decode_error("flac: residual partition too small for given predictor order");
    }
    Ordering::Less | Ordering::Equal => (),
  }

  let expected = n_partitions.checked_mul(n_partition_samples);

  match expected.filter(|total| *total == length) {
    Some(_) => (),
    None => return decode_error("flac: block size is not same as encoded residual"),
  }

  let Ok(partition) = index(n_partition_samples);
  let Ok(prelude) = index(n_prelude_samples);

  let (first, rest) = match buf.split_at_mut_checked(partition) {
    Some(parts) => parts,
    None => return decode_error("flac: block size is not same as encoded residual"),
  };

  match first.get_mut(prelude..) {
    Some(first_2) => {
      decode_rice_partition(bs, &coding, first_2)?;
    }
    None => return decode_error("flac: residual partition too small for given predictor order"),
  }

  match partition {
    0 => return decode_error("flac: block size is not same as encoded residual"),
    _ => (),
  }

  for chunk in rest.chunks_mut(partition.max(1)) {
    decode_rice_partition(bs, &coding, chunk)?;
  }

  Ok(())
}

fn decode_rice_partition<B: ReadBitsLtr>(
  bs: &mut B,
  coding: &RiceCoding,
  buf: &mut [i32],
) -> Result<()> {
  let rice_param = bs.read_bits_leq32(coding.width)?;

  match rice_param.cmp(&coding.escape) {
    Ordering::Less | Ordering::Greater => {
      for sample in buf.iter_mut() {
        let q = bs.read_unary_zeros()?;
        let r = bs.read_bits_leq32(rice_param)?;
        let Ok(residual) = rice_signed_to_i32(q.wrapping_shl(rice_param) | r);

        *sample = residual;
      }
    }
    Ordering::Equal => {
      let residual_bits = bs.read_bits_leq32(5)?;

      for sample in buf.iter_mut() {
        let value = bs.read_bits_leq32_signed(residual_bits)?;

        *sample = value;
      }
    }
  }

  Ok(())
}

fn rice_signed_to_i32(word: u32) -> std::result::Result<i32, Never> {
  let Ok(half) = fitted::<u32, i32>(word.wrapping_shr(1));
  let Ok(odd) = fitted::<u32, i32>(word & 1);

  Ok(half ^ odd.wrapping_neg())
}

fn low_word(value: i64) -> std::result::Result<i32, Never> {
  let [a, b, c, d, ..] = value.to_le_bytes();

  Ok(i32::from_le_bytes([a, b, c, d]))
}

fn predict(coefficients: &[i32], shift: u32, buf: &mut [i32]) -> std::result::Result<(), Never> {
  let Ok(order) = index(coefficients.len());

  let cells = Cell::from_mut(buf).as_slice_of_cells();

  for window in cells.windows(order.saturating_add(1)) {
    match window.split_last() {
      Some((target, history)) => {
        let predicted = coefficients.iter().zip(history).fold(0i64, |sum, (&c, s)| {
          sum.wrapping_add(i64::from(c).wrapping_mul(i64::from(s.get())))
        });
        let Ok(correction) = low_word(predicted.wrapping_shr(shift));

        target.set(target.get().wrapping_add(correction));
      }
      None => (),
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_rice_word_alternates_between_the_positive_and_the_negative() {
    let table = [
      (0, 0),
      (1, -1),
      (2, 1),
      (3, -2),
      (4, 2),
      (5, -3),
      (6, 3),
      (7, -4),
      (8, 4),
      (9, -5),
      (10, 5),
    ];

    for (word, expected) in table {
      assert_eq!(rice_signed_to_i32(word), Ok(expected));
    }

    assert_eq!(rice_signed_to_i32(u32::MAX), Ok(-2_147_483_648));
  }

  #[test]
  fn a_second_order_fixed_predictor_continues_a_ramp() {
    let mut ramp = [0, 10, 0, 0, 0];

    assert_eq!(predict(&[-1, 2], 0, &mut ramp), Ok(()));
    assert_eq!(ramp, [0, 10, 20, 30, 40]);
  }
}
