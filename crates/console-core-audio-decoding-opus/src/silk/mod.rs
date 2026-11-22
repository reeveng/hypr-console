//! SILK layer decoder (RFC 6716 section 4.2).
//!
//! Parameter decoding and reconstruction follow the normative fixed-point arithmetic, so the
//! output matches the reference decoder sample for sample at the internal rate, and the
//! interpolator in `resampler` takes it to 48 kHz. A packet's in-band redundancy is read past
//! rather than decoded, because nothing here conceals a loss with it; a lost frame is concealed
//! by repeating the last pitch period or, where there was none, by noise shaped by the last
//! filter. The decoder is handed its range decoder when there is a frame to read and nothing
//! when the frame was lost, so the two cases are one argument rather than a flag and a decoder
//! that might disagree with it.
//!
//! The shell decoder splits a block of sixteen pulses in halves until every half is one pulse
//! wide or empty; the halves are kept on a stack of their own rather than in the frames of a
//! function that calls itself, and a block is at most thirty-one splits.

pub(crate) mod lpc;
mod resampler;
pub(crate) mod tables;

use std::cmp::Ordering;

use std::cell::Cell;
use std::num::NonZeroU32;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index, narrow_f32, toward_zero_i32};

use lpc::{
  Codebook, Ratio, Width, bwexpander, div32_varq, inverse32_varq, narrow, nlsf_decode, nlsf2a,
  rshift_round, sat16, set_at, slice_at, smulwb, smulwb_low, smulww,
};
use resampler::Resampler;
use tables::{
  CB_NB_10, CB_NB_20, CB_WB_10, CB_WB_20, DELTA_GAIN, EXC_LSB, EXC_SIGN, GAIN_MSB, LBRR_FLAGS,
  LTP_FILTER, LTP_PERIODICITY, LTP_SCALE, LTP_TAPS_0, LTP_TAPS_1, LTP_TAPS_2, MID_ONLY, NLSF_EXT,
  NLSF_INTERP, NLSF_SEL_NBMB, NLSF_SEL_WB, NLSF_STAGE1, NLSF_STAGE2, PITCH_CONTOUR, PITCH_DELTA,
  PITCH_HIGH, PITCH_LOW, PULSE_COUNT, RATE_LEVEL, SHELL_2, SHELL_4, SHELL_8, SHELL_16,
  STEREO_PRED_JOINT, STEREO_WEIGHTS_Q13, TYPE_ACTIVE, TYPE_INACTIVE, UNIFORM3, UNIFORM4,
  UNIFORM5, UNIFORM8,
};

use crate::packet::Channels;
use crate::quantities::LossFade;
use crate::range::{Bit, RangeDecoder};

const VOICED: u32 = 2;
const LTP_ORDER: u32 = 5;
const PULSE_BLOCK: u32 = 16;
const LSB_MARKER: u32 = 17;
const MOST_LSBS: u32 = 10;
const SHELL_SPLITS: u32 = 31;
const OUTPUT_RATE: u32 = 48_000;
const FULL_SCALE: f32 = 32768.0;
const PLC_FADE: f32 = 0.85;
const PLC_NOISE: f32 = 1.7;
const PLC_CHIRP: i32 = 64880;
const LOSS_CHIRP: i32 = 63570;
const RESET_LAG: i32 = 100;
const RESET_GAIN_INDEX: i32 = 10;
const UNITY_Q16: i32 = 1 << 16;
const LTP_UNIT: [i32; 5] = [0, 0, 1 << 12, 0, 0];
const RAND_MULTIPLIER: i32 = 196_314_165;
const RAND_INCREMENT: i32 = 907_633_515;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cond {
  Independently,
  IndependentlyNoLtpScaling,
  Conditionally,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Coding {
  Regular,
  Redundant,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum History {
  Fresh,
  Continuing,
}

enum Reception {
  Received,
  Lost,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Middle {
  Both,
  MidOnly,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MonoSide {
  Duplicated,
  Resampled,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Start {
  NewPacket,
  Continuing,
}

#[derive(Clone, Default)]
struct Indices {
  gains: [i32; 4],
  ltp_index: [u32; 4],
  nlsf: [i32; 17],
  lag_index: i32,
  contour_index: u32,
  signal_type: u32,
  quant_offset_type: u32,
  nlsf_interp_q2: i32,
  per_index: u32,
  ltp_scale_index: u32,
  seed: i32,
}

struct Channel {
  fs_khz: u32,
  subframes: u32,
  frame_length: u32,
  subframe_length: u32,
  ltp_memory: u32,
  order: u32,
  prev_gain_q16: i32,
  s_lpc_q14: [i32; 16],
  out_buf: [i16; 480],
  lag_prev: i32,
  last_gain_index: i32,
  prev_signal_type: u32,
  history: History,
  prev_nlsf_q15: [i32; 16],
  ec_prev_signal_type: u32,
  ec_prev_lag_index: i32,
  vad_flags: [Bit; 3],
  lbrr_flag: Bit,
  lbrr_flags: [Bit; 3],
  frames_decoded: u32,
  frames_per_packet: u32,
  loss_count: u32,
  indices: Indices,
  resampler: Option<Resampler>,
  plc_lpc_q12: [i32; 16],
  plc_pitch: i32,
  plc_rand: u32,
}

#[derive(Default)]
struct Ctrl {
  gains_q16: [i32; 4],
  pred_coef_q12: [[i32; 16]; 2],
  pitch_l: [i32; 4],
  ltp_coef_q14: [i32; 20],
  ltp_scale_q14: i32,
}

struct Synthesis {
  s_ltp: [i16; 320],
  s_ltp_q15: [i32; 640],
  res_q14: [i32; 80],
  s_lpc: [i32; 96],
  ltp_buf_idx: u32,
}

struct Subframe {
  number: u32,
  lag: i32,
  taps: [i32; 5],
  inverse_gain: i32,
  adjustment: i32,
}

#[derive(Clone, Copy)]
struct Count {
  pulses: u32,
  lsbs: u32,
}

#[derive(Clone, Copy)]
struct Split {
  from: u32,
  width: u32,
  count: u32,
}

#[derive(Clone, Copy)]
struct Shape {
  fs_khz: u32,
  length: u32,
}

#[derive(Default, Clone)]
struct Stereo {
  pred_prev_q13: [i32; 2],
  mid: [i16; 2],
  side: [i16; 2],
}

fn row<'a>(tables: &[&'a [u8]], place: u32) -> Result<&'a [u8], Never> {
  let Ok(at) = index(place);

  Ok(match tables.get(at) {
    Some(table) => table,
    None => &[],
  })
}

fn flag(flags: &[Bit; 3], place: u32) -> Result<Bit, Never> {
  let Ok(at) = index(place);

  Ok(match flags.get(at) {
    Some(bit) => *bit,
    None => Bit::Zero,
  })
}

fn clipped(value: i32) -> Result<i16, Never> {
  let Ok(saturated) = sat16(value);

  fitted::<i32, i16>(saturated)
}

fn float(value: i32) -> Result<f32, Never> {
  narrow_f32(f64::from(value))
}

fn log2lin(in_log_q7: i32) -> Result<i32, Never> {
  let out = 1i32.wrapping_shl(in_log_q7.wrapping_shr(7).cast_unsigned());
  let fraction = in_log_q7 & 0x7f;
  let Ok(curve) = smulwb(fraction.wrapping_mul(128i32.wrapping_sub(fraction)), -174);
  let bend = fraction.wrapping_add(curve);

  Ok(match in_log_q7.cmp(&2048) {
    Ordering::Less => out.wrapping_add(out.wrapping_mul(bend).wrapping_shr(7)),
    Ordering::Equal | Ordering::Greater => {
      out.wrapping_add(out.wrapping_shr(7).wrapping_mul(bend))
    }
  })
}

fn lpc_analysis_filter(
  out: &mut [i16],
  input: &[i16],
  filter: &[i32; 16],
  order: u32,
) -> Result<(), Never> {
  let Ok(taps) = index(order);

  for (slot, window) in out
    .iter_mut()
    .skip(taps)
    .zip(input.windows(taps.saturating_add(1)))
  {
    match window.split_last() {
      Some((current, history)) => {
        let mut sum = 0i32;

        for (earlier, coefficient) in history.iter().rev().zip(filter.iter()) {
          sum = sum.wrapping_add(i32::from(*earlier).wrapping_mul(*coefficient));
        }

        let residual = i32::from(*current).wrapping_shl(12).wrapping_sub(sum);
        let Ok(rounded) = rshift_round(residual, 12);
        let Ok(value) = clipped(rounded);

        *slot = value;
      }
      None => {}
    }
  }

  for slot in out.iter_mut().take(taps) {
    *slot = 0;
  }

  Ok(())
}

fn pulse_counts(
  dec: &mut RangeDecoder<'_>,
  crate::quantities::SignalType(signal_type): crate::quantities::SignalType,
  crate::quantities::BlockCount(blocks): crate::quantities::BlockCount,
) -> Result<[Count; 20], Never> {
  let Ok(level_table) = row(&RATE_LEVEL, signal_type.wrapping_shr(1));
  let Ok(rate_level) = dec.symbol(level_table, 8);
  let Ok(taken) = index(blocks);
  let mut counts = [Count { pulses: 0, lsbs: 0 }; 20];

  for count in counts.iter_mut().take(taken) {
    let Ok(table) = row(&PULSE_COUNT, rate_level);
    let Ok(first) = dec.symbol(table, 8);
    let mut pulses = first;
    let mut lsbs = 0u32;

    'pulse_bits: for _round in 0..MOST_LSBS {
      match pulses.cmp(&LSB_MARKER) {
        Ordering::Equal => {
          lsbs = lsbs.wrapping_add(1);

          let shifted = match lsbs.cmp(&MOST_LSBS) {
            Ordering::Equal => 10,
            Ordering::Less | Ordering::Greater => 9,
          };
          let Ok(table_2) = row(&PULSE_COUNT, shifted);
          let Ok(next) = dec.symbol(table_2, 8);

          pulses = next;
        }
        Ordering::Less | Ordering::Greater => break 'pulse_bits,
      }
    }

    *count = Count { pulses, lsbs };
  }

  Ok(counts)
}

fn decode_pulses(
  dec: &mut RangeDecoder<'_>,
  pulses: &mut [i32; 336],
  indices: &Indices,
  frame_length: u32,
) -> Result<(), Never> {
  let blocks = frame_length.div_ceil(PULSE_BLOCK);
  let Ok(mut counts) = pulse_counts(
    dec,
    crate::quantities::SignalType(indices.signal_type),
    crate::quantities::BlockCount(blocks),
  );
  let Ok(taken) = index(blocks);

  for (block, count) in pulses
    .as_chunks_mut::<16>()
    .0
    .iter_mut()
    .zip(counts.iter())
    .take(taken)
  {
    match count.pulses.cmp(&0) {
      Ordering::Greater => {
        let Ok(()) = shell_decode(dec, block, count.pulses);
      }
      Ordering::Less | Ordering::Equal => block.fill(0),
    }
  }

  for (block, count) in pulses
    .as_chunks_mut::<16>()
    .0
    .iter_mut()
    .zip(counts.iter_mut())
    .take(taken)
  {
    match count.lsbs.cmp(&0) {
      Ordering::Greater => {
        for pulse in block.iter_mut() {
          let mut value = *pulse;

          for _lsb in 0..count.lsbs {
            let Ok(bit) = dec.symbol(&EXC_LSB, 8);

            value = value.wrapping_shl(1).wrapping_add(bit.cast_signed());
          }

          *pulse = value;
        }

        count.pulses |= count.lsbs.wrapping_shl(5);
      }
      Ordering::Less | Ordering::Equal => {}
    }
  }

  let base = indices
    .quant_offset_type
    .wrapping_add(indices.signal_type.wrapping_shl(1))
    .wrapping_mul(7);

  for (block, count) in pulses
    .as_chunks_mut::<16>()
    .0
    .iter_mut()
    .zip(counts.iter())
    .take(taken)
  {
    match count.pulses.cmp(&0) {
      Ordering::Greater => {}
      Ordering::Less | Ordering::Equal => continue,
    }

    let Ok(table) = row(&EXC_SIGN, base.wrapping_add((count.pulses & 0x1f).min(6)));

    'over_pulse_2: for pulse in block.iter_mut() {
      match (*pulse).cmp(&0) {
        Ordering::Greater => {}
        Ordering::Less | Ordering::Equal => continue 'over_pulse_2,
      }

      let Ok(sign) = dec.symbol(table, 8);

      match sign {
        0 => *pulse = pulse.wrapping_neg(),
        _positive => {}
      }
    }
  }

  Ok(())
}

