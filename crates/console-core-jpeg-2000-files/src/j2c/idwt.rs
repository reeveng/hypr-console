//! Performing the inverse discrete wavelet transform, as specified in Annex F.

use alloc::vec::Vec;
use console_core_size::Size;
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, indices, wrapped_index};

use super::build::{Decomposition, SubBand};
use super::codestream::WaveletTransform;
use super::decode::{DecompositionStorage, TileDecodeContext, TileDecompositions};
use super::rect::IntRect;
use crate::error::{DecodingError, Result};
use crate::j2c::Header;
use crate::math::{self, Float32x8, Level, SIMD_WIDTH, Simd, dispatch};

pub(crate) struct IDWTOutput {
  pub(crate) coefficients: Vec<f32>,
  pub(crate) rect: IntRect,
}

impl Default for IDWTOutput {
  fn default() -> Self {
    Self {
      coefficients: Vec::new(),
      rect: IntRect {
        x0: 0,
        y0: 0,
        x1: u32::MAX,
        y1: u32::MAX,
      },
    }
  }
}

pub(crate) fn apply(
  storage: &DecompositionStorage<'_>,
  tile_ctx: &mut TileDecodeContext,
  tile: &TileDecompositions,
  header: &Header<'_>,
  transform: WaveletTransform,
) -> Result<()> {
  let Ok(decomposition_range) = indices(tile.decompositions.clone());
  let decompositions = storage
    .decompositions
    .get(decomposition_range)
    .ok_or(DecodingError::InvalidPrecinct)?;
  let Ok(skipped) = wrapped_index(header.skipped_resolution_levels);
  let retained = decompositions
    .get(..decompositions.len().saturating_sub(skipped))
    .ok_or(DecodingError::InvalidPrecinct)?;
  let Ok(ll_index) = index(tile.first_ll_sub_band);
  let low_band = storage
    .sub_bands
    .get(ll_index)
    .ok_or(DecodingError::InvalidPrecinct)?;
  let low = band_coefficients(storage, low_band)?;
  let output = &mut tile_ctx.idwt_output;
  let scratch = &mut tile_ctx.idwt_scratch_buffer;

  let (first, rest) = match retained {
    [one, remaining @ ..] => (one, remaining),
    [] => {
      output.coefficients.clear();
      output.coefficients.extend_from_slice(low);
      output.rect = low_band.rect;
      return Ok(());
    }
  };

  let (mut current, mut next) =
    match ::core::primitive::bool::then_some(rest.len().is_multiple_of(2), ()) {
      ::core::option::Option::Some(()) => (&mut output.coefficients, scratch),
      ::core::option::Option::None => (scratch, &mut output.coefficients),
    };

  filter_2d(low, current, first, transform, storage)?;
  let mut rect = first.rect;

  for decomposition in rest {
    filter_2d(current, next, decomposition, transform, storage)?;
    core::mem::swap(&mut current, &mut next);
    rect = decomposition.rect;
  }

  output.rect = rect;
  Ok(())
}

fn band_coefficients<'a>(
  storage: &'a DecompositionStorage<'_>,
  band: &SubBand,
) -> Result<&'a [f32]> {
  let Ok(coefficient_range) = indices(band.coefficients.clone());
  let slice = storage
    .coefficients
    .get(coefficient_range)
    .ok_or(DecodingError::InvalidPrecinct)?;
  Ok(slice)
}

struct Bands<'a> {
  ll: &'a [f32],
  hl: &'a [f32],
  lh: &'a [f32],
  hh: &'a [f32],
}

fn filter_2d(
  ll: &[f32],
  output: &mut Vec<f32>,
  decomposition: &Decomposition,
  transform: WaveletTransform,
  storage: &DecompositionStorage<'_>,
) -> Result<()> {
  let [hl_index, lh_index, hh_index] = decomposition.sub_bands;
  let Ok(hl_position) = index(hl_index);
  let hl_band = storage
    .sub_bands
    .get(hl_position)
    .ok_or(DecodingError::InvalidPrecinct)?;
  let Ok(lh_position) = index(lh_index);
  let lh_band = storage
    .sub_bands
    .get(lh_position)
    .ok_or(DecodingError::InvalidPrecinct)?;
  let Ok(hh_position) = index(hh_index);
  let hh_band = storage
    .sub_bands
    .get(hh_position)
    .ok_or(DecodingError::InvalidPrecinct)?;
  let hl = band_coefficients(storage, hl_band)?;
  let lh = band_coefficients(storage, lh_band)?;
  let hh = band_coefficients(storage, hh_band)?;
  let bands = Bands { ll, hl, lh, hh };
  let rect = decomposition.rect;

  dispatch!(Level::new(), simd => {
    interleave(simd, bands, output, rect)?;
    filter_horizontal(output, rect, transform)?;
    filter_vertical(simd, output, rect, transform)
  })
}

