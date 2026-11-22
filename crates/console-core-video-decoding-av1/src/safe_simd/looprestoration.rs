//! Safe SIMD implementations for Loop Restoration
#![cfg_attr(not(feature = "unchecked"), forbid(unsafe_code))]
#![cfg_attr(feature = "unchecked", deny(unsafe_code))]
//!
//! Loop restoration applies two types of filtering:
//! 1. Wiener filter - 7-tap or 5-tap separable filter
//! 2. SGR (Self-Guided Restoration) - guided filter based on local statistics

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, fixed_offset, index};

#[cfg(target_arch = "x86_64")]
use console_core_pixel_buffers::{RowLayout, RowLayoutError};
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

#[cfg(all(test, target_arch = "x86_64"))]
use crate::src::cpu::summon_avx2;
use archmage::{Desktop64, Server64, arcane, rite};
use std::ffi::c_int;

#[cfg(target_arch = "x86_64")]
use crate::src::safe_simd::partial_simd;
#[cfg(target_arch = "x86_64")]
use crate::src::safe_simd::pixel_access::{
  loadu_64, loadu_128, loadu_256, loadu_512, storeu_128, storeu_256, storeu_512,
};

use crate::include::common::bitdepth::{AsPrimitive, BitDepth};
use crate::include::common::bitdepth::BitDepth8;
use crate::include::common::bitdepth::BitDepth16;
use crate::include::common::intops::iclip;
use crate::include::dav1d::picture::PicOffset;
use crate::src::align::aligned_inner;
#[cfg(all(test, target_arch = "x86_64"))]
use crate::src::{align::AlignedVec64, disjoint_mut::DisjointMut};
use crate::src::error::{Rav1dError, Rav1dResult};
use crate::src::looprestoration::{RESTORATION_INTERIOR, RestorationDeltas, RestorationMoments, RestorationPixels, RestorationRow, restoration_rows, restoration_rows_mut, REST_UNIT_STRIDE, LooprestorationParams, RestorationDispatch, RestorationKernel, RestorationPadding, RestorationRequest, BoxRadius, SgrSelection, SgrStencil, SGR_STRIDE, padding};
#[cfg(all(test, target_arch = "x86_64"))]
use crate::src::looprestoration::{LrEdgeFlags, RestorationInteriorRow};
use crate::src::tables::dav1d_sgr_x_by_x;

#[cfg(target_arch = "x86_64")]
#[inline]
fn wiener_row_blocks<'a, T, const LANES: usize>(
  rows: &mut [&'a [T]; 7],
) -> Result<ZipTaps<core::slice::Iter<'a, [T; LANES]>, 7>, Never> {
  let blocks = rows.each_mut().map(|row| {
    let (vectors, tail) = core::mem::take(row).as_chunks::<LANES>();
    *row = tail;
    vectors.iter()
  });

  Ok(ZipTaps { taps: blocks })
}

#[cfg(target_arch = "x86_64")]
struct ZipTaps<I, const TAPS: usize> {
  taps: [I; TAPS],
}

#[cfg(target_arch = "x86_64")]
impl<I: Iterator, const TAPS: usize> Iterator for ZipTaps<I, TAPS>
where
  I::Item: Copy,
{
  type Item = [I::Item; TAPS];

  fn next(&mut self) -> Option<Self::Item> {
    let (first, rest) = self.taps.split_first_mut()?;
    let sample = first.next()?;
    let mut window = [sample; TAPS];

    for (target, tap) in window.iter_mut().skip(1).zip(rest) {
      let tap_next = tap.next()?;
      *target = tap_next;
    }

    Some(window)
  }
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn tapped_blocks<T, const LANES: usize, const TAPS: usize>(
  source: &[T],
) -> Result<ZipTaps<core::slice::Iter<'_, [T; LANES]>, TAPS>, Never> {
  let taps = core::array::from_fn(|tap| {
    let remaining = match source.get(tap..) {
      Some(samples) => samples,
      None => &[],
    };
    let (vectors, _tail) = remaining.as_chunks();
    vectors.iter()
  });
  Ok(ZipTaps { taps })
}

#[cfg(target_arch = "x86_64")]
struct WienerInputWindow {
  row: u64,
  width: u64,
}

