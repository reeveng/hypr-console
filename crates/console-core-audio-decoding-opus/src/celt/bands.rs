//! Band shape decoding: PVQ, spreading, TF changes, band splitting, stereo, folding
//! (RFC 6716 sections 4.3.4 to 4.3.6).
//!
//! A band too large for one codeword is split in half and each half coded on its own, and a
//! half can be split again while the band's resolution allows: the resolution starts at the
//! frame's and drops by one at each split, stopping at the half-block resolution, so a band is cut at most
//! four times and is a tree of at most thirty-one parts. The reference decoder walks that tree
//! by recursion. Here it is walked with a stack of the halves still owed: a part that splits
//! reads its angle and goes on the stack while its first half is coded, the second half's
//! budget is settled from what the first half spent, and the two halves' collapse masks are
//! joined when both are done. The decoder reads exactly what the recursion read, in the
//! order it read it.
//!
//! The arithmetic is the section's. The PVQ index, the angle and the band budgets are
//! integers of the widths the RFC gives them, and the bit-exact cosine and log-tangent are
//! Q15 fixed point that the RFC truncates to sixteen bits on every product; the truncation is
//! said once, where a value is taken to sixteen bits. The shapes themselves are floats, in the
//! order the reference decoder sums them.

use std::cmp::Ordering;

use std::cell::Cell;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index, narrow_f32};

use super::Layout;
use super::rate::{Mode, QTHETA_OFFSET, QTHETA_OFFSET_TWOPHASE, edge, get_pulses, of, width};
use super::tables::{BANDS, E_MEANS, ORDERY, SHORT_MDCT};
use crate::quantities::{BandGain, MidGain, SideGain};
use crate::range::{BITRES, Bit, RangeDecoder, Span};

const ONE_BIT: i32 = 1 << BITRES;
const PARTITION_STEPS: u32 = 64;
const BIT_INTERLEAVE: [u32; 16] = [0, 1, 1, 1, 2, 3, 3, 3, 2, 3, 3, 3, 2, 3, 3, 3];
const BIT_DEINTERLEAVE: [u32; 16] = [
  0x00, 0x03, 0x0C, 0x0F, 0x30, 0x33, 0x3C, 0x3F, 0xC0, 0xC3, 0xCC, 0xCF, 0xF0, 0xF3, 0xFC, 0xFF,
];
const EXP2_TABLE8: [i32; 8] = [16384, 17866, 19483, 21247, 23170, 25267, 27554, 30048];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Spread {
  None,
  Light,
  Normal,
  Aggressive,
}

impl Spread {
  pub(crate) fn from_symbol(symbol: u32) -> Result<Spread, Never> {
    Ok(match symbol {
      0 => Spread::None,
      1 => Spread::Light,
      2 => Spread::Normal,
      _aggressive => Spread::Aggressive,
    })
  }

  fn factor(self) -> Result<Option<u32>, Never> {
    Ok(match self {
      Spread::None => None,
      Spread::Light => Some(15),
      Spread::Normal => Some(10),
      Spread::Aggressive => Some(5),
    })
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Inversion {
  Allowed,
  Disabled,
}

#[derive(Clone, Copy)]
enum Coupling {
  Mono,
  Stereo,
}

pub(crate) fn lcg_rand(seed: u32) -> Result<u32, Never> {
  Ok(seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223))
}

fn noise(seed: u32) -> Result<f32, Never> {
  let Ok(value) = narrow_f32(f64::from(seed.cast_signed().wrapping_shr(20)));

  Ok(value)
}

fn float(value: u32) -> Result<f32, Never> {
  narrow_f32(f64::from(value))
}

fn pvq_rows(
  v: &mut Vec<u64>,
  crate::quantities::VectorSize(size): crate::quantities::VectorSize,
  crate::quantities::PulseCount(pulses): crate::quantities::PulseCount,
) -> Result<u32, Never> {
  let row = pulses.wrapping_add(1);
  let Ok(wide) = index(row);
  let Ok(cells) = index(size.wrapping_add(1).wrapping_mul(row));

  v.clear();
  v.resize(cells.min(const { (176 + 1) * (128 + 1) }), 0);

  match v.first_mut() {
    Some(first) => *first = 1,
    None => {}
  }

  for dimension in 1..=size {
    let Ok(split) = index(dimension.wrapping_mul(row));
    let (before, after) = match v.split_at_mut_checked(split) {
      Some(rows) => rows,
      None => break,
    };
    let previous = before.rchunks_exact(wide.max(1)).next();
    let current = after.chunks_exact_mut(wide.max(1)).next();

    match (previous, current) {
      (Some(previous_2), Some(current_2)) => {
        let mut left = 1u64;

        match current_2.first_mut() {
          Some(first) => *first = 1,
          None => {}
        }

        for ((slot, up), diagonal) in current_2
          .iter_mut()
          .skip(1)
          .zip(previous_2.iter().skip(1))
          .zip(previous_2.iter())
        {
          *slot = up.saturating_add(left).saturating_add(*diagonal);
          left = *slot;
        }
      }
      (None, _) | (_, None) => {}
    }
  }

  Ok(row)
}

fn pvq_at(v: &[u64], row: u32, place: (u32, u32)) -> Result<u64, Never> {
  let (dimension, pulses) = place;
  let Ok(at) = index(dimension.wrapping_mul(row).wrapping_add(pulses));

  Ok(match v.get(at) {
    Some(count) => *count,
    None => 0,
  })
}

enum PulseSign {
  Positive,
  Negative,
}

fn decode_pulses(
  y: &mut [i32],
  pulses: u32,
  dec: &mut RangeDecoder<'_>,
  v: &mut Vec<u64>,
) -> Result<f32, Never> {
  let Ok(size) = fitted::<_, u32>(y.len());
  let Ok(row) = pvq_rows(
    v,
    crate::quantities::VectorSize(size),
    crate::quantities::PulseCount(pulses),
  );
  let Ok(total) = pvq_at(v, row, (size, pulses));
  let Ok(range) = fitted::<u64, u32>(total);
  let mut left = match range.cmp(&2) {
    Ordering::Equal | Ordering::Greater => {
      let Ok(chosen) = dec.uint(range);

      u64::from(chosen)
    }
    Ordering::Less => 0,
  };
  let mut remaining = pulses;
  let mut energy = 0f32;

  for (place, slot) in (0u32..).zip(y.iter_mut()) {
    let rest = size.wrapping_sub(place);

    match remaining {
      0 => {
        *slot = 0;

        continue;
      }
      _some => {}
    }

    let Ok(below) = pvq_at(v, row, (rest.wrapping_sub(1), remaining));
    let Ok(here) = pvq_at(v, row, (rest, remaining));
    let mut bound = below.wrapping_add(here).wrapping_shr(1);
    let sign = match left.cmp(&bound) {
      Ordering::Equal | Ordering::Greater => {
        left = left.wrapping_sub(bound);

        PulseSign::Negative
      }
      Ordering::Less => PulseSign::Positive,
    };

    let before = remaining;

    bound = bound.wrapping_sub(below);

    'pulse_magnitude: for _pulse in 0..before {
      match (bound.cmp(&left), remaining) {
        (Ordering::Greater, 1..) => {
          remaining = remaining.wrapping_sub(1);

          let Ok(next) = pvq_at(v, row, (rest.wrapping_sub(1), remaining));

          bound = bound.wrapping_sub(next);
        }
        _finished => break 'pulse_magnitude,
      }
    }

    let magnitude = before.wrapping_sub(remaining).cast_signed();

    *slot = match sign {
      PulseSign::Negative => magnitude.wrapping_neg(),
      PulseSign::Positive => magnitude,
    };
    left = left.wrapping_sub(bound.min(left));

    let Ok(squared) = narrow_f32(f64::from(magnitude.wrapping_mul(magnitude)));

    energy += squared;
  }

  Ok(energy)
}

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "sine and cosine are direction ratios for the spreading rotation"
  )
)]
struct Rotation {
  stride: u32,
  cosine: f32,
  sine: f32,
}

fn exp_rotation1(x: &mut [f32], turn: Rotation) -> Result<(), Never> {
  let opposite = -turn.sine;
  let cells = Cell::from_mut(x).as_slice_of_cells();
  let Ok(length) = fitted::<_, u32>(cells.len());
  let Ok(stride) = index(turn.stride);

  match length.cmp(&turn.stride) {
    Ordering::Greater => {
      for (first, second) in cells.iter().zip(cells.iter().skip(stride)) {
        let (x1, x2) = (first.get(), second.get());

        second.set(turn.cosine * x2 + turn.sine * x1);
        first.set(turn.cosine * x1 + opposite * x2);
      }

      let Ok(back) = index(length.saturating_sub(turn.stride.saturating_mul(2)));

      for (first, second) in cells.iter().zip(cells.iter().skip(stride)).take(back).rev() {
        let (x1, x2) = (first.get(), second.get());

        second.set(turn.cosine * x2 + turn.sine * x1);
        first.set(turn.cosine * x1 + opposite * x2);
      }
    }
    Ordering::Less | Ordering::Equal => {}
  }

  Ok(())
}

#[derive(Clone, Copy)]
struct Spreading {
  blocks: u32,
  pulses: u32,
  spread: Spread,
}