#[inline(always)]
fn interleave<S: Simd>(
  simd: S,
  bands: Bands<'_>,
  output: &mut Vec<f32>,
  rect: IntRect,
) -> Result<()> {
  let Ok(width) = rect.width();
  let Ok(height) = rect.height();
  let length = u64::from(width).wrapping_mul(u64::from(height));
  let Ok(ll) = fixed_index(bands.ll.len());
  let Ok(hl) = fixed_index(bands.hl.len());
  let Ok(lh) = fixed_index(bands.lh.len());
  let Ok(hh) = fixed_index(bands.hh.len());
  let held = ll.saturating_add(hl).saturating_add(lh).saturating_add(hh);

  match length.cmp(&held) {
    core::cmp::Ordering::Equal => {}
    core::cmp::Ordering::Less | core::cmp::Ordering::Greater => {
      return Err(crate::DecodeError::Decoding(DecodingError::InvalidPrecinct));
    }
  }

  crate::resize_decoder_buffer(output, held, crate::DecoderBuffer::Block, 0.0)?;

  let Ok(width_index) = index(width);
  let low_width = rect.x1.div_ceil(2).wrapping_sub(rect.x0.div_ceil(2));
  let high_width = width.wrapping_sub(low_width);
  let (mut low_y, mut high_y) =
    match ::core::primitive::bool::then_some(rect.x0.is_multiple_of(2), ()) {
      ::core::option::Option::Some(()) => (
        BandRows {
          first: bands.ll,
          second: bands.hl,
          widths: BandWidths {
            first: low_width,
            second: high_width,
          },
        },
        BandRows {
          first: bands.lh,
          second: bands.hh,
          widths: BandWidths {
            first: low_width,
            second: high_width,
          },
        },
      ),
      ::core::option::Option::None => (
        BandRows {
          first: bands.hl,
          second: bands.ll,
          widths: BandWidths {
            first: high_width,
            second: low_width,
          },
        },
        BandRows {
          first: bands.hh,
          second: bands.lh,
          widths: BandWidths {
            first: high_width,
            second: low_width,
          },
        },
      ),
    };
  let mut rows = match ::core::primitive::bool::then_some(rect.y0.is_multiple_of(2), ()) {
    ::core::option::Option::Some(()) => [&mut low_y, &mut high_y],
    ::core::option::Option::None => [&mut high_y, &mut low_y],
  };

  for out_row in output.chunks_exact_mut(width_index.max(1)) {
    let [current, next] = &mut rows;
    let (first, second) = current.next()?;
    interleave_row(simd, [first, second], out_row)?;
    core::mem::swap(current, next);
  }

  Ok(())
}

struct BandWidths {
  first: u32,
  second: u32,
}

struct BandRows<'a> {
  first: &'a [f32],
  second: &'a [f32],
  widths: BandWidths,
}

impl<'a> BandRows<'a> {
  #[must_use]
  fn next(&mut self) -> Result<(&'a [f32], &'a [f32])> {
    let Ok(first_width) = wrapped_index(self.widths.first);
    let Ok(second_width) = wrapped_index(self.widths.second);
    let (first, first_rest) = self
      .first
      .split_at_checked(first_width)
      .ok_or(DecodingError::InvalidPrecinct)?;
    let (second, second_rest) = self
      .second
      .split_at_checked(second_width)
      .ok_or(DecodingError::InvalidPrecinct)?;
    self.first = first_rest;
    self.second = second_rest;
    Ok((first, second))
  }
}