fn shell_table(
  crate::quantities::ShellWidth(width): crate::quantities::ShellWidth,
  crate::quantities::PulseCount(count): crate::quantities::PulseCount,
) -> Result<&'static [u8], Never> {
  let tables: &[&'static [u8]; 16] = match width {
    16 => &SHELL_16,
    8 => &SHELL_8,
    4 => &SHELL_4,
    _two => &SHELL_2,
  };

  row(tables, count.wrapping_sub(1))
}

fn shell_decode(
  dec: &mut RangeDecoder<'_>,
  block: &mut [i32; 16],
  total: u32,
) -> Result<(), Never> {
  let unused = Split {
    from: 0,
    width: 0,
    count: 0,
  };
  let root = Split {
    from: 0,
    width: PULSE_BLOCK,
    count: total.min(PULSE_BLOCK),
  };
  let mut stack = [root, unused, unused, unused, unused, unused, unused, unused];
  let mut height = 1u32;

  for _split in 0..SHELL_SPLITS {
    let top = match height.checked_sub(1) {
      Some(top) => top,
      None => break,
    };
    let Ok(at) = index(top);
    let node = match stack.get(at) {
      Some(node) => *node,
      None => unused,
    };

    height = top;

    let (width, count) = match (node.width, node.count) {
      (1, count) => {
        let Ok(()) = set_at(block, node.from, count.cast_signed());

        continue;
      }
      (_wider, 0) => {
        let Ok(from) = index(node.from);
        let Ok(width) = index(node.width);

        for pulse in block.iter_mut().skip(from).take(width) {
          *pulse = 0;
        }

        continue;
      }
      (wide, pulses) => (wide, pulses),
    };
    let Ok(table) = shell_table(
      crate::quantities::ShellWidth(width),
      crate::quantities::PulseCount(count),
    );
    let Ok(left) = dec.symbol(table, 8);
    let half = width.wrapping_shr(1);
    let right = Split {
      from: node.from.wrapping_add(half),
      width: half,
      count: count.wrapping_sub(left),
    };
    let first = Split {
      from: node.from,
      width: half,
      count: left,
    };

    for split in [right, first] {
      let Ok(at_2) = index(height);

      match stack.get_mut(at_2) {
        Some(slot) => *slot = split,
        None => {}
      }

      height = height.wrapping_add(1);
    }
  }

  Ok(())
}

fn stereo_weight(place: u32, fine: i32) -> Result<i32, Never> {
  let Ok(at) = index(place);
  let low = match STEREO_WEIGHTS_Q13.get(at) {
    Some(low) => *low,
    None => 0,
  };
  let high = match STEREO_WEIGHTS_Q13.get(at.saturating_add(1)) {
    Some(high) => *high,
    None => 0,
  };
  let Ok(step) = smulwb(high.wrapping_sub(low), 6554);

  Ok(low.wrapping_add(step.wrapping_mul(fine.wrapping_mul(2).wrapping_add(1))))
}

fn stereo_decode_pred(dec: &mut RangeDecoder<'_>) -> Result<[i32; 2], Never> {
  let Ok(joint) = dec.symbol(&STEREO_PRED_JOINT, 8);
  let coarse_first = joint.wrapping_div(5);
  let coarse_second = joint.wrapping_sub(coarse_first.wrapping_mul(5));
  let Ok(place_first) = dec.symbol(&UNIFORM3, 8);
  let Ok(fine_first) = dec.symbol(&UNIFORM5, 8);
  let Ok(place_second) = dec.symbol(&UNIFORM3, 8);
  let Ok(fine_second) = dec.symbol(&UNIFORM5, 8);
  let Ok(first) = stereo_weight(
    place_first.wrapping_add(coarse_first.wrapping_mul(3)),
    fine_first.cast_signed(),
  );
  let Ok(second) = stereo_weight(
    place_second.wrapping_add(coarse_second.wrapping_mul(3)),
    fine_second.cast_signed(),
  );

  Ok([first.wrapping_sub(second), second])
}

fn copied16(into: &mut [i16], from: &[i16]) -> Result<(), Never> {
  for (slot, value) in into.iter_mut().zip(from.iter()) {
    *slot = *value;
  }

  Ok(())
}

fn interpolation_step(
  crate::quantities::CurrentValue(now): crate::quantities::CurrentValue,
  crate::quantities::PreviousValue(before): crate::quantities::PreviousValue,
  denominator: i16,
) -> Result<i32, Never> {
  let Ok(change) = narrow(now.wrapping_sub(before));

  rshift_round(i32::from(change).wrapping_mul(i32::from(denominator)), 16)
}