#[cfg(target_arch = "x86_64")]
fn wiener_inputs<T, const COLUMNS: usize>(
  source: &[[T; COLUMNS]],
  window: WienerInputWindow,
) -> Result<[&[T]; 7], RowLayoutError> {
  let Ok(row) = index(window.row);
  let Ok(width) = index(window.width);
  let neighbors = source.get(row..).and_then(<[_]>::first_chunk::<7>).ok_or(RowLayoutError::Bounds)?;

  match neighbors.each_ref().map(|samples| samples.get(..width)) {
    [Some(a), Some(b), Some(c), Some(d), Some(e), Some(f), Some(g)] => Ok([a, b, c, d, e, f, g]),
    _incomplete => Err(RowLayoutError::Bounds),
  }
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn wiener_filter7_8bpc_avx2_inner(
  _token: Desktop64,
  request: RestorationPadding<'_, BitDepth8>,
  params: &LooprestorationParams,
) -> Result<(), RowLayoutError> {
  let extent = request.size;
  let destination = request.picture;
  let maximum_width = u64::from(RESTORATION_INTERIOR);

  match ((1..=maximum_width).contains(&extent.width) && (1..=64).contains(&extent.height)).then_some(()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(RowLayoutError::Bounds),
  }

  let mut tmp: RestorationPixels<_> = [0u8; _];

  padding::<BitDepth8>(&mut tmp, request).map_err(|_padding| RowLayoutError::Bounds)?;

  let mut hor: RestorationPixels<_> = [0u16; _];

  let [horizontal_filter, vertical_filter] = &params.filter;
  let round_bits_h = 3i32;
  let rounding_off_h = ::core::primitive::i32::wrapping_shl(
    1i32,
    ::core::primitive::i32::unsigned_abs(::core::primitive::i32::wrapping_sub(round_bits_h, 1)),
  );
  let clip_limit = ::core::primitive::i32::wrapping_shl(
    1i32,
    ::core::primitive::i32::unsigned_abs(::core::primitive::i32::wrapping_sub(
      ::core::primitive::i32::wrapping_add(::core::primitive::i32::wrapping_add(8, 1), 7),
      round_bits_h,
    )),
  );

  let [f0, f1, f2, f3, f4, f5, f6, _] = horizontal_filter.map(i32::from);
  let [hf0, hf1, hf2, hf3, hf4, hf5, hf6] = [f0, f1, f2, f3.wrapping_add(128), f4, f5, f6]
    .map(|coefficient| _mm256_set1_epi32(coefficient));
  let dc_offset = _mm256_set1_epi32(::core::primitive::i32::wrapping_shl(1i32, 14_u32));
  let h_rounding = _mm256_set1_epi32(rounding_off_h);
  let h_clip_max = _mm256_set1_epi32(i32::wrapping_sub(clip_limit, 1));
  let h_zero = _mm256_setzero_si256();

  let Ok(padded) = restoration_rows(&tmp);
  let Ok(intermediate) = restoration_rows_mut(&mut hor);

  let Ok(rows) = index(extent.height.wrapping_add(6));
  let Ok(columns) = index(extent.width);

  for (src, row) in padded.iter().zip(intermediate).take(rows) {
    let row_7 = row.get_mut(..columns).ok_or(RowLayoutError::Bounds)?;
    let (blocks, tail) = row_7.as_chunks_mut::<8>();
    let Ok(tail_length) = fixed_index(tail.len());
    let Ok(tail_start) = index(extent.width.wrapping_sub(tail_length));
    let Ok(windows) = tapped_blocks::<_, 8, 7>(src);

    for (
      target,
      [
        sample_0,
        sample_1,
        sample_2,
        sample_3,
        sample_4,
        sample_5,
        sample_6,
      ],
    ) in blocks.iter_mut().zip(windows)
    {
      let p0 = _mm256_cvtepu8_epi32(loadu_64!(sample_0));
      let p1 = _mm256_cvtepu8_epi32(loadu_64!(sample_1));
      let p2 = _mm256_cvtepu8_epi32(loadu_64!(sample_2));
      let p3 = _mm256_cvtepu8_epi32(loadu_64!(sample_3));
      let p4 = _mm256_cvtepu8_epi32(loadu_64!(sample_4));
      let p5 = _mm256_cvtepu8_epi32(loadu_64!(sample_5));
      let p6 = _mm256_cvtepu8_epi32(loadu_64!(sample_6));

      let mut sum = dc_offset;
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p0, hf0));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p1, hf1));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p2, hf2));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p3, hf3));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p4, hf4));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p5, hf5));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p6, hf6));

      sum = _mm256_add_epi32(sum, h_rounding);
      sum = _mm256_srai_epi32::<3>(sum);
      sum = _mm256_max_epi32(sum, h_zero);
      sum = _mm256_min_epi32(sum, h_clip_max);

      let packed = _mm256_packus_epi32(sum, sum);
      let lo = _mm256_castsi256_si128(packed);
      let hi = _mm256_extracti128_si256(packed, 1);
      let combined = _mm_unpacklo_epi64(lo, hi);

      storeu_128!(target, combined);
    }

    for (target, samples) in tail
      .iter_mut()
      .zip(src.array_windows::<7>().skip(tail_start))
    {
      let mut sum = ::core::primitive::i32::wrapping_shl(1i32, 14_u32);
      let [_, _, _, center, ..] = samples;
      sum = sum.wrapping_add(i32::from(*center).wrapping_mul(128));

      for (sample, coefficient) in samples.iter().zip(horizontal_filter) {
        sum = sum.wrapping_add(i32::from(*sample).wrapping_mul(i32::from(*coefficient)));
      }

      *target =
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(iclip(
          ::core::primitive::i32::wrapping_shr(
            i32::wrapping_add(sum, rounding_off_h),
            ::core::primitive::i32::unsigned_abs(round_bits_h),
          ),
          0,
          i32::wrapping_sub(clip_limit, 1),
        )) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
    }
  }

  let round_bits_v = 11i32;
  let rounding_off_v = ::core::primitive::i32::wrapping_shl(
    1i32,
    ::core::primitive::i32::unsigned_abs(::core::primitive::i32::wrapping_sub(round_bits_v, 1)),
  );
  let round_offset = ::core::primitive::i32::wrapping_shl(
    1i32,
    ::core::primitive::i32::unsigned_abs(::core::primitive::i32::wrapping_sub(
      ::core::primitive::i32::wrapping_add(8, round_bits_v),
      1,
    )),
  );

  let [vf0, vf1, vf2, vf3, vf4, vf5, vf6, _] =
    vertical_filter.map(|coefficient| _mm256_set1_epi32(i32::from(coefficient)));
  let v_round_offset = _mm256_set1_epi32(i32::wrapping_neg(round_offset));
  let v_rounding = _mm256_set1_epi32(rounding_off_v);

  let Ok(intermediate_2) = restoration_rows(&hor);
  let mut intermediate_rows = intermediate_2;
  destination.checked_for_rows_mut_with_context::<BitDepth8, _>(extent, &mut intermediate_rows, |row, output, scratch_rows| {
    let mut inputs = wiener_inputs(scratch_rows, WienerInputWindow { row, width: extent.width })?;


    let (blocks_8, tail_8) = output.as_chunks_mut::<8>();
    let Ok(columns_8) = wiener_row_blocks::<_, 8>(&mut inputs);

    for (
      target,
      [
        input_0,
        input_1,
        input_2,
        input_3,
        input_4,
        input_5,
        input_6,
      ],
    ) in blocks_8.iter_mut().zip(columns_8)
    {
      let r0 = loadu_128!(input_0);
      let r1 = loadu_128!(input_1);
      let r2 = loadu_128!(input_2);
      let r3 = loadu_128!(input_3);
      let r4 = loadu_128!(input_4);
      let r5 = loadu_128!(input_5);
      let r6 = loadu_128!(input_6);

      let r0_lo = _mm256_cvtepu16_epi32(r0);
      let r1_lo = _mm256_cvtepu16_epi32(r1);
      let r2_lo = _mm256_cvtepu16_epi32(r2);
      let r3_lo = _mm256_cvtepu16_epi32(r3);
      let r4_lo = _mm256_cvtepu16_epi32(r4);
      let r5_lo = _mm256_cvtepu16_epi32(r5);
      let r6_lo = _mm256_cvtepu16_epi32(r6);

      let mut sum = v_round_offset;
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(r0_lo, vf0));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(r1_lo, vf1));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(r2_lo, vf2));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(r3_lo, vf3));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(r4_lo, vf4));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(r5_lo, vf5));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(r6_lo, vf6));

      sum = _mm256_add_epi32(sum, v_rounding);
      sum = _mm256_srai_epi32::<11>(sum);

      let sum16 = _mm256_packus_epi32(sum, sum);
      let sum16_lo = _mm256_castsi256_si128(sum16);
      let sum16_hi = _mm256_extracti128_si256(sum16, 1);
      let sum16_combined = _mm_unpacklo_epi64(sum16_lo, sum16_hi);
      let sum8 = _mm_packus_epi16(sum16_combined, sum16_combined);
      let ::core::result::Result::Ok(()) = partial_simd::mm_storel_epi64(target, sum8);
    }

    let Ok(single_pixels) = wiener_row_blocks::<_, 1>(&mut inputs);

    for (pixel, samples) in tail_8.iter_mut().zip(single_pixels) {
      let mut sum = i32::wrapping_neg(round_offset);

      for ([sample], coefficient) in samples.into_iter().zip(vertical_filter) {
        sum = sum.wrapping_add(i32::from(*sample).wrapping_mul(i32::from(*coefficient)));
      }

      *pixel = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(iclip(
        ::core::primitive::i32::wrapping_shr(
          i32::wrapping_add(sum, rounding_off_v),
          ::core::primitive::i32::unsigned_abs(round_bits_v),
        ),
        0,
        255,
      )) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    }

    Ok(())
  })
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn wiener_filter7_8bpc_avx512_inner(
  _token: Server64,
  request: RestorationPadding<'_, BitDepth8>,
  params: &LooprestorationParams,
) -> Result<(), RowLayoutError> {
  let extent = request.size;
  let destination = request.picture;
  let maximum_width = u64::from(RESTORATION_INTERIOR);

  match ((1..=maximum_width).contains(&extent.width) && (1..=64).contains(&extent.height)).then_some(()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(RowLayoutError::Bounds),
  }

  let mut tmp: RestorationPixels<_> = [0u8; _];
  padding::<BitDepth8>(&mut tmp, request).map_err(|_padding| RowLayoutError::Bounds)?;

  let mut hor: RestorationPixels<_> = [0u16; _];

  let [horizontal_filter, vertical_filter] = &params.filter;
  let round_bits_h = 3i32;
  let rounding_off_h = ::core::primitive::i32::wrapping_shl(
    1i32,
    ::core::primitive::i32::unsigned_abs(::core::primitive::i32::wrapping_sub(round_bits_h, 1)),
  );
  let clip_limit = ::core::primitive::i32::wrapping_shl(
    1i32,
    ::core::primitive::i32::unsigned_abs(::core::primitive::i32::wrapping_sub(
      ::core::primitive::i32::wrapping_add(::core::primitive::i32::wrapping_add(8, 1), 7),
      round_bits_h,
    )),
  );

  let [f0, f1, f2, f3, f4, f5, f6, _] = horizontal_filter.map(i32::from);
  let [hf0, hf1, hf2, hf3, hf4, hf5, hf6] = [f0, f1, f2, f3.wrapping_add(128), f4, f5, f6]
    .map(|coefficient| _mm512_set1_epi32(coefficient));
  let dc_offset = _mm512_set1_epi32(::core::primitive::i32::wrapping_shl(1i32, 14_u32));
  let h_rounding = _mm512_set1_epi32(rounding_off_h);
  let h_clip_max = _mm512_set1_epi32(i32::wrapping_sub(clip_limit, 1));
  let h_zero = _mm512_setzero_si512();

  let Ok(padded) = restoration_rows(&tmp);
  let Ok(intermediate) = restoration_rows_mut(&mut hor);

  let Ok(rows) = index(extent.height.wrapping_add(6));
  let Ok(columns) = index(extent.width);

  for (src, row) in padded.iter().zip(intermediate).take(rows) {
    let row_7 = row.get_mut(..columns).ok_or(RowLayoutError::Bounds)?;
    let (blocks, tail) = row_7.as_chunks_mut::<16>();
    let Ok(tail_length) = fixed_index(tail.len());
    let Ok(tail_start) = index(extent.width.wrapping_sub(tail_length));
    let Ok(windows) = tapped_blocks::<_, 16, 7>(src);

    for (
      target,
      [
        sample_0,
        sample_1,
        sample_2,
        sample_3,
        sample_4,
        sample_5,
        sample_6,
      ],
    ) in blocks.iter_mut().zip(windows)
    {
      let p0 = _mm512_cvtepu8_epi32(loadu_128!(sample_0));
      let p1 = _mm512_cvtepu8_epi32(loadu_128!(sample_1));
      let p2 = _mm512_cvtepu8_epi32(loadu_128!(sample_2));
      let p3 = _mm512_cvtepu8_epi32(loadu_128!(sample_3));
      let p4 = _mm512_cvtepu8_epi32(loadu_128!(sample_4));
      let p5 = _mm512_cvtepu8_epi32(loadu_128!(sample_5));
      let p6 = _mm512_cvtepu8_epi32(loadu_128!(sample_6));

      let mut sum = dc_offset;
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p0, hf0));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p1, hf1));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p2, hf2));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p3, hf3));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p4, hf4));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p5, hf5));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p6, hf6));

      sum = _mm512_add_epi32(sum, h_rounding);
      sum = _mm512_srai_epi32::<3>(sum);
      sum = _mm512_max_epi32(sum, h_zero);
      sum = _mm512_min_epi32(sum, h_clip_max);

      let result_u16: __m256i = _mm512_cvtusepi32_epi16(sum);
      storeu_256!(target, result_u16);
    }

    for (target, samples) in tail
      .iter_mut()
      .zip(src.array_windows::<7>().skip(tail_start))
    {
      let mut sum = ::core::primitive::i32::wrapping_shl(1i32, 14_u32);
      let [_, _, _, center, ..] = samples;
      sum = sum.wrapping_add(i32::from(*center).wrapping_mul(128));

      for (sample, coefficient) in samples.iter().zip(horizontal_filter) {
        sum = sum.wrapping_add(i32::from(*sample).wrapping_mul(i32::from(*coefficient)));
      }

      *target =
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(iclip(
          ::core::primitive::i32::wrapping_shr(
            i32::wrapping_add(sum, rounding_off_h),
            ::core::primitive::i32::unsigned_abs(round_bits_h),
          ),
          0,
          i32::wrapping_sub(clip_limit, 1),
        )) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
    }
  }

  let round_bits_v = 11i32;
  let rounding_off_v = ::core::primitive::i32::wrapping_shl(
    1i32,
    ::core::primitive::i32::unsigned_abs(::core::primitive::i32::wrapping_sub(round_bits_v, 1)),
  );
  let round_offset = ::core::primitive::i32::wrapping_shl(
    1i32,
    ::core::primitive::i32::unsigned_abs(::core::primitive::i32::wrapping_sub(
      ::core::primitive::i32::wrapping_add(8, round_bits_v),
      1,
    )),
  );
  let [vf0, vf1, vf2, vf3, vf4, vf5, vf6, _] =
    vertical_filter.map(|coefficient| _mm512_set1_epi32(i32::from(coefficient)));
  let v_round_offset = _mm512_set1_epi32(i32::wrapping_neg(round_offset));
  let v_rounding = _mm512_set1_epi32(rounding_off_v);
  let zero_512 = _mm512_setzero_si512();

  let Ok(intermediate_2) = restoration_rows(&hor);
  let mut intermediate_rows = intermediate_2;
  destination.checked_for_rows_mut_with_context::<BitDepth8, _>(extent, &mut intermediate_rows, |row, output, scratch_rows| {
    let mut inputs = wiener_inputs(scratch_rows, WienerInputWindow { row, width: extent.width })?;


    let (blocks_16, tail_16) = output.as_chunks_mut::<16>();
    let Ok(columns_16) = wiener_row_blocks::<_, 16>(&mut inputs);

    for (
      target,
      [
        input_0,
        input_1,
        input_2,
        input_3,
        input_4,
        input_5,
        input_6,
      ],
    ) in blocks_16.iter_mut().zip(columns_16)
    {
      let r0 = _mm512_cvtepu16_epi32(loadu_256!(input_0));
      let r1 = _mm512_cvtepu16_epi32(loadu_256!(input_1));
      let r2 = _mm512_cvtepu16_epi32(loadu_256!(input_2));
      let r3 = _mm512_cvtepu16_epi32(loadu_256!(input_3));
      let r4 = _mm512_cvtepu16_epi32(loadu_256!(input_4));
      let r5 = _mm512_cvtepu16_epi32(loadu_256!(input_5));
      let r6 = _mm512_cvtepu16_epi32(loadu_256!(input_6));

      let mut sum = v_round_offset;
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r0, vf0));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r1, vf1));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r2, vf2));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r3, vf3));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r4, vf4));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r5, vf5));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r6, vf6));

      sum = _mm512_add_epi32(sum, v_rounding);
      sum = _mm512_srai_epi32::<11>(sum);

      let clamped = _mm512_max_epi32(sum, zero_512);
      let result_u8: __m128i = _mm512_cvtusepi32_epi8(clamped);

      storeu_128!(target, result_u8);
    }

    let (blocks_8, tail_8) = tail_16.as_chunks_mut::<8>();
    let Ok(columns_8) = wiener_row_blocks::<_, 8>(&mut inputs);

    for (
      target,
      [
        input_0,
        input_1,
        input_2,
        input_3,
        input_4,
        input_5,
        input_6,
      ],
    ) in blocks_8.iter_mut().zip(columns_8)
    {
      let r0 = loadu_128!(input_0);
      let r1 = loadu_128!(input_1);
      let r2 = loadu_128!(input_2);
      let r3 = loadu_128!(input_3);
      let r4 = loadu_128!(input_4);
      let r5 = loadu_128!(input_5);
      let r6 = loadu_128!(input_6);

      let [
        vf0_256,
        vf1_256,
        vf2_256,
        vf3_256,
        vf4_256,
        vf5_256,
        vf6_256,
        _,
      ] = vertical_filter.map(|coefficient| _mm256_set1_epi32(i32::from(coefficient)));

      let r0_lo = _mm256_cvtepu16_epi32(r0);
      let r1_lo = _mm256_cvtepu16_epi32(r1);
      let r2_lo = _mm256_cvtepu16_epi32(r2);
      let r3_lo = _mm256_cvtepu16_epi32(r3);
      let r4_lo = _mm256_cvtepu16_epi32(r4);
      let r5_lo = _mm256_cvtepu16_epi32(r5);
      let r6_lo = _mm256_cvtepu16_epi32(r6);

      let mut sum = _mm256_set1_epi32(i32::wrapping_neg(round_offset));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(r0_lo, vf0_256));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(r1_lo, vf1_256));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(r2_lo, vf2_256));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(r3_lo, vf3_256));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(r4_lo, vf4_256));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(r5_lo, vf5_256));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(r6_lo, vf6_256));
      sum = _mm256_add_epi32(sum, _mm256_set1_epi32(rounding_off_v));
      sum = _mm256_srai_epi32::<11>(sum);

      let sum16 = _mm256_packus_epi32(sum, sum);
      let sum16_lo = _mm256_castsi256_si128(sum16);
      let sum16_hi = _mm256_extracti128_si256(sum16, 1);
      let sum16_combined = _mm_unpacklo_epi64(sum16_lo, sum16_hi);
      let sum8 = _mm_packus_epi16(sum16_combined, sum16_combined);
      let ::core::result::Result::Ok(()) = partial_simd::mm_storel_epi64(target, sum8);
    }

    let Ok(single_pixels) = wiener_row_blocks::<_, 1>(&mut inputs);

    for (pixel, samples) in tail_8.iter_mut().zip(single_pixels) {
      let mut sum = i32::wrapping_neg(round_offset);

      for ([sample], coefficient) in samples.into_iter().zip(vertical_filter) {
        sum = sum.wrapping_add(i32::from(*sample).wrapping_mul(i32::from(*coefficient)));
      }

      *pixel = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(iclip(
        ::core::primitive::i32::wrapping_shr(
          i32::wrapping_add(sum, rounding_off_v),
          ::core::primitive::i32::unsigned_abs(round_bits_v),
        ),
        0,
        255,
      )) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    }

    Ok(())
  })
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn wiener_filter7_16bpc_avx512_inner(
  _token: Server64,
  request: RestorationPadding<'_, BitDepth16>,
  params: &LooprestorationParams,
  bitdepth_max: c_int,
) -> Result<(), RowLayoutError> {
  let extent = request.size;
  let destination = request.picture;
  let maximum_width = u64::from(RESTORATION_INTERIOR);

  match ((1..=maximum_width).contains(&extent.width) && (1..=64).contains(&extent.height)).then_some(()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(RowLayoutError::Bounds),
  }

  let bitdepth = match ::core::primitive::bool::then_some(bitdepth_max == 1023, ()) {
    ::core::option::Option::Some(()) => 10,
    ::core::option::Option::None => 12,
  };

  let mut tmp: RestorationPixels<_> = [0u16; _];
  padding::<BitDepth16>(&mut tmp, request).map_err(|_padding| RowLayoutError::Bounds)?;

  let mut hor: RestorationPixels<_> = [0i32; _];

  let [horizontal_filter, vertical_filter] = &params.filter;
  let round_bits_h = match ::core::primitive::bool::then_some(bitdepth == 12, ()) {
    ::core::option::Option::Some(()) => 5,
    ::core::option::Option::None => 3,
  };
  let rounding_off_h =
    i32::wrapping_shl(1i32, i32::cast_unsigned(i32::wrapping_sub(round_bits_h, 1)));
  let clip_limit = i32::wrapping_shl(
    1i32,
    i32::cast_unsigned(i32::wrapping_sub(
      i32::wrapping_add(i32::wrapping_add(bitdepth, 1), 7),
      round_bits_h,
    )),
  );

  let [hf0, hf1, hf2, hf3, hf4, hf5, hf6, _] =
    horizontal_filter.map(|coefficient| _mm512_set1_epi32(i32::from(coefficient)));
  let dc_offset_h = _mm512_set1_epi32(i32::wrapping_shl(
    1i32,
    i32::cast_unsigned(i32::wrapping_add(bitdepth, 6)),
  ));
  let h_rounding = _mm512_set1_epi32(rounding_off_h);
  let h_clip_max = _mm512_set1_epi32(i32::wrapping_sub(clip_limit, 1));
  let h_zero = _mm512_setzero_si512();

  let Ok(padded) = restoration_rows(&tmp);
  let Ok(intermediate) = restoration_rows_mut(&mut hor);

  let Ok(rows) = index(extent.height.wrapping_add(6));
  let Ok(columns) = index(extent.width);

  for (src, row) in padded.iter().zip(intermediate).take(rows) {
    let row_7 = row.get_mut(..columns).ok_or(RowLayoutError::Bounds)?;
    let (blocks, tail) = row_7.as_chunks_mut::<16>();
    let Ok(tail_length) = fixed_index(tail.len());
    let Ok(tail_start) = index(extent.width.wrapping_sub(tail_length));
    let Ok(windows) = tapped_blocks::<_, 16, 7>(src);

    for (
      target,
      [
        sample_0,
        sample_1,
        sample_2,
        sample_3,
        sample_4,
        sample_5,
        sample_6,
      ],
    ) in blocks.iter_mut().zip(windows)
    {
      let p0 = _mm512_cvtepu16_epi32(loadu_256!(sample_0));
      let p1 = _mm512_cvtepu16_epi32(loadu_256!(sample_1));
      let p2 = _mm512_cvtepu16_epi32(loadu_256!(sample_2));
      let p3 = _mm512_cvtepu16_epi32(loadu_256!(sample_3));
      let p4 = _mm512_cvtepu16_epi32(loadu_256!(sample_4));
      let p5 = _mm512_cvtepu16_epi32(loadu_256!(sample_5));
      let p6 = _mm512_cvtepu16_epi32(loadu_256!(sample_6));

      let mut sum = dc_offset_h;
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p0, hf0));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p1, hf1));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p2, hf2));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p3, hf3));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p4, hf4));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p5, hf5));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p6, hf6));

      sum = _mm512_add_epi32(sum, h_rounding);
      let shifted = match ::core::primitive::bool::then_some(bitdepth == 12, ()) {
        ::core::option::Option::Some(()) => _mm512_srai_epi32::<5>(sum),
        ::core::option::Option::None => _mm512_srai_epi32::<3>(sum),
      };
      let clamped = _mm512_max_epi32(shifted, h_zero);
      let clamped_2 = _mm512_min_epi32(clamped, h_clip_max);

      storeu_512!(target, clamped_2);
    }

    for (target, samples) in tail
      .iter_mut()
      .zip(src.array_windows::<7>().skip(tail_start))
    {
      let mut sum = i32::wrapping_shl(1i32, i32::cast_unsigned(i32::wrapping_add(bitdepth, 6)));

      for (sample, coefficient) in samples.iter().zip(horizontal_filter) {
        sum = sum.wrapping_add(i32::from(*sample).wrapping_mul(i32::from(*coefficient)));
      }

      *target = iclip(
        i32::wrapping_shr(
          i32::wrapping_add(sum, rounding_off_h),
          i32::cast_unsigned(round_bits_h),
        ),
        0,
        i32::wrapping_sub(clip_limit, 1),
      );
    }
  }

  let round_bits_v = match ::core::primitive::bool::then_some(bitdepth == 12, ()) {
    ::core::option::Option::Some(()) => 9,
    ::core::option::Option::None => 11,
  };
  let rounding_off_v =
    i32::wrapping_shl(1i32, i32::cast_unsigned(i32::wrapping_sub(round_bits_v, 1)));
  let round_offset = i32::wrapping_shl(
    1i32,
    i32::cast_unsigned(i32::wrapping_sub(
      i32::wrapping_add(bitdepth, round_bits_v),
      1,
    )),
  );
  let [vf0, vf1, vf2, vf3, vf4, vf5, vf6, _] =
    vertical_filter.map(|coefficient| _mm512_set1_epi32(i32::from(coefficient)));
  let v_round_offset = _mm512_set1_epi32(i32::wrapping_neg(round_offset));
  let v_rounding = _mm512_set1_epi32(rounding_off_v);
  let zero_512 = _mm512_setzero_si512();
  let max_512 = _mm512_set1_epi32(bitdepth_max);

  let Ok(intermediate_2) = restoration_rows(&hor);
  let mut intermediate_rows = intermediate_2;
  destination.checked_for_rows_mut_with_context::<BitDepth16, _>(extent, &mut intermediate_rows, |row, output, scratch_rows| {
    let mut inputs = wiener_inputs(scratch_rows, WienerInputWindow { row, width: extent.width })?;


    let (blocks_16, tail_16) = output.as_chunks_mut::<16>();
    let Ok(columns_16) = wiener_row_blocks::<_, 16>(&mut inputs);

    for (
      target,
      [
        input_0,
        input_1,
        input_2,
        input_3,
        input_4,
        input_5,
        input_6,
      ],
    ) in blocks_16.iter_mut().zip(columns_16)
    {
      let r0 = loadu_512!(input_0);
      let r1 = loadu_512!(input_1);
      let r2 = loadu_512!(input_2);
      let r3 = loadu_512!(input_3);
      let r4 = loadu_512!(input_4);
      let r5 = loadu_512!(input_5);
      let r6 = loadu_512!(input_6);

      let mut sum = v_round_offset;
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r0, vf0));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r1, vf1));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r2, vf2));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r3, vf3));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r4, vf4));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r5, vf5));
      sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r6, vf6));

      sum = _mm512_add_epi32(sum, v_rounding);
      let shifted = match ::core::primitive::bool::then_some(bitdepth == 12, ()) {
        ::core::option::Option::Some(()) => _mm512_srai_epi32::<9>(sum),
        ::core::option::Option::None => _mm512_srai_epi32::<11>(sum),
      };

      let clamped = _mm512_min_epi32(_mm512_max_epi32(shifted, zero_512), max_512);
      let result_u16: __m256i = _mm512_cvtusepi32_epi16(clamped);

      storeu_256!(target, result_u16);
    }

    let Ok(single_pixels) = wiener_row_blocks::<_, 1>(&mut inputs);

    for (pixel, samples) in tail_16.iter_mut().zip(single_pixels) {
      let mut sum = i32::wrapping_neg(round_offset);

      for ([sample], coefficient) in samples.into_iter().zip(vertical_filter) {
        sum = sum.wrapping_add(i32::from(*sample).wrapping_mul(i32::from(*coefficient)));
      }

      *pixel =
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(iclip(
          i32::wrapping_shr(
            i32::wrapping_add(sum, rounding_off_v),
            i32::cast_unsigned(round_bits_v),
          ),
          0,
          bitdepth_max,
        )) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
    }

    Ok(())
  })
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn wiener_filter7_16bpc_avx2_inner(
  _token: Desktop64,
  request: RestorationPadding<'_, BitDepth16>,
  params: &LooprestorationParams,
  bitdepth_max: c_int,
) -> Result<(), RowLayoutError> {
  let extent = request.size;
  let destination = request.picture;
  let maximum_width = u64::from(RESTORATION_INTERIOR);

  match ((1..=maximum_width).contains(&extent.width) && (1..=64).contains(&extent.height)).then_some(()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(RowLayoutError::Bounds),
  }

  let bitdepth = match ::core::primitive::bool::then_some(bitdepth_max == 1023, ()) {
    ::core::option::Option::Some(()) => 10,
    ::core::option::Option::None => 12,
  };

  let mut tmp: RestorationPixels<_> = [0u16; _];

  padding::<BitDepth16>(&mut tmp, request).map_err(|_padding| RowLayoutError::Bounds)?;

  let mut hor: RestorationPixels<_> = [0i32; _];

  let [horizontal_filter, vertical_filter] = &params.filter;

  let round_bits_h = match ::core::primitive::bool::then_some(bitdepth == 12, ()) {
    ::core::option::Option::Some(()) => 5,
    ::core::option::Option::None => 3,
  };
  let rounding_off_h =
    i32::wrapping_shl(1i32, i32::cast_unsigned(i32::wrapping_sub(round_bits_h, 1)));
  let clip_limit = i32::wrapping_shl(
    1i32,
    i32::cast_unsigned(i32::wrapping_sub(
      i32::wrapping_add(i32::wrapping_add(bitdepth, 1), 7),
      round_bits_h,
    )),
  );

  let [hf0, hf1, hf2, hf3, hf4, hf5, hf6, _] =
    horizontal_filter.map(|coefficient| _mm256_set1_epi32(i32::from(coefficient)));
  let dc_offset_h = _mm256_set1_epi32(i32::wrapping_shl(
    1i32,
    i32::cast_unsigned(i32::wrapping_add(bitdepth, 6)),
  ));
  let h_rounding = _mm256_set1_epi32(rounding_off_h);
  let h_clip_max = _mm256_set1_epi32(i32::wrapping_sub(clip_limit, 1));
  let h_zero = _mm256_setzero_si256();

  let Ok(padded) = restoration_rows(&tmp);
  let Ok(intermediate) = restoration_rows_mut(&mut hor);

  let Ok(rows) = index(extent.height.wrapping_add(6));
  let Ok(columns) = index(extent.width);

  for (src, row) in padded.iter().zip(intermediate).take(rows) {
    let row_7 = row.get_mut(..columns).ok_or(RowLayoutError::Bounds)?;
    let (blocks, tail) = row_7.as_chunks_mut::<8>();
    let Ok(tail_length) = fixed_index(tail.len());
    let Ok(tail_start) = index(extent.width.wrapping_sub(tail_length));
    let Ok(windows) = tapped_blocks::<_, 8, 7>(src);

    for (
      target,
      [
        sample_0,
        sample_1,
        sample_2,
        sample_3,
        sample_4,
        sample_5,
        sample_6,
      ],
    ) in blocks.iter_mut().zip(windows)
    {
      let p0 = _mm256_cvtepu16_epi32(loadu_128!(sample_0));
      let p1 = _mm256_cvtepu16_epi32(loadu_128!(sample_1));
      let p2 = _mm256_cvtepu16_epi32(loadu_128!(sample_2));
      let p3 = _mm256_cvtepu16_epi32(loadu_128!(sample_3));
      let p4 = _mm256_cvtepu16_epi32(loadu_128!(sample_4));
      let p5 = _mm256_cvtepu16_epi32(loadu_128!(sample_5));
      let p6 = _mm256_cvtepu16_epi32(loadu_128!(sample_6));

      let mut sum = dc_offset_h;
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p0, hf0));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p1, hf1));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p2, hf2));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p3, hf3));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p4, hf4));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p5, hf5));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p6, hf6));

      sum = _mm256_add_epi32(sum, h_rounding);
      let shifted = match ::core::primitive::bool::then_some(bitdepth == 12, ()) {
        ::core::option::Option::Some(()) => _mm256_srai_epi32::<5>(sum),
        ::core::option::Option::None => _mm256_srai_epi32::<3>(sum),
      };
      let clamped = _mm256_max_epi32(shifted, h_zero);
      let clamped_2 = _mm256_min_epi32(clamped, h_clip_max);

      storeu_256!(target, clamped_2);
    }

    for (target, samples) in tail
      .iter_mut()
      .zip(src.array_windows::<7>().skip(tail_start))
    {
      let mut sum = i32::wrapping_shl(1i32, i32::cast_unsigned(i32::wrapping_add(bitdepth, 6)));

      for (sample, coefficient) in samples.iter().zip(horizontal_filter) {
        sum = sum.wrapping_add(i32::from(*sample).wrapping_mul(i32::from(*coefficient)));
      }

      *target = iclip(
        i32::wrapping_shr(
          i32::wrapping_add(sum, rounding_off_h),
          i32::cast_unsigned(round_bits_h),
        ),
        0,
        i32::wrapping_sub(clip_limit, 1),
      );
    }
  }

  let round_bits_v = match ::core::primitive::bool::then_some(bitdepth == 12, ()) {
    ::core::option::Option::Some(()) => 9,
    ::core::option::Option::None => 11,
  };
  let rounding_off_v =
    i32::wrapping_shl(1i32, i32::cast_unsigned(i32::wrapping_sub(round_bits_v, 1)));
  let round_offset = i32::wrapping_shl(
    1i32,
    i32::cast_unsigned(i32::wrapping_sub(
      i32::wrapping_add(bitdepth, round_bits_v),
      1,
    )),
  );
  let [vf0, vf1, vf2, vf3, vf4, vf5, vf6, _] =
    vertical_filter.map(|coefficient| _mm256_set1_epi32(i32::from(coefficient)));
  let v_round_offset = _mm256_set1_epi32(i32::wrapping_neg(round_offset));
  let v_rounding = _mm256_set1_epi32(rounding_off_v);
  let v_max = _mm256_set1_epi32(bitdepth_max);
  let v_zero = _mm256_setzero_si256();

  let Ok(intermediate_2) = restoration_rows(&hor);
  let mut intermediate_rows = intermediate_2;
  destination.checked_for_rows_mut_with_context::<BitDepth16, _>(extent, &mut intermediate_rows, |row, output, scratch_rows| {
    let mut inputs = wiener_inputs(scratch_rows, WienerInputWindow { row, width: extent.width })?;


    let (blocks_8, tail_8) = output.as_chunks_mut::<8>();
    let Ok(columns_8) = wiener_row_blocks::<_, 8>(&mut inputs);

    for (
      target,
      [
        input_0,
        input_1,
        input_2,
        input_3,
        input_4,
        input_5,
        input_6,
      ],
    ) in blocks_8.iter_mut().zip(columns_8)
    {
      let r0 = loadu_256!(input_0);
      let r1 = loadu_256!(input_1);
      let r2 = loadu_256!(input_2);
      let r3 = loadu_256!(input_3);
      let r4 = loadu_256!(input_4);
      let r5 = loadu_256!(input_5);
      let r6 = loadu_256!(input_6);

      let mut sum = v_round_offset;
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(r0, vf0));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(r1, vf1));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(r2, vf2));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(r3, vf3));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(r4, vf4));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(r5, vf5));
      sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(r6, vf6));

      sum = _mm256_add_epi32(sum, v_rounding);
      let shifted = match ::core::primitive::bool::then_some(bitdepth == 12, ()) {
        ::core::option::Option::Some(()) => _mm256_srai_epi32::<9>(sum),
        ::core::option::Option::None => _mm256_srai_epi32::<11>(sum),
      };

      let clamped = _mm256_min_epi32(_mm256_max_epi32(shifted, v_zero), v_max);
      let packed = _mm256_packus_epi32(clamped, clamped);
      let lo = _mm256_castsi256_si128(packed);
      let hi = _mm256_extracti128_si256(packed, 1);
      let combined = _mm_unpacklo_epi64(lo, hi);

      storeu_128!(target, combined);
    }

    let Ok(single_pixels) = wiener_row_blocks::<_, 1>(&mut inputs);

    for (pixel, samples) in tail_8.iter_mut().zip(single_pixels) {
      let mut sum = i32::wrapping_neg(round_offset);

      for ([sample], coefficient) in samples.into_iter().zip(vertical_filter) {
        sum = sum.wrapping_add(i32::from(*sample).wrapping_mul(i32::from(*coefficient)));
      }

      *pixel =
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(iclip(
          i32::wrapping_shr(
            i32::wrapping_add(sum, rounding_off_v),
            i32::cast_unsigned(round_bits_v),
          ),
          0,
          bitdepth_max,
        )) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
    }

    Ok(())
  })
}

