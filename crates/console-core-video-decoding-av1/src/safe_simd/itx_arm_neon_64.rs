//! AArch64 transforms spanning a 64-sample axis. The full DCT keeps the scalar
//! transform's reduced coefficient rectangle, per-axis clipping and rounding,
//! and applies the resulting residuals with eight-lane NEON operations.

#![cfg_attr(not(feature = "unchecked"), forbid(unsafe_code))]
#![cfg_attr(feature = "unchecked", deny(unsafe_code))]

use archmage::intrinsics::aarch64 as safe_simd;
use archmage::{Arm64, arcane, rite};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};
use console_core_pixel_buffers::RowLayout;
use core::arch::aarch64::*;
use core::num::NonZeroU32;

use super::itx_arm_neon_8x8::transpose_8x8h;
use crate::include::common::intops::iclip;
use crate::src::error::{Rav1dError::EINVAL, Rav1dResult};
use crate::src::itx::{CoefficientBounds, Itx1dFn};
use crate::src::itx_1d::{rav1d_inv_dct16_1d_c, rav1d_inv_dct32_1d_c, rav1d_inv_dct64_1d_c};

pub(crate) struct LargeNeonOutput<'a> {
  pixels: &'a mut [u8],
  layout: RowLayout,
  size: Size<u64>,
  first: u64,
  pitch: i64,
}

impl<'a> LargeNeonOutput<'a> {
  pub(crate) fn new(
    pixels: &'a mut [u8],
    size: Size<u64>,
    origin: (u64, i64),
  ) -> Rav1dResult<Self> {
    let (first, pitch) = origin;
    let layout = RowLayout::new(size, first, pitch).map_err(|_layout| EINVAL)?;
    layout.validate_len(pixels.len()).map_err(|_extent| EINVAL)?;

    Ok(Self { pixels, layout, size, first, pitch })
  }

  pub(super) fn size(&self) -> Result<Size<u64>, Never> {
    Ok(self.size)
  }

  pub(super) fn lane(&mut self, point: Point<u64>) -> Rav1dResult<&mut [u8; 8]> {
    let line = point.y.cast_signed().wrapping_mul(self.pitch);
    let first = self.first.wrapping_add_signed(line).wrapping_add(point.x);
    let Ok(position) = index(first);
    self.pixels.get_mut(position..).and_then(<[_]>::first_chunk_mut).ok_or(EINVAL)
  }
}

pub(crate) struct LargeNeonBlock<'a> {
  pub(crate) output: LargeNeonOutput<'a>,
  pub(crate) coefficients: &'a mut [i16],
  pub(crate) eob: i32,
}

#[rite(neon)]
pub(super) fn add_pixel_lane(target: &mut [u8; 8], residual: int16x8_t) -> Result<(), Never> {
  let pixels = safe_simd::vld1_u8(target);
  let sum = vreinterpretq_s16_u16(vaddw_u8(vreinterpretq_u16_s16(residual), pixels));
  safe_simd::vst1_u8(target, vqmovun_s16(sum));
  Ok(())
}

#[rite(neon)]
pub(super) fn add_uniform(output: LargeNeonOutput<'_>, residual: int16x8_t) -> Rav1dResult {
  let rows = output.layout.rows_mut(output.pixels).map_err(|_bounds| EINVAL)?;

  for row in rows {
    let (lanes, _tail) = row.as_chunks_mut();

    for lane in lanes {
      let Ok(()) = add_pixel_lane(lane, residual);
    }
  }

  Ok(())
}

#[derive(Clone, Copy)]
enum FirstPass {
  ShiftOne,
  ShiftTwo,
}

impl FirstPass {
  fn shift(self) -> Result<u32, Never> {
    Ok(match self {
      Self::ShiftOne => 1,
      Self::ShiftTwo => 2,
    })
  }
}