impl Stereo {
  fn unmix(
    &mut self,
    mid: &mut [i16; 322],
    side: &mut [i16; 322],
    prediction: [i32; 2],
    shape: Shape,
  ) -> Result<(), Never> {
    let Ok(length) = index(shape.length);
    let Ok(()) = copied16(mid, &self.mid);
    let Ok(()) = copied16(side, &self.side);
    let Ok(()) = copied16(&mut self.mid, mid.get(length..).map_or(&[], |rest| rest));
    let Ok(()) = copied16(&mut self.side, side.get(length..).map_or(&[], |rest| rest));
    let [before_mid, before_side] = self.pred_prev_q13;
    let [now_mid, now_side] = prediction;
    let denominator = match 65536i32.checked_div(shape.fs_khz.wrapping_mul(8).cast_signed()) {
      Some(denominator) => denominator,
      None => 0,
    };
    let Ok(denominator_2) = narrow(denominator);
    let Ok(step_mid) = interpolation_step(
      crate::quantities::CurrentValue(now_mid),
      crate::quantities::PreviousValue(before_mid),
      denominator_2,
    );
    let Ok(step_side) = interpolation_step(
      crate::quantities::CurrentValue(now_side),
      crate::quantities::PreviousValue(before_side),
      denominator_2,
    );
    let interpolated = shape.fs_khz.wrapping_mul(8);
    let mut weight_mid = before_mid;
    let mut weight_side = before_side;

    for ((moment, window), out) in (0u32..)
      .zip(mid.windows(3))
      .zip(side.iter_mut().skip(1))
      .take(length)
    {
      match (moment.cmp(&interpolated), moment.cmp(&interpolated)) {
        (Ordering::Less, _) => {
          weight_mid = weight_mid.wrapping_add(step_mid);
          weight_side = weight_side.wrapping_add(step_side);
        }
        (Ordering::Equal | Ordering::Greater, Ordering::Equal) => {
          weight_mid = now_mid;
          weight_side = now_side;
        }
        (Ordering::Equal | Ordering::Greater, Ordering::Less | Ordering::Greater) => {}
      }

      match window {
        [earlier, current, later] => {
          let sum = i32::from(*earlier)
            .wrapping_add(i32::from(*later))
            .wrapping_add(i32::from(*current).wrapping_shl(1))
            .wrapping_shl(9);
          let Ok(from_sum) = smulwb_low(
            crate::quantities::WideWord(sum),
            crate::quantities::LowWord(weight_mid),
          );
          let Ok(from_mid) = smulwb_low(
            crate::quantities::WideWord(i32::from(*current).wrapping_shl(11)),
            crate::quantities::LowWord(weight_side),
          );
          let value = i32::from(*out)
            .wrapping_shl(8)
            .wrapping_add(from_sum)
            .wrapping_add(from_mid);
          let Ok(rounded) = rshift_round(value, 8);
          let Ok(result) = clipped(rounded);

          *out = result;
        }
        _uneven => {}
      }
    }

    self.pred_prev_q13 = prediction;

    for (left, right) in mid
      .iter_mut()
      .skip(1)
      .zip(side.iter_mut().skip(1))
      .take(length)
    {
      let sum = i32::from(*left).wrapping_add(i32::from(*right));
      let difference = i32::from(*left).wrapping_sub(i32::from(*right));
      let Ok(new_left) = clipped(sum);
      let Ok(new_right) = clipped(difference);

      *left = new_left;
      *right = new_right;
    }

    Ok(())
  }
}

impl Channel {
  fn new() -> Result<Channel, Never> {
    Ok(Channel {
      fs_khz: 0,
      subframes: 4,
      frame_length: 0,
      subframe_length: 0,
      ltp_memory: 0,
      order: 0,
      prev_gain_q16: UNITY_Q16,
      s_lpc_q14: [0; 16],
      out_buf: [0; 480],
      lag_prev: RESET_LAG,
      last_gain_index: RESET_GAIN_INDEX,
      prev_signal_type: 0,
      history: History::Fresh,
      prev_nlsf_q15: [0; 16],
      ec_prev_signal_type: 0,
      ec_prev_lag_index: 0,
      vad_flags: [Bit::Zero; 3],
      lbrr_flag: Bit::Zero,
      lbrr_flags: [Bit::Zero; 3],
      frames_decoded: 0,
      frames_per_packet: 1,
      loss_count: 0,
      indices: Indices::default(),
      resampler: None,
      plc_lpc_q12: [0; 16],
      plc_pitch: 0,
      plc_rand: 22222,
    })
  }

  fn forget(&mut self) -> Result<(), Never> {
    self.out_buf = [0; 480];
    self.s_lpc_q14 = [0; 16];
    self.lag_prev = RESET_LAG;
    self.last_gain_index = RESET_GAIN_INDEX;
    self.prev_signal_type = 0;
    self.history = History::Fresh;

    Ok(())
  }

  fn set_fs(&mut self, fs_khz: u32) -> Result<(), Never> {
    self.subframe_length = fs_khz.wrapping_mul(5);

    let frame_length = self.subframes.wrapping_mul(self.subframe_length);
    let rate = fs_khz.wrapping_mul(1000);
    let current = match &self.resampler {
      Some(resampler) => {
        let Ok(rate_2) = resampler.input_rate();

        Some(rate_2)
      }
      None => None,
    };

    match (self.fs_khz.cmp(&fs_khz), current.cmp(&Some(rate))) {
      (Ordering::Equal, Ordering::Equal) => {}
      (Ordering::Less | Ordering::Greater, _)
      | (Ordering::Equal, Ordering::Less | Ordering::Greater) => {
        let Ok(made) = Resampler::new(rate);

        self.resampler = Some(made);
      }
    }

    match (
      self.fs_khz.cmp(&fs_khz),
      frame_length.cmp(&self.frame_length),
    ) {
      (Ordering::Equal, Ordering::Equal) => {}
      (Ordering::Less | Ordering::Greater, _) => {
        self.ltp_memory = fs_khz.wrapping_mul(20);
        self.order = match fs_khz {
          16 => 16,
          _narrower => 10,
        };

        let Ok(()) = self.forget();

        self.fs_khz = fs_khz;
        self.frame_length = frame_length;
      }
      (Ordering::Equal, Ordering::Less | Ordering::Greater) => self.frame_length = frame_length,
    }

    Ok(())
  }

  fn codebook(&self) -> Result<Codebook, Never> {
    Ok(match self.order {
      16 => Codebook::Wide,
      _narrower => Codebook::NarrowMedium,
    })
  }

  fn decode_indices(
    &mut self,
    dec: &mut RangeDecoder<'_>,
    frame: u32,
    coding: Coding,
    cond: Cond,
  ) -> Result<(), Never> {
    let Ok(voice) = flag(&self.vad_flags, frame);
    let kind = match (coding, voice) {
      (Coding::Redundant, _) | (Coding::Regular, Bit::One) => {
        let Ok(symbol) = dec.symbol(&TYPE_ACTIVE, 8);

        symbol.wrapping_add(2)
      }
      (Coding::Regular, Bit::Zero) => {
        let Ok(symbol) = dec.symbol(&TYPE_INACTIVE, 8);

        symbol
      }
    };

    self.indices.signal_type = kind.wrapping_shr(1);
    self.indices.quant_offset_type = kind & 1;

    let Ok(()) = self.decode_gain_indices(dec, cond);
    let Ok(()) = self.decode_nlsf_indices(dec);

    match self.indices.signal_type.cmp(&VOICED) {
      Ordering::Equal => {
        let Ok(()) = self.decode_pitch_indices(dec, cond);
      }
      Ordering::Less | Ordering::Greater => {}
    }

    self.ec_prev_signal_type = self.indices.signal_type;

    let Ok(seed) = dec.symbol(&UNIFORM4, 8);

    self.indices.seed = seed.cast_signed();

    Ok(())
  }

  fn decode_gain_indices(&mut self, dec: &mut RangeDecoder<'_>, cond: Cond) -> Result<(), Never> {
    let first = match cond {
      Cond::Conditionally => {
        let Ok(delta) = dec.symbol(&DELTA_GAIN, 8);

        delta.cast_signed()
      }
      Cond::Independently | Cond::IndependentlyNoLtpScaling => {
        let Ok(table) = row(&GAIN_MSB, self.indices.signal_type);
        let Ok(high) = dec.symbol(table, 8);
        let Ok(low) = dec.symbol(&UNIFORM8, 8);

        high
          .cast_signed()
          .wrapping_shl(3)
          .wrapping_add(low.cast_signed())
      }
    };
    let Ok(subframes) = index(self.subframes);
    let [slot, ..] = &mut self.indices.gains;

    *slot = first;

    for gain in self.indices.gains.iter_mut().take(subframes).skip(1) {
      let Ok(delta) = dec.symbol(&DELTA_GAIN, 8);

      *gain = delta.cast_signed();
    }

    Ok(())
  }

