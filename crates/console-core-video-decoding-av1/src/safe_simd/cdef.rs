//! Safe SIMD implementations for CDEF (Constrained Directional Enhancement Filter)
//!
//! CDEF applies direction-dependent filtering to remove coding artifacts
//! while preserving edges.

#![deny(unsafe_code)]


#[cfg(target_arch = "x86_64")]
use archmage::{Desktop64, Server64, arcane, rite};
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

#[cfg(any(target_arch = "x86_64", target_arch = "wasm32"))]
use std::ffi::c_int;
#[cfg(target_arch = "x86_64")]
use std::ffi::c_uint;

#[cfg(any(target_arch = "x86_64", target_arch = "wasm32"))]
use crate::include::common::bitdepth::BitDepth;
#[cfg(any(target_arch = "x86_64", target_arch = "wasm32"))]
use crate::include::dav1d::picture::PicOffset;
#[cfg(target_arch = "x86_64")]
use crate::src::cdef::{CdefClip, CdefDispatch, CdefFilter, CdefStencil, cdef_padding, cdef_stencil};
#[cfg(target_arch = "x86_64")]
use crate::src::error::{Rav1dError, Rav1dError::EINVAL, Rav1dResult};
#[cfg(target_arch = "x86_64")]
use console_core_geometry::Size;
#[cfg(target_arch = "x86_64")]
use console_core_number_conversion::{fixed_index, fixed_offset, index, wrapped};
#[cfg(any(target_arch = "x86_64", target_arch = "wasm32"))]
use crate::src::strided::Strided as _;

#[cfg(target_arch = "x86_64")]
#[rite]
#[must_use]
fn constrain_128(_t: Desktop64, diff: __m128i, threshold: __m128i, shift: __m128i) -> Result<__m128i, console_core_never::Never> {
  let zero = _mm_setzero_si128();
  let adiff = _mm_abs_epi16(diff);
  let shifted = _mm_sra_epi16(adiff, shift);
  let term = _mm_sub_epi16(threshold, shifted);
  let max_term = _mm_max_epi16(term, zero);
  let result_abs = _mm_min_epi16(adiff, max_term);
  let sign_mask = _mm_cmpgt_epi16(zero, diff);
  let neg_result = _mm_sub_epi16(zero, result_abs);
  Ok(_mm_blendv_epi8(result_abs, neg_result, sign_mask))
}

#[cfg(target_arch = "x86_64")]
#[rite]
#[must_use]
fn constrain_avx512(_t: Server64, diff: __m512i, threshold: __m512i, shift: __m128i) -> Result<__m512i, console_core_never::Never> {
  let zero = _mm512_setzero_si512();
  let adiff = _mm512_abs_epi16(diff);
  let shifted = _mm512_sra_epi16(adiff, shift);
  let term = _mm512_sub_epi16(threshold, shifted);
  let max_term = _mm512_max_epi16(term, zero);
  let result_abs = _mm512_min_epi16(adiff, max_term);
  let neg_result = _mm512_sub_epi16(zero, result_abs);

  let sign: __mmask32 = _mm512_cmpgt_epi16_mask(zero, diff);
  Ok(_mm512_mask_blend_epi16(sign, result_abs, neg_result))
}

#[cfg(target_arch = "x86_64")]
struct CdefX86Block<'a> {
  pixels: &'a [u16; 144],
  stencil: CdefStencil,
  extent: Size<u8>,
  destination: PicOffset<'a>,
  maximum: i16,
}

#[cfg(target_arch = "x86_64")]
struct CdefOutput<'a, T> {
  pixels: &'a mut [T],
  origin: u64,
  stride: i64,
  width: u8,
}