#[rite(neon)]
fn dc_only(block: LargeNeonBlock<'_>, pass: FirstPass) -> Rav1dResult {
  let coefficient = block.coefficients.first_mut().ok_or(EINVAL)?;
  let initial = vdupq_n_s16(*coefficient);
  *coefficient = 0;
  let scale = vdupq_n_s16(23168);
  let primary = vqrdmulhq_s16(initial, scale);
  let Size { width, height } = block.output.size;
  let normalized = match (width, height) {
    (32, 64) | (64, 32) => vqrdmulhq_s16(primary, scale),
    (64, 64) | (64, 16) | (16, 64) => primary,
    _invalid => return Err(EINVAL),
  };
  let intermediate = match pass {
    FirstPass::ShiftOne => vrshrq_n_s16::<1>(normalized),
    FirstPass::ShiftTwo => vrshrq_n_s16::<2>(normalized),
  };
  let scaled = vqrdmulhq_s16(intermediate, scale);
  add_uniform(block.output, vrshrq_n_s16::<4>(scaled))
}

fn select_dct(points: u64) -> Rav1dResult<Itx1dFn> {
  match points {
    16 => Ok(rav1d_inv_dct16_1d_c),
    32 => Ok(rav1d_inv_dct32_1d_c),
    64 => Ok(rav1d_inv_dct64_1d_c),
    _invalid => Err(EINVAL),
  }
}

fn large_dct_rows(
  block: &LargeNeonBlock<'_>,
  pass: FirstPass,
  scratch: &mut [[i32; 64]; 64],
) -> Rav1dResult {
  let Size { width, height } = block.output.size;
  let horizontal = select_dct(width)?;
  let square = height.min(32);
  let Ok(rows) = index(square);
  let Ok(written) = index(width);
  let Ok(columns) = index(width.min(32));
  let Ok(shift) = pass.shift();
  let rounding = 1_i32.wrapping_shl(shift.wrapping_sub(1));
  let bounds = CoefficientBounds { lower: i32::from(i16::MIN), upper: i32::from(i16::MAX) };
  let rect = width.wrapping_mul(2) == height || height.wrapping_mul(2) == width;

  for (y, row) in (0_u64..).zip(scratch.iter_mut().take(rows)) {
    for (x, value) in (0_u64..).zip(row.iter_mut().take(columns)) {
      let Ok(position) = index(x.wrapping_mul(square).wrapping_add(y));
      let coefficient = match block.coefficients.get(position) {
        Some(&sample) => i32::from(sample),
        None => 0,
      };
      *value = match rect.then_some(()) {
        Some(()) => coefficient.wrapping_mul(181).wrapping_add(128).wrapping_shr(8),
        None => coefficient,
      };
    }

    horizontal(row, NonZeroU32::MIN, bounds)?;

    for value in row.iter_mut().take(written) {
      *value = iclip(value.wrapping_add(rounding).wrapping_shr(shift), bounds.lower, bounds.upper);
    }
  }

  Ok(())
}

fn large_dct_columns(size: Size<u64>, scratch: &mut [[i32; 64]; 64]) -> Rav1dResult {
  let vertical = select_dct(size.height)?;
  let Ok(width) = index(size.width);
  let Ok(height) = index(size.height);
  let bounds = CoefficientBounds { lower: i32::from(i16::MIN), upper: i32::from(i16::MAX) };
  let mut column = [0_i32; 64];

  for x in 0..width {
    for (value, row) in column.iter_mut().zip(scratch.iter().take(height)) {
      let &sample = row.get(x).ok_or(EINVAL)?;
      *value = sample;
    }

    vertical(&mut column, NonZeroU32::MIN, bounds)?;

    for (&value, row) in column.iter().zip(scratch.iter_mut().take(height)) {
      let sample = row.get_mut(x).ok_or(EINVAL)?;
      *sample = value;
    }
  }

  Ok(())
}