fn exp_rotation(x: &mut [f32], spreading: Spreading) -> Result<(), Never> {
  let Ok(length) = fitted::<_, u32>(x.len());

  let factor = match (
    spreading.pulses.wrapping_mul(2).cmp(&length),
    spreading.spread.factor(),
  ) {
    (Ordering::Equal | Ordering::Greater, _) | (Ordering::Less, Ok(None)) => return Ok(()),
    (Ordering::Less, Ok(Some(factor))) => factor,
  };
  let Ok(whole) = float(length);
  let Ok(spread_out) = float(length.wrapping_add(factor.wrapping_mul(spreading.pulses)));
  let gain = whole / spread_out;
  let theta = 0.5 * gain * gain;
  let cosine = (0.5 * std::f32::consts::PI * theta).cos();
  let sine = (0.5 * std::f32::consts::PI * (1.0 - theta)).cos();
  let stride = spreading.blocks;
  let mut wide = 0u32;

  match length.cmp(&stride.wrapping_mul(8)) {
    Ordering::Equal | Ordering::Greater => {
      wide = 1;

      for _step in 0..length {
        match wide
          .wrapping_mul(wide)
          .wrapping_add(wide)
          .wrapping_mul(stride)
          .wrapping_add(stride.wrapping_shr(2))
          .cmp(&length)
        {
          Ordering::Less => wide = wide.wrapping_add(1),
          Ordering::Equal | Ordering::Greater => break,
        }
      }
    }
    Ordering::Less => {}
  }

  let sub = match length.checked_div(stride) {
    Some(sub) => sub,
    None => 0,
  };
  let Ok(each) = index(sub);
  let apart = wide;
  let Ok(count) = index(stride);

  for block in x.chunks_exact_mut(each.max(1)).take(count) {
    match apart {
      0 => {}
      _some => {
        let Ok(()) = exp_rotation1(
          block,
          Rotation {
            stride: apart,
            cosine: sine,
            sine: cosine,
          },
        );
      }
    }

    let Ok(()) = exp_rotation1(
      block,
      Rotation {
        stride: 1,
        cosine,
        sine,
      },
    );
  }

  Ok(())
}

fn extract_collapse_mask(iy: &[i32], blocks: u32) -> Result<u32, Never> {
  match blocks.cmp(&1) {
    Ordering::Less | Ordering::Equal => Ok(1),
    Ordering::Greater => {
      let Ok(size) = fitted::<_, u32>(iy.len());
      let each = match size.checked_div(blocks) {
        Some(each) => each,
        None => 0,
      };
      let Ok(apart) = index(each);
      let Ok(count) = index(blocks);
      let mut mask = 0u32;

      for (block, values) in (0u32..).zip(iy.chunks_exact(apart.max(1)).take(count)) {
        match values.iter().find(|value| **value != 0) {
          Some(_) => mask |= 1u32.wrapping_shl(block),
          None => {}
        }
      }

      Ok(mask)
    }
  }
}

pub(crate) fn renormalise_vector(x: &mut [f32], BandGain(gain): BandGain) -> Result<(), Never> {
  let energy: f32 = 1e-15 + x.iter().map(|value| value * value).sum::<f32>();
  let scale = gain / energy.sqrt();

  for value in x.iter_mut() {
    *value *= scale;
  }

  Ok(())
}

fn haar1(
  x: &mut [f32],
  crate::quantities::VectorSize(size): crate::quantities::VectorSize,
  crate::quantities::Stride(stride): crate::quantities::Stride,
) -> Result<(), Never> {
  const S: f32 = std::f32::consts::FRAC_1_SQRT_2;
  let Ok(pairs) = index(size.wrapping_shr(1));
  let Ok(across) = index(stride);

  for pair in x
    .chunks_exact_mut(across.saturating_mul(2).max(1))
    .take(pairs)
  {
    let (even, odd) = pair.split_at_mut(across.min(pair.len()));

    for (first, second) in even.iter_mut().zip(odd.iter_mut()) {
      let a = *first * S;
      let b = *second * S;

      *first = a + b;
      *second = a - b;
    }
  }

  Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Order {
  Hadamard,
  Plain,
}

#[derive(Clone, Copy)]
struct Interleaving {
  size: u32,
  stride: u32,
  order: Order,
}

fn row_of(shape: Interleaving, block: u32) -> Result<u32, Never> {
  match shape.order {
    Order::Plain => Ok(block),
    Order::Hadamard => {
      let Ok(at) = index(shape.stride.wrapping_sub(2).wrapping_add(block));

      Ok(match ORDERY.get(at) {
        Some(row) => *row,
        None => block,
      })
    }
  }
}

#[derive(Clone, Copy)]
enum Direction {
  Deinterleave,
  Interleave,
}

fn reorder_hadamard(
  x: &mut [f32],
  shape: Interleaving,
  tmp: &mut Vec<f32>,
  direction: Direction,
) -> Result<(), Never> {
  let Ok(length) = index(shape.size.wrapping_mul(shape.stride));

  tmp.clear();
  tmp.resize(length.min(x.len()), 0.0);

  for block in 0..shape.stride {
    let Ok(row) = row_of(shape, block);

    for step in 0..shape.size {
      let spread = block.wrapping_add(step.wrapping_mul(shape.stride));
      let packed = row.wrapping_mul(shape.size).wrapping_add(step);
      let (written, read) = match direction {
        Direction::Deinterleave => (packed, spread),
        Direction::Interleave => (spread, packed),
      };
      let Ok(to) = index(written);
      let Ok(from) = index(read);

      match (tmp.get_mut(to), x.get(from)) {
        (Some(slot), Some(value)) => *slot = *value,
        (Some(_) | None, _) => {}
      }
    }
  }

  for (slot, value) in x.iter_mut().zip(tmp.iter()) {
    *slot = *value;
  }

  Ok(())
}

fn low16(value: i32) -> Result<i32, Never> {
  Ok(value.wrapping_shl(16).wrapping_shr(16))
}

#[cfg_attr(
  dylint_lib = "explicit024_no_swappable_params",
  allow(
    explicit024_no_swappable_params,
    reason = "a product is commutative, so its two factors taken in either order are the same call"
  )
)]
fn frac_mul16(a: i32, b: i32) -> Result<i32, Never> {
  let Ok(a_2) = low16(a);
  let Ok(b_2) = low16(b);

  Ok(a_2.wrapping_mul(b_2).wrapping_add(16384).wrapping_shr(15))
}

fn bitexact_cos(x: i32) -> Result<i32, Never> {
  let squared = x.wrapping_mul(x).wrapping_add(4096).wrapping_shr(13);
  let Ok(inner) = frac_mul16(-626, squared);
  let Ok(middle) = frac_mul16(squared, inner.wrapping_add(8277));
  let Ok(outer) = frac_mul16(squared, middle.wrapping_add(-7651));

  Ok(
    32767i32
      .wrapping_sub(squared)
      .wrapping_add(outer)
      .wrapping_add(1),
  )
}

#[derive(Clone, Copy)]
struct Angle {
  sine: i32,
  cosine: i32,
}

fn bitexact_log2tan(angle: Angle) -> Result<i32, Never> {
  let Ok(cosine_bits) = crate::range::significant_bits(angle.cosine.cast_unsigned());
  let Ok(sine_bits) = crate::range::significant_bits(angle.sine.cast_unsigned());
  let cosine = angle.cosine.wrapping_shl(15u32.wrapping_sub(cosine_bits));
  let sine = angle.sine.wrapping_shl(15u32.wrapping_sub(sine_bits));
  let Ok(sine_inner) = frac_mul16(sine, -2597);
  let Ok(sine_term) = frac_mul16(sine, sine_inner.wrapping_add(7932));
  let Ok(cosine_inner) = frac_mul16(cosine, -2597);
  let Ok(cosine_term) = frac_mul16(cosine, cosine_inner.wrapping_add(7932));
  let whole = sine_bits
    .cast_signed()
    .wrapping_sub(cosine_bits.cast_signed())
    .wrapping_mul(1i32.wrapping_shl(11));

  Ok(whole.wrapping_add(sine_term).wrapping_sub(cosine_term))
}

fn halved(value: i32) -> Result<i32, Never> {
  Ok(match value.checked_div(2) {
    Some(half) => half,
    None => 0,
  })
}

#[derive(Clone, Copy)]
struct Quantizer {
  size: i32,
  bits: i32,
  offset: i32,
  pulse_cap: i32,
  coupling: Coupling,
}

fn compute_qn(asked: Quantizer) -> Result<i32, Never> {
  let degrees = match (asked.coupling, asked.size) {
    (Coupling::Stereo, 2) => asked.size.wrapping_mul(2).wrapping_sub(2),
    (Coupling::Stereo | Coupling::Mono, _) => asked.size.wrapping_mul(2).wrapping_sub(1),
  };
  let share = match asked
    .bits
    .wrapping_add(degrees.wrapping_mul(asked.offset))
    .checked_div(degrees)
  {
    Some(share) => share,
    None => 0,
  };
  let share_2 = share
    .min(
      asked
        .bits
        .wrapping_sub(asked.pulse_cap)
        .wrapping_sub(4i32.wrapping_shl(BITRES)),
    )
    .min(8i32.wrapping_shl(BITRES));

  match share_2.cmp(&ONE_BIT.wrapping_shr(1)) {
    Ordering::Less => Ok(1),
    Ordering::Equal | Ordering::Greater => {
      let Ok(at) = index(share_2 & 0x7);
      let step = match EXP2_TABLE8.get(at) {
        Some(step) => *step,
        None => 0,
      };
      let levels = step.wrapping_shr(
        14i32
          .wrapping_sub(share_2.wrapping_shr(BITRES))
          .cast_unsigned(),
      );

      Ok(levels.wrapping_add(1).wrapping_shr(1).wrapping_shl(1))
    }
  }
}

pub(crate) struct BandCtx<'m, 'd, 'b> {
  pub(crate) m: &'m Mode,
  pub(crate) dec: &'d mut RangeDecoder<'b>,
  pub(crate) band: u32,
  pub(crate) intensity: u32,
  pub(crate) spread: Spread,
  pub(crate) tf_change: i32,
  pub(crate) remaining_bits: i32,
  pub(crate) seed: u32,
  pub(crate) inversion: Inversion,
  pub(crate) pvq_v: Vec<u64>,
  pub(crate) iy: Vec<i32>,
  pub(crate) tmp: Vec<f32>,
}

#[derive(Clone, Copy)]
struct Split {
  inv: Bit,
  imid: i32,
  iside: i32,
  delta: i32,
  itheta: i32,
  qalloc: i32,
}

impl Split {
  fn gains(self) -> Result<(MidGain, SideGain), Never> {
    let Ok(mid) = narrow_f32(f64::from(self.imid));
    let Ok(side) = narrow_f32(f64::from(self.iside));

    Ok((MidGain(mid / 32768.0), SideGain(side / 32768.0)))
  }
}

#[derive(Clone, Copy)]
struct Theta {
  size: i32,
  blocks: u32,
  original_blocks: u32,
  lm: i32,
  coupling: Coupling,
}