#[inline(always)]
fn interleave_row<S: Simd>(
  simd: S,
  [first, second]: [&[f32]; 2],
  output: &mut [f32],
) -> Result<()> {
  let Ok(pair_count) = fixed_index(first.len().min(second.len()));
  let Ok(lanes) = fixed_index(SIMD_WIDTH);
  let full_pairs = pair_count.wrapping_div(lanes.max(1)).wrapping_mul(lanes);
  let Ok(full_width) = index(full_pairs);
  let Ok(full_output_width) = index(full_pairs.wrapping_mul(2));
  let (first_simd, first_tail) = first
    .split_at_checked(full_width)
    .ok_or(DecodingError::InvalidPrecinct)?;
  let (second_simd, second_tail) = second
    .split_at_checked(full_width)
    .ok_or(DecodingError::InvalidPrecinct)?;
  let (simd_output, scalar_output) = output
    .split_at_mut_checked(full_output_width)
    .ok_or(DecodingError::InvalidPrecinct)?;
  let (first_chunks, _) = first_simd.as_chunks::<SIMD_WIDTH>();
  let (second_chunks, _) = second_simd.as_chunks::<SIMD_WIDTH>();
  let (output_chunks, _) = simd_output.as_chunks_mut::<{ SIMD_WIDTH * 2 }>();

  for ((first_chunk, second_chunk), out_chunk) in
    first_chunks.iter().zip(second_chunks).zip(output_chunks)
  {
    let Ok(a) = Float32x8::from_slice(simd, first_chunk);
    let Ok(b) = Float32x8::from_slice(simd, second_chunk);
    let (halves, _) = out_chunk.as_chunks_mut::<SIMD_WIDTH>();
    let (low, high) = match halves {
      [low, high] => (low, high),
      _ => {
        return Err(crate::error::DecodeError::Decoding(
          DecodingError::InvalidPrecinct,
        ));
      }
    };
    let Ok(low_values) = a.zip_low(b);
    let Ok(high_values) = a.zip_high(b);
    let Ok(()) = low_values.store(low);
    let Ok(()) = high_values.store(high);
  }

  let (pairs, last) = scalar_output.as_chunks_mut::<2>();

  for ((a, b), [left, right]) in first_tail.iter().zip(second_tail).zip(pairs.iter_mut()) {
    *left = *a;
    *right = *b;
  }

  match last {
    [last_sample] => {
      let source = first_tail.last().ok_or(DecodingError::InvalidPrecinct)?;
      *last_sample = *source;
    }
    _ => {}
  }

  Ok(())
}

const NEG_ALPHA: f32 = 1.586_134_3;
const NEG_BETA: f32 = 0.052_980_117;
const NEG_GAMMA: f32 = -0.882_911_1;
const NEG_DELTA: f32 = -0.443_506_87;
const KAPPA: f32 = 1.230_174_1;
const INV_KAPPA: f32 = 1.0 / KAPPA;

#[derive(Clone, Copy)]
enum Lifting {
  ReversibleUpdate,
  ReversiblePredict,
  Irreversible(f32),
}

struct Neighbors<T> {
  current: T,
  before: T,
  after: T,
}

impl Lifting {
  #[inline(always)]
  fn scalar(self, samples: Neighbors<f32>) -> core::result::Result<f32, Never> {
    let Neighbors {
      current,
      before,
      after,
    } = samples;
    let sum = before + after;

    match self {
      Self::ReversibleUpdate => {
        let Ok(update) = math::mul_add(math::MultiplyAdd {
          value: math::Sample(sum),
          factor: math::Gain(0.25),
          offset: math::Sample(0.5),
        });
        let Ok(rounded) = math::floor_f32(math::Sample(update));
        Ok(current - rounded)
      }
      Self::ReversiblePredict => {
        let Ok(rounded) = math::floor_f32(math::Sample(sum * 0.5));
        Ok(current + rounded)
      }
      Self::Irreversible(weight) => math::mul_add(math::MultiplyAdd {
        value: math::Sample(sum),
        factor: math::Gain(weight),
        offset: math::Sample(current),
      }),
    }
  }