  fn decode_nlsf_indices(&mut self, dec: &mut RangeDecoder<'_>) -> Result<(), Never> {
    let Ok(codebook) = self.codebook();
    let wide = match codebook {
      Codebook::Wide => 2,
      Codebook::NarrowMedium => 0,
    };
    let Ok(stage1) = row(
      &NLSF_STAGE1,
      self.indices.signal_type.wrapping_shr(1).wrapping_add(wide),
    );
    let Ok(first) = dec.symbol(stage1, 8);
    let Ok(at) = index(first);
    let selection: &[u8] = match codebook {
      Codebook::Wide => NLSF_SEL_WB.get(at).map_or(&[], |chosen| chosen.as_slice()),
      Codebook::NarrowMedium => NLSF_SEL_NBMB
        .get(at)
        .map_or(&[], |chosen| chosen.as_slice()),
    };
    let offset = match codebook {
      Codebook::Wide => 8,
      Codebook::NarrowMedium => 0,
    };

    let [slot, ..] = &mut self.indices.nlsf;

    *slot = first.cast_signed();

    for (slot_2, chosen) in self.indices.nlsf.iter_mut().skip(1).zip(selection.iter()) {
      let Ok(table) = row(&NLSF_STAGE2, u32::from(*chosen).wrapping_add(offset));
      let Ok(coded) = dec.symbol(table, 8);
      let value = match coded {
        0 => {
          let Ok(extension) = dec.symbol(&NLSF_EXT, 8);

          extension.cast_signed().wrapping_neg()
        }
        8 => {
          let Ok(extension) = dec.symbol(&NLSF_EXT, 8);

          extension.cast_signed().wrapping_add(8)
        }
        _inner => coded.cast_signed(),
      };

      *slot_2 = value.wrapping_sub(4);
    }

    self.indices.nlsf_interp_q2 = match self.subframes {
      4 => {
        let Ok(interpolation) = dec.symbol(&NLSF_INTERP, 8);

        interpolation.cast_signed()
      }
      _shorter => 4,
    };

    Ok(())
  }

  fn decode_pitch_indices(
    &mut self,
    dec: &mut RangeDecoder<'_>,
    cond: Cond,
  ) -> Result<(), Never> {
    let delta = match (cond, self.ec_prev_signal_type.cmp(&VOICED)) {
      (Cond::Conditionally, Ordering::Equal) => {
        let Ok(delta) = dec.symbol(&PITCH_DELTA, 8);

        match delta.cmp(&0) {
          Ordering::Greater => Some(delta.cast_signed()),
          Ordering::Less | Ordering::Equal => None,
        }
      }
      (Cond::Conditionally, Ordering::Less | Ordering::Greater)
      | (Cond::Independently | Cond::IndependentlyNoLtpScaling, _) => None,
    };

    self.indices.lag_index = match delta {
      Some(delta_2) => self.ec_prev_lag_index.wrapping_add(delta_2).wrapping_sub(9),
      None => {
        let Ok(high) = dec.symbol(&PITCH_HIGH, 8);
        let low_table = match self.fs_khz {
          8 => 0,
          12 => 1,
          _wideband => 2,
        };
        let Ok(table) = row(&PITCH_LOW, low_table);
        let Ok(low) = dec.symbol(table, 8);

        high
          .cast_signed()
          .wrapping_mul(self.fs_khz.cast_signed().wrapping_shr(1))
          .wrapping_add(low.cast_signed())
      }
    };
    self.ec_prev_lag_index = self.indices.lag_index;

    let contour = match (self.fs_khz, self.subframes) {
      (8, 2) => 0,
      (8, _four) => 1,
      (_wider, 2) => 2,
      (_wider, _four) => 3,
    };
    let Ok(table) = row(&PITCH_CONTOUR, contour);
    let Ok(contour_index) = dec.symbol(table, 8);
    let Ok(per_index) = dec.symbol(&LTP_PERIODICITY, 8);
    let Ok(filter) = row(&LTP_FILTER, per_index);
    let Ok(subframes) = index(self.subframes);

    self.indices.contour_index = contour_index;
    self.indices.per_index = per_index;

    for slot in self.indices.ltp_index.iter_mut().take(subframes) {
      let Ok(chosen) = dec.symbol(filter, 8);

      *slot = chosen;
    }

    self.indices.ltp_scale_index = match cond {
      Cond::Independently => {
        let Ok(scale) = dec.symbol(&LTP_SCALE, 8);

        scale
      }
      Cond::IndependentlyNoLtpScaling | Cond::Conditionally => 0,
    };

    Ok(())
  }

  fn decode_frame(
    &mut self,
    dec: Option<&mut RangeDecoder<'_>>,
    out: &mut [i16],
    cond: Cond,
  ) -> Result<(), Never> {
    match dec {
      Some(dec_2) => {
        let Ok(()) = self.decode_indices(dec_2, self.frames_decoded, Coding::Regular, cond);
        let mut pulses = [0i32; 336];
        let Ok(()) = decode_pulses(dec_2, &mut pulses, &self.indices, self.frame_length);
        let Ok(ctrl) = self.decode_parameters(cond);
        let Ok(()) = self.decode_core(&ctrl, &pulses, out);
        let Ok(last) = index(self.subframes.wrapping_sub(1));
        let pitch = match ctrl.pitch_l.get(last) {
          Some(pitch) => *pitch,
          None => 0,
        };

        let [_first, second] = ctrl.pred_coef_q12;

        self.plc_lpc_q12 = second;
        self.plc_pitch = match self.indices.signal_type.cmp(&VOICED) {
          Ordering::Equal => pitch,
          Ordering::Less | Ordering::Greater => 0,
        };
        self.loss_count = 0;
        self.prev_signal_type = self.indices.signal_type;
        self.history = History::Continuing;
        self.lag_prev = pitch;
      }
      None => {
        let Ok(()) = self.conceal(out);
      }
    }

    let kept = self.ltp_memory.saturating_sub(self.frame_length);
    let Ok(kept_2) = index(kept);
    let Ok(length) = index(self.frame_length);

    match length.checked_add(kept_2) {
      Some(end) => match end.cmp(&self.out_buf.len()) {
        Ordering::Less | Ordering::Equal => self.out_buf.copy_within(length..end, 0),
        Ordering::Greater => {}
      },
      None => {}
    }

    for (slot, value) in self
      .out_buf
      .iter_mut()
      .skip(kept_2)
      .zip(out.iter())
      .take(length)
    {
      *slot = *value;
    }

    Ok(())
  }

  fn dequantise_gains(&mut self, cond: Cond) -> Result<[i32; 4], Never> {
    let mut previous = self.last_gain_index;
    let mut gains = [0i32; 4];
    let Ok(subframes) = index(self.subframes);

    for ((number, slot), coded) in (0u32..)
      .zip(gains.iter_mut())
      .zip(self.indices.gains.iter())
      .take(subframes)
    {
      previous = match (cond, number) {
        (Cond::Independently | Cond::IndependentlyNoLtpScaling, 0) => {
          (*coded).max(previous.wrapping_sub(16))
        }
        _relative => {
          let step = coded.wrapping_sub(4);
          let threshold = previous.wrapping_add(8);

          match step.cmp(&threshold) {
            Ordering::Greater => previous
              .wrapping_add(step.wrapping_shl(1))
              .wrapping_sub(threshold),
            Ordering::Less | Ordering::Equal => previous.wrapping_add(step),
          }
        }
      };
      previous = previous.clamp(0, 63);

      let Ok(level) = narrow(previous);
      let Ok(scaled) = smulwb(0x1D_1C71, level);
      let Ok(gain) = log2lin(scaled.wrapping_add(2090).min(3967));

      *slot = gain;
    }

    self.last_gain_index = previous;

    Ok(gains)
  }