#[derive(Clone, Copy)]
struct Budget {
  bits: i32,
  fill: u32,
}

#[derive(Clone, Copy)]
enum Resolution {
  HalfBlock,
  WholeBlocks(u32),
}

impl Resolution {
  fn exponent(self) -> Result<i32, Never> {
    Ok(match self {
      Self::HalfBlock => -1,
      Self::WholeBlocks(exponent) => exponent.cast_signed(),
    })
  }

  fn halved(self) -> Result<Self, Never> {
    Ok(match self {
      Self::HalfBlock | Self::WholeBlocks(0) => Self::HalfBlock,
      Self::WholeBlocks(exponent) => Self::WholeBlocks(exponent.wrapping_sub(1)),
    })
  }
}

#[derive(Clone, Copy)]
struct Part {
  offset: u32,
  size: u32,
  bits: i32,
  blocks: u32,
  lowband: Option<u32>,
  resolution: Resolution,
  gain: BandGain,
  fill: u32,
}

#[derive(Clone, Copy)]
enum First {
  Mid,
  Side,
}

#[derive(Clone, Copy)]
enum Turn {
  First,
  Second,
}

#[derive(Clone, Copy)]
struct Fork {
  mid: Part,
  side: Part,
  first: First,
  turn: Turn,
  shift: u32,
  itheta: i32,
  rebalance: i32,
  mask: u32,
}

enum Visit {
  Forked(Fork, Part),
  Leaf,
}

struct Partition<'x, 'l> {
  x: &'x mut [f32],
  lowband: Option<&'l [f32]>,
}

