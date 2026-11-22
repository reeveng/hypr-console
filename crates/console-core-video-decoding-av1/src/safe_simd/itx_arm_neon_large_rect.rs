//! AArch64 8×32, 32×8, 16×32 and 32×16 DCT transforms.
//! Each horizontal kernel transposes its eight-row stripe into fixed-size
//! scratch tiles. The vertical kernels consume the same tile columns.

#![cfg_attr(not(feature = "unchecked"), forbid(unsafe_code))]
#![cfg_attr(feature = "unchecked", deny(unsafe_code))]

use archmage::intrinsics::aarch64 as safe_simd;
use archmage::{Arm64, arcane, rite};
use console_core_geometry::Point;
use console_core_never::Never;
use console_core_number_conversion::{index, wrapped};
use core::arch::aarch64::*;

use super::itx_arm_neon_8x8::{idct_8_q, transpose_8x8h};
use super::itx_arm_neon_16x16::idct_16_q;
use super::itx_arm_neon_32::idct32_odd_q;
use super::itx_arm_neon_64::{LargeNeonBlock, LargeNeonOutput, add_pixel_lane, add_uniform};
use crate::src::error::{Rav1dError::EINVAL, Rav1dResult};

#[derive(Clone, Copy)]
enum Axis {
  Eight,
  Sixteen,
  ThirtyTwo,
}

impl Axis {
  fn length(self) -> Result<u64, Never> {
    Ok(match self {
      Self::Eight => 8,
      Self::Sixteen => 16,
      Self::ThirtyTwo => 32,
    })
  }
}

#[derive(Clone, Copy)]
enum RectNormalization {
  Equal,
  Scale,
}

impl RectNormalization {
  fn shift(self) -> Result<u32, Never> {
    Ok(match self {
      Self::Equal => 2,
      Self::Scale => 1,
    })
  }
}

struct RectPlan {
  horizontal: Axis,
  vertical: Axis,
  normalization: RectNormalization,
  row_thresholds: [i32; 4],
}

#[rite(neon)]
fn dc_only(block: LargeNeonBlock<'_>, normalization: RectNormalization) -> Rav1dResult {
  let coefficient = block.coefficients.first_mut().ok_or(EINVAL)?;
  let dc = i32::from(*coefficient);
  *coefficient = 0;
  let Ok(shift) = normalization.shift();
  let rounding = 1_i32.wrapping_shl(shift.wrapping_sub(1));
  let normalized = match normalization {
    RectNormalization::Equal => dc,
    RectNormalization::Scale => dc.wrapping_mul(181).wrapping_add(128).wrapping_shr(8),
  };
  let intermediate = normalized.wrapping_mul(181).wrapping_add(128).wrapping_shr(8);
  let first = intermediate.wrapping_add(rounding).wrapping_shr(shift);
  let residual = first.wrapping_mul(181).wrapping_add(2176).wrapping_shr(12);
  let Ok(value) = wrapped::<_, i16>(residual);
  add_uniform(block.output, vdupq_n_s16(value))
}

#[rite(neon)]
fn dct32(input: [int16x8_t; 32]) -> Result<[int16x8_t; 32], Never> {
  let (pairs, _tail) = input.as_chunks::<2>();
  let mut even = [vdupq_n_s16(0); 16];
  let mut odd = even;

  for ((even_lane, odd_lane), [first, second]) in even.iter_mut().zip(&mut odd).zip(pairs) {
    *even_lane = *first;
    *odd_lane = *second;
  }

  let even_result = idct_16_q(even);
  let odd_result = idct32_odd_q(odd);
  let mut low = even_result;
  let mut high = even_result;

  for (((front, back), &even_lane), &odd_lane) in low.iter_mut().zip(high.iter_mut().rev())
    .zip(&even_result).zip(odd_result.iter().rev())
  {
    *front = vqaddq_s16(even_lane, odd_lane);
    *back = vqsubq_s16(even_lane, odd_lane);
  }

  let [a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p] = low;
  let [q, r, s, t, u, v, w, x, y, z, aa, ab, ac, ad, ae, af] = high;
  Ok([a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q, r, s, t, u, v, w, x, y, z, aa, ab, ac, ad, ae, af])
}

#[rite(neon)]
fn transform_lanes(mut samples: [int16x8_t; 32], axis: Axis) -> Result<[int16x8_t; 32], Never> {
  match axis {
    Axis::Eight => {
      let [a, b, c, d, e, f, g, h, ..] = samples;
      let (r0, r1, r2, r3, r4, r5, r6, r7) = idct_8_q(a, b, c, d, e, f, g, h);

      for (target, value) in samples.iter_mut().zip([r0, r1, r2, r3, r4, r5, r6, r7]) {
        *target = value;
      }

      Ok(samples)
    }
    Axis::Sixteen => {
      let [a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, ..] = samples;
      let output = idct_16_q([a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p]);

      for (target, value) in samples.iter_mut().zip(output) {
        *target = value;
      }

      Ok(samples)
    }
    Axis::ThirtyTwo => dct32(samples),
  }
}

type RectScratch = [[[i16; 8]; 4]; 32];

struct RectStripe {
  start: u64,
  row_pitch: u64,
}