  fn filters(&mut self) -> Result<[[i32; 16]; 2], Never> {
    let Ok(codebook) = self.codebook();
    let Ok(nlsf) = nlsf_decode(&self.indices.nlsf, codebook);
    let Ok(length) = index(self.order);
    let used = match nlsf.get(..length) {
      Some(used) => used,
      None => &[],
    };
    let Ok(second) = nlsf2a(used);

    match self.history {
      History::Fresh => self.indices.nlsf_interp_q2 = 4,
      History::Continuing => {}
    }

    let first = match self.indices.nlsf_interp_q2.cmp(&4) {
      Ordering::Less => {
        let mut blended = [0i32; 16];

        for ((slot, now), before) in blended
          .iter_mut()
          .zip(nlsf.iter())
          .zip(self.prev_nlsf_q15.iter())
          .take(length)
        {
          *slot = before.wrapping_add(
            self
              .indices
              .nlsf_interp_q2
              .wrapping_mul(now.wrapping_sub(*before))
              .wrapping_shr(2),
          );
        }

        let Ok(first) = nlsf2a(blended.get(..length).map_or(&[], |used_2| used_2));

        first
      }
      Ordering::Equal | Ordering::Greater => second,
    };

    self.prev_nlsf_q15 = nlsf;

    let mut filters = [first, second];

    match self.loss_count.cmp(&0) {
      Ordering::Greater => {
        for filter in filters.iter_mut() {
          let Ok(()) = bwexpander(
            filter.get_mut(..length).map_or(&mut [], |used_3| used_3),
            LOSS_CHIRP,
            Width::Sixteen,
          );
        }
      }
      Ordering::Less | Ordering::Equal => {}
    }

    Ok(filters)
  }

  fn pitch(&self, ctrl: &mut Ctrl) -> Result<(), Never> {
    let rate = self.fs_khz.cast_signed();
    let min_lag = rate.wrapping_mul(2);
    let max_lag = rate.wrapping_mul(18);
    let lag = min_lag.wrapping_add(self.indices.lag_index);
    let Ok(contour) = index(self.indices.contour_index);
    let offsets: &[i32] = match (self.fs_khz, self.subframes) {
      (8, 2) => CB_NB_10
        .get(contour)
        .map_or(&[], |offsets| offsets.as_slice()),
      (8, _four) => CB_NB_20
        .get(contour)
        .map_or(&[], |offsets| offsets.as_slice()),
      (_wider, 2) => CB_WB_10
        .get(contour)
        .map_or(&[], |offsets| offsets.as_slice()),
      (_wider, _four) => CB_WB_20
        .get(contour)
        .map_or(&[], |offsets| offsets.as_slice()),
    };
    let Ok(subframes) = index(self.subframes);

    for (slot, offset) in ctrl.pitch_l.iter_mut().zip(offsets.iter()).take(subframes) {
      *slot = lag.wrapping_add(*offset).clamp(min_lag, max_lag);
    }

    for (taps, chosen) in ctrl
      .ltp_coef_q14
      .as_chunks_mut::<5>()
      .0
      .iter_mut()
      .zip(self.indices.ltp_index.iter())
      .take(subframes)
    {
      let Ok(at) = index(*chosen);
      let codebook = match self.indices.per_index {
        0 => LTP_TAPS_0.get(at.min(7)),
        1 => LTP_TAPS_1.get(at.min(15)),
        _richest => LTP_TAPS_2.get(at.min(31)),
      };

      match codebook {
        Some(row) => {
          for (slot, tap) in taps.iter_mut().zip(row.iter()) {
            *slot = tap.wrapping_shl(7);
          }
        }
        None => {}
      }
    }

    ctrl.ltp_scale_q14 = match self.indices.ltp_scale_index {
      0 => 15565,
      1 => 12288,
      _smallest => 8192,
    };

    Ok(())
  }

  fn decode_parameters(&mut self, cond: Cond) -> Result<Ctrl, Never> {
    let Ok(gains_q16) = self.dequantise_gains(cond);
    let Ok(pred_coef_q12) = self.filters();
    let mut ctrl = Ctrl {
      gains_q16,
      pred_coef_q12,
      ..Ctrl::default()
    };

    match self.indices.signal_type.cmp(&VOICED) {
      Ordering::Equal => {
        let Ok(()) = self.pitch(&mut ctrl);
      }
      Ordering::Less | Ordering::Greater => {}
    }

    Ok(ctrl)
  }

  fn excitation(&self, pulses: &[i32; 336]) -> Result<[i32; 320], Never> {
    let offset_q10 = match (
      self.indices.signal_type.wrapping_shr(1),
      self.indices.quant_offset_type,
    ) {
      (0, 0) => 100i32,
      (0, _high) => 240,
      (_voiced, 0) => 32,
      (_voiced, _high) => 100,
    };
    let Ok(length) = index(self.frame_length);
    let mut excitation = [0i32; 320];
    let mut seed = self.indices.seed;

    for (slot, pulse) in excitation.iter_mut().zip(pulses.iter()).take(length) {
      seed = seed
        .wrapping_mul(RAND_MULTIPLIER)
        .wrapping_add(RAND_INCREMENT);

      let value = pulse.wrapping_shl(14);
      let value_2 = match (value.cmp(&0), value.cmp(&0)) {
        (Ordering::Greater, _) => value.wrapping_sub(1280),
        (Ordering::Less | Ordering::Equal, Ordering::Less) => value.wrapping_add(1280),
        (Ordering::Less | Ordering::Equal, Ordering::Equal | Ordering::Greater) => value,
      };
      let value_3 = value_2.wrapping_add(offset_q10.wrapping_shl(4));

      *slot = match seed.cmp(&0) {
        Ordering::Less => value_3.wrapping_neg(),
        Ordering::Equal | Ordering::Greater => value_3,
      };
      seed = seed.wrapping_add(*pulse);
    }

    Ok(excitation)
  }

  fn decode_core(
    &mut self,
    ctrl: &Ctrl,
    pulses: &[i32; 336],
    xq: &mut [i16],
  ) -> Result<(), Never> {
    let Ok(excitation) = self.excitation(pulses);
    let mut synthesis = Synthesis {
      s_ltp: [0; 320],
      s_ltp_q15: [0; 640],
      res_q14: [0; 80],
      s_lpc: [0; 96],
      ltp_buf_idx: self.ltp_memory,
    };

    for (slot, value) in synthesis.s_lpc.iter_mut().zip(self.s_lpc_q14.iter()) {
      *slot = *value;
    }

    for number in 0..self.subframes {
      let Ok(()) = self.subframe(ctrl, &excitation, &mut synthesis, xq, number);
    }

    for (slot, value) in self.s_lpc_q14.iter_mut().zip(synthesis.s_lpc.iter()) {
      *slot = *value;
    }

    Ok(())
  }

  fn subframe(
    &mut self,
    ctrl: &Ctrl,
    excitation: &[i32; 320],
    synthesis: &mut Synthesis,
    xq: &mut [i16],
    number: u32,
  ) -> Result<(), Never> {
    let Ok(at) = index(number);
    let Ok(half) = index(number.wrapping_shr(1));
    let filter = match ctrl.pred_coef_q12.get(half) {
      Some(filter) => *filter,
      None => [0; 16],
    };
    let mut taps = match ctrl.ltp_coef_q14.as_chunks::<5>().0.get(at) {
      Some(taps) => *taps,
      None => [0; 5],
    };
    let gain = match ctrl.gains_q16.get(at) {
      Some(gain) => *gain,
      None => 0,
    };
    let Ok(inverse_gain) = inverse32_varq(gain, 47);
    let adjustment = match gain.cmp(&self.prev_gain_q16) {
      Ordering::Equal => UNITY_Q16,
      Ordering::Less | Ordering::Greater => {
        let Ok(adjustment) = div32_varq(
          Ratio {
            numerator: self.prev_gain_q16,
            denominator: gain,
          },
          16,
        );

        for value in synthesis.s_lpc.iter_mut().take(16) {
          let Ok(scaled) = smulww(adjustment, *value);

          *value = scaled;
        }

        adjustment
      }
    };

    self.prev_gain_q16 = gain;

    let mut signal_type = self.indices.signal_type;
    let mut lag = match ctrl.pitch_l.get(at) {
      Some(lag) => *lag,
      None => 0,
    };

    match (
      self.loss_count.cmp(&0),
      self.prev_signal_type.cmp(&VOICED),
      signal_type.cmp(&VOICED),
      number.cmp(&2),
    ) {
      (
        Ordering::Greater,
        Ordering::Equal,
        Ordering::Less | Ordering::Greater,
        Ordering::Less,
      ) => {
        taps = LTP_UNIT;
        signal_type = VOICED;
        lag = self.lag_prev;
      }
      (_, _, _, _) => {}
    }

    let step = Subframe {
      number,
      lag,
      taps,
      inverse_gain,
      adjustment,
    };

    match signal_type.cmp(&VOICED) {
      Ordering::Equal => {
        let Ok(()) = self.long_term(ctrl, excitation, synthesis, xq, &step, &filter);
      }
      Ordering::Less | Ordering::Greater => {
        let Ok(from) = index(number.wrapping_mul(self.subframe_length));

        let Ok(length) = index(self.subframe_length);

        for (slot, value) in synthesis
          .res_q14
          .iter_mut()
          .zip(excitation.iter().skip(from))
          .take(length)
        {
          *slot = *value;
        }
      }
    }

    self.short_term(synthesis, xq, &step, &filter, gain.wrapping_shr(6))
  }