impl BandCtx<'_, '_, '_> {
  fn stereo_angle(&mut self, levels: u32) -> Result<u32, Never> {
    let weight = 3u32;
    let middle = levels.wrapping_shr(1);
    let total = weight
      .wrapping_mul(middle.wrapping_add(1))
      .wrapping_add(middle);
    let Ok(found) = self.dec.decode(total);
    let ramp = middle.wrapping_add(1).wrapping_mul(weight);
    let chosen = match found.cmp(&ramp) {
      Ordering::Less => match found.checked_div(weight) {
        Some(step) => step,
        None => 0,
      },
      Ordering::Equal | Ordering::Greater => middle
        .wrapping_add(1)
        .wrapping_add(found.wrapping_sub(ramp)),
    };
    let span = match chosen.cmp(&middle) {
      Ordering::Less | Ordering::Equal => Span {
        low: weight.wrapping_mul(chosen),
        high: weight.wrapping_mul(chosen.wrapping_add(1)),
      },
      Ordering::Greater => Span {
        low: chosen
          .wrapping_sub(1)
          .wrapping_sub(middle)
          .wrapping_add(ramp),
        high: chosen.wrapping_sub(middle).wrapping_add(ramp),
      },
    };
    let Ok(()) = self.dec.update(span, total);

    Ok(chosen)
  }

  fn triangular_angle(&mut self, levels: u32) -> Result<u32, Never> {
    let half = levels.wrapping_shr(1);
    let total = half.wrapping_add(1).wrapping_mul(half.wrapping_add(1));
    let Ok(found) = self.dec.decode(total);

    let (chosen, low, share) =
      match found.cmp(&half.wrapping_mul(half.wrapping_add(1)).wrapping_shr(1)) {
        Ordering::Less => {
          let chosen = found
            .wrapping_mul(8)
            .wrapping_add(1)
            .isqrt()
            .wrapping_sub(1)
            .wrapping_shr(1);

          (
            chosen,
            chosen.wrapping_mul(chosen.wrapping_add(1)).wrapping_shr(1),
            chosen.wrapping_add(1),
          )
        }
        Ordering::Equal | Ordering::Greater => {
          let root = total
            .wrapping_sub(found)
            .wrapping_sub(1)
            .wrapping_mul(8)
            .wrapping_add(1)
            .isqrt();
          let chosen = levels
            .wrapping_add(1)
            .wrapping_mul(2)
            .wrapping_sub(root)
            .wrapping_shr(1);
          let share = levels.wrapping_add(1).wrapping_sub(chosen);
          let low = total.wrapping_sub(
            share
              .wrapping_mul(levels.wrapping_add(2).wrapping_sub(chosen))
              .wrapping_shr(1),
          );

          (chosen, low, share)
        }
      };
    let Ok(()) = self.dec.update(
      Span {
        low,
        high: low.wrapping_add(share),
      },
      total,
    );

    Ok(chosen)
  }

  fn coded_angle(&mut self, levels: i32, shape: Theta) -> Result<i32, Never> {
    let levels_2 = levels.cast_unsigned();
    let chosen = match (
      shape.coupling,
      shape.size.cmp(&2),
      shape.original_blocks.cmp(&1),
    ) {
      (Coupling::Stereo, Ordering::Greater, _) => {
        let Ok(chosen) = self.stereo_angle(levels_2);

        chosen
      }
      (Coupling::Stereo, Ordering::Less | Ordering::Equal, _)
      | (Coupling::Mono, _, Ordering::Greater) => {
        let Ok(chosen) = self.dec.uint(levels_2.wrapping_add(1));

        chosen
      }
      (Coupling::Mono, _, Ordering::Less | Ordering::Equal) => {
        let Ok(chosen) = self.triangular_angle(levels_2);

        chosen
      }
    };
    let scaled = match chosen.wrapping_mul(16384).checked_div(levels_2) {
      Some(scaled) => scaled,
      None => 0,
    };

    Ok(scaled.cast_signed())
  }

  fn inversion(&mut self, bits: i32) -> Result<Bit, Never> {
    let read = match bits
      .min(self.remaining_bits)
      .cmp(&2i32.wrapping_shl(BITRES))
    {
      Ordering::Greater => {
        let Ok(bit) = self.dec.bit_logp(2);

        bit
      }
      Ordering::Less | Ordering::Equal => Bit::Zero,
    };

    Ok(match self.inversion {
      Inversion::Disabled => Bit::Zero,
      Inversion::Allowed => read,
    })
  }

  fn compute_theta(&mut self, shape: Theta, budget: &mut Budget) -> Result<Split, Never> {
    let Ok(log_n) = of(&self.m.log_n, self.band);
    let pulse_cap = log_n.wrapping_add(shape.lm.wrapping_mul(ONE_BIT));
    let offset = pulse_cap
      .wrapping_shr(1)
      .wrapping_sub(match (shape.coupling, shape.size) {
        (Coupling::Stereo, 2) => QTHETA_OFFSET_TWOPHASE,
        (Coupling::Stereo | Coupling::Mono, _) => QTHETA_OFFSET,
      });
    let Ok(levels) = compute_qn(Quantizer {
      size: shape.size,
      bits: budget.bits,
      offset,
      pulse_cap,
      coupling: shape.coupling,
    });
    let levels_2 = match (shape.coupling, self.band.cmp(&self.intensity)) {
      (Coupling::Stereo, Ordering::Equal | Ordering::Greater) => 1,
      (Coupling::Stereo | Coupling::Mono, _) => levels,
    };
    let Ok(told) = self.dec.tell_frac();

    let (itheta, inv) = match (levels_2, shape.coupling) {
      (1, Coupling::Stereo) => {
        let Ok(inv) = self.inversion(budget.bits);

        (0, inv)
      }
      (1, Coupling::Mono) => (0, Bit::Zero),
      (_, _) => {
        let Ok(itheta) = self.coded_angle(levels_2, shape);

        (itheta, Bit::Zero)
      }
    };
    let Ok(spent) = self.dec.tell_frac();
    let qalloc = spent.wrapping_sub(told).cast_signed();

    budget.bits = budget.bits.wrapping_sub(qalloc);

    let mask = 1u32.wrapping_shl(shape.blocks).wrapping_sub(1);

    let (imid, iside, delta) = match itheta {
      0 => {
        budget.fill &= mask;

        (32767, 0, -16384)
      }
      16384 => {
        budget.fill &= mask.wrapping_shl(shape.blocks);

        (0, 32767, 16384)
      }
      _between => {
        let Ok(imid) = bitexact_cos(itheta);
        let Ok(iside) = bitexact_cos(16384i32.wrapping_sub(itheta));
        let Ok(tangent) = bitexact_log2tan(Angle {
          sine: iside,
          cosine: imid,
        });
        let Ok(delta) = frac_mul16(shape.size.wrapping_sub(1).wrapping_shl(7), tangent);

        (imid, iside, delta)
      }
    };

    Ok(Split {
      inv,
      imid,
      iside,
      delta,
      itheta,
      qalloc,
    })
  }

  fn sign(&mut self) -> Result<f32, Never> {
    let bit = match self.remaining_bits.cmp(&ONE_BIT) {
      Ordering::Equal | Ordering::Greater => {
        let Ok(bit) = self.dec.bits(1);

        self.remaining_bits = self.remaining_bits.wrapping_sub(ONE_BIT);

        bit
      }
      Ordering::Less => 0,
    };

    Ok(match bit {
      0 => 1.0,
      _negative => -1.0,
    })
  }

  fn quant_band_n1(
    &mut self,
    x: &mut [f32],
    y: Option<&mut [f32]>,
    lowband_out: Option<&mut [f32]>,
  ) -> Result<u32, Never> {
    let Ok(first) = self.sign();

    match x.first_mut() {
      Some(slot) => *slot = first,
      None => {}
    }

    match y {
      Some(y_2) => {
        let Ok(second) = self.sign();

        match y_2.first_mut() {
          Some(slot) => *slot = second,
          None => {}
        }
      }
      None => {}
    }

    match (lowband_out.and_then(|out| out.first_mut()), x.first()) {
      (Some(slot), Some(value)) => *slot = *value,
      (Some(_) | None, _) => {}
    }

    Ok(1)
  }

  fn fork(&mut self, part: Part) -> Result<Visit, Never> {
    match part.resolution {
      Resolution::HalfBlock => return Ok(Visit::Leaf),
      Resolution::WholeBlocks(_) => {}
    }

    let Ok(exponent) = part.resolution.exponent();
    let Ok(cache) = self.m.cache(exponent, self.band);
    let top = match cache.first() {
      Some(top) => *top,
      None => 0,
    };
    let Ok(at) = index(top);
    let cache_max = match cache.get(at) {
      Some(most) => i32::from(*most),
      None => 0,
    };

    match (part.bits.cmp(&cache_max.wrapping_add(12)), part.size) {
      (Ordering::Greater, 3..) => {
        let Ok(fork) = self.split_part(part);

        Ok(fork)
      }
      _unsplit => Ok(Visit::Leaf),
    }
  }

  fn split_part(&mut self, part: Part) -> Result<Visit, Never> {
    let size = part.size.wrapping_shr(1);
    let Ok(resolution) = part.resolution.halved();
    let Ok(lm) = resolution.exponent();
    let fill = match part.blocks {
      1 => (part.fill & 1) | part.fill.wrapping_shl(1),
      _more => part.fill,
    };
    let blocks = part.blocks.wrapping_add(1).wrapping_shr(1);
    let mut budget = Budget {
      bits: part.bits,
      fill,
    };
    let Ok(sized) = fitted::<u32, i32>(size);
    let Ok(split) = self.compute_theta(
      Theta {
        size: sized,
        blocks,
        original_blocks: part.blocks,
        lm,
        coupling: Coupling::Mono,
      },
      &mut budget,
    );
    let Ok(delta) = adjusted_delta(
      split,
      part.blocks,
      crate::quantities::BandWidth(sized),
      crate::quantities::BlockExponent(lm),
    );
    let Ok(half) = halved(budget.bits.wrapping_sub(delta));
    let mid_bits = budget.bits.min(half).max(0);
    let side_bits = budget.bits.wrapping_sub(mid_bits);
    let Ok((MidGain(mid_gain), SideGain(side_gain))) = split.gains();

    self.remaining_bits = self.remaining_bits.wrapping_sub(split.qalloc);

    let mid_part = Part {
      offset: part.offset,
      size,
      bits: mid_bits,
      blocks,
      lowband: part.lowband,
      resolution,
      gain: BandGain(part.gain.0 * mid_gain),
      fill: budget.fill,
    };
    let side_part = Part {
      offset: part.offset.wrapping_add(size),
      size,
      bits: side_bits,
      blocks,
      lowband: part.lowband.map(|start| start.wrapping_add(size)),
      resolution,
      gain: BandGain(part.gain.0 * side_gain),
      fill: budget.fill.wrapping_shr(blocks),
    };
    let first = match mid_bits.cmp(&side_bits) {
      Ordering::Equal | Ordering::Greater => First::Mid,
      Ordering::Less => First::Side,
    };
    Ok(Visit::Forked(
      Fork {
        mid: mid_part,
        side: side_part,
        first,
        turn: Turn::First,
        shift: part.blocks.wrapping_shr(1),
        itheta: split.itheta,
        rebalance: self.remaining_bits,
        mask: 0,
      },
      match first {
        First::Mid => mid_part,
        First::Side => side_part,
      },
    ))
  }

  fn second_half(&mut self, fork: &mut Fork, answer: u32) -> Result<Part, Never> {
    let spent = fork.rebalance.wrapping_sub(self.remaining_bits);

    match fork.first {
      First::Mid => {
        fork.mask = answer;

        let rebalance = fork.mid.bits.wrapping_sub(spent);

        match (rebalance.cmp(&3i32.wrapping_shl(BITRES)), fork.itheta) {
          (_, 0) => {}
          (Ordering::Greater, _) => {
            fork.side.bits = fork
              .side
              .bits
              .wrapping_add(rebalance.wrapping_sub(3i32.wrapping_shl(BITRES)))
          }
          (Ordering::Equal | Ordering::Less, _) => {}
        }

        Ok(fork.side)
      }
      First::Side => {
        fork.mask = answer.wrapping_shl(fork.shift);

        let rebalance = fork.side.bits.wrapping_sub(spent);

        match (rebalance.cmp(&3i32.wrapping_shl(BITRES)), fork.itheta) {
          (_, 16384) => {}
          (Ordering::Greater, _) => {
            fork.mid.bits = fork
              .mid
              .bits
              .wrapping_add(rebalance.wrapping_sub(3i32.wrapping_shl(BITRES)))
          }
          (Ordering::Equal | Ordering::Less, _) => {}
        }

        Ok(fork.mid)
      }
    }
  }

  fn quant_partition(&mut self, partition: Partition<'_, '_>, root: Part) -> Result<u32, Never> {
    let mut forks: Vec<Fork> = Vec::new();
    let mut visiting = Some(root);
    let mut answer = 0u32;

    for _step in 0..PARTITION_STEPS {
      match visiting.take() {
        Some(part) => {
          let Ok(visit) = self.fork(part);

          match visit {
            Visit::Forked(fork, first) => {
              forks.push(fork);
              visiting = Some(first);
            }
            Visit::Leaf => {
              let Ok(mask) = self.leaf(
                Partition {
                  x: &mut *partition.x,
                  lowband: partition.lowband,
                },
                part,
              );

              answer = mask;
            }
          }

          continue;
        }
        None => {}
      }

      let mut fork = match forks.pop() {
        None => break,
        Some(remains) => remains,
      };

      match fork.turn {
        Turn::First => {
          let Ok(second) = self.second_half(&mut fork, answer);

          fork.turn = Turn::Second;
          forks.push(fork);
          visiting = Some(second);
        }
        Turn::Second => {
          answer = match fork.first {
            First::Mid => fork.mask | answer.wrapping_shl(fork.shift),
            First::Side => fork.mask | answer,
          };
        }
      }
    }

    Ok(answer)
  }

  fn affordable(
    &mut self,
    crate::quantities::BlockExponent(lm): crate::quantities::BlockExponent,
    crate::quantities::BitBudget(bits): crate::quantities::BitBudget,
  ) -> Result<i32, Never> {
    let Ok(mut pulses) = self.m.bits2pulses(
      self.band,
      crate::quantities::BlockExponent(lm),
      crate::quantities::BitBudget(bits),
    );
    let Ok(mut cost) = self.m.pulses2bits(
      self.band,
      crate::quantities::BlockExponent(lm),
      crate::quantities::PulseIndex(pulses),
    );

    self.remaining_bits = self.remaining_bits.wrapping_sub(cost);

    let available_pulses = pulses;

    for _pulse in 0..available_pulses {
      match (self.remaining_bits.cmp(&0), pulses) {
        (Ordering::Less, 1..) => {
          self.remaining_bits = self.remaining_bits.wrapping_add(cost);
          pulses = pulses.wrapping_sub(1);

          let Ok(cheaper) = self.m.pulses2bits(
            self.band,
            crate::quantities::BlockExponent(lm),
            crate::quantities::PulseIndex(pulses),
          );

          cost = cheaper;
          self.remaining_bits = self.remaining_bits.wrapping_sub(cost);
        }
        _within_budget => break,
      }
    }

    Ok(pulses)
  }

  fn leaf(&mut self, partition: Partition<'_, '_>, part: Part) -> Result<u32, Never> {
    let Ok(from) = index(part.offset);
    let Ok(size) = index(part.size);
    let x = match partition.x.get_mut(from..from.saturating_add(size)) {
      Some(x) => x,
      None => &mut [],
    };
    let Ok(exponent) = part.resolution.exponent();
    let Ok(pulses) = self.affordable(
      crate::quantities::BlockExponent(exponent),
      crate::quantities::BitBudget(part.bits),
    );

    match pulses {
      0 => {
        let lowband = match (partition.lowband, part.lowband) {
          (Some(lowband), Some(start)) => {
            let Ok(at) = index(start);

            lowband.get(at..)
          }
          (None, _) | (_, None) => None,
        };

        self.folded(x, lowband, part)
      }
      _some => {
        let Ok(pulses_2) = get_pulses(pulses);
        let mut iy = std::mem::take(&mut self.iy);

        iy.clear();
        iy.resize(size.min(x.len()), 0);

        let Ok(energy) =
          decode_pulses(&mut iy, pulses_2.cast_unsigned(), self.dec, &mut self.pvq_v);
        let gain = part.gain.0 / energy.sqrt();

        for (slot, pulse) in x.iter_mut().zip(iy.iter()) {
          let Ok(value) = narrow_f32(f64::from(*pulse));

          *slot = gain * value;
        }

        let Ok(()) = exp_rotation(
          x,
          Spreading {
            blocks: part.blocks,
            pulses: pulses_2.cast_unsigned(),
            spread: self.spread,
          },
        );
        let Ok(mask) = extract_collapse_mask(&iy, part.blocks);

        self.iy = iy;

        Ok(mask)
      }
    }
  }

  fn folded(&mut self, x: &mut [f32], lowband: Option<&[f32]>, part: Part) -> Result<u32, Never> {
    let mask = 1u32.wrapping_shl(part.blocks).wrapping_sub(1);
    let fill = part.fill & mask;

    match fill {
      0 => {
        x.fill(0.0);

        return Ok(0);
      }
      _some => {}
    }

    let collapsed = match lowband {
      None => {
        for slot in x.iter_mut() {
          let Ok(seed) = lcg_rand(self.seed);
          let Ok(value) = noise(seed);

          self.seed = seed;
          *slot = value;
        }

        mask
      }
      Some(lowband_2) => {
        for (slot, low) in x.iter_mut().zip(lowband_2.iter()) {
          let Ok(seed) = lcg_rand(self.seed);

          self.seed = seed;

          let dither = match seed & 0x8000 {
            0 => -1.0 / 256.0,
            _set => 1.0 / 256.0,
          };

          *slot = *low + dither;
        }

        fill
      }
    };
    let Ok(()) = renormalise_vector(x, part.gain);

    Ok(collapsed)
  }
}

fn adjusted_delta(
  split: Split,
  original_blocks: u32,
  crate::quantities::BandWidth(size): crate::quantities::BandWidth,
  crate::quantities::BlockExponent(lm): crate::quantities::BlockExponent,
) -> Result<i32, Never> {
  let delta = split.delta;

  Ok(
    match (
      original_blocks.cmp(&1),
      split.itheta & 0x3fff,
      split.itheta.cmp(&8192),
    ) {
      (Ordering::Less | Ordering::Equal, _, _) | (Ordering::Greater, 0, _) => delta,
      (Ordering::Greater, _, Ordering::Greater) => {
        delta.wrapping_sub(delta.wrapping_shr(4i32.wrapping_sub(lm).cast_unsigned()))
      }
      (Ordering::Greater, _, Ordering::Less | Ordering::Equal) => 0.min(
        delta.wrapping_add(
          size
            .wrapping_shl(BITRES)
            .wrapping_shr(5i32.wrapping_sub(lm).cast_unsigned()),
        ),
      ),
    },
  )
}