#[cfg(target_arch = "x86_64")]
impl<T> CdefOutput<'_, T> {
  fn row(&mut self, row: u8) -> Rav1dResult<&mut [T]> {
    let displacement = i64::from(row).wrapping_mul(self.stride);
    let start = self.origin.wrapping_add_signed(displacement);
    let Ok(offset) = index(start);
    let Ok(width) = index(self.width);
    self.pixels.get_mut(offset..).and_then(|tail| tail.get_mut(..width)).ok_or(EINVAL)
  }
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
fn cdef_load8(pixels: &[u16; 144], column: i32) -> Rav1dResult<&[u16; 8]> {
  let position = u16::try_from(column).map_err(|_outside_padding| EINVAL)?;
  let Ok(offset) = index(position);
  pixels.get(offset..).and_then(<[_]>::first_chunk).ok_or(EINVAL)
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn cdef_load_sse(_token: Desktop64, pixels: &[u16; 144], column: i32) -> Rav1dResult<__m128i> {
  let samples = cdef_load8(pixels, column)?;
  Ok(archmage::intrinsics::x86_64::_mm_loadu_si128(samples))
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn cdef_load4(_token: Server64, pixels: &[u16; 144], column: i32) -> Rav1dResult<__m512i> {
  let first_samples = cdef_load8(pixels, column)?;
  let first = archmage::intrinsics::x86_64::_mm_loadu_si128(first_samples);
  let second_samples = cdef_load8(pixels, column.wrapping_add(12))?;
  let second = archmage::intrinsics::x86_64::_mm_loadu_si128(second_samples);
  let third_samples = cdef_load8(pixels, column.wrapping_add(24))?;
  let third = archmage::intrinsics::x86_64::_mm_loadu_si128(third_samples);
  let fourth_samples = cdef_load8(pixels, column.wrapping_add(36))?;
  let fourth = archmage::intrinsics::x86_64::_mm_loadu_si128(fourth_samples);
  let first_pair = _mm512_inserti32x4::<1>(_mm512_castsi128_si512(first), second);
  let extended = _mm512_inserti32x4::<2>(first_pair, third);
  Ok(_mm512_inserti32x4::<3>(extended, fourth))
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn cdef_store8(
  _token: Desktop64,
  destination: &mut [u8],
  vector: __m128i,
) -> Rav1dResult {
  let packed = _mm_packus_epi16(vector, _mm_setzero_si128());

  match destination {
    [a, b, c, d] => {
      let [first, second, third, fourth] = _mm_cvtsi128_si32(packed).to_le_bytes();
      *a = first;
      *b = second;
      *c = third;
      *d = fourth;
    }
    [a, b, c, d, e, f, g, h] => {
      let [first, second, third, fourth, fifth, sixth, seventh, eighth] =
        _mm_cvtsi128_si64(packed).to_le_bytes();
      *a = first;
      *b = second;
      *c = third;
      *d = fourth;
      *e = fifth;
      *f = sixth;
      *g = seventh;
      *h = eighth;
    }
    _incorrect_width => return Err(EINVAL),
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn cdef_store16(
  _token: Desktop64,
  destination: &mut [u16],
  vector: __m128i,
) -> Rav1dResult {
  let mut result = [0_u16; 8];
  archmage::intrinsics::x86_64::_mm_storeu_si128(&mut result, vector);
  let selected = result.get(..destination.len()).ok_or(EINVAL)?;

  for (target, pixel) in destination.iter_mut().zip(selected) {
    *target = *pixel;
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn cdef_filter_sse8(token: Desktop64, block: &CdefX86Block<'_>) -> Rav1dResult {
  use crate::include::common::bitdepth::BitDepth8;

  let Ok(width) = index(block.extent.width);
  let Ok(height) = index(block.extent.height);
  crate::include::dav1d::picture::with_pixel_guard_mut::<BitDepth8, _>(
    &block.destination, width, height, |pixels, first, pitch| {
      let Ok(origin) = fixed_index(first);
      let Ok(stride) = fixed_offset(pitch);
      let mut output = CdefOutput { pixels, origin, stride, width: block.extent.width };

      for row in 0..block.extent.height {
        let center = i32::from(row).wrapping_mul(12).wrapping_add(26);
        let filtered = cdef_filter_sse_row(token, block, center)?;
        let destination = output.row(row)?;
        cdef_store8(token, destination, filtered)?;
      }

      Ok(())
    },
  )
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn cdef_filter_zmm8(token: Server64, block: &CdefX86Block<'_>) -> Rav1dResult {
  use crate::include::common::bitdepth::BitDepth8;

  let Ok(width) = index(block.extent.width);
  let Ok(height) = index(block.extent.height);
  crate::include::dav1d::picture::with_pixel_guard_mut::<BitDepth8, _>(
    &block.destination, width, height, |pixels, first, pitch| {
      let Ok(origin) = fixed_index(first);
      let Ok(stride) = fixed_offset(pitch);
      let mut output = CdefOutput { pixels, origin, stride, width: block.extent.width };
      let base = token.v3();

      for row in (0..block.extent.height).step_by(4) {
        let center = i32::from(row).wrapping_mul(12).wrapping_add(26);
        let filtered = cdef_filter_zmm_rows(token, block, center)?;
        let lanes = [
          _mm512_extracti32x4_epi32::<0>(filtered),
          _mm512_extracti32x4_epi32::<1>(filtered),
          _mm512_extracti32x4_epi32::<2>(filtered),
          _mm512_extracti32x4_epi32::<3>(filtered),
        ];

        for (number, lane) in (row..row.wrapping_add(4)).zip(lanes) {
          let destination = output.row(number)?;
          cdef_store8(base, destination, lane)?;
        }
      }

      Ok(())
    },
  )
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn cdef_filter_sse16(token: Desktop64, block: &CdefX86Block<'_>) -> Rav1dResult {
  use crate::include::common::bitdepth::BitDepth16;

  let Ok(width) = index(block.extent.width);
  let Ok(height) = index(block.extent.height);
  let zero = _mm_setzero_si128();
  let maximum = _mm_set1_epi16(block.maximum);
  crate::include::dav1d::picture::with_pixel_guard_mut::<BitDepth16, _>(
    &block.destination, width, height, |bytes, first, pitch| {
      let pixels = zerocopy::FromBytes::mut_from_bytes(bytes).map_err(|_unaligned| EINVAL)?;
      let Ok(origin) = fixed_index(first);
      let Ok(stride) = fixed_offset(pitch);
      let mut output = CdefOutput {
        pixels,
        origin: origin.wrapping_div(2),
        stride: stride.wrapping_div(2),
        width: block.extent.width,
      };

      for row in 0..block.extent.height {
        let center = i32::from(row).wrapping_mul(12).wrapping_add(26);
        let filtered = cdef_filter_sse_row(token, block, center)?;
        let valid = match block.stencil.clip {
          CdefClip::OriginalRange => filtered,
          CdefClip::NoClip => _mm_min_epi16(_mm_max_epi16(filtered, zero), maximum),
        };
        let destination = output.row(row)?;
        cdef_store16(token, destination, valid)?;
      }

      Ok(())
    },
  )
}


#[cfg(target_arch = "x86_64")]
#[rite]
fn cdef_filter_sse_row(
  token: Desktop64,
  block: &CdefX86Block<'_>,
  center: i32,
) -> Rav1dResult<__m128i> {
  let zero = _mm_setzero_si128();
  let pixels = cdef_load_sse(token, block.pixels, center)?;
  let mut sum = zero;
  let mut minimum = pixels;
  let mut maximum = pixels;

  for group in block.stencil.groups.iter().flatten() {
    let Ok(threshold) = wrapped::<_, i16>(group.attenuation.threshold);
    let thresh = _mm_set1_epi16(threshold);
    let shift = _mm_cvtsi32_si128(group.attenuation.shift.cast_signed());

    for tap in group.taps {
      let first = cdef_load_sse(token, block.pixels, center.wrapping_add(tap.distance))?;
      let second = cdef_load_sse(token, block.pixels, center.wrapping_sub(tap.distance))?;
      let Ok(first_delta) = constrain_128(token, _mm_sub_epi16(first, pixels), thresh, shift);
      let Ok(second_delta) = constrain_128(token, _mm_sub_epi16(second, pixels), thresh, shift);
      let Ok(weight) = wrapped::<_, i16>(tap.weight);
      let boosted = _mm_mullo_epi16(_mm_set1_epi16(weight), _mm_add_epi16(first_delta, second_delta));
      sum = _mm_add_epi16(sum, boosted);

      match block.stencil.clip {
        CdefClip::OriginalRange => {
          minimum = _mm_min_epu16(minimum, _mm_min_epu16(first, second));
          maximum = _mm_max_epi16(maximum, _mm_max_epi16(first, second));
        }
        CdefClip::NoClip => {}
      }
    }
  }

  let negative = _mm_cmpgt_epi16(zero, sum);
  let rounded = _mm_add_epi16(_mm_add_epi16(sum, negative), _mm_set1_epi16(8));
  let result = _mm_add_epi16(pixels, _mm_srai_epi16::<4>(rounded));

  Ok(match block.stencil.clip {
    CdefClip::OriginalRange => _mm_min_epi16(_mm_max_epi16(result, minimum), maximum),
    CdefClip::NoClip => result,
  })
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn cdef_filter_zmm_rows(
  token: Server64,
  block: &CdefX86Block<'_>,
  center: i32,
) -> Rav1dResult<__m512i> {
  let zero = _mm512_setzero_si512();
  let pixels = cdef_load4(token, block.pixels, center)?;
  let mut sum = zero;
  let mut minimum = pixels;
  let mut maximum = pixels;

  for group in block.stencil.groups.iter().flatten() {
    let Ok(threshold) = wrapped::<_, i16>(group.attenuation.threshold);
    let thresh = _mm512_set1_epi16(threshold);
    let shift = _mm_cvtsi32_si128(group.attenuation.shift.cast_signed());

    for tap in group.taps {
      let first = cdef_load4(token, block.pixels, center.wrapping_add(tap.distance))?;
      let second = cdef_load4(token, block.pixels, center.wrapping_sub(tap.distance))?;
      let Ok(first_delta) = constrain_avx512(token, _mm512_sub_epi16(first, pixels), thresh, shift);
      let Ok(second_delta) = constrain_avx512(token, _mm512_sub_epi16(second, pixels), thresh, shift);
      let Ok(weight) = wrapped::<_, i16>(tap.weight);
      let boosted = _mm512_mullo_epi16(_mm512_set1_epi16(weight), _mm512_add_epi16(first_delta, second_delta));
      sum = _mm512_add_epi16(sum, boosted);

      match block.stencil.clip {
        CdefClip::OriginalRange => {
          minimum = _mm512_min_epu16(minimum, _mm512_min_epu16(first, second));
          maximum = _mm512_max_epi16(maximum, _mm512_max_epi16(first, second));
        }
        CdefClip::NoClip => {}
      }
    }
  }

  let negative = _mm512_mask_blend_epi16(_mm512_cmpgt_epi16_mask(zero, sum), zero, _mm512_set1_epi16(-1));
  let rounded = _mm512_add_epi16(_mm512_add_epi16(sum, negative), _mm512_set1_epi16(8));
  let result = _mm512_add_epi16(pixels, _mm512_srai_epi16::<4>(rounded));

  Ok(match block.stencil.clip {
    CdefClip::OriginalRange => _mm512_min_epi16(_mm512_max_epi16(result, minimum), maximum),
    CdefClip::NoClip => result,
  })
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn accumulate_direction8(
  _token: Desktop64,
  sums: &mut [i16; 16],
  start: u8,
  row: __m128i,
) -> Rav1dResult {
  use super::pixel_access::{loadu_128, storeu_128};
  use console_core_number_conversion::index;

  let Ok(offset) = index(start);
  let remaining = sums.get_mut(offset..).ok_or(Rav1dError::EINVAL)?;
  let chunk = remaining.first_chunk_mut::<8>().ok_or(Rav1dError::EINVAL)?;
  let current = loadu_128!(&*chunk);
  storeu_128!(chunk, _mm_add_epi16(current, row));

  Ok(())
}

#[cfg(target_arch = "x86_64")]
fn accumulate_direction4(sums: &mut [i16; 16], start: u8, samples: [i16; 4]) -> Rav1dResult {
  use console_core_number_conversion::index;

  let Ok(offset) = index(start);
  let remaining = sums.get_mut(offset..).ok_or(Rav1dError::EINVAL)?;
  let chunk = remaining.first_chunk_mut::<4>().ok_or(Rav1dError::EINVAL)?;

  for (sum, sample) in chunk.iter_mut().zip(samples) {
    *sum = sum.wrapping_add(sample);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn cdef_find_direction(
  token: Desktop64,
  variance: &mut c_uint,
  read_row: impl Fn(u8) -> Rav1dResult<__m128i>,
) -> Rav1dResult<c_int> {
  use super::pixel_access::storeu_128;
  use crate::src::cdef::{cdef_best_direction, cdef_line_costs, cdef_widen_direction_lines};

  let mut diagonal_0 = [0_i16; 16];
  let mut diagonal_1 = [0_i16; 16];
  let mut offset_0 = [0_i16; 16];
  let mut offset_1 = [0_i16; 16];
  let mut offset_2 = [0_i16; 16];
  let mut offset_3 = [0_i16; 16];
  let mut horizontal = [0_i16; 8];
  let mut vertical = _mm_setzero_si128();

  for (y, sum) in (0..8_u8).zip(&mut horizontal) {
    let row = read_row(y)?;
    let mut pixels = [0_i16; 8];
    storeu_128!(&mut pixels, row);
    *sum = pixels.into_iter().fold(0_i16, i16::wrapping_add);
    vertical = _mm_add_epi16(vertical, row);

    let reverse = _mm_shuffle_epi8(
      row,
      _mm_set_epi8(1, 0, 3, 2, 5, 4, 7, 6, 9, 8, 11, 10, 13, 12, 15, 14),
    );
    let half_y = y.wrapping_shr(1);
    accumulate_direction8(token, &mut diagonal_0, y, row)?;
    accumulate_direction8(token, &mut diagonal_1, y, reverse)?;
    accumulate_direction8(token, &mut offset_2, 3_u8.wrapping_sub(half_y), row)?;
    accumulate_direction8(token, &mut offset_3, half_y, row)?;

    let mut pairs = [0_i16; 8];
    storeu_128!(&mut pairs, _mm_hadd_epi16(row, row));
    let [first, second, third, fourth, ..] = pairs;
    accumulate_direction4(&mut offset_0, y, [first, second, third, fourth])?;
    accumulate_direction4(&mut offset_1, y, [fourth, third, second, first])?;
  }

  let mut vertical_sums = [0; 8];
  storeu_128!(&mut vertical_sums, vertical);
  let Ok(line_0) = cdef_widen_direction_lines(diagonal_0);
  let Ok(line_1) = cdef_widen_direction_lines(offset_0);
  let Ok(line_2) = cdef_widen_direction_lines(horizontal);
  let Ok(line_3) = cdef_widen_direction_lines(offset_1);
  let Ok(line_4) = cdef_widen_direction_lines(diagonal_1);
  let Ok(line_5) = cdef_widen_direction_lines(offset_2);
  let Ok(line_6) = cdef_widen_direction_lines(vertical_sums);
  let Ok(line_7) = cdef_widen_direction_lines(offset_3);
  let Ok(costs) = cdef_line_costs([line_0, line_1, line_2, line_3, line_4, line_5, line_6, line_7]);

  cdef_best_direction(costs, variance)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn cdef_find_dir_simd_8bpc(
  token: Desktop64,
  img: PicOffset,
  variance: &mut c_uint,
) -> Rav1dResult<c_int> {
  use super::pixel_access::loadu_128;
  use crate::include::common::bitdepth::BitDepth8;
  use console_core_number_conversion::{fixed_offset, wrapped_offset};

  let sub128 = _mm_set1_epi16(128);
  let Ok(stride) = fixed_offset(match img.pixel_stride::<BitDepth8>() {
    Ok(native) => native,
  });
  cdef_find_direction(token, variance, |y| {
    let Ok(offset) = wrapped_offset(i64::from(y).wrapping_mul(stride));
    let row_img = img + offset;
    let row_slice = row_img.slice::<BitDepth8>(8);
    let mut row_bytes = [0u8; 16];
    let first = row_bytes.first_chunk_mut::<8>().ok_or(Rav1dError::EINVAL)?;
    let pixels = row_slice.first_chunk::<8>().ok_or(Rav1dError::EINVAL)?;
    *first = *pixels;
    let row = loadu_128!(&row_bytes);
    Ok(_mm_sub_epi16(_mm_cvtepu8_epi16(row), sub128))
  })
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn cdef_find_dir_simd_16bpc(
  token: Desktop64,
  img: PicOffset,
  variance: &mut c_uint,
  bitdepth: u8,
) -> Rav1dResult<c_int> {
  use super::pixel_access::loadu_128;
  use crate::include::common::bitdepth::BitDepth16;
  use console_core_number_conversion::{fixed_offset, wrapped_offset};

  let shift = i32::from(bitdepth.wrapping_sub(8));
  let shift_v = _mm_cvtsi32_si128(shift);
  let sub128 = _mm_set1_epi16(128);
  let Ok(stride) = fixed_offset(match img.pixel_stride::<BitDepth16>() {
    Ok(native) => native,
  });

  cdef_find_direction(token, variance, |y| {
    let Ok(offset) = wrapped_offset(i64::from(y).wrapping_mul(stride));
    let row_img = img + offset;
    let row_slice = row_img.slice::<BitDepth16>(8);
    let row = row_slice.first_chunk::<8>().ok_or(Rav1dError::EINVAL)?;
    let raw = loadu_128!(row);
    Ok(_mm_sub_epi16(_mm_srl_epi16(raw, shift_v), sub128))
  })
}

#[cfg(all(test, target_arch = "x86_64"))]
mod tests_avx512 {
  use super::*;
  use crate::src::safe_simd::pixel_access::{loadu_128, loadu_512, storeu_128, storeu_512};

  #[arcane]
  #[must_use]
  fn run_constrain_avx512(
    t: Server64,
    diff: &[i16; 32],
    threshold: i16,
    shift: i32,
  ) -> Result<[i16; 32], console_core_never::Never> {
    let diff_v = loadu_512!(diff, [i16; 32]);
    let thr_v = _mm512_set1_epi16(threshold);
    let shift_v = _mm_cvtsi32_si128(shift);
    let Ok(r) = constrain_avx512(t, diff_v, thr_v, shift_v);
    let mut out = [0i16; 32];
    storeu_512!(&mut out, [i16; 32], r);
    Ok(out)
  }

  #[arcane]
  #[must_use]
  fn run_constrain_128(t: Desktop64, diff: &[i16; 8], threshold: i16, shift: i32) -> Result<[i16; 8], console_core_never::Never> {
    let diff_v = loadu_128!(diff, [i16; 8]);
    let thr_v = _mm_set1_epi16(threshold);
    let shift_v = _mm_cvtsi32_si128(shift);
    let Ok(r) = constrain_128(t, diff_v, thr_v, shift_v);
    let mut out = [0i16; 8];
    storeu_128!(&mut out, r);
    Ok(out)
  }

  fn constrain_ref(
    [d, threshold]: [i16; 2],
    shift: i32,
  ) -> core::result::Result<i16, console_core_never::Never> {
    Ok({
      let adiff = d.wrapping_abs();
      let shifted = i32::wrapping_shr(
        {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(adiff);
          numeric_value
        },
        {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(shift);
          numeric_value
        },
      );
      let Ok(term) = console_core_number_conversion::wrapped::<_, i16>(
        ({
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(threshold);
          numeric_value
        })
        .wrapping_sub(shifted),
      );
      let max_term = term.max(0);
      let result_abs = adiff.min(max_term);

      match ::core::primitive::bool::then_some(d < 0, ()) {
        ::core::option::Option::Some(()) => (0i16).wrapping_sub(result_abs),
        ::core::option::Option::None => result_abs,
      }
    })
  }

  struct Lcg(u64);

  impl Lcg {
    fn next_u32(&mut self) -> core::result::Result<u32, console_core_never::Never> {
      Ok({
        self.0 = self
          .0
          .wrapping_mul(6364136223846793005)
          .wrapping_add(1442695040888963407);

        {
          let Ok(numeric_value) =
            console_core_number_conversion::wrapped::<_, u32>(u64::wrapping_shr(self.0, 33_u32));
          numeric_value
        }
      })
    }
  }

  #[test]
  fn avx512_constrain_matches_avx2_and_scalar() {
    let Ok(summon_avx512) = crate::src::cpu::summon_avx512();
    let Ok(summon_avx2) = crate::src::cpu::summon_avx2();
    let (v4, v3) = match (summon_avx512, summon_avx2) {
      (Some(a), Some(b)) => (a, b),
      _ => return,
    };

    let mut rng = Lcg(0x1234_5678_9abc_def0);

    for _ in 0..4096 {
      let Ok(threshold) = console_core_number_conversion::wrapped::<_, i16>(u32::wrapping_add(
        1,
        ::core::primitive::u32::wrapping_rem(
          {
            let Ok(explicit_value) = rng.next_u32();
            explicit_value
          },
          63,
        ),
      ));
      let Ok(shift) =
        console_core_number_conversion::wrapped::<_, i32>(::core::primitive::u32::wrapping_rem(
          {
            let Ok(explicit_value) = rng.next_u32();
            explicit_value
          },
          8,
        ));

      let mut diff = [0i16; 32];

      for d in diff.iter_mut() {
        let Ok(r) = rng.next_u32();
        *d = match ::core::primitive::u32::wrapping_rem(r, 8) {
          0 => 0x7fff,
          1 => -0x8000,
          2 => {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(0x8001u16);
            numeric_value
          }
          _ => i16::wrapping_sub(
            {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(
                ::core::primitive::u32::wrapping_rem(r, 1023),
              );
              numeric_value
            },
            511,
          ),
        };
      }

      let Ok(out512) = run_constrain_avx512(v4, &diff, threshold, shift);

      for (i, (&d, actual)) in diff.iter().zip(out512).enumerate() {
        assert_eq!(
          actual,
          ({
            let Ok(explicit_value) = constrain_ref([d, threshold], shift);
            explicit_value
          }),
          "v4 vs scalar mismatch: diff={d} thr={threshold} shift={shift} lane={i}"
        );
      }

      for (chunk, (d8, expected)) in diff
        .as_chunks::<8>()
        .0
        .iter()
        .zip(out512.as_chunks::<8>().0)
        .enumerate()
      {
        let Ok(out128) = run_constrain_128(v3, d8, threshold, shift);
        assert_eq!(
          expected, &out128,
          "v4 vs avx2 mismatch: thr={threshold} shift={shift} chunk={chunk}"
        );
      }
    }
  }
}

#[cfg(target_arch = "wasm32")]
use crate::include::common::intops::iclip;
#[cfg(target_arch = "wasm32")]
use crate::include::common::bitdepth::LeftPixelRow2px;
#[cfg(target_arch = "wasm32")]
use crate::src::cdef::{CdefBottom, CdefEdgeFlags, CdefTop};
#[cfg(target_arch = "wasm32")]
use crate::src::safe_simd::pixel_access::Flex;
#[cfg(target_arch = "wasm32")]
use crate::src::pic_or_buf::PicOrBuf;
#[cfg(target_arch = "wasm32")]
use crate::src::with_offset::WithOffset;
#[cfg(target_arch = "wasm32")]
use crate::src::tables::dav1d_cdef_directions;
#[cfg(target_arch = "wasm32")]
use core::cmp;


#[cfg(target_arch = "wasm32")]
pub(super) const TMP_STRIDE: usize = 12;

#[cfg(target_arch = "wasm32")]
#[inline(always)]
fn constrain_scalar(
  diff: i32,
  threshold: c_int,
  shift: c_int,
) -> core::result::Result<i32, console_core_never::Never> {
  Ok({
    let adiff = diff.abs();
    let term = i32::wrapping_sub(
      threshold,
      i32::wrapping_shr(adiff, {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(shift);
        numeric_value
      }),
    );
    let max_term = cmp::max(0, term);
    let result = cmp::min(adiff, max_term);

    match ::core::primitive::bool::then_some(diff < 0, ()) {
      ::core::option::Option::Some(()) => (result).wrapping_neg(),
      ::core::option::Option::None => result,
    }
  })
}

#[cfg(target_arch = "wasm32")]
pub(super) fn cdef_filter_block_scalar_8bpc(
  tmp: &[u16],
  tmp_offset: usize,
  dst: PicOffset,
  _stride: isize,
  w: usize,
  h: usize,
  dir: usize,
  pri_strength: c_int,
  sec_strength: c_int,
  damping: c_int,
) -> core::result::Result<(), console_core_never::Never> {
  {
    use crate::include::common::bitdepth::BitDepth8;

    let tmp_2 = tmp.flex();

    let _ = crate::include::dav1d::picture::with_pixel_guard_mut::<BitDepth8, _>(
      &dst,
      w,
      h,
      |bytes, offset, stride| {
        let mut p = bytes.flex_mut();

        match ::core::primitive::bool::then_some(pri_strength != 0, ()) {
          ::core::option::Option::Some(()) => {
            let pri_tap = i32::wrapping_sub(4, pri_strength & 1);
            let Ok(ilog2) =
              console_core_number_conversion::wrapped::<_, i32>(pri_strength.ilog2());
            let pri_shift = cmp::max(0, i32::wrapping_sub(damping, ilog2));

            match ::core::primitive::bool::then_some(sec_strength != 0, ()) {
              ::core::option::Option::Some(()) => {
                let Ok(sec_strength_ilog2) =
                  console_core_number_conversion::wrapped::<_, i32>(sec_strength.ilog2());
                let sec_shift = i32::wrapping_sub(damping, sec_strength_ilog2);

                for y in 0..h {
                  let Ok(row_base) = console_core_number_conversion::wrapped_offset(
                    tmp_offset.wrapping_add(y.wrapping_mul(TMP_STRIDE)),
                  );
                  let Ok(row_off) = console_core_number_conversion::wrapped_index(
                    ({
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped_offset(offset);
                      numeric_value
                    })
                    .wrapping_add(
                      ({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(y);
                        numeric_value
                      })
                      .wrapping_mul(stride),
                    ),
                  );

                  for x in 0..w {
                    let Ok(px) = console_core_number_conversion::wrapped::<_, i32>(
                      p[row_off.wrapping_add(x)],
                    );
                    let mut sum = 0i32;
                    let mut max = px;
                    let mut min = px;
                    let ::core::result::Result::Ok(base) =
                      ::console_core_number_conversion::fixed_offset(row_base.wrapping_add({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(x);
                        numeric_value
                      }));

                    let mut pri_tap_k = pri_tap;

                    for k in 0..2 {
                      let Ok(off1) = console_core_number_conversion::wrapped_offset(
                        dav1d_cdef_directions[dir.wrapping_add(2)][k],
                      );

                      let Ok(p0) = console_core_number_conversion::wrapped::<_, i32>({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(
                          tmp_2[{
                            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                              (match ::console_core_number_conversion::wrapped_offset(base) {
                                ::core::result::Result::Ok(native_position) => native_position,
                              })
                              .wrapping_add(off1),
                            );
                            numeric_value
                          }],
                        );
                        numeric_value
                      });
                      let Ok(p1) = console_core_number_conversion::wrapped::<_, i32>({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(
                          tmp_2[{
                            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                              (match ::console_core_number_conversion::wrapped_offset(base) {
                                ::core::result::Result::Ok(native_position) => native_position,
                              })
                              .wrapping_sub(off1),
                            );
                            numeric_value
                          }],
                        );
                        numeric_value
                      });

                      {
                        let numeric_amount = i32::wrapping_mul(pri_tap_k, {
                          let Ok(explicit_value) =
                            constrain_scalar(i32::wrapping_sub(p0, px), pri_strength, pri_shift);
                          explicit_value
                        });
                        let numeric_target = &mut (sum);
                        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                      };

                      {
                        let numeric_amount = i32::wrapping_mul(pri_tap_k, {
                          let Ok(explicit_value) =
                            constrain_scalar(i32::wrapping_sub(p1, px), pri_strength, pri_shift);
                          explicit_value
                        });
                        let numeric_target = &mut (sum);
                        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                      };

                      pri_tap_k = pri_tap_k & 3 | 2;

                      min = {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, i32>(cmp::min(
                            cmp::min(
                              {
                                let Ok(numeric_value) =
                                  console_core_number_conversion::wrapped::<_, u32>(p0);
                                numeric_value
                              },
                              {
                                let Ok(numeric_value) =
                                  console_core_number_conversion::wrapped::<_, u32>(p1);
                                numeric_value
                              },
                            ),
                            {
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped::<_, u32>(min);
                              numeric_value
                            },
                          ));
                        numeric_value
                      };

                      max = cmp::max(cmp::max(p0, p1), max);

                      let Ok(off2) = console_core_number_conversion::wrapped_offset(
                        dav1d_cdef_directions[dir.wrapping_add(4)][k],
                      );
                      let Ok(off3) = console_core_number_conversion::wrapped_offset(
                        dav1d_cdef_directions[dir.wrapping_add(0)][k],
                      );
                      let Ok(s0) = console_core_number_conversion::wrapped::<_, i32>({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(
                          tmp_2[{
                            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                              (match ::console_core_number_conversion::wrapped_offset(base) {
                                ::core::result::Result::Ok(native_position) => native_position,
                              })
                              .wrapping_add(off2),
                            );
                            numeric_value
                          }],
                        );
                        numeric_value
                      });
                      let Ok(s1) = console_core_number_conversion::wrapped::<_, i32>({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(
                          tmp_2[{
                            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                              (match ::console_core_number_conversion::wrapped_offset(base) {
                                ::core::result::Result::Ok(native_position) => native_position,
                              })
                              .wrapping_sub(off2),
                            );
                            numeric_value
                          }],
                        );
                        numeric_value
                      });
                      let Ok(s2) = console_core_number_conversion::wrapped::<_, i32>({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(
                          tmp_2[{
                            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                              (match ::console_core_number_conversion::wrapped_offset(base) {
                                ::core::result::Result::Ok(native_position) => native_position,
                              })
                              .wrapping_add(off3),
                            );
                            numeric_value
                          }],
                        );
                        numeric_value
                      });
                      let Ok(s3) = console_core_number_conversion::wrapped::<_, i32>({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(
                          tmp_2[{
                            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                              (match ::console_core_number_conversion::wrapped_offset(base) {
                                ::core::result::Result::Ok(native_position) => native_position,
                              })
                              .wrapping_sub(off3),
                            );
                            numeric_value
                          }],
                        );
                        numeric_value
                      });

                      let sec_tap = i32::wrapping_sub(2, {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, i32>(k);
                        numeric_value
                      });

                      {
                        let numeric_amount = i32::wrapping_mul(sec_tap, {
                          let Ok(explicit_value) =
                            constrain_scalar(i32::wrapping_sub(s0, px), sec_strength, sec_shift);
                          explicit_value
                        });
                        let numeric_target = &mut (sum);
                        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                      };

                      {
                        let numeric_amount = i32::wrapping_mul(sec_tap, {
                          let Ok(explicit_value) =
                            constrain_scalar(i32::wrapping_sub(s1, px), sec_strength, sec_shift);
                          explicit_value
                        });
                        let numeric_target = &mut (sum);
                        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                      };

                      {
                        let numeric_amount = i32::wrapping_mul(sec_tap, {
                          let Ok(explicit_value) =
                            constrain_scalar(i32::wrapping_sub(s2, px), sec_strength, sec_shift);
                          explicit_value
                        });
                        let numeric_target = &mut (sum);
                        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                      };

                      {
                        let numeric_amount = i32::wrapping_mul(sec_tap, {
                          let Ok(explicit_value) =
                            constrain_scalar(i32::wrapping_sub(s3, px), sec_strength, sec_shift);
                          explicit_value
                        });
                        let numeric_target = &mut (sum);
                        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                      };

                      min = {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, i32>(cmp::min(
                            cmp::min(
                              cmp::min(
                                {
                                  let Ok(numeric_value) =
                                    console_core_number_conversion::wrapped::<_, u32>(s0);
                                  numeric_value
                                },
                                {
                                  let Ok(numeric_value) =
                                    console_core_number_conversion::wrapped::<_, u32>(s1);
                                  numeric_value
                                },
                              ),
                              cmp::min(
                                {
                                  let Ok(numeric_value) =
                                    console_core_number_conversion::wrapped::<_, u32>(s2);
                                  numeric_value
                                },
                                {
                                  let Ok(numeric_value) =
                                    console_core_number_conversion::wrapped::<_, u32>(s3);
                                  numeric_value
                                },
                              ),
                            ),
                            {
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped::<_, u32>(min);
                              numeric_value
                            },
                          ));
                        numeric_value
                      };
                      max = cmp::max(cmp::max(cmp::max(cmp::max(s0, s1), s2), s3), max);
                    }

                    p[row_off.wrapping_add(x)] = {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, u8>(iclip(
                          i32::wrapping_add(
                            px,
                            i32::wrapping_shr(
                              i32::wrapping_add(i32::wrapping_sub(sum, i32::from(sum < 0)), 8),
                              4_u32,
                            ),
                          ),
                          min,
                          max,
                        ));
                      numeric_value
                    };
                  }
                }
              }
              ::core::option::Option::None => {
                for y in 0..h {
                  let Ok(row_base) = console_core_number_conversion::wrapped_offset(
                    tmp_offset.wrapping_add(y.wrapping_mul(TMP_STRIDE)),
                  );
                  let Ok(row_off) = console_core_number_conversion::wrapped_index(
                    ({
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped_offset(offset);
                      numeric_value
                    })
                    .wrapping_add(
                      ({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(y);
                        numeric_value
                      })
                      .wrapping_mul(stride),
                    ),
                  );

                  for x in 0..w {
                    let Ok(px) = console_core_number_conversion::wrapped::<_, i32>(
                      p[row_off.wrapping_add(x)],
                    );
                    let mut sum = 0i32;
                    let ::core::result::Result::Ok(base) =
                      ::console_core_number_conversion::fixed_offset(row_base.wrapping_add({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(x);
                        numeric_value
                      }));

                    let mut pri_tap_k = pri_tap;

                    for k in 0..2 {
                      let Ok(off) = console_core_number_conversion::wrapped_offset(
                        dav1d_cdef_directions[dir.wrapping_add(2)][k],
                      );
                      let Ok(p0) = console_core_number_conversion::wrapped::<_, i32>(
                        tmp_2[{
                          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                            (match ::console_core_number_conversion::wrapped_offset(base) {
                              ::core::result::Result::Ok(native_position) => native_position,
                            })
                            .wrapping_add(off),
                          );
                          numeric_value
                        }],
                      );
                      let Ok(p1) = console_core_number_conversion::wrapped::<_, i32>(
                        tmp_2[{
                          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                            (match ::console_core_number_conversion::wrapped_offset(base) {
                              ::core::result::Result::Ok(native_position) => native_position,
                            })
                            .wrapping_sub(off),
                          );
                          numeric_value
                        }],
                      );

                      {
                        let numeric_amount = i32::wrapping_mul(pri_tap_k, {
                          let Ok(explicit_value) =
                            constrain_scalar(i32::wrapping_sub(p0, px), pri_strength, pri_shift);
                          explicit_value
                        });
                        let numeric_target = &mut (sum);
                        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                      };

                      {
                        let numeric_amount = i32::wrapping_mul(pri_tap_k, {
                          let Ok(explicit_value) =
                            constrain_scalar(i32::wrapping_sub(p1, px), pri_strength, pri_shift);
                          explicit_value
                        });
                        let numeric_target = &mut (sum);
                        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                      };

                      pri_tap_k = pri_tap_k & 3 | 2;
                    }

                    p[row_off.wrapping_add(x)] = {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, u8>(i32::wrapping_add(
                          px,
                          i32::wrapping_shr(
                            i32::wrapping_add(i32::wrapping_sub(sum, i32::from(sum < 0)), 8),
                            4_u32,
                          ),
                        ));
                      numeric_value
                    };
                  }
                }
              }
            }
          }
          ::core::option::Option::None => {
            let Ok(ilog2) =
              console_core_number_conversion::wrapped::<_, i32>(sec_strength.ilog2());
            let sec_shift = i32::wrapping_sub(damping, ilog2);

            for y in 0..h {
              let Ok(row_base) = console_core_number_conversion::wrapped_offset(
                tmp_offset.wrapping_add(y.wrapping_mul(TMP_STRIDE)),
              );
              let Ok(row_off) = console_core_number_conversion::wrapped_index(
                ({
                  let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(offset);
                  numeric_value
                })
                .wrapping_add(
                  ({
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(y);
                    numeric_value
                  })
                  .wrapping_mul(stride),
                ),
              );

              for x in 0..w {
                let Ok(px) =
                  console_core_number_conversion::wrapped::<_, i32>(p[row_off.wrapping_add(x)]);
                let mut sum = 0i32;
                let ::core::result::Result::Ok(base) =
                  ::console_core_number_conversion::fixed_offset(row_base.wrapping_add({
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(x);
                    numeric_value
                  }));

                for k in 0..2 {
                  let Ok(off1) = console_core_number_conversion::wrapped_offset(
                    dav1d_cdef_directions[dir.wrapping_add(4)][k],
                  );
                  let Ok(off2) = console_core_number_conversion::wrapped_offset(
                    dav1d_cdef_directions[dir.wrapping_add(0)][k],
                  );
                  let Ok(s0) = console_core_number_conversion::wrapped::<_, i32>(
                    tmp_2[{
                      let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                        (match ::console_core_number_conversion::wrapped_offset(base) {
                          ::core::result::Result::Ok(native_position) => native_position,
                        })
                        .wrapping_add(off1),
                      );
                      numeric_value
                    }],
                  );
                  let Ok(s1) = console_core_number_conversion::wrapped::<_, i32>(
                    tmp_2[{
                      let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                        (match ::console_core_number_conversion::wrapped_offset(base) {
                          ::core::result::Result::Ok(native_position) => native_position,
                        })
                        .wrapping_sub(off1),
                      );
                      numeric_value
                    }],
                  );
                  let Ok(s2) = console_core_number_conversion::wrapped::<_, i32>(
                    tmp_2[{
                      let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                        (match ::console_core_number_conversion::wrapped_offset(base) {
                          ::core::result::Result::Ok(native_position) => native_position,
                        })
                        .wrapping_add(off2),
                      );
                      numeric_value
                    }],
                  );
                  let Ok(s3) = console_core_number_conversion::wrapped::<_, i32>(
                    tmp_2[{
                      let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                        (match ::console_core_number_conversion::wrapped_offset(base) {
                          ::core::result::Result::Ok(native_position) => native_position,
                        })
                        .wrapping_sub(off2),
                      );
                      numeric_value
                    }],
                  );

                  let sec_tap = i32::wrapping_sub(2, {
                    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(k);
                    numeric_value
                  });

                  {
                    let numeric_amount = i32::wrapping_mul(sec_tap, {
                      let Ok(explicit_value) =
                        constrain_scalar(i32::wrapping_sub(s0, px), sec_strength, sec_shift);
                      explicit_value
                    });
                    let numeric_target = &mut (sum);
                    *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                  };

                  {
                    let numeric_amount = i32::wrapping_mul(sec_tap, {
                      let Ok(explicit_value) =
                        constrain_scalar(i32::wrapping_sub(s1, px), sec_strength, sec_shift);
                      explicit_value
                    });
                    let numeric_target = &mut (sum);
                    *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                  };

                  {
                    let numeric_amount = i32::wrapping_mul(sec_tap, {
                      let Ok(explicit_value) =
                        constrain_scalar(i32::wrapping_sub(s2, px), sec_strength, sec_shift);
                      explicit_value
                    });
                    let numeric_target = &mut (sum);
                    *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                  };

                  {
                    let numeric_amount = i32::wrapping_mul(sec_tap, {
                      let Ok(explicit_value) =
                        constrain_scalar(i32::wrapping_sub(s3, px), sec_strength, sec_shift);
                      explicit_value
                    });
                    let numeric_target = &mut (sum);
                    *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                  };
                }

                p[row_off.wrapping_add(x)] = {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, u8>(i32::wrapping_add(
                      px,
                      i32::wrapping_shr(
                        i32::wrapping_add(i32::wrapping_sub(sum, i32::from(sum < 0)), 8),
                        4_u32,
                      ),
                    ));
                  numeric_value
                };
              }
            }
          }
        }
      },
    );
  };

  Ok(())
}

#[cfg(target_arch = "wasm32")]
pub(super) fn padding_8bpc(
  tmp: &mut [u16],
  dst: PicOffset,
  left: &[LeftPixelRow2px<u8>; 8],
  top: &CdefTop,
  bottom: &CdefBottom,
  w: usize,
  h: usize,
  edges: CdefEdgeFlags,
) -> core::result::Result<(), console_core_never::Never> {
  {
    use crate::include::common::bitdepth::BitDepth8;

    let Ok(stride) = ::console_core_number_conversion::fixed_offset(match dst.pixel_stride::<BitDepth8>() { ::core::result::Result::Ok(stride) => stride });

    let very_large = 0xC000u16;
    tmp.fill(very_large);
    let mut tmp_2 = tmp.flex_mut();

    let ::core::result::Result::Ok(tmp_offset) = ::console_core_number_conversion::fixed_index(
      (::core::num::Wrapping(2) * ::core::num::Wrapping(TMP_STRIDE))
        .0
        .wrapping_add(2),
    );
    let need_left = edges.contains(CdefEdgeFlags::HAVE_LEFT);
    let need_right = edges.contains(CdefEdgeFlags::HAVE_RIGHT);

    let slice_w = w.wrapping_add(match ::core::primitive::bool::then_some(need_right, ()) {
      ::core::option::Option::Some(()) => 2,
      ::core::option::Option::None => 0,
    });

    for y in 0..h {
      let ::core::result::Result::Ok(row_offset) = ::console_core_number_conversion::fixed_index(
        (match ::console_core_number_conversion::index(tmp_offset) {
          ::core::result::Result::Ok(native_position_) => native_position_,
        })
        .wrapping_add(y.wrapping_mul(TMP_STRIDE)),
      );

      match ::core::primitive::bool::then_some(need_left, ()) {
        ::core::option::Option::Some(()) => {
          tmp_2[(match ::console_core_number_conversion::index(row_offset) {
            ::core::result::Result::Ok(native_position) => native_position,
          })
          .wrapping_sub(2)] = {
            let Ok(numeric_value) =
              console_core_number_conversion::wrapped::<_, u16>(*match &left[y] {
                [element, ..] => element,
              });
            numeric_value
          };
          tmp_2[(match ::console_core_number_conversion::index(row_offset) {
            ::core::result::Result::Ok(native_position) => native_position,
          })
          .wrapping_sub(1)] = {
            let Ok(numeric_value) =
              console_core_number_conversion::wrapped::<_, u16>(*match &left[y] {
                [_, element, ..] => element,
              });
            numeric_value
          };
        }
        ::core::option::Option::None => {}
      }

      let src = (dst
        + ({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(y);
          numeric_value
        })
        .wrapping_mul(match ::console_core_number_conversion::wrapped_offset(stride) { ::core::result::Result::Ok(native_position___) => native_position___ }))
      .slice::<BitDepth8>(slice_w);

      for x in 0..slice_w {
        tmp_2[(match ::console_core_number_conversion::index(row_offset) {
          ::core::result::Result::Ok(native_position) => native_position,
        })
        .wrapping_add(x)] = {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(src[x]);
          numeric_value
        };
      }
    }

    match ::core::primitive::bool::then_some(edges.contains(CdefEdgeFlags::HAVE_TOP), ()) {
      ::core::option::Option::Some(()) => {
        let have_left = edges.contains(CdefEdgeFlags::HAVE_LEFT);

        let ::core::result::Result::Ok(left_ext) =
          ::console_core_number_conversion::fixed_index(match ::core::primitive::bool::then_some(have_left, ()) {
            ::core::option::Option::Some(()) => 2usize,
            ::core::option::Option::None => 0,
          });
        let ::core::result::Result::Ok(x_end) = ::console_core_number_conversion::fixed_index(
          match ::core::primitive::bool::then_some(edges.contains(CdefEdgeFlags::HAVE_RIGHT), ()) {
            ::core::option::Option::Some(()) => w.wrapping_add(4),
            ::core::option::Option::None => w.wrapping_add(2),
          },
        );
        let guard_len = (match ::console_core_number_conversion::index(x_end) {
          ::core::result::Result::Ok(native_position__) => native_position__,
        })
        .wrapping_sub(2)
        .wrapping_add(match ::console_core_number_conversion::index(left_ext) {
          ::core::result::Result::Ok(native_position__) => native_position__,
        });

        for dy in 0..2usize {
          let ::core::result::Result::Ok(row_offset) =
            ::console_core_number_conversion::fixed_index(
              (match ::console_core_number_conversion::index(tmp_offset) {
                ::core::result::Result::Ok(native_position_) => native_position_,
              })
              .wrapping_sub(
                (::core::num::Wrapping(2) - ::core::num::Wrapping(dy))
                  .0
                  .wrapping_mul(TMP_STRIDE),
              ),
            );
          let ::core::result::Result::Ok(guard_start) =
            ::console_core_number_conversion::fixed_index(
              top
                .offset
                .wrapping_sub(match ::console_core_number_conversion::index(left_ext) {
                  ::core::result::Result::Ok(native_position__) => native_position__,
                })
                .wrapping_add_signed(
                  ({
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(dy);
                    numeric_value
                  })
                  .wrapping_mul(match ::console_core_number_conversion::wrapped_offset(stride) { ::core::result::Result::Ok(native_position___) => native_position___ }),
                ),
            );
          let slice = top.data.slice_as::<_, u8>((
            match ::console_core_number_conversion::index(guard_start) {
              ::core::result::Result::Ok(native_position__) => native_position__,
            }..,
            ..guard_len,
          ));

          for i in 0..guard_len {
            tmp_2[(match ::console_core_number_conversion::index(row_offset) {
              ::core::result::Result::Ok(native_position) => native_position,
            })
            .wrapping_add(i)
            .wrapping_sub(match ::console_core_number_conversion::index(left_ext) {
              ::core::result::Result::Ok(native_position__) => native_position__,
            })] = {
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(slice[i]);
              numeric_value
            };
          }
        }
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(edges.contains(CdefEdgeFlags::HAVE_BOTTOM), ()) {
      ::core::option::Option::Some(()) => {
        let x_start = match ::core::primitive::bool::then_some(edges.contains(CdefEdgeFlags::HAVE_LEFT), ()) {
          ::core::option::Option::Some(()) => 0usize,
          ::core::option::Option::None => 2,
        };
        let x_end = match ::core::primitive::bool::then_some(edges.contains(CdefEdgeFlags::HAVE_RIGHT), ()) {
          ::core::option::Option::Some(()) => w.wrapping_add(4),
          ::core::option::Option::None => w.wrapping_add(2),
        };

        for dy in 0..2usize {
          let ::core::result::Result::Ok(row_offset) =
            ::console_core_number_conversion::fixed_index(
              (match ::console_core_number_conversion::index(tmp_offset) {
                ::core::result::Result::Ok(native_position_) => native_position_,
              })
              .wrapping_add(h.wrapping_add(dy).wrapping_mul(TMP_STRIDE)),
            );
          let bottom_row = WithOffset {
            data: bottom.data,
            offset: bottom.offset.wrapping_sub(2).wrapping_add_signed(
              ({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(dy);
                numeric_value
              })
              .wrapping_mul(match ::console_core_number_conversion::wrapped_offset(stride) { ::core::result::Result::Ok(native_position___) => native_position___ }),
            ),
          };

          let slice = match bottom_row.data {
            PicOrBuf::Pic(pic) => {
              let guard = pic.slice::<BitDepth8, _>((
                bottom_row.offset.wrapping_add(x_start)..,
                ..x_end.wrapping_sub(x_start),
              ));

              for x in x_start..x_end {
                tmp_2[(match ::console_core_number_conversion::index(row_offset) {
                  ::core::result::Result::Ok(native_position) => native_position,
                })
                .wrapping_add(x)
                .wrapping_sub(2)] = {
                  let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(
                    guard[x.wrapping_sub(x_start)],
                  );
                  numeric_value
                };
              }

              continue;
            }
            PicOrBuf::Buf(buf) => buf.slice_as::<_, u8>((
              bottom_row.offset.wrapping_add(x_start)..,
              ..x_end.wrapping_sub(x_start),
            )),
          };

          for x in x_start..x_end {
            tmp_2[(match ::console_core_number_conversion::index(row_offset) {
              ::core::result::Result::Ok(native_position) => native_position,
            })
            .wrapping_add(x)
            .wrapping_sub(2)] = {
              let Ok(numeric_value) =
                console_core_number_conversion::wrapped::<_, u16>(slice[x.wrapping_sub(x_start)]);
              numeric_value
            };
          }
        }
      }
      ::core::option::Option::None => {}
    }
  };

  Ok(())
}

#[cfg(target_arch = "wasm32")]
pub(super) fn padding_16bpc(
  tmp: &mut [u16],
  dst: PicOffset,
  left: &[LeftPixelRow2px<u16>; 8],
  top: &CdefTop,
  bottom: &CdefBottom,
  w: usize,
  h: usize,
  edges: CdefEdgeFlags,
  bitdepth_max: c_int,
) -> core::result::Result<(), console_core_never::Never> {
  {
    use crate::include::common::bitdepth::BitDepth16;

    let _bd = BitDepth16::new({
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(bitdepth_max);
      numeric_value
    });

    let very_large = 0xC000u16;
    tmp.fill(very_large);
    let mut tmp_2 = tmp.flex_mut();

    let ::core::result::Result::Ok(tmp_offset) = ::console_core_number_conversion::fixed_index(
      (::core::num::Wrapping(2) * ::core::num::Wrapping(TMP_STRIDE))
        .0
        .wrapping_add(2),
    );
    let Ok(pixel_stride) = dst.pixel_stride::<BitDepth16>();
    let need_left = edges.contains(CdefEdgeFlags::HAVE_LEFT);
    let need_right = edges.contains(CdefEdgeFlags::HAVE_RIGHT);

    let slice_w = w.wrapping_add(match ::core::primitive::bool::then_some(need_right, ()) {
      ::core::option::Option::Some(()) => 2,
      ::core::option::Option::None => 0,
    });

    for y in 0..h {
      let ::core::result::Result::Ok(row_offset) = ::console_core_number_conversion::fixed_index(
        (match ::console_core_number_conversion::index(tmp_offset) {
          ::core::result::Result::Ok(native_position_) => native_position_,
        })
        .wrapping_add(y.wrapping_mul(TMP_STRIDE)),
      );

      match ::core::primitive::bool::then_some(need_left, ()) {
        ::core::option::Option::Some(()) => {
          tmp_2[(match ::console_core_number_conversion::index(row_offset) {
            ::core::result::Result::Ok(native_position) => native_position,
          })
          .wrapping_sub(2)] = *match &left[y] {
            [element, ..] => element,
          };
          tmp_2[(match ::console_core_number_conversion::index(row_offset) {
            ::core::result::Result::Ok(native_position) => native_position,
          })
          .wrapping_sub(1)] = *match &left[y] {
            [_, element, ..] => element,
          };
        }
        ::core::option::Option::None => {}
      }

      let src = (dst
        + ({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(y);
          numeric_value
        })
        .wrapping_mul(pixel_stride))
      .slice::<BitDepth16>(slice_w);

      for x in 0..slice_w {
        tmp_2[(match ::console_core_number_conversion::index(row_offset) {
          ::core::result::Result::Ok(native_position) => native_position,
        })
        .wrapping_add(x)] = src[x];
      }
    }

    match ::core::primitive::bool::then_some(edges.contains(CdefEdgeFlags::HAVE_TOP), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(pixel_stride_2) = ::console_core_number_conversion::fixed_offset(match dst.pixel_stride::<BitDepth16>() { ::core::result::Result::Ok(pixel_stride_2) => pixel_stride_2 });
        let have_left = edges.contains(CdefEdgeFlags::HAVE_LEFT);
        let ::core::result::Result::Ok(left_ext) =
          ::console_core_number_conversion::fixed_index(match ::core::primitive::bool::then_some(have_left, ()) {
            ::core::option::Option::Some(()) => 2usize,
            ::core::option::Option::None => 0,
          });
        let ::core::result::Result::Ok(x_end) = ::console_core_number_conversion::fixed_index(
          match ::core::primitive::bool::then_some(edges.contains(CdefEdgeFlags::HAVE_RIGHT), ()) {
            ::core::option::Option::Some(()) => w.wrapping_add(4),
            ::core::option::Option::None => w.wrapping_add(2),
          },
        );
        let guard_len = (match ::console_core_number_conversion::index(x_end) {
          ::core::result::Result::Ok(native_position__) => native_position__,
        })
        .wrapping_sub(2)
        .wrapping_add(match ::console_core_number_conversion::index(left_ext) {
          ::core::result::Result::Ok(native_position__) => native_position__,
        });

        for dy in 0..2usize {
          let ::core::result::Result::Ok(row_offset) =
            ::console_core_number_conversion::fixed_index(
              (match ::console_core_number_conversion::index(tmp_offset) {
                ::core::result::Result::Ok(native_position_) => native_position_,
              })
              .wrapping_sub(
                (::core::num::Wrapping(2) - ::core::num::Wrapping(dy))
                  .0
                  .wrapping_mul(TMP_STRIDE),
              ),
            );
          let ::core::result::Result::Ok(guard_start) =
            ::console_core_number_conversion::fixed_index(
              top
                .offset
                .wrapping_sub(match ::console_core_number_conversion::index(left_ext) {
                  ::core::result::Result::Ok(native_position__) => native_position__,
                })
                .wrapping_add_signed(
                  ({
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(dy);
                    numeric_value
                  })
                  .wrapping_mul(match ::console_core_number_conversion::wrapped_offset(pixel_stride_2) { ::core::result::Result::Ok(native_position___) => native_position___ }),
                ),
            );
          let slice = top.data.slice_as::<_, u16>((
            match ::console_core_number_conversion::index(guard_start) {
              ::core::result::Result::Ok(native_position__) => native_position__,
            }..,
            ..guard_len,
          ));

          for i in 0..guard_len {
            tmp_2[(match ::console_core_number_conversion::index(row_offset) {
              ::core::result::Result::Ok(native_position) => native_position,
            })
            .wrapping_add(i)
            .wrapping_sub(match ::console_core_number_conversion::index(left_ext) {
              ::core::result::Result::Ok(native_position__) => native_position__,
            })] = slice[i];
          }
        }
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(edges.contains(CdefEdgeFlags::HAVE_BOTTOM), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(pixel_stride_3) = ::console_core_number_conversion::fixed_offset(match dst.pixel_stride::<BitDepth16>() { ::core::result::Result::Ok(pixel_stride_3) => pixel_stride_3 });
        let x_start = match ::core::primitive::bool::then_some(edges.contains(CdefEdgeFlags::HAVE_LEFT), ()) {
          ::core::option::Option::Some(()) => 0usize,
          ::core::option::Option::None => 2,
        };
        let x_end = match ::core::primitive::bool::then_some(edges.contains(CdefEdgeFlags::HAVE_RIGHT), ()) {
          ::core::option::Option::Some(()) => w.wrapping_add(4),
          ::core::option::Option::None => w.wrapping_add(2),
        };

        for dy in 0..2usize {
          let ::core::result::Result::Ok(row_offset) =
            ::console_core_number_conversion::fixed_index(
              (match ::console_core_number_conversion::index(tmp_offset) {
                ::core::result::Result::Ok(native_position_) => native_position_,
              })
              .wrapping_add(h.wrapping_add(dy).wrapping_mul(TMP_STRIDE)),
            );
          let bottom_row = WithOffset {
            data: bottom.data,
            offset: bottom.offset.wrapping_sub(2).wrapping_add_signed(
              ({
                let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(dy);
                numeric_value
              })
              .wrapping_mul(match ::console_core_number_conversion::wrapped_offset(pixel_stride_3) { ::core::result::Result::Ok(native_position___) => native_position___ }),
            ),
          };

          let slice = match bottom_row.data {
            PicOrBuf::Pic(pic) => {
              let guard = pic.slice::<BitDepth16, _>((
                bottom_row.offset.wrapping_add(x_start)..,
                ..x_end.wrapping_sub(x_start),
              ));

              for x in x_start..x_end {
                tmp_2[(match ::console_core_number_conversion::index(row_offset) {
                  ::core::result::Result::Ok(native_position) => native_position,
                })
                .wrapping_add(x)
                .wrapping_sub(2)] = guard[x.wrapping_sub(x_start)];
              }

              continue;
            }
            PicOrBuf::Buf(buf) => buf.slice_as::<_, u16>((
              bottom_row.offset.wrapping_add(x_start)..,
              ..x_end.wrapping_sub(x_start),
            )),
          };

          for x in x_start..x_end {
            tmp_2[(match ::console_core_number_conversion::index(row_offset) {
              ::core::result::Result::Ok(native_position) => native_position,
            })
            .wrapping_add(x)
            .wrapping_sub(2)] = slice[x.wrapping_sub(x_start)];
          }
        }
      }
      ::core::option::Option::None => {}
    }
  };

  Ok(())
}

#[cfg(target_arch = "wasm32")]
pub(super) fn cdef_filter_block_scalar_16bpc(
  tmp: &[u16],
  tmp_offset: usize,
  dst: PicOffset,
  _stride: isize,
  w: usize,
  h: usize,
  dir: usize,
  pri_strength: c_int,
  sec_strength: c_int,
  damping: c_int,
  bitdepth_max: c_int,
) -> core::result::Result<(), console_core_never::Never> {
  {
    use crate::include::common::bitdepth::BitDepth16;

    let Ok(ilog2) = console_core_number_conversion::wrapped::<_, i32>(
      ({
        let Ok(numeric_value) =
          console_core_number_conversion::wrapped::<_, u32>(i32::wrapping_add(bitdepth_max, 1));
        numeric_value
      })
      .ilog2(),
    );
    let bitdepth_min_8 = i32::wrapping_sub(ilog2, 8);

    let tmp_2 = tmp.flex();

    let _ = crate::include::dav1d::picture::with_pixel_guard_mut::<BitDepth16, _>(
      &dst,
      w,
      h,
      |bytes, offset, stride| {
        let p_u16: &mut [u16] = zerocopy::FromBytes::mut_from_bytes(&mut *bytes)
          .expect("bytes alignment/size mismatch for u16 reinterpretation");
        let mut p = p_u16.flex_mut();

        match ::core::primitive::bool::then_some(pri_strength != 0, ()) {
          ::core::option::Option::Some(()) => {
            let pri_tap = i32::wrapping_sub(
              4,
              i32::wrapping_shr(pri_strength, {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, u32>(bitdepth_min_8);
                numeric_value
              }) & 1,
            );
            let Ok(pri_strength_ilog2) =
              console_core_number_conversion::wrapped::<_, i32>(pri_strength.ilog2());
            let pri_shift = cmp::max(0, i32::wrapping_sub(damping, pri_strength_ilog2));

            match ::core::primitive::bool::then_some(sec_strength != 0, ()) {
              ::core::option::Option::Some(()) => {
                let Ok(sec_strength_ilog2) =
                  console_core_number_conversion::wrapped::<_, i32>(sec_strength.ilog2());
                let sec_shift = i32::wrapping_sub(damping, sec_strength_ilog2);

                for y in 0..h {
                  let Ok(row_base) = console_core_number_conversion::wrapped_offset(
                    tmp_offset.wrapping_add(y.wrapping_mul(TMP_STRIDE)),
                  );
                  let ::core::result::Result::Ok(row_off) =
                    ::console_core_number_conversion::fixed_index(
                      ({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                          ({
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped_offset(offset);
                            numeric_value
                          })
                          .wrapping_add(
                            ({
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped_offset(y);
                              numeric_value
                            })
                            .wrapping_mul(stride),
                          ),
                        );
                        numeric_value
                      })
                      .wrapping_div(2),
                    );

                  for x in 0..w {
                    let Ok(px) = console_core_number_conversion::wrapped::<_, i32>(
                      p[(match ::console_core_number_conversion::index(row_off) {
                        ::core::result::Result::Ok(native_position) => native_position,
                      })
                      .wrapping_add(x)],
                    );
                    let mut sum = 0i32;
                    let mut max = px;
                    let mut min = px;
                    let ::core::result::Result::Ok(base) =
                      ::console_core_number_conversion::fixed_offset(row_base.wrapping_add({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(x);
                        numeric_value
                      }));

                    let mut pri_tap_k = pri_tap;

                    for k in 0..2 {
                      let Ok(off1) = console_core_number_conversion::wrapped_offset(
                        dav1d_cdef_directions[dir.wrapping_add(2)][k],
                      );

                      let Ok(p0) = console_core_number_conversion::wrapped::<_, i32>({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(
                          tmp_2[{
                            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                              (match ::console_core_number_conversion::wrapped_offset(base) {
                                ::core::result::Result::Ok(native_position_) => native_position_,
                              })
                              .wrapping_add(off1),
                            );
                            numeric_value
                          }],
                        );
                        numeric_value
                      });
                      let Ok(p1) = console_core_number_conversion::wrapped::<_, i32>({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(
                          tmp_2[{
                            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                              (match ::console_core_number_conversion::wrapped_offset(base) {
                                ::core::result::Result::Ok(native_position_) => native_position_,
                              })
                              .wrapping_sub(off1),
                            );
                            numeric_value
                          }],
                        );
                        numeric_value
                      });

                      {
                        let numeric_amount = i32::wrapping_mul(pri_tap_k, {
                          let Ok(explicit_value) =
                            constrain_scalar(i32::wrapping_sub(p0, px), pri_strength, pri_shift);
                          explicit_value
                        });
                        let numeric_target = &mut (sum);
                        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                      };

                      {
                        let numeric_amount = i32::wrapping_mul(pri_tap_k, {
                          let Ok(explicit_value) =
                            constrain_scalar(i32::wrapping_sub(p1, px), pri_strength, pri_shift);
                          explicit_value
                        });
                        let numeric_target = &mut (sum);
                        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                      };

                      pri_tap_k = pri_tap_k & 3 | 2;

                      min = {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, i32>(cmp::min(
                            cmp::min(
                              {
                                let Ok(numeric_value) =
                                  console_core_number_conversion::wrapped::<_, u32>(p0);
                                numeric_value
                              },
                              {
                                let Ok(numeric_value) =
                                  console_core_number_conversion::wrapped::<_, u32>(p1);
                                numeric_value
                              },
                            ),
                            {
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped::<_, u32>(min);
                              numeric_value
                            },
                          ));
                        numeric_value
                      };

                      max = cmp::max(cmp::max(p0, p1), max);

                      let Ok(off2) = console_core_number_conversion::wrapped_offset(
                        dav1d_cdef_directions[dir.wrapping_add(4)][k],
                      );
                      let Ok(off3) = console_core_number_conversion::wrapped_offset(
                        dav1d_cdef_directions[dir.wrapping_add(0)][k],
                      );
                      let Ok(s0) = console_core_number_conversion::wrapped::<_, i32>({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(
                          tmp_2[{
                            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                              (match ::console_core_number_conversion::wrapped_offset(base) {
                                ::core::result::Result::Ok(native_position_) => native_position_,
                              })
                              .wrapping_add(off2),
                            );
                            numeric_value
                          }],
                        );
                        numeric_value
                      });
                      let Ok(s1) = console_core_number_conversion::wrapped::<_, i32>({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(
                          tmp_2[{
                            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                              (match ::console_core_number_conversion::wrapped_offset(base) {
                                ::core::result::Result::Ok(native_position_) => native_position_,
                              })
                              .wrapping_sub(off2),
                            );
                            numeric_value
                          }],
                        );
                        numeric_value
                      });
                      let Ok(s2) = console_core_number_conversion::wrapped::<_, i32>({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(
                          tmp_2[{
                            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                              (match ::console_core_number_conversion::wrapped_offset(base) {
                                ::core::result::Result::Ok(native_position_) => native_position_,
                              })
                              .wrapping_add(off3),
                            );
                            numeric_value
                          }],
                        );
                        numeric_value
                      });
                      let Ok(s3) = console_core_number_conversion::wrapped::<_, i32>({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(
                          tmp_2[{
                            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                              (match ::console_core_number_conversion::wrapped_offset(base) {
                                ::core::result::Result::Ok(native_position_) => native_position_,
                              })
                              .wrapping_sub(off3),
                            );
                            numeric_value
                          }],
                        );
                        numeric_value
                      });

                      let sec_tap = i32::wrapping_sub(2, {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, i32>(k);
                        numeric_value
                      });

                      {
                        let numeric_amount = i32::wrapping_mul(sec_tap, {
                          let Ok(explicit_value) =
                            constrain_scalar(i32::wrapping_sub(s0, px), sec_strength, sec_shift);
                          explicit_value
                        });
                        let numeric_target = &mut (sum);
                        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                      };

                      {
                        let numeric_amount = i32::wrapping_mul(sec_tap, {
                          let Ok(explicit_value) =
                            constrain_scalar(i32::wrapping_sub(s1, px), sec_strength, sec_shift);
                          explicit_value
                        });
                        let numeric_target = &mut (sum);
                        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                      };

                      {
                        let numeric_amount = i32::wrapping_mul(sec_tap, {
                          let Ok(explicit_value) =
                            constrain_scalar(i32::wrapping_sub(s2, px), sec_strength, sec_shift);
                          explicit_value
                        });
                        let numeric_target = &mut (sum);
                        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                      };

                      {
                        let numeric_amount = i32::wrapping_mul(sec_tap, {
                          let Ok(explicit_value) =
                            constrain_scalar(i32::wrapping_sub(s3, px), sec_strength, sec_shift);
                          explicit_value
                        });
                        let numeric_target = &mut (sum);
                        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                      };

                      min = {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, i32>(cmp::min(
                            cmp::min(
                              cmp::min(
                                {
                                  let Ok(numeric_value) =
                                    console_core_number_conversion::wrapped::<_, u32>(s0);
                                  numeric_value
                                },
                                {
                                  let Ok(numeric_value) =
                                    console_core_number_conversion::wrapped::<_, u32>(s1);
                                  numeric_value
                                },
                              ),
                              cmp::min(
                                {
                                  let Ok(numeric_value) =
                                    console_core_number_conversion::wrapped::<_, u32>(s2);
                                  numeric_value
                                },
                                {
                                  let Ok(numeric_value) =
                                    console_core_number_conversion::wrapped::<_, u32>(s3);
                                  numeric_value
                                },
                              ),
                            ),
                            {
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped::<_, u32>(min);
                              numeric_value
                            },
                          ));
                        numeric_value
                      };
                      max = cmp::max(cmp::max(cmp::max(cmp::max(s0, s1), s2), s3), max);
                    }

                    p[(match ::console_core_number_conversion::index(row_off) {
                      ::core::result::Result::Ok(native_position) => native_position,
                    })
                    .wrapping_add(x)] = {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, u16>(iclip(
                          i32::wrapping_add(
                            px,
                            i32::wrapping_shr(
                              i32::wrapping_add(i32::wrapping_sub(sum, i32::from(sum < 0)), 8),
                              4_u32,
                            ),
                          ),
                          min,
                          max,
                        ));
                      numeric_value
                    };
                  }
                }
              }
              ::core::option::Option::None => {
                for y in 0..h {
                  let Ok(row_base) = console_core_number_conversion::wrapped_offset(
                    tmp_offset.wrapping_add(y.wrapping_mul(TMP_STRIDE)),
                  );
                  let ::core::result::Result::Ok(row_off) =
                    ::console_core_number_conversion::fixed_index(
                      ({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                          ({
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped_offset(offset);
                            numeric_value
                          })
                          .wrapping_add(
                            ({
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped_offset(y);
                              numeric_value
                            })
                            .wrapping_mul(stride),
                          ),
                        );
                        numeric_value
                      })
                      .wrapping_div(2),
                    );

                  for x in 0..w {
                    let Ok(px) = console_core_number_conversion::wrapped::<_, i32>(
                      p[(match ::console_core_number_conversion::index(row_off) {
                        ::core::result::Result::Ok(native_position) => native_position,
                      })
                      .wrapping_add(x)],
                    );
                    let mut sum = 0i32;
                    let ::core::result::Result::Ok(base) =
                      ::console_core_number_conversion::fixed_offset(row_base.wrapping_add({
                        let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(x);
                        numeric_value
                      }));

                    let mut pri_tap_k = pri_tap;

                    for k in 0..2 {
                      let Ok(off) = console_core_number_conversion::wrapped_offset(
                        dav1d_cdef_directions[dir.wrapping_add(2)][k],
                      );
                      let Ok(p0) = console_core_number_conversion::wrapped::<_, i32>(
                        tmp_2[{
                          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                            (match ::console_core_number_conversion::wrapped_offset(base) {
                              ::core::result::Result::Ok(native_position_) => native_position_,
                            })
                            .wrapping_add(off),
                          );
                          numeric_value
                        }],
                      );
                      let Ok(p1) = console_core_number_conversion::wrapped::<_, i32>(
                        tmp_2[{
                          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                            (match ::console_core_number_conversion::wrapped_offset(base) {
                              ::core::result::Result::Ok(native_position_) => native_position_,
                            })
                            .wrapping_sub(off),
                          );
                          numeric_value
                        }],
                      );

                      {
                        let numeric_amount = i32::wrapping_mul(pri_tap_k, {
                          let Ok(explicit_value) =
                            constrain_scalar(i32::wrapping_sub(p0, px), pri_strength, pri_shift);
                          explicit_value
                        });
                        let numeric_target = &mut (sum);
                        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                      };

                      {
                        let numeric_amount = i32::wrapping_mul(pri_tap_k, {
                          let Ok(explicit_value) =
                            constrain_scalar(i32::wrapping_sub(p1, px), pri_strength, pri_shift);
                          explicit_value
                        });
                        let numeric_target = &mut (sum);
                        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                      };

                      pri_tap_k = pri_tap_k & 3 | 2;
                    }

                    let result = i32::wrapping_add(
                      px,
                      i32::wrapping_shr(
                        i32::wrapping_add(i32::wrapping_sub(sum, i32::from(sum < 0)), 8),
                        4_u32,
                      ),
                    );
                    p[(match ::console_core_number_conversion::index(row_off) {
                      ::core::result::Result::Ok(native_position) => native_position,
                    })
                    .wrapping_add(x)] = {
                      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(
                        iclip(result, 0, bitdepth_max),
                      );
                      numeric_value
                    };
                  }
                }
              }
            }
          }
          ::core::option::Option::None => {
            let Ok(sec_strength_ilog2) =
              console_core_number_conversion::wrapped::<_, i32>(sec_strength.ilog2());
            let sec_shift = i32::wrapping_sub(damping, sec_strength_ilog2);

            for y in 0..h {
              let Ok(row_base) = console_core_number_conversion::wrapped_offset(
                tmp_offset.wrapping_add(y.wrapping_mul(TMP_STRIDE)),
              );
              let ::core::result::Result::Ok(row_off) =
                ::console_core_number_conversion::fixed_index(
                  ({
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                      ({
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped_offset(offset);
                        numeric_value
                      })
                      .wrapping_add(
                        ({
                          let Ok(numeric_value) =
                            console_core_number_conversion::wrapped_offset(y);
                          numeric_value
                        })
                        .wrapping_mul(stride),
                      ),
                    );
                    numeric_value
                  })
                  .wrapping_div(2),
                );

              for x in 0..w {
                let Ok(px) = console_core_number_conversion::wrapped::<_, i32>(
                  p[(match ::console_core_number_conversion::index(row_off) {
                    ::core::result::Result::Ok(native_position) => native_position,
                  })
                  .wrapping_add(x)],
                );
                let mut sum = 0i32;
                let ::core::result::Result::Ok(base) =
                  ::console_core_number_conversion::fixed_offset(row_base.wrapping_add({
                    let Ok(numeric_value) = console_core_number_conversion::wrapped_offset(x);
                    numeric_value
                  }));

                for k in 0..2 {
                  let Ok(off1) = console_core_number_conversion::wrapped_offset(
                    dav1d_cdef_directions[dir.wrapping_add(4)][k],
                  );
                  let Ok(off2) = console_core_number_conversion::wrapped_offset(
                    dav1d_cdef_directions[dir.wrapping_add(0)][k],
                  );
                  let Ok(s0) = console_core_number_conversion::wrapped::<_, i32>(
                    tmp_2[{
                      let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                        (match ::console_core_number_conversion::wrapped_offset(base) {
                          ::core::result::Result::Ok(native_position_) => native_position_,
                        })
                        .wrapping_add(off1),
                      );
                      numeric_value
                    }],
                  );
                  let Ok(s1) = console_core_number_conversion::wrapped::<_, i32>(
                    tmp_2[{
                      let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                        (match ::console_core_number_conversion::wrapped_offset(base) {
                          ::core::result::Result::Ok(native_position_) => native_position_,
                        })
                        .wrapping_sub(off1),
                      );
                      numeric_value
                    }],
                  );
                  let Ok(s2) = console_core_number_conversion::wrapped::<_, i32>(
                    tmp_2[{
                      let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                        (match ::console_core_number_conversion::wrapped_offset(base) {
                          ::core::result::Result::Ok(native_position_) => native_position_,
                        })
                        .wrapping_add(off2),
                      );
                      numeric_value
                    }],
                  );
                  let Ok(s3) = console_core_number_conversion::wrapped::<_, i32>(
                    tmp_2[{
                      let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                        (match ::console_core_number_conversion::wrapped_offset(base) {
                          ::core::result::Result::Ok(native_position_) => native_position_,
                        })
                        .wrapping_sub(off2),
                      );
                      numeric_value
                    }],
                  );

                  let sec_tap = i32::wrapping_sub(2, {
                    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(k);
                    numeric_value
                  });

                  {
                    let numeric_amount = i32::wrapping_mul(sec_tap, {
                      let Ok(explicit_value) =
                        constrain_scalar(i32::wrapping_sub(s0, px), sec_strength, sec_shift);
                      explicit_value
                    });
                    let numeric_target = &mut (sum);
                    *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                  };

                  {
                    let numeric_amount = i32::wrapping_mul(sec_tap, {
                      let Ok(explicit_value) =
                        constrain_scalar(i32::wrapping_sub(s1, px), sec_strength, sec_shift);
                      explicit_value
                    });
                    let numeric_target = &mut (sum);
                    *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                  };

                  {
                    let numeric_amount = i32::wrapping_mul(sec_tap, {
                      let Ok(explicit_value) =
                        constrain_scalar(i32::wrapping_sub(s2, px), sec_strength, sec_shift);
                      explicit_value
                    });
                    let numeric_target = &mut (sum);
                    *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                  };

                  {
                    let numeric_amount = i32::wrapping_mul(sec_tap, {
                      let Ok(explicit_value) =
                        constrain_scalar(i32::wrapping_sub(s3, px), sec_strength, sec_shift);
                      explicit_value
                    });
                    let numeric_target = &mut (sum);
                    *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                  };
                }

                let result = i32::wrapping_add(
                  px,
                  i32::wrapping_shr(
                    i32::wrapping_add(i32::wrapping_sub(sum, i32::from(sum < 0)), 8),
                    4_u32,
                  ),
                );
                p[(match ::console_core_number_conversion::index(row_off) {
                  ::core::result::Result::Ok(native_position) => native_position,
                })
                .wrapping_add(x)] = {
                  let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(
                    iclip(result, 0, bitdepth_max),
                  );
                  numeric_value
                };
              }
            }
          }
        }
      },
    );
  };

  Ok(())
}