  fn long_term(
    &mut self,
    ctrl: &Ctrl,
    excitation: &[i32; 320],
    synthesis: &mut Synthesis,
    xq: &[i16],
    step: &Subframe,
    filter: &[i32; 16],
  ) -> Result<(), Never> {
    let interpolation = self.indices.nlsf_interp_q2.cmp(&4);
    let reach = step
      .lag
      .cast_unsigned()
      .wrapping_add(LTP_ORDER.wrapping_shr(1));

    match (step.number, interpolation) {
      (0, _) | (2, Ordering::Less) => {
        let start = self
          .ltp_memory
          .cast_signed()
          .wrapping_sub(step.lag)
          .wrapping_sub(self.order.cast_signed())
          .wrapping_sub(2)
          .max(0)
          .cast_unsigned();

        match step.number {
          2 => {
            let Ok(from) = index(self.ltp_memory);
            let Ok(length) = index(self.subframe_length.wrapping_mul(2));

            for (slot, value) in self
              .out_buf
              .iter_mut()
              .skip(from)
              .zip(xq.iter())
              .take(length)
            {
              *slot = *value;
            }
          }
          _first => {}
        }

        let Ok(begin) = index(start);
        let Ok(memory) = index(self.ltp_memory);
        let Ok(source) =
          index(start.wrapping_add(step.number.wrapping_mul(self.subframe_length)));
        let Ok(length) = index(self.ltp_memory.saturating_sub(start));
        let input = match self.out_buf.get(source..source.saturating_add(length)) {
          Some(input) => input,
          None => &[],
        };
        let output = match synthesis.s_ltp.get_mut(begin..memory) {
          Some(output) => output,
          None => &mut [],
        };
        let Ok(()) = lpc_analysis_filter(output, input, filter, self.order);
        let inverse_gain = match step.number {
          0 => {
            let Ok(scaled) = smulwb_low(
              crate::quantities::WideWord(step.inverse_gain),
              crate::quantities::LowWord(ctrl.ltp_scale_q14),
            );

            scaled.wrapping_shl(2)
          }
          _later => step.inverse_gain,
        };
        let Ok(filled) = index(synthesis.ltp_buf_idx);
        let Ok(count) = index(reach.min(self.ltp_memory));

        for (slot, value) in synthesis
          .s_ltp_q15
          .iter_mut()
          .take(filled)
          .rev()
          .zip(synthesis.s_ltp.iter().take(memory).rev())
          .take(count)
        {
          let Ok(scaled) = smulwb(inverse_gain, *value);

          *slot = scaled;
        }
      }
      (_, _) => match step.adjustment.cmp(&UNITY_Q16) {
        Ordering::Equal => {}
        Ordering::Less | Ordering::Greater => {
          let Ok(filled) = index(synthesis.ltp_buf_idx);
          let Ok(count) = index(reach.min(synthesis.ltp_buf_idx));

          for slot in synthesis
            .s_ltp_q15
            .iter_mut()
            .take(filled)
            .rev()
            .take(count)
          {
            let Ok(scaled) = smulww(step.adjustment, *slot);

            *slot = scaled;
          }
        }
      },
    }

    self.predict(excitation, synthesis, step)
  }

  fn predict(
    &self,
    excitation: &[i32; 320],
    synthesis: &mut Synthesis,
    step: &Subframe,
  ) -> Result<(), Never> {
    let base = i64::from(synthesis.ltp_buf_idx)
      .wrapping_sub(i64::from(step.lag))
      .wrapping_add(i64::from(LTP_ORDER.wrapping_shr(1)));
    let Ok(from) = index(step.number.wrapping_mul(self.subframe_length));
    let Ok(length) = index(self.subframe_length);

    for ((moment, residual), coded) in (0i64..)
      .zip(synthesis.res_q14.iter_mut())
      .zip(excitation.iter().skip(from))
      .take(length)
    {
      let centre = base.wrapping_add(moment);
      let mut prediction = 2i32;

      for (back, tap) in (0i64..).zip(step.taps.iter()) {
        let place = centre.wrapping_sub(back);
        let earlier = match place.cmp(&0) {
          Ordering::Less => 0,
          Ordering::Equal | Ordering::Greater => {
            let Ok(at) = fitted::<i64, u32>(place);
            let Ok(value) = slice_at(&synthesis.s_ltp_q15, at);

            value
          }
        };
        let Ok(term) = smulwb_low(
          crate::quantities::WideWord(earlier),
          crate::quantities::LowWord(*tap),
        );

        prediction = prediction.wrapping_add(term);
      }

      *residual = coded.wrapping_add(prediction.wrapping_shl(1));

      let Ok(()) = set_at(
        &mut synthesis.s_ltp_q15,
        synthesis.ltp_buf_idx,
        residual.wrapping_shl(1),
      );

      synthesis.ltp_buf_idx = synthesis.ltp_buf_idx.wrapping_add(1);
    }

    Ok(())
  }

  fn short_term(
    &self,
    synthesis: &mut Synthesis,
    xq: &mut [i16],
    step: &Subframe,
    filter: &[i32; 16],
    gain_q10: i32,
  ) -> Result<(), Never> {
    let Ok(length) = index(self.subframe_length);
    let Ok(order) = index(self.order);
    let Ok(from) = index(step.number.wrapping_mul(self.subframe_length));
    let cells = Cell::from_mut(synthesis.s_lpc.as_mut_slice()).as_slice_of_cells();

    for ((window, residual), out) in cells
      .windows(17)
      .zip(synthesis.res_q14.iter())
      .zip(xq.iter_mut().skip(from))
      .take(length)
    {
      match window.split_last() {
        Some((slot, history)) => {
          let mut prediction = self.order.wrapping_shr(1).cast_signed();

          for (earlier, coefficient) in history.iter().rev().zip(filter.iter()).take(order) {
            let Ok(term) = smulwb_low(
              crate::quantities::WideWord(earlier.get()),
              crate::quantities::LowWord(*coefficient),
            );

            prediction = prediction.wrapping_add(term);
          }

          let value = residual.saturating_add(prediction.saturating_mul(16));
          let Ok(scaled) = smulww(value, gain_q10);
          let Ok(rounded) = rshift_round(scaled, 8);
          let Ok(sample) = clipped(rounded);

          slot.set(value);
          *out = sample;
        }
        None => {}
      }
    }

    match length.checked_add(16) {
      Some(end) => match end.cmp(&synthesis.s_lpc.len()) {
        Ordering::Less | Ordering::Equal => synthesis.s_lpc.copy_within(length..end, 0),
        Ordering::Greater => {}
      },
      None => {}
    }

    Ok(())
  }

  fn conceal(&mut self, out: &mut [i16]) -> Result<(), Never> {
    match self.frame_length {
      0 => return Ok(()),
      _framed => {}
    }

    self.loss_count = self.loss_count.wrapping_add(1);

    let Ok(memory) = index(self.ltp_memory);
    let Ok(length) = index(self.frame_length);
    let Ok(losses) = fitted::<u32, i32>(self.loss_count);
    let fade = LossFade(PLC_FADE.powi(losses));
    let history: Vec<f32> = self
      .out_buf
      .iter()
      .take(memory)
      .map(|value| f32::from(*value))
      .collect();
    let pitch = self.plc_pitch.cast_unsigned();
    let Ok(pitch_at) = index(pitch);
    let Ok(made) = match (pitch, pitch_at.cmp(&memory)) {
      (1.., Ordering::Less) => repeated(
        &history,
        crate::quantities::PitchPeriod(pitch),
        fade,
        crate::quantities::FrameSize(self.frame_length),
      ),
      _noise => self.noise(&history, fade, self.frame_length),
    };

    for (slot, value) in out.iter_mut().zip(made.iter().skip(memory)).take(length) {
      let Ok(whole) = toward_zero_i32(f64::from(value.clamp(-32768.0, 32767.0)));
      let Ok(sample) = fitted::<i32, i16>(whole);

      *slot = sample;
    }

    let gain = i64::from(self.prev_gain_q16.max(1));

    for (slot, value) in self
      .s_lpc_q14
      .iter_mut()
      .zip(out.iter().take(length).skip(length.saturating_sub(16)))
    {
      let scaled = match i64::from(*value).wrapping_shl(30).checked_div(gain) {
        Some(scaled) => scaled,
        None => 0,
      };
      let Ok(fitted_value) = fitted::<i64, i32>(scaled);

      *slot = fitted_value;
    }

    self.lag_prev = self.plc_pitch.max(1);
    self.history = History::Continuing;

    Ok(())
  }