pub(crate) struct Band<'a> {
  pub(crate) size: u32,
  pub(crate) bits: i32,
  pub(crate) blocks: u32,
  pub(crate) lowband: Option<&'a mut [f32]>,
  pub(crate) lm: i32,
  pub(crate) lowband_out: Option<&'a mut [f32]>,
  pub(crate) gain: BandGain,
  pub(crate) fill: u32,
}

struct Reshaped {
  blocks: u32,
  each: u32,
  fill: u32,
  recombine: u32,
  time_divide: u32,
}

impl BandCtx<'_, '_, '_> {
  fn reshape(&mut self, band: &mut Band<'_>) -> Result<Reshaped, Never> {
    let recombine = self.tf_change.max(0).cast_unsigned();
    let mut fill = band.fill;
    let mut each = match band.size.checked_div(band.blocks) {
      Some(each) => each,
      None => 0,
    };

    for step in 0..recombine {
      match band.lowband.as_deref_mut() {
        Some(lowband) => {
          let Ok(()) = haar1(
            lowband,
            crate::quantities::VectorSize(band.size.wrapping_shr(step)),
            crate::quantities::Stride(1u32.wrapping_shl(step)),
          );
        }
        None => {}
      }

      let Ok(low) = index(fill & 0xF);
      let Ok(high) = index(fill.wrapping_shr(4));
      let low_2 = match BIT_INTERLEAVE.get(low) {
        Some(bits) => *bits,
        None => 0,
      };
      let high_2 = match BIT_INTERLEAVE.get(high) {
        Some(bits) => *bits,
        None => 0,
      };

      fill = low_2 | high_2.wrapping_shl(2);
    }

    let mut blocks = band.blocks.wrapping_shr(recombine);
    let mut time_divide = 0u32;

    each = each.wrapping_shl(recombine);

    for _step in self.tf_change..0 {
      match each & 1 {
        0 => {
          match band.lowband.as_deref_mut() {
            Some(lowband) => {
              let Ok(()) = haar1(
                lowband,
                crate::quantities::VectorSize(each),
                crate::quantities::Stride(blocks),
              );
            }
            None => {}
          }

          fill |= fill.wrapping_shl(blocks);
          blocks = blocks.wrapping_shl(1);
          each = each.wrapping_shr(1);
          time_divide = time_divide.wrapping_add(1);
        }
        _odd => break,
      }
    }

    Ok(Reshaped {
      blocks,
      each,
      fill,
      recombine,
      time_divide,
    })
  }

  pub(crate) fn quant_band(&mut self, x: &mut [f32], band: Band<'_>) -> Result<u32, Never> {
    let mut band_2 = band;

    match band_2.size {
      1 => self.quant_band_n1(x, None, band_2.lowband_out),
      _wider => {
        let long_blocks = match band_2.blocks {
          1 => Order::Hadamard,
          _short => Order::Plain,
        };
        let Ok(shape) = self.reshape(&mut band_2);
        let interleaving = Interleaving {
          size: shape.each.wrapping_shr(shape.recombine),
          stride: shape.blocks.wrapping_shl(shape.recombine),
          order: long_blocks,
        };
        let mut tmp = std::mem::take(&mut self.tmp);

        match (shape.blocks.cmp(&1), band_2.lowband.as_deref_mut()) {
          (Ordering::Greater, Some(lowband)) => {
            let Ok(()) =
              reorder_hadamard(lowband, interleaving, &mut tmp, Direction::Deinterleave);
          }
          (Ordering::Greater, None) | (Ordering::Less | Ordering::Equal, _) => {}
        }

        let root = Part {
          offset: 0,
          size: band_2.size,
          bits: band_2.bits,
          blocks: shape.blocks,
          lowband: band_2.lowband.as_ref().map(|_lowband| 0),
          resolution: Resolution::WholeBlocks(band_2.lm.cast_unsigned()),
          gain: band_2.gain,
          fill: shape.fill,
        };
        let Ok(mut mask) = self.quant_partition(
          Partition {
            x: &mut *x,
            lowband: band_2.lowband.as_deref(),
          },
          root,
        );

        match shape.blocks.cmp(&1) {
          Ordering::Greater => {
            let Ok(()) = reorder_hadamard(x, interleaving, &mut tmp, Direction::Interleave);
          }
          Ordering::Less | Ordering::Equal => {}
        }

        self.tmp = tmp;

        let Ok(blocks) = undo_shape(
          x,
          &shape,
          crate::quantities::VectorSize(band_2.size),
          &mut mask,
        );

        match band_2.lowband_out {
          Some(out) => {
            let Ok(scale) = float(band_2.size);
            let root_2 = scale.sqrt();

            for (slot, value) in out.iter_mut().zip(x.iter()) {
              *slot = root_2 * *value;
            }
          }
          None => {}
        }

        Ok(mask & 1u32.wrapping_shl(blocks).wrapping_sub(1))
      }
    }
  }
}

fn undo_shape(
  x: &mut [f32],
  shape: &Reshaped,
  crate::quantities::VectorSize(size): crate::quantities::VectorSize,
  mask: &mut u32,
) -> Result<u32, Never> {
  let mut each = shape.each;
  let mut blocks = shape.blocks;

  for _step in 0..shape.time_divide {
    blocks = blocks.wrapping_shr(1);
    each = each.wrapping_shl(1);
    *mask |= mask.wrapping_shr(blocks);

    let Ok(()) = haar1(
      x,
      crate::quantities::VectorSize(each),
      crate::quantities::Stride(blocks),
    );
  }

  for step in 0..shape.recombine {
    let Ok(low) = index(*mask & 0xF);

    *mask = match BIT_DEINTERLEAVE.get(low) {
      Some(bits) => *bits,
      None => 0,
    };

    let Ok(()) = haar1(
      x,
      crate::quantities::VectorSize(size.wrapping_shr(step)),
      crate::quantities::Stride(1u32.wrapping_shl(step)),
    );
  }

  Ok(blocks.wrapping_shl(shape.recombine))
}

pub(crate) struct StereoBand<'a> {
  pub(crate) size: u32,
  pub(crate) bits: i32,
  pub(crate) blocks: u32,
  pub(crate) lowband: Option<&'a mut [f32]>,
  pub(crate) lm: i32,
  pub(crate) lowband_out: Option<&'a mut [f32]>,
  pub(crate) fill: u32,
}

struct Channels<'x, 'y> {
  x: &'x mut [f32],
  y: &'y mut [f32],
}

impl BandCtx<'_, '_, '_> {
  fn two_phase(
    &mut self,
    channels: Channels<'_, '_>,
    band: StereoBand<'_>,
    split: Split,
  ) -> Result<u32, Never> {
    let side_bits = match split.itheta {
      0 | 16384 => 0,
      _between => ONE_BIT,
    };
    let mid_bits = band.bits.wrapping_sub(side_bits);

    self.remaining_bits = self
      .remaining_bits
      .wrapping_sub(split.qalloc.wrapping_add(side_bits));

    let bit = match side_bits {
      0 => 0,
      _some => {
        let Ok(bit) = self.dec.bits(1);

        bit
      }
    };
    let sign: f32 = match bit {
      0 => 1.0,
      _set => -1.0,
    };
    let inner = Band {
      size: band.size,
      bits: mid_bits,
      blocks: band.blocks,
      lowband: band.lowband,
      lm: band.lm,
      lowband_out: band.lowband_out,
      gain: BandGain(1.0),
      fill: band.fill,
    };
    let (coded, other) = match split.itheta.cmp(&8192) {
      Ordering::Greater => (channels.y, channels.x),
      Ordering::Less | Ordering::Equal => (channels.x, channels.y),
    };
    let Ok(mask) = self.quant_band(coded, inner);

    match (coded.first_chunk::<2>(), other.first_chunk_mut::<2>()) {
      (Some([c0, c1]), Some([o0, o1])) => {
        *o0 = -sign * *c1;
        *o1 = sign * *c0;
      }
      (Some(_) | None, _) => {}
    }

    Ok(mask)
  }

  fn quant_band_stereo(
    &mut self,
    channels: Channels<'_, '_>,
    band: StereoBand<'_>,
  ) -> Result<u32, Never> {
    let original_fill = band.fill;

    match band.size {
      1 => self.quant_band_n1(channels.x, Some(channels.y), band.lowband_out),
      _wider => {
        let Ok(sized) = fitted::<u32, i32>(band.size);
        let mut budget = Budget {
          bits: band.bits,
          fill: band.fill,
        };
        let Ok(split) = self.compute_theta(
          Theta {
            size: sized,
            blocks: band.blocks,
            original_blocks: band.blocks,
            lm: band.lm,
            coupling: Coupling::Stereo,
          },
          &mut budget,
        );
        let Ok((mid, side)) = split.gains();
        let Ok(size) = index(band.size);

        let mask = match band.size {
          2 => {
            let x = &mut *channels.x;
            let y = &mut *channels.y;
            let Ok(mask) = self.two_phase(
              Channels {
                x: &mut *x,
                y: &mut *y,
              },
              StereoBand {
                bits: budget.bits,
                fill: original_fill,
                ..band
              },
              split,
            );
            let Ok(()) = mix_two(x, y, mid, side);

            mask
          }
          _wider => {
            let Ok(mask) = self.coupled(
              Channels {
                x: &mut *channels.x,
                y: &mut *channels.y,
              },
              StereoBand {
                bits: budget.bits,
                fill: budget.fill,
                ..band
              },
              split,
              side,
            );
            let x = match channels.x.get_mut(..size) {
              Some(x) => x,
              None => &mut [],
            };
            let y = match channels.y.get_mut(..size) {
              Some(y) => y,
              None => &mut [],
            };
            let Ok(()) = stereo_merge(x, y, mid);

            mask
          }
        };

        match split.inv {
          Bit::One => {
            for value in channels.y.iter_mut().take(size) {
              *value = -*value;
            }
          }
          Bit::Zero => {}
        }

        Ok(mask)
      }
    }
  }

  fn coupled(
    &mut self,
    channels: Channels<'_, '_>,
    band: StereoBand<'_>,
    split: Split,
    SideGain(side): SideGain,
  ) -> Result<u32, Never> {
    let Ok(half) = halved(band.bits.wrapping_sub(split.delta));
    let mut mid_bits = band.bits.min(half).max(0);
    let mut side_bits = band.bits.wrapping_sub(mid_bits);

    self.remaining_bits = self.remaining_bits.wrapping_sub(split.qalloc);

    let rebalance = self.remaining_bits;
    let side_fill = band.fill.wrapping_shr(band.blocks);

    match mid_bits.cmp(&side_bits) {
      Ordering::Equal | Ordering::Greater => {
        let Ok(mid_mask) = self.quant_band(
          channels.x,
          Band {
            size: band.size,
            bits: mid_bits,
            blocks: band.blocks,
            lowband: band.lowband,
            lm: band.lm,
            lowband_out: band.lowband_out,
            gain: BandGain(1.0),
            fill: band.fill,
          },
        );
        let left = mid_bits.wrapping_sub(rebalance.wrapping_sub(self.remaining_bits));

        match (left.cmp(&3i32.wrapping_shl(BITRES)), split.itheta) {
          (_, 0) => {}
          (Ordering::Greater, _) => {
            side_bits = side_bits.wrapping_add(left.wrapping_sub(3i32.wrapping_shl(BITRES)))
          }
          (Ordering::Equal | Ordering::Less, _) => {}
        }

        let Ok(side_mask) = self.quant_band(
          channels.y,
          Band {
            size: band.size,
            bits: side_bits,
            blocks: band.blocks,
            lowband: None,
            lm: band.lm,
            lowband_out: None,
            gain: BandGain(side),
            fill: side_fill,
          },
        );

        Ok(mid_mask | side_mask)
      }
      Ordering::Less => {
        let Ok(side_mask) = self.quant_band(
          channels.y,
          Band {
            size: band.size,
            bits: side_bits,
            blocks: band.blocks,
            lowband: None,
            lm: band.lm,
            lowband_out: None,
            gain: BandGain(side),
            fill: side_fill,
          },
        );
        let left = side_bits.wrapping_sub(rebalance.wrapping_sub(self.remaining_bits));

        match (left.cmp(&3i32.wrapping_shl(BITRES)), split.itheta) {
          (_, 16384) => {}
          (Ordering::Greater, _) => {
            mid_bits = mid_bits.wrapping_add(left.wrapping_sub(3i32.wrapping_shl(BITRES)));
          }
          (Ordering::Equal | Ordering::Less, _) => {}
        }

        let Ok(mid_mask) = self.quant_band(
          channels.x,
          Band {
            size: band.size,
            bits: mid_bits,
            blocks: band.blocks,
            lowband: band.lowband,
            lm: band.lm,
            lowband_out: band.lowband_out,
            gain: BandGain(1.0),
            fill: band.fill,
          },
        );

        Ok(side_mask | mid_mask)
      }
    }
  }
}