  #[inline(always)]
  fn lanes<S: Simd>(
    self,
    simd: S,
    samples: Neighbors<Float32x8<S>>,
  ) -> core::result::Result<Float32x8<S>, Never> {
    let Neighbors {
      current,
      before,
      after,
    } = samples;
    let sum = before + after;

    match self {
      Self::ReversibleUpdate => {
        let Ok(rounded) = ((sum + 2.0) * 0.25).floor();
        Ok(current - rounded)
      }
      Self::ReversiblePredict => {
        let Ok(rounded) = (sum * 0.5).floor();
        Ok(current + rounded)
      }
      Self::Irreversible(weight) => {
        let Ok(weights) = Float32x8::splat(simd, crate::math::Sample(weight));
        sum.mul_add(weights, current)
      }
    }
  }
}

fn filter_horizontal(
  coefficients: &mut [f32],
  rect: IntRect,
  transform: WaveletTransform,
) -> Result<()> {
  let Ok(width) = rect.width();
  let Ok(native_width) = index(width);

  for scanline in coefficients.chunks_exact_mut(native_width.max(1)) {
    filter_row(scanline, rect.x0, transform)?;
  }

  Ok(())
}

fn filter_row(scanline: &mut [f32], x0: u32, transform: WaveletTransform) -> Result<()> {
  let first_even = x0.wrapping_rem(2);
  let first_odd = 1u32.wrapping_sub(first_even);

  match scanline {
    [sample] => {
      match first_even {
        1 => *sample *= 0.5,
        _ => {}
      }

      return Ok(());
    }
    _ => {}
  }

  match transform {
    WaveletTransform::Reversible53 => {
      lift_row(scanline, first_even, Lifting::ReversibleUpdate)?;
      lift_row(scanline, first_odd, Lifting::ReversiblePredict)?;
    }
    WaveletTransform::Irreversible97 => {
      let (first_gain, second_gain) = match first_even {
        0 => (KAPPA, INV_KAPPA),
        _ => (INV_KAPPA, KAPPA),
      };
      let (pairs, trailing) = scanline.as_chunks_mut::<2>();

      for [first, second] in pairs {
        *first *= first_gain;
        *second *= second_gain;
      }

      for sample in trailing {
        *sample *= first_gain;
      }

      lift_row(scanline, first_even, Lifting::Irreversible(NEG_DELTA))?;
      lift_row(scanline, first_odd, Lifting::Irreversible(NEG_GAMMA))?;
      lift_row(scanline, first_even, Lifting::Irreversible(NEG_BETA))?;
      lift_row(scanline, first_odd, Lifting::Irreversible(NEG_ALPHA))?;
    }
  }

  Ok(())
}

#[inline(always)]
fn lift_row(scanline: &mut [f32], parity: u32, step: Lifting) -> Result<()> {
  let Ok(length) = fixed_index(scanline.len());
  let Ok(reflection_limit) = fixed_index(scanline.len().saturating_sub(2));

  for sample_index in (u64::from(parity)..length).step_by(2) {
    let Ok(current) = index(sample_index);
    let Ok(before) = index(sample_index.abs_diff(1));
    let after_index = sample_index.wrapping_add(1);
    let Ok(after) = index(match after_index.cmp(&length) {
      core::cmp::Ordering::Less => after_index,
      core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => reflection_limit,
    });
    let a = ::core::result::Result::<&_, _>::copied(
      scanline.get(before).ok_or(DecodingError::InvalidPrecinct),
    )?;
    let b = ::core::result::Result::<&_, _>::copied(
      scanline.get(after).ok_or(DecodingError::InvalidPrecinct),
    )?;
    let sample = scanline
      .get_mut(current)
      .ok_or(DecodingError::InvalidPrecinct)?;
    let Ok(lifted) = step.scalar(Neighbors {
      current: *sample,
      before: a,
      after: b,
    });
    *sample = lifted;
  }

  Ok(())
}