  fn noise(&mut self, history: &[f32], LossFade(fade): LossFade, length: u32) -> Result<Vec<f32>, Never> {
    let Ok(order) = index(self.order);
    let mut expanded = self.plc_lpc_q12;
    let Ok(()) = bwexpander(
      expanded.get_mut(..order).map_or(&mut [], |used| used),
      PLC_CHIRP,
      Width::Sixteen,
    );

    self.plc_lpc_q12 = expanded;

    let mut filter = Vec::new();

    for value in expanded.iter().take(order) {
      let Ok(coefficient) = float(*value);

      filter.push(coefficient / 4096.0);
    }

    let mut energy = 0f32;

    for window in history.windows(order.saturating_add(1)) {
      match window.split_last() {
        Some((current, earlier)) => {
          let mut residual = *current;

          for (coefficient, past) in filter.iter().zip(earlier.iter().rev()) {
            residual -= coefficient * past;
          }

          energy += residual * residual;
        }
        None => {}
      }
    }

    let Ok(span) = float(
      self
        .ltp_memory
        .saturating_sub(self.order)
        .max(1)
        .cast_signed(),
    );
    let level = (energy / span).sqrt() * fade;
    let mut made = history.to_vec();

    for _sample in 0..length {
      self.plc_rand = self
        .plc_rand
        .wrapping_mul(RAND_MULTIPLIER.cast_unsigned())
        .wrapping_add(RAND_INCREMENT.cast_unsigned());

      let Ok(random) = float(self.plc_rand.cast_signed().wrapping_shr(16));
      let mut value = random / FULL_SCALE * level * PLC_NOISE;

      for (coefficient, past) in filter.iter().zip(made.iter().rev()) {
        value += coefficient * past;
      }

      made.push(value.clamp(-32768.0, 32767.0));
    }

    Ok(made)
  }
}

fn repeated(
  history: &[f32],
  crate::quantities::PitchPeriod(pitch): crate::quantities::PitchPeriod,
  LossFade(fade): LossFade,
  crate::quantities::FrameSize(length): crate::quantities::FrameSize,
) -> Result<Vec<f32>, Never> {
  let mut made = history.to_vec();
  let Ok(period) = index(pitch);

  for _sample in 0..length {
    let value = match made.len().checked_sub(period).and_then(|at| made.get(at)) {
      Some(value) => *value * fade,
      None => 0.0,
    };

    made.push(value);
  }

  Ok(made)
}

pub(crate) struct SilkDecoder {
  output: Channels,
  channels: [Channel; 2],
  stereo: Stereo,
  internal: Channels,
  internal_before: Option<Channels>,
  internal_rate: u32,
  middle_before: Middle,
  prediction: [i32; 2],
  middle: Middle,
}

impl SilkDecoder {
  pub(crate) fn new(output: Channels) -> Result<SilkDecoder, Never> {
    let Ok(first) = Channel::new();
    let Ok(second) = Channel::new();

    Ok(SilkDecoder {
      output,
      channels: [first, second],
      stereo: Stereo::default(),
      internal: output,
      internal_before: None,
      internal_rate: 16_000,
      middle_before: Middle::Both,
      prediction: [0; 2],
      middle: Middle::Both,
    })
  }

  pub(crate) fn reset(&mut self) -> Result<(), Never> {
    let Ok(first) = Channel::new();
    let Ok(second) = Channel::new();

    self.channels = [first, second];
    self.stereo = Stereo::default();
    self.middle_before = Middle::Both;
    self.internal_before = None;

    Ok(())
  }

  pub(crate) fn set_stream(&mut self, internal: Channels, rate: u32) -> Result<(), Never> {
    self.internal = internal;
    self.internal_rate = rate;

    Ok(())
  }

  fn prepare(&mut self, start: Start, samples: u32) -> Result<MonoSide, Never> {
    match start {
      Start::NewPacket => {
        for channel in self.channels.iter_mut() {
          channel.frames_decoded = 0;
        }
      }
      Start::Continuing => {}
    }

    match (self.internal, self.internal_before) {
      (Channels::Stereo, Some(Channels::Mono)) => {
        let Ok(fresh) = Channel::new();
        let [_first, second] = &mut self.channels;

        *second = fresh;
      }
      (Channels::Stereo, Some(Channels::Stereo) | None) | (Channels::Mono, _) => {}
    }

    let [first, _second] = &self.channels;
    let side = match (
      self.internal,
      self.internal_before,
      self.internal_rate.cmp(&first.fs_khz.wrapping_mul(1000)),
    ) {
      (Channels::Mono, Some(Channels::Stereo), Ordering::Equal) => MonoSide::Resampled,
      (Channels::Mono, Some(Channels::Stereo), Ordering::Less | Ordering::Greater)
      | (Channels::Mono, Some(Channels::Mono) | None, _)
      | (Channels::Stereo, _, _) => MonoSide::Duplicated,
    };

    match first.frames_decoded {
      0 => {
        let (frames, subframes) = match samples.wrapping_div(48).max(10) {
          0..=10 => (1, 2),
          11..=20 => (1, 4),
          21..=40 => (2, 4),
          _sixty => (3, 4),
        };
        let fs_khz = self.internal_rate.wrapping_shr(10).wrapping_add(1);
        let Ok(count) = self.internal.count();
        let Ok(taken) = index(count);

        for channel in self.channels.iter_mut().take(taken) {
          channel.frames_per_packet = frames;
          channel.subframes = subframes;

          let Ok(()) = channel.set_fs(fs_khz);
        }
      }
      _later => {}
    }

    match (self.output, self.internal, self.internal_before) {
      (Channels::Stereo, Channels::Stereo, Some(Channels::Mono)) => {
        self.stereo.pred_prev_q13 = [0; 2];
        self.stereo.side = [0; 2];

        let [first_2, second] = &mut self.channels;

        second.resampler.clone_from(&first_2.resampler);
      }
      (Channels::Stereo, Channels::Stereo, Some(Channels::Stereo) | None)
      | (Channels::Stereo, Channels::Mono, _)
      | (Channels::Mono, _, _) => {}
    }

    self.internal_before = Some(self.internal);

    Ok(side)
  }

  fn read_flags(&mut self, dec: &mut RangeDecoder<'_>) -> Result<(), Never> {
    let Ok(count) = self.internal.count();
    let Ok(taken) = index(count);

    for channel in self.channels.iter_mut().take(taken) {
      let Ok(frames) = index(channel.frames_per_packet);

      for voice in channel.vad_flags.iter_mut().take(frames) {
        let Ok(bit) = dec.bit_logp(1);

        *voice = bit;
      }

      let Ok(bit) = dec.bit_logp(1);

      channel.lbrr_flag = bit;
    }

    for channel in self.channels.iter_mut().take(taken) {
      channel.lbrr_flags = [Bit::Zero; 3];

      match (channel.lbrr_flag, channel.frames_per_packet) {
        (Bit::One, 1) => channel.lbrr_flags = [Bit::One, Bit::Zero, Bit::Zero],
        (Bit::One, frames) => {
          let Ok(table) = row(&LBRR_FLAGS, frames.wrapping_sub(2));
          let Ok(symbol) = dec.symbol(table, 8);
          let symbol_2 = symbol.wrapping_add(1);
          let Ok(length) = index(frames);

          for (place, slot) in (0u32..).zip(channel.lbrr_flags.iter_mut()).take(length) {
            *slot = match symbol_2.wrapping_shr(place) & 1 {
              0 => Bit::Zero,
              _set => Bit::One,
            };
          }
        }
        (Bit::Zero, _) => {}
      }
    }

    self.skip_redundancy(dec)
  }