fn mix_two(
  x: &mut [f32],
  y: &mut [f32],
  crate::quantities::MidGain(mid): crate::quantities::MidGain,
  crate::quantities::SideGain(side): crate::quantities::SideGain,
) -> Result<(), Never> {
  match (x.first_chunk_mut::<2>(), y.first_chunk_mut::<2>()) {
    (Some([x0, x1]), Some([y0, y1])) => {
      *x0 *= mid;
      *x1 *= mid;
      *y0 *= side;
      *y1 *= side;

      let first = *x0;

      *x0 = first - *y0;
      *y0 += first;

      let second = *x1;

      *x1 = second - *y1;
      *y1 += second;
    }
    (Some(_) | None, _) => {}
  }

  Ok(())
}

fn stereo_merge(x: &mut [f32], y: &mut [f32], MidGain(mid): MidGain) -> Result<(), Never> {
  let mut cross = 0f32;
  let mut side = 0f32;

  for (a, b) in x.iter().zip(y.iter()) {
    cross += a * b;
    side += b * b;
  }

  cross *= mid;

  let left_energy = mid * mid + side - 2.0 * cross;
  let right_energy = mid * mid + side + 2.0 * cross;

  match (
    right_energy.partial_cmp(&6e-4),
    left_energy.partial_cmp(&6e-4),
  ) {
    (Some(Ordering::Less), _) | (_, Some(Ordering::Less)) => {
      for (slot, value) in y.iter_mut().zip(x.iter()) {
        *slot = *value;
      }
    }
    _enough_energy => {
      let left_gain = 1.0 / left_energy.sqrt();
      let right_gain = 1.0 / right_energy.sqrt();

      for (a, b) in x.iter_mut().zip(y.iter_mut()) {
        let left = mid * *a;
        let right = *b;

        *a = left_gain * (left - right);
        *b = right_gain * (left + right);
      }
    }
  }

  Ok(())
}

pub(crate) struct AllBands<'a> {
  pub(crate) layout: Layout,
  pub(crate) pulses: &'a [i32; 21],
  pub(crate) short_blocks: Bit,
  pub(crate) spread: Spread,
  pub(crate) dual_stereo: Bit,
  pub(crate) intensity: u32,
  pub(crate) tf_res: &'a [i32; 21],
  pub(crate) total_bits: i32,
  pub(crate) balance: i32,
  pub(crate) lm: u32,
  pub(crate) coded_bands: u32,
  pub(crate) inversion: Inversion,
}

pub(crate) struct Spectra<'x, 'y> {
  pub(crate) x: &'x mut [f32],
  pub(crate) y: Option<&'y mut [f32]>,
}

enum Lowband {
  Available,
  Outside,
}

struct Folding {
  norm: Vec<f32>,
  norm2: Vec<f32>,
  scratch: Vec<f32>,
  scratch2: Vec<f32>,
  norm_offset: u32,
  lowband_offset: u32,
  update_lowband: Bit,
  balance: i32,
  dual_stereo: Bit,
}

fn scaled_edge(
  crate::quantities::BandIndex(band): crate::quantities::BandIndex,
  crate::quantities::BlockOrder(lm): crate::quantities::BlockOrder,
) -> Result<u32, Never> {
  let Ok(at) = edge(band);

  Ok(at.cast_unsigned().wrapping_shl(lm))
}

fn copy_within(buffer: &mut [f32], from: (u32, u32), to: u32) -> Result<(), Never> {
  let Ok(length) = fitted::<_, u32>(buffer.len());
  let (start, end) = from;

  match (
    start.cmp(&end),
    end.cmp(&length),
    to.saturating_add(end.saturating_sub(start)).cmp(&length),
  ) {
    (
      Ordering::Less | Ordering::Equal,
      Ordering::Less | Ordering::Equal,
      Ordering::Less | Ordering::Equal,
    ) => {
      let Ok(start_2) = index(start);
      let Ok(end_2) = index(end);
      let Ok(target) = index(to);

      buffer.copy_within(start_2..end_2, target);
    }
    _outside => {}
  }

  Ok(())
}

fn fold_masks(
  collapse_masks: &[u32],
  channels: u32,
  folds: (u32, u32),
) -> Result<(u32, u32), Never> {
  let (fold_start, fold_end) = folds;
  let mut x_mask = 0u32;
  let mut y_mask = 0u32;

  for band in fold_start..fold_end.max(fold_start.wrapping_add(1)) {
    let Ok(first) = index(band.wrapping_mul(channels));
    let Ok(last) = index(
      band
        .wrapping_mul(channels)
        .wrapping_add(channels)
        .wrapping_sub(1),
    );

    x_mask |= match collapse_masks.get(first) {
      Some(mask) => *mask,
      None => 0,
    };
    y_mask |= match collapse_masks.get(last) {
      Some(mask) => *mask,
      None => 0,
    };
  }

  Ok((x_mask, y_mask))
}

#[derive(Clone, Copy)]
struct Reach {
  band: u32,
  effective: u32,
  size: u32,
  lm: u32,
}

fn folds(folding: &Folding, reach: Reach) -> Result<(u32, u32), Never> {
  let mut fold_start = folding.lowband_offset;

  for candidate in (0..folding.lowband_offset).rev() {
    fold_start = candidate;

    let Ok(at) = scaled_edge(
      crate::quantities::BandIndex(candidate),
      crate::quantities::BlockOrder(reach.lm),
    );

    match at.cmp(&reach.effective.wrapping_add(folding.norm_offset)) {
      Ordering::Less | Ordering::Equal => break,
      Ordering::Greater => {}
    }
  }

  let mut fold_end = folding.lowband_offset;

  for candidate in folding.lowband_offset..=reach.band {
    fold_end = candidate;

    let Ok(at) = scaled_edge(
      crate::quantities::BandIndex(candidate),
      crate::quantities::BlockOrder(reach.lm),
    );

    let edge = reach
      .effective
      .wrapping_add(folding.norm_offset)
      .wrapping_add(reach.size);

    match (candidate.cmp(&reach.band), at.cmp(&edge)) {
      (Ordering::Less, Ordering::Less) => {}
      _outside => break,
    }
  }

  Ok((fold_start, fold_end))
}

fn lowband_copy<'s>(
  norm: &[f32],
  scratch: &'s mut [f32],
  effective: Option<u32>,
  size: u32,
) -> Result<Option<&'s mut [f32]>, Never> {
  let Ok(length) = index(size);

  match effective {
    None => Ok(None),
    Some(from) => {
      let Ok(start) = index(from);
      let target = match scratch.get_mut(..length) {
        Some(target) => target,
        None => &mut [],
      };

      for (slot, value) in target.iter_mut().zip(norm.iter().skip(start).take(length)) {
        *slot = *value;
      }

      Ok(Some(target))
    }
  }
}

fn lowband_out(
  norm: &mut [f32],
  last: Bit,
  crate::quantities::StartOffset(from): crate::quantities::StartOffset,
  crate::quantities::VectorSize(size): crate::quantities::VectorSize,
) -> Result<Option<&mut [f32]>, Never> {
  let Ok(start) = index(from);
  let Ok(length) = index(size);

  Ok(match last {
    Bit::One => None,
    Bit::Zero => norm.get_mut(start..start.saturating_add(length)),
  })
}