#[rite(neon)]
fn horizontal(
  coefficients: &mut [i16],
  scratch: &mut RectScratch,
  stripe: RectStripe,
  plan: &RectPlan,
) -> Rav1dResult {
  let Ok(width) = plan.horizontal.length();
  let Ok(active) = index(width);
  let mut columns = [vdupq_n_s16(0); 32];

  for (column, target) in (0_u64..).zip(columns.iter_mut().take(active)) {
    let from = column.wrapping_mul(stripe.row_pitch).wrapping_add(stripe.start);
    let Ok(start) = index(from);
    let group = coefficients.get_mut(start..).and_then(<[_]>::first_chunk_mut).ok_or(EINVAL)?;
    let loaded = safe_simd::vld1q_s16(group);
    *target = match plan.normalization {
      RectNormalization::Equal => loaded,
      RectNormalization::Scale => vqrdmulhq_n_s16(loaded, 23168),
    };
    group.fill(0);
  }

  let Ok(transformed) = transform_lanes(columns, plan.horizontal);
  let (column_groups, _tail) = transformed.as_chunks::<8>();
  let Ok(tile_columns) = index(width.wrapping_div(8));
  let Ok(row_start) = index(stripe.start);
  let rows = scratch.get_mut(row_start..).and_then(<[_]>::first_chunk_mut::<8>).ok_or(EINVAL)?;

  for (group, values) in (0..tile_columns).zip(column_groups) {
    let [a, b, c, d, e, f, g, h] = values.map(|lane| match plan.normalization {
      RectNormalization::Equal => vrshrq_n_s16::<2>(lane),
      RectNormalization::Scale => vrshrq_n_s16::<1>(lane),
    });
    let (r0, r1, r2, r3, r4, r5, r6, r7) = transpose_8x8h(a, b, c, d, e, f, g, h);

    for (destination, residual) in rows.iter_mut().zip([r0, r1, r2, r3, r4, r5, r6, r7]) {
      let target = destination.get_mut(group).ok_or(EINVAL)?;
      safe_simd::vst1q_s16(target, residual);
    }
  }

  Ok(())
}

#[rite(neon)]
fn vertical(
  mut output: LargeNeonOutput<'_>,
  scratch: &RectScratch,
  plan: &RectPlan,
) -> Rav1dResult {
  let Ok(height) = plan.vertical.length();
  let Ok(width) = plan.horizontal.length();
  let Ok(active) = index(height);

  for group in 0..width.wrapping_div(8) {
    let Ok(column) = index(group);
    let mut lanes = [vdupq_n_s16(0); 32];

    for (slot, source) in lanes.iter_mut().zip(scratch.iter().take(active)) {
      let samples = source.get(column).ok_or(EINVAL)?;
      *slot = safe_simd::vld1q_s16(samples);
    }

    let Ok(residuals) = transform_lanes(lanes, plan.vertical);

    for (y, residual) in (0..height).zip(residuals) {
      let point = Point { x: group.wrapping_mul(8), y };
      let target = output.lane(point)?;
      let Ok(()) = add_pixel_lane(target, vrshrq_n_s16::<4>(residual));
    }
  }

  Ok(())
}

#[arcane]
pub(crate) fn dct_large_rect_neon(_token: Arm64, block: LargeNeonBlock<'_>) -> Rav1dResult {
  const ALL_ROWS: [i32; 4] = [i32::MIN; 4];
  let Ok(size) = block.output.size();
  let plan = match (size.width, size.height) {
    (8, 32) => RectPlan { horizontal: Axis::Eight, vertical: Axis::ThirtyTwo, normalization: RectNormalization::Equal, row_thresholds: ALL_ROWS },
    (32, 8) => RectPlan { horizontal: Axis::ThirtyTwo, vertical: Axis::Eight, normalization: RectNormalization::Equal, row_thresholds: ALL_ROWS },
    (16, 32) => RectPlan { horizontal: Axis::Sixteen, vertical: Axis::ThirtyTwo, normalization: RectNormalization::Scale, row_thresholds: [i32::MIN, 36, 151, 279] },
    (32, 16) => RectPlan { horizontal: Axis::ThirtyTwo, vertical: Axis::Sixteen, normalization: RectNormalization::Scale, row_thresholds: ALL_ROWS },
    _invalid => return Err(EINVAL),
  };

  match block.eob {
    0 => dc_only(block, plan.normalization),
    _non_dc => {
      let mut scratch = [[[0_i16; 8]; 4]; 32];
      let row_groups = size.height.wrapping_div(8);
      let active_rows = plan.row_thresholds.into_iter().zip(0..row_groups)
        .take_while(|&(threshold, _group)| block.eob >= threshold);
      let Ok(group_count) = console_core_number_conversion::fixed_index(active_rows.clone().count());
      let required = size.width.wrapping_sub(1).wrapping_mul(size.height).wrapping_add(group_count.wrapping_mul(8));
      let Ok(count) = index(required);
      block.coefficients.get(..count).ok_or(EINVAL)?;

      for (_threshold, group) in active_rows {
        let stripe = RectStripe { start: group.wrapping_mul(8), row_pitch: size.height };
        horizontal(block.coefficients, &mut scratch, stripe, &plan)?;
      }

      vertical(block.output, &scratch, &plan)
    }
  }
}