#[rite(neon)]
fn add_transformed(output: LargeNeonOutput<'_>, scratch: &[[i32; 64]; 64]) -> Rav1dResult {
  let rows = output.layout.rows_mut(output.pixels).map_err(|_bounds| EINVAL)?;

  for (row, transformed) in rows.zip(scratch) {
    let (destination, _tail) = row.as_chunks_mut();
    let (source, _unused) = transformed.as_chunks::<8>();

    for (target, values) in destination.iter_mut().zip(source) {
      let residuals = values.map(|value| {
        let code = value.wrapping_add(8).wrapping_shr(4);
        let Ok(sample) = console_core_number_conversion::fitted::<_, i16>(code);
        sample
      });
      let Ok(()) = add_pixel_lane(target, safe_simd::vld1q_s16(&residuals));
    }
  }

  Ok(())
}

#[arcane]
pub(crate) fn dct_64_axis_neon(_token: Arm64, block: LargeNeonBlock<'_>) -> Rav1dResult {
  let pass = match (block.output.size.width, block.output.size.height) {
    (32, 64) | (64, 32) => FirstPass::ShiftOne,
    (64, 64) | (64, 16) | (16, 64) => FirstPass::ShiftTwo,
    _invalid => return Err(EINVAL),
  };

  match block.eob {
    0 => dc_only(block, pass),
    _non_dc => {
      let mut scratch = [[0_i32; 64]; 64];
      large_dct_rows(&block, pass, &mut scratch)?;
      large_dct_columns(block.output.size, &mut scratch)?;
      add_transformed(block.output, &scratch)?;
      block.coefficients.fill(0);
      Ok(())
    }
  }
}

#[derive(Clone, Copy)]
enum IdentityScaling {
  LargeAxis,
  TallNarrow,
  ShortWide,
  TallRect,
  WideRect,
}

#[derive(Clone, Copy)]
struct IdentityThresholds {
  rows: &'static [i32],
  columns: &'static [i32],
}

fn identity_groups(
  size: Size<u64>,
  scaling: IdentityScaling,
) -> Rav1dResult<IdentityThresholds> {
  const TWO: &[i32] = &[36];
  const FOUR: &[i32] = &[36, 136, 300];
  const FOUR_RECT: &[i32] = &[36, 151, 279];
  const EIGHT: &[i32] = &[36, 136, 300, 1024, 1024, 1024, 1024];

  Ok(match (scaling, size.width, size.height) {
    (IdentityScaling::LargeAxis, 64, 64) => IdentityThresholds { rows: EIGHT, columns: EIGHT },
    (IdentityScaling::LargeAxis, 64, 32) => IdentityThresholds { rows: FOUR, columns: EIGHT },
    (IdentityScaling::LargeAxis, 32, 64) => IdentityThresholds { rows: EIGHT, columns: FOUR },
    (IdentityScaling::LargeAxis, 16, 64) => IdentityThresholds { rows: EIGHT, columns: TWO },
    (IdentityScaling::LargeAxis, 64, 16) => IdentityThresholds { rows: TWO, columns: EIGHT },
    (IdentityScaling::TallNarrow, 8, 32) => IdentityThresholds { rows: &[i32::MIN; 3], columns: &[] },
    (IdentityScaling::ShortWide, 32, 8) => IdentityThresholds { rows: &[], columns: &[i32::MIN; 3] },
    (IdentityScaling::TallRect, 16, 32) => IdentityThresholds { rows: FOUR_RECT, columns: TWO },
    (IdentityScaling::WideRect, 32, 16) => IdentityThresholds { rows: TWO, columns: FOUR_RECT },
    _invalid => return Err(EINVAL),
  })
}

#[rite(neon)]
fn scale_identity_column(values: int16x8_t, scaling: IdentityScaling) -> Result<int16x8_t, Never> {
  Ok(match scaling {
    IdentityScaling::TallRect => {
      let scaled = vqrdmulhq_n_s16(values, 23168);
      let fraction = vrshrq_n_s16::<1>(vqrdmulhq_n_s16(scaled, 27152));
      vqaddq_s16(scaled, fraction)
    }
    IdentityScaling::WideRect => {
      let scaled = vqshlq_n_s16::<1>(vqrdmulhq_n_s16(values, 23168));
      let fraction = vqrdmulhq_n_s16(scaled, 27152);
      vqaddq_s16(vqaddq_s16(scaled, scaled), fraction)
    }
    IdentityScaling::LargeAxis | IdentityScaling::TallNarrow | IdentityScaling::ShortWide => values,
  })
}