fn band_budget(
  frame: &AllBands<'_>,
  crate::quantities::BitBalance(balance): crate::quantities::BitBalance,
  band: u32,
  crate::quantities::RemainingBits(remaining): crate::quantities::RemainingBits,
) -> Result<i32, Never> {
  match band.cmp(&frame.coded_bands) {
    Ordering::Less => {
      let Ok(left) = fitted::<u32, i32>(frame.coded_bands.wrapping_sub(band));
      let current = match balance.checked_div(left.min(3)) {
        Some(current) => current,
        None => 0,
      };
      let Ok(pulses) = of(frame.pulses, band);

      Ok(
        remaining
          .wrapping_add(1)
          .min(pulses.wrapping_add(current))
          .clamp(0, 16383),
      )
    }
    Ordering::Equal | Ordering::Greater => Ok(0),
  }
}

#[derive(Clone, Copy)]
struct Placement {
  band: u32,
  start: u32,
  size: u32,
  blocks: u32,
  bits: i32,
  last: Bit,
  lm: i32,
}

struct Masks {
  x: u32,
  y: u32,
  effective: Option<u32>,
}

fn lowband_choice(
  folding: &mut Folding,
  frame: &AllBands<'_>,
  collapse_masks: &[u32],
  placement: Placement,
  channels: u32,
) -> Result<Masks, Never> {
  let lm = frame.lm;
  let layout = frame.layout;
  let first = layout.start.wrapping_add(1);

  let availability = match (
    placement
      .start
      .cmp(&placement.size.wrapping_add(folding.norm_offset)),
    placement.band.cmp(&first),
  ) {
    (Ordering::Equal | Ordering::Greater, _) | (_, Ordering::Equal) => Lowband::Available,
    (Ordering::Less, Ordering::Less | Ordering::Greater) => Lowband::Outside,
  };

  match (availability, folding.update_lowband, folding.lowband_offset) {
    (Lowband::Available, Bit::One, _) | (Lowband::Available, Bit::Zero, 0) => {
      folding.lowband_offset = placement.band;
    }
    (Lowband::Available, Bit::Zero, _) | (Lowband::Outside, _, _) => {}
  }

  match placement.band.cmp(&first) {
    Ordering::Equal => {
      let Ok(narrow) = width(layout.start);
      let Ok(wide) = width(first);
      let n1 = narrow.cast_unsigned().wrapping_shl(lm);
      let n2 = wide.cast_unsigned().wrapping_shl(lm);

      match n2.cmp(&n1) {
        Ordering::Greater => {
          let from = (n1.wrapping_mul(2).wrapping_sub(n2), n1);
          let Ok(()) = copy_within(&mut folding.norm, from, n1);
          let Ok(()) = copy_within(&mut folding.norm2, from, n1);
        }
        Ordering::Less | Ordering::Equal => {}
      }
    }
    Ordering::Less | Ordering::Greater => {}
  }

  let Ok(tf_change) = of(frame.tf_res, placement.band);

  match (
    folding.lowband_offset,
    frame.spread,
    placement.blocks,
    tf_change.cmp(&0),
  ) {
    (0, _, _, _) | (_, Spread::Aggressive, 0 | 1, Ordering::Equal | Ordering::Greater) => {
      let all = 1u32.wrapping_shl(placement.blocks).wrapping_sub(1);

      Ok(Masks {
        x: all,
        y: all,
        effective: None,
      })
    }
    _fold => {
      let Ok(low) = scaled_edge(
        crate::quantities::BandIndex(folding.lowband_offset),
        crate::quantities::BlockOrder(lm),
      );
      let effective = low.saturating_sub(folding.norm_offset.wrapping_add(placement.size));
      let Ok(range) = folds(
        folding,
        Reach {
          band: placement.band,
          effective,
          size: placement.size,
          lm,
        },
      );
      let Ok((x, y)) = fold_masks(collapse_masks, channels, range);

      Ok(Masks {
        x,
        y,
        effective: Some(effective),
      })
    }
  }
}

fn code_band(
  ctx: &mut BandCtx<'_, '_, '_>,
  spectra: &mut Spectra<'_, '_>,
  folding: &mut Folding,
  placement: Placement,
  masks: &Masks,
) -> Result<(u32, u32), Never> {
  let lm = placement.lm;
  let out_from = placement.start.wrapping_sub(folding.norm_offset);
  let Ok(from) = index(placement.start);
  let Ok(size) = index(placement.size);
  let x = match spectra.x.get_mut(from..from.saturating_add(size)) {
    Some(x) => x,
    None => &mut [],
  };
  let y = match spectra.y.as_deref_mut() {
    Some(y) => y.get_mut(from..from.saturating_add(size)),
    None => None,
  };

  match (folding.dual_stereo, y) {
    (Bit::One, Some(y_2)) => {
      let Ok(half) = halved(placement.bits);
      let Ok(lowband) = lowband_copy(
        &folding.norm,
        &mut folding.scratch,
        masks.effective,
        placement.size,
      );
      let Ok(out) = lowband_out(
        &mut folding.norm,
        placement.last,
        crate::quantities::StartOffset(out_from),
        crate::quantities::VectorSize(placement.size),
      );
      let Ok(x_mask) = ctx.quant_band(
        x,
        Band {
          size: placement.size,
          bits: half,
          blocks: placement.blocks,
          lowband,
          lm,
          lowband_out: out,
          gain: BandGain(1.0),
          fill: masks.x,
        },
      );
      let Ok(lowband_2) = lowband_copy(
        &folding.norm2,
        &mut folding.scratch2,
        masks.effective,
        placement.size,
      );
      let Ok(out_2) = lowband_out(
        &mut folding.norm2,
        placement.last,
        crate::quantities::StartOffset(out_from),
        crate::quantities::VectorSize(placement.size),
      );
      let Ok(y_mask) = ctx.quant_band(
        y_2,
        Band {
          size: placement.size,
          bits: half,
          blocks: placement.blocks,
          lowband: lowband_2,
          lm,
          lowband_out: out_2,
          gain: BandGain(1.0),
          fill: masks.y,
        },
      );

      Ok((x_mask, y_mask))
    }
    (Bit::Zero, y_3) | (Bit::One, y_3 @ None) => {
      let Ok(lowband) = lowband_copy(
        &folding.norm,
        &mut folding.scratch,
        masks.effective,
        placement.size,
      );
      let Ok(out) = lowband_out(
        &mut folding.norm,
        placement.last,
        crate::quantities::StartOffset(out_from),
        crate::quantities::VectorSize(placement.size),
      );
      let fill = masks.x | masks.y;
      let Ok(mask) = match y_3 {
        Some(y_4) => ctx.quant_band_stereo(
          Channels { x, y: y_4 },
          StereoBand {
            size: placement.size,
            bits: placement.bits,
            blocks: placement.blocks,
            lowband,
            lm,
            lowband_out: out,
            fill,
          },
        ),
        None => ctx.quant_band(
          x,
          Band {
            size: placement.size,
            bits: placement.bits,
            blocks: placement.blocks,
            lowband,
            lm,
            lowband_out: out,
            gain: BandGain(1.0),
            fill,
          },
        ),
      };

      Ok((mask, mask))
    }
  }
}

fn place(
  ctx: &mut BandCtx<'_, '_, '_>,
  frame: &AllBands<'_>,
  balance: &mut i32,
  band: u32,
) -> Result<(Placement, i32), Never> {
  let lm = frame.lm;
  let layout = frame.layout;
  let blocks = match frame.short_blocks {
    Bit::One => 1u32.wrapping_shl(lm),
    Bit::Zero => 1,
  };
  let Ok(start) = scaled_edge(
    crate::quantities::BandIndex(band),
    crate::quantities::BlockOrder(lm),
  );
  let Ok(top) = scaled_edge(
    crate::quantities::BandIndex(band.wrapping_add(1)),
    crate::quantities::BlockOrder(lm),
  );
  let Ok(told) = ctx.dec.tell_frac();
  let tell = told.cast_signed();

  match band.cmp(&layout.start) {
    Ordering::Equal => {}
    Ordering::Less | Ordering::Greater => *balance = balance.wrapping_sub(tell),
  }

  let remaining = frame.total_bits.wrapping_sub(tell).wrapping_sub(1);

  ctx.band = band;
  ctx.remaining_bits = remaining;

  let Ok(bits) = band_budget(
    frame,
    crate::quantities::BitBalance(*balance),
    band,
    crate::quantities::RemainingBits(remaining),
  );
  let last = match band.cmp(&layout.end.wrapping_sub(1)) {
    Ordering::Equal => Bit::One,
    Ordering::Less | Ordering::Greater => Bit::Zero,
  };
  let Ok(lm_signed) = fitted::<u32, i32>(lm);

  Ok((
    Placement {
      band,
      start,
      size: top.wrapping_sub(start),
      blocks,
      bits,
      last,
      lm: lm_signed,
    },
    tell,
  ))
}