  fn skip_redundancy(&mut self, dec: &mut RangeDecoder<'_>) -> Result<(), Never> {
    let Ok(count) = self.internal.count();
    let Ok(taken) = index(count);
    let [first, _second] = &self.channels;
    let frames = first.frames_per_packet;

    for frame in 0..frames {
      let [_first, second] = &self.channels;
      let Ok(side_redundant) = flag(&second.lbrr_flags, frame);

      'inner_for_2: for (number, channel) in (0u32..).zip(self.channels.iter_mut()).take(taken) {
        let Ok(here) = flag(&channel.lbrr_flags, frame);

        match here {
          Bit::One => {}
          Bit::Zero => continue 'inner_for_2,
        }

        match (self.internal, number, side_redundant) {
          (Channels::Stereo, 0, Bit::Zero) => {
            let Ok(_skipped) = stereo_decode_pred(dec);
            let Ok(_middle) = dec.symbol(&MID_ONLY, 8);
          }
          (Channels::Stereo, 0, Bit::One) => {
            let Ok(_skipped) = stereo_decode_pred(dec);
          }
          (Channels::Stereo, _, _) | (Channels::Mono, _, _) => {}
        }

        let before = match frame.checked_sub(1) {
          Some(earlier) => {
            let Ok(bit) = flag(&channel.lbrr_flags, earlier);

            bit
          }
          None => Bit::Zero,
        };
        let cond = match before {
          Bit::One => Cond::Conditionally,
          Bit::Zero => Cond::Independently,
        };
        let Ok(()) = channel.decode_indices(dec, frame, Coding::Redundant, cond);
        let mut pulses = [0i32; 336];
        let Ok(()) = decode_pulses(dec, &mut pulses, &channel.indices, channel.frame_length);
      }
    }

    Ok(())
  }

  fn stereo_side(&mut self, dec: Option<&mut RangeDecoder<'_>>) -> Result<(), Never> {
    match (self.internal, dec) {
      (Channels::Stereo, Some(dec_2)) => {
        let [first, second] = &self.channels;
        let Ok(voice) = flag(&second.vad_flags, first.frames_decoded);
        let Ok(prediction) = stereo_decode_pred(dec_2);

        self.prediction = prediction;
        self.middle = match voice {
          Bit::Zero => {
            let Ok(symbol) = dec_2.symbol(&MID_ONLY, 8);

            match symbol {
              1 => Middle::MidOnly,
              _both => Middle::Both,
            }
          }
          Bit::One => Middle::Both,
        };
      }
      (Channels::Stereo, None) => self.prediction = self.stereo.pred_prev_q13,
      (Channels::Mono, _) => {}
    }

    match (self.internal, self.middle, self.middle_before) {
      (Channels::Stereo, Middle::Both, Middle::MidOnly) => {
        let [_first, second] = &mut self.channels;
        let Ok(()) = second.forget();
      }
      (Channels::Stereo, Middle::Both, Middle::Both)
      | (Channels::Stereo, Middle::MidOnly, _)
      | (Channels::Mono, _, _) => {}
    }

    Ok(())
  }

  fn frames(
    &mut self,
    dec: Option<&mut RangeDecoder<'_>>,
    buffers: &mut [[i16; 322]; 2],
  ) -> Result<(), Never> {
    let mut dec_2 = dec;
    let side = match &dec_2 {
      Some(_dec) => self.middle,
      None => self.middle_before,
    };
    let Ok(count) = self.internal.count();
    let Ok(taken) = index(count);
    let [first, _second] = &self.channels;
    let decoded = first.frames_decoded;

    for ((number, channel), buffer) in (0u32..)
      .zip(self.channels.iter_mut())
      .zip(buffers.iter_mut())
      .take(taken)
    {
      match (number, side) {
        (0, _) | (_, Middle::Both) => {
          let cond = match (decoded, number, self.middle_before) {
            (0, _, _) => Cond::Independently,
            (_, 1.., Middle::MidOnly) => Cond::IndependentlyNoLtpScaling,
            _relative => Cond::Conditionally,
          };
          let out = match buffer.get_mut(2..) {
            Some(out) => out,
            None => &mut [],
          };
          let Ok(()) = channel.decode_frame(dec_2.as_deref_mut(), out, cond);
        }
        (_side, Middle::MidOnly) => {}
      }

      channel.frames_decoded = channel.frames_decoded.wrapping_add(1);
    }

    Ok(())
  }

  fn emit(
    &mut self,
    buffers: &mut [[i16; 322]; 2],
    out: &mut [f32],
    side: MonoSide,
  ) -> Result<u32, Never> {
    let [first, _second] = &self.channels;
    let shape = Shape {
      fs_khz: first.fs_khz,
      length: first.frame_length,
    };
    let Ok(length) = index(shape.length);
    let [mid, other] = &mut *buffers;

    match (self.output, self.internal) {
      (Channels::Stereo, Channels::Stereo) => {
        let Ok(()) = self.stereo.unmix(mid, other, self.prediction, shape);
      }
      (Channels::Stereo, Channels::Mono) | (Channels::Mono, _) => {
        let Ok(()) = copied16(mid, &self.stereo.mid);
        let Ok(()) = copied16(
          &mut self.stereo.mid,
          mid.get(length..).map_or(&[], |rest| rest),
        );
      }
    }

    let made = match NonZeroU32::new(shape.fs_khz.wrapping_mul(1000)) {
      Some(rate) => shape.length.wrapping_mul(OUTPUT_RATE) / rate,
      None => 0,
    };
    let Ok(made_at) = index(made);
    let Ok(width) = self.output.count();
    let Ok(internal) = self.internal.count();
    let Ok(lanes) = index(width.min(internal));
    let mut resampled = Vec::with_capacity(made_at.min(5760));

    for ((lane, channel), buffer) in (0u32..)
      .zip(self.channels.iter_mut())
      .zip(buffers.iter())
      .take(lanes)
    {
      resampled.clear();

      let Ok(()) = resample(channel, buffer, shape.length, &mut resampled);
      let Ok(()) = interleave(
        out,
        &resampled,
        Lane {
          lane,
          width,
          length: made,
        },
      );
    }

    match (self.output, self.internal, side) {
      (Channels::Stereo, Channels::Mono, MonoSide::Resampled) => {
        resampled.clear();

        let [_first, second] = &mut self.channels;
        let [mid_2, _other] = &*buffers;
        let Ok(()) = resample(second, mid_2, shape.length, &mut resampled);
        let Ok(()) = interleave(
          out,
          &resampled,
          Lane {
            lane: 1,
            width: 2,
            length: made,
          },
        );
      }
      (Channels::Stereo, Channels::Mono, MonoSide::Duplicated) => {
        for [left, right] in out.as_chunks_mut::<2>().0.iter_mut().take(made_at) {
          *right = *left;
        }
      }
      (Channels::Stereo, Channels::Stereo, _) | (Channels::Mono, _, _) => {}
    }

    Ok(made)
  }

  pub(crate) fn decode(
    &mut self,
    dec: Option<&mut RangeDecoder<'_>>,
    start: Start,
    samples: u32,
    out: &mut [f32],
  ) -> Result<u32, Never> {
    let mut dec_2 = dec;
    let Ok(side) = self.prepare(start, samples);
    let [first, _second] = &self.channels;

    match (dec_2.as_deref_mut(), first.frames_decoded) {
      (Some(dec_3), 0) => {
        let Ok(()) = self.read_flags(dec_3);
      }
      (Some(_), _) | (None, _) => {}
    }

    let Ok(()) = self.stereo_side(dec_2.as_deref_mut());
    let mut buffers = [[0i16; 322]; 2];
    let reception = match &dec_2 {
      Some(_) => Reception::Received,
      None => Reception::Lost,
    };
    let Ok(()) = self.frames(dec_2, &mut buffers);
    let Ok(made) = self.emit(&mut buffers, out, side);
    let Ok(count) = self.internal.count();
    let Ok(taken) = index(count);

    match reception {
      Reception::Lost => {
        for channel in self.channels.iter_mut().take(taken) {
          channel.last_gain_index = RESET_GAIN_INDEX;
        }
      }
      Reception::Received => self.middle_before = self.middle,
    }

    Ok(made)
  }
}

struct Lane {
  lane: u32,
  width: u32,
  length: u32,
}

fn resample(
  channel: &mut Channel,
  buffer: &[i16; 322],
  length: u32,
  into: &mut Vec<f32>,
) -> Result<(), Never> {
  let Ok(end) = index(length.saturating_add(1));
  let input = match buffer.get(1..end) {
    Some(input) => input,
    None => &[],
  };

  match &mut channel.resampler {
    Some(resampler) => resampler.process(input, into),
    None => Ok(()),
  }
}

fn interleave(out: &mut [f32], samples: &[f32], lane: Lane) -> Result<(), Never> {
  let Ok(width) = index(lane.width);
  let Ok(at) = index(lane.lane);
  let Ok(length) = index(lane.length);

  for (frame, value) in out
    .chunks_exact_mut(width.max(1))
    .zip(samples.iter())
    .take(length)
  {
    match frame.get_mut(at) {
      Some(slot) => *slot = value / FULL_SCALE,
      None => {}
    }
  }

  Ok(())
}