#[cfg(target_arch = "x86_64")]
pub(crate) fn cdef_filter_dispatch<BD: BitDepth>(
  request: &CdefFilter<'_, '_, BD>,
) -> Rav1dResult<CdefDispatch> {
  use crate::include::common::bitdepth::BPC;

  let Ok(avx2) = crate::src::cpu::summon_avx2();
  let token = match avx2 {
    Some(proof) => proof,
    None => return Ok(CdefDispatch::Unavailable),
  };

  match (request.primary, request.secondary) {
    (0, 0) => return Ok(CdefDispatch::Applied),
    (_primary, _secondary) => {}
  }

  let Ok(extent) = request.shape.size();
  let padding = cdef_padding(request, -16384)?;
  let Ok(maximum) = wrapped::<_, i16>(request.bitdepth.into_c());
  let block = CdefX86Block {
    pixels: zerocopy::transmute_ref!(&padding),
    stencil: { let checked_value = cdef_stencil(request)?; checked_value },
    extent,
    destination: request.destination,
    maximum,
  };

  let filtered = match BD::BPC {
    BPC::BPC8 => {
      let Ok(avx512) = crate::src::cpu::summon_avx512();

      match avx512 {
        Some(proof) => cdef_filter_zmm8(proof, &block),
        None => cdef_filter_sse8(token, &block),
      }
    }
    BPC::BPC16 => cdef_filter_sse16(token, &block),
  };
  filtered?;

  Ok(CdefDispatch::Applied)
}

#[cfg(target_arch = "x86_64")]
pub fn cdef_dir_dispatch<BD: BitDepth>(
  destination: PicOffset,
  variance: &mut c_uint,
  bitdepth: BD,
) -> Rav1dResult<Option<c_int>> {
  use crate::include::common::bitdepth::BPC;

  let Ok(avx2) = crate::src::cpu::summon_avx2();
  let token = match avx2 {
    Some(proof) => proof,
    None => return Ok(None),
  };

  let direction = match BD::BPC {
    BPC::BPC8 => cdef_find_dir_simd_8bpc(token, destination, variance)?,
    BPC::BPC16 => cdef_find_dir_simd_16bpc(token, destination, variance, bitdepth.bitdepth())?,
  };

  Ok(Some(direction))
}