#[inline(always)]
fn filter_vertical<S: Simd>(
  simd: S,
  coefficients: &mut [f32],
  rect: IntRect,
  transform: WaveletTransform,
) -> Result<()> {
  let Ok(width) = rect.width();
  let Ok(height) = rect.height();
  let parity = rect.y0.wrapping_rem(2);

  match (width, height, parity) {
    (0, _, _) | (_, 0, _) | (_, 1, 0) => return Ok(()),
    (_, 1, _) => {
      for sample in coefficients {
        *sample *= 0.5;
      }

      return Ok(());
    }
    _ => {}
  }

  let shape = Size { width, height };
  let opposite = 1u32.wrapping_sub(parity);

  match transform {
    WaveletTransform::Reversible53 => {
      lift_columns(simd, coefficients, shape, parity, Lifting::ReversibleUpdate)?;
      lift_columns(
        simd,
        coefficients,
        shape,
        opposite,
        Lifting::ReversiblePredict,
      )?;
    }
    WaveletTransform::Irreversible97 => {
      let Ok(native_width) = wrapped_index(width);
      let mut gain = match parity {
        0 => [KAPPA, INV_KAPPA],
        _ => [INV_KAPPA, KAPPA],
      };

      for line in coefficients.chunks_exact_mut(native_width.max(1)) {
        let [factor, _] = gain;

        for sample in line {
          *sample *= factor;
        }

        gain.reverse();
      }

      lift_columns(
        simd,
        coefficients,
        shape,
        parity,
        Lifting::Irreversible(NEG_DELTA),
      )?;
      lift_columns(
        simd,
        coefficients,
        shape,
        opposite,
        Lifting::Irreversible(NEG_GAMMA),
      )?;
      lift_columns(
        simd,
        coefficients,
        shape,
        parity,
        Lifting::Irreversible(NEG_BETA),
      )?;
      lift_columns(
        simd,
        coefficients,
        shape,
        opposite,
        Lifting::Irreversible(NEG_ALPHA),
      )?;
    }
  }

  Ok(())
}

#[inline(always)]
fn lift_columns<S: Simd>(
  simd: S,
  coefficients: &mut [f32],
  raster: Size<u32>,
  parity: u32,
  step: Lifting,
) -> Result<()> {
  let Ok(width) = wrapped_index(raster.width);

  for row in (parity..raster.height).step_by(2) {
    let offset = u64::from(row).wrapping_mul(u64::from(raster.width));
    let Ok(start) = index(offset);
    let (above, remaining) = coefficients
      .split_at_mut_checked(start)
      .ok_or(DecodingError::InvalidPrecinct)?;
    let (current, below) = remaining
      .split_at_mut_checked(width)
      .ok_or(DecodingError::InvalidPrecinct)?;
    let previous = match above.len().checked_sub(width) {
      Some(row_start) => above.get(row_start..),
      None => None,
    };
    let following = below.get(..width);
    let (before, after) = match (previous, following) {
      (Some(before), Some(after)) => (&*before, &*after),
      (Some(before), None) => (&*before, &*before),
      (None, Some(after)) => (&*after, &*after),
      (None, None) => {
        return Err(crate::error::DecodeError::Decoding(
          DecodingError::InvalidPrecinct,
        ));
      }
    };
    let Ok(()) = lift_column_row(
      simd,
      step,
      RowNeighbors {
        current,
        before,
        after,
      },
    );
  }

  Ok(())
}

#[inline(always)]
fn lift_column_row<S: Simd>(
  simd: S,
  step: Lifting,
  rows: RowNeighbors<'_>,
) -> core::result::Result<(), Never> {
  let RowNeighbors {
    current,
    before,
    after,
  } = rows;
  let (current_chunks, current_tail) = current.as_chunks_mut::<SIMD_WIDTH>();
  let (before_chunks, before_tail) = before.as_chunks::<SIMD_WIDTH>();
  let (after_chunks, after_tail) = after.as_chunks::<SIMD_WIDTH>();

  for ((out, a), b) in current_chunks
    .iter_mut()
    .zip(before_chunks)
    .zip(after_chunks)
  {
    let Ok(current_2) = Float32x8::from_slice(simd, out);
    let Ok(before_2) = Float32x8::from_slice(simd, a);
    let Ok(after_2) = Float32x8::from_slice(simd, b);
    let Ok(value) = step.lanes(
      simd,
      Neighbors {
        current: current_2,
        before: before_2,
        after: after_2,
      },
    );
    let Ok(()) = value.store(out);
  }

  for ((out, before_3), after_3) in current_tail.iter_mut().zip(before_tail).zip(after_tail) {
    let Ok(value) = step.scalar(Neighbors {
      current: *out,
      before: *before_3,
      after: *after_3,
    });
    *out = value;
  }

  Ok(())
}

struct RowNeighbors<'a> {
  current: &'a mut [f32],
  before: &'a [f32],
  after: &'a [f32],
}