#[inline]
fn sgr_factor(value: u32) -> Rav1dResult<u32> {
  let Ok(index) = console_core_number_conversion::index(value.min(255));
  aligned_inner(&dav1d_sgr_x_by_x)
    .get(index)
    .copied()
    .map(u32::from)
    .ok_or(Rav1dError::EINVAL)
}

#[derive(Clone, Copy)]
struct PaddedBox {
  width: u16,
  rows: u16,
}

impl PaddedBox {
  fn new(size: Size<impl TryInto<u16>>) -> Rav1dResult<Self> {
    let width = size.width.try_into().map_err(|_wide| Rav1dError::EINVAL)?;
    let height = size.height.try_into().map_err(|_tall| Rav1dError::EINVAL)?;

    match ::core::primitive::bool::then_some(width >= 5
      && width <= REST_UNIT_STRIDE
      && (5..=70).contains(&height), ())
    {
      ::core::option::Option::Some(()) => Ok(Self {
        width,
        rows: height.wrapping_sub(4),
      }),
      ::core::option::Option::None => Err(Rav1dError::EINVAL),
    }
  }

  fn horizontal<T: Copy + Default, const TAPS: usize>(
    self,
    values: &mut [T],
    mut filter_row: impl FnMut(&[T], &mut [T]),
  ) -> Rav1dResult {
    let columns = self.width.wrapping_sub(4);
    let first = match TAPS {
      3 => 1,
      5 => 0,
      _ => return Err(Rav1dError::EINVAL),
    };
    let last = self.width.wrapping_sub(first);
    let Ok(scratch_stride) = index(REST_UNIT_STRIDE);
    let storage = values
      .get_mut(
        ..match ::console_core_number_conversion::index::<::core::primitive::u16>(
          self.rows.wrapping_add(1),
        ) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        }
        .wrapping_mul(scratch_stride),
      )
      .ok_or(Rav1dError::EINVAL)?;
    let mut scratch: RestorationRow<_> = [T::default(); _];
    let output = scratch
      .get_mut(
        ..match ::console_core_number_conversion::index::<::core::primitive::u16>(columns) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        },
      )
      .ok_or(Rav1dError::EINVAL)?;

    let Ok(storage_rows) = restoration_rows_mut(storage);

    for row in storage_rows.iter_mut().skip(1) {
      let source = row
        .get(
          match ::console_core_number_conversion::index::<::core::primitive::u16>(first) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          }
            ..match ::console_core_number_conversion::index::<::core::primitive::u16>(last) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            },
        )
        .ok_or(Rav1dError::EINVAL)?;
      filter_row(source, output);
      let destination = row
        .get_mut(
          2..match ::console_core_number_conversion::index::<::core::primitive::u16>(
            self.width.wrapping_sub(2),
          ) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          },
        )
        .ok_or(Rav1dError::EINVAL)?;

      for (target, &sample) in destination.iter_mut().zip(output.iter()) {
        *target = sample;
      }
    }

    Ok(())
  }
}