fn one_band(
  ctx: &mut BandCtx<'_, '_, '_>,
  spectra: &mut Spectra<'_, '_>,
  collapse_masks: &mut [u32],
  frame: &AllBands<'_>,
  folding: &mut Folding,
  band: u32,
) -> Result<(), Never> {
  let channels = match spectra.y {
    Some(_) => 2u32,
    None => 1,
  };
  let Ok((placement, tell)) = place(ctx, frame, &mut folding.balance, band);
  let Ok(masks) = lowband_choice(folding, frame, collapse_masks, placement, channels);
  let Ok(tf_change) = of(frame.tf_res, band);

  ctx.tf_change = tf_change;

  match (folding.dual_stereo, band.cmp(&frame.intensity)) {
    (Bit::One, Ordering::Equal) => {
      folding.dual_stereo = Bit::Zero;

      let Ok(count) = index(placement.start.wrapping_sub(folding.norm_offset));

      for (first, second) in folding
        .norm
        .iter_mut()
        .zip(folding.norm2.iter())
        .take(count)
      {
        *first = 0.5 * (*first + *second);
      }
    }
    (Bit::One, Ordering::Less | Ordering::Greater) | (Bit::Zero, _) => {}
  }

  let Ok((x_mask, y_mask)) = code_band(ctx, spectra, folding, placement, &masks);
  let Ok(first) = index(band.wrapping_mul(channels));
  let Ok(last) = index(
    band
      .wrapping_mul(channels)
      .wrapping_add(channels)
      .wrapping_sub(1),
  );

  match collapse_masks.get_mut(first) {
    Some(slot) => *slot = x_mask,
    None => {}
  }

  match collapse_masks.get_mut(last) {
    Some(slot) => *slot = y_mask,
    None => {}
  }

  let Ok(pulses) = of(frame.pulses, band);

  folding.balance = folding.balance.wrapping_add(pulses).wrapping_add(tell);
  folding.update_lowband = match placement
    .bits
    .cmp(&placement.size.cast_signed().wrapping_shl(BITRES))
  {
    Ordering::Greater => Bit::One,
    Ordering::Less | Ordering::Equal => Bit::Zero,
  };

  Ok(())
}

pub(crate) fn quant_all_bands(
  mode: &Mode,
  spectra: Spectra<'_, '_>,
  collapse_masks: &mut [u32],
  frame: &AllBands<'_>,
  dec: &mut RangeDecoder<'_>,
  seed: &mut u32,
) -> Result<(), Never> {
  let layout = frame.layout;
  let Ok(norm_offset) = scaled_edge(
    crate::quantities::BandIndex(layout.start),
    crate::quantities::BlockOrder(frame.lm),
  );
  let Ok(norm_top) = scaled_edge(
    crate::quantities::BandIndex(BANDS.wrapping_sub(1)),
    crate::quantities::BlockOrder(frame.lm),
  );
  let norm_length = norm_top.wrapping_sub(norm_offset);
  let Ok(widest) = width(BANDS.wrapping_sub(1));
  let Ok(scratch_length) = index(widest.cast_unsigned().wrapping_shl(frame.lm));
  let stereo_length = match spectra.y {
    Some(_) => norm_length,
    None => 0,
  };
  let Ok(norm_length_2) = index(norm_length);
  let Ok(stereo_length_2) = index(stereo_length);
  let mut folding = Folding {
    norm: vec![0f32; norm_length_2.min(624)],
    norm2: vec![0f32; stereo_length_2.min(624)],
    scratch: vec![0f32; scratch_length.min(176)],
    scratch2: vec![0f32; scratch_length.min(176)],
    norm_offset,
    lowband_offset: 0,
    update_lowband: Bit::One,
    balance: frame.balance,
    dual_stereo: frame.dual_stereo,
  };
  let mut ctx = BandCtx {
    m: mode,
    dec,
    band: 0,
    intensity: frame.intensity,
    spread: frame.spread,
    tf_change: 0,
    remaining_bits: 0,
    seed: *seed,
    inversion: frame.inversion,
    pvq_v: Vec::new(),
    iy: Vec::new(),
    tmp: Vec::new(),
  };
  let mut spectra_2 = spectra;

  for band in layout.start..layout.end {
    let Ok(()) = one_band(
      &mut ctx,
      &mut spectra_2,
      collapse_masks,
      frame,
      &mut folding,
      band,
    );
  }

  *seed = ctx.seed;

  Ok(())
}

pub(crate) struct Collapse<'a> {
  pub(crate) masks: &'a [u32],
  pub(crate) log_e: &'a [f32; 42],
  pub(crate) prev1: &'a [f32; 42],
  pub(crate) prev2: &'a [f32; 42],
  pub(crate) pulses: &'a [i32; 21],
  pub(crate) lm: u32,
  pub(crate) size: u32,
}

fn energy_at(energies: &[f32; 42], at: u32) -> Result<f32, Never> {
  let Ok(place) = index(at);

  Ok(match energies.get(place) {
    Some(energy) => *energy,
    None => 0.0,
  })
}

fn collapse_level(
  collapse: &Collapse<'_>,
  layout: Layout,
  crate::quantities::BandIndex(band): crate::quantities::BandIndex,
  crate::quantities::ChannelIndex(channel): crate::quantities::ChannelIndex,
) -> Result<f32, Never> {
  let at = band.wrapping_add(channel.wrapping_mul(BANDS));
  let Ok(mut p1) = energy_at(collapse.prev1, at);
  let Ok(mut p2) = energy_at(collapse.prev2, at);

  match layout.channels {
    1 => {
      let Ok(other1) = energy_at(collapse.prev1, band.wrapping_add(BANDS));
      let Ok(other2) = energy_at(collapse.prev2, band.wrapping_add(BANDS));

      p1 = p1.max(other1);
      p2 = p2.max(other2);
    }
    _stereo => {}
  }

  let Ok(energy) = energy_at(collapse.log_e, at);
  let difference = (energy - p1.min(p2)).max(0.0);
  let mut level = 2.0 * (-difference).exp2();

  match collapse.lm {
    3 => level *= std::f32::consts::SQRT_2,
    _shorter => {}
  }

  Ok(level)
}

pub(crate) fn anti_collapse(
  x: &mut [f32],
  collapse: &Collapse<'_>,
  layout: Layout,
  seed: u32,
) -> Result<(), Never> {
  let mut seed_2 = seed;

  for band in layout.start..layout.end {
    let Ok(span) = width(band);
    let span_2 = span.cast_unsigned();
    let Ok(pulses) = of(collapse.pulses, band);
    let depth = match pulses.wrapping_add(1).cast_unsigned().checked_div(span_2) {
      Some(depth) => depth.wrapping_shr(collapse.lm),
      None => 0,
    };
    let Ok(depth_2) = float(depth);
    let thresh = 0.5 * (-0.125 * depth_2).exp2();
    let Ok(size) = float(span_2.wrapping_shl(collapse.lm));
    let sqrt_1 = 1.0 / size.sqrt();
    let Ok(edge_at) = edge(band);

    for channel in 0..layout.channels {
      let Ok(level) = collapse_level(
        collapse,
        layout,
        crate::quantities::BandIndex(band),
        crate::quantities::ChannelIndex(channel),
      );
      let level_2 = level.min(thresh) * sqrt_1;
      let Ok(from) = index(
        channel
          .wrapping_mul(collapse.size)
          .wrapping_add(edge_at.cast_unsigned().wrapping_shl(collapse.lm)),
      );
      let Ok(length) = index(span_2.wrapping_shl(collapse.lm));
      let Ok(at) = index(band.wrapping_mul(layout.channels).wrapping_add(channel));
      let mask = match collapse.masks.get(at) {
        Some(mask) => *mask,
        None => 0,
      };
      let block = match x.get_mut(from..from.saturating_add(length)) {
        Some(block) => block,
        None => &mut [],
      };
      let Ok(renormalise) = fill_collapsed(
        block,
        crate::quantities::CollapseMask(mask),
        (level_2, collapse.lm),
        &mut seed_2,
      );

      match renormalise {
        Bit::One => {
          let Ok(()) = renormalise_vector(block, BandGain(1.0));
        }
        Bit::Zero => {}
      }
    }
  }

  Ok(())
}

fn fill_collapsed(
  block: &mut [f32],
  crate::quantities::CollapseMask(mask): crate::quantities::CollapseMask,
  shape: (f32, u32),
  seed: &mut u32,
) -> Result<Bit, Never> {
  let (level, lm) = shape;
  let Ok(apart) = index(1u32.wrapping_shl(lm));
  let mut filled = Bit::Zero;

  for short in 0..1u32.wrapping_shl(lm) {
    match mask & 1u32.wrapping_shl(short) {
      0 => {
        let Ok(skip) = index(short);

        for slot in block.iter_mut().skip(skip).step_by(apart.max(1)) {
          let Ok(next) = lcg_rand(*seed);

          *seed = next;
          *slot = match next & 0x8000 {
            0 => -level,
            _set => level,
          };
        }

        filled = Bit::One;
      }
      _kept => {}
    }
  }

  Ok(filled)
}

pub(crate) struct Denormal<'a> {
  pub(crate) band_log_e: &'a [f32],
  pub(crate) start: u32,
  pub(crate) end: u32,
  pub(crate) lm: u32,
  pub(crate) silence: Bit,
}

pub(crate) fn denormalise_bands(
  x: &[f32],
  freq: &mut [f32],
  asked: &Denormal<'_>,
) -> Result<(), Never> {
  let lm = asked.lm;
  let Ok(length) = index(SHORT_MDCT.wrapping_shl(lm));
  let (start, end) = match asked.silence {
    Bit::One => (0, 0),
    Bit::Zero => (asked.start, asked.end),
  };
  let Ok(bound) = scaled_edge(
    crate::quantities::BandIndex(end),
    crate::quantities::BlockOrder(lm),
  );
  let Ok(bound_2) = index(bound);
  let Ok(first) = scaled_edge(
    crate::quantities::BandIndex(start),
    crate::quantities::BlockOrder(lm),
  );
  let Ok(first_2) = index(first);

  for slot in freq.iter_mut().take(first_2) {
    *slot = 0.0;
  }

  for band in start..end {
    let Ok(low_edge) = scaled_edge(
      crate::quantities::BandIndex(band),
      crate::quantities::BlockOrder(lm),
    );
    let Ok(high_edge) = scaled_edge(
      crate::quantities::BandIndex(band.wrapping_add(1)),
      crate::quantities::BlockOrder(lm),
    );
    let Ok(low) = index(low_edge);
    let Ok(count) = index(high_edge.wrapping_sub(low_edge));
    let Ok(at) = index(band);
    let energy = match asked.band_log_e.get(at) {
      Some(energy) => *energy,
      None => 0.0,
    };
    let mean = match E_MEANS.get(at) {
      Some(mean) => *mean,
      None => 0.0,
    };
    let gain = (energy + mean).min(32.0).exp2();

    for (slot, value) in freq
      .iter_mut()
      .skip(low)
      .take(count)
      .zip(x.iter().skip(low))
    {
      *slot = *value * gain;
    }
  }

  for slot in freq.iter_mut().take(length).skip(bound_2) {
    *slot = 0.0;
  }

  Ok(())
}