#[rite(neon)]
fn round_identity_row(values: int16x8_t, scaling: IdentityScaling) -> Result<int16x8_t, Never> {
  Ok(match scaling {
    IdentityScaling::TallNarrow => vrshrq_n_s16::<2>(vrshrq_n_s16::<1>(values)),
    IdentityScaling::ShortWide => vrshrq_n_s16::<3>(values),
    IdentityScaling::WideRect => vrshrq_n_s16::<4>(values),
    IdentityScaling::LargeAxis | IdentityScaling::TallRect => vrshrq_n_s16::<2>(values),
  })
}

#[rite(neon)]
fn write_identity_tile(
  block: &mut LargeNeonBlock<'_>,
  origin: Point<u64>,
  scaling: IdentityScaling,
) -> Rav1dResult {
  let mut inputs = [vdupq_n_s16(0); 8];

  for (column, target) in (origin.x..).zip(&mut inputs) {
    let Ok(position) = index(column.wrapping_mul(block.output.size.height).wrapping_add(origin.y));
    let coefficients = block.coefficients.get_mut(position..)
      .and_then(<[_]>::first_chunk_mut).ok_or(EINVAL)?;
    let loaded = safe_simd::vld1q_s16(coefficients);
    let Ok(scaled) = scale_identity_column(loaded, scaling);
    *target = scaled;
    coefficients.fill(0);
  }

  let [a, b, c, d, e, f, g, h] = inputs;
  let (r0, r1, r2, r3, r4, r5, r6, r7) = transpose_8x8h(a, b, c, d, e, f, g, h);

  for (y, values) in (origin.y..).zip([r0, r1, r2, r3, r4, r5, r6, r7]) {
    let target = block.output.lane(Point { x: origin.x, y })?;
    let Ok(shifted) = round_identity_row(values, scaling);
    let Ok(()) = add_pixel_lane(target, shifted);
  }

  Ok(())
}

#[rite(neon)]
fn run_large_identity(mut block: LargeNeonBlock<'_>, scaling: IdentityScaling) -> Rav1dResult {
  let thresholds = identity_groups(block.output.size, scaling)?;
  let row_groups = core::iter::once(i32::MIN).chain(thresholds.rows.iter().copied())
    .take_while(|&threshold| block.eob >= threshold);
  let columns = core::iter::once(i32::MIN).chain(thresholds.columns.iter().copied())
    .take_while(|&threshold| block.eob >= threshold);
  let Ok(column_count) = fixed_index(columns.count());
  let Ok(row_count) = fixed_index(row_groups.count());
  let last = column_count.wrapping_mul(8).wrapping_sub(1)
    .wrapping_mul(block.output.size.height).wrapping_add(row_count.wrapping_mul(8));
  let Ok(required) = index(last);
  block.coefficients.get(..required).ok_or(EINVAL)?;

  for tile_y in 0..row_count {
    for tile_x in 0..column_count {
      let origin = Point { x: tile_x.wrapping_mul(8), y: tile_y.wrapping_mul(8) };
      write_identity_tile(&mut block, origin, scaling)?;
    }
  }

  Ok(())
}

#[arcane]
pub(crate) fn identity_64_axis_neon(_token: Arm64, block: LargeNeonBlock<'_>) -> Rav1dResult {
  run_large_identity(block, IdentityScaling::LargeAxis)
}

#[arcane]
pub(crate) fn identity_rect_neon(_token: Arm64, block: LargeNeonBlock<'_>) -> Rav1dResult {
  let scaling = match (block.output.size.width, block.output.size.height) {
    (8, 32) => IdentityScaling::TallNarrow,
    (32, 8) => IdentityScaling::ShortWide,
    (16, 32) => IdentityScaling::TallRect,
    (32, 16) => IdentityScaling::WideRect,
    _invalid => return Err(EINVAL),
  };
  run_large_identity(block, scaling)
}