#[inline]
fn scalar_boxsum_row<T: Copy + Default, const TAPS: usize>(
  source: &[T],
  output: &mut [T],
  add: impl Fn(T, T) -> T + Copy,
) -> Result<(), Never> {
  for (target, window) in output.iter_mut().zip(source.array_windows::<TAPS>()) {
    *target = window.iter().copied().fold(T::default(), add);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn vector_boxsum_row<T: Copy + Default, const TAPS: usize, const LANES: usize>(
  source: &[T],
  output: &mut [T],
  mut filter_block: impl FnMut([&[T; LANES]; TAPS], &mut [T; LANES]),
  add: impl Fn(T, T) -> T + Copy,
) -> Result<(), Never> {
  let (blocks, tail) = output.as_chunks_mut::<LANES>();
  let Ok(taps) = tapped_blocks::<_, LANES, TAPS>(source);

  for (target, windows) in blocks.iter_mut().zip(taps) {
    filter_block(windows, target);
  }

  let source_tail = source
    .array_windows::<TAPS>()
    .skip(blocks.len().wrapping_mul(LANES));

  for (target, window) in tail.iter_mut().zip(source_tail) {
    *target = window.iter().copied().fold(T::default(), add);
  }

  Ok(())
}

#[derive(Clone, Copy, Default)]
struct ColumnMoments {
  sum: i32,
  squares: i32,
}

impl ColumnMoments {
  fn push(&mut self, sample: i32) -> Result<(), console_core_never::Never> {
    self.sum = self.sum.wrapping_add(sample);
    self.squares = self.squares.wrapping_add(sample.wrapping_mul(sample));
    Ok(())
  }

  fn remove(&mut self, sample: i32) -> Result<(), console_core_never::Never> {
    self.sum = self.sum.wrapping_sub(sample);
    self.squares = self.squares.wrapping_sub(sample.wrapping_mul(sample));
    Ok(())
  }
}

struct ColumnRead<'a, Pixel> {
  initial: &'a [RestorationRow<Pixel>],
  old: &'a [RestorationRow<Pixel>],
  incoming: &'a [RestorationRow<Pixel>],
}

struct ColumnWrite<'a, Sum, Square> {
  sums: &'a mut [RestorationRow<Sum>],
  squares: &'a mut [RestorationRow<Square>],
}

fn measure_box_column<Pixel, Sum, Square>(
  source: &ColumnRead<'_, Pixel>,
  output: ColumnWrite<'_, Sum, Square>,
  column: u64,
) -> Rav1dResult
where
  Pixel: Copy,
  Sum: TryFrom<i32>,
  Square: From<i32>,
  i32: From<Pixel>,
{
  let Ok(position) = index(column);
  let mut moments = ColumnMoments::default();

  for row in source.initial {
    let &pixel = row.get(position).ok_or(Rav1dError::EINVAL)?;
    let Ok(()) = moments.push(i32::from(pixel));
  }

  for ((old, next), (sum, squares)) in source.old.iter().zip(source.incoming).zip(output.sums.iter_mut().zip(output.squares)) {
    let &new_pixel = next.get(position).ok_or(Rav1dError::EINVAL)?;
    let &old_pixel = old.get(position).ok_or(Rav1dError::EINVAL)?;
    let Ok(()) = moments.push(i32::from(new_pixel));
    let sample = Sum::try_from(moments.sum).map_err(|_outside_bitdepth| Rav1dError::EINVAL)?;
    let target = sum.get_mut(position).ok_or(Rav1dError::EINVAL)?;
    let second_moment = squares.get_mut(position).ok_or(Rav1dError::EINVAL)?;
    *target = sample;
    *second_moment = Square::from(moments.squares);
    let Ok(()) = moments.remove(i32::from(old_pixel));
  }

  Ok(())
}

struct MomentAddition<Sum, Square> {
  sum: fn(Sum, Sum) -> Sum,
  squares: fn(Square, Square) -> Square,
}

fn scalar_boxsum<Pixel, Sum, Square, const N: usize>(
  pass: VerticalBox<'_, Pixel, Sum, Square>,
  area: PaddedBox,
  add: MomentAddition<Sum, Square>,
) -> Rav1dResult
where
  Pixel: Copy,
  Sum: Copy + Default + TryFrom<i32>,
  Square: Copy + Default + From<i32>,
  i32: From<Pixel>,
{
  let Ok(pixels) = restoration_rows(pass.input);
  let Ok(sums) = restoration_rows_mut(pass.sums);
  let Ok(squares) = restoration_rows_mut(pass.squares);
  let first = match N {
    3 => 1_u64,
    5 => 0_u64,
    _invalid => return Err(Rav1dError::EINVAL),
  };
  let Ok(taps) = fixed_index(N);
  let rows = pass.extent.height.wrapping_sub(4);
  let Ok(start) = index(first);
  let Ok(initial_end) = index(first.wrapping_add(taps).wrapping_sub(1));
  let Ok(last) = index(first.wrapping_add(rows));
  let Ok(incoming_end) = index(first.wrapping_add(taps).wrapping_sub(1).wrapping_add(rows));
  let Ok(target_end) = index(rows.wrapping_add(1));
  let initial = pixels.get(start..initial_end).ok_or(Rav1dError::EINVAL)?;
  let old = pixels.get(start..last).ok_or(Rav1dError::EINVAL)?;
  let incoming = pixels.get(initial_end..incoming_end).ok_or(Rav1dError::EINVAL)?;
  let samples = ColumnRead {
    initial,
    old,
    incoming,
  };
  let sum_rows = sums.get_mut(1..target_end).ok_or(Rav1dError::EINVAL)?;
  let square_rows = squares.get_mut(1..target_end).ok_or(Rav1dError::EINVAL)?;

  for column in first..pass.extent.width.wrapping_sub(first) {
    measure_box_column(
      &samples,
      ColumnWrite {
        sums: &mut *sum_rows,
        squares: &mut *square_rows,
      },
      column,
    )?;
  }

  area.horizontal::<_, N>(pass.sums, |source, target| {
    let Ok(()) = scalar_boxsum_row::<_, N>(source, target, add.sum);
  })?;
  area.horizontal::<_, N>(pass.squares, |source, target| {
    let Ok(()) = scalar_boxsum_row::<_, N>(source, target, add.squares);
  })
}

#[cfg(target_arch = "x86_64")]
struct VerticalBox<'a, Pixel, Sum, Square> {
  input: &'a [Pixel],
  sums: &'a mut [Sum],
  squares: &'a mut [Square],
  extent: Size<u64>,
}

#[cfg(target_arch = "x86_64")]
struct BoxVector<'a, Pixel, Sum, Square, const LANES: usize, const TAPS: usize> {
  pixels: &'a [RestorationRow<Pixel>; TAPS],
  sums: &'a mut [Sum; LANES],
  squares: &'a mut [Square; LANES],
  column: u64,
}

#[cfg(target_arch = "x86_64")]
fn box_view<T, const N: usize>(values: &[T], column: u64) -> Rav1dResult<&[T; N]> {
  let Ok(position) = index(column);
  values.get(position..).and_then(<[_]>::first_chunk).ok_or(Rav1dError::EINVAL)
}

#[cfg(target_arch = "x86_64")]
fn box_view_mut<T, const N: usize>(values: &mut [T], column: u64) -> Rav1dResult<&mut [T; N]> {
  let Ok(position) = index(column);
  values.get_mut(position..).and_then(<[_]>::first_chunk_mut).ok_or(Rav1dError::EINVAL)
}

#[cfg(target_arch = "x86_64")]
struct VerticalTail<'a, Pixel, Sum, Square, const TAPS: usize> {
  pixels: &'a [RestorationRow<Pixel>; TAPS],
  sums: &'a mut RestorationRow<Sum>,
  squares: &'a mut RestorationRow<Square>,
  columns: core::ops::Range<u64>,
}

#[cfg(target_arch = "x86_64")]
fn fill_box_tail<Pixel: Copy, Sum: TryFrom<i32>, Square: From<i32>, const TAPS: usize>(
  tail: VerticalTail<'_, Pixel, Sum, Square, TAPS>,
) -> Rav1dResult
where
  i32: From<Pixel>,
{
  for column in tail.columns {
    let Ok(x) = index(column);
    let mut sum = 0_i32;
    let mut squares = 0_i32;

    for line in tail.pixels {
      let &sample = line.get(x).ok_or(Rav1dError::EINVAL)?;
      let value = i32::from(sample);
      sum = sum.wrapping_add(value);
      squares = squares.wrapping_add(value.wrapping_mul(value));
    }

    let destination = tail.sums.get_mut(x).ok_or(Rav1dError::EINVAL)?;
    let converted_sum = Sum::try_from(sum).map_err(|_overflow| Rav1dError::EINVAL)?;
    *destination = converted_sum;
    let square_destination = tail.squares.get_mut(x).ok_or(Rav1dError::EINVAL)?;
    *square_destination = Square::from(squares);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
fn vertical_box_pass<Pixel: Copy, Sum: TryFrom<i32>, Square: From<i32>, const LANES: usize, const TAPS: usize>(
  pass: VerticalBox<'_, Pixel, Sum, Square>,
  mut vector: impl FnMut(BoxVector<'_, Pixel, Sum, Square, LANES, TAPS>) -> Rav1dResult,
) -> Rav1dResult
where
  i32: From<Pixel>,
{
  let Ok(source) = restoration_rows(pass.input);
  let Ok(sum_rows) = restoration_rows_mut(pass.sums);
  let Ok(square_rows) = restoration_rows_mut(pass.squares);
  let first = match TAPS {
    3 => 1_u64,
    5 => 0_u64,
    _invalid => return Err(Rav1dError::EINVAL),
  };
  let last = pass.extent.width.wrapping_sub(first);
  let Ok(lanes) = fixed_index(LANES);
  let available = last.wrapping_sub(first);
  let complete_blocks = available.checked_div(lanes).ok_or(Rav1dError::EINVAL)?;
  let tail_start = first.wrapping_add(complete_blocks.wrapping_mul(lanes));
  let rows = pass.extent.height.wrapping_sub(4);
  let Ok(end) = index(rows.wrapping_add(1));
  let moments = sum_rows.get_mut(1..end).ok_or(Rav1dError::EINVAL)?;
  let squares = square_rows.get_mut(1..end).ok_or(Rav1dError::EINVAL)?;

  for (number, (sum, squared)) in (0..rows).zip(moments.iter_mut().zip(squares)) {
    let input = box_view(source, number.wrapping_add(first))?;

    for block in 0..complete_blocks {
      let column = first.wrapping_add(block.wrapping_mul(lanes));
      let sums = box_view_mut(sum, column)?;
      let squares_2 = box_view_mut(squared, column)?;
      vector(BoxVector { pixels: input, sums, squares: squares_2, column })?;
    }

    fill_box_tail(VerticalTail { pixels: input, sums: sum, squares: squared, columns: tail_start..last })?;
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn boxsum_v_avx2<const TAPS: usize>(
  _token: Desktop64,
  pass: VerticalBox<'_, u8, i16, i32>,
) -> Rav1dResult {
  vertical_box_pass::<_, _, _, 16, TAPS>(pass, |block| {
    let mut sum = _mm256_setzero_si256();
    let mut squares_low = _mm256_setzero_si256();
    let mut squares_high = _mm256_setzero_si256();

    for row in block.pixels {
      let pixels = box_view(row, block.column)?;
      let bytes = archmage::intrinsics::x86_64::_mm_loadu_si128(pixels);
      let wide = _mm256_cvtepu8_epi16(bytes);
      let low = _mm256_cvtepu8_epi32(bytes);
      let high = _mm256_cvtepu8_epi32(_mm_srli_si128::<8>(bytes));
      sum = _mm256_add_epi16(sum, wide);
      squares_low = _mm256_add_epi32(squares_low, _mm256_mullo_epi32(low, low));
      squares_high = _mm256_add_epi32(squares_high, _mm256_mullo_epi32(high, high));
    }

    let [low, high]: &mut [[i32; 8]; 2] = zerocopy::transmute_mut!(block.squares);
    archmage::intrinsics::x86_64::_mm256_storeu_si256(block.sums, sum);
    archmage::intrinsics::x86_64::_mm256_storeu_si256(low, squares_low);
    archmage::intrinsics::x86_64::_mm256_storeu_si256(high, squares_high);
    Ok(())
  })
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn boxsum_v_avx512<const TAPS: usize>(
  _token: Server64,
  pass: VerticalBox<'_, u8, i16, i32>,
) -> Rav1dResult {
  vertical_box_pass::<_, _, _, 32, TAPS>(pass, |block| {
    let mut sum = _mm512_setzero_si512();
    let mut squares_low = _mm512_setzero_si512();
    let mut squares_high = _mm512_setzero_si512();

    for row in block.pixels {
      let pixels = box_view(row, block.column)?;
      let bytes = archmage::intrinsics::x86_64::_mm256_loadu_si256(pixels);
      let wide = _mm512_cvtepu8_epi16(bytes);
      let low = _mm512_cvtepu8_epi32(_mm256_castsi256_si128(bytes));
      let high = _mm512_cvtepu8_epi32(_mm256_extracti128_si256::<1>(bytes));
      sum = _mm512_add_epi16(sum, wide);
      squares_low = _mm512_add_epi32(squares_low, _mm512_mullo_epi32(low, low));
      squares_high = _mm512_add_epi32(squares_high, _mm512_mullo_epi32(high, high));
    }

    let [low, high]: &mut [[i32; 16]; 2] = zerocopy::transmute_mut!(block.squares);
    archmage::intrinsics::x86_64::_mm512_storeu_si512(block.sums, sum);
    archmage::intrinsics::x86_64::_mm512_storeu_si512(low, squares_low);
    archmage::intrinsics::x86_64::_mm512_storeu_si512(high, squares_high);
    Ok(())
  })
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn boxsum_v_16bpc_avx512<const TAPS: usize>(
  _token: Server64,
  pass: VerticalBox<'_, u16, i32, i64>,
) -> Rav1dResult {
  vertical_box_pass::<_, _, _, 16, TAPS>(pass, |block| {
    let mut sum = _mm512_setzero_si512();
    let mut squares = _mm512_setzero_si512();

    for row in block.pixels {
      let pixels = box_view(row, block.column)?;
      let words = archmage::intrinsics::x86_64::_mm256_loadu_si256(pixels);
      let wide = _mm512_cvtepu16_epi32(words);
      sum = _mm512_add_epi32(sum, wide);
      squares = _mm512_add_epi32(squares, _mm512_mullo_epi32(wide, wide));
    }

    let [low, high]: &mut [[i64; 8]; 2] = zerocopy::transmute_mut!(block.squares);
    let first = _mm512_cvtepu32_epi64(_mm512_castsi512_si256(squares));
    let second = _mm512_cvtepu32_epi64(_mm512_extracti64x4_epi64::<1>(squares));
    archmage::intrinsics::x86_64::_mm512_storeu_si512(block.sums, sum);
    archmage::intrinsics::x86_64::_mm512_storeu_si512(low, first);
    archmage::intrinsics::x86_64::_mm512_storeu_si512(high, second);
    Ok(())
  })
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn boxsum5_h_avx2(
  _token: Desktop64,
  sumsq: &mut RestorationMoments<i32>,
  sum: &mut RestorationMoments<i16>,
  area: PaddedBox,
) -> Rav1dResult {
  area.horizontal::<_, 5>(sum, |source, target| {
    let Ok(()) = vector_boxsum_row::<_, 5, 16>(
      source,
      target,
      |windows, out| {
        let [a, b, c, d, e] = windows.map(|block| loadu_256!(block, [i16; 16]));
        let sum_2 = _mm256_add_epi16(
          _mm256_add_epi16(_mm256_add_epi16(a, b), _mm256_add_epi16(c, d)),
          e,
        );
        storeu_256!(out, [i16; 16], sum_2);
      },
      i16::wrapping_add,
    );
  })?;

  area.horizontal::<_, 5>(sumsq, |source, target| {
    let Ok(()) = vector_boxsum_row::<_, 5, 8>(
      source,
      target,
      |windows, out| {
        let [a, b, c, d, e] = windows.map(|block| loadu_256!(block, [i32; 8]));
        let sum_3 = _mm256_add_epi32(
          _mm256_add_epi32(_mm256_add_epi32(a, b), _mm256_add_epi32(c, d)),
          e,
        );
        storeu_256!(out, [i32; 8], sum_3);
      },
      i32::wrapping_add,
    );
  })?;

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn boxsum3_h_avx2(
  _token: Desktop64,
  sumsq: &mut RestorationMoments<i32>,
  sum: &mut RestorationMoments<i16>,
  area: PaddedBox,
) -> Rav1dResult {
  area.horizontal::<_, 3>(sum, |source, target| {
    let Ok(()) = vector_boxsum_row::<_, 3, 16>(
      source,
      target,
      |windows, out| {
        let [a, b, c] = windows.map(|block| loadu_256!(block, [i16; 16]));
        let sum_2 = _mm256_add_epi16(_mm256_add_epi16(a, b), c);
        storeu_256!(out, [i16; 16], sum_2);
      },
      i16::wrapping_add,
    );
  })?;

  area.horizontal::<_, 3>(sumsq, |source, target| {
    let Ok(()) = vector_boxsum_row::<_, 3, 8>(
      source,
      target,
      |windows, out| {
        let [a, b, c] = windows.map(|block| loadu_256!(block, [i32; 8]));
        let sum_3 = _mm256_add_epi32(_mm256_add_epi32(a, b), c);
        storeu_256!(out, [i32; 8], sum_3);
      },
      i32::wrapping_add,
    );
  })?;

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn boxsum5_h_avx512(
  _token: Server64,
  sumsq: &mut RestorationMoments<i32>,
  sum: &mut RestorationMoments<i16>,
  area: PaddedBox,
) -> Rav1dResult {
  area.horizontal::<_, 5>(sum, |source, target| {
    let Ok(()) = vector_boxsum_row::<_, 5, 32>(
      source,
      target,
      |windows, out| {
        let [a, b, c, d, e] = windows.map(|block| loadu_512!(block, [i16; 32]));
        let sum_2 = _mm512_add_epi16(
          _mm512_add_epi16(_mm512_add_epi16(a, b), _mm512_add_epi16(c, d)),
          e,
        );
        storeu_512!(out, [i16; 32], sum_2);
      },
      i16::wrapping_add,
    );
  })?;

  area.horizontal::<_, 5>(sumsq, |source, target| {
    let Ok(()) = vector_boxsum_row::<_, 5, 16>(
      source,
      target,
      |windows, out| {
        let [a, b, c, d, e] = windows.map(|block| loadu_512!(block, [i32; 16]));
        let sum_3 = _mm512_add_epi32(
          _mm512_add_epi32(_mm512_add_epi32(a, b), _mm512_add_epi32(c, d)),
          e,
        );
        storeu_512!(out, [i32; 16], sum_3);
      },
      i32::wrapping_add,
    );
  })?;

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn boxsum3_h_avx512(
  _token: Server64,
  sumsq: &mut RestorationMoments<i32>,
  sum: &mut RestorationMoments<i16>,
  area: PaddedBox,
) -> Rav1dResult {
  area.horizontal::<_, 3>(sum, |source, target| {
    let Ok(()) = vector_boxsum_row::<_, 3, 32>(
      source,
      target,
      |windows, out| {
        let [a, b, c] = windows.map(|block| loadu_512!(block, [i16; 32]));
        let sum_2 = _mm512_add_epi16(_mm512_add_epi16(a, b), c);
        storeu_512!(out, [i16; 32], sum_2);
      },
      i16::wrapping_add,
    );
  })?;

  area.horizontal::<_, 3>(sumsq, |source, target| {
    let Ok(()) = vector_boxsum_row::<_, 3, 16>(
      source,
      target,
      |windows, out| {
        let [a, b, c] = windows.map(|block| loadu_512!(block, [i32; 16]));
        let sum_3 = _mm512_add_epi32(_mm512_add_epi32(a, b), c);
        storeu_512!(out, [i32; 16], sum_3);
      },
      i32::wrapping_add,
    );
  })?;

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct SgrProjection<'a, Pixel> {
  pixels: &'a mut [Pixel],
  picture: RowLayout,
  residual: RowLayout,
}

#[cfg(target_arch = "x86_64")]
impl<'a, Pixel> SgrProjection<'a, Pixel> {
  fn new(
    pixels: &'a mut [Pixel],
    size: Size<u64>,
    origin: (u64, i64),
  ) -> Rav1dResult<Self> {
    let (first, step) = origin;
    let residual_stride = i64::from(RESTORATION_INTERIOR);
    let picture = RowLayout::new(size, first, step).map_err(|_layout| Rav1dError::EINVAL)?;
    let residual =
      RowLayout::new(size, 0, residual_stride).map_err(|_layout| Rav1dError::EINVAL)?;

    Ok(Self {
      pixels,
      picture,
      residual,
    })
  }

  fn rows<'r, Residual>(
    self,
    residuals: [&'r [Residual]; 2],
  ) -> Rav1dResult<impl Iterator<Item = (&'a mut [Pixel], [&'r [Residual]; 2])>> {
    let [first, second] = residuals;
    let first_rows = self
      .residual
      .rows(first)
      .map_err(|_bounds| Rav1dError::EINVAL)?;
    let second_rows = self
      .residual
      .rows(second)
      .map_err(|_bounds| Rav1dError::EINVAL)?;
    let pixels = self
      .picture
      .rows_mut(self.pixels)
      .map_err(|_bounds| Rav1dError::EINVAL)?;

    Ok(
      pixels
        .zip(first_rows)
        .zip(second_rows)
        .map(|((row, first_row), second_row)| (row, [first_row, second_row])),
    )
  }
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn project_scalar_row<const MIXED: bool, Pixel: Copy + Into<i32>, Residual: Copy + Into<i32>>(
  pixels: &mut [Pixel],
  residuals: [&[Residual]; 2],
  weights: [i32; 2],
  maximum: i32,
  store: impl Fn(i32) -> Pixel,
) -> Result<(), Never> {
  let [first, second] = residuals;
  let [first_weight, second_weight] = weights;

  for ((pixel, &primary), &secondary) in pixels.iter_mut().zip(first).zip(second) {
    let weighted = first_weight.wrapping_mul(<Residual as Into<i32>>::into(primary));
    let combined = match ::core::primitive::bool::then_some(MIXED, ()) {
      ::core::option::Option::Some(()) => weighted
        .wrapping_add(second_weight.wrapping_mul(<Residual as Into<i32>>::into(secondary))),
      ::core::option::Option::None => weighted,
    };
    let delta = combined.wrapping_add(1024).wrapping_shr(11);
    let current = <Pixel as Into<i32>>::into(*pixel);
    *pixel = store(iclip(current.wrapping_add(delta), 0, maximum));
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
fn apply_sgr8<const MIXED: bool>(
  token: Desktop64,
  picture: &PicOffset,
  size: Size<u64>,
  residuals: [&[i16]; 2],
  weights: [i32; 2],
) -> Rav1dResult {
  let Ok(width) = index(size.width);
  let Ok(height) = index(size.height);
  crate::include::dav1d::picture::with_pixel_guard_mut::<BitDepth8, _>(
    picture,
    width,
    height,
    |bytes, offset, stride| {
      let Ok(first) = fixed_index(offset);
      let Ok(pitch) = fixed_offset(stride);
      let projection = SgrProjection::new(bytes, size, (first, pitch))?;
      project_vector8::<MIXED>(token, projection, residuals, weights, 255)
    },
  )
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn project_vector8<const MIXED: bool>(
  _token: Desktop64,
  projection: SgrProjection<'_, u8>,
  residuals: [&[i16]; 2],
  weights: [i32; 2],
  maximum: i32,
) -> Rav1dResult {
  let [first_weight, second_weight] = weights;
  let primary = _mm256_set1_epi32(first_weight);
  let secondary = _mm256_set1_epi32(second_weight);
  let rounding = _mm256_set1_epi32(1024);
  let zero = _mm256_setzero_si256();
  let ceiling = _mm256_set1_epi32(maximum);
  let rows = projection.rows(residuals)?;

  for (pixels, [first, second]) in rows {
    let (blocks, tail) = pixels.as_chunks_mut::<8>();
    let (first_blocks, first_tail) = first.as_chunks::<8>();
    let (second_blocks, second_tail) = second.as_chunks::<8>();

    for ((pixel_block, first_block), second_block) in
      blocks.iter_mut().zip(first_blocks).zip(second_blocks)
    {
      let first_values = _mm256_cvtepi16_epi32(loadu_128!(first_block));
      let weighted = _mm256_mullo_epi32(first_values, primary);
      let combined = match ::core::primitive::bool::then_some(MIXED, ()) {
        ::core::option::Option::Some(()) => {
          let second_values = _mm256_cvtepi16_epi32(loadu_128!(second_block));
          _mm256_add_epi32(weighted, _mm256_mullo_epi32(second_values, secondary))
        }
        ::core::option::Option::None => weighted,
      };
      let delta = _mm256_srai_epi32::<11>(_mm256_add_epi32(combined, rounding));
      let current = _mm256_cvtepu8_epi32(loadu_64!(&*pixel_block));
      let result = _mm256_add_epi32(current, delta);
      let clamped = _mm256_max_epi32(_mm256_min_epi32(result, ceiling), zero);
      let lo = _mm256_castsi256_si128(clamped);
      let hi = _mm256_extracti128_si256::<1>(clamped);
      let packed = _mm_packus_epi16(_mm_packs_epi32(lo, hi), _mm_setzero_si128());
      let mut bytes = [0; 16];
      storeu_128!(&mut bytes, packed);
      let [a, b, c, d, e, f, g, h, ..] = bytes;
      *pixel_block = [a, b, c, d, e, f, g, h];
    }

    let Ok(()) = project_scalar_row::<MIXED, _, _>(
      tail,
      [first_tail, second_tail],
      weights,
      maximum,
      |value| {
        let Ok(sample) = console_core_number_conversion::wrapped::<_, u8>(value);
        sample
      },
    );
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[derive(Clone, Copy)]
struct SgrBackend {
  avx2: Desktop64,
  avx512: Option<Server64>,
}

#[cfg(target_arch = "x86_64")]
struct SgrStage<'a, Pixel, Residual> {
  padded: &'a RestorationPixels<Pixel>,
  residual: &'a mut RestorationDeltas<Residual>,
  extent: Size<u64>,
  radius: BoxRadius,
  strength: u32,
  maximum: i32,
}

#[cfg(target_arch = "x86_64")]
fn check_sgr_extent(extent: Size<u64>) -> Rav1dResult {
  let maximum = u64::from(RESTORATION_INTERIOR);

  match ((1..=maximum).contains(&extent.width) && (1..=64).contains(&extent.height)).then_some(()) {
    Some(()) => Ok(()),
    None => Err(Rav1dError::EINVAL),
  }
}

#[derive(Clone, Copy)]
struct SgrShape {
  extent: Size<u64>,
  radius: BoxRadius,
  strength: u32,
  bitdepth_shift: u32,
}

impl SgrShape {
  fn padded(self) -> Result<Size<u64>, console_core_never::Never> {
    Ok(Size {
      width: self.extent.width.wrapping_add(6),
      height: self.extent.height.wrapping_add(6),
    })
  }
}

struct SgrModelPass<'a, Square, Sum> {
  squares: &'a mut [Square],
  sums: &'a mut [Sum],
  shape: SgrShape,
}

fn fit_sgr_model<Square, Sum>(request: SgrModelPass<'_, Square, Sum>) -> Rav1dResult
where
  Square: Copy + TryFrom<i32>,
  Sum: Copy + TryFrom<u32>,
  i64: From<Square>,
  i32: From<Sum>,
{
  let (samples, reciprocal, stride) = match request.shape.radius {
    BoxRadius::One => (9_i32, 455_u32, 1_u64),
    BoxRadius::Two => (25, 164, 2),
  };
  let depth_shift = request.shape.bitdepth_shift;
  let square_shift = depth_shift.wrapping_mul(2);
  let square_round = 1_i64.wrapping_shl(square_shift).wrapping_shr(1);
  let sum_round = 1_i32.wrapping_shl(depth_shift).wrapping_shr(1);
  let Ok(squares) = restoration_rows_mut(request.squares);
  let Ok(sums) = restoration_rows_mut(request.sums);
  let Ok(right) = index(request.shape.extent.width.wrapping_add(4));
  let height = request.shape.extent.height.wrapping_add(2).div_ceil(stride);

  for logical in 0..height {
    let Ok(row) = index(logical.wrapping_mul(stride).wrapping_add(1));
    let square_row = squares.get_mut(row).and_then(|row_2| row_2.get_mut(2..right)).ok_or(Rav1dError::EINVAL)?;
    let sum_row = sums.get_mut(row).and_then(|row_3| row_3.get_mut(2..right)).ok_or(Rav1dError::EINVAL)?;

    for (output_square, output_sum) in square_row.iter_mut().zip(sum_row) {
      let a = i64::from(*output_square).wrapping_add(square_round).wrapping_shr(square_shift);
      let Ok(scaled) = console_core_number_conversion::wrapped::<_, i32>(a);
      let raw = i32::from(*output_sum);
      let b = raw.wrapping_add(sum_round).wrapping_shr(depth_shift);
      let variance = scaled.wrapping_mul(samples).wrapping_sub(b.wrapping_mul(b)).max(0).cast_unsigned();
      let z = variance.wrapping_mul(request.shape.strength).wrapping_add(::core::primitive::u32::wrapping_shl(1, 19_u32)).wrapping_shr(20);
      let weight = sgr_factor(z)?;
      let product = weight.wrapping_mul(raw.cast_unsigned()).wrapping_mul(reciprocal);
      let intercept = product.wrapping_add(::core::primitive::u32::wrapping_shl(1, 11_u32)).wrapping_shr(12).cast_signed();
      let model_weight = Sum::try_from(weight).map_err(|_excess| Rav1dError::EINVAL)?;
      let model_intercept = Square::try_from(intercept).map_err(|_excess| Rav1dError::EINVAL)?;
      *output_sum = model_weight;
      *output_square = model_intercept;
    }
  }

  Ok(())
}

#[derive(Clone, Copy)]
enum SgrCoefficientPlane<'a> {
  Narrow(&'a [i16]),
  Wide(&'a [i32]),
  Extended(&'a [i64]),
}

fn sgr_coefficient(plane: SgrCoefficientPlane<'_>, coordinate: u64) -> Rav1dResult<i32> {
  let Ok(position) = index(coordinate);

  match plane {
    SgrCoefficientPlane::Narrow(samples) => samples.get(position).copied().map(i32::from).ok_or(Rav1dError::EINVAL),
    SgrCoefficientPlane::Wide(samples) => samples.get(position).copied().ok_or(Rav1dError::EINVAL),
    SgrCoefficientPlane::Extended(samples) => {
      let &sample = samples.get(position).ok_or(Rav1dError::EINVAL)?;
      i32::try_from(sample).map_err(|_excess| Rav1dError::EINVAL)
    }
  }
}

fn scalar_sgr_stencil(
  plane: SgrCoefficientPlane<'_>,
  center: u64,
  stencil: SgrStencil,
) -> Rav1dResult<i32> {
  let Ok(taps) = stencil.taps();
  let mut result = 0_i32;

  for group in taps {
    let mut sum = 0_i32;

    for &offset in group.positions {
      let value = sgr_coefficient(plane, center.wrapping_add_signed(offset))?;
      sum = sum.wrapping_add(value);
    }

    result = result.wrapping_add(sum.wrapping_mul(group.weight));
  }

  Ok(result)
}

struct SgrModelOutput<'a, BD: BitDepth> {
  destination: &'a mut [BD::Coef],
  source: &'a [BD::Pixel],
  weights: SgrCoefficientPlane<'a>,
  intercepts: SgrCoefficientPlane<'a>,
  shape: SgrShape,
}

struct SgrOutputRow<'a, BD: BitDepth> {
  destination: &'a mut [BD::Coef],
  source: &'a [BD::Pixel],
  center: u64,
  weights: SgrCoefficientPlane<'a>,
  intercepts: SgrCoefficientPlane<'a>,
  stencil: SgrStencil,
}

fn sgr_write_rows<BD: BitDepth>(
  output: SgrModelOutput<'_, BD>,
  mut render: impl FnMut(SgrOutputRow<'_, BD>) -> Rav1dResult,
) -> Rav1dResult {
  let stride = SGR_STRIDE.cast_unsigned();
  let Ok(width) = index(output.shape.extent.width);
  let destination_stride = u64::from(RESTORATION_INTERIOR);

  for line in 0..output.shape.extent.height {
    let Ok(stencil) = output.shape.radius.stencil(line);
    let center = line.wrapping_add(2).wrapping_mul(stride).wrapping_add(3);
    let Ok(pixel_start) = index(center.wrapping_add(stride));
    let Ok(target_start) = index(line.wrapping_mul(destination_stride));
    let samples = output.source.get(pixel_start..).and_then(|row| row.get(..width)).ok_or(Rav1dError::EINVAL)?;
    let destination = output.destination.get_mut(target_start..).and_then(|row| row.get_mut(..width)).ok_or(Rav1dError::EINVAL)?;
    render(SgrOutputRow {
      destination, source: samples, center, weights: output.weights,
      intercepts: output.intercepts, stencil,
    })?;
  }

  Ok(())
}

fn scalar_sgr_output<BD: BitDepth>(row: SgrOutputRow<'_, BD>) -> Rav1dResult {
  let Ok((rounding, shift)) = row.stencil.rounding();

  for (column, (&pixel, output)) in (0_u64..).zip(row.source.iter().zip(row.destination)) {
    let position = row.center.wrapping_add(column);
    let intercept = scalar_sgr_stencil(row.intercepts, position, row.stencil)?;
    let weight = scalar_sgr_stencil(row.weights, position, row.stencil)?;
    let result = intercept.wrapping_sub(weight.wrapping_mul(pixel.as_::<i32>())).wrapping_add(rounding).wrapping_shr(shift);
    *output = result.as_::<BD::Coef>();
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn sgr_load_avx2(_token: Desktop64, plane: SgrCoefficientPlane<'_>, position: u64) -> Rav1dResult<__m256i> {
  Ok(match plane {
    SgrCoefficientPlane::Narrow(data) => {
      let chunk = box_view(data, position)?;
      _mm256_cvtepi16_epi32(archmage::intrinsics::x86_64::_mm_loadu_si128(chunk))
    }
    SgrCoefficientPlane::Wide(data) => {
      let chunk = box_view(data, position)?;
      archmage::intrinsics::x86_64::_mm256_loadu_si256(chunk)
    }
    SgrCoefficientPlane::Extended(data) => {
      let chunk = box_view::<_, 8>(data, position)?;
      let fitted = chunk.map(|value| {
        let Ok(coefficient) = console_core_number_conversion::wrapped::<_, i32>(value);
        coefficient
      });
      archmage::intrinsics::x86_64::_mm256_loadu_si256(&fitted)
    }
  })
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn sgr_weighted_avx2(
  token: Desktop64,
  plane: SgrCoefficientPlane<'_>,
  position: u64,
  stencil: SgrStencil,
) -> Rav1dResult<__m256i> {
  let Ok(taps) = stencil.taps();
  let zero = _mm256_setzero_si256();
  let mut result = zero;

  for group in taps {
    let mut sum = zero;

    for &offset in group.positions {
      let value = sgr_load_avx2(token, plane, position.wrapping_add_signed(offset))?;
      sum = _mm256_add_epi32(sum, value);
    }

    result = _mm256_add_epi32(result, _mm256_mullo_epi32(sum, _mm256_set1_epi32(group.weight)));
  }

  Ok(result)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn sgr_output_avx2<BD: BitDepth>(token: Desktop64, row: SgrOutputRow<'_, BD>) -> Rav1dResult {
  let (blocks, source_tail) = row.source.as_chunks::<8>();
  let (target, target_tail) = row.destination.as_chunks_mut::<8>();
  let Ok((rounding, _shift)) = row.stencil.rounding();
  let round = _mm256_set1_epi32(rounding);

  for (column, (sample, delta)) in (0_u64..).zip(blocks.iter().zip(target)) {
    let position = row.center.wrapping_add(column.wrapping_mul(8));
    let intercept = sgr_weighted_avx2(token, row.intercepts, position, row.stencil)?;
    let weight = sgr_weighted_avx2(token, row.weights, position, row.stencil)?;
    let samples = sample.map(|pixel| pixel.as_::<i32>());
    let pixels = archmage::intrinsics::x86_64::_mm256_loadu_si256(&samples);
    let product = _mm256_mullo_epi32(weight, pixels);
    let rounded = _mm256_add_epi32(_mm256_sub_epi32(intercept, product), round);
    let values = match row.stencil {
      SgrStencil::Middle => _mm256_srai_epi32::<8>(rounded),
      SgrStencil::Alternate | SgrStencil::Full => _mm256_srai_epi32::<9>(rounded),
    };
    let mut result = [0_i32; 8];
    archmage::intrinsics::x86_64::_mm256_storeu_si256(&mut result, values);
    *delta = result.map(|value| value.as_::<BD::Coef>());
  }

  let Ok(total) = fixed_index(blocks.len());
  let center = row.center.wrapping_add(total.wrapping_mul(8));
  scalar_sgr_output::<BD>(SgrOutputRow {
    source: source_tail, destination: target_tail, center, ..row
  })
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn sgr_load_avx512(_token: Server64, plane: SgrCoefficientPlane<'_>, position: u64) -> Rav1dResult<__m512i> {
  Ok(match plane {
    SgrCoefficientPlane::Narrow(data) => {
      let chunk = box_view(data, position)?;
      _mm512_cvtepi16_epi32(archmage::intrinsics::x86_64::_mm256_loadu_si256(chunk))
    }
    SgrCoefficientPlane::Wide(data) => {
      let chunk = box_view(data, position)?;
      archmage::intrinsics::x86_64::_mm512_loadu_si512(chunk)
    }
    SgrCoefficientPlane::Extended(data) => {
      let chunk = box_view::<_, 16>(data, position)?;
      let fitted = chunk.map(|value| {
        let Ok(coefficient) = console_core_number_conversion::wrapped::<_, i32>(value);
        coefficient
      });
      archmage::intrinsics::x86_64::_mm512_loadu_si512(&fitted)
    }
  })
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn sgr_weighted_avx512(
  token: Server64,
  plane: SgrCoefficientPlane<'_>,
  position: u64,
  stencil: SgrStencil,
) -> Rav1dResult<__m512i> {
  let Ok(taps) = stencil.taps();
  let zero = _mm512_setzero_si512();
  let mut result = zero;

  for group in taps {
    let mut sum = zero;

    for &offset in group.positions {
      let value = sgr_load_avx512(token, plane, position.wrapping_add_signed(offset))?;
      sum = _mm512_add_epi32(sum, value);
    }

    result = _mm512_add_epi32(result, _mm512_mullo_epi32(sum, _mm512_set1_epi32(group.weight)));
  }

  Ok(result)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn sgr_output_avx512<BD: BitDepth>(token: Server64, row: SgrOutputRow<'_, BD>) -> Rav1dResult {
  let (blocks, source_tail) = row.source.as_chunks::<16>();
  let (target, target_tail) = row.destination.as_chunks_mut::<16>();
  let Ok((rounding, _shift)) = row.stencil.rounding();
  let round = _mm512_set1_epi32(rounding);

  for (column, (sample, delta)) in (0_u64..).zip(blocks.iter().zip(target)) {
    let position = row.center.wrapping_add(column.wrapping_mul(16));
    let intercept = sgr_weighted_avx512(token, row.intercepts, position, row.stencil)?;
    let weight = sgr_weighted_avx512(token, row.weights, position, row.stencil)?;
    let samples = sample.map(|pixel| pixel.as_::<i32>());
    let pixels = archmage::intrinsics::x86_64::_mm512_loadu_si512(&samples);
    let product = _mm512_mullo_epi32(weight, pixels);
    let rounded = _mm512_add_epi32(_mm512_sub_epi32(intercept, product), round);
    let values = match row.stencil {
      SgrStencil::Middle => _mm512_srai_epi32::<8>(rounded),
      SgrStencil::Alternate | SgrStencil::Full => _mm512_srai_epi32::<9>(rounded),
    };
    let mut result = [0_i32; 16];
    archmage::intrinsics::x86_64::_mm512_storeu_si512(&mut result, values);
    *delta = result.map(|value| value.as_::<BD::Coef>());
  }

  let Ok(total) = fixed_index(blocks.len());
  let center = row.center.wrapping_add(total.wrapping_mul(16));
  scalar_sgr_output::<BD>(SgrOutputRow {
    source: source_tail, destination: target_tail, center, ..row
  })
}

#[derive(Clone, Copy)]
enum SgrComputation {
  #[cfg(test)]
  Scalar,
  Avx2(Desktop64),
  Avx512(Server64),
}

fn render_sgr_model<BD: BitDepth>(
  computation: SgrComputation,
  model: SgrModelOutput<'_, BD>,
) -> Rav1dResult {
  match computation {
    #[cfg(test)]
    SgrComputation::Scalar => sgr_write_rows(model, scalar_sgr_output::<BD>),
    SgrComputation::Avx2(token) => sgr_write_rows(model, |row| sgr_output_avx2::<BD>(token, row)),
    SgrComputation::Avx512(token) => sgr_write_rows(model, |row| sgr_output_avx512::<BD>(token, row)),
  }
}

fn compute_sgr8(computation: SgrComputation, stage: SgrStage<'_, u8, i16>) -> Rav1dResult {
  let shape = SgrShape { extent: stage.extent, radius: stage.radius, strength: stage.strength, bitdepth_shift: 0 };
  let Ok(padded) = shape.padded();
  let bounds = PaddedBox::new(padded)?;
  let mut squares: RestorationMoments<_> = [0_i32; _];
  let mut sums: RestorationMoments<_> = [0_i16; _];

  match computation {
    #[cfg(test)]
    SgrComputation::Scalar => {
      let pass = VerticalBox { input: stage.padded, sums: &mut sums, squares: &mut squares, extent: padded };
      let addition = MomentAddition { sum: i16::wrapping_add, squares: i32::wrapping_add };
      let computed = match shape.radius {
        BoxRadius::One => scalar_boxsum::<_, _, _, 3>(pass, bounds, addition),
        BoxRadius::Two => scalar_boxsum::<_, _, _, 5>(pass, bounds, addition),
      };
      computed?;
    }
    SgrComputation::Avx2(token) => sgr_moments_avx2(token, stage.padded, &mut squares, &mut sums, (bounds, padded, shape.radius))?,
    SgrComputation::Avx512(token) => sgr_moments_avx512(token, stage.padded, &mut squares, &mut sums, (bounds, padded, shape.radius))?,
  }

  fit_sgr_model(SgrModelPass { squares: &mut squares, sums: &mut sums, shape })?;
  render_sgr_model::<BitDepth8>(computation, SgrModelOutput {
    destination: stage.residual, source: stage.padded,
    weights: SgrCoefficientPlane::Narrow(&sums), intercepts: SgrCoefficientPlane::Wide(&squares), shape,
  })
}

fn compute_sgr16(computation: SgrComputation, stage: SgrStage<'_, u16, i32>) -> Rav1dResult {
  let bitdepth_shift = match stage.maximum {
    1023 => 2,
    _twelve_bits => 4,
  };
  let shape = SgrShape { extent: stage.extent, radius: stage.radius, strength: stage.strength, bitdepth_shift };
  let Ok(padded) = shape.padded();
  let mut squares: RestorationMoments<_> = [0_i64; _];
  let mut sums: RestorationMoments<_> = [0_i32; _];

  let bounds = PaddedBox::new(padded)?;
  let moment_backend = match computation {
    SgrComputation::Avx512(token) => Some(token),
    SgrComputation::Avx2(_token) => None,
    #[cfg(test)]
    SgrComputation::Scalar => None,
  };

  match moment_backend {
    Some(token) => {
      sgr_moments16_avx512(token, stage.padded, &mut squares, &mut sums, (bounds, padded, shape.radius))?;
    }
    None => {
      let pass = VerticalBox { input: stage.padded, sums: &mut sums, squares: &mut squares, extent: padded };
      let addition = MomentAddition { sum: i32::wrapping_add, squares: i64::wrapping_add };
      let computed = match shape.radius {
        BoxRadius::One => scalar_boxsum::<_, _, _, 3>(pass, bounds, addition),
        BoxRadius::Two => scalar_boxsum::<_, _, _, 5>(pass, bounds, addition),
      };

      computed?;
    }
  }

  fit_sgr_model(SgrModelPass { squares: &mut squares, sums: &mut sums, shape })?;
  render_sgr_model::<BitDepth16>(computation, SgrModelOutput {
    destination: stage.residual, source: stage.padded,
    weights: SgrCoefficientPlane::Wide(&sums), intercepts: SgrCoefficientPlane::Extended(&squares), shape,
  })
}


#[arcane]
fn sgr_moments_avx2(
  token: Desktop64,
  input: &RestorationPixels<u8>,
  squares: &mut RestorationMoments<i32>,
  sums: &mut RestorationMoments<i16>,
  (bounds, extent, radius): (PaddedBox, Size<u64>, BoxRadius),
) -> Rav1dResult {
  match radius {
    BoxRadius::One => {
      boxsum_v_avx2::<3>(token, VerticalBox {
        input, squares, sums, extent,
      })?;
      boxsum3_h_avx2(token, squares, sums, bounds)
    }
    BoxRadius::Two => {
      boxsum_v_avx2::<5>(token, VerticalBox {
        input, squares, sums, extent,
      })?;
      boxsum5_h_avx2(token, squares, sums, bounds)
    }
  }
}

#[arcane]
fn sgr_moments_avx512(
  token: Server64,
  input: &RestorationPixels<u8>,
  squares: &mut RestorationMoments<i32>,
  sums: &mut RestorationMoments<i16>,
  (bounds, extent, radius): (PaddedBox, Size<u64>, BoxRadius),
) -> Rav1dResult {
  match radius {
    BoxRadius::One => {
      boxsum_v_avx512::<3>(token, VerticalBox {
        input, squares, sums, extent,
      })?;
      boxsum3_h_avx512(token, squares, sums, bounds)
    }
    BoxRadius::Two => {
      boxsum_v_avx512::<5>(token, VerticalBox {
        input, squares, sums, extent,
      })?;
      boxsum5_h_avx512(token, squares, sums, bounds)
    }
  }
}

#[arcane]
fn sgr_moments16_avx512(
  token: Server64,
  input: &RestorationPixels<u16>,
  squares: &mut RestorationMoments<i64>,
  sums: &mut RestorationMoments<i32>,
  (bounds, extent, radius): (PaddedBox, Size<u64>, BoxRadius),
) -> Rav1dResult {
  match radius {
    BoxRadius::One => {
      boxsum_v_16bpc_avx512::<3>(token, VerticalBox {
        input, squares, sums, extent,
      })?;
      boxsum3_h_16bpc_avx512(token, squares, sums, bounds)
    }
    BoxRadius::Two => {
      boxsum_v_16bpc_avx512::<5>(token, VerticalBox {
        input, squares, sums, extent,
      })?;
      boxsum5_h_16bpc_avx512(token, squares, sums, bounds)
    }
  }
}

#[cfg(target_arch = "x86_64")]
fn filter_sgr_stage_8(backend: SgrBackend, stage: SgrStage<'_, u8, i16>) -> Rav1dResult {
  let computation = match backend.avx512 {
    Some(token) => SgrComputation::Avx512(token),
    None => SgrComputation::Avx2(backend.avx2),
  };
  compute_sgr8(computation, stage)
}

#[cfg(target_arch = "x86_64")]
fn sgr_8bpc(
  backend: SgrBackend,
  request: RestorationPadding<'_, BitDepth8>,
  selection: SgrSelection,
  params: &LooprestorationParams,
) -> Rav1dResult {
  let extent = request.size;
  check_sgr_extent(extent)?;
  let destination = request.picture;
  let mut pixels: RestorationPixels<_> = [0_u8; _];
  let mut first: RestorationDeltas<_> = [0_i16; _];
  padding::<BitDepth8>(&mut pixels, request)?;
  let sgr = params.sgr()?;
  let maximum = 255;

  let (radius, strength, weight) = match selection {
    SgrSelection::FiveByFive | SgrSelection::Mixed => (BoxRadius::Two, sgr.s0, i32::from(sgr.w0)),
    SgrSelection::ThreeByThree => (BoxRadius::One, sgr.s1, i32::from(sgr.w1)),
  };
  filter_sgr_stage_8(backend, SgrStage {
    padded: &pixels, residual: &mut first, extent, radius, strength, maximum,
  })?;

  match selection {
    SgrSelection::FiveByFive | SgrSelection::ThreeByThree => apply_sgr8::<false>(
      backend.avx2, &destination, extent, [&first, &first], [weight, 0],
    ),
    SgrSelection::Mixed => {
      let mut second: RestorationDeltas<_> = [0_i16; _];
      filter_sgr_stage_8(backend, SgrStage {
        padded: &pixels, residual: &mut second, extent, radius: BoxRadius::One,
        strength: sgr.s1, maximum,
      })?;
      apply_sgr8::<true>(
        backend.avx2, &destination, extent, [&first, &second], [weight, i32::from(sgr.w1)],
      )
    }
  }
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn boxsum5_h_16bpc_avx512(
  _token: Server64,
  sumsq: &mut RestorationMoments<i64>,
  sum: &mut RestorationMoments<i32>,
  area: PaddedBox,
) -> Rav1dResult {
  area.horizontal::<_, 5>(sum, |source, target| {
    let Ok(()) = vector_boxsum_row::<_, 5, 16>(
      source,
      target,
      |windows, out| {
        let [a, b, c, d, e] = windows.map(|block| loadu_512!(block, [i32; 16]));
        let sum_2 = _mm512_add_epi32(
          _mm512_add_epi32(_mm512_add_epi32(a, b), _mm512_add_epi32(c, d)),
          e,
        );
        storeu_512!(out, [i32; 16], sum_2);
      },
      i32::wrapping_add,
    );
  })?;

  area.horizontal::<_, 5>(sumsq, |source, target| {
    let Ok(()) = vector_boxsum_row::<_, 5, 8>(
      source,
      target,
      |windows, out| {
        let [a, b, c, d, e] = windows.map(|block| loadu_512!(block, [i64; 8]));
        let sum_3 = _mm512_add_epi64(
          _mm512_add_epi64(_mm512_add_epi64(a, b), _mm512_add_epi64(c, d)),
          e,
        );
        storeu_512!(out, [i64; 8], sum_3);
      },
      i64::wrapping_add,
    );
  })?;

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn boxsum3_h_16bpc_avx512(
  _token: Server64,
  sumsq: &mut RestorationMoments<i64>,
  sum: &mut RestorationMoments<i32>,
  area: PaddedBox,
) -> Rav1dResult {
  area.horizontal::<_, 3>(sum, |source, target| {
    let Ok(()) = vector_boxsum_row::<_, 3, 16>(
      source,
      target,
      |windows, out| {
        let [a, b, c] = windows.map(|block| loadu_512!(block, [i32; 16]));
        let sum_2 = _mm512_add_epi32(_mm512_add_epi32(a, b), c);
        storeu_512!(out, [i32; 16], sum_2);
      },
      i32::wrapping_add,
    );
  })?;

  area.horizontal::<_, 3>(sumsq, |source, target| {
    let Ok(()) = vector_boxsum_row::<_, 3, 8>(
      source,
      target,
      |windows, out| {
        let [a, b, c] = windows.map(|block| loadu_512!(block, [i64; 8]));
        let sum_3 = _mm512_add_epi64(_mm512_add_epi64(a, b), c);
        storeu_512!(out, [i64; 8], sum_3);
      },
      i64::wrapping_add,
    );
  })?;

  Ok(())
}

#[cfg(target_arch = "x86_64")]
fn apply_sgr16<const MIXED: bool>(
  token: Desktop64,
  picture: &PicOffset,
  size: Size<u64>,
  residuals: [&[i32]; 2],
  weights: [i32; 2],
  maximum: i32,
) -> Rav1dResult {
  let Ok(width) = index(size.width);
  let Ok(height) = index(size.height);
  crate::include::dav1d::picture::with_pixel_guard_mut::<BitDepth16, _>(
    picture,
    width,
    height,
    |bytes, offset, stride| {
      match ::core::primitive::bool::then_some(offset.wrapping_rem(2) == 0 && stride.wrapping_rem(2) == 0, ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => return Err(Rav1dError::EINVAL),
      }

      let pixels: &mut [u16] =
        zerocopy::FromBytes::mut_from_bytes(bytes).map_err(|_layout| Rav1dError::EINVAL)?;
      let Ok(first) = fixed_index(offset.wrapping_div(2));
      let Ok(pitch) = fixed_offset(stride.wrapping_div(2));
      let projection = SgrProjection::new(pixels, size, (first, pitch))?;
      project_vector16::<MIXED>(token, projection, residuals, weights, maximum)
    },
  )
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn project_vector16<const MIXED: bool>(
  _token: Desktop64,
  projection: SgrProjection<'_, u16>,
  residuals: [&[i32]; 2],
  weights: [i32; 2],
  maximum: i32,
) -> Rav1dResult {
  let [first_weight, second_weight] = weights;
  let primary = _mm256_set1_epi32(first_weight);
  let secondary = _mm256_set1_epi32(second_weight);
  let rounding = _mm256_set1_epi32(1024);
  let zero = _mm256_setzero_si256();
  let ceiling = _mm256_set1_epi32(maximum);
  let rows = projection.rows(residuals)?;

  for (pixels, [first, second]) in rows {
    let (blocks, tail) = pixels.as_chunks_mut::<8>();
    let (first_blocks, first_tail) = first.as_chunks::<8>();
    let (second_blocks, second_tail) = second.as_chunks::<8>();

    for ((pixel_block, first_block), second_block) in
      blocks.iter_mut().zip(first_blocks).zip(second_blocks)
    {
      let first_values = loadu_256!(first_block);
      let weighted = _mm256_mullo_epi32(first_values, primary);
      let combined = match ::core::primitive::bool::then_some(MIXED, ()) {
        ::core::option::Option::Some(()) => {
          let second_values = loadu_256!(second_block);
          _mm256_add_epi32(weighted, _mm256_mullo_epi32(second_values, secondary))
        }
        ::core::option::Option::None => weighted,
      };
      let delta = _mm256_srai_epi32::<11>(_mm256_add_epi32(combined, rounding));
      let current = _mm256_cvtepu16_epi32(loadu_128!(&*pixel_block));
      let result = _mm256_add_epi32(current, delta);
      let clamped = _mm256_max_epi32(_mm256_min_epi32(result, ceiling), zero);
      let lo = _mm256_castsi256_si128(clamped);
      let hi = _mm256_extracti128_si256::<1>(clamped);
      storeu_128!(pixel_block, _mm_packus_epi32(lo, hi));
    }

    let Ok(()) = project_scalar_row::<MIXED, _, _>(
      tail,
      [first_tail, second_tail],
      weights,
      maximum,
      |value| {
        let Ok(sample) = console_core_number_conversion::wrapped::<_, u16>(value);
        sample
      },
    );
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
fn filter_sgr_stage_16(backend: SgrBackend, stage: SgrStage<'_, u16, i32>) -> Rav1dResult {
  let computation = match backend.avx512 {
    Some(token) => SgrComputation::Avx512(token),
    None => SgrComputation::Avx2(backend.avx2),
  };
  compute_sgr16(computation, stage)
}

#[cfg(target_arch = "x86_64")]
fn sgr_16bpc(
  backend: SgrBackend,
  request: RestorationPadding<'_, BitDepth16>,
  selection: SgrSelection,
  params: &LooprestorationParams,
  maximum: i32,
) -> Rav1dResult {
  let extent = request.size;
  check_sgr_extent(extent)?;
  let destination = request.picture;
  let mut pixels: RestorationPixels<_> = [0_u16; _];
  let mut first: RestorationDeltas<_> = [0_i32; _];
  padding::<BitDepth16>(&mut pixels, request)?;
  let sgr = params.sgr()?;

  let (radius, strength, weight) = match selection {
    SgrSelection::FiveByFive | SgrSelection::Mixed => (BoxRadius::Two, sgr.s0, i32::from(sgr.w0)),
    SgrSelection::ThreeByThree => (BoxRadius::One, sgr.s1, i32::from(sgr.w1)),
  };
  filter_sgr_stage_16(backend, SgrStage {
    padded: &pixels, residual: &mut first, extent, radius, strength, maximum,
  })?;

  match selection {
    SgrSelection::FiveByFive | SgrSelection::ThreeByThree => apply_sgr16::<false>(
      backend.avx2, &destination, extent, [&first, &first], [weight, 0], maximum,
    ),
    SgrSelection::Mixed => {
      let mut second: RestorationDeltas<_> = [0_i32; _];
      filter_sgr_stage_16(backend, SgrStage {
        padded: &pixels, residual: &mut second, extent, radius: BoxRadius::One,
        strength: sgr.s1, maximum,
      })?;
      apply_sgr16::<true>(
        backend.avx2, &destination, extent, [&first, &second], [weight, i32::from(sgr.w1)], maximum,
      )
    }
  }
}

#[cfg(target_arch = "x86_64")]
pub(crate) fn lr_filter_dispatch<BD: BitDepth>(
  request: &RestorationRequest<'_, BD>,
) -> Rav1dResult<RestorationDispatch> {
  use crate::include::common::bitdepth::BPC;
  use crate::src::safe_simd::pixel_access::reinterpret_slice;

  let Ok(avx512) = crate::src::cpu::summon_avx512();
  let Ok(avx2) = crate::src::cpu::summon_avx2();
  let token = match avx2 {
    Some(proof) => proof,
    None => return Ok(RestorationDispatch::Fallback),
  };

  let RestorationPadding { picture, backup, lines, offset, size, edges } = request.padding;
  let params = request.parameters;
  let backend = SgrBackend { avx2: token, avx512 };

  let dispatched = match BD::BPC {
    BPC::BPC8 => {
      let Ok(reinterpreted) = reinterpret_slice(backup);
      let left = reinterpreted.ok_or(Rav1dError::EINVAL)?;

      match request.kernel {
        RestorationKernel::Wiener => {
          let padded = RestorationPadding { picture, backup: left, lines, offset, size, edges };
          let filtered = match avx512 {
            Some(proof) => wiener_filter7_8bpc_avx512_inner(proof, padded, params),
            None => wiener_filter7_8bpc_avx2_inner(token, padded, params),
          };
          RestorationDispatch::from(filtered)
        }
        RestorationKernel::SelfGuided(selection) => {
          let padded = RestorationPadding { picture, backup: left, lines, offset, size, edges };
          RestorationDispatch::from(sgr_8bpc(backend, padded, selection, params))
        }
      }
    }
    BPC::BPC16 => {
      let Ok(reinterpreted) = reinterpret_slice(backup);
      let left = reinterpreted.ok_or(Rav1dError::EINVAL)?;
      let maximum = request.bitdepth.into_c();

      match request.kernel {
        RestorationKernel::Wiener => {
          let padded = RestorationPadding { picture, backup: left, lines, offset, size, edges };
          let filtered = match avx512 {
            Some(proof) => wiener_filter7_16bpc_avx512_inner(proof, padded, params, maximum),
            None => wiener_filter7_16bpc_avx2_inner(token, padded, params, maximum),
          };
          RestorationDispatch::from(filtered)
        }
        RestorationKernel::SelfGuided(selection) => {
          let padded = RestorationPadding { picture, backup: left, lines, offset, size, edges };
          RestorationDispatch::from(sgr_16bpc(backend, padded, selection, params, maximum))
        }
      }
    }
  };

  Ok(dispatched)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn sgr_horizontal_windows_preserve_the_halo_and_other_rows() -> Rav1dResult {
    let dimensions = Size {
      width: 7u16,
      height: 5u16,
    };
    let area = PaddedBox::new(dimensions)?;

    const UNTOUCHED_SCRATCH: i32 = 31769;

    for (five_taps, expected) in [
      (false, [1, 2, 9, 12, 15, 6, 7]),
      (true, [1, 2, 15, 20, 25, 6, 7]),
    ] {
      let mut storage: [RestorationRow<_>; 3] = [[UNTOUCHED_SCRATCH; _]; 3];
      let [_, row, _] = &mut storage;
      let prefix = row.first_chunk_mut::<7>().ok_or(Rav1dError::EINVAL)?;
      *prefix = [1, 2, 3, 4, 5, 6, 7];

      match ::core::primitive::bool::then_some(five_taps, ()) {
        ::core::option::Option::Some(()) => area.horizontal::<_, 5>(storage.as_flattened_mut(), |row_2, out| {
          let Ok(()) = scalar_boxsum_row::<_, 5>(row_2, out, i32::wrapping_add);
        }),
        ::core::option::Option::None => area.horizontal::<_, 3>(storage.as_flattened_mut(), |row_3, out| {
          let Ok(()) = scalar_boxsum_row::<_, 3>(row_3, out, i32::wrapping_add);
        }),
      }?;

      let [first, filtered, last] = storage;
      assert!(first.iter().chain(&last).all(|&sample| sample == UNTOUCHED_SCRATCH));
      assert_eq!(filtered.first_chunk::<7>(), Some(&expected));
      assert!(filtered.iter().skip(7).all(|&sample| sample == UNTOUCHED_SCRATCH));
    }

    Ok(())
  }

  #[cfg(target_arch = "x86_64")]
  #[test]
  fn sgr_vector_windows_match_scalar_filters() -> Rav1dResult {
    let Ok(_tokens) = crate::src::safe_simd::token_test_lock();
    let Ok(avx2) = summon_avx2();
    let Ok(avx512) = crate::src::cpu::summon_avx512();
    let token = match avx2 {
      Some(token) => token,
      None => return Ok(()),
    };

    let source8 = core::array::from_fn(|offset| {
      let Ok(value) = console_core_number_conversion::wrapped::<_, u8>(
        offset.wrapping_mul(17).wrapping_add(offset.wrapping_shr(4)),
      );
      value
    });
    let source16 = core::array::from_fn(|offset| {
      let Ok(value) = console_core_number_conversion::wrapped::<_, u16>(
        offset.wrapping_mul(29).wrapping_add(offset.wrapping_shr(3)),
      );
      value.wrapping_rem(4096)
    });

    let versions = [Some(SgrComputation::Avx2(token)), avx512.map(SgrComputation::Avx512)];

    for (width, height) in [(7, 4), (15, 5), (16, 6), (31, 5), (32, 6), (33, 7), (384, 64)] {
      let extent = Size { width, height };

      for (radius, strength) in [(BoxRadius::One, 255), (BoxRadius::Two, 140)] {
        let mut scalar8: RestorationDeltas<_> = [0_i16; _];
        let mut vector8 = scalar8;
        let mut scalar16: RestorationDeltas<_> = [0_i32; _];
        let mut vector16 = scalar16;

        compute_sgr8(SgrComputation::Scalar, SgrStage {
          padded: &source8, residual: &mut scalar8, extent, radius, strength, maximum: 255,
        })?;
        compute_sgr16(SgrComputation::Scalar, SgrStage {
          padded: &source16, residual: &mut scalar16, extent, radius, strength, maximum: 4095,
        })?;

        for version in versions.into_iter().flatten() {
          compute_sgr8(version, SgrStage {
            padded: &source8, residual: &mut vector8, extent, radius, strength, maximum: 255,
          })?;
          compute_sgr16(version, SgrStage {
            padded: &source16, residual: &mut vector16, extent, radius, strength, maximum: 4095,
          })?;
          assert_eq!(scalar8, vector8, "eight-bit {width}x{height}");
          assert_eq!(scalar16, vector16, "twelve-bit {width}x{height}");
        }
      }
    }

    Ok(())
  }

  #[cfg(target_arch = "x86_64")]
  #[test]
  fn sgr_projection_checks_the_full_rectangle_before_writing() -> Rav1dResult {
    let size = Size {
      width: 9,
      height: 2,
    };
    let mut originals = [[100u8; 12]; 2];
    let mut vectors = originals;
    let mut first: [RestorationInteriorRow<_>; 2] = [[0i16; _]; 2];
    let [_, last] = &mut first;
    last.fill(2048);
    let second: [RestorationInteriorRow<_>; 2] = [[1024i16; _]; 2];
    let residuals = [first.as_flattened(), second.as_flattened()];
    let weights = [1, 1];

    let short_projection = SgrProjection::new(originals.as_flattened_mut(), size, (12, -12))?;
    let [first_plane, _] = residuals;

    assert_eq!(
      short_projection.rows([first_plane, &[0]]).map(|_rows| ()),
      Err(Rav1dError::EINVAL),
    );
    assert_eq!(originals, [[100u8; 12]; 2]);

    let scalar_projection = SgrProjection::new(originals.as_flattened_mut(), size, (12, -12))?;
    let rows = scalar_projection.rows(residuals)?;

    for (pixels, corrections) in rows {
      let Ok(()) = project_scalar_row::<true, _, _>(pixels, corrections, weights, 255, |value| {
        let Ok(sample) = console_core_number_conversion::wrapped::<_, u8>(value);
        sample
      });
    }

    let expected = [
      [102, 102, 102, 102, 102, 102, 102, 102, 102, 100, 100, 100],
      [101, 101, 101, 101, 101, 101, 101, 101, 101, 100, 100, 100],
    ];
    assert_eq!(originals, expected);

    let Ok(avx2) = summon_avx2();

    match avx2 {
      Some(token) => {
        let vector_projection = SgrProjection::new(vectors.as_flattened_mut(), size, (12, -12))?;
        project_vector8::<true>(token, vector_projection, residuals, weights, 255)?;
        assert_eq!(vectors, expected);

        let mut pixels = [[1000u16; 12]; 2];
        let wide_projection = SgrProjection::new(pixels.as_flattened_mut(), size, (12, -12))?;
        let correction = first.map(|row| row.map(i32::from));
        let both = correction.as_flattened();
        project_vector16::<false>(token, wide_projection, [both, both], [1, 0], 4095)?;
        assert_eq!(
          pixels,
          [
            [
              1001, 1001, 1001, 1001, 1001, 1001, 1001, 1001, 1001, 1000, 1000, 1000
            ],
            [1000; 12],
          ]
        );
      }
      None => {}
    }

    Ok(())
  }

  #[cfg(target_arch = "x86_64")]
  fn check_wiener<BD: BitDepth>(
    depth: BD,
    sample: impl Fn(u32) -> BD::Pixel,
    simd: impl Fn(RestorationPadding<'_, BD>, &LooprestorationParams) -> Result<(), RowLayoutError>,
  ) -> Rav1dResult {
    use crate::include::common::bitdepth::BPC;
    use crate::include::dav1d::picture::{PictureThreading, Rav1dPictureDataComponent};
    use crate::src::looprestoration::wiener_rust;
    use crate::src::with_offset::WithOffset;

    const STRIDE: u32 = 448;
    const HEIGHT: u32 = 64;
    let Ok(native_stride) = index(STRIDE);
    let original: Vec<_> = (0..STRIDE.wrapping_mul(HEIGHT)).map(sample).collect();
    let lpf = DisjointMut::new(AlignedVec64::default());
    let left = [[BD::Pixel::from(0); 4]; 64];
    let center = match BD::BPC {
      BPC::BPC8 => 0i16,
      BPC::BPC16 => 128,
    };

    for (width, height) in [(7, 4), (31, 4), (383, 64), (384, 64)] {
      for (a, threads) in [(0i16, 1), (2, 8)] {
        let outer_sum = a.wrapping_mul(2);
        let params = LooprestorationParams {
          filter: [
            [
              a,
              -6,
              11,
              center.wrapping_sub(10).wrapping_sub(outer_sum),
              11,
              -6,
              a,
              0,
            ],
            [a, -4, 6, 124i16.wrapping_sub(outer_sum), 6, -4, a, 0],
          ],
        };
        let mut scalar_pixels = AlignedVec64::new();
        let mut simd_pixels = AlignedVec64::new();

        for pixels in [&mut scalar_pixels, &mut simd_pixels] {
          pixels.resize(original.len(), BD::Pixel::from(0));
          pixels.copy_from_slice(&original);
        }

        let mut scalar = Rav1dPictureDataComponent::wrap_buf::<BD>(&mut scalar_pixels, native_stride);
        let mut vector = Rav1dPictureDataComponent::wrap_buf::<BD>(&mut simd_pixels, native_stride);
        use crate::include::dav1d::picture::PictureThreadingCounts;
        let Ok(scalar_policy) = PictureThreading::new(PictureThreadingCounts { workers: 1, tiles: 1 });
        let Ok(workers) = console_core_number_conversion::fixed_index(threads);
        let Ok(vector_policy) = PictureThreading::new(PictureThreadingCounts { workers, tiles: 4 });
        let Ok(()) = scalar.set_threading_policy(scalar_policy);
        let Ok(()) = vector.set_threading_policy(vector_policy);
        let at = |data| WithOffset { data, offset: 0 };
        let Ok(scalar_width) = fixed_index(width);
        let Ok(scalar_height) = fixed_index(height);
        wiener_rust::<BD>(RestorationPadding {
          picture: at(&scalar), backup: &left, lines: &lpf, offset: 0,
          size: Size { width: scalar_width, height: scalar_height }, edges: LrEdgeFlags::empty(),
        }, &params, depth)?;
        simd(RestorationPadding {
          picture: at(&vector), backup: &left, lines: &lpf, offset: 0,
          size: Size { width: scalar_width, height: scalar_height }, edges: LrEdgeFlags::empty(),
        }, &params).map_err(|_simd| Rav1dError::EINVAL)?;
        assert!(
          *scalar.slice::<BD, _>(..) == *vector.slice::<BD, _>(..),
          "Wiener output differs at {width}x{height}, {threads} workers",
        );
      }
    }

    Ok(())
  }

  #[test]
  #[cfg(target_arch = "x86_64")]
  fn wiener_simd_matches_scalar_with_partial_and_full_vectors() -> Rav1dResult {
    use crate::src::cpu::summon_avx512;

    let Ok(avx2) = summon_avx2();
    let Ok(avx512) = summon_avx512();
    let sample_8 = |position: u32| {
      let Ok(sample) = console_core_number_conversion::wrapped::<_, u8>(
        position.wrapping_mul(59).wrapping_rem(251)
      );
      sample
    };

    match avx2 {
      Some(token) => {
        check_wiener(
          BitDepth8::new(()),
          sample_8,
          |request, params| wiener_filter7_8bpc_avx2_inner(token, request, params),
        )?;
      }
      ::core::option::Option::None => {}
    }

    match avx512 {
      Some(token) => {
        check_wiener(
          BitDepth8::new(()),
          sample_8,
          |request, params| wiener_filter7_8bpc_avx512_inner(token, request, params),
        )?;
      }
      ::core::option::Option::None => {}
    }

    for max in [1023u16, 4095] {
      let sample_16 = |position: u32| {
        let Ok(sample) = console_core_number_conversion::wrapped::<_, u16>(
          position.wrapping_mul(197) & u32::from(max)
        );
        sample
      };

      match avx2 {
        Some(token) => {
          check_wiener(
            BitDepth16::new(max),
            sample_16,
            |request, params| wiener_filter7_16bpc_avx2_inner(token, request, params, i32::from(max)),
          )?;
        }
        ::core::option::Option::None => {}
      }

      match avx512 {
        Some(token) => {
          check_wiener(
            BitDepth16::new(max),
            sample_16,
            |request, params| wiener_filter7_16bpc_avx512_inner(token, request, params, i32::from(max)),
          )?;
        }
        ::core::option::Option::None => {}
      }
    }

    Ok(())
  }

  #[test]
  fn test_rest_unit_stride() {
    assert_eq!(
      REST_UNIT_STRIDE,
      u16::wrapping_mul(256, 3)
        .wrapping_div(2)
        .wrapping_add(3)
        .wrapping_add(3)
    );
    assert_eq!(REST_UNIT_STRIDE, 390);
  }

  #[test]
  fn test_max_restoration_width() {
    assert_eq!(RESTORATION_INTERIOR, 384);
  }
}
