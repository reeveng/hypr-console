//! Safe SIMD implementations of motion compensation functions
#![cfg_attr(not(feature = "unchecked"), forbid(unsafe_code))]
#![cfg_attr(feature = "unchecked", deny(unsafe_code))]
//!
//! These replace the hand-written assembly in src/x86/mc_*.asm
//!
//! Uses archmage tokens for safe SIMD invocation:
//! - Desktop64 (X64V3Token) for AVX2+FMA on x86-64
//! - The runtime CPU check happens at init time in the dispatch table setup
//! - extern "C" wrappers declare V3 with #[rite(v3)] and use from_context()

use core::num::Wrapping;

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
use crate::src::error::{Rav1dError, Rav1dResult};

#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
use crate::include::common::bitdepth::BitDepth;
#[cfg(target_arch = "x86_64")]
use crate::include::dav1d::headers::Rav1dFilterMode;
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
use crate::include::dav1d::headers::Rav1dPixelLayoutSubSampled;
#[cfg(target_arch = "x86_64")]
use crate::include::dav1d::picture::PicOffset;
#[cfg(target_arch = "x86_64")]
use crate::src::internal::COMPINTER_LEN;
#[cfg(target_arch = "x86_64")]
use crate::src::levels::Filter2d;
#[cfg(target_arch = "x86_64")]
use crate::src::safe_simd::pixel_access::{
  loadi64, loadu_64, loadu_128, loadu_256, loadu_512, storeu_128, storeu_256, storeu_512,
};
#[cfg(target_arch = "x86_64")]
use crate::src::strided::Strided as _;
#[cfg(target_arch = "x86_64")]
use archmage::{Desktop64, Server64, arcane, rite};
#[cfg(target_arch = "x86_64")]
use console_core_geometry::Point;
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
use console_core_geometry::Size;
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
use console_core_never::Never;

use std::cell::Cell;

#[cfg(target_arch = "x86_64")]
#[path = "mc_reference.rs"]
mod reference;

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
#[path = "mc_blend_rows.rs"]
pub(super) mod blend_rows;
#[cfg(target_arch = "x86_64")]
use blend_rows::BlendLayoutError;
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
use blend_rows::BlendSize;

type MidBuffer<T, const ROWS: console_core_api_compatibility::NativeSize> =
  Box<[[T; MID_STRIDE]; ROWS]>;
type MidPool<T, const ROWS: console_core_api_compatibility::NativeSize> =
  std::thread::LocalKey<Cell<Option<MidBuffer<T, ROWS>>>>;

thread_local! {
    static MID_I16_135: Cell<Option<MidBuffer<i16, 135>>> = const { Cell::new(None) };
    static MID_I32_135: Cell<Option<MidBuffer<i32, 135>>> = const { Cell::new(None) };
    static MID_I16_130: Cell<Option<MidBuffer<i16, 130>>> = const { Cell::new(None) };
    static MID_I32_130: Cell<Option<MidBuffer<i32, 130>>> = const { Cell::new(None) };
}

#[inline(always)]
fn take_mid<T: Copy + Default, const ROWS: console_core_api_compatibility::NativeSize>(
  pool: &'static MidPool<T, ROWS>,
) -> Result<MidBuffer<T, ROWS>, console_core_never::Never> {
  Ok(match pool.with(Cell::take) {
    Some(reused) => reused,
    None => Box::new([[T::default(); MID_STRIDE]; ROWS]),
  })
}

#[inline(always)]
fn put_mid<T: 'static, const ROWS: console_core_api_compatibility::NativeSize>(
  pool: &'static MidPool<T, ROWS>,
  buffer: MidBuffer<T, ROWS>,
) -> Result<(), console_core_never::Never> {
  pool.with(|cell| cell.set(Some(buffer)));
  Ok(())
}

const PW_1024: i16 = 1024;

#[cfg(target_arch = "x86_64")]
#[inline]
fn avg_8bpc_sample([first, second]: [i16; 2]) -> Result<u8, console_core_never::Never> {
  let sum = i32::from(first.wrapping_add(second));
  let rounded = ::core::primitive::i32::wrapping_shr(
    sum.wrapping_mul(i32::from(PW_1024)).wrapping_add(16384),
    15_u32,
  );
  console_core_number_conversion::wrapped(rounded.clamp(0, 255))
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn avg_8bpc_avx2_safe(
  _token: Desktop64,
  dst: &mut [u8],
  byte_stride: u64,
  tmp1: &[i16; COMPINTER_LEN],
  tmp2: &[i16; COMPINTER_LEN],
  block_extent: Size<i32>,
) -> Result<(), BlendLayoutError> {
  let Size {
    width: w,
    height: h,
  } = block_extent;
  let size = BlendSize::new(Size {
    width: w,
    height: h,
  })?;

  // SAFETY: AVX2 is available (checked at dispatch time via target_feature)
  let round = _mm256_set1_epi16(PW_1024);

  let rows = size.blend_rows::<u8>([tmp1, tmp2], dst, byte_stride)?;

  for mut row in rows {
    let Ok(blocks_32) = row.blocks::<32>();

    for (target, first, second) in blocks_32 {
      let [first_low, first_high]: &[[i16; 16]; 2] = zerocopy::transmute_ref!(first);
      let [second_low, second_high]: &[[i16; 16]; 2] = zerocopy::transmute_ref!(second);
      let t1_lo = loadu_256!(first_low);
      let t1_hi = loadu_256!(first_high);
      let t2_lo = loadu_256!(second_low);
      let t2_hi = loadu_256!(second_high);

      let sum_lo = _mm256_add_epi16(t1_lo, t2_lo);
      let sum_hi = _mm256_add_epi16(t1_hi, t2_hi);

      let avg_lo = _mm256_mulhrs_epi16(sum_lo, round);
      let avg_hi = _mm256_mulhrs_epi16(sum_hi, round);

      let packed = _mm256_packus_epi16(avg_lo, avg_hi);

      let result = _mm256_permute4x64_epi64(packed, 0b11_01_10_00);

      storeu_256!(target, result);
    }

    let Ok(blocks_16) = row.blocks::<16>();

    for (target, first, second) in blocks_16 {
      let t1 = loadu_256!(first);
      let t2 = loadu_256!(second);

      let sum = _mm256_add_epi16(t1, t2);
      let avg = _mm256_mulhrs_epi16(sum, round);

      let packed = _mm256_packus_epi16(avg, avg);
      let lo = _mm256_castsi256_si128(packed);
      let hi = _mm256_extracti128_si256(packed, 1);
      let result = _mm_unpacklo_epi64(lo, hi);

      storeu_128!(target, result);
    }

    let Ok(pixels) = row.pixels();

    for (target, first, second) in pixels {
      let Ok(sample) = avg_8bpc_sample([*first, *second]);
      *target = sample;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn avg_8bpc_avx512_safe(
  _token: Server64,
  dst: &mut [u8],
  byte_stride: u64,
  tmp1: &[i16; COMPINTER_LEN],
  tmp2: &[i16; COMPINTER_LEN],
  block_extent: Size<i32>,
) -> Result<(), BlendLayoutError> {
  let Size {
    width: w,
    height: h,
  } = block_extent;
  let size = BlendSize::new(Size {
    width: w,
    height: h,
  })?;

  let round = _mm512_set1_epi16(PW_1024);
  let zero = _mm512_setzero_si512();

  let rows = size.blend_rows::<u8>([tmp1, tmp2], dst, byte_stride)?;

  for mut row in rows {
    let Ok(blocks_64) = row.blocks::<64>();

    for (target, first, second) in blocks_64 {
      let [first_low, first_high]: &[[i16; 32]; 2] = zerocopy::transmute_ref!(first);
      let [second_low, second_high]: &[[i16; 32]; 2] = zerocopy::transmute_ref!(second);
      let t1_lo = loadu_512!(first_low);
      let t2_lo = loadu_512!(second_low);
      let sum_lo = _mm512_add_epi16(t1_lo, t2_lo);
      let avg_lo = _mm512_mulhrs_epi16(sum_lo, round);
      let avg_lo_2 = _mm512_max_epi16(avg_lo, zero);
      let result_lo: __m256i = _mm512_cvtusepi16_epi8(avg_lo_2);

      let t1_hi = loadu_512!(first_high);
      let t2_hi = loadu_512!(second_high);
      let sum_hi = _mm512_add_epi16(t1_hi, t2_hi);
      let avg_hi = _mm512_mulhrs_epi16(sum_hi, round);
      let avg_hi_2 = _mm512_max_epi16(avg_hi, zero);
      let result_hi: __m256i = _mm512_cvtusepi16_epi8(avg_hi_2);

      let combined = _mm512_inserti64x4::<1>(_mm512_castsi256_si512(result_lo), result_hi);
      storeu_512!(target, combined);
    }

    let Ok(blocks_32) = row.blocks::<32>();

    for (target, first, second) in blocks_32 {
      let t1 = loadu_512!(first);
      let t2 = loadu_512!(second);
      let sum = _mm512_add_epi16(t1, t2);
      let avg = _mm512_mulhrs_epi16(sum, round);
      let avg_2 = _mm512_max_epi16(avg, zero);
      let result: __m256i = _mm512_cvtusepi16_epi8(avg_2);

      storeu_256!(target, result);
    }

    let round_256 = _mm256_set1_epi16(PW_1024);

    let Ok(blocks_16) = row.blocks::<16>();

    for (target, first, second) in blocks_16 {
      let t1 = loadu_256!(first);
      let t2 = loadu_256!(second);
      let sum = _mm256_add_epi16(t1, t2);
      let avg = _mm256_mulhrs_epi16(sum, round_256);
      let packed = _mm256_packus_epi16(avg, avg);
      let lo = _mm256_castsi256_si128(packed);
      let hi = _mm256_extracti128_si256(packed, 1);
      let result = _mm_unpacklo_epi64(lo, hi);
      storeu_128!(target, result);
    }

    let Ok(pixels) = row.pixels();

    for (target, first, second) in pixels {
      let Ok(sample) = avg_8bpc_sample([*first, *second]);
      *target = sample;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn avg_16bpc_avx2_safe(
  _token: Desktop64,
  dst: &mut [u8],
  byte_stride: u64,
  tmp1: &[i16; COMPINTER_LEN],
  tmp2: &[i16; COMPINTER_LEN],
  block_extent: Size<i32>,
  bitdepth_max: i32,
) -> Result<(), BlendLayoutError> {
  let Size {
    width: w,
    height: h,
  } = block_extent;
  let size = BlendSize::new(Size {
    width: w,
    height: h,
  })?;

  let intermediate_bits = match ::core::primitive::bool::then_some(
    ::core::primitive::i32::wrapping_shr(bitdepth_max, 11_u32) != 0,
    (),
  ) {
    ::core::option::Option::Some(()) => 2i32,
    ::core::option::Option::None => 4i32,
  };

  let sh = i32::wrapping_add(intermediate_bits, 1);
  let rnd = i32::wrapping_add(
    i32::wrapping_shl(1, i32::cast_unsigned(intermediate_bits)),
    ::core::primitive::i32::wrapping_mul(8192, 2),
  );
  let max =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(bitdepth_max) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };

  let rnd_vec = _mm256_set1_epi32(rnd);
  let zero = _mm256_setzero_si256();
  let max_vec = _mm256_set1_epi32(max);

  let rows = size.blend_rows::<u16>([tmp1, tmp2], dst, byte_stride)?;

  for mut row in rows {
    let Ok(blocks_16) = row.blocks::<16>();

    for (target, first, second) in blocks_16 {
      let t1 = loadu_256!(first);
      let t2 = loadu_256!(second);

      let t1_lo = _mm256_cvtepi16_epi32(_mm256_castsi256_si128(t1));
      let t2_lo = _mm256_cvtepi16_epi32(_mm256_castsi256_si128(t2));
      let t1_hi = _mm256_cvtepi16_epi32(_mm256_extracti128_si256(t1, 1));
      let t2_hi = _mm256_cvtepi16_epi32(_mm256_extracti128_si256(t2, 1));

      let sum_lo = _mm256_add_epi32(_mm256_add_epi32(t1_lo, t2_lo), rnd_vec);
      let sum_hi = _mm256_add_epi32(_mm256_add_epi32(t1_hi, t2_hi), rnd_vec);

      let (result_lo, result_hi) = match ::core::primitive::bool::then_some(sh == 3, ()) {
        ::core::option::Option::Some(()) => {
          (_mm256_srai_epi32(sum_lo, 3), _mm256_srai_epi32(sum_hi, 3))
        }
        ::core::option::Option::None => {
          (_mm256_srai_epi32(sum_lo, 5), _mm256_srai_epi32(sum_hi, 5))
        }
      };

      let clamped_lo = _mm256_min_epi32(_mm256_max_epi32(result_lo, zero), max_vec);
      let clamped_hi = _mm256_min_epi32(_mm256_max_epi32(result_hi, zero), max_vec);

      let packed = _mm256_packus_epi32(clamped_lo, clamped_hi);
      let packed_2 = _mm256_permute4x64_epi64(packed, 0b11011000);

      storeu_256!(target, packed_2);
    }

    let Ok(pixels) = row.pixels();

    for (target, first, second) in pixels {
      let sum = ::core::primitive::i32::wrapping_add(
        <i32 as ::core::convert::From<::core::primitive::i16>>::from(*first),
        <i32 as ::core::convert::From<::core::primitive::i16>>::from(*second),
      );
      let output_sample =
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
          i32::wrapping_shr(i32::wrapping_add(sum, rnd), i32::cast_unsigned(sh)).clamp(0, max),
        ) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
      *target = output_sample;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn avg_16bpc_avx512_safe(
  _token: Server64,
  dst: &mut [u8],
  byte_stride: u64,
  tmp1: &[i16; COMPINTER_LEN],
  tmp2: &[i16; COMPINTER_LEN],
  block_extent: Size<i32>,
  bitdepth_max: i32,
) -> Result<(), BlendLayoutError> {
  let Size {
    width: w,
    height: h,
  } = block_extent;
  let size = BlendSize::new(Size {
    width: w,
    height: h,
  })?;

  let intermediate_bits = match ::core::primitive::bool::then_some(
    ::core::primitive::i32::wrapping_shr(bitdepth_max, 11_u32) != 0,
    (),
  ) {
    ::core::option::Option::Some(()) => 2i32,
    ::core::option::Option::None => 4i32,
  };
  let sh = i32::wrapping_add(intermediate_bits, 1);
  let rnd = i32::wrapping_add(
    i32::wrapping_shl(1, i32::cast_unsigned(intermediate_bits)),
    ::core::primitive::i32::wrapping_mul(8192, 2),
  );
  let max =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(bitdepth_max) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };

  let rnd_vec = _mm512_set1_epi32(rnd);
  let zero = _mm512_setzero_si512();
  let max_vec = _mm512_set1_epi32(max);

  let rows = size.blend_rows::<u16>([tmp1, tmp2], dst, byte_stride)?;

  for mut row in rows {
    let Ok(blocks_32) = row.blocks::<32>();

    for (target, first, second) in blocks_32 {
      let t1_full = loadu_512!(first);
      let t2_full = loadu_512!(second);

      let t1_lo = _mm512_cvtepi16_epi32(_mm512_castsi512_si256(t1_full));
      let t2_lo = _mm512_cvtepi16_epi32(_mm512_castsi512_si256(t2_full));
      let sum_lo = _mm512_add_epi32(_mm512_add_epi32(t1_lo, t2_lo), rnd_vec);
      let result_lo = match ::core::primitive::bool::then_some(sh == 3, ()) {
        ::core::option::Option::Some(()) => _mm512_srai_epi32::<3>(sum_lo),
        ::core::option::Option::None => _mm512_srai_epi32::<5>(sum_lo),
      };
      let clamped_lo = _mm512_min_epi32(_mm512_max_epi32(result_lo, zero), max_vec);
      let packed_lo: __m256i = _mm512_cvtusepi32_epi16(clamped_lo);

      let t1_hi = _mm512_cvtepi16_epi32(_mm512_extracti64x4_epi64::<1>(t1_full));
      let t2_hi = _mm512_cvtepi16_epi32(_mm512_extracti64x4_epi64::<1>(t2_full));
      let sum_hi = _mm512_add_epi32(_mm512_add_epi32(t1_hi, t2_hi), rnd_vec);
      let result_hi = match ::core::primitive::bool::then_some(sh == 3, ()) {
        ::core::option::Option::Some(()) => _mm512_srai_epi32::<3>(sum_hi),
        ::core::option::Option::None => _mm512_srai_epi32::<5>(sum_hi),
      };
      let clamped_hi = _mm512_min_epi32(_mm512_max_epi32(result_hi, zero), max_vec);
      let packed_hi: __m256i = _mm512_cvtusepi32_epi16(clamped_hi);

      let combined = _mm512_inserti64x4::<1>(_mm512_castsi256_si512(packed_lo), packed_hi);
      storeu_512!(target, combined);
    }

    let Ok(blocks_16) = row.blocks::<16>();

    for (target, first, second) in blocks_16 {
      let t1 = loadu_256!(first);
      let t2 = loadu_256!(second);
      let t1_lo = _mm256_cvtepi16_epi32(_mm256_castsi256_si128(t1));
      let t2_lo = _mm256_cvtepi16_epi32(_mm256_castsi256_si128(t2));
      let t1_hi = _mm256_cvtepi16_epi32(_mm256_extracti128_si256(t1, 1));
      let t2_hi = _mm256_cvtepi16_epi32(_mm256_extracti128_si256(t2, 1));

      let rnd_256 = _mm256_set1_epi32(rnd);
      let sum_lo = _mm256_add_epi32(_mm256_add_epi32(t1_lo, t2_lo), rnd_256);
      let sum_hi = _mm256_add_epi32(_mm256_add_epi32(t1_hi, t2_hi), rnd_256);

      let (result_lo, result_hi) = match ::core::primitive::bool::then_some(sh == 3, ()) {
        ::core::option::Option::Some(()) => {
          (_mm256_srai_epi32(sum_lo, 3), _mm256_srai_epi32(sum_hi, 3))
        }
        ::core::option::Option::None => {
          (_mm256_srai_epi32(sum_lo, 5), _mm256_srai_epi32(sum_hi, 5))
        }
      };

      let zero_256 = _mm256_setzero_si256();
      let max_256 = _mm256_set1_epi32(max);
      let clamped_lo = _mm256_min_epi32(_mm256_max_epi32(result_lo, zero_256), max_256);
      let clamped_hi = _mm256_min_epi32(_mm256_max_epi32(result_hi, zero_256), max_256);

      let packed = _mm256_packus_epi32(clamped_lo, clamped_hi);
      let packed_2 = _mm256_permute4x64_epi64(packed, 0b11011000);

      storeu_256!(target, packed_2);
    }

    let Ok(pixels) = row.pixels();

    for (target, first, second) in pixels {
      let sum = ::core::primitive::i32::wrapping_add(
        <i32 as ::core::convert::From<::core::primitive::i16>>::from(*first),
        <i32 as ::core::convert::From<::core::primitive::i16>>::from(*second),
      );
      let output_sample =
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
          i32::wrapping_shr(i32::wrapping_add(sum, rnd), i32::cast_unsigned(sh)).clamp(0, max),
        ) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
      *target = output_sample;
    }
  }

  Ok(())
}

const PW_2048: i16 = 2048;

#[cfg(target_arch = "x86_64")]
#[inline]
fn weighted_8bpc_inputs(
  [first, second]: [&[i16]; 2],
  weight: i32,
) -> Result<([&[i16]; 2], i16), console_core_never::Never> {
  let (ordered, coefficient) = match weight {
    8.. => ([first, second], weight.wrapping_sub(16)),
    ..=7 => ([second, first], weight.wrapping_neg()),
  };
  let Ok(scaled) =
    console_core_number_conversion::wrapped::<_, i16>(coefficient.wrapping_shl(12));
  Ok((ordered, scaled))
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn weighted_8bpc_sample(
  [first, second]: [i16; 2],
  coefficient: i16,
) -> Result<u8, console_core_never::Never> {
  let a = i32::from(first);
  let b = i32::from(second);
  let weighted_difference = ::core::primitive::i32::wrapping_shr(
    a.wrapping_sub(b).wrapping_mul(i32::from(coefficient)),
    16_u32,
  );
  let rounded = ::core::primitive::i32::wrapping_shr(
    a.wrapping_add(weighted_difference).wrapping_add(8),
    4_u32,
  );
  console_core_number_conversion::wrapped(rounded.clamp(0, 255))
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn w_avg_8bpc_avx2_safe(
  _token: Desktop64,
  dst: &mut [u8],
  byte_stride: u64,
  tmp1: &[i16; COMPINTER_LEN],
  tmp2: &[i16; COMPINTER_LEN],
  block_extent: Size<i32>,
  weight: i32,
) -> Result<(), BlendLayoutError> {
  let Size {
    width: w,
    height: h,
  } = block_extent;
  let size = BlendSize::new(Size {
    width: w,
    height: h,
  })?;

  let Ok((ordered_inputs, weight_scaled)) = weighted_8bpc_inputs([tmp1, tmp2], weight);

  let weight_vec = _mm256_set1_epi16(weight_scaled);
  let round = _mm256_set1_epi16(PW_2048);

  let rows = size.blend_rows::<u8>(ordered_inputs, dst, byte_stride)?;

  for mut row in rows {
    let Ok(blocks_32) = row.blocks::<32>();

    for (target, first, second) in blocks_32 {
      let [first_low, first_high]: &[[i16; 16]; 2] = zerocopy::transmute_ref!(first);
      let [second_low, second_high]: &[[i16; 16]; 2] = zerocopy::transmute_ref!(second);
      let t1_lo = loadu_256!(first_low);
      let t1_hi = loadu_256!(first_high);
      let t2_lo = loadu_256!(second_low);
      let t2_hi = loadu_256!(second_high);

      let diff_lo = _mm256_sub_epi16(t1_lo, t2_lo);
      let diff_hi = _mm256_sub_epi16(t1_hi, t2_hi);

      let scaled_lo = _mm256_mulhi_epi16(diff_lo, weight_vec);
      let scaled_hi = _mm256_mulhi_epi16(diff_hi, weight_vec);

      let sum_lo = _mm256_add_epi16(t1_lo, scaled_lo);
      let sum_hi = _mm256_add_epi16(t1_hi, scaled_hi);

      let avg_lo = _mm256_mulhrs_epi16(sum_lo, round);
      let avg_hi = _mm256_mulhrs_epi16(sum_hi, round);

      let packed = _mm256_packus_epi16(avg_lo, avg_hi);
      let result = _mm256_permute4x64_epi64(packed, 0b11_01_10_00);

      storeu_256!(target, result);
    }

    let Ok(pixels) = row.pixels();

    for (target, first, second) in pixels {
      let Ok(sample) = weighted_8bpc_sample([*first, *second], weight_scaled);
      *target = sample;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn w_avg_8bpc_avx512_safe(
  _token: Server64,
  dst: &mut [u8],
  byte_stride: u64,
  tmp1: &[i16; COMPINTER_LEN],
  tmp2: &[i16; COMPINTER_LEN],
  block_extent: Size<i32>,
  weight: i32,
) -> Result<(), BlendLayoutError> {
  let Size {
    width: w,
    height: h,
  } = block_extent;
  let size = BlendSize::new(Size {
    width: w,
    height: h,
  })?;

  let Ok((ordered_inputs, weight_scaled)) = weighted_8bpc_inputs([tmp1, tmp2], weight);

  let weight_vec = _mm512_set1_epi16(weight_scaled);
  let round = _mm512_set1_epi16(PW_2048);
  let zero = _mm512_setzero_si512();

  let rows = size.blend_rows::<u8>(ordered_inputs, dst, byte_stride)?;

  for mut row in rows {
    let Ok(blocks_64) = row.blocks::<64>();

    for (target, first, second) in blocks_64 {
      let [first_low, first_high]: &[[i16; 32]; 2] = zerocopy::transmute_ref!(first);
      let [second_low, second_high]: &[[i16; 32]; 2] = zerocopy::transmute_ref!(second);
      let t1_lo = loadu_512!(first_low);
      let t2_lo = loadu_512!(second_low);
      let diff_lo = _mm512_sub_epi16(t1_lo, t2_lo);
      let scaled_lo = _mm512_mulhi_epi16(diff_lo, weight_vec);
      let sum_lo = _mm512_add_epi16(t1_lo, scaled_lo);
      let avg_lo = _mm512_mulhrs_epi16(sum_lo, round);
      let avg_lo_2 = _mm512_max_epi16(avg_lo, zero);
      let result_lo: __m256i = _mm512_cvtusepi16_epi8(avg_lo_2);

      let t1_hi = loadu_512!(first_high);
      let t2_hi = loadu_512!(second_high);
      let diff_hi = _mm512_sub_epi16(t1_hi, t2_hi);
      let scaled_hi = _mm512_mulhi_epi16(diff_hi, weight_vec);
      let sum_hi = _mm512_add_epi16(t1_hi, scaled_hi);
      let avg_hi = _mm512_mulhrs_epi16(sum_hi, round);
      let avg_hi_2 = _mm512_max_epi16(avg_hi, zero);
      let result_hi: __m256i = _mm512_cvtusepi16_epi8(avg_hi_2);

      let combined = _mm512_inserti64x4::<1>(_mm512_castsi256_si512(result_lo), result_hi);
      storeu_512!(target, combined);
    }

    let Ok(blocks_32) = row.blocks::<32>();

    for (target, first, second) in blocks_32 {
      let t1 = loadu_512!(first);
      let t2 = loadu_512!(second);
      let diff = _mm512_sub_epi16(t1, t2);
      let scaled = _mm512_mulhi_epi16(diff, weight_vec);
      let sum = _mm512_add_epi16(t1, scaled);
      let avg = _mm512_mulhrs_epi16(sum, round);
      let avg_2 = _mm512_max_epi16(avg, zero);
      let result: __m256i = _mm512_cvtusepi16_epi8(avg_2);

      storeu_256!(target, result);
    }

    let Ok(pixels) = row.pixels();

    for (target, first, second) in pixels {
      let Ok(sample) = weighted_8bpc_sample([*first, *second], weight_scaled);
      *target = sample;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct WeightedAverageScale {
  weight: i32,
  bitdepth_max: i32,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn w_avg_16bpc_avx2_safe(
  _token: Desktop64,
  dst: &mut [u8],
  byte_stride: u64,
  tmp1: &[i16; COMPINTER_LEN],
  tmp2: &[i16; COMPINTER_LEN],
  block_extent: Size<i32>,
  scale: WeightedAverageScale,
) -> Result<(), BlendLayoutError> {
  let WeightedAverageScale {
    weight,
    bitdepth_max,
  } = scale;
  let Size {
    width: w,
    height: h,
  } = block_extent;
  let size = BlendSize::new(Size {
    width: w,
    height: h,
  })?;

  let intermediate_bits = match ::core::primitive::bool::then_some(
    ::core::primitive::i32::wrapping_shr(bitdepth_max, 11_u32) != 0,
    (),
  ) {
    ::core::option::Option::Some(()) => 2i32,
    ::core::option::Option::None => 4i32,
  };

  let sh = i32::wrapping_add(intermediate_bits, 4);
  let rnd = i32::wrapping_add(
    i32::wrapping_shl(8, i32::cast_unsigned(intermediate_bits)),
    ::core::primitive::i32::wrapping_mul(8192, 16),
  );
  let max =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(bitdepth_max) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  let inv_weight = i32::wrapping_sub(16, weight);

  let rnd_vec = _mm256_set1_epi32(rnd);
  let zero = _mm256_setzero_si256();
  let max_vec = _mm256_set1_epi32(max);
  let weight_vec = _mm256_set1_epi32(weight);
  let inv_weight_vec = _mm256_set1_epi32(inv_weight);

  let rows = size.blend_rows::<u16>([tmp1, tmp2], dst, byte_stride)?;

  for mut row in rows {
    let Ok(blocks_8) = row.blocks::<8>();

    for (target, first, second) in blocks_8 {
      let t1_16 = loadu_128!(first);
      let t2_16 = loadu_128!(second);
      let t1 = _mm256_cvtepi16_epi32(t1_16);
      let t2 = _mm256_cvtepi16_epi32(t2_16);

      let term1 = _mm256_mullo_epi32(t1, weight_vec);
      let term2 = _mm256_mullo_epi32(t2, inv_weight_vec);
      let sum = _mm256_add_epi32(_mm256_add_epi32(term1, term2), rnd_vec);

      let result = match ::core::primitive::bool::then_some(sh == 6, ()) {
        ::core::option::Option::Some(()) => _mm256_srai_epi32(sum, 6),
        ::core::option::Option::None => _mm256_srai_epi32(sum, 8),
      };

      let clamped = _mm256_min_epi32(_mm256_max_epi32(result, zero), max_vec);

      let packed = _mm256_packus_epi32(clamped, clamped);
      let lo128 = _mm256_castsi256_si128(packed);
      let hi128 = _mm256_extracti128_si256(packed, 1);
      let result_128 = _mm_unpacklo_epi64(lo128, hi128);

      storeu_128!(target, result_128);
    }

    let Ok(pixels) = row.pixels();

    for (target, first, second) in pixels {
      let a = <i32 as ::core::convert::From<::core::primitive::i16>>::from(*first);
      let b = <i32 as ::core::convert::From<::core::primitive::i16>>::from(*second);
      let output_sample = i32::wrapping_shr(
        i32::wrapping_add(
          i32::wrapping_add(
            i32::wrapping_mul(a, weight),
            i32::wrapping_mul(b, inv_weight),
          ),
          rnd,
        ),
        i32::cast_unsigned(sh),
      );
      *target = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
        output_sample.clamp(0, max),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn w_avg_16bpc_avx512_safe(
  _token: Server64,
  dst: &mut [u8],
  byte_stride: u64,
  tmp1: &[i16; COMPINTER_LEN],
  tmp2: &[i16; COMPINTER_LEN],
  block_extent: Size<i32>,
  scale: WeightedAverageScale,
) -> Result<(), BlendLayoutError> {
  let WeightedAverageScale {
    weight,
    bitdepth_max,
  } = scale;
  let Size {
    width: w,
    height: h,
  } = block_extent;
  let size = BlendSize::new(Size {
    width: w,
    height: h,
  })?;

  let intermediate_bits = match ::core::primitive::bool::then_some(
    ::core::primitive::i32::wrapping_shr(bitdepth_max, 11_u32) != 0,
    (),
  ) {
    ::core::option::Option::Some(()) => 2i32,
    ::core::option::Option::None => 4i32,
  };
  let sh = i32::wrapping_add(intermediate_bits, 4);
  let rnd = i32::wrapping_add(
    i32::wrapping_shl(8, i32::cast_unsigned(intermediate_bits)),
    ::core::primitive::i32::wrapping_mul(8192, 16),
  );
  let max =
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(bitdepth_max) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  let inv_weight = i32::wrapping_sub(16, weight);

  let rnd_vec = _mm512_set1_epi32(rnd);
  let zero = _mm512_setzero_si512();
  let max_vec = _mm512_set1_epi32(max);
  let weight_vec = _mm512_set1_epi32(weight);
  let inv_weight_vec = _mm512_set1_epi32(inv_weight);

  let rows = size.blend_rows::<u16>([tmp1, tmp2], dst, byte_stride)?;

  for mut row in rows {
    let Ok(blocks_16) = row.blocks::<16>();

    for (target, first, second) in blocks_16 {
      let t1 = _mm512_cvtepi16_epi32(loadu_256!(first));
      let t2 = _mm512_cvtepi16_epi32(loadu_256!(second));

      let term1 = _mm512_mullo_epi32(t1, weight_vec);
      let term2 = _mm512_mullo_epi32(t2, inv_weight_vec);
      let sum = _mm512_add_epi32(_mm512_add_epi32(term1, term2), rnd_vec);

      let result = match ::core::primitive::bool::then_some(sh == 6, ()) {
        ::core::option::Option::Some(()) => _mm512_srai_epi32::<6>(sum),
        ::core::option::Option::None => _mm512_srai_epi32::<8>(sum),
      };

      let clamped = _mm512_min_epi32(_mm512_max_epi32(result, zero), max_vec);
      let packed: __m256i = _mm512_cvtusepi32_epi16(clamped);

      storeu_256!(target, packed);
    }

    let Ok(pixels) = row.pixels();

    for (target, first, second) in pixels {
      let a = <i32 as ::core::convert::From<::core::primitive::i16>>::from(*first);
      let b = <i32 as ::core::convert::From<::core::primitive::i16>>::from(*second);
      let output_sample = i32::wrapping_shr(
        i32::wrapping_add(
          i32::wrapping_add(
            i32::wrapping_mul(a, weight),
            i32::wrapping_mul(b, inv_weight),
          ),
          rnd,
        ),
        i32::cast_unsigned(sh),
      );
      *target = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
        output_sample.clamp(0, max),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[derive(Clone, Copy)]
struct MaskBlendRounding {
  adjustment: i32,
  fraction_bits: u32,
  maximum: i32,
}

#[cfg(target_arch = "x86_64")]
impl MaskBlendRounding {
  const BYTE: Self = Self {
    adjustment: 512,
    fraction_bits: 10,
    maximum: 255,
  };

  #[inline]
  fn for_word(maximum: i32) -> Result<Self, console_core_never::Never> {
    let intermediate_bits = match ::core::primitive::i32::wrapping_shr(maximum, 11_u32) {
      0 => 4,
      _ => 2,
    };
    Ok(Self {
      adjustment: 32i32.wrapping_shl(intermediate_bits).wrapping_add(524288),
      fraction_bits: intermediate_bits.wrapping_add(6),
      maximum,
    })
  }

  #[inline]
  fn sample(
    self,
    [first, second]: [i16; 2],
    alpha: u8,
  ) -> Result<i32, console_core_never::Never> {
    let factor = i32::from(alpha);
    let first_term = i32::from(first).wrapping_mul(factor);
    let second_term = i32::from(second).wrapping_mul(64i32.wrapping_sub(factor));
    let adjusted = first_term
      .wrapping_add(second_term)
      .wrapping_add(self.adjustment);
    Ok(
      adjusted
        .wrapping_shr(self.fraction_bits)
        .clamp(0, self.maximum),
    )
  }
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn mask_8bpc_avx2_safe(
  _token: Desktop64,
  dst: &mut [u8],
  byte_stride: u64,
  tmp1: &[i16; COMPINTER_LEN],
  tmp2: &[i16; COMPINTER_LEN],
  block_extent: Size<i32>,
  mask: &[u8],
) -> Result<(), BlendLayoutError> {
  let Size {
    width: w,
    height: h,
  } = block_extent;
  let size = BlendSize::new(Size {
    width: w,
    height: h,
  })?;

  let rounding = MaskBlendRounding::BYTE;
  let rnd = _mm256_set1_epi32(rounding.adjustment);

  let rows = size.blend_rows::<u8>([tmp1, tmp2], dst, byte_stride)?;
  let mask_rows = size.input_rows(mask)?;

  for (mut row, mut mask_row) in rows.zip(mask_rows) {
    let Ok(blocks_16) = row.blocks::<16>();
    let (masks_16, mask_tail_16) = mask_row.as_chunks::<16>();
    mask_row = mask_tail_16;

    for ((target, first, second), mask_block) in blocks_16.zip(masks_16) {
      let t1_lo = loadu_256!(first);
      let t2_lo = loadu_256!(second);
      let mask_bytes = loadu_128!(mask_block);
      let mask_lo = _mm256_cvtepu8_epi16(mask_bytes);

      let diff = _mm256_sub_epi16(t1_lo, t2_lo);

      let diff_lo = _mm256_cvtepi16_epi32(_mm256_castsi256_si128(diff));
      let mask_lo_32 = _mm256_cvtepi16_epi32(_mm256_castsi256_si128(mask_lo));
      let t2_lo_32 = _mm256_cvtepi16_epi32(_mm256_castsi256_si128(t2_lo));

      let prod_lo = _mm256_mullo_epi32(diff_lo, mask_lo_32);
      let t2_64_lo = _mm256_slli_epi32(t2_lo_32, 6);
      let sum_lo = _mm256_add_epi32(_mm256_add_epi32(prod_lo, t2_64_lo), rnd);

      let diff_hi = _mm256_cvtepi16_epi32(_mm256_extracti128_si256(diff, 1));
      let mask_hi_32 = _mm256_cvtepi16_epi32(_mm256_extracti128_si256(mask_lo, 1));
      let t2_hi_32 = _mm256_cvtepi16_epi32(_mm256_extracti128_si256(t2_lo, 1));

      let prod_hi = _mm256_mullo_epi32(diff_hi, mask_hi_32);
      let t2_64_hi = _mm256_slli_epi32(t2_hi_32, 6);
      let sum_hi = _mm256_add_epi32(_mm256_add_epi32(prod_hi, t2_64_hi), rnd);

      let result_lo = _mm256_srai_epi32(sum_lo, 10);
      let result_hi = _mm256_srai_epi32(sum_hi, 10);

      let result_16 = _mm256_packs_epi32(result_lo, result_hi);
      let result_16_2 = _mm256_permute4x64_epi64(result_16, 0b11011000);

      let result_8 = _mm256_packus_epi16(result_16_2, result_16_2);
      let result_8_2 = _mm256_permute4x64_epi64(result_8, 0b11011000);

      storeu_128!(target, _mm256_castsi256_si128(result_8_2));
    }

    let Ok(pixels) = row.pixels();

    for ((target, first, second), alpha) in pixels.zip(mask_row) {
      let Ok(sample) = rounding.sample([*first, *second], *alpha);
      let Ok(pixel) = console_core_number_conversion::wrapped::<_, u8>(sample);
      *target = pixel;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn mask_8bpc_avx512_safe(
  _token: Server64,
  dst: &mut [u8],
  byte_stride: u64,
  tmp1: &[i16; COMPINTER_LEN],
  tmp2: &[i16; COMPINTER_LEN],
  block_extent: Size<i32>,
  mask: &[u8],
) -> Result<(), BlendLayoutError> {
  let Size {
    width: w,
    height: h,
  } = block_extent;
  let size = BlendSize::new(Size {
    width: w,
    height: h,
  })?;

  let rounding = MaskBlendRounding::BYTE;
  let rnd = _mm512_set1_epi32(rounding.adjustment);

  let rows = size.blend_rows::<u8>([tmp1, tmp2], dst, byte_stride)?;
  let mask_rows = size.input_rows(mask)?;

  for (mut row, mut mask_row) in rows.zip(mask_rows) {
    let Ok(blocks_32) = row.blocks::<32>();
    let (masks_32, mask_tail_32) = mask_row.as_chunks::<32>();
    mask_row = mask_tail_32;

    for ((target, first, second), mask_block) in blocks_32.zip(masks_32) {
      let t1_full = loadu_512!(first);
      let t2_full = loadu_512!(second);
      let mask_bytes = loadu_256!(mask_block);
      let mask_16 = _mm512_cvtepu8_epi16(mask_bytes);
      let diff = _mm512_sub_epi16(t1_full, t2_full);

      let diff_lo = _mm512_cvtepi16_epi32(_mm512_castsi512_si256(diff));
      let mask_lo = _mm512_cvtepi16_epi32(_mm512_castsi512_si256(mask_16));
      let t2_lo = _mm512_cvtepi16_epi32(_mm512_castsi512_si256(t2_full));

      let prod_lo = _mm512_mullo_epi32(diff_lo, mask_lo);
      let t2_64_lo = _mm512_slli_epi32(t2_lo, 6);
      let sum_lo = _mm512_add_epi32(_mm512_add_epi32(prod_lo, t2_64_lo), rnd);
      let result_lo = _mm512_srai_epi32::<10>(sum_lo);

      let diff_hi = _mm512_cvtepi16_epi32(_mm512_extracti64x4_epi64::<1>(diff));
      let mask_hi = _mm512_cvtepi16_epi32(_mm512_extracti64x4_epi64::<1>(mask_16));
      let t2_hi = _mm512_cvtepi16_epi32(_mm512_extracti64x4_epi64::<1>(t2_full));

      let prod_hi = _mm512_mullo_epi32(diff_hi, mask_hi);
      let t2_64_hi = _mm512_slli_epi32(t2_hi, 6);
      let sum_hi = _mm512_add_epi32(_mm512_add_epi32(prod_hi, t2_64_hi), rnd);
      let result_hi = _mm512_srai_epi32::<10>(sum_hi);

      let result_16 = _mm512_packs_epi32(result_lo, result_hi);

      let perm_idx = _mm512_set_epi64(7, 5, 3, 1, 6, 4, 2, 0);
      let result_16_2 = _mm512_permutexvar_epi64(perm_idx, result_16);

      let result_8: __m256i =
        _mm512_cvtusepi16_epi8(_mm512_max_epi16(result_16_2, _mm512_setzero_si512()));

      storeu_256!(target, result_8);
    }

    let Ok(blocks_16) = row.blocks::<16>();
    let (masks_16, mask_tail_16) = mask_row.as_chunks::<16>();
    mask_row = mask_tail_16;

    for ((target, first, second), mask_block) in blocks_16.zip(masks_16) {
      let t1_lo = loadu_256!(first);
      let t2_lo = loadu_256!(second);
      let mask_bytes = loadu_128!(mask_block);
      let mask_lo = _mm256_cvtepu8_epi16(mask_bytes);

      let diff = _mm256_sub_epi16(t1_lo, t2_lo);

      let diff_lo = _mm256_cvtepi16_epi32(_mm256_castsi256_si128(diff));
      let mask_lo_32 = _mm256_cvtepi16_epi32(_mm256_castsi256_si128(mask_lo));
      let t2_lo_32 = _mm256_cvtepi16_epi32(_mm256_castsi256_si128(t2_lo));

      let prod_lo = _mm256_mullo_epi32(diff_lo, mask_lo_32);
      let t2_64_lo = _mm256_slli_epi32(t2_lo_32, 6);
      let sum_lo = _mm256_add_epi32(_mm256_add_epi32(prod_lo, t2_64_lo), _mm256_set1_epi32(512));

      let diff_hi = _mm256_cvtepi16_epi32(_mm256_extracti128_si256(diff, 1));
      let mask_hi_32 = _mm256_cvtepi16_epi32(_mm256_extracti128_si256(mask_lo, 1));
      let t2_hi_32 = _mm256_cvtepi16_epi32(_mm256_extracti128_si256(t2_lo, 1));

      let prod_hi = _mm256_mullo_epi32(diff_hi, mask_hi_32);
      let t2_64_hi = _mm256_slli_epi32(t2_hi_32, 6);
      let sum_hi = _mm256_add_epi32(_mm256_add_epi32(prod_hi, t2_64_hi), _mm256_set1_epi32(512));

      let result_lo = _mm256_srai_epi32(sum_lo, 10);
      let result_hi = _mm256_srai_epi32(sum_hi, 10);

      let result_16 = _mm256_packs_epi32(result_lo, result_hi);
      let result_16_3 = _mm256_permute4x64_epi64(result_16, 0b11011000);
      let result_8 = _mm256_packus_epi16(result_16_3, result_16_3);
      let result_8_2 = _mm256_permute4x64_epi64(result_8, 0b11011000);

      storeu_128!(target, _mm256_castsi256_si128(result_8_2));
    }

    let Ok(pixels) = row.pixels();

    for ((target, first, second), alpha) in pixels.zip(mask_row) {
      let Ok(sample) = rounding.sample([*first, *second], *alpha);
      let Ok(pixel) = console_core_number_conversion::wrapped::<_, u8>(sample);
      *target = pixel;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn mask_word_eight_lanes(
  _token: Desktop64,
  [first, second]: [&[i16; 8]; 2],
  alpha: &[u8; 8],
  rounding: MaskBlendRounding,
) -> Result<__m128i, console_core_never::Never> {
  let first_values = _mm256_cvtepi16_epi32(loadu_128!(first));
  let second_values = _mm256_cvtepi16_epi32(loadu_128!(second));
  let alpha_values = _mm256_cvtepu8_epi32(loadu_64!(alpha));
  let inverse = _mm256_sub_epi32(_mm256_set1_epi32(64), alpha_values);
  let first_term = _mm256_mullo_epi32(first_values, alpha_values);
  let second_term = _mm256_mullo_epi32(second_values, inverse);
  let sum = _mm256_add_epi32(
    _mm256_add_epi32(first_term, second_term),
    _mm256_set1_epi32(rounding.adjustment),
  );
  let shifted = match rounding.fraction_bits {
    8 => _mm256_srai_epi32(sum, 8),
    _ => _mm256_srai_epi32(sum, 10),
  };
  let clipped = _mm256_min_epi32(
    _mm256_max_epi32(shifted, _mm256_setzero_si256()),
    _mm256_set1_epi32(rounding.maximum),
  );
  let packed = _mm256_packus_epi32(clipped, clipped);
  let low = _mm256_castsi256_si128(packed);
  let high = _mm256_extracti128_si256(packed, 1);
  Ok(_mm_unpacklo_epi64(low, high))
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn mask_16bpc_avx512_safe(
  token: Server64,
  dst: &mut [u8],
  byte_stride: u64,
  tmp1: &[i16; COMPINTER_LEN],
  tmp2: &[i16; COMPINTER_LEN],
  block_extent: Size<i32>,
  mask: &[u8],
  bitdepth_max: i32,
) -> Result<(), BlendLayoutError> {
  let narrow = token.v3();
  let Size {
    width: w,
    height: h,
  } = block_extent;
  let size = BlendSize::new(Size {
    width: w,
    height: h,
  })?;
  let Ok(rounding) = MaskBlendRounding::for_word(bitdepth_max);
  let MaskBlendRounding {
    adjustment: rnd,
    fraction_bits: sh,
    maximum: max,
  } = rounding;

  let rnd_vec = _mm512_set1_epi32(rnd);
  let zero = _mm512_setzero_si512();
  let max_vec = _mm512_set1_epi32(max);
  let sixty_four = _mm512_set1_epi32(64);

  let rows = size.blend_rows::<u16>([tmp1, tmp2], dst, byte_stride)?;
  let mask_rows = size.input_rows(mask)?;

  for (mut row, mut mask_row) in rows.zip(mask_rows) {
    let Ok(blocks_16) = row.blocks::<16>();
    let (masks_16, mask_tail_16) = mask_row.as_chunks::<16>();
    mask_row = mask_tail_16;

    for ((target, first, second), mask_block) in blocks_16.zip(masks_16) {
      let t1_16 = loadu_256!(first);
      let t2_16 = loadu_256!(second);
      let t1 = _mm512_cvtepi16_epi32(t1_16);
      let t2 = _mm512_cvtepi16_epi32(t2_16);

      let mask_128 = loadu_128!(mask_block);
      let mask_32 = _mm512_cvtepu8_epi32(mask_128);

      let inv_mask = _mm512_sub_epi32(sixty_four, mask_32);

      let term1 = _mm512_mullo_epi32(t1, mask_32);
      let term2 = _mm512_mullo_epi32(t2, inv_mask);
      let sum = _mm512_add_epi32(_mm512_add_epi32(term1, term2), rnd_vec);

      let result = match ::core::primitive::bool::then_some(sh == 8, ()) {
        ::core::option::Option::Some(()) => _mm512_srai_epi32::<8>(sum),
        ::core::option::Option::None => _mm512_srai_epi32::<10>(sum),
      };
      let clamped = _mm512_min_epi32(_mm512_max_epi32(result, zero), max_vec);

      let packed = _mm512_cvtusepi32_epi16(clamped);
      storeu_256!(target, packed);
    }

    let Ok(blocks_8) = row.blocks::<8>();
    let (masks_8, mask_tail_8) = mask_row.as_chunks::<8>();
    mask_row = mask_tail_8;

    for ((target, first, second), mask_block) in blocks_8.zip(masks_8) {
      let Ok(pixels) = mask_word_eight_lanes(narrow, [first, second], mask_block, rounding);
      storeu_128!(target, pixels);
    }

    let Ok(pixels) = row.pixels();

    for ((target, first, second), alpha) in pixels.zip(mask_row) {
      let Ok(sample) = rounding.sample([*first, *second], *alpha);
      let Ok(pixel) = console_core_number_conversion::wrapped::<_, u16>(sample);
      *target = pixel;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn mask_16bpc_avx2_safe(
  token: Desktop64,
  dst: &mut [u8],
  byte_stride: u64,
  tmp1: &[i16; COMPINTER_LEN],
  tmp2: &[i16; COMPINTER_LEN],
  block_extent: Size<i32>,
  mask: &[u8],
  bitdepth_max: i32,
) -> Result<(), BlendLayoutError> {
  let Size {
    width: w,
    height: h,
  } = block_extent;
  let size = BlendSize::new(Size {
    width: w,
    height: h,
  })?;
  let Ok(rounding) = MaskBlendRounding::for_word(bitdepth_max);

  let rows = size.blend_rows::<u16>([tmp1, tmp2], dst, byte_stride)?;
  let mask_rows = size.input_rows(mask)?;

  for (mut row, mut mask_row) in rows.zip(mask_rows) {
    let Ok(blocks_8) = row.blocks::<8>();
    let (masks_8, mask_tail_8) = mask_row.as_chunks::<8>();
    mask_row = mask_tail_8;

    for ((target, first, second), mask_block) in blocks_8.zip(masks_8) {
      let Ok(pixels) = mask_word_eight_lanes(token, [first, second], mask_block, rounding);
      storeu_128!(target, pixels);
    }

    let Ok(pixels) = row.pixels();

    for ((target, first, second), alpha) in pixels.zip(mask_row) {
      let Ok(sample) = rounding.sample([*first, *second], *alpha);
      let Ok(pixel) = console_core_number_conversion::wrapped::<_, u16>(sample);
      *target = pixel;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
use crate::src::internal::{SCRATCH_INTER_INTRA_BUF_LEN, SCRATCH_LAP_LEN};

#[cfg(target_arch = "x86_64")]
#[arcane]
fn blend_8bpc_avx2_safe(
  _token: Desktop64,
  dst: &mut [u8],
  dst_stride: u64,
  tmp: &[u8],
  block_extent: Size<i32>,
  mask: &[u8],
) -> Rav1dResult {
  let Size {
    width: w,
    height: h,
  } = block_extent;
  let shape = BlendSize::new(Size {
    width: w,
    height: h,
  })
  .map_err(|_shape| Rav1dError::EINVAL)?;
  let width = w.cast_unsigned();
  let tmp_pixels = tmp;
  let tmp_rows = shape
    .input_rows(tmp_pixels)
    .map_err(|_layout| Rav1dError::EINVAL)?;
  let masks = shape
    .input_rows(mask)
    .map_err(|_layout| Rav1dError::EINVAL)?;
  let dst_rows = shape
    .destination_rows::<u8>(dst, dst_stride)
    .map_err(|_layout| Rav1dError::EINVAL)?;

  let sixty_four = _mm256_set1_epi16(64);
  let rnd = _mm256_set1_epi16(32);

  for ((dst_row, tmp_row), mask_row) in dst_rows.zip(tmp_rows).zip(masks) {
    let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

    for column in (0..vector_columns).step_by(16) {
      let Ok(col) = console_core_number_conversion::index(column);
      let checked_value =
        <[_]>::get(&*dst_row, col..(Wrapping(col) + Wrapping(16)).0).ok_or(Rav1dError::EINVAL)?;
      let dst_bytes = loadu_128!(checked_value, [u8; 16]);
      let checked_value_2 =
        <[_]>::get(&*tmp_row, col..(Wrapping(col) + Wrapping(16)).0).ok_or(Rav1dError::EINVAL)?;
      let tmp_bytes = loadu_128!(checked_value_2, [u8; 16]);
      let checked_value_3 = <[_]>::get(&*mask_row, col..(Wrapping(col) + Wrapping(16)).0)
        .ok_or(Rav1dError::EINVAL)?;
      let mask_bytes = loadu_128!(checked_value_3, [u8; 16]);

      let dst_16 = _mm256_cvtepu8_epi16(dst_bytes);
      let tmp_16 = _mm256_cvtepu8_epi16(tmp_bytes);
      let mask_16 = _mm256_cvtepu8_epi16(mask_bytes);

      let inv_mask = _mm256_sub_epi16(sixty_four, mask_16);

      let term1 = _mm256_mullo_epi16(dst_16, inv_mask);
      let term2 = _mm256_mullo_epi16(tmp_16, mask_16);
      let sum = _mm256_add_epi16(_mm256_add_epi16(term1, term2), rnd);

      let result_16 = _mm256_srli_epi16(sum, 6);

      let result_8 = _mm256_packus_epi16(result_16, result_16);
      let result_8_2 = _mm256_permute4x64_epi64(result_8, 0b11011000);

      let checked_value_4 = <[_]>::get_mut(&mut *dst_row, col..(Wrapping(col) + Wrapping(16)).0)
        .ok_or(Rav1dError::EINVAL)?;
      storeu_128!(
        checked_value_4,
        [u8; 16],
        _mm256_castsi256_si128(result_8_2)
      );
    }

    for column in vector_columns..width {
      let Ok(col) = console_core_number_conversion::index(column);
      let checked_value_5 = <[_]>::get(&*dst_row, col).ok_or(Rav1dError::EINVAL)?;
      let a = <u32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_5);
      let checked_value_6 = <[_]>::get(&*tmp_row, col).ok_or(Rav1dError::EINVAL)?;
      let b = <u32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_6);
      let checked_value_7 = <[_]>::get(&*mask_row, col).ok_or(Rav1dError::EINVAL)?;
      let m = <u32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_7);
      let output_sample = ::core::primitive::u32::wrapping_shr(
        u32::wrapping_add(
          u32::wrapping_add(
            u32::wrapping_mul(a, u32::wrapping_sub(64, m)),
            ::core::primitive::u32::wrapping_mul(b, m),
          ),
          32,
        ),
        6_u32,
      );
      *{
        let checked_value_9 = <[_]>::get_mut(&mut *dst_row, col).ok_or(Rav1dError::EINVAL)?;
        checked_value_9
      } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        output_sample,
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn blend_16bpc_avx2_safe(
  _token: Desktop64,
  dst: &mut [u8],
  dst_stride: u64,
  tmp: &[u8],
  block_extent: Size<i32>,
  mask: &[u8],
) -> Rav1dResult {
  let Size {
    width: w,
    height: h,
  } = block_extent;
  let shape = BlendSize::new(Size {
    width: w,
    height: h,
  })
  .map_err(|_shape| Rav1dError::EINVAL)?;
  let width = w.cast_unsigned();
  let tmp_pixels: &[u16] =
    zerocopy::FromBytes::ref_from_bytes(tmp).map_err(|_layout| Rav1dError::EINVAL)?;
  let tmp_rows = shape
    .input_rows(tmp_pixels)
    .map_err(|_layout| Rav1dError::EINVAL)?;
  let masks = shape
    .input_rows(mask)
    .map_err(|_layout| Rav1dError::EINVAL)?;
  let dst_rows = shape
    .destination_rows::<u16>(dst, dst_stride)
    .map_err(|_layout| Rav1dError::EINVAL)?;

  let rnd = _mm256_set1_epi32(32);
  let sixty_four = _mm256_set1_epi32(64);

  for ((dst_row, tmp_row), mask_row) in dst_rows.zip(tmp_rows).zip(masks) {
    let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

    for column in (0..vector_columns).step_by(8) {
      let Ok(col) = console_core_number_conversion::index(column);
      let checked_value =
        <[_]>::get(&*dst_row, col..(Wrapping(col) + Wrapping(8)).0).ok_or(Rav1dError::EINVAL)?;
      let dst_16 = loadu_128!(checked_value, [u16; 8]);
      let checked_value_2 =
        <[_]>::get(&*tmp_row, col..(Wrapping(col) + Wrapping(8)).0).ok_or(Rav1dError::EINVAL)?;
      let tmp_16 = loadu_128!(checked_value_2, [u16; 8]);
      let dst_32 = _mm256_cvtepu16_epi32(dst_16);
      let tmp_32 = _mm256_cvtepu16_epi32(tmp_16);

      let mut mask_pad = [0u8; 16];

      {
        let checked_copy_destination =
          <[_]>::first_chunk_mut::<8>(&mut mask_pad).ok_or(Rav1dError::EINVAL)?;
        let checked_copy_source = <[_]>::get(&*mask_row, col..)
          .and_then(<[_]>::first_chunk::<8>)
          .ok_or(Rav1dError::EINVAL)?;
        *checked_copy_destination = *checked_copy_source;
      };

      let mask_bytes = loadu_128!(&mask_pad);
      let mask_32 = _mm256_cvtepu8_epi32(mask_bytes);

      let inv_mask = _mm256_sub_epi32(sixty_four, mask_32);

      let term1 = _mm256_mullo_epi32(dst_32, inv_mask);
      let term2 = _mm256_mullo_epi32(tmp_32, mask_32);
      let sum = _mm256_add_epi32(_mm256_add_epi32(term1, term2), rnd);

      let result = _mm256_srli_epi32(sum, 6);

      let packed = _mm256_packus_epi32(result, result);
      let lo128 = _mm256_castsi256_si128(packed);
      let hi128 = _mm256_extracti128_si256(packed, 1);
      let result_128 = _mm_unpacklo_epi64(lo128, hi128);

      let checked_value_3 = <[_]>::get_mut(&mut *dst_row, col..(Wrapping(col) + Wrapping(8)).0)
        .ok_or(Rav1dError::EINVAL)?;
      storeu_128!(checked_value_3, [u16; 8], result_128);
    }

    for column in vector_columns..width {
      let Ok(col) = console_core_number_conversion::index(column);
      let checked_value_4 = <[_]>::get(&*dst_row, col).ok_or(Rav1dError::EINVAL)?;
      let a = <u32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_4);
      let checked_value_5 = <[_]>::get(&*tmp_row, col).ok_or(Rav1dError::EINVAL)?;
      let b = <u32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_5);
      let checked_value_6 = <[_]>::get(&*mask_row, col).ok_or(Rav1dError::EINVAL)?;
      let m = <u32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_6);
      let output_sample = ::core::primitive::u32::wrapping_shr(
        u32::wrapping_add(
          u32::wrapping_add(
            u32::wrapping_mul(a, u32::wrapping_sub(64, m)),
            ::core::primitive::u32::wrapping_mul(b, m),
          ),
          32,
        ),
        6_u32,
      );
      *{
        let checked_value_11 = <[_]>::get_mut(&mut *dst_row, col).ok_or(Rav1dError::EINVAL)?;
        checked_value_11
      } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
        output_sample,
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
use crate::src::mc::{FilterExtent, fractional_filter};
#[cfg(target_arch = "x86_64")]
use crate::src::tables::dav1d_obmc_masks;
#[cfg(target_arch = "x86_64")]
use console_core_video_interpolation::round_shift;

#[cfg(target_arch = "x86_64")]
#[inline]
fn try_convolve8<C: Copy + Into<i32>>(
  filter: &[C; 8],
  mut sample_at: impl FnMut(u32) -> Rav1dResult<i32>,
) -> Rav1dResult<i32> {
  let mut sum = 0_i32;

  for (ordinal, &coefficient) in (0_u32..8).zip(filter) {
    let sample = sample_at(ordinal)?;
    let weight = Into::<i32>::into(coefficient);
    sum = sum.wrapping_add(weight.wrapping_mul(sample));
  }

  Ok(sum)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn blend_v_8bpc_avx2_safe(
  _token: Desktop64,
  dst: &mut [u8],
  dst_stride: u64,
  tmp: &[u8],
  block_extent: Size<i32>,
) -> Rav1dResult {
  let Size {
    width: w,
    height: h,
  } = block_extent;
  let checked_value_5 = w.checked_mul(3).ok_or(Rav1dError::EINVAL)?;
  let active_width = checked_value_5.wrapping_shr(2);
  let shape = BlendSize::new(Size {
    width: active_width,
    height: h,
  })
  .map_err(|_shape| Rav1dError::EINVAL)?;
  let Ok(w_2) = console_core_number_conversion::wrapped_index(w);
  let Ok(w_eff) = console_core_number_conversion::wrapped_index(active_width);
  let obmc_mask = crate::src::align::aligned_inner(&dav1d_obmc_masks)
    .get(w_2..w_2.wrapping_add(w_eff))
    .ok_or(Rav1dError::EINVAL)?;
  let tmp_pixels = tmp;
  let width = u64::try_from(active_width).map_err(|_negative| Rav1dError::EINVAL)?;
  let height = u64::try_from(h).map_err(|_negative| Rav1dError::EINVAL)?;
  let active = Size { width, height };
  let source_layout = console_core_pixel_buffers::RowLayout::new(active, 0, i64::from(w))
    .map_err(|_layout| Rav1dError::EINVAL)?;
  let tmp_rows = source_layout
    .rows(tmp_pixels)
    .map_err(|_layout| Rav1dError::EINVAL)?;
  let dst_rows = shape
    .destination_rows::<u8>(dst, dst_stride)
    .map_err(|_layout| Rav1dError::EINVAL)?;

  let rnd = _mm256_set1_epi16(32);
  let sixty_four = _mm256_set1_epi16(64);

  for (dst_row, tmp_row) in dst_rows.zip(tmp_rows) {
    let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

    for column in (0..vector_columns).step_by(16) {
      let Ok(col) = console_core_number_conversion::index(column);
      let checked_value =
        <[_]>::get(&*dst_row, col..(Wrapping(col) + Wrapping(16)).0).ok_or(Rav1dError::EINVAL)?;
      let dst_bytes = loadu_128!(checked_value, [u8; 16]);
      let checked_value_2 =
        <[_]>::get(&*tmp_row, col..(Wrapping(col) + Wrapping(16)).0).ok_or(Rav1dError::EINVAL)?;
      let tmp_bytes = loadu_128!(checked_value_2, [u8; 16]);
      let checked_value_3 = <[_]>::get(&*obmc_mask, col..(Wrapping(col) + Wrapping(16)).0)
        .ok_or(Rav1dError::EINVAL)?;
      let mask_bytes = loadu_128!(checked_value_3, [u8; 16]);

      let dst_16 = _mm256_cvtepu8_epi16(dst_bytes);
      let tmp_16 = _mm256_cvtepu8_epi16(tmp_bytes);
      let mask_16 = _mm256_cvtepu8_epi16(mask_bytes);
      let inv_mask = _mm256_sub_epi16(sixty_four, mask_16);

      let term1 = _mm256_mullo_epi16(dst_16, inv_mask);
      let term2 = _mm256_mullo_epi16(tmp_16, mask_16);
      let sum = _mm256_add_epi16(_mm256_add_epi16(term1, term2), rnd);
      let result_16 = _mm256_srli_epi16(sum, 6);

      let result_8 = _mm256_packus_epi16(result_16, result_16);
      let result_8_2 = _mm256_permute4x64_epi64(result_8, 0b11011000);

      let checked_value_4 = <[_]>::get_mut(&mut *dst_row, col..(Wrapping(col) + Wrapping(16)).0)
        .ok_or(Rav1dError::EINVAL)?;
      storeu_128!(
        checked_value_4,
        [u8; 16],
        _mm256_castsi256_si128(result_8_2)
      );
    }

    for column in vector_columns..width {
      let Ok(col) = console_core_number_conversion::index(column);
      let checked_value_6 = <[_]>::get(&*dst_row, col).ok_or(Rav1dError::EINVAL)?;
      let a = <u32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_6);
      let checked_value_7 = <[_]>::get(&*tmp_row, col).ok_or(Rav1dError::EINVAL)?;
      let b = <u32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_7);
      let checked_value_8 = <[_]>::get(&*obmc_mask, col).ok_or(Rav1dError::EINVAL)?;
      let m = <u32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_8);
      let output_sample = ::core::primitive::u32::wrapping_shr(
        u32::wrapping_add(
          u32::wrapping_add(
            u32::wrapping_mul(a, u32::wrapping_sub(64, m)),
            ::core::primitive::u32::wrapping_mul(b, m),
          ),
          32,
        ),
        6_u32,
      );
      *{
        let checked_value_9 = <[_]>::get_mut(&mut *dst_row, col).ok_or(Rav1dError::EINVAL)?;
        checked_value_9
      } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        output_sample,
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn blend_h_8bpc_avx2_safe(
  _token: Desktop64,
  dst: &mut [u8],
  dst_stride: u64,
  tmp: &[u8],
  block_extent: Size<i32>,
) -> Rav1dResult {
  let Size {
    width: w,
    height: h,
  } = block_extent;
  let checked_value_4 = h.checked_mul(3).ok_or(Rav1dError::EINVAL)?;
  let active_height = checked_value_4.wrapping_shr(2);
  let shape = BlendSize::new(Size {
    width: w,
    height: active_height,
  })
  .map_err(|_shape| Rav1dError::EINVAL)?;
  let Ok(h_2) = console_core_number_conversion::wrapped_index(h);
  let Ok(h_eff) = console_core_number_conversion::wrapped_index(active_height);
  let width = w.cast_unsigned();
  let obmc_mask = crate::src::align::aligned_inner(&dav1d_obmc_masks)
    .get(h_2..h_2.wrapping_add(h_eff))
    .ok_or(Rav1dError::EINVAL)?;
  let tmp_pixels = tmp;
  let tmp_rows = shape
    .input_rows(tmp_pixels)
    .map_err(|_layout| Rav1dError::EINVAL)?;
  let dst_rows = shape
    .destination_rows::<u8>(dst, dst_stride)
    .map_err(|_layout| Rav1dError::EINVAL)?;

  let rnd = _mm256_set1_epi16(32);
  let sixty_four = _mm256_set1_epi16(64);

  for ((dst_row, tmp_row), &row_weight) in dst_rows.zip(tmp_rows).zip(obmc_mask) {
    let m = row_weight;

    let mask_16 = _mm256_set1_epi16(<i16 as ::core::convert::From<::core::primitive::u8>>::from(
      m,
    ));
    let inv_mask = _mm256_sub_epi16(sixty_four, mask_16);

    let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

    for column in (0..vector_columns).step_by(16) {
      let Ok(col) = console_core_number_conversion::index(column);
      let checked_value =
        <[_]>::get(&*dst_row, col..(Wrapping(col) + Wrapping(16)).0).ok_or(Rav1dError::EINVAL)?;
      let dst_bytes = loadu_128!(checked_value, [u8; 16]);
      let checked_value_2 =
        <[_]>::get(&*tmp_row, col..(Wrapping(col) + Wrapping(16)).0).ok_or(Rav1dError::EINVAL)?;
      let tmp_bytes = loadu_128!(checked_value_2, [u8; 16]);

      let dst_16 = _mm256_cvtepu8_epi16(dst_bytes);
      let tmp_16 = _mm256_cvtepu8_epi16(tmp_bytes);

      let term1 = _mm256_mullo_epi16(dst_16, inv_mask);
      let term2 = _mm256_mullo_epi16(tmp_16, mask_16);
      let sum = _mm256_add_epi16(_mm256_add_epi16(term1, term2), rnd);
      let result_16 = _mm256_srli_epi16(sum, 6);

      let result_8 = _mm256_packus_epi16(result_16, result_16);
      let result_8_2 = _mm256_permute4x64_epi64(result_8, 0b11011000);

      let checked_value_3 = <[_]>::get_mut(&mut *dst_row, col..(Wrapping(col) + Wrapping(16)).0)
        .ok_or(Rav1dError::EINVAL)?;
      storeu_128!(
        checked_value_3,
        [u8; 16],
        _mm256_castsi256_si128(result_8_2)
      );
    }

    for column in vector_columns..width {
      let Ok(col) = console_core_number_conversion::index(column);
      let checked_value_5 = <[_]>::get(&*dst_row, col).ok_or(Rav1dError::EINVAL)?;
      let a = <u32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_5);
      let checked_value_6 = <[_]>::get(&*tmp_row, col).ok_or(Rav1dError::EINVAL)?;
      let b = <u32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_6);
      let output_sample = ::core::primitive::u32::wrapping_shr(
        u32::wrapping_add(
          u32::wrapping_add(
            u32::wrapping_mul(
              a,
              u32::wrapping_sub(
                64,
                <u32 as ::core::convert::From<::core::primitive::u8>>::from(m),
              ),
            ),
            ::core::primitive::u32::wrapping_mul(
              b,
              <u32 as ::core::convert::From<::core::primitive::u8>>::from(m),
            ),
          ),
          32,
        ),
        6_u32,
      );
      *{
        let checked_value_9 = <[_]>::get_mut(&mut *dst_row, col).ok_or(Rav1dError::EINVAL)?;
        checked_value_9
      } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        output_sample,
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn blend_v_16bpc_avx2_safe(
  _token: Desktop64,
  dst: &mut [u8],
  dst_stride: u64,
  tmp: &[u8],
  block_extent: Size<i32>,
) -> Rav1dResult {
  let Size {
    width: w,
    height: h,
  } = block_extent;
  let checked_value_4 = w.checked_mul(3).ok_or(Rav1dError::EINVAL)?;
  let active_width = checked_value_4.wrapping_shr(2);
  let shape = BlendSize::new(Size {
    width: active_width,
    height: h,
  })
  .map_err(|_shape| Rav1dError::EINVAL)?;
  let Ok(w_2) = console_core_number_conversion::wrapped_index(w);
  let Ok(w_eff) = console_core_number_conversion::wrapped_index(active_width);
  let obmc_mask = crate::src::align::aligned_inner(&dav1d_obmc_masks)
    .get(w_2..w_2.wrapping_add(w_eff))
    .ok_or(Rav1dError::EINVAL)?;
  let tmp_pixels: &[u16] =
    zerocopy::FromBytes::ref_from_bytes(tmp).map_err(|_layout| Rav1dError::EINVAL)?;
  let width = u64::try_from(active_width).map_err(|_negative| Rav1dError::EINVAL)?;
  let height = u64::try_from(h).map_err(|_negative| Rav1dError::EINVAL)?;
  let active = Size { width, height };
  let source_layout = console_core_pixel_buffers::RowLayout::new(active, 0, i64::from(w))
    .map_err(|_layout| Rav1dError::EINVAL)?;
  let tmp_rows = source_layout
    .rows(tmp_pixels)
    .map_err(|_layout| Rav1dError::EINVAL)?;
  let dst_rows = shape
    .destination_rows::<u16>(dst, dst_stride)
    .map_err(|_layout| Rav1dError::EINVAL)?;

  let rnd = _mm256_set1_epi32(32);
  let sixty_four = _mm256_set1_epi32(64);

  for (dst_row, tmp_row) in dst_rows.zip(tmp_rows) {
    let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

    for column in (0..vector_columns).step_by(8) {
      let Ok(col) = console_core_number_conversion::index(column);
      let checked_value =
        <[_]>::get(&*dst_row, col..(Wrapping(col) + Wrapping(8)).0).ok_or(Rav1dError::EINVAL)?;
      let dst_16 = loadu_128!(checked_value, [u16; 8]);
      let checked_value_2 =
        <[_]>::get(&*tmp_row, col..(Wrapping(col) + Wrapping(8)).0).ok_or(Rav1dError::EINVAL)?;
      let tmp_16 = loadu_128!(checked_value_2, [u16; 8]);
      let dst_32 = _mm256_cvtepu16_epi32(dst_16);
      let tmp_32 = _mm256_cvtepu16_epi32(tmp_16);

      let mut mask_pad = [0u8; 16];

      {
        let checked_copy_destination =
          <[_]>::first_chunk_mut::<8>(&mut mask_pad).ok_or(Rav1dError::EINVAL)?;
        let checked_copy_source = <[_]>::get(&*obmc_mask, col..)
          .and_then(<[_]>::first_chunk::<8>)
          .ok_or(Rav1dError::EINVAL)?;
        *checked_copy_destination = *checked_copy_source;
      };

      let mask_bytes = loadu_128!(&mask_pad);
      let mask_32 = _mm256_cvtepu8_epi32(mask_bytes);
      let inv_mask = _mm256_sub_epi32(sixty_four, mask_32);

      let term1 = _mm256_mullo_epi32(dst_32, inv_mask);
      let term2 = _mm256_mullo_epi32(tmp_32, mask_32);
      let sum = _mm256_add_epi32(_mm256_add_epi32(term1, term2), rnd);

      let result = _mm256_srli_epi32(sum, 6);

      let packed = _mm256_packus_epi32(result, result);
      let lo128 = _mm256_castsi256_si128(packed);
      let hi128 = _mm256_extracti128_si256(packed, 1);
      let result_128 = _mm_unpacklo_epi64(lo128, hi128);

      let checked_value_3 = <[_]>::get_mut(&mut *dst_row, col..(Wrapping(col) + Wrapping(8)).0)
        .ok_or(Rav1dError::EINVAL)?;
      storeu_128!(checked_value_3, [u16; 8], result_128);
    }

    for column in vector_columns..width {
      let Ok(col) = console_core_number_conversion::index(column);
      let checked_value_5 = <[_]>::get(&*dst_row, col).ok_or(Rav1dError::EINVAL)?;
      let a = <u32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_5);
      let checked_value_6 = <[_]>::get(&*tmp_row, col).ok_or(Rav1dError::EINVAL)?;
      let b = <u32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_6);
      let checked_value_7 = <[_]>::get(&*obmc_mask, col).ok_or(Rav1dError::EINVAL)?;
      let m = <u32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_7);
      let output_sample = ::core::primitive::u32::wrapping_shr(
        u32::wrapping_add(
          u32::wrapping_add(
            u32::wrapping_mul(a, u32::wrapping_sub(64, m)),
            ::core::primitive::u32::wrapping_mul(b, m),
          ),
          32,
        ),
        6_u32,
      );
      *{
        let checked_value_11 = <[_]>::get_mut(&mut *dst_row, col).ok_or(Rav1dError::EINVAL)?;
        checked_value_11
      } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
        output_sample,
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn blend_h_16bpc_avx2_safe(
  _token: Desktop64,
  dst: &mut [u8],
  dst_stride: u64,
  tmp: &[u8],
  block_extent: Size<i32>,
) -> Rav1dResult {
  let Size {
    width: w,
    height: h,
  } = block_extent;
  let checked_value_4 = h.checked_mul(3).ok_or(Rav1dError::EINVAL)?;
  let active_height = checked_value_4.wrapping_shr(2);
  let shape = BlendSize::new(Size {
    width: w,
    height: active_height,
  })
  .map_err(|_shape| Rav1dError::EINVAL)?;
  let Ok(h_2) = console_core_number_conversion::wrapped_index(h);
  let Ok(h_eff) = console_core_number_conversion::wrapped_index(active_height);
  let width = w.cast_unsigned();
  let obmc_mask = crate::src::align::aligned_inner(&dav1d_obmc_masks)
    .get(h_2..h_2.wrapping_add(h_eff))
    .ok_or(Rav1dError::EINVAL)?;
  let tmp_pixels: &[u16] =
    zerocopy::FromBytes::ref_from_bytes(tmp).map_err(|_layout| Rav1dError::EINVAL)?;
  let tmp_rows = shape
    .input_rows(tmp_pixels)
    .map_err(|_layout| Rav1dError::EINVAL)?;
  let dst_rows = shape
    .destination_rows::<u16>(dst, dst_stride)
    .map_err(|_layout| Rav1dError::EINVAL)?;

  let rnd = _mm256_set1_epi32(32);
  let sixty_four = _mm256_set1_epi32(64);

  for ((dst_row, tmp_row), &row_weight) in dst_rows.zip(tmp_rows).zip(obmc_mask) {
    let m = u32::from(row_weight);

    let mask_32 = _mm256_set1_epi32(::core::primitive::u32::cast_signed(m));
    let inv_mask = _mm256_sub_epi32(sixty_four, mask_32);

    let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

    for column in (0..vector_columns).step_by(8) {
      let Ok(col) = console_core_number_conversion::index(column);
      let checked_value =
        <[_]>::get(&*dst_row, col..(Wrapping(col) + Wrapping(8)).0).ok_or(Rav1dError::EINVAL)?;
      let dst_16 = loadu_128!(checked_value, [u16; 8]);
      let checked_value_2 =
        <[_]>::get(&*tmp_row, col..(Wrapping(col) + Wrapping(8)).0).ok_or(Rav1dError::EINVAL)?;
      let tmp_16 = loadu_128!(checked_value_2, [u16; 8]);
      let dst_32 = _mm256_cvtepu16_epi32(dst_16);
      let tmp_32 = _mm256_cvtepu16_epi32(tmp_16);

      let term1 = _mm256_mullo_epi32(dst_32, inv_mask);
      let term2 = _mm256_mullo_epi32(tmp_32, mask_32);
      let sum = _mm256_add_epi32(_mm256_add_epi32(term1, term2), rnd);

      let result = _mm256_srli_epi32(sum, 6);

      let packed = _mm256_packus_epi32(result, result);
      let lo128 = _mm256_castsi256_si128(packed);
      let hi128 = _mm256_extracti128_si256(packed, 1);
      let result_128 = _mm_unpacklo_epi64(lo128, hi128);

      let checked_value_3 = <[_]>::get_mut(&mut *dst_row, col..(Wrapping(col) + Wrapping(8)).0)
        .ok_or(Rav1dError::EINVAL)?;
      storeu_128!(checked_value_3, [u16; 8], result_128);
    }

    for column in vector_columns..width {
      let Ok(col) = console_core_number_conversion::index(column);
      let checked_value_5 = <[_]>::get(&*dst_row, col).ok_or(Rav1dError::EINVAL)?;
      let a = <u32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_5);
      let checked_value_6 = <[_]>::get(&*tmp_row, col).ok_or(Rav1dError::EINVAL)?;
      let b = <u32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_6);
      let output_sample = ::core::primitive::u32::wrapping_shr(
        u32::wrapping_add(
          u32::wrapping_add(
            u32::wrapping_mul(a, u32::wrapping_sub(64, m)),
            ::core::primitive::u32::wrapping_mul(b, m),
          ),
          32,
        ),
        6_u32,
      );
      *{
        let checked_value_9 = <[_]>::get_mut(&mut *dst_row, col).ok_or(Rav1dError::EINVAL)?;
        checked_value_9
      } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
        output_sample,
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    }
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the shared interpolation scratch width is used exclusively as a Rust array length"
  )
)]
const MID_STRIDE: usize = 128;

#[cfg(target_arch = "x86_64")]
struct EightTapBlock {
  size: Size<u32>,
  coefficients: Point<Option<&'static [i8; 8]>>,
}

#[cfg(target_arch = "x86_64")]
impl EightTapBlock {
  #[inline]
  fn new(sampling: reference::Sampling, axes: Point<Rav1dFilterMode>) -> Rav1dResult<Self> {
    let dimension = |length| u32::try_from(length).map_err(|_| Rav1dError::EINVAL);
    let width = dimension(sampling.size.width)?;
    let height = dimension(sampling.size.height)?;
    let horizontal_phase = dimension(sampling.phase.x)?;
    let vertical_phase = dimension(sampling.phase.y)?;
    let Ok(x) = fractional_filter(
      FilterExtent {
        samples: width,
        phase: horizontal_phase,
      },
      axes.x,
    );
    let Ok(y) = fractional_filter(
      FilterExtent {
        samples: height,
        phase: vertical_phase,
      },
      axes.y,
    );
    Ok(Self {
      size: Size { width, height },
      coefficients: Point { x, y },
    })
  }
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn fill_horizontal_tap_rows<Source, Intermediate>(
  source: &PixelSource<'_, Source>,
  visible_rows: u32,
  scratch: &mut [[Intermediate; MID_STRIDE]],
  mut filter_row: impl FnMut(&mut [Intermediate], &[Source]) -> Rav1dResult,
) -> Rav1dResult {
  let rows = visible_rows.checked_add(7).ok_or(Rav1dError::EINVAL)?;
  let Ok(end) = console_core_number_conversion::index(rows);
  let target_rows = scratch.get_mut(..end).ok_or(Rav1dError::EINVAL)?;

  for (ordinal, target) in (0..rows).zip(target_rows) {
    let position = Point {
      x: -3,
      y: i64::from(ordinal).wrapping_sub(3),
    };
    let reference_row = source.row_tail(position)?;
    filter_row(target, reference_row)?;
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn pack_byte_tap_pairs(
  [a, b, c, d, e, f, g, h]: [i8; 8],
) -> Result<[i16; 4], console_core_never::Never> {
  Ok([[a, b], [c, d], [e, f], [g, h]].map(|pair| i16::from_le_bytes(pair.map(i8::cast_unsigned))))
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn horizontal_windows<Sample>(input: &[Sample; 23]) -> Rav1dResult<[&[Sample; 16]; 8]> {
  let mut windows = input.array_windows::<16>();
  let first = windows.next().ok_or(Rav1dError::EINVAL)?;
  let second = windows.next().ok_or(Rav1dError::EINVAL)?;
  let third = windows.next().ok_or(Rav1dError::EINVAL)?;
  let fourth = windows.next().ok_or(Rav1dError::EINVAL)?;
  let fifth = windows.next().ok_or(Rav1dError::EINVAL)?;
  let sixth = windows.next().ok_or(Rav1dError::EINVAL)?;
  let seventh = windows.next().ok_or(Rav1dError::EINVAL)?;
  let eighth = windows.next().ok_or(Rav1dError::EINVAL)?;
  Ok([first, second, third, fourth, fifth, sixth, seventh, eighth])
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn horizontal_byte_sums(
  _token: Desktop64,
  input: &[u8; 23],
  [
    coefficient_01,
    coefficient_23,
    coefficient_45,
    coefficient_67,
  ]: [__m256i; 4],
) -> Rav1dResult<__m256i> {
  let [first, second, third, fourth, fifth, sixth, seventh, eighth] = horizontal_windows(input)?;
  let a = loadu_128!(first);
  let b = loadu_128!(second);
  let c = loadu_128!(third);
  let d = loadu_128!(fourth);
  let e = loadu_128!(fifth);
  let f = loadu_128!(sixth);
  let g = loadu_128!(seventh);
  let h = loadu_128!(eighth);
  let pair_01 = _mm256_set_m128i(_mm_unpackhi_epi8(a, b), _mm_unpacklo_epi8(a, b));
  let pair_23 = _mm256_set_m128i(_mm_unpackhi_epi8(c, d), _mm_unpacklo_epi8(c, d));
  let pair_45 = _mm256_set_m128i(_mm_unpackhi_epi8(e, f), _mm_unpacklo_epi8(e, f));
  let pair_67 = _mm256_set_m128i(_mm_unpackhi_epi8(g, h), _mm_unpacklo_epi8(g, h));
  let products_01 = _mm256_maddubs_epi16(pair_01, coefficient_01);
  let products_23 = _mm256_maddubs_epi16(pair_23, coefficient_23);
  let products_45 = _mm256_maddubs_epi16(pair_45, coefficient_45);
  let products_67 = _mm256_maddubs_epi16(pair_67, coefficient_67);
  let first_half = _mm256_add_epi16(products_01, products_23);
  let third_pair = _mm256_add_epi16(first_half, products_45);
  Ok(_mm256_add_epi16(third_pair, products_67))
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn word_tap_coefficients(
  _token: Server64,
  filter: [i8; 8],
) -> Result<[__m512i; 8], console_core_never::Never> {
  let [a, b, c, d, e, f, g, h] = filter.map(i32::from);
  Ok([
    _mm512_set1_epi32(a),
    _mm512_set1_epi32(b),
    _mm512_set1_epi32(c),
    _mm512_set1_epi32(d),
    _mm512_set1_epi32(e),
    _mm512_set1_epi32(f),
    _mm512_set1_epi32(g),
    _mm512_set1_epi32(h),
  ])
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn horizontal_word_sums(
  _token: Server64,
  input: &[u16; 23],
  [a, b, c, d, e, f, g, h]: [__m512i; 8],
) -> Rav1dResult<__m512i> {
  let [first, second, third, fourth, fifth, sixth, seventh, eighth] = horizontal_windows(input)?;
  let term_0 = _mm512_mullo_epi32(_mm512_cvtepu16_epi32(loadu_256!(first)), a);
  let term_1 = _mm512_mullo_epi32(_mm512_cvtepu16_epi32(loadu_256!(second)), b);
  let term_2 = _mm512_mullo_epi32(_mm512_cvtepu16_epi32(loadu_256!(third)), c);
  let term_3 = _mm512_mullo_epi32(_mm512_cvtepu16_epi32(loadu_256!(fourth)), d);
  let term_4 = _mm512_mullo_epi32(_mm512_cvtepu16_epi32(loadu_256!(fifth)), e);
  let term_5 = _mm512_mullo_epi32(_mm512_cvtepu16_epi32(loadu_256!(sixth)), f);
  let term_6 = _mm512_mullo_epi32(_mm512_cvtepu16_epi32(loadu_256!(seventh)), g);
  let term_7 = _mm512_mullo_epi32(_mm512_cvtepu16_epi32(loadu_256!(eighth)), h);
  let first_two = _mm512_add_epi32(term_0, term_1);
  let first_three = _mm512_add_epi32(first_two, term_2);
  let first_four = _mm512_add_epi32(first_three, term_3);
  let first_five = _mm512_add_epi32(first_four, term_4);
  let first_six = _mm512_add_epi32(first_five, term_5);
  let first_seven = _mm512_add_epi32(first_six, term_6);
  Ok(_mm512_add_epi32(first_seven, term_7))
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn h_filter_8tap_8bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [i16],
  src: &[u8],
  width: u32,
  filter: &[i8; 8],
  sh: u8,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let src_2 = &*src;

  let Ok([pair_01, pair_23, pair_45, pair_67]) = pack_byte_tap_pairs(*filter);
  let coeff_01 = _mm256_set1_epi16(pair_01);
  let coeff_23 = _mm256_set1_epi16(pair_23);
  let coeff_45 = _mm256_set1_epi16(pair_45);
  let coeff_67 = _mm256_set1_epi16(pair_67);

  let rnd = _mm256_set1_epi16(::core::primitive::i16::wrapping_shr(
    i16::wrapping_shl(1i16, u32::from(sh)),
    1_u32,
  ));

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let window = src_2
      .get(column_index..)
      .and_then(<[u8]>::first_chunk::<23>)
      .ok_or(Rav1dError::EINVAL)?;
    let sum = horizontal_byte_sums(token, window, [coeff_01, coeff_23, coeff_45, coeff_67])?;

    let shift_count = _mm_cvtsi32_si128(
      <i32 as ::core::convert::From<::core::primitive::u8>>::from(sh),
    );
    let result = _mm256_sra_epi16(_mm256_add_epi16(sum, rnd), shift_count);

    let target = dst_2
      .get_mut(column_index..)
      .and_then(<[i16]>::first_chunk_mut::<16>)
      .ok_or(Rav1dError::EINVAL)?;
    storeu_256!(target, result);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value =
        <[_]>::get(&*src_2, column_index.wrapping_add(tap)).ok_or(Rav1dError::EINVAL)?;
      Ok(i32::from(*checked_value))
    })?;

    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(round_shift(
      sum,
      u32::from(sh),
    )) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn h_filter_8tap_8bpc_avx512_inner(
  _token: Server64,
  dst: &mut [i16],
  src: &[u8],
  width: u32,
  filter: &[i8; 8],
  sh: u8,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let src_2 = &*src;

  let Ok([pair_01, pair_23, pair_45, pair_67]) = pack_byte_tap_pairs(*filter);
  let coeff_01 = _mm512_set1_epi16(pair_01);
  let coeff_23 = _mm512_set1_epi16(pair_23);
  let coeff_45 = _mm512_set1_epi16(pair_45);
  let coeff_67 = _mm512_set1_epi16(pair_67);

  let rnd = _mm512_set1_epi16(::core::primitive::i16::wrapping_shr(
    i16::wrapping_shl(1i16, u32::from(sh)),
    1_u32,
  ));

  let vector_columns = width.wrapping_sub(width.wrapping_rem(32));

  for col in (0..vector_columns).step_by(32) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_2 = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(32)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let s0 = loadu_256!(checked_value_2);
    let checked_value_3 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(1)).0..(Wrapping(column_index) + Wrapping(33)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let s1 = loadu_256!(checked_value_3);
    let checked_value_4 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(2)).0..(Wrapping(column_index) + Wrapping(34)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let s2 = loadu_256!(checked_value_4);
    let checked_value_5 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(3)).0..(Wrapping(column_index) + Wrapping(35)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let s3 = loadu_256!(checked_value_5);
    let checked_value_6 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(4)).0..(Wrapping(column_index) + Wrapping(36)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let s4 = loadu_256!(checked_value_6);
    let checked_value_7 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(5)).0..(Wrapping(column_index) + Wrapping(37)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let s5 = loadu_256!(checked_value_7);
    let checked_value_8 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(6)).0..(Wrapping(column_index) + Wrapping(38)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let s6 = loadu_256!(checked_value_8);
    let checked_value_9 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(7)).0..(Wrapping(column_index) + Wrapping(39)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let s7 = loadu_256!(checked_value_9);

    let lo01 = _mm256_unpacklo_epi8(s0, s1);
    let hi01 = _mm256_unpackhi_epi8(s0, s1);
    let p01 = _mm512_inserti64x4::<1>(_mm512_castsi256_si512(lo01), hi01);

    let lo23 = _mm256_unpacklo_epi8(s2, s3);
    let hi23 = _mm256_unpackhi_epi8(s2, s3);
    let p23 = _mm512_inserti64x4::<1>(_mm512_castsi256_si512(lo23), hi23);

    let lo45 = _mm256_unpacklo_epi8(s4, s5);
    let hi45 = _mm256_unpackhi_epi8(s4, s5);
    let p45 = _mm512_inserti64x4::<1>(_mm512_castsi256_si512(lo45), hi45);

    let lo67 = _mm256_unpacklo_epi8(s6, s7);
    let hi67 = _mm256_unpackhi_epi8(s6, s7);
    let p67 = _mm512_inserti64x4::<1>(_mm512_castsi256_si512(lo67), hi67);

    let ma01 = _mm512_maddubs_epi16(p01, coeff_01);
    let ma23 = _mm512_maddubs_epi16(p23, coeff_23);
    let ma45 = _mm512_maddubs_epi16(p45, coeff_45);
    let ma67 = _mm512_maddubs_epi16(p67, coeff_67);

    let sum = _mm512_add_epi16(_mm512_add_epi16(ma01, ma23), _mm512_add_epi16(ma45, ma67));

    let shift_count = _mm_cvtsi32_si128(
      <i32 as ::core::convert::From<::core::primitive::u8>>::from(sh),
    );
    let result = _mm512_sra_epi16(_mm512_add_epi16(sum, rnd), shift_count);

    let result_2 = _mm512_shuffle_i64x2::<0b11_01_10_00>(result, result);

    let checked_value_10 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(32)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_512!(checked_value_10, result_2);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value =
        <[_]>::get(&*src_2, column_index.wrapping_add(tap)).ok_or(Rav1dError::EINVAL)?;
      Ok(i32::from(*checked_value))
    })?;

    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(round_shift(
      sum,
      u32::from(sh),
    )) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_filter_8tap_8bpc_avx2_inner(
  _token: Desktop64,
  dst: &mut [u8],
  mid: &[[i16; MID_STRIDE]],
  width: u32,
  filter: &[i8; 8],
  sh: u8,
  max: i32,
) -> Rav1dResult {
  let dst_2 = &mut *dst;

  let rnd = _mm256_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1i32, u32::from(sh)),
    1_u32,
  ));
  let zero = _mm256_setzero_si256();
  let _max_vec = _mm256_set1_epi16(
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(max) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    },
  );

  let c0 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[element, ..] => element,
    },
  ));
  let c1 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, element, ..] => element,
    },
  ));
  let c2 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, element, ..] => element,
    },
  ));
  let c3 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, element, ..] => element,
    },
  ));
  let c4 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c5 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c6 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c7 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));

  let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

  for col in (0..vector_columns).step_by(8) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_2 = <[_]>::get(
      &(*{
        let checked_value_36 = <[_]>::get(&*mid, 0).ok_or(Rav1dError::EINVAL)?;
        checked_value_36
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m0 = _mm256_cvtepi16_epi32(loadu_128!(checked_value_2));
    let checked_value_3 = <[_]>::get(
      &(*{
        let checked_value_37 = <[_]>::get(&*mid, 1).ok_or(Rav1dError::EINVAL)?;
        checked_value_37
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m1 = _mm256_cvtepi16_epi32(loadu_128!(checked_value_3));
    let checked_value_4 = <[_]>::get(
      &(*{
        let checked_value_38 = <[_]>::get(&*mid, 2).ok_or(Rav1dError::EINVAL)?;
        checked_value_38
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m2 = _mm256_cvtepi16_epi32(loadu_128!(checked_value_4));
    let checked_value_5 = <[_]>::get(
      &(*{
        let checked_value_39 = <[_]>::get(&*mid, 3).ok_or(Rav1dError::EINVAL)?;
        checked_value_39
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m3 = _mm256_cvtepi16_epi32(loadu_128!(checked_value_5));
    let checked_value_6 = <[_]>::get(
      &(*{
        let checked_value_40 = <[_]>::get(&*mid, 4).ok_or(Rav1dError::EINVAL)?;
        checked_value_40
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m4 = _mm256_cvtepi16_epi32(loadu_128!(checked_value_6));
    let checked_value_7 = <[_]>::get(
      &(*{
        let checked_value_41 = <[_]>::get(&*mid, 5).ok_or(Rav1dError::EINVAL)?;
        checked_value_41
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m5 = _mm256_cvtepi16_epi32(loadu_128!(checked_value_7));
    let checked_value_8 = <[_]>::get(
      &(*{
        let checked_value_42 = <[_]>::get(&*mid, 6).ok_or(Rav1dError::EINVAL)?;
        checked_value_42
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m6 = _mm256_cvtepi16_epi32(loadu_128!(checked_value_8));
    let checked_value_10 = <[_]>::get(
      &(*{
        let checked_value_43 = <[_]>::get(&*mid, 7).ok_or(Rav1dError::EINVAL)?;
        checked_value_43
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m7 = _mm256_cvtepi16_epi32(loadu_128!(checked_value_10));

    let mut sum = _mm256_mullo_epi32(m0, c0);
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m1, c1));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m2, c2));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m3, c3));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m4, c4));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m5, c5));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m6, c6));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m7, c7));

    let shift_count = _mm_cvtsi32_si128(
      <i32 as ::core::convert::From<::core::primitive::u8>>::from(sh),
    );
    let shifted = _mm256_sra_epi32(_mm256_add_epi32(sum, rnd), shift_count);

    let clamped = _mm256_min_epi32(_mm256_max_epi32(shifted, zero), _mm256_set1_epi32(max));

    let packed16 = _mm256_packs_epi32(clamped, clamped);
    let packed16_2 = _mm256_permute4x64_epi64(packed16, 0b11011000);
    let packed8 = _mm256_packus_epi16(packed16_2, packed16_2);

    let result_64 = _mm256_extract_epi64(packed8, 0);

    {
      let explicit_byte_copy_destination = <[_]>::get_mut(&mut *dst_2, column_index..)
        .and_then(<[_]>::first_chunk_mut::<8>)
        .ok_or(Rav1dError::EINVAL)?;
      *explicit_byte_copy_destination = result_64.to_le_bytes();
    };
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value_2 = <[_]>::get(
        &(*{
          let checked_value_9 = <[_]>::get(&*mid, tap).ok_or(Rav1dError::EINVAL)?;
          checked_value_9
        }),
        column_index,
      )
      .ok_or(Rav1dError::EINVAL)?;
      Ok(i32::from(*checked_value_2))
    })?;

    let output_sample = round_shift(sum, u32::from(sh)).clamp(0, max);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(output_sample)
    {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_filter_8tap_8bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  mid: &[[i16; MID_STRIDE]],
  width: u32,
  filter: &[i8; 8],
  sh: u8,
  max: i32,
) -> Rav1dResult {
  let dst_2 = &mut *dst;

  let rnd = _mm512_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1i32, u32::from(sh)),
    1_u32,
  ));
  let zero = _mm512_setzero_si512();
  let max_v = _mm512_set1_epi32(max);

  let c0 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[element, ..] => element,
    },
  ));
  let c1 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, element, ..] => element,
    },
  ));
  let c2 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, element, ..] => element,
    },
  ));
  let c3 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, element, ..] => element,
    },
  ));
  let c4 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c5 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c6 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c7 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_2 = <[_]>::get(
      &(*{
        let checked_value_36 = <[_]>::get(&*mid, 0).ok_or(Rav1dError::EINVAL)?;
        checked_value_36
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m0 = _mm512_cvtepi16_epi32(loadu_256!(checked_value_2));
    let checked_value_3 = <[_]>::get(
      &(*{
        let checked_value_37 = <[_]>::get(&*mid, 1).ok_or(Rav1dError::EINVAL)?;
        checked_value_37
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m1 = _mm512_cvtepi16_epi32(loadu_256!(checked_value_3));
    let checked_value_4 = <[_]>::get(
      &(*{
        let checked_value_38 = <[_]>::get(&*mid, 2).ok_or(Rav1dError::EINVAL)?;
        checked_value_38
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m2 = _mm512_cvtepi16_epi32(loadu_256!(checked_value_4));
    let checked_value_5 = <[_]>::get(
      &(*{
        let checked_value_39 = <[_]>::get(&*mid, 3).ok_or(Rav1dError::EINVAL)?;
        checked_value_39
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m3 = _mm512_cvtepi16_epi32(loadu_256!(checked_value_5));
    let checked_value_6 = <[_]>::get(
      &(*{
        let checked_value_40 = <[_]>::get(&*mid, 4).ok_or(Rav1dError::EINVAL)?;
        checked_value_40
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m4 = _mm512_cvtepi16_epi32(loadu_256!(checked_value_6));
    let checked_value_7 = <[_]>::get(
      &(*{
        let checked_value_41 = <[_]>::get(&*mid, 5).ok_or(Rav1dError::EINVAL)?;
        checked_value_41
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m5 = _mm512_cvtepi16_epi32(loadu_256!(checked_value_7));
    let checked_value_8 = <[_]>::get(
      &(*{
        let checked_value_42 = <[_]>::get(&*mid, 6).ok_or(Rav1dError::EINVAL)?;
        checked_value_42
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m6 = _mm512_cvtepi16_epi32(loadu_256!(checked_value_8));
    let checked_value_10 = <[_]>::get(
      &(*{
        let checked_value_43 = <[_]>::get(&*mid, 7).ok_or(Rav1dError::EINVAL)?;
        checked_value_43
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m7 = _mm512_cvtepi16_epi32(loadu_256!(checked_value_10));

    let mut sum = _mm512_mullo_epi32(m0, c0);
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m1, c1));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m2, c2));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m3, c3));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m4, c4));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m5, c5));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m6, c6));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m7, c7));

    let shift_count = _mm_cvtsi32_si128(
      <i32 as ::core::convert::From<::core::primitive::u8>>::from(sh),
    );
    let shifted = _mm512_sra_epi32(_mm512_add_epi32(sum, rnd), shift_count);
    let clamped = _mm512_min_epi32(_mm512_max_epi32(shifted, zero), max_v);

    let packed = _mm512_cvtusepi32_epi8(clamped);

    let checked_value_11 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [u8; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_128!(checked_value_11, packed);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value_2 = <[_]>::get(
        &(*{
          let checked_value_9 = <[_]>::get(&*mid, tap).ok_or(Rav1dError::EINVAL)?;
          checked_value_9
        }),
        column_index,
      )
      .ok_or(Rav1dError::EINVAL)?;
      Ok(i32::from(*checked_value_2))
    })?;

    let output_sample = round_shift(sum, u32::from(sh)).clamp(0, max);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(output_sample)
    {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn h_filter_8tap_8bpc_put_avx2_inner(
  token: Desktop64,
  dst: &mut [u8],
  src: &[u8],
  width: u32,
  filter: &[i8; 8],
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let src_2 = &*src;

  let Ok([pair_01, pair_23, pair_45, pair_67]) = pack_byte_tap_pairs(*filter);
  let coeff_01 = _mm256_set1_epi16(pair_01);
  let coeff_23 = _mm256_set1_epi16(pair_23);
  let coeff_45 = _mm256_set1_epi16(pair_45);
  let coeff_67 = _mm256_set1_epi16(pair_67);

  let rnd = _mm256_set1_epi16(34);
  let zero = _mm256_setzero_si256();

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let window = src_2
      .get(column_index..)
      .and_then(<[u8]>::first_chunk::<23>)
      .ok_or(Rav1dError::EINVAL)?;
    let sum = horizontal_byte_sums(token, window, [coeff_01, coeff_23, coeff_45, coeff_67])?;

    let shift_count = _mm_cvtsi32_si128(6);
    let shifted = _mm256_sra_epi16(_mm256_add_epi16(sum, rnd), shift_count);
    let clamped = _mm256_max_epi16(_mm256_min_epi16(shifted, _mm256_set1_epi16(255)), zero);

    let packed = _mm256_packus_epi16(clamped, clamped);
    let packed_2 = _mm256_permute4x64_epi64(packed, 0b11011000);

    let target = dst_2
      .get_mut(column_index..)
      .and_then(<[u8]>::first_chunk_mut::<16>)
      .ok_or(Rav1dError::EINVAL)?;
    storeu_128!(target, _mm256_castsi256_si128(packed_2));
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value =
        <[_]>::get(&*src_2, column_index.wrapping_add(tap)).ok_or(Rav1dError::EINVAL)?;
      Ok(i32::from(*checked_value))
    })?;

    let output_sample =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(sum, 34), 6_u32).clamp(0, 255);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(output_sample)
    {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn h_filter_8tap_8bpc_put_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  src: &[u8],
  width: u32,
  filter: &[i8; 8],
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let src_2 = &*src;

  let Ok([pair_01, pair_23, pair_45, pair_67]) = pack_byte_tap_pairs(*filter);
  let coeff_01 = _mm512_set1_epi16(pair_01);
  let coeff_23 = _mm512_set1_epi16(pair_23);
  let coeff_45 = _mm512_set1_epi16(pair_45);
  let coeff_67 = _mm512_set1_epi16(pair_67);

  let rnd = _mm512_set1_epi16(34);
  let zero = _mm512_setzero_si512();
  let max_v = _mm512_set1_epi16(255);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(32));

  for col in (0..vector_columns).step_by(32) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_2 = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(32)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let s0 = loadu_256!(checked_value_2);
    let checked_value_3 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(1)).0..(Wrapping(column_index) + Wrapping(33)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let s1 = loadu_256!(checked_value_3);
    let checked_value_4 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(2)).0..(Wrapping(column_index) + Wrapping(34)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let s2 = loadu_256!(checked_value_4);
    let checked_value_5 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(3)).0..(Wrapping(column_index) + Wrapping(35)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let s3 = loadu_256!(checked_value_5);
    let checked_value_6 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(4)).0..(Wrapping(column_index) + Wrapping(36)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let s4 = loadu_256!(checked_value_6);
    let checked_value_7 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(5)).0..(Wrapping(column_index) + Wrapping(37)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let s5 = loadu_256!(checked_value_7);
    let checked_value_8 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(6)).0..(Wrapping(column_index) + Wrapping(38)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let s6 = loadu_256!(checked_value_8);
    let checked_value_9 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(7)).0..(Wrapping(column_index) + Wrapping(39)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let s7 = loadu_256!(checked_value_9);

    let lo01 = _mm256_unpacklo_epi8(s0, s1);
    let hi01 = _mm256_unpackhi_epi8(s0, s1);
    let p01 = _mm512_inserti64x4::<1>(_mm512_castsi256_si512(lo01), hi01);

    let lo23 = _mm256_unpacklo_epi8(s2, s3);
    let hi23 = _mm256_unpackhi_epi8(s2, s3);
    let p23 = _mm512_inserti64x4::<1>(_mm512_castsi256_si512(lo23), hi23);

    let lo45 = _mm256_unpacklo_epi8(s4, s5);
    let hi45 = _mm256_unpackhi_epi8(s4, s5);
    let p45 = _mm512_inserti64x4::<1>(_mm512_castsi256_si512(lo45), hi45);

    let lo67 = _mm256_unpacklo_epi8(s6, s7);
    let hi67 = _mm256_unpackhi_epi8(s6, s7);
    let p67 = _mm512_inserti64x4::<1>(_mm512_castsi256_si512(lo67), hi67);

    let ma01 = _mm512_maddubs_epi16(p01, coeff_01);
    let ma23 = _mm512_maddubs_epi16(p23, coeff_23);
    let ma45 = _mm512_maddubs_epi16(p45, coeff_45);
    let ma67 = _mm512_maddubs_epi16(p67, coeff_67);

    let sum = _mm512_add_epi16(_mm512_add_epi16(ma01, ma23), _mm512_add_epi16(ma45, ma67));

    let shift_count = _mm_cvtsi32_si128(6);
    let shifted = _mm512_sra_epi16(_mm512_add_epi16(sum, rnd), shift_count);
    let clamped = _mm512_max_epi16(_mm512_min_epi16(shifted, max_v), zero);

    let clamped_2 = _mm512_shuffle_i64x2::<0b11_01_10_00>(clamped, clamped);
    let packed = _mm512_cvtusepi16_epi8(clamped_2);

    let checked_value_10 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(32)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_256!(checked_value_10, packed);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value =
        <[_]>::get(&*src_2, column_index.wrapping_add(tap)).ok_or(Rav1dError::EINVAL)?;
      Ok(i32::from(*checked_value))
    })?;

    let output_sample =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(sum, 34), 6_u32).clamp(0, 255);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(output_sample)
    {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_filter_8tap_8bpc_direct_avx2_inner(
  _token: Desktop64,
  dst: &mut [u8],
  src: &[u8],
  src_stride: i64,
  width: u32,
  filter: &[i8; 8],
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let src_2 = &*src;
  let pitch = u64::try_from(src_stride).map_err(|_| Rav1dError::EINVAL)?;
  let Ok(stride) = console_core_number_conversion::index(pitch);
  let c0 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[element, ..] => element,
    },
  ));
  let c1 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, element, ..] => element,
    },
  ));
  let c2 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, element, ..] => element,
    },
  ));
  let c3 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, element, ..] => element,
    },
  ));
  let c4 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c5 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c6 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c7 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));

  let rnd = _mm256_set1_epi32(32);
  let zero = _mm256_setzero_si256();
  let max = _mm256_set1_epi32(255);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

  for col in (0..vector_columns).step_by(8) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_2 = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let p0 = _mm256_cvtepu8_epi32(loadi64!(checked_value_2));
    let checked_value_3 = <[_]>::get(
      &*src_2,
      (Wrapping(stride) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping(stride) + Wrapping(column_index)).0) + Wrapping(8)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let p1 = _mm256_cvtepu8_epi32(loadi64!(checked_value_3));
    let checked_value_4 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(2) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(2) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let p2 = _mm256_cvtepu8_epi32(loadi64!(checked_value_4));
    let checked_value_5 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(3) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(3) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let p3 = _mm256_cvtepu8_epi32(loadi64!(checked_value_5));
    let checked_value_6 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(4) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(4) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let p4 = _mm256_cvtepu8_epi32(loadi64!(checked_value_6));
    let checked_value_7 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(5) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(5) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let p5 = _mm256_cvtepu8_epi32(loadi64!(checked_value_7));
    let checked_value_8 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(6) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(6) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let p6 = _mm256_cvtepu8_epi32(loadi64!(checked_value_8));
    let checked_value_9 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(7) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(7) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let p7 = _mm256_cvtepu8_epi32(loadi64!(checked_value_9));

    let mut sum = _mm256_mullo_epi32(p0, c0);
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p1, c1));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p2, c2));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p3, c3));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p4, c4));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p5, c5));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p6, c6));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p7, c7));

    let shift_count = _mm_cvtsi32_si128(6);
    let shifted = _mm256_sra_epi32(_mm256_add_epi32(sum, rnd), shift_count);
    let clamped = _mm256_min_epi32(_mm256_max_epi32(shifted, zero), max);

    let packed16 = _mm256_packs_epi32(clamped, clamped);
    let packed16_2 = _mm256_permute4x64_epi64(packed16, 0b11011000);
    let packed8 = _mm256_packus_epi16(packed16_2, packed16_2);

    let result_64 = _mm256_extract_epi64(packed8, 0);

    {
      let explicit_byte_copy_destination = <[_]>::get_mut(&mut *dst_2, column_index..)
        .and_then(<[_]>::first_chunk_mut::<8>)
        .ok_or(Rav1dError::EINVAL)?;
      *explicit_byte_copy_destination = result_64.to_le_bytes();
    };
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let ::core::result::Result::Ok(offset) =
        ::console_core_number_conversion::fixed_index(tap.wrapping_mul(stride));
      let checked_value = <[_]>::get(
        &*src_2,
        column_index.wrapping_add(match ::console_core_number_conversion::index(offset) {
          ::core::result::Result::Ok(native_position) => native_position,
        }),
      )
      .ok_or(Rav1dError::EINVAL)?;
      Ok(i32::from(*checked_value))
    })?;

    let output_sample =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(sum, 32), 6_u32).clamp(0, 255);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(output_sample)
    {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_filter_8tap_8bpc_direct_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  src: &[u8],
  src_stride: i64,
  width: u32,
  filter: &[i8; 8],
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let src_2 = &*src;

  let c0 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[element, ..] => element,
    },
  ));
  let c1 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, element, ..] => element,
    },
  ));
  let c2 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, element, ..] => element,
    },
  ));
  let c3 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, element, ..] => element,
    },
  ));
  let c4 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c5 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c6 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c7 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));

  let rnd = _mm512_set1_epi32(32);
  let zero = _mm512_setzero_si512();
  let max = _mm512_set1_epi32(255);

  let pitch = u64::try_from(src_stride).map_err(|_| Rav1dError::EINVAL)?;
  let Ok(stride) = console_core_number_conversion::index(pitch);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_2 = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p0 = _mm512_cvtepu8_epi32(loadu_128!(checked_value_2));
    let checked_value_3 = <[_]>::get(
      &*src_2,
      (Wrapping(stride) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping(stride) + Wrapping(column_index)).0) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p1 = _mm512_cvtepu8_epi32(loadu_128!(checked_value_3));
    let checked_value_4 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(2) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(2) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p2 = _mm512_cvtepu8_epi32(loadu_128!(checked_value_4));
    let checked_value_5 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(3) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(3) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p3 = _mm512_cvtepu8_epi32(loadu_128!(checked_value_5));
    let checked_value_6 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(4) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(4) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p4 = _mm512_cvtepu8_epi32(loadu_128!(checked_value_6));
    let checked_value_7 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(5) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(5) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p5 = _mm512_cvtepu8_epi32(loadu_128!(checked_value_7));
    let checked_value_8 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(6) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(6) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p6 = _mm512_cvtepu8_epi32(loadu_128!(checked_value_8));
    let checked_value_9 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(7) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(7) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p7 = _mm512_cvtepu8_epi32(loadu_128!(checked_value_9));

    let mut sum = _mm512_mullo_epi32(p0, c0);
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p1, c1));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p2, c2));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p3, c3));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p4, c4));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p5, c5));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p6, c6));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p7, c7));

    let shift_count = _mm_cvtsi32_si128(6);
    let shifted = _mm512_sra_epi32(_mm512_add_epi32(sum, rnd), shift_count);
    let clamped = _mm512_min_epi32(_mm512_max_epi32(shifted, zero), max);

    let packed = _mm512_cvtusepi32_epi8(clamped);

    let checked_value_10 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [u8; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_128!(checked_value_10, packed);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let ::core::result::Result::Ok(offset) =
        ::console_core_number_conversion::fixed_index(tap.wrapping_mul(stride));
      let checked_value = <[_]>::get(
        &*src_2,
        column_index.wrapping_add(match ::console_core_number_conversion::index(offset) {
          ::core::result::Result::Ok(native_position) => native_position,
        }),
      )
      .ok_or(Rav1dError::EINVAL)?;
      Ok(i32::from(*checked_value))
    })?;

    let output_sample =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(sum, 32), 6_u32).clamp(0, 255);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(output_sample)
    {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn put_8tap_8bpc_avx2_impl_inner(
  token: Desktop64,
  request: EightTapOutput<'_, u8>,
) -> Rav1dResult {
  let writer = MotionRowWriters {
    horizontal_buffered: |target: &mut [i16], input: &[u8], width, taps: &[i8; 8], shift| {
      let Ok(precision) = console_core_number_conversion::wrapped(shift);
      h_filter_8tap_8bpc_avx2_inner(token, target, input, width, taps, precision)
    },
    vertical_buffered: |target: &mut [u8],
                        scratch: &[[i16; MID_STRIDE]],
                        row: MotionRow,
                        taps: &[i8; 8],
                        scale: &WordVerticalScale| {
      let Ok(start) = console_core_number_conversion::index(row.ordinal);
      let filtered = scratch.get(start..).ok_or(Rav1dError::EINVAL)?;
      let Ok(precision) = console_core_number_conversion::wrapped(scale.shift);
      v_filter_8tap_8bpc_avx2_inner(
        token,
        target,
        filtered,
        row.width,
        taps,
        precision,
        scale.maximum,
      )
    },
    horizontal_direct: |target: &mut [u8], input: &[u8], width, taps: &[i8; 8], _maximum| {
      h_filter_8tap_8bpc_put_avx2_inner(token, target, input, width, taps)
    },
    vertical_direct: |target: &mut [u8], input: &[u8], pitch, width, taps: &[i8; 8], _maximum| {
      v_filter_8tap_8bpc_direct_avx2_inner(token, target, input, pitch, width, taps)
    },
  };
  put_eight_tap(writer, request, &MID_I16_135)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn put_8tap_8bpc_avx512_impl_inner(
  token: Server64,
  request: EightTapOutput<'_, u8>,
) -> Rav1dResult {
  let writer = MotionRowWriters {
    horizontal_buffered: |target: &mut [i16], input: &[u8], width, taps: &[i8; 8], shift| {
      let Ok(precision) = console_core_number_conversion::wrapped(shift);
      h_filter_8tap_8bpc_avx512_inner(token, target, input, width, taps, precision)
    },
    vertical_buffered: |target: &mut [u8],
                        scratch: &[[i16; MID_STRIDE]],
                        row: MotionRow,
                        taps: &[i8; 8],
                        scale: &WordVerticalScale| {
      let Ok(start) = console_core_number_conversion::index(row.ordinal);
      let filtered = scratch.get(start..).ok_or(Rav1dError::EINVAL)?;
      let Ok(precision) = console_core_number_conversion::wrapped(scale.shift);
      v_filter_8tap_8bpc_avx512_inner(
        token,
        target,
        filtered,
        row.width,
        taps,
        precision,
        scale.maximum,
      )
    },
    horizontal_direct: |target: &mut [u8], input: &[u8], width, taps: &[i8; 8], _maximum| {
      h_filter_8tap_8bpc_put_avx512_inner(token, target, input, width, taps)
    },
    vertical_direct: |target: &mut [u8], input: &[u8], pitch, width, taps: &[i8; 8], _maximum| {
      v_filter_8tap_8bpc_direct_avx512_inner(token, target, input, pitch, width, taps)
    },
  };
  put_eight_tap(writer, request, &MID_I16_135)
}

#[cfg(target_arch = "x86_64")]
struct EightTapPrediction<'a, Pixel> {
  output: &'a mut [i16],
  source: PixelSource<'a, Pixel>,
  sampling: reference::Sampling,
  axes: Point<Rav1dFilterMode>,
}

#[cfg(target_arch = "x86_64")]
fn prepare_eight_tap<Pixel, Intermediate: Copy + Default + 'static>(
  request: EightTapPrediction<'_, Pixel>,
  writer: MotionRowWriters<
    impl Fn(&mut [Intermediate], &[Pixel], u32, &[i8; 8]) -> Rav1dResult,
    impl Fn(&mut [i16], &[[Intermediate; MID_STRIDE]], MotionRow, &[i8; 8]) -> Rav1dResult,
    impl Fn(&mut [i16], &[Pixel], u32, &[i8; 8]) -> Rav1dResult,
    impl Fn(&mut [i16], &PixelSource<'_, Pixel>, MotionRow, &[i8; 8]) -> Rav1dResult,
  >,
  pool: &'static MidPool<Intermediate, 135>,
  copy: impl Fn(&mut [i16], &[Pixel]) -> Result<(), Never>,
) -> Rav1dResult {
  use console_core_pixel_buffers::RowLayout;
  let EightTapPrediction {
    output,
    source,
    sampling,
    axes,
  } = request;
  let extent = sampling.checked_size()?;
  let EightTapBlock {
    size: Size { width, height },
    coefficients,
  } = EightTapBlock::new(sampling, axes)?;
  let Point {
    x: horizontal_taps,
    y: vertical_taps,
  } = coefficients;
  let layout = RowLayout::new(extent, 0, i64::from(width)).map_err(|_| Rav1dError::EINVAL)?;
  let output_rows = layout.rows_mut(output).map_err(|_| Rav1dError::EINVAL)?;
  let reference_x = i64::from(horizontal_taps.is_some()).wrapping_mul(-3);
  let reference_y = i64::from(vertical_taps.is_some()).wrapping_mul(-3);
  let reference_extent = Size {
    width: extent
      .width
      .wrapping_add(u64::from(horizontal_taps.is_some()).wrapping_mul(7)),
    height: extent
      .height
      .wrapping_add(u64::from(vertical_taps.is_some()).wrapping_mul(7)),
  };
  let Ok(pixel_bytes) =
    console_core_number_conversion::fixed_index(core::mem::size_of::<Pixel>());
  let pitch = source
    .stride_bytes
    .checked_div(pixel_bytes.cast_signed())
    .ok_or(Rav1dError::EINVAL)?;

  match pitch {
    1.. => {}
    _ => return Err(Rav1dError::EINVAL),
  }

  let origin = reference_y
    .checked_mul(pitch)
    .and_then(|value| value.checked_add(reference_x))
    .ok_or(Rav1dError::EINVAL)?;
  let start = source
    .origin
    .checked_add_signed(origin)
    .ok_or(Rav1dError::EINVAL)?;
  let input = RowLayout::new(reference_extent, start, pitch).map_err(|_| Rav1dError::EINVAL)?;
  input
    .validate_len(source.samples.len())
    .map_err(|_| Rav1dError::EINVAL)?;

  match (horizontal_taps, vertical_taps) {
    (Some(horizontal), Some(vertical)) => {
      let Ok(mut storage) = take_mid(pool);
      let filtered = storage.as_mut();
      fill_horizontal_tap_rows(&source, height, filtered, |target, input_2| {
        (writer.horizontal_buffered)(target, input_2, width, horizontal)
      })?;

      for (ordinal, target) in (0..height).zip(output_rows) {
        let row = MotionRow { width, ordinal };
        (writer.vertical_buffered)(target, filtered, row, vertical)?;
      }

      let Ok(()) = put_mid(pool, storage);
    }
    (Some(horizontal), None) => {
      for (ordinal, target) in (0..height).zip(output_rows) {
        let position = Point {
          x: -3,
          y: i64::from(ordinal),
        };
        let reference = source.row_tail(position)?;
        (writer.horizontal_direct)(target, reference, width, horizontal)?;
      }
    }
    (None, Some(vertical)) => {
      for (ordinal, target) in (0..height).zip(output_rows) {
        let row = MotionRow { width, ordinal };
        (writer.vertical_direct)(target, &source, row, vertical)?;
      }
    }
    (None, None) => {
      let inputs = input.rows(source.samples).map_err(|_| Rav1dError::EINVAL)?;

      for (target, reference) in output_rows.zip(inputs) {
        let Ok(()) = copy(target, reference);
      }
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[doc = "Connects each byte SIMD backend to the shared eight-tap prediction loop."]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one four-callback definition covers both byte eight-tap predictor backends while retaining their compile-time CPU tokens"
  )
)]
macro_rules! byte_eight_tap_prediction {
  ($token:ident; $horizontal:path, $vertical:path) => {
    MotionRowWriters {
      horizontal_buffered: |target: &mut [i16], reference: &[u8], width, taps: &[i8; 8]| {
        $horizontal($token, target, reference, width, taps, 2)
      },
      vertical_buffered: |target: &mut [i16],
                          mid: &[[i16; MID_STRIDE]],
                          row: MotionRow,
                          taps: &[i8; 8]| {
        let Ok(ordinal) = console_core_number_conversion::index(row.ordinal);
        let filtered = mid.get(ordinal..).ok_or(Rav1dError::EINVAL)?;
        $vertical($token, filtered, target, row.width, taps, 6)
      },
      horizontal_direct: |target: &mut [i16], reference: &[u8], width, taps: &[i8; 8]| {
        $horizontal($token, target, reference, width, taps, 2)
      },
      vertical_direct: |target: &mut [i16],
                        source: &PixelSource<'_, u8>,
                        row: MotionRow,
                        taps: &[i8; 8]| {
        let mut filtered = [[0i16; MID_STRIDE]; 8];
        let Ok(width) = console_core_number_conversion::index(row.width);
        let first = i64::from(row.ordinal).wrapping_sub(3);

        for (ordinal, line) in (0_i64..8).zip(&mut filtered) {
          let position = Point {
            x: 0,
            y: first.wrapping_add(ordinal),
          };
          let reference = source.row_tail(position)?;
          let input = reference.get(..width).ok_or(Rav1dError::EINVAL)?;
          let Ok(()) = prepare_byte_bilinear_identity(line, input);
        }

        $vertical($token, &filtered, target, row.width, taps, 6)
      },
    }
  };
}

#[cfg(target_arch = "x86_64")]
#[doc = "Connects each word SIMD backend to the shared eight-tap prediction loop."]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one four-callback definition covers both word eight-tap predictor backends while retaining their compile-time CPU tokens"
  )
)]
macro_rules! word_eight_tap_prediction {
  ($token:ident, $shift:expr; $horizontal:path, $vertical:path, $horizontal_direct:path, $vertical_direct:path) => {
    MotionRowWriters {
      horizontal_buffered: |target: &mut [i32], reference: &[u16], width, taps: &[i8; 8]| {
        $horizontal($token, target, reference, width, taps, $shift)
      },
      vertical_buffered: |target: &mut [i16],
                          mid: &[[i32; MID_STRIDE]],
                          row: MotionRow,
                          taps: &[i8; 8]| {
        $vertical(
          $token,
          target,
          mid,
          row,
          taps,
          &WordPreparationScale {
            shift: 6,
            bias: 8192,
          },
        )
      },
      horizontal_direct: |target: &mut [i16], reference: &[u16], width, taps: &[i8; 8]| {
        $horizontal_direct(
          $token,
          target,
          reference,
          width,
          taps,
          WordPreparationScale {
            shift: $shift,
            bias: 8192,
          },
        )
      },
      vertical_direct: |target: &mut [i16],
                        source: &PixelSource<'_, u16>,
                        row: MotionRow,
                        taps: &[i8; 8]| {
        let position = Point {
          x: 0,
          y: i64::from(row.ordinal).wrapping_sub(3),
        };
        let reference = source.row_tail(position)?;
        let pitch = source.stride_bytes.wrapping_div(2);
        $vertical_direct(
          $token,
          target,
          reference,
          pitch,
          row.width,
          taps,
          WordPreparationScale {
            shift: $shift,
            bias: 8192,
          },
        )
      },
    }
  };
}

#[cfg(target_arch = "x86_64")]
fn prepare_word_identity(
  output: &mut [i16],
  source: &[u16],
  intermediate_bits: u32,
) -> Result<(), Never> {
  for (target, &pixel) in output.iter_mut().zip(source) {
    let scaled = i32::from(pixel)
      .wrapping_shl(intermediate_bits)
      .wrapping_sub(8192);
    let Ok(sample) = console_core_number_conversion::wrapped::<_, i16>(scaled);
    *target = sample;
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn prep_8tap_8bpc_avx2_impl_inner(
  token: Desktop64,
  output: &mut [i16],
  source: PixelSource<'_, u8>,
  block_extent: Size<i32>,
  motion_phase: Point<i32>,
  axes: Point<Rav1dFilterMode>,
) -> Rav1dResult {
  let sampling = reference::Sampling {
    size: block_extent,
    phase: motion_phase,
  };
  let request = EightTapPrediction {
    output,
    source,
    sampling,
    axes,
  };
  let writer = byte_eight_tap_prediction!(
    token;
    h_filter_8tap_8bpc_avx2_inner,
    v_filter_8tap_to_i16_avx2_inner
  );
  prepare_eight_tap(
    request,
    writer,
    &MID_I16_135,
    prepare_byte_bilinear_identity,
  )
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn prep_8tap_8bpc_avx512_impl_inner(
  token: Server64,
  output: &mut [i16],
  source: PixelSource<'_, u8>,
  block_extent: Size<i32>,
  motion_phase: Point<i32>,
  axes: Point<Rav1dFilterMode>,
) -> Rav1dResult {
  let sampling = reference::Sampling {
    size: block_extent,
    phase: motion_phase,
  };
  let request = EightTapPrediction {
    output,
    source,
    sampling,
    axes,
  };
  let writer = byte_eight_tap_prediction!(
    token;
    h_filter_8tap_8bpc_avx512_inner,
    v_filter_8tap_to_i16_avx512_inner
  );
  prepare_eight_tap(
    request,
    writer,
    &MID_I16_135,
    prepare_byte_bilinear_identity,
  )
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_filter_8tap_to_i16_avx2_inner(
  _token: Desktop64,
  mid: &[[i16; MID_STRIDE]],
  dst: &mut [i16],
  width: u32,
  filter: &[i8; 8],
  sh: u8,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let rnd = _mm256_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1i32, u32::from(sh)),
    1_u32,
  ));

  let c0 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[element, ..] => element,
    },
  ));
  let c1 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, element, ..] => element,
    },
  ));
  let c2 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, element, ..] => element,
    },
  ));
  let c3 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, element, ..] => element,
    },
  ));
  let c4 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c5 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c6 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c7 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));

  let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

  for col in (0..vector_columns).step_by(8) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_2 = <[_]>::get(
      &(*{
        let checked_value_36 = <[_]>::get(&*mid, 0).ok_or(Rav1dError::EINVAL)?;
        checked_value_36
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m0 = _mm256_cvtepi16_epi32(loadu_128!(checked_value_2));
    let checked_value_3 = <[_]>::get(
      &(*{
        let checked_value_37 = <[_]>::get(&*mid, 1).ok_or(Rav1dError::EINVAL)?;
        checked_value_37
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m1 = _mm256_cvtepi16_epi32(loadu_128!(checked_value_3));
    let checked_value_4 = <[_]>::get(
      &(*{
        let checked_value_38 = <[_]>::get(&*mid, 2).ok_or(Rav1dError::EINVAL)?;
        checked_value_38
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m2 = _mm256_cvtepi16_epi32(loadu_128!(checked_value_4));
    let checked_value_5 = <[_]>::get(
      &(*{
        let checked_value_39 = <[_]>::get(&*mid, 3).ok_or(Rav1dError::EINVAL)?;
        checked_value_39
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m3 = _mm256_cvtepi16_epi32(loadu_128!(checked_value_5));
    let checked_value_6 = <[_]>::get(
      &(*{
        let checked_value_40 = <[_]>::get(&*mid, 4).ok_or(Rav1dError::EINVAL)?;
        checked_value_40
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m4 = _mm256_cvtepi16_epi32(loadu_128!(checked_value_6));
    let checked_value_7 = <[_]>::get(
      &(*{
        let checked_value_41 = <[_]>::get(&*mid, 5).ok_or(Rav1dError::EINVAL)?;
        checked_value_41
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m5 = _mm256_cvtepi16_epi32(loadu_128!(checked_value_7));
    let checked_value_8 = <[_]>::get(
      &(*{
        let checked_value_42 = <[_]>::get(&*mid, 6).ok_or(Rav1dError::EINVAL)?;
        checked_value_42
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m6 = _mm256_cvtepi16_epi32(loadu_128!(checked_value_8));
    let checked_value_10 = <[_]>::get(
      &(*{
        let checked_value_43 = <[_]>::get(&*mid, 7).ok_or(Rav1dError::EINVAL)?;
        checked_value_43
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m7 = _mm256_cvtepi16_epi32(loadu_128!(checked_value_10));

    let mut sum = _mm256_mullo_epi32(m0, c0);
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m1, c1));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m2, c2));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m3, c3));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m4, c4));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m5, c5));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m6, c6));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m7, c7));

    let shift_count = _mm_cvtsi32_si128(
      <i32 as ::core::convert::From<::core::primitive::u8>>::from(sh),
    );
    let shifted = _mm256_sra_epi32(_mm256_add_epi32(sum, rnd), shift_count);

    let packed = _mm256_packs_epi32(shifted, shifted);
    let packed_2 = _mm256_permute4x64_epi64(packed, 0b11011000);

    let checked_value_11 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_128!(checked_value_11, _mm256_castsi256_si128(packed_2));
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value_2 = <[_]>::get(
        &(*{
          let checked_value_9 = <[_]>::get(&*mid, tap).ok_or(Rav1dError::EINVAL)?;
          checked_value_9
        }),
        column_index,
      )
      .ok_or(Rav1dError::EINVAL)?;
      Ok(i32::from(*checked_value_2))
    })?;

    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(round_shift(
      sum,
      u32::from(sh),
    )) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_filter_8tap_to_i16_avx512_inner(
  _token: Server64,
  mid: &[[i16; MID_STRIDE]],
  dst: &mut [i16],
  width: u32,
  filter: &[i8; 8],
  sh: u8,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let rnd = _mm512_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1i32, u32::from(sh)),
    1_u32,
  ));

  let c0 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[element, ..] => element,
    },
  ));
  let c1 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, element, ..] => element,
    },
  ));
  let c2 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, element, ..] => element,
    },
  ));
  let c3 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, element, ..] => element,
    },
  ));
  let c4 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c5 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c6 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c7 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_2 = <[_]>::get(
      &(*{
        let checked_value_36 = <[_]>::get(&*mid, 0).ok_or(Rav1dError::EINVAL)?;
        checked_value_36
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m0 = _mm512_cvtepi16_epi32(loadu_256!(checked_value_2));
    let checked_value_3 = <[_]>::get(
      &(*{
        let checked_value_37 = <[_]>::get(&*mid, 1).ok_or(Rav1dError::EINVAL)?;
        checked_value_37
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m1 = _mm512_cvtepi16_epi32(loadu_256!(checked_value_3));
    let checked_value_4 = <[_]>::get(
      &(*{
        let checked_value_38 = <[_]>::get(&*mid, 2).ok_or(Rav1dError::EINVAL)?;
        checked_value_38
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m2 = _mm512_cvtepi16_epi32(loadu_256!(checked_value_4));
    let checked_value_5 = <[_]>::get(
      &(*{
        let checked_value_39 = <[_]>::get(&*mid, 3).ok_or(Rav1dError::EINVAL)?;
        checked_value_39
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m3 = _mm512_cvtepi16_epi32(loadu_256!(checked_value_5));
    let checked_value_6 = <[_]>::get(
      &(*{
        let checked_value_40 = <[_]>::get(&*mid, 4).ok_or(Rav1dError::EINVAL)?;
        checked_value_40
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m4 = _mm512_cvtepi16_epi32(loadu_256!(checked_value_6));
    let checked_value_7 = <[_]>::get(
      &(*{
        let checked_value_41 = <[_]>::get(&*mid, 5).ok_or(Rav1dError::EINVAL)?;
        checked_value_41
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m5 = _mm512_cvtepi16_epi32(loadu_256!(checked_value_7));
    let checked_value_8 = <[_]>::get(
      &(*{
        let checked_value_42 = <[_]>::get(&*mid, 6).ok_or(Rav1dError::EINVAL)?;
        checked_value_42
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m6 = _mm512_cvtepi16_epi32(loadu_256!(checked_value_8));
    let checked_value_10 = <[_]>::get(
      &(*{
        let checked_value_43 = <[_]>::get(&*mid, 7).ok_or(Rav1dError::EINVAL)?;
        checked_value_43
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let m7 = _mm512_cvtepi16_epi32(loadu_256!(checked_value_10));

    let mut sum = _mm512_mullo_epi32(m0, c0);
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m1, c1));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m2, c2));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m3, c3));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m4, c4));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m5, c5));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m6, c6));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m7, c7));

    let shift_count = _mm_cvtsi32_si128(
      <i32 as ::core::convert::From<::core::primitive::u8>>::from(sh),
    );
    let shifted = _mm512_sra_epi32(_mm512_add_epi32(sum, rnd), shift_count);

    let packed = _mm512_cvtsepi32_epi16(shifted);

    let checked_value_11 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_256!(checked_value_11, packed);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value_2 = <[_]>::get(
        &(*{
          let checked_value_9 = <[_]>::get(&*mid, tap).ok_or(Rav1dError::EINVAL)?;
          checked_value_9
        }),
        column_index,
      )
      .ok_or(Rav1dError::EINVAL)?;
      Ok(i32::from(*checked_value_2))
    })?;

    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(round_shift(
      sum,
      u32::from(sh),
    )) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn h_filter_8tap_16bpc_avx2_inner(
  _token: Desktop64,
  dst: &mut [i32],
  src: &[u16],
  width: u32,
  filter: &[i8; 8],
  sh: i32,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let src_2 = &*src;

  let coeff0 = _mm256_set1_epi32(
    ::core::primitive::i32::wrapping_shl(
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(
        <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
          &[_, element, ..] => element,
        }),
      ),
      16_u32,
    ) | <i32 as ::core::convert::From<::core::primitive::u16>>::from(
      ::core::primitive::i16::cast_unsigned(<i16 as ::core::convert::From<
        ::core::primitive::i8,
      >>::from(match filter {
        &[element, ..] => element,
      })),
    ),
  );
  let coeff2 = _mm256_set1_epi32(
    ::core::primitive::i32::wrapping_shl(
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(
        <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
          &[_, _, _, element, ..] => element,
        }),
      ),
      16_u32,
    ) | <i32 as ::core::convert::From<::core::primitive::u16>>::from(
      ::core::primitive::i16::cast_unsigned(<i16 as ::core::convert::From<
        ::core::primitive::i8,
      >>::from(match filter {
        &[_, _, element, ..] => element,
      })),
    ),
  );
  let coeff4 = _mm256_set1_epi32(
    ::core::primitive::i32::wrapping_shl(
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(
        <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
          &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
      ),
      16_u32,
    ) | <i32 as ::core::convert::From<::core::primitive::u16>>::from(
      ::core::primitive::i16::cast_unsigned(<i16 as ::core::convert::From<
        ::core::primitive::i8,
      >>::from(match filter {
        &[_, _, _, _, _explicit_element, ..] => _explicit_element,
      })),
    ),
  );
  let coeff6 = _mm256_set1_epi32(
    ::core::primitive::i32::wrapping_shl(
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(
        <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
          &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
      ),
      16_u32,
    ) | <i32 as ::core::convert::From<::core::primitive::u16>>::from(
      ::core::primitive::i16::cast_unsigned(<i16 as ::core::convert::From<
        ::core::primitive::i8,
      >>::from(match filter {
        &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      })),
    ),
  );

  let rnd = _mm256_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1, i32::cast_unsigned(sh)),
    1_u32,
  ));
  let shift_count = _mm_cvtsi32_si128(sh);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

  for col in (0..vector_columns).step_by(8) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_2 = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(4)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a0 = loadu_64!(checked_value_2);
    let checked_value_3 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(1)).0..(Wrapping(column_index) + Wrapping(5)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a1 = loadu_64!(checked_value_3);
    let checked_value_4 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(2)).0..(Wrapping(column_index) + Wrapping(6)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a2 = loadu_64!(checked_value_4);
    let checked_value_5 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(3)).0..(Wrapping(column_index) + Wrapping(7)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a3 = loadu_64!(checked_value_5);
    let checked_value_6 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(4)).0..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a4 = loadu_64!(checked_value_6);
    let checked_value_7 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(5)).0..(Wrapping(column_index) + Wrapping(9)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a5 = loadu_64!(checked_value_7);
    let checked_value_8 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(6)).0..(Wrapping(column_index) + Wrapping(10)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a6 = loadu_64!(checked_value_8);
    let checked_value_9 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(7)).0..(Wrapping(column_index) + Wrapping(11)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a7 = loadu_64!(checked_value_9);
    let checked_value_10 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(8)).0..(Wrapping(column_index) + Wrapping(12)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a8 = loadu_64!(checked_value_10);
    let checked_value_11 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(9)).0..(Wrapping(column_index) + Wrapping(13)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a9 = loadu_64!(checked_value_11);
    let checked_value_12 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(10)).0..(Wrapping(column_index) + Wrapping(14)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a10 = loadu_64!(checked_value_12);
    let checked_value_13 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(11)).0..(Wrapping(column_index) + Wrapping(15)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a11 = loadu_64!(checked_value_13);

    let s0 = _mm256_inserti128_si256(_mm256_castsi128_si256(a0), a4, 1);
    let s1 = _mm256_inserti128_si256(_mm256_castsi128_si256(a1), a5, 1);
    let s2 = _mm256_inserti128_si256(_mm256_castsi128_si256(a2), a6, 1);
    let s3 = _mm256_inserti128_si256(_mm256_castsi128_si256(a3), a7, 1);
    let s4 = _mm256_inserti128_si256(_mm256_castsi128_si256(a4), a8, 1);
    let s5 = _mm256_inserti128_si256(_mm256_castsi128_si256(a5), a9, 1);
    let s6 = _mm256_inserti128_si256(_mm256_castsi128_si256(a6), a10, 1);
    let s7 = _mm256_inserti128_si256(_mm256_castsi128_si256(a7), a11, 1);

    let p01 = _mm256_unpacklo_epi16(s0, s1);
    let p23 = _mm256_unpacklo_epi16(s2, s3);
    let p45 = _mm256_unpacklo_epi16(s4, s5);
    let p67 = _mm256_unpacklo_epi16(s6, s7);

    let ma01 = _mm256_madd_epi16(p01, coeff0);
    let ma23 = _mm256_madd_epi16(p23, coeff2);
    let ma45 = _mm256_madd_epi16(p45, coeff4);
    let ma67 = _mm256_madd_epi16(p67, coeff6);

    let sum = _mm256_add_epi32(_mm256_add_epi32(ma01, ma23), _mm256_add_epi32(ma45, ma67));

    let result = _mm256_sra_epi32(_mm256_add_epi32(sum, rnd), shift_count);

    let checked_value_14 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_256!(checked_value_14, result);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value =
        <[_]>::get(&*src_2, column_index.wrapping_add(tap)).ok_or(Rav1dError::EINVAL)?;
      Ok(i32::from(*checked_value))
    })?;

    let r =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_shl(1, i32::cast_unsigned(sh)), 1_u32);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = i32::wrapping_shr(i32::wrapping_add(sum, r), i32::cast_unsigned(sh));
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_filter_8tap_16bpc_avx2_inner(
  _token: Desktop64,
  dst: &mut [u16],
  mid: &[[i32; MID_STRIDE]],
  row: MotionRow,
  filter: &[i8; 8],
  scale: &WordVerticalScale,
) -> Rav1dResult {
  let MotionRow { width, ordinal } = row;
  let Ok(y) = console_core_number_conversion::index(ordinal);
  let WordVerticalScale {
    shift: sh,
    maximum: max,
  } = *scale;
  let dst_2 = &mut *dst;

  let coeff: [i32; 8] = [
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[element, ..] => element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, element, ..] => element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, element, ..] => element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, element, ..] => element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    }),
  ];

  let rnd = _mm256_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1, i32::cast_unsigned(sh)),
    1_u32,
  ));
  let shift_count = _mm_cvtsi32_si128(sh);
  let zero = _mm256_setzero_si256();
  let max_val = _mm256_set1_epi32(max);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

  for col in (0..vector_columns).step_by(8) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      &(*{
        let checked_value_36 = <[_]>::get(&*mid, y.wrapping_add(0)).ok_or(Rav1dError::EINVAL)?;
        checked_value_36
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let r0 = loadu_256!(checked_value);
    let checked_value_2 = <[_]>::get(
      &(*{
        let checked_value_37 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_37
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let r1 = loadu_256!(checked_value_2);
    let checked_value_3 = <[_]>::get(
      &(*{
        let checked_value_38 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(2)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_38
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let r2 = loadu_256!(checked_value_3);
    let checked_value_4 = <[_]>::get(
      &(*{
        let checked_value_39 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(3)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_39
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let r3 = loadu_256!(checked_value_4);
    let checked_value_5 = <[_]>::get(
      &(*{
        let checked_value_40 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(4)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_40
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let r4 = loadu_256!(checked_value_5);
    let checked_value_6 = <[_]>::get(
      &(*{
        let checked_value_41 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(5)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_41
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let r5 = loadu_256!(checked_value_6);
    let checked_value_7 = <[_]>::get(
      &(*{
        let checked_value_42 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(6)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_42
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let r6 = loadu_256!(checked_value_7);
    let checked_value_8 = <[_]>::get(
      &(*{
        let checked_value_43 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(7)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_43
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let r7 = loadu_256!(checked_value_8);

    let c0 = _mm256_set1_epi32(match coeff {
      [element, ..] => element,
    });
    let c1 = _mm256_set1_epi32(match coeff {
      [_, element, ..] => element,
    });
    let c2 = _mm256_set1_epi32(match coeff {
      [_, _, element, ..] => element,
    });
    let c3 = _mm256_set1_epi32(match coeff {
      [_, _, _, element, ..] => element,
    });
    let c4 = _mm256_set1_epi32(match coeff {
      [_, _, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c5 = _mm256_set1_epi32(match coeff {
      [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c6 = _mm256_set1_epi32(match coeff {
      [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c7 = _mm256_set1_epi32(match coeff {
      [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    });

    let m0 = _mm256_mullo_epi32(r0, c0);
    let m1 = _mm256_mullo_epi32(r1, c1);
    let m2 = _mm256_mullo_epi32(r2, c2);
    let m3 = _mm256_mullo_epi32(r3, c3);
    let m4 = _mm256_mullo_epi32(r4, c4);
    let m5 = _mm256_mullo_epi32(r5, c5);
    let m6 = _mm256_mullo_epi32(r6, c6);
    let m7 = _mm256_mullo_epi32(r7, c7);

    let sum = _mm256_add_epi32(
      _mm256_add_epi32(_mm256_add_epi32(m0, m1), _mm256_add_epi32(m2, m3)),
      _mm256_add_epi32(_mm256_add_epi32(m4, m5), _mm256_add_epi32(m6, m7)),
    );

    let shifted = _mm256_sra_epi32(_mm256_add_epi32(sum, rnd), shift_count);
    let clamped_lo = _mm256_max_epi32(shifted, zero);
    let clamped = _mm256_min_epi32(clamped_lo, max_val);

    let packed = _mm256_packus_epi32(clamped, zero);

    let result = _mm256_permute4x64_epi64(packed, 0b00_00_10_00);

    let checked_value_9 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_128!(checked_value_9, _mm256_castsi256_si128(result));
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let mut sum = 0i32;

    for (i, coeff_element) in <[_]>::iter(&coeff).enumerate() {
      match (
        i32::wrapping_mul(*coeff_element, *{
          let checked_value_44 = <[_]>::get(
            &(*{
              let checked_value_19 =
                <[_]>::get(&*mid, (Wrapping(y) + Wrapping(i)).0).ok_or(Rav1dError::EINVAL)?;
              checked_value_19
            }),
            column_index,
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_44
        }),
        &mut sum,
      ) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };
    }

    let r =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_shl(1, i32::cast_unsigned(sh)), 1_u32);
    let output_sample =
      i32::wrapping_shr(i32::wrapping_add(sum, r), i32::cast_unsigned(sh)).clamp(0, max);
    *{
      let checked_value_20 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_20
    } =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(output_sample)
      {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_filter_8tap_16bpc_prep_avx2_inner(
  _token: Desktop64,
  dst: &mut [i16],
  mid: &[[i32; MID_STRIDE]],
  row: MotionRow,
  filter: &[i8; 8],
  scale: &WordPreparationScale,
) -> Rav1dResult {
  let MotionRow { width, ordinal } = row;
  let Ok(y) = console_core_number_conversion::index(ordinal);
  let WordPreparationScale {
    shift: sh,
    bias: prep_bias,
  } = *scale;
  let dst_2 = &mut *dst;
  let coeff: [i32; 8] = [
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[element, ..] => element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, element, ..] => element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, element, ..] => element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, element, ..] => element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    }),
  ];

  let rnd = _mm256_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1, i32::cast_unsigned(sh)),
    1_u32,
  ));
  let shift_count = _mm_cvtsi32_si128(sh);
  let bias = _mm256_set1_epi32(prep_bias);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

  for col in (0..vector_columns).step_by(8) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      &(*{
        let checked_value_36 = <[_]>::get(&*mid, y.wrapping_add(0)).ok_or(Rav1dError::EINVAL)?;
        checked_value_36
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let r0 = loadu_256!(checked_value);
    let checked_value_2 = <[_]>::get(
      &(*{
        let checked_value_37 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_37
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let r1 = loadu_256!(checked_value_2);
    let checked_value_3 = <[_]>::get(
      &(*{
        let checked_value_38 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(2)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_38
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let r2 = loadu_256!(checked_value_3);
    let checked_value_4 = <[_]>::get(
      &(*{
        let checked_value_39 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(3)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_39
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let r3 = loadu_256!(checked_value_4);
    let checked_value_5 = <[_]>::get(
      &(*{
        let checked_value_40 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(4)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_40
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let r4 = loadu_256!(checked_value_5);
    let checked_value_6 = <[_]>::get(
      &(*{
        let checked_value_41 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(5)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_41
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let r5 = loadu_256!(checked_value_6);
    let checked_value_7 = <[_]>::get(
      &(*{
        let checked_value_42 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(6)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_42
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let r6 = loadu_256!(checked_value_7);
    let checked_value_8 = <[_]>::get(
      &(*{
        let checked_value_43 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(7)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_43
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let r7 = loadu_256!(checked_value_8);

    let c0 = _mm256_set1_epi32(match coeff {
      [element, ..] => element,
    });
    let c1 = _mm256_set1_epi32(match coeff {
      [_, element, ..] => element,
    });
    let c2 = _mm256_set1_epi32(match coeff {
      [_, _, element, ..] => element,
    });
    let c3 = _mm256_set1_epi32(match coeff {
      [_, _, _, element, ..] => element,
    });
    let c4 = _mm256_set1_epi32(match coeff {
      [_, _, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c5 = _mm256_set1_epi32(match coeff {
      [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c6 = _mm256_set1_epi32(match coeff {
      [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c7 = _mm256_set1_epi32(match coeff {
      [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    });

    let m0 = _mm256_mullo_epi32(r0, c0);
    let m1 = _mm256_mullo_epi32(r1, c1);
    let m2 = _mm256_mullo_epi32(r2, c2);
    let m3 = _mm256_mullo_epi32(r3, c3);
    let m4 = _mm256_mullo_epi32(r4, c4);
    let m5 = _mm256_mullo_epi32(r5, c5);
    let m6 = _mm256_mullo_epi32(r6, c6);
    let m7 = _mm256_mullo_epi32(r7, c7);

    let sum = _mm256_add_epi32(
      _mm256_add_epi32(_mm256_add_epi32(m0, m1), _mm256_add_epi32(m2, m3)),
      _mm256_add_epi32(_mm256_add_epi32(m4, m5), _mm256_add_epi32(m6, m7)),
    );

    let shifted = _mm256_sra_epi32(_mm256_add_epi32(sum, rnd), shift_count);
    let biased = _mm256_sub_epi32(shifted, bias);

    let packed = _mm256_packs_epi32(biased, biased);

    let result = _mm256_permute4x64_epi64(packed, 0b00_00_10_00);

    let checked_value_9 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_128!(checked_value_9, _mm256_castsi256_si128(result));
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let mut sum = 0i32;

    for (i, coeff_element) in <[_]>::iter(&coeff).enumerate() {
      match (
        i32::wrapping_mul(*coeff_element, *{
          let checked_value_44 = <[_]>::get(
            &(*{
              let checked_value_19 =
                <[_]>::get(&*mid, (Wrapping(y) + Wrapping(i)).0).ok_or(Rav1dError::EINVAL)?;
              checked_value_19
            }),
            column_index,
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_44
        }),
        &mut sum,
      ) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };
    }

    let r =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_shl(1, i32::cast_unsigned(sh)), 1_u32);
    let output_sample = i32::wrapping_sub(
      i32::wrapping_shr(i32::wrapping_add(sum, r), i32::cast_unsigned(sh)),
      prep_bias,
    );
    *{
      let checked_value_20 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_20
    } =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(output_sample)
      {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn h_filter_8tap_16bpc_put_avx2_inner(
  _token: Desktop64,
  dst: &mut [u16],
  src: &[u16],
  width: u32,
  filter: &[i8; 8],
  max: i32,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let src_2 = &*src;

  let coeff0 = _mm256_set1_epi32(
    ::core::primitive::i32::wrapping_shl(
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(
        <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
          &[_, element, ..] => element,
        }),
      ),
      16_u32,
    ) | <i32 as ::core::convert::From<::core::primitive::u16>>::from(
      ::core::primitive::i16::cast_unsigned(<i16 as ::core::convert::From<
        ::core::primitive::i8,
      >>::from(match filter {
        &[element, ..] => element,
      })),
    ),
  );
  let coeff2 = _mm256_set1_epi32(
    ::core::primitive::i32::wrapping_shl(
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(
        <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
          &[_, _, _, element, ..] => element,
        }),
      ),
      16_u32,
    ) | <i32 as ::core::convert::From<::core::primitive::u16>>::from(
      ::core::primitive::i16::cast_unsigned(<i16 as ::core::convert::From<
        ::core::primitive::i8,
      >>::from(match filter {
        &[_, _, element, ..] => element,
      })),
    ),
  );
  let coeff4 = _mm256_set1_epi32(
    ::core::primitive::i32::wrapping_shl(
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(
        <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
          &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
      ),
      16_u32,
    ) | <i32 as ::core::convert::From<::core::primitive::u16>>::from(
      ::core::primitive::i16::cast_unsigned(<i16 as ::core::convert::From<
        ::core::primitive::i8,
      >>::from(match filter {
        &[_, _, _, _, _explicit_element, ..] => _explicit_element,
      })),
    ),
  );
  let coeff6 = _mm256_set1_epi32(
    ::core::primitive::i32::wrapping_shl(
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(
        <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
          &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
      ),
      16_u32,
    ) | <i32 as ::core::convert::From<::core::primitive::u16>>::from(
      ::core::primitive::i16::cast_unsigned(<i16 as ::core::convert::From<
        ::core::primitive::i8,
      >>::from(match filter {
        &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      })),
    ),
  );

  let intermediate_bits = match ::core::primitive::bool::then_some(
    ::core::primitive::i32::wrapping_shr(max, 11_u32) != 0,
    (),
  ) {
    ::core::option::Option::Some(()) => 2,
    ::core::option::Option::None => 4,
  };
  let rnd = _mm256_set1_epi32(i32::wrapping_add(
    32,
    ::core::primitive::i32::wrapping_shr(
      i32::wrapping_shl(
        1,
        i32::cast_unsigned(i32::wrapping_sub(6, intermediate_bits)),
      ),
      1_u32,
    ),
  ));
  let shift_count = _mm_cvtsi32_si128(6);
  let zero = _mm256_setzero_si256();
  let max_val = _mm256_set1_epi32(max);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

  for col in (0..vector_columns).step_by(8) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_2 = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(4)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a0 = loadu_64!(checked_value_2);
    let checked_value_3 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(1)).0..(Wrapping(column_index) + Wrapping(5)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a1 = loadu_64!(checked_value_3);
    let checked_value_4 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(2)).0..(Wrapping(column_index) + Wrapping(6)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a2 = loadu_64!(checked_value_4);
    let checked_value_5 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(3)).0..(Wrapping(column_index) + Wrapping(7)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a3 = loadu_64!(checked_value_5);
    let checked_value_6 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(4)).0..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a4 = loadu_64!(checked_value_6);
    let checked_value_7 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(5)).0..(Wrapping(column_index) + Wrapping(9)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a5 = loadu_64!(checked_value_7);
    let checked_value_8 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(6)).0..(Wrapping(column_index) + Wrapping(10)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a6 = loadu_64!(checked_value_8);
    let checked_value_9 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(7)).0..(Wrapping(column_index) + Wrapping(11)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a7 = loadu_64!(checked_value_9);
    let checked_value_10 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(8)).0..(Wrapping(column_index) + Wrapping(12)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a8 = loadu_64!(checked_value_10);
    let checked_value_11 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(9)).0..(Wrapping(column_index) + Wrapping(13)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a9 = loadu_64!(checked_value_11);
    let checked_value_12 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(10)).0..(Wrapping(column_index) + Wrapping(14)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a10 = loadu_64!(checked_value_12);
    let checked_value_13 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(11)).0..(Wrapping(column_index) + Wrapping(15)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a11 = loadu_64!(checked_value_13);

    let s0 = _mm256_inserti128_si256(_mm256_castsi128_si256(a0), a4, 1);
    let s1 = _mm256_inserti128_si256(_mm256_castsi128_si256(a1), a5, 1);
    let s2 = _mm256_inserti128_si256(_mm256_castsi128_si256(a2), a6, 1);
    let s3 = _mm256_inserti128_si256(_mm256_castsi128_si256(a3), a7, 1);
    let s4 = _mm256_inserti128_si256(_mm256_castsi128_si256(a4), a8, 1);
    let s5 = _mm256_inserti128_si256(_mm256_castsi128_si256(a5), a9, 1);
    let s6 = _mm256_inserti128_si256(_mm256_castsi128_si256(a6), a10, 1);
    let s7 = _mm256_inserti128_si256(_mm256_castsi128_si256(a7), a11, 1);

    let p01 = _mm256_unpacklo_epi16(s0, s1);
    let p23 = _mm256_unpacklo_epi16(s2, s3);
    let p45 = _mm256_unpacklo_epi16(s4, s5);
    let p67 = _mm256_unpacklo_epi16(s6, s7);

    let ma01 = _mm256_madd_epi16(p01, coeff0);
    let ma23 = _mm256_madd_epi16(p23, coeff2);
    let ma45 = _mm256_madd_epi16(p45, coeff4);
    let ma67 = _mm256_madd_epi16(p67, coeff6);

    let sum = _mm256_add_epi32(_mm256_add_epi32(ma01, ma23), _mm256_add_epi32(ma45, ma67));

    let shifted = _mm256_sra_epi32(_mm256_add_epi32(sum, rnd), shift_count);
    let clamped_lo = _mm256_max_epi32(shifted, zero);
    let clamped = _mm256_min_epi32(clamped_lo, max_val);

    let packed = _mm256_packus_epi32(clamped, zero);
    let result = _mm256_permute4x64_epi64(packed, 0b00_00_10_00);

    let checked_value_14 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_128!(checked_value_14, _mm256_castsi256_si128(result));
  }

  let scalar_rnd = i32::wrapping_add(
    32,
    ::core::primitive::i32::wrapping_shr(
      i32::wrapping_shl(
        1,
        i32::cast_unsigned(i32::wrapping_sub(6, intermediate_bits)),
      ),
      1_u32,
    ),
  );

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value =
        <[_]>::get(&*src_2, column_index.wrapping_add(tap)).ok_or(Rav1dError::EINVAL)?;
      Ok(i32::from(*checked_value))
    })?;

    let output_sample =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(sum, scalar_rnd), 6_u32)
        .clamp(0, max);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(output_sample)
      {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_filter_8tap_16bpc_direct_avx2_inner(
  _token: Desktop64,
  dst: &mut [u16],
  src: &[u16],
  src_stride: i64,
  width: u32,
  filter: &[i8; 8],
  max: i32,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let src_2 = &*src;
  let coeff: [i32; 8] = [
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[element, ..] => element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, element, ..] => element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, element, ..] => element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, element, ..] => element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    }),
  ];

  let c0 = _mm256_set1_epi32(match coeff {
    [element, ..] => element,
  });
  let c1 = _mm256_set1_epi32(match coeff {
    [_, element, ..] => element,
  });
  let c2 = _mm256_set1_epi32(match coeff {
    [_, _, element, ..] => element,
  });
  let c3 = _mm256_set1_epi32(match coeff {
    [_, _, _, element, ..] => element,
  });
  let c4 = _mm256_set1_epi32(match coeff {
    [_, _, _, _, _explicit_element, ..] => _explicit_element,
  });
  let c5 = _mm256_set1_epi32(match coeff {
    [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
  });
  let c6 = _mm256_set1_epi32(match coeff {
    [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
  });
  let c7 = _mm256_set1_epi32(match coeff {
    [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
  });

  let rnd = _mm256_set1_epi32(32);
  let shift_count = _mm_cvtsi32_si128(6);
  let zero = _mm256_setzero_si256();
  let max_val = _mm256_set1_epi32(max);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

  let pitch = u64::try_from(src_stride).map_err(|_| Rav1dError::EINVAL)?;
  let Ok(stride_u) = console_core_number_conversion::index(pitch);

  for col in (0..vector_columns).step_by(8) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p0 = _mm256_cvtepu16_epi32(loadu_128!(checked_value));
    let checked_value_2 = <[_]>::get(
      &*src_2,
      (Wrapping(stride_u) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping(stride_u) + Wrapping(column_index)).0) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p1 = _mm256_cvtepu16_epi32(loadu_128!(checked_value_2));
    let checked_value_3 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(2) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(2) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p2 = _mm256_cvtepu16_epi32(loadu_128!(checked_value_3));
    let checked_value_4 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(3) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(3) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p3 = _mm256_cvtepu16_epi32(loadu_128!(checked_value_4));
    let checked_value_5 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(4) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(4) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p4 = _mm256_cvtepu16_epi32(loadu_128!(checked_value_5));
    let checked_value_6 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(5) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(5) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p5 = _mm256_cvtepu16_epi32(loadu_128!(checked_value_6));
    let checked_value_7 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(6) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(6) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p6 = _mm256_cvtepu16_epi32(loadu_128!(checked_value_7));
    let checked_value_8 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(7) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(7) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p7 = _mm256_cvtepu16_epi32(loadu_128!(checked_value_8));

    let m0 = _mm256_mullo_epi32(p0, c0);
    let m1 = _mm256_mullo_epi32(p1, c1);
    let m2 = _mm256_mullo_epi32(p2, c2);
    let m3 = _mm256_mullo_epi32(p3, c3);
    let m4 = _mm256_mullo_epi32(p4, c4);
    let m5 = _mm256_mullo_epi32(p5, c5);
    let m6 = _mm256_mullo_epi32(p6, c6);
    let m7 = _mm256_mullo_epi32(p7, c7);

    let sum = _mm256_add_epi32(
      _mm256_add_epi32(_mm256_add_epi32(m0, m1), _mm256_add_epi32(m2, m3)),
      _mm256_add_epi32(_mm256_add_epi32(m4, m5), _mm256_add_epi32(m6, m7)),
    );

    let shifted = _mm256_sra_epi32(_mm256_add_epi32(sum, rnd), shift_count);
    let clamped_lo = _mm256_max_epi32(shifted, zero);
    let clamped = _mm256_min_epi32(clamped_lo, max_val);

    let packed = _mm256_packus_epi32(clamped, zero);
    let result = _mm256_permute4x64_epi64(packed, 0b00_00_10_00);

    let checked_value_9 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_128!(checked_value_9, _mm256_castsi256_si128(result));
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let mut sum = 0i32;

    for (i, coeff_element) in <[_]>::iter(&coeff).enumerate() {
      let checked_value_10 = <[_]>::get(
        &*src_2,
        (Wrapping((Wrapping(i) * Wrapping(stride_u)).0) + Wrapping(column_index)).0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let px = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_10);

      match (i32::wrapping_mul(*coeff_element, px), &mut sum) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };
    }

    let output_sample =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(sum, 32), 6_u32).clamp(0, max);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(output_sample)
      {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn h_filter_8tap_16bpc_prep_direct_avx2_inner(
  _token: Desktop64,
  dst: &mut [i16],
  src: &[u16],
  width: u32,
  filter: &[i8; 8],
  scale: WordPreparationScale,
) -> Rav1dResult {
  let WordPreparationScale {
    shift: sh,
    bias: prep_bias,
  } = scale;
  let dst_2 = &mut *dst;
  let src_2 = &*src;

  let coeff0 = _mm256_set1_epi32(
    ::core::primitive::i32::wrapping_shl(
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(
        <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
          &[_, element, ..] => element,
        }),
      ),
      16_u32,
    ) | <i32 as ::core::convert::From<::core::primitive::u16>>::from(
      ::core::primitive::i16::cast_unsigned(<i16 as ::core::convert::From<
        ::core::primitive::i8,
      >>::from(match filter {
        &[element, ..] => element,
      })),
    ),
  );
  let coeff2 = _mm256_set1_epi32(
    ::core::primitive::i32::wrapping_shl(
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(
        <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
          &[_, _, _, element, ..] => element,
        }),
      ),
      16_u32,
    ) | <i32 as ::core::convert::From<::core::primitive::u16>>::from(
      ::core::primitive::i16::cast_unsigned(<i16 as ::core::convert::From<
        ::core::primitive::i8,
      >>::from(match filter {
        &[_, _, element, ..] => element,
      })),
    ),
  );
  let coeff4 = _mm256_set1_epi32(
    ::core::primitive::i32::wrapping_shl(
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(
        <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
          &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
      ),
      16_u32,
    ) | <i32 as ::core::convert::From<::core::primitive::u16>>::from(
      ::core::primitive::i16::cast_unsigned(<i16 as ::core::convert::From<
        ::core::primitive::i8,
      >>::from(match filter {
        &[_, _, _, _, _explicit_element, ..] => _explicit_element,
      })),
    ),
  );
  let coeff6 = _mm256_set1_epi32(
    ::core::primitive::i32::wrapping_shl(
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(
        <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
          &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
      ),
      16_u32,
    ) | <i32 as ::core::convert::From<::core::primitive::u16>>::from(
      ::core::primitive::i16::cast_unsigned(<i16 as ::core::convert::From<
        ::core::primitive::i8,
      >>::from(match filter {
        &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      })),
    ),
  );

  let rnd = _mm256_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1, i32::cast_unsigned(sh)),
    1_u32,
  ));
  let shift_count = _mm_cvtsi32_si128(sh);
  let bias = _mm256_set1_epi32(prep_bias);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

  for col in (0..vector_columns).step_by(8) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_2 = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(4)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a0 = loadu_64!(checked_value_2);
    let checked_value_3 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(1)).0..(Wrapping(column_index) + Wrapping(5)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a1 = loadu_64!(checked_value_3);
    let checked_value_4 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(2)).0..(Wrapping(column_index) + Wrapping(6)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a2 = loadu_64!(checked_value_4);
    let checked_value_5 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(3)).0..(Wrapping(column_index) + Wrapping(7)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a3 = loadu_64!(checked_value_5);
    let checked_value_6 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(4)).0..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a4 = loadu_64!(checked_value_6);
    let checked_value_7 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(5)).0..(Wrapping(column_index) + Wrapping(9)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a5 = loadu_64!(checked_value_7);
    let checked_value_8 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(6)).0..(Wrapping(column_index) + Wrapping(10)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a6 = loadu_64!(checked_value_8);
    let checked_value_9 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(7)).0..(Wrapping(column_index) + Wrapping(11)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a7 = loadu_64!(checked_value_9);
    let checked_value_10 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(8)).0..(Wrapping(column_index) + Wrapping(12)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a8 = loadu_64!(checked_value_10);
    let checked_value_11 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(9)).0..(Wrapping(column_index) + Wrapping(13)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a9 = loadu_64!(checked_value_11);
    let checked_value_12 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(10)).0..(Wrapping(column_index) + Wrapping(14)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a10 = loadu_64!(checked_value_12);
    let checked_value_13 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(11)).0..(Wrapping(column_index) + Wrapping(15)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 4]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let a11 = loadu_64!(checked_value_13);

    let s0 = _mm256_inserti128_si256(_mm256_castsi128_si256(a0), a4, 1);
    let s1 = _mm256_inserti128_si256(_mm256_castsi128_si256(a1), a5, 1);
    let s2 = _mm256_inserti128_si256(_mm256_castsi128_si256(a2), a6, 1);
    let s3 = _mm256_inserti128_si256(_mm256_castsi128_si256(a3), a7, 1);
    let s4 = _mm256_inserti128_si256(_mm256_castsi128_si256(a4), a8, 1);
    let s5 = _mm256_inserti128_si256(_mm256_castsi128_si256(a5), a9, 1);
    let s6 = _mm256_inserti128_si256(_mm256_castsi128_si256(a6), a10, 1);
    let s7 = _mm256_inserti128_si256(_mm256_castsi128_si256(a7), a11, 1);

    let p01 = _mm256_unpacklo_epi16(s0, s1);
    let p23 = _mm256_unpacklo_epi16(s2, s3);
    let p45 = _mm256_unpacklo_epi16(s4, s5);
    let p67 = _mm256_unpacklo_epi16(s6, s7);

    let ma01 = _mm256_madd_epi16(p01, coeff0);
    let ma23 = _mm256_madd_epi16(p23, coeff2);
    let ma45 = _mm256_madd_epi16(p45, coeff4);
    let ma67 = _mm256_madd_epi16(p67, coeff6);

    let sum = _mm256_add_epi32(_mm256_add_epi32(ma01, ma23), _mm256_add_epi32(ma45, ma67));

    let shifted = _mm256_sra_epi32(_mm256_add_epi32(sum, rnd), shift_count);
    let biased = _mm256_sub_epi32(shifted, bias);

    let packed = _mm256_packs_epi32(biased, biased);
    let result = _mm256_permute4x64_epi64(packed, 0b00_00_10_00);

    let checked_value_14 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_128!(checked_value_14, _mm256_castsi256_si128(result));
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value =
        <[_]>::get(&*src_2, column_index.wrapping_add(tap)).ok_or(Rav1dError::EINVAL)?;
      Ok(i32::from(*checked_value))
    })?;

    let r =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_shl(1, i32::cast_unsigned(sh)), 1_u32);
    let output_sample = i32::wrapping_sub(
      i32::wrapping_shr(i32::wrapping_add(sum, r), i32::cast_unsigned(sh)),
      prep_bias,
    );
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(output_sample)
      {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_filter_8tap_16bpc_prep_direct_avx2_inner(
  _token: Desktop64,
  dst: &mut [i16],
  src: &[u16],
  src_stride: i64,
  width: u32,
  filter: &[i8; 8],
  scale: WordPreparationScale,
) -> Rav1dResult {
  let WordPreparationScale {
    shift: sh,
    bias: prep_bias,
  } = scale;
  let dst_2 = &mut *dst;
  let src_2 = &*src;
  let coeff: [i32; 8] = [
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[element, ..] => element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, element, ..] => element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, element, ..] => element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, element, ..] => element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    }),
    <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    }),
  ];

  let c0 = _mm256_set1_epi32(match coeff {
    [element, ..] => element,
  });
  let c1 = _mm256_set1_epi32(match coeff {
    [_, element, ..] => element,
  });
  let c2 = _mm256_set1_epi32(match coeff {
    [_, _, element, ..] => element,
  });
  let c3 = _mm256_set1_epi32(match coeff {
    [_, _, _, element, ..] => element,
  });
  let c4 = _mm256_set1_epi32(match coeff {
    [_, _, _, _, _explicit_element, ..] => _explicit_element,
  });
  let c5 = _mm256_set1_epi32(match coeff {
    [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
  });
  let c6 = _mm256_set1_epi32(match coeff {
    [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
  });
  let c7 = _mm256_set1_epi32(match coeff {
    [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
  });

  let rnd = _mm256_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1, i32::cast_unsigned(sh)),
    1_u32,
  ));
  let shift_count = _mm_cvtsi32_si128(sh);
  let bias = _mm256_set1_epi32(prep_bias);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

  let pitch = u64::try_from(src_stride).map_err(|_| Rav1dError::EINVAL)?;
  let Ok(stride_u) = console_core_number_conversion::index(pitch);

  for col in (0..vector_columns).step_by(8) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p0 = _mm256_cvtepu16_epi32(loadu_128!(checked_value));
    let checked_value_2 = <[_]>::get(
      &*src_2,
      (Wrapping(stride_u) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping(stride_u) + Wrapping(column_index)).0) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p1 = _mm256_cvtepu16_epi32(loadu_128!(checked_value_2));
    let checked_value_3 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(2) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(2) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p2 = _mm256_cvtepu16_epi32(loadu_128!(checked_value_3));
    let checked_value_4 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(3) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(3) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p3 = _mm256_cvtepu16_epi32(loadu_128!(checked_value_4));
    let checked_value_5 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(4) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(4) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p4 = _mm256_cvtepu16_epi32(loadu_128!(checked_value_5));
    let checked_value_6 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(5) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(5) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p5 = _mm256_cvtepu16_epi32(loadu_128!(checked_value_6));
    let checked_value_7 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(6) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(6) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p6 = _mm256_cvtepu16_epi32(loadu_128!(checked_value_7));
    let checked_value_8 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(7) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(7) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p7 = _mm256_cvtepu16_epi32(loadu_128!(checked_value_8));

    let m0 = _mm256_mullo_epi32(p0, c0);
    let m1 = _mm256_mullo_epi32(p1, c1);
    let m2 = _mm256_mullo_epi32(p2, c2);
    let m3 = _mm256_mullo_epi32(p3, c3);
    let m4 = _mm256_mullo_epi32(p4, c4);
    let m5 = _mm256_mullo_epi32(p5, c5);
    let m6 = _mm256_mullo_epi32(p6, c6);
    let m7 = _mm256_mullo_epi32(p7, c7);

    let sum = _mm256_add_epi32(
      _mm256_add_epi32(_mm256_add_epi32(m0, m1), _mm256_add_epi32(m2, m3)),
      _mm256_add_epi32(_mm256_add_epi32(m4, m5), _mm256_add_epi32(m6, m7)),
    );

    let shifted = _mm256_sra_epi32(_mm256_add_epi32(sum, rnd), shift_count);
    let biased = _mm256_sub_epi32(shifted, bias);

    let packed = _mm256_packs_epi32(biased, biased);
    let result = _mm256_permute4x64_epi64(packed, 0b00_00_10_00);

    let checked_value_9 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_128!(checked_value_9, _mm256_castsi256_si128(result));
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let mut sum = 0i32;

    for (i, coeff_element) in <[_]>::iter(&coeff).enumerate() {
      let checked_value_10 = <[_]>::get(
        &*src_2,
        (Wrapping((Wrapping(i) * Wrapping(stride_u)).0) + Wrapping(column_index)).0,
      )
      .ok_or(Rav1dError::EINVAL)?;
      let px = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_10);

      match (i32::wrapping_mul(*coeff_element, px), &mut sum) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };
    }

    let r =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_shl(1, i32::cast_unsigned(sh)), 1_u32);
    let output_sample = i32::wrapping_sub(
      i32::wrapping_shr(i32::wrapping_add(sum, r), i32::cast_unsigned(sh)),
      prep_bias,
    );
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(output_sample)
      {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn h_filter_8tap_16bpc_avx512_inner(
  _token: Server64,
  dst: &mut [i32],
  src: &[u16],
  width: u32,
  filter: &[i8; 8],
  sh: i32,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let src_2 = &*src;

  let c0 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[element, ..] => element,
    },
  ));
  let c1 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, element, ..] => element,
    },
  ));
  let c2 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, element, ..] => element,
    },
  ));
  let c3 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, element, ..] => element,
    },
  ));
  let c4 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c5 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c6 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c7 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));

  let rnd = _mm512_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1, i32::cast_unsigned(sh)),
    1_u32,
  ));
  let shift_count = _mm_cvtsi32_si128(sh);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_2 = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p0 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_2));
    let checked_value_3 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(1)).0..(Wrapping(column_index) + Wrapping(17)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p1 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_3));
    let checked_value_4 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(2)).0..(Wrapping(column_index) + Wrapping(18)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p2 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_4));
    let checked_value_5 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(3)).0..(Wrapping(column_index) + Wrapping(19)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p3 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_5));
    let checked_value_6 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(4)).0..(Wrapping(column_index) + Wrapping(20)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p4 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_6));
    let checked_value_7 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(5)).0..(Wrapping(column_index) + Wrapping(21)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p5 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_7));
    let checked_value_8 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(6)).0..(Wrapping(column_index) + Wrapping(22)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p6 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_8));
    let checked_value_9 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(7)).0..(Wrapping(column_index) + Wrapping(23)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p7 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_9));

    let mut sum = _mm512_mullo_epi32(p0, c0);
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p1, c1));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p2, c2));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p3, c3));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p4, c4));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p5, c5));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p6, c6));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p7, c7));

    let result = _mm512_sra_epi32(_mm512_add_epi32(sum, rnd), shift_count);
    let checked_value_10 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    storeu_512!(checked_value_10, [i32; 16], result);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value =
        <[_]>::get(&*src_2, column_index.wrapping_add(tap)).ok_or(Rav1dError::EINVAL)?;
      Ok(i32::from(*checked_value))
    })?;

    let r =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_shl(1, i32::cast_unsigned(sh)), 1_u32);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = i32::wrapping_shr(i32::wrapping_add(sum, r), i32::cast_unsigned(sh));
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_filter_8tap_16bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u16],
  mid: &[[i32; MID_STRIDE]],
  row: MotionRow,
  filter: &[i8; 8],
  scale: &WordVerticalScale,
) -> Rav1dResult {
  let MotionRow { width, ordinal } = row;
  let Ok(y) = console_core_number_conversion::index(ordinal);
  let WordVerticalScale {
    shift: sh,
    maximum: max,
  } = *scale;
  let dst_2 = &mut *dst;

  let c0 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[element, ..] => element,
    },
  ));
  let c1 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, element, ..] => element,
    },
  ));
  let c2 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, element, ..] => element,
    },
  ));
  let c3 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, element, ..] => element,
    },
  ));
  let c4 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c5 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c6 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c7 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));

  let rnd = _mm512_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1, i32::cast_unsigned(sh)),
    1_u32,
  ));
  let shift_count = _mm_cvtsi32_si128(sh);
  let zero = _mm512_setzero_si512();
  let max_val = _mm512_set1_epi32(max);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      &(*{
        let checked_value_36 = <[_]>::get(&*mid, y.wrapping_add(0)).ok_or(Rav1dError::EINVAL)?;
        checked_value_36
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r0 = loadu_512!(checked_value, [i32; 16]);
    let checked_value_2 = <[_]>::get(
      &(*{
        let checked_value_37 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_37
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r1 = loadu_512!(checked_value_2, [i32; 16]);
    let checked_value_3 = <[_]>::get(
      &(*{
        let checked_value_38 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(2)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_38
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r2 = loadu_512!(checked_value_3, [i32; 16]);
    let checked_value_4 = <[_]>::get(
      &(*{
        let checked_value_39 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(3)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_39
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r3 = loadu_512!(checked_value_4, [i32; 16]);
    let checked_value_5 = <[_]>::get(
      &(*{
        let checked_value_40 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(4)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_40
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r4 = loadu_512!(checked_value_5, [i32; 16]);
    let checked_value_6 = <[_]>::get(
      &(*{
        let checked_value_41 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(5)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_41
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r5 = loadu_512!(checked_value_6, [i32; 16]);
    let checked_value_7 = <[_]>::get(
      &(*{
        let checked_value_42 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(6)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_42
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r6 = loadu_512!(checked_value_7, [i32; 16]);
    let checked_value_8 = <[_]>::get(
      &(*{
        let checked_value_43 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(7)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_43
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r7 = loadu_512!(checked_value_8, [i32; 16]);

    let mut sum = _mm512_mullo_epi32(r0, c0);
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r1, c1));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r2, c2));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r3, c3));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r4, c4));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r5, c5));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r6, c6));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r7, c7));

    let shifted = _mm512_sra_epi32(_mm512_add_epi32(sum, rnd), shift_count);
    let clamped = _mm512_min_epi32(_mm512_max_epi32(shifted, zero), max_val);
    let packed = _mm512_cvtusepi32_epi16(clamped);
    let checked_value_9 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_256!(checked_value_9, packed);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let coeff: [i32; 8] = filter.map(i32::from);
    let mut sum = 0i32;

    for (i, coeff_element) in <[_]>::iter(&coeff).enumerate() {
      match (
        i32::wrapping_mul(*coeff_element, *{
          let checked_value_44 = <[_]>::get(
            &(*{
              let checked_value_19 =
                <[_]>::get(&*mid, (Wrapping(y) + Wrapping(i)).0).ok_or(Rav1dError::EINVAL)?;
              checked_value_19
            }),
            column_index,
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_44
        }),
        &mut sum,
      ) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };
    }

    let r =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_shl(1, i32::cast_unsigned(sh)), 1_u32);
    let output_sample =
      i32::wrapping_shr(i32::wrapping_add(sum, r), i32::cast_unsigned(sh)).clamp(0, max);
    *{
      let checked_value_20 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_20
    } =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(output_sample)
      {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_filter_8tap_16bpc_prep_avx512_inner(
  _token: Server64,
  dst: &mut [i16],
  mid: &[[i32; MID_STRIDE]],
  row: MotionRow,
  filter: &[i8; 8],
  scale: &WordPreparationScale,
) -> Rav1dResult {
  let MotionRow { width, ordinal } = row;
  let Ok(y) = console_core_number_conversion::index(ordinal);
  let WordPreparationScale {
    shift: sh,
    bias: prep_bias,
  } = *scale;
  let dst_2 = &mut *dst;

  let c0 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[element, ..] => element,
    },
  ));
  let c1 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, element, ..] => element,
    },
  ));
  let c2 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, element, ..] => element,
    },
  ));
  let c3 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, element, ..] => element,
    },
  ));
  let c4 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c5 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c6 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c7 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));

  let rnd = _mm512_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1, i32::cast_unsigned(sh)),
    1_u32,
  ));
  let shift_count = _mm_cvtsi32_si128(sh);
  let bias = _mm512_set1_epi32(prep_bias);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      &(*{
        let checked_value_36 = <[_]>::get(&*mid, y.wrapping_add(0)).ok_or(Rav1dError::EINVAL)?;
        checked_value_36
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r0 = loadu_512!(checked_value, [i32; 16]);
    let checked_value_2 = <[_]>::get(
      &(*{
        let checked_value_37 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_37
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r1 = loadu_512!(checked_value_2, [i32; 16]);
    let checked_value_3 = <[_]>::get(
      &(*{
        let checked_value_38 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(2)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_38
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r2 = loadu_512!(checked_value_3, [i32; 16]);
    let checked_value_4 = <[_]>::get(
      &(*{
        let checked_value_39 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(3)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_39
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r3 = loadu_512!(checked_value_4, [i32; 16]);
    let checked_value_5 = <[_]>::get(
      &(*{
        let checked_value_40 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(4)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_40
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r4 = loadu_512!(checked_value_5, [i32; 16]);
    let checked_value_6 = <[_]>::get(
      &(*{
        let checked_value_41 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(5)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_41
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r5 = loadu_512!(checked_value_6, [i32; 16]);
    let checked_value_7 = <[_]>::get(
      &(*{
        let checked_value_42 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(6)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_42
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r6 = loadu_512!(checked_value_7, [i32; 16]);
    let checked_value_8 = <[_]>::get(
      &(*{
        let checked_value_43 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(7)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_43
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r7 = loadu_512!(checked_value_8, [i32; 16]);

    let mut sum = _mm512_mullo_epi32(r0, c0);
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r1, c1));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r2, c2));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r3, c3));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r4, c4));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r5, c5));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r6, c6));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(r7, c7));

    let shifted = _mm512_sra_epi32(_mm512_add_epi32(sum, rnd), shift_count);
    let biased = _mm512_sub_epi32(shifted, bias);
    let packed = _mm512_cvtsepi32_epi16(biased);
    let checked_value_9 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_256!(checked_value_9, packed);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let coeff: [i32; 8] = filter.map(i32::from);
    let mut sum = 0i32;

    for (i, coeff_element) in <[_]>::iter(&coeff).enumerate() {
      match (
        i32::wrapping_mul(*coeff_element, *{
          let checked_value_44 = <[_]>::get(
            &(*{
              let checked_value_19 =
                <[_]>::get(&*mid, (Wrapping(y) + Wrapping(i)).0).ok_or(Rav1dError::EINVAL)?;
              checked_value_19
            }),
            column_index,
          )
          .ok_or(Rav1dError::EINVAL)?;
          checked_value_44
        }),
        &mut sum,
      ) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };
    }

    let r =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_shl(1, i32::cast_unsigned(sh)), 1_u32);
    let output_sample = i32::wrapping_sub(
      i32::wrapping_shr(i32::wrapping_add(sum, r), i32::cast_unsigned(sh)),
      prep_bias,
    );
    *{
      let checked_value_20 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_20
    } =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(output_sample)
      {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn h_filter_8tap_16bpc_put_avx512_inner(
  token: Server64,
  dst: &mut [u16],
  src: &[u16],
  width: u32,
  filter: &[i8; 8],
  max: i32,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let src_2 = &*src;

  let Ok(coefficients) = word_tap_coefficients(token, *filter);

  let intermediate_bits = match ::core::primitive::bool::then_some(
    ::core::primitive::i32::wrapping_shr(max, 11_u32) != 0,
    (),
  ) {
    ::core::option::Option::Some(()) => 2,
    ::core::option::Option::None => 4,
  };
  let rnd = _mm512_set1_epi32(i32::wrapping_add(
    32,
    ::core::primitive::i32::wrapping_shr(
      i32::wrapping_shl(
        1,
        i32::cast_unsigned(i32::wrapping_sub(6, intermediate_bits)),
      ),
      1_u32,
    ),
  ));
  let shift_count = _mm_cvtsi32_si128(6);
  let zero = _mm512_setzero_si512();
  let max_val = _mm512_set1_epi32(max);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let window = src_2
      .get(column_index..)
      .and_then(<[u16]>::first_chunk::<23>)
      .ok_or(Rav1dError::EINVAL)?;
    let sum = horizontal_word_sums(token, window, coefficients)?;

    let shifted = _mm512_sra_epi32(_mm512_add_epi32(sum, rnd), shift_count);
    let clamped = _mm512_min_epi32(_mm512_max_epi32(shifted, zero), max_val);
    let packed = _mm512_cvtusepi32_epi16(clamped);
    let target = dst_2
      .get_mut(column_index..)
      .and_then(<[u16]>::first_chunk_mut::<16>)
      .ok_or(Rav1dError::EINVAL)?;
    storeu_256!(target, packed);
  }

  let scalar_rnd = i32::wrapping_add(
    32,
    ::core::primitive::i32::wrapping_shr(
      i32::wrapping_shl(
        1,
        i32::cast_unsigned(i32::wrapping_sub(6, intermediate_bits)),
      ),
      1_u32,
    ),
  );

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value =
        <[_]>::get(&*src_2, column_index.wrapping_add(tap)).ok_or(Rav1dError::EINVAL)?;
      Ok(i32::from(*checked_value))
    })?;

    let output_sample =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(sum, scalar_rnd), 6_u32)
        .clamp(0, max);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(output_sample)
      {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn h_filter_8tap_16bpc_prep_direct_avx512_inner(
  token: Server64,
  dst: &mut [i16],
  src: &[u16],
  width: u32,
  filter: &[i8; 8],
  scale: WordPreparationScale,
) -> Rav1dResult {
  let WordPreparationScale {
    shift: sh,
    bias: prep_bias,
  } = scale;
  let dst_2 = &mut *dst;
  let src_2 = &*src;

  let Ok(coefficients) = word_tap_coefficients(token, *filter);

  let rnd = _mm512_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1, i32::cast_unsigned(sh)),
    1_u32,
  ));
  let shift_count = _mm_cvtsi32_si128(sh);
  let bias = _mm512_set1_epi32(prep_bias);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let window = src_2
      .get(column_index..)
      .and_then(<[u16]>::first_chunk::<23>)
      .ok_or(Rav1dError::EINVAL)?;
    let sum = horizontal_word_sums(token, window, coefficients)?;

    let shifted = _mm512_sra_epi32(_mm512_add_epi32(sum, rnd), shift_count);
    let biased = _mm512_sub_epi32(shifted, bias);
    let packed = _mm512_cvtsepi32_epi16(biased);
    let target = dst_2
      .get_mut(column_index..)
      .and_then(<[i16]>::first_chunk_mut::<16>)
      .ok_or(Rav1dError::EINVAL)?;
    storeu_256!(target, packed);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value =
        <[_]>::get(&*src_2, column_index.wrapping_add(tap)).ok_or(Rav1dError::EINVAL)?;
      Ok(i32::from(*checked_value))
    })?;

    let r =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_shl(1, i32::cast_unsigned(sh)), 1_u32);
    let output_sample = i32::wrapping_sub(
      i32::wrapping_shr(i32::wrapping_add(sum, r), i32::cast_unsigned(sh)),
      prep_bias,
    );
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(output_sample)
      {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_filter_8tap_16bpc_direct_avx512_inner(
  _token: Server64,
  dst: &mut [u16],
  src: &[u16],
  src_stride: i64,
  width: u32,
  filter: &[i8; 8],
  max: i32,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let src_2 = &*src;

  let c0 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[element, ..] => element,
    },
  ));
  let c1 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, element, ..] => element,
    },
  ));
  let c2 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, element, ..] => element,
    },
  ));
  let c3 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, element, ..] => element,
    },
  ));
  let c4 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c5 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c6 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c7 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));

  let rnd = _mm512_set1_epi32(32);
  let shift_count = _mm_cvtsi32_si128(6);
  let zero = _mm512_setzero_si512();
  let max_val = _mm512_set1_epi32(max);

  let pitch = u64::try_from(src_stride).map_err(|_| Rav1dError::EINVAL)?;
  let Ok(stride_u) = console_core_number_conversion::index(pitch);
  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p0 = _mm512_cvtepu16_epi32(loadu_256!(checked_value));
    let checked_value_2 = <[_]>::get(
      &*src_2,
      (Wrapping(stride_u) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping(stride_u) + Wrapping(column_index)).0) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p1 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_2));
    let checked_value_3 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(2) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(2) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p2 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_3));
    let checked_value_4 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(3) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(3) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p3 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_4));
    let checked_value_5 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(4) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(4) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p4 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_5));
    let checked_value_6 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(5) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(5) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p5 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_6));
    let checked_value_7 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(6) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(6) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p6 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_7));
    let checked_value_8 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(7) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(7) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p7 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_8));

    let mut sum = _mm512_mullo_epi32(p0, c0);
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p1, c1));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p2, c2));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p3, c3));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p4, c4));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p5, c5));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p6, c6));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p7, c7));

    let shifted = _mm512_sra_epi32(_mm512_add_epi32(sum, rnd), shift_count);
    let clamped = _mm512_min_epi32(_mm512_max_epi32(shifted, zero), max_val);
    let packed = _mm512_cvtusepi32_epi16(clamped);
    let checked_value_9 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_256!(checked_value_9, packed);
  }

  let coeff: [i32; 8] = filter.map(i32::from);

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let mut sum = 0i32;

    for (i, coeff_element) in <[_]>::iter(&coeff).enumerate() {
      match (
        i32::wrapping_mul(
          *coeff_element,
          <i32 as ::core::convert::From<::core::primitive::u16>>::from(*{
            let checked_value_18 = <[_]>::get(
              &*src_2,
              (Wrapping((Wrapping(i) * Wrapping(stride_u)).0) + Wrapping(column_index)).0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            checked_value_18
          }),
        ),
        &mut sum,
      ) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };
    }

    let output_sample =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(sum, 32), 6_u32).clamp(0, max);
    *{
      let checked_value_19 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_19
    } =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(output_sample)
      {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_filter_8tap_16bpc_prep_direct_avx512_inner(
  _token: Server64,
  dst: &mut [i16],
  src: &[u16],
  src_stride: i64,
  width: u32,
  filter: &[i8; 8],
  scale: WordPreparationScale,
) -> Rav1dResult {
  let WordPreparationScale {
    shift: sh,
    bias: prep_bias,
  } = scale;
  let dst_2 = &mut *dst;
  let src_2 = &*src;

  let c0 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[element, ..] => element,
    },
  ));
  let c1 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, element, ..] => element,
    },
  ));
  let c2 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, element, ..] => element,
    },
  ));
  let c3 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, element, ..] => element,
    },
  ));
  let c4 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c5 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c6 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c7 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));

  let rnd = _mm512_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1, i32::cast_unsigned(sh)),
    1_u32,
  ));
  let shift_count = _mm_cvtsi32_si128(sh);
  let bias = _mm512_set1_epi32(prep_bias);

  let pitch = u64::try_from(src_stride).map_err(|_| Rav1dError::EINVAL)?;
  let Ok(stride_u) = console_core_number_conversion::index(pitch);
  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p0 = _mm512_cvtepu16_epi32(loadu_256!(checked_value));
    let checked_value_2 = <[_]>::get(
      &*src_2,
      (Wrapping(stride_u) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping(stride_u) + Wrapping(column_index)).0) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p1 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_2));
    let checked_value_3 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(2) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(2) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p2 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_3));
    let checked_value_4 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(3) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(3) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p3 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_4));
    let checked_value_5 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(4) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(4) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p4 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_5));
    let checked_value_6 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(5) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(5) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p5 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_6));
    let checked_value_7 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(6) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(6) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p6 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_7));
    let checked_value_8 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(7) * Wrapping(stride_u)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(7) * Wrapping(stride_u)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p7 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_8));

    let mut sum = _mm512_mullo_epi32(p0, c0);
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p1, c1));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p2, c2));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p3, c3));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p4, c4));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p5, c5));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p6, c6));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p7, c7));

    let shifted = _mm512_sra_epi32(_mm512_add_epi32(sum, rnd), shift_count);
    let biased = _mm512_sub_epi32(shifted, bias);
    let packed = _mm512_cvtsepi32_epi16(biased);
    let checked_value_9 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_256!(checked_value_9, packed);
  }

  let coeff: [i32; 8] = filter.map(i32::from);

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let mut sum = 0i32;

    for (i, coeff_element) in <[_]>::iter(&coeff).enumerate() {
      match (
        i32::wrapping_mul(
          *coeff_element,
          <i32 as ::core::convert::From<::core::primitive::u16>>::from(*{
            let checked_value_18 = <[_]>::get(
              &*src_2,
              (Wrapping((Wrapping(i) * Wrapping(stride_u)).0) + Wrapping(column_index)).0,
            )
            .ok_or(Rav1dError::EINVAL)?;
            checked_value_18
          }),
        ),
        &mut sum,
      ) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::i32::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };
    }

    let r =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_shl(1, i32::cast_unsigned(sh)), 1_u32);
    let output_sample = i32::wrapping_sub(
      i32::wrapping_shr(i32::wrapping_add(sum, r), i32::cast_unsigned(sh)),
      prep_bias,
    );
    *{
      let checked_value_19 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_19
    } =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(output_sample)
      {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct WordVerticalScale {
  shift: i32,
  maximum: i32,
}

#[cfg(target_arch = "x86_64")]
struct WordPreparationScale {
  shift: i32,
  bias: i32,
}

#[cfg(target_arch = "x86_64")]
struct MotionRow {
  width: u32,
  ordinal: u32,
}

#[cfg(target_arch = "x86_64")]
struct MotionRowWriters<HorizontalBuffered, VerticalBuffered, HorizontalDirect, VerticalDirect> {
  horizontal_buffered: HorizontalBuffered,
  vertical_buffered: VerticalBuffered,
  horizontal_direct: HorizontalDirect,
  vertical_direct: VerticalDirect,
}

#[cfg(target_arch = "x86_64")]
struct MotionOutput<'a, Pixel> {
  samples: &'a mut [Pixel],
  row_stride: i64,
  maximum: i32,
}

#[cfg(target_arch = "x86_64")]
impl<Pixel> MotionOutput<'_, Pixel> {
  #[inline]
  fn row_tail(&mut self, row: u32) -> Rav1dResult<&mut [Pixel]> {
    let offset = i64::from(row)
      .checked_mul(self.row_stride)
      .ok_or(Rav1dError::EINVAL)?;
    let start =
      console_core_number_conversion::checked_index(offset).map_err(|_| Rav1dError::EINVAL)?;
    self.samples.get_mut(start..).ok_or(Rav1dError::EINVAL)
  }
}

#[cfg(target_arch = "x86_64")]
struct EightTapOutput<'a, Pixel> {
  output: MotionOutput<'a, Pixel>,
  source: PixelSource<'a, Pixel>,
  sampling: reference::Sampling,
  axes: Point<Rav1dFilterMode>,
}

#[cfg(target_arch = "x86_64")]
#[doc = "Connects the selected word SIMD backend to the shared eight-tap output loop."]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one four-callback definition covers both word eight-tap output backends while retaining their compile-time CPU tokens"
  )
)]
macro_rules! word_tap_writer {
  ($token:ident; $horizontal:path, $vertical:path, $horizontal_direct:path, $vertical_direct:path) => {
    MotionRowWriters {
      horizontal_buffered: |target: &mut [i32], input: &[u16], width, taps: &[i8; 8], shift| {
        $horizontal($token, target, input, width, taps, shift)
      },
      vertical_buffered: |target: &mut [u16],
                          filtered: &[[i32; MID_STRIDE]],
                          geometry: MotionRow,
                          taps: &[i8; 8],
                          scale: &WordVerticalScale| {
        $vertical($token, target, filtered, geometry, taps, scale)
      },
      horizontal_direct: |target: &mut [u16], input: &[u16], width, taps: &[i8; 8], maximum| {
        $horizontal_direct($token, target, input, width, taps, maximum)
      },
      vertical_direct: |target: &mut [u16],
                        input: &[u16],
                        pitch,
                        width,
                        taps: &[i8; 8],
                        maximum| {
        $vertical_direct($token, target, input, pitch, width, taps, maximum)
      },
    }
  };
}

#[cfg(target_arch = "x86_64")]
fn put_eight_tap<Pixel: Copy, Intermediate: Copy + Default + 'static>(
  writer: MotionRowWriters<
    impl Fn(&mut [Intermediate], &[Pixel], u32, &[i8; 8], i32) -> Rav1dResult,
    impl Fn(
      &mut [Pixel],
      &[[Intermediate; MID_STRIDE]],
      MotionRow,
      &[i8; 8],
      &WordVerticalScale,
    ) -> Rav1dResult,
    impl Fn(&mut [Pixel], &[Pixel], u32, &[i8; 8], i32) -> Rav1dResult,
    impl Fn(&mut [Pixel], &[Pixel], i64, u32, &[i8; 8], i32) -> Rav1dResult,
  >,
  request: EightTapOutput<'_, Pixel>,
  pool: &'static MidPool<Intermediate, 135>,
) -> Rav1dResult {
  let EightTapOutput {
    mut output,
    source,
    sampling,
    axes,
  } = request;
  let EightTapBlock {
    size: Size { width, height },
    coefficients,
  } = EightTapBlock::new(sampling, axes)?;
  let maximum = output.maximum;
  let intermediate_bits = match maximum.wrapping_shr(11) {
    0 => 4i32,
    _ => 2i32,
  };
  let Point {
    x: horizontal,
    y: vertical,
  } = coefficients;

  match (horizontal, vertical) {
    (Some(horizontal_pass), Some(vertical_pass)) => {
      let Ok(mut scratch) = take_mid(pool);
      let horizontal_shift = 6i32.wrapping_sub(intermediate_bits);
      let vertical_scale = WordVerticalScale {
        shift: 6i32.wrapping_add(intermediate_bits),
        maximum,
      };
      fill_horizontal_tap_rows(&source, height, scratch.as_mut(), |target, input| {
        (writer.horizontal_buffered)(target, input, width, horizontal_pass, horizontal_shift)
      })?;

      for row in 0..height {
        let target = output.row_tail(row)?;
        (writer.vertical_buffered)(
          target,
          scratch.as_ref(),
          MotionRow {
            width,
            ordinal: row,
          },
          vertical_pass,
          &vertical_scale,
        )?;
      }

      let Ok(()) = put_mid(pool, scratch);
    }
    (Some(horizontal_only), None) => {
      for row in 0..height {
        let input = source.row_tail(Point {
          x: -3,
          y: i64::from(row),
        })?;
        let target = output.row_tail(row)?;
        (writer.horizontal_direct)(target, input, width, horizontal_only, maximum)?;
      }
    }
    (None, Some(vertical_only)) => {
      let Ok(pixel_bytes) =
        console_core_number_conversion::fixed_index(core::mem::size_of::<Pixel>());
      let stride = source
        .stride_bytes
        .checked_div(pixel_bytes.cast_signed())
        .ok_or(Rav1dError::EINVAL)?;

      for row in 0..height {
        let input = source.row_tail(Point {
          x: 0,
          y: i64::from(row).wrapping_sub(3),
        })?;
        let target = output.row_tail(row)?;
        (writer.vertical_direct)(target, input, stride, width, vertical_only, maximum)?;
      }
    }
    (None, None) => {
      let Ok(columns) = console_core_number_conversion::index(width);

      for row in 0..height {
        let input = source.row_tail(Point {
          x: 0,
          y: i64::from(row),
        })?;
        let target = output.row_tail(row)?;
        let into = target.get_mut(..columns).ok_or(Rav1dError::EINVAL)?;
        let from = input.get(..columns).ok_or(Rav1dError::EINVAL)?;
        let Ok(()) = copy_bilinear_pixels(into, from);
      }
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn put_8tap_16bpc_avx512_impl_inner(
  token: Server64,
  request: EightTapOutput<'_, u16>,
) -> Rav1dResult {
  let writer = word_tap_writer!(
    token;
    h_filter_8tap_16bpc_avx512_inner,
    v_filter_8tap_16bpc_avx512_inner,
    h_filter_8tap_16bpc_put_avx512_inner,
    v_filter_8tap_16bpc_direct_avx512_inner
  );
  put_eight_tap(writer, request, &MID_I32_135)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn prep_8tap_16bpc_avx512_impl_inner(
  token: Server64,
  output: &mut [i16],
  source: PixelSource<'_, u16>,
  block_extent: Size<i32>,
  motion_phase: Point<i32>,
  bitdepth_max: i32,
  axes: Point<Rav1dFilterMode>,
) -> Rav1dResult {
  let sampling = reference::Sampling {
    size: block_extent,
    phase: motion_phase,
  };
  let request = EightTapPrediction {
    output,
    source,
    sampling,
    axes,
  };
  let precision = match bitdepth_max.wrapping_shr(11) {
    0 => 4_u32,
    _ => 2_u32,
  };
  let Ok(shift) =
    console_core_number_conversion::wrapped::<_, i32>(6_u32.wrapping_sub(precision));
  let writer = word_eight_tap_prediction!(
    token, shift;
    h_filter_8tap_16bpc_avx512_inner,
    v_filter_8tap_16bpc_prep_avx512_inner,
    h_filter_8tap_16bpc_prep_direct_avx512_inner,
    v_filter_8tap_16bpc_prep_direct_avx512_inner
  );
  prepare_eight_tap(request, writer, &MID_I32_135, |target, reference| {
    prepare_word_identity(target, reference, precision)
  })
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn put_8tap_16bpc_avx2_impl_inner(
  token: Desktop64,
  request: EightTapOutput<'_, u16>,
) -> Rav1dResult {
  let writer = word_tap_writer!(
    token;
    h_filter_8tap_16bpc_avx2_inner,
    v_filter_8tap_16bpc_avx2_inner,
    h_filter_8tap_16bpc_put_avx2_inner,
    v_filter_8tap_16bpc_direct_avx2_inner
  );
  put_eight_tap(writer, request, &MID_I32_135)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn prep_8tap_16bpc_avx2_impl_inner(
  token: Desktop64,
  output: &mut [i16],
  source: PixelSource<'_, u16>,
  block_extent: Size<i32>,
  motion_phase: Point<i32>,
  bitdepth_max: i32,
  axes: Point<Rav1dFilterMode>,
) -> Rav1dResult {
  let sampling = reference::Sampling {
    size: block_extent,
    phase: motion_phase,
  };
  let request = EightTapPrediction {
    output,
    source,
    sampling,
    axes,
  };
  let precision = match bitdepth_max.wrapping_shr(11) {
    0 => 4_u32,
    _ => 2_u32,
  };
  let Ok(shift) =
    console_core_number_conversion::wrapped::<_, i32>(6_u32.wrapping_sub(precision));
  let writer = word_eight_tap_prediction!(
    token, shift;
    h_filter_8tap_16bpc_avx2_inner,
    v_filter_8tap_16bpc_prep_avx2_inner,
    h_filter_8tap_16bpc_prep_direct_avx2_inner,
    v_filter_8tap_16bpc_prep_direct_avx2_inner
  );
  prepare_eight_tap(request, writer, &MID_I32_135, |target, reference| {
    prepare_word_identity(target, reference, precision)
  })
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn h_filter_bilin_8bpc_avx2_inner(
  _token: Desktop64,
  dst: &mut [i16],
  src: &[u8],
  width: u32,
  mx: i32,
  sh: u8,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let src_2 = &*src;

  let mx_2 = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i8>(mx) {
    ::core::result::Result::Ok(converted_position) => converted_position,
  };
  let coeff0 = ::core::primitive::i8::cast_unsigned(i8::wrapping_sub(16, mx_2));
  let coeff1 = ::core::primitive::i8::cast_unsigned(mx_2);

  let coeffs = _mm256_set1_epi16(
    ::core::primitive::i16::wrapping_shl(
      <i16 as ::core::convert::From<::core::primitive::u8>>::from(coeff1),
      8_u32,
    ) | <i16 as ::core::convert::From<::core::primitive::u8>>::from(coeff0),
  );

  let rnd = match ::core::primitive::bool::then_some(sh > 0, ()) {
    ::core::option::Option::Some(()) => {
      _mm256_set1_epi16(i16::wrapping_shl(1, u32::from(u8::wrapping_sub(sh, 1))))
    }
    ::core::option::Option::None => _mm256_setzero_si256(),
  };

  let sh_reg = _mm_cvtsi32_si128(<i32 as ::core::convert::From<::core::primitive::u8>>::from(
    sh,
  ));

  let vector_columns = width.wrapping_sub(width.wrapping_rem(32));

  for x in (0..vector_columns).step_by(32) {
    let Ok(column_index) = console_core_number_conversion::index(x);

    let checked_value = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(32)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let src_lo = loadu_256!(checked_value);
    let checked_value_2 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(1)).0..(Wrapping(column_index) + Wrapping(33)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let src_hi = loadu_256!(checked_value_2);

    let pairs_lo = _mm256_unpacklo_epi8(src_lo, src_hi);
    let pairs_hi = _mm256_unpackhi_epi8(src_lo, src_hi);

    let result_lo = _mm256_maddubs_epi16(pairs_lo, coeffs);
    let result_hi = _mm256_maddubs_epi16(pairs_hi, coeffs);

    let result_lo_2 = _mm256_sra_epi16(_mm256_add_epi16(result_lo, rnd), sh_reg);
    let result_hi_2 = _mm256_sra_epi16(_mm256_add_epi16(result_hi, rnd), sh_reg);

    let lo_128 = _mm256_permute2x128_si256(result_lo_2, result_hi_2, 0x20);
    let hi_128 = _mm256_permute2x128_si256(result_lo_2, result_hi_2, 0x31);

    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_256!(checked_value_3, lo_128);
    let checked_value_4 = <[_]>::get_mut(
      &mut *dst_2,
      (Wrapping(column_index) + Wrapping(16)).0..(Wrapping(column_index) + Wrapping(32)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_256!(checked_value_4, hi_128);
  }

  for x in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(x);

    let checked_value_5 = <[_]>::get(&*src_2, column_index).ok_or(Rav1dError::EINVAL)?;
    let x0 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_5);
    let checked_value_6 =
      <[_]>::get(&*src_2, (Wrapping(column_index) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
    let x1 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_6);
    let pixel = i32::wrapping_add(
      i32::wrapping_mul(
        ::core::primitive::i32::wrapping_sub(
          16,
          <i32 as ::core::convert::From<::core::primitive::i8>>::from(mx_2),
        ),
        x0,
      ),
      i32::wrapping_mul(
        <i32 as ::core::convert::From<::core::primitive::i8>>::from(mx_2),
        x1,
      ),
    );
    let result = match ::core::primitive::bool::then_some(sh > 0, ()) {
      ::core::option::Option::Some(()) => i32::wrapping_shr(
        i32::wrapping_add(
          pixel,
          i32::wrapping_shl(1, u32::from(u8::wrapping_sub(sh, 1))),
        ),
        u32::from(sh),
      ),
      ::core::option::Option::None => pixel,
    };
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(result) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_filter_bilin_8bpc_avx2_inner(
  _token: Desktop64,
  dst: &mut [u8],
  mid: &[&[i16]],
  width: u32,
  my: i32,
  sh: u8,
  bd_max: i16,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let my_2 = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(my) {
    ::core::result::Result::Ok(converted_position) => converted_position,
  };
  let coeff0 = i32::wrapping_sub(16, my_2);
  let coeff1 = my_2;

  let c0_32 = _mm256_set1_epi32(coeff0);
  let c1_32 = _mm256_set1_epi32(coeff1);
  let rnd = _mm256_set1_epi32(match ::core::primitive::bool::then_some(sh > 0, ()) {
    ::core::option::Option::Some(()) => i32::wrapping_shl(1, u32::from(u8::wrapping_sub(sh, 1))),
    ::core::option::Option::None => 0,
  });
  let zero = _mm256_setzero_si256();
  let max_32 =
    _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i16>>::from(bd_max));
  let sh_reg = _mm_cvtsi32_si128(<i32 as ::core::convert::From<::core::primitive::u8>>::from(
    sh,
  ));

  let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

  for x in (0..vector_columns).step_by(8) {
    let Ok(column_index) = console_core_number_conversion::index(x);

    let checked_value = <[_]>::get(
      &*(*{
        let checked_value_36 = <[_]>::get(&*mid, 0).ok_or(Rav1dError::EINVAL)?;
        checked_value_36
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let row0 = _mm256_cvtepi16_epi32(loadu_128!(checked_value));
    let checked_value_2 = <[_]>::get(
      &*(*{
        let checked_value_37 = <[_]>::get(&*mid, 1).ok_or(Rav1dError::EINVAL)?;
        checked_value_37
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let row1 = _mm256_cvtepi16_epi32(loadu_128!(checked_value_2));

    let mul0 = _mm256_mullo_epi32(row0, c0_32);
    let mul1 = _mm256_mullo_epi32(row1, c1_32);
    let sum = _mm256_add_epi32(mul0, mul1);

    let result = _mm256_sra_epi32(_mm256_add_epi32(sum, rnd), sh_reg);

    let result_2 = _mm256_max_epi32(result, zero);
    let result_3 = _mm256_min_epi32(result_2, max_32);

    let packed16 = _mm256_packs_epi32(result_3, result_3);
    let packed16_2 = _mm256_permute4x64_epi64(packed16, 0xD8);
    let packed8 = _mm256_packus_epi16(packed16_2, packed16_2);

    let result_64 = _mm256_extract_epi64(packed8, 0);

    {
      let explicit_byte_copy_destination = <[_]>::get_mut(&mut *dst_2, column_index..)
        .and_then(<[_]>::first_chunk_mut::<8>)
        .ok_or(Rav1dError::EINVAL)?;
      *explicit_byte_copy_destination = result_64.to_le_bytes();
    };
  }

  for x in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(x);

    let checked_value_38 = <[_]>::get(
      &*(*{
        let checked_value_18 = <[_]>::get(&*mid, 0).ok_or(Rav1dError::EINVAL)?;
        checked_value_18
      }),
      column_index,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r0 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(*checked_value_38);
    let checked_value_39 = <[_]>::get(
      &*(*{
        let checked_value_20 = <[_]>::get(&*mid, 1).ok_or(Rav1dError::EINVAL)?;
        checked_value_20
      }),
      column_index,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r1 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(*checked_value_39);
    let pixel = i32::wrapping_add(i32::wrapping_mul(coeff0, r0), i32::wrapping_mul(coeff1, r1));
    let result = match ::core::primitive::bool::then_some(sh > 0, ()) {
      ::core::option::Option::Some(()) => i32::wrapping_shr(
        i32::wrapping_add(
          pixel,
          i32::wrapping_shl(1, u32::from(u8::wrapping_sub(sh, 1))),
        ),
        u32::from(sh),
      )
      .clamp(
        0,
        <i32 as ::core::convert::From<::core::primitive::i16>>::from(bd_max),
      ),
      ::core::option::Option::None => pixel.clamp(
        0,
        <i32 as ::core::convert::From<::core::primitive::i16>>::from(bd_max),
      ),
    };
    *{
      let checked_value_21 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_21
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn h_bilin_8bpc_put_avx2_inner(
  _token: Desktop64,
  dst: &mut [u8],
  src: &[u8],
  width: u32,
  mx: i32,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let src_2 = &*src;
  let mx_2 = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i8>(mx) {
    ::core::result::Result::Ok(converted_position) => converted_position,
  };
  let coeff0 = ::core::primitive::i8::cast_unsigned(i8::wrapping_sub(16, mx_2));
  let coeff1 = ::core::primitive::i8::cast_unsigned(mx_2);
  let coeffs = _mm256_set1_epi16(
    ::core::primitive::i16::wrapping_shl(
      <i16 as ::core::convert::From<::core::primitive::u8>>::from(coeff1),
      8_u32,
    ) | <i16 as ::core::convert::From<::core::primitive::u8>>::from(coeff0),
  );
  let rnd = _mm256_set1_epi16(8);
  let zero = _mm256_setzero_si256();
  let max = _mm256_set1_epi16(255);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(32));

  for x in (0..vector_columns).step_by(32) {
    let Ok(column_index) = console_core_number_conversion::index(x);

    let checked_value = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(32)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let src_lo = loadu_256!(checked_value);
    let checked_value_2 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(1)).0..(Wrapping(column_index) + Wrapping(33)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let src_hi = loadu_256!(checked_value_2);

    let pairs_lo = _mm256_unpacklo_epi8(src_lo, src_hi);
    let pairs_hi = _mm256_unpackhi_epi8(src_lo, src_hi);

    let result_lo = _mm256_maddubs_epi16(pairs_lo, coeffs);
    let result_hi = _mm256_maddubs_epi16(pairs_hi, coeffs);

    let sh_reg = _mm_cvtsi32_si128(4);
    let result_lo_2 = _mm256_sra_epi16(_mm256_add_epi16(result_lo, rnd), sh_reg);
    let result_hi_2 = _mm256_sra_epi16(_mm256_add_epi16(result_hi, rnd), sh_reg);

    let result_lo_3 = _mm256_max_epi16(_mm256_min_epi16(result_lo_2, max), zero);
    let result_hi_3 = _mm256_max_epi16(_mm256_min_epi16(result_hi_2, max), zero);

    let packed = _mm256_packus_epi16(result_lo_3, result_hi_3);

    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(32)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_256!(checked_value_3, packed);
  }

  for x in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(x);

    let checked_value_4 = <[_]>::get(&*src_2, column_index).ok_or(Rav1dError::EINVAL)?;
    let x0 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_4);
    let checked_value_5 =
      <[_]>::get(&*src_2, (Wrapping(column_index) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
    let x1 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_5);
    let pixel = i32::wrapping_add(
      i32::wrapping_mul(
        ::core::primitive::i32::wrapping_sub(
          16,
          <i32 as ::core::convert::From<::core::primitive::i8>>::from(mx_2),
        ),
        x0,
      ),
      i32::wrapping_mul(
        <i32 as ::core::convert::From<::core::primitive::i8>>::from(mx_2),
        x1,
      ),
    );
    let result =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(pixel, 8), 4_u32).clamp(0, 255);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_bilin_8bpc_direct_avx2_inner(
  _token: Desktop64,
  dst: &mut [u8],
  src0: &[u8],
  src1: &[u8],
  width: u32,
  my: i32,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let src0_2 = &*src0;
  let src1_2 = &*src1;
  let my_2 = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i8>(my) {
    ::core::result::Result::Ok(converted_position) => converted_position,
  };
  let coeff0 = ::core::primitive::i8::cast_unsigned(i8::wrapping_sub(16, my_2));
  let coeff1 = ::core::primitive::i8::cast_unsigned(my_2);
  let coeffs = _mm256_set1_epi16(
    ::core::primitive::i16::wrapping_shl(
      <i16 as ::core::convert::From<::core::primitive::u8>>::from(coeff1),
      8_u32,
    ) | <i16 as ::core::convert::From<::core::primitive::u8>>::from(coeff0),
  );
  let rnd = _mm256_set1_epi16(8);
  let sh_reg = _mm_cvtsi32_si128(4);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(32));

  for x in (0..vector_columns).step_by(32) {
    let Ok(column_index) = console_core_number_conversion::index(x);

    let checked_value = <[_]>::get(
      &*src0_2,
      column_index..(Wrapping(column_index) + Wrapping(32)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let s0 = loadu_256!(checked_value);
    let checked_value_2 = <[_]>::get(
      &*src1_2,
      column_index..(Wrapping(column_index) + Wrapping(32)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let s1 = loadu_256!(checked_value_2);

    let pairs_lo = _mm256_unpacklo_epi8(s0, s1);
    let pairs_hi = _mm256_unpackhi_epi8(s0, s1);

    let result_lo = _mm256_maddubs_epi16(pairs_lo, coeffs);
    let result_hi = _mm256_maddubs_epi16(pairs_hi, coeffs);

    let result_lo_2 = _mm256_sra_epi16(_mm256_add_epi16(result_lo, rnd), sh_reg);
    let result_hi_2 = _mm256_sra_epi16(_mm256_add_epi16(result_hi, rnd), sh_reg);

    let packed = _mm256_packus_epi16(result_lo_2, result_hi_2);

    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(32)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [u8; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_256!(checked_value_3, packed);
  }

  for x in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(x);

    let checked_value_4 = <[_]>::get(&*src0_2, column_index).ok_or(Rav1dError::EINVAL)?;
    let x0 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_4);
    let checked_value_5 = <[_]>::get(&*src1_2, column_index).ok_or(Rav1dError::EINVAL)?;
    let x1 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_5);
    let pixel = i32::wrapping_add(
      i32::wrapping_mul(
        ::core::primitive::i32::wrapping_sub(
          16,
          <i32 as ::core::convert::From<::core::primitive::i8>>::from(my_2),
        ),
        x0,
      ),
      i32::wrapping_mul(
        <i32 as ::core::convert::From<::core::primitive::i8>>::from(my_2),
        x1,
      ),
    );
    let result =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(pixel, 8), 4_u32).clamp(0, 255);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn h_filter_bilin_8bpc_avx512_inner(
  _token: Server64,
  dst: &mut [i16],
  src: &[u8],
  width: u32,
  mx: i32,
  sh: u8,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let src_2 = &*src;
  let mx_2 = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i8>(mx) {
    ::core::result::Result::Ok(converted_position) => converted_position,
  };
  let coeff0 = ::core::primitive::i8::cast_unsigned(i8::wrapping_sub(16, mx_2));
  let coeff1 = ::core::primitive::i8::cast_unsigned(mx_2);
  let coeffs = _mm512_set1_epi16(
    ::core::primitive::i16::wrapping_shl(
      <i16 as ::core::convert::From<::core::primitive::u8>>::from(coeff1),
      8_u32,
    ) | <i16 as ::core::convert::From<::core::primitive::u8>>::from(coeff0),
  );
  let rnd = match ::core::primitive::bool::then_some(sh > 0, ()) {
    ::core::option::Option::Some(()) => {
      _mm512_set1_epi16(i16::wrapping_shl(1, u32::from(u8::wrapping_sub(sh, 1))))
    }
    ::core::option::Option::None => _mm512_setzero_si512(),
  };
  let sh_reg = _mm_cvtsi32_si128(<i32 as ::core::convert::From<::core::primitive::u8>>::from(
    sh,
  ));

  let idx0 = _mm512_setr_epi64(0, 1, 8, 9, 2, 3, 10, 11);
  let idx1 = _mm512_setr_epi64(4, 5, 12, 13, 6, 7, 14, 15);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(64));

  for x in (0..vector_columns).step_by(64) {
    let Ok(column_index) = console_core_number_conversion::index(x);

    let checked_value = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(64)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let src_lo = loadu_512!(checked_value, [u8; 64]);
    let checked_value_2 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(1)).0..(Wrapping(column_index) + Wrapping(65)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let src_hi = loadu_512!(checked_value_2, [u8; 64]);

    let pairs_lo = _mm512_unpacklo_epi8(src_lo, src_hi);
    let pairs_hi = _mm512_unpackhi_epi8(src_lo, src_hi);

    let result_lo = _mm512_maddubs_epi16(pairs_lo, coeffs);
    let result_hi = _mm512_maddubs_epi16(pairs_hi, coeffs);

    let result_lo_2 = _mm512_sra_epi16(_mm512_add_epi16(result_lo, rnd), sh_reg);
    let result_hi_2 = _mm512_sra_epi16(_mm512_add_epi16(result_hi, rnd), sh_reg);

    let out0 = _mm512_permutex2var_epi64(result_lo_2, idx0, result_hi_2);
    let out1 = _mm512_permutex2var_epi64(result_lo_2, idx1, result_hi_2);

    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(32)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_512!(checked_value_3, out0);
    let checked_value_4 = <[_]>::get_mut(
      &mut *dst_2,
      (Wrapping(column_index) + Wrapping(32)).0..(Wrapping(column_index) + Wrapping(64)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_512!(checked_value_4, out1);
  }

  for x in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(x);

    let checked_value_5 = <[_]>::get(&*src_2, column_index).ok_or(Rav1dError::EINVAL)?;
    let x0 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_5);
    let checked_value_6 =
      <[_]>::get(&*src_2, (Wrapping(column_index) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
    let x1 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_6);
    let pixel = i32::wrapping_add(
      i32::wrapping_mul(
        ::core::primitive::i32::wrapping_sub(
          16,
          <i32 as ::core::convert::From<::core::primitive::i8>>::from(mx_2),
        ),
        x0,
      ),
      i32::wrapping_mul(
        <i32 as ::core::convert::From<::core::primitive::i8>>::from(mx_2),
        x1,
      ),
    );
    let result = match ::core::primitive::bool::then_some(sh > 0, ()) {
      ::core::option::Option::Some(()) => i32::wrapping_shr(
        i32::wrapping_add(
          pixel,
          i32::wrapping_shl(1, u32::from(u8::wrapping_sub(sh, 1))),
        ),
        u32::from(sh),
      ),
      ::core::option::Option::None => pixel,
    };
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(result) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_filter_bilin_8bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  mid: &[&[i16]],
  width: u32,
  my: i32,
  sh: u8,
  _bd_max: i16,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let my_2 = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(my) {
    ::core::result::Result::Ok(converted_position) => converted_position,
  };
  let coeff0 = i32::wrapping_sub(16, my_2);
  let coeff1 = my_2;

  let c0_32 = _mm512_set1_epi32(coeff0);
  let c1_32 = _mm512_set1_epi32(coeff1);
  let rnd = _mm512_set1_epi32(match ::core::primitive::bool::then_some(sh > 0, ()) {
    ::core::option::Option::Some(()) => i32::wrapping_shl(1, u32::from(u8::wrapping_sub(sh, 1))),
    ::core::option::Option::None => 0,
  });
  let zero = _mm512_setzero_si512();
  let max_32 = _mm512_set1_epi32(255);
  let sh_reg = _mm_cvtsi32_si128(<i32 as ::core::convert::From<::core::primitive::u8>>::from(
    sh,
  ));

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for x in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(x);

    let checked_value = <[_]>::get(
      &*(*{
        let checked_value_36 = <[_]>::get(&*mid, 0).ok_or(Rav1dError::EINVAL)?;
        checked_value_36
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let row0 = _mm512_cvtepi16_epi32(loadu_256!(checked_value));
    let checked_value_2 = <[_]>::get(
      &*(*{
        let checked_value_37 = <[_]>::get(&*mid, 1).ok_or(Rav1dError::EINVAL)?;
        checked_value_37
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let row1 = _mm512_cvtepi16_epi32(loadu_256!(checked_value_2));

    let mul0 = _mm512_mullo_epi32(row0, c0_32);
    let mul1 = _mm512_mullo_epi32(row1, c1_32);
    let sum = _mm512_add_epi32(mul0, mul1);

    let result = _mm512_sra_epi32(_mm512_add_epi32(sum, rnd), sh_reg);
    let result_2 = _mm512_max_epi32(result, zero);
    let result_3 = _mm512_min_epi32(result_2, max_32);

    let packed = _mm512_cvtusepi32_epi8(result_3);
    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [u8; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_128!(checked_value_3, packed);
  }

  for x in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(x);

    let checked_value_38 = <[_]>::get(
      &*(*{
        let checked_value_18 = <[_]>::get(&*mid, 0).ok_or(Rav1dError::EINVAL)?;
        checked_value_18
      }),
      column_index,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r0 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(*checked_value_38);
    let checked_value_39 = <[_]>::get(
      &*(*{
        let checked_value_20 = <[_]>::get(&*mid, 1).ok_or(Rav1dError::EINVAL)?;
        checked_value_20
      }),
      column_index,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r1 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(*checked_value_39);
    let pixel = i32::wrapping_add(i32::wrapping_mul(coeff0, r0), i32::wrapping_mul(coeff1, r1));
    let result = match ::core::primitive::bool::then_some(sh > 0, ()) {
      ::core::option::Option::Some(()) => i32::wrapping_shr(
        i32::wrapping_add(
          pixel,
          i32::wrapping_shl(1, u32::from(u8::wrapping_sub(sh, 1))),
        ),
        u32::from(sh),
      )
      .clamp(0, 255),
      ::core::option::Option::None => pixel.clamp(0, 255),
    };
    *{
      let checked_value_21 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_21
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_filter_bilin_8bpc_prep_avx512_inner(
  _token: Server64,
  dst: &mut [i16],
  mid: &[&[i16]],
  width: u32,
  my: i32,
  sh: u8,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let my_2 = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(my) {
    ::core::result::Result::Ok(converted_position) => converted_position,
  };
  let coeff0 = i32::wrapping_sub(16, my_2);
  let coeff1 = my_2;

  let c0_32 = _mm512_set1_epi32(coeff0);
  let c1_32 = _mm512_set1_epi32(coeff1);
  let rnd = _mm512_set1_epi32(match ::core::primitive::bool::then_some(sh > 0, ()) {
    ::core::option::Option::Some(()) => i32::wrapping_shl(1, u32::from(u8::wrapping_sub(sh, 1))),
    ::core::option::Option::None => 0,
  });
  let sh_reg = _mm_cvtsi32_si128(<i32 as ::core::convert::From<::core::primitive::u8>>::from(
    sh,
  ));

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for x in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(x);

    let checked_value = <[_]>::get(
      &*(*{
        let checked_value_36 = <[_]>::get(&*mid, 0).ok_or(Rav1dError::EINVAL)?;
        checked_value_36
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let row0 = _mm512_cvtepi16_epi32(loadu_256!(checked_value));
    let checked_value_2 = <[_]>::get(
      &*(*{
        let checked_value_37 = <[_]>::get(&*mid, 1).ok_or(Rav1dError::EINVAL)?;
        checked_value_37
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let row1 = _mm512_cvtepi16_epi32(loadu_256!(checked_value_2));

    let mul0 = _mm512_mullo_epi32(row0, c0_32);
    let mul1 = _mm512_mullo_epi32(row1, c1_32);
    let sum = _mm512_add_epi32(mul0, mul1);

    let result = _mm512_sra_epi32(_mm512_add_epi32(sum, rnd), sh_reg);

    let packed = _mm512_cvtsepi32_epi16(result);
    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_256!(checked_value_3, packed);
  }

  for x in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(x);

    let checked_value_38 = <[_]>::get(
      &*(*{
        let checked_value_18 = <[_]>::get(&*mid, 0).ok_or(Rav1dError::EINVAL)?;
        checked_value_18
      }),
      column_index,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r0 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(*checked_value_38);
    let checked_value_39 = <[_]>::get(
      &*(*{
        let checked_value_20 = <[_]>::get(&*mid, 1).ok_or(Rav1dError::EINVAL)?;
        checked_value_20
      }),
      column_index,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r1 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(*checked_value_39);
    let pixel = i32::wrapping_add(i32::wrapping_mul(coeff0, r0), i32::wrapping_mul(coeff1, r1));
    let result = match ::core::primitive::bool::then_some(sh > 0, ()) {
      ::core::option::Option::Some(()) => i32::wrapping_shr(
        i32::wrapping_add(
          pixel,
          i32::wrapping_shl(1, u32::from(u8::wrapping_sub(sh, 1))),
        ),
        u32::from(sh),
      ),
      ::core::option::Option::None => pixel,
    };
    *{
      let checked_value_21 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_21
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(result) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn h_bilin_8bpc_put_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  src: &[u8],
  width: u32,
  mx: i32,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let src_2 = &*src;
  let mx_2 = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i8>(mx) {
    ::core::result::Result::Ok(converted_position) => converted_position,
  };
  let coeff0 = ::core::primitive::i8::cast_unsigned(i8::wrapping_sub(16, mx_2));
  let coeff1 = ::core::primitive::i8::cast_unsigned(mx_2);
  let coeffs = _mm512_set1_epi16(
    ::core::primitive::i16::wrapping_shl(
      <i16 as ::core::convert::From<::core::primitive::u8>>::from(coeff1),
      8_u32,
    ) | <i16 as ::core::convert::From<::core::primitive::u8>>::from(coeff0),
  );
  let rnd = _mm512_set1_epi16(8);
  let sh_reg = _mm_cvtsi32_si128(4);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(64));

  for x in (0..vector_columns).step_by(64) {
    let Ok(column_index) = console_core_number_conversion::index(x);

    let checked_value = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(64)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let src_lo = loadu_512!(checked_value, [u8; 64]);
    let checked_value_2 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(1)).0..(Wrapping(column_index) + Wrapping(65)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let src_hi = loadu_512!(checked_value_2, [u8; 64]);

    let pairs_lo = _mm512_unpacklo_epi8(src_lo, src_hi);
    let pairs_hi = _mm512_unpackhi_epi8(src_lo, src_hi);

    let result_lo = _mm512_maddubs_epi16(pairs_lo, coeffs);
    let result_hi = _mm512_maddubs_epi16(pairs_hi, coeffs);

    let result_lo_2 = _mm512_sra_epi16(_mm512_add_epi16(result_lo, rnd), sh_reg);
    let result_hi_2 = _mm512_sra_epi16(_mm512_add_epi16(result_hi, rnd), sh_reg);

    let packed = _mm512_packus_epi16(result_lo_2, result_hi_2);
    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(64)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    storeu_512!(checked_value_3, [u8; 64], packed);
  }

  for x in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(x);

    let checked_value_4 = <[_]>::get(&*src_2, column_index).ok_or(Rav1dError::EINVAL)?;
    let x0 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_4);
    let checked_value_5 =
      <[_]>::get(&*src_2, (Wrapping(column_index) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
    let x1 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_5);
    let pixel = i32::wrapping_add(
      i32::wrapping_mul(
        ::core::primitive::i32::wrapping_sub(
          16,
          <i32 as ::core::convert::From<::core::primitive::i8>>::from(mx_2),
        ),
        x0,
      ),
      i32::wrapping_mul(
        <i32 as ::core::convert::From<::core::primitive::i8>>::from(mx_2),
        x1,
      ),
    );
    let result =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(pixel, 8), 4_u32).clamp(0, 255);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_bilin_8bpc_direct_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  src0: &[u8],
  src1: &[u8],
  width: u32,
  my: i32,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let src0_2 = &*src0;
  let src1_2 = &*src1;
  let my_2 = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i8>(my) {
    ::core::result::Result::Ok(converted_position) => converted_position,
  };
  let coeff0 = ::core::primitive::i8::cast_unsigned(i8::wrapping_sub(16, my_2));
  let coeff1 = ::core::primitive::i8::cast_unsigned(my_2);
  let coeffs = _mm512_set1_epi16(
    ::core::primitive::i16::wrapping_shl(
      <i16 as ::core::convert::From<::core::primitive::u8>>::from(coeff1),
      8_u32,
    ) | <i16 as ::core::convert::From<::core::primitive::u8>>::from(coeff0),
  );
  let rnd = _mm512_set1_epi16(8);
  let sh_reg = _mm_cvtsi32_si128(4);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(64));

  for x in (0..vector_columns).step_by(64) {
    let Ok(column_index) = console_core_number_conversion::index(x);

    let checked_value = <[_]>::get(
      &*src0_2,
      column_index..(Wrapping(column_index) + Wrapping(64)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let s0 = loadu_512!(checked_value, [u8; 64]);
    let checked_value_2 = <[_]>::get(
      &*src1_2,
      column_index..(Wrapping(column_index) + Wrapping(64)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let s1 = loadu_512!(checked_value_2, [u8; 64]);

    let pairs_lo = _mm512_unpacklo_epi8(s0, s1);
    let pairs_hi = _mm512_unpackhi_epi8(s0, s1);

    let result_lo = _mm512_maddubs_epi16(pairs_lo, coeffs);
    let result_hi = _mm512_maddubs_epi16(pairs_hi, coeffs);

    let result_lo_2 = _mm512_sra_epi16(_mm512_add_epi16(result_lo, rnd), sh_reg);
    let result_hi_2 = _mm512_sra_epi16(_mm512_add_epi16(result_hi, rnd), sh_reg);

    let packed = _mm512_packus_epi16(result_lo_2, result_hi_2);
    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(64)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    storeu_512!(checked_value_3, [u8; 64], packed);
  }

  for x in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(x);

    let checked_value_4 = <[_]>::get(&*src0_2, column_index).ok_or(Rav1dError::EINVAL)?;
    let x0 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_4);
    let checked_value_5 = <[_]>::get(&*src1_2, column_index).ok_or(Rav1dError::EINVAL)?;
    let x1 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_5);
    let pixel = i32::wrapping_add(
      i32::wrapping_mul(
        ::core::primitive::i32::wrapping_sub(
          16,
          <i32 as ::core::convert::From<::core::primitive::i8>>::from(my_2),
        ),
        x0,
      ),
      i32::wrapping_mul(
        <i32 as ::core::convert::From<::core::primitive::i8>>::from(my_2),
        x1,
      ),
    );
    let result =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(pixel, 8), 4_u32).clamp(0, 255);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_bilin_8bpc_prep_direct_avx512_inner(
  _token: Server64,
  dst: &mut [i16],
  src0: &[u8],
  src1: &[u8],
  width: u32,
  my: i32,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let src0_2 = &*src0;
  let src1_2 = &*src1;
  let my_2 = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i8>(my) {
    ::core::result::Result::Ok(converted_position) => converted_position,
  };
  let coeff0 = ::core::primitive::i8::cast_unsigned(i8::wrapping_sub(16, my_2));
  let coeff1 = ::core::primitive::i8::cast_unsigned(my_2);
  let coeffs = _mm512_set1_epi16(
    ::core::primitive::i16::wrapping_shl(
      <i16 as ::core::convert::From<::core::primitive::u8>>::from(coeff1),
      8_u32,
    ) | <i16 as ::core::convert::From<::core::primitive::u8>>::from(coeff0),
  );

  let idx0 = _mm512_setr_epi64(0, 1, 8, 9, 2, 3, 10, 11);
  let idx1 = _mm512_setr_epi64(4, 5, 12, 13, 6, 7, 14, 15);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(64));

  for x in (0..vector_columns).step_by(64) {
    let Ok(column_index) = console_core_number_conversion::index(x);

    let checked_value = <[_]>::get(
      &*src0_2,
      column_index..(Wrapping(column_index) + Wrapping(64)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let s0 = loadu_512!(checked_value, [u8; 64]);
    let checked_value_2 = <[_]>::get(
      &*src1_2,
      column_index..(Wrapping(column_index) + Wrapping(64)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let s1 = loadu_512!(checked_value_2, [u8; 64]);

    let pairs_lo = _mm512_unpacklo_epi8(s0, s1);
    let pairs_hi = _mm512_unpackhi_epi8(s0, s1);

    let result_lo = _mm512_maddubs_epi16(pairs_lo, coeffs);
    let result_hi = _mm512_maddubs_epi16(pairs_hi, coeffs);

    let out0 = _mm512_permutex2var_epi64(result_lo, idx0, result_hi);
    let out1 = _mm512_permutex2var_epi64(result_lo, idx1, result_hi);

    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(32)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_512!(checked_value_3, out0);
    let checked_value_4 = <[_]>::get_mut(
      &mut *dst_2,
      (Wrapping(column_index) + Wrapping(32)).0..(Wrapping(column_index) + Wrapping(64)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 32]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_512!(checked_value_4, out1);
  }

  for x in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(x);

    let checked_value_5 = <[_]>::get(&*src0_2, column_index).ok_or(Rav1dError::EINVAL)?;
    let x0 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_5);
    let checked_value_6 = <[_]>::get(&*src1_2, column_index).ok_or(Rav1dError::EINVAL)?;
    let x1 = <i32 as ::core::convert::From<::core::primitive::u8>>::from(*checked_value_6);
    let pixel = i32::wrapping_add(
      i32::wrapping_mul(
        ::core::primitive::i32::wrapping_sub(
          16,
          <i32 as ::core::convert::From<::core::primitive::i8>>::from(my_2),
        ),
        x0,
      ),
      i32::wrapping_mul(
        <i32 as ::core::convert::From<::core::primitive::i8>>::from(my_2),
        x1,
      ),
    );
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(pixel) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
use crate::src::internal::SEG_MASK_LEN;

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
pub(super) struct WeightedMaskMode<BD> {
  pub(super) layout: Rav1dPixelLayoutSubSampled,
  pub(super) sign: u8,
  pub(super) depth: BD,
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
struct WeightedMaskRounding {
  pixel_shift: u32,
  pixel_rounding: i32,
  weight_shift: u32,
  weight_rounding: u16,
  maximum: i32,
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
impl WeightedMaskRounding {
  fn new<BD: BitDepth>(depth: BD) -> Result<Self, Never> {
    use crate::include::common::bitdepth::BPC;

    let maximum = depth.into_c();
    let (bitdepth, intermediate_bits, predictor_bias) = match BD::BPC {
      BPC::BPC8 => (8_u32, 4_u32, 0_i32),
      BPC::BPC16 => match maximum {
        1023 => (10, 4, 8192),
        _ => (12, 2, 8192),
      },
    };
    let pixel_shift = intermediate_bits.wrapping_add(6);
    let pixel_rounding = 32_i32
      .wrapping_shl(intermediate_bits)
      .wrapping_add(predictor_bias.wrapping_mul(64));
    let weight_shift = bitdepth.wrapping_add(intermediate_bits).wrapping_sub(4);
    let weight_rounding = 1_u16.wrapping_shl(weight_shift.wrapping_sub(5));
    Ok(Self {
      pixel_shift,
      pixel_rounding,
      weight_shift,
      weight_rounding,
      maximum,
    })
  }

  #[inline(always)]
  fn sample<Pixel: TryFrom<i32>>(
    &self,
    output: &mut Pixel,
    [first, second]: [i16; 2],
  ) -> Rav1dResult<u8> {
    let difference = first.abs_diff(second);
    let weight = 38_u16
      .wrapping_add(
        difference
          .saturating_add(self.weight_rounding)
          .wrapping_shr(self.weight_shift),
      )
      .min(64);
    let second_weight = 64_i32.wrapping_sub(i32::from(weight));
    let value = i32::from(first)
      .wrapping_mul(i32::from(weight))
      .wrapping_add(i32::from(second).wrapping_mul(second_weight))
      .wrapping_add(self.pixel_rounding)
      .wrapping_shr(self.pixel_shift);
    let clipped =
      Pixel::try_from(value.clamp(0, self.maximum)).map_err(|_| Rav1dError::EINVAL)?;
    *output = clipped;
    let Ok(mask) = console_core_number_conversion::wrapped(weight);
    Ok(mask)
  }
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
enum WeightedMaskRow {
  Full,
  Horizontal { sign: u8 },
  VerticalFirst,
  VerticalSecond { sign: u8 },
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn write_weighted_mask_row<Pixel: TryFrom<i32>>(
  mut row: blend_rows::BlendRow<'_, Pixel>,
  mask: &mut [u8],
  operation: WeightedMaskRow,
  rounding: &WeightedMaskRounding,
) -> Rav1dResult {
  match operation {
    WeightedMaskRow::Full => {
      let Ok(pixels) = row.pixels();

      for ((destination, first, second), output) in pixels.zip(mask) {
        let weight = rounding.sample(destination, [*first, *second])?;
        *output = weight;
      }
    }
    WeightedMaskRow::Horizontal { .. }
    | WeightedMaskRow::VerticalFirst
    | WeightedMaskRow::VerticalSecond { .. } => {
      let Ok(pairs) = row.blocks::<2>();

      for (([left, right], &[left_first, right_first], &[left_second, right_second]), output) in
        pairs.zip(mask)
      {
        let left_weight = rounding.sample(left, [left_first, left_second])?;
        let right_weight = rounding.sample(right, [right_first, right_second])?;
        let sum = u16::from(left_weight).wrapping_add(u16::from(right_weight));
        let weight = match operation {
          WeightedMaskRow::Horizontal { sign } => sum
            .wrapping_add(1)
            .wrapping_sub(u16::from(sign))
            .wrapping_shr(1),
          WeightedMaskRow::VerticalFirst => sum,
          WeightedMaskRow::VerticalSecond { sign } => sum
            .wrapping_add(2)
            .wrapping_sub(u16::from(sign))
            .wrapping_add(u16::from(*output))
            .wrapping_shr(2),
          WeightedMaskRow::Full => return Err(Rav1dError::EINVAL),
        };
        let Ok(value) = console_core_number_conversion::wrapped(weight);
        *output = value;
      }
    }
  }

  Ok(())
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
pub(super) fn write_weighted_mask<BD: BitDepth>(
  destination: &mut [u8],
  stride: u64,
  predictors: [&[i16]; 2],
  extent: Size<i32>,
  mask: &mut [u8],
  mode: WeightedMaskMode<BD>,
) -> Rav1dResult {
  use Rav1dPixelLayoutSubSampled::{I420, I422, I444};
  use console_core_number_conversion::indices;

  let WeightedMaskMode {
    layout,
    sign,
    depth,
  } = mode;
  let shape = BlendSize::new(extent).map_err(|_| Rav1dError::EINVAL)?;
  let width = u64::from(extent.width.cast_unsigned());
  let height = u64::from(extent.height.cast_unsigned());
  let mask_extent = match (layout, width & 1) {
    (I444, _) => Size { width, height },
    (I422, 0) => Size {
      width: width.wrapping_shr(1),
      height,
    },
    (I420, 0) => Size {
      width: width.wrapping_shr(1),
      height: height.div_ceil(2),
    },
    (I420 | I422, _) => return Err(Rav1dError::EINVAL),
  };
  let mask_layout =
    console_core_pixel_buffers::RowLayout::new(mask_extent, 0, mask_extent.width.cast_signed())
      .map_err(|_| Rav1dError::EINVAL)?;
  mask_layout
    .validate_len(mask.len())
    .map_err(|_| Rav1dError::EINVAL)?;
  let rows = shape
    .blend_rows::<BD::Pixel>(predictors, destination, stride)
    .map_err(|_| Rav1dError::EINVAL)?;
  let Ok(rounding) = WeightedMaskRounding::new(depth);

  for (ordinal, row) in (0..height).zip(rows) {
    let (index, operation) = match (layout, ordinal & 1) {
      (I444, _) => (ordinal, WeightedMaskRow::Full),
      (I422, _) => (ordinal, WeightedMaskRow::Horizontal { sign }),
      (I420, 0) => (ordinal.wrapping_shr(1), WeightedMaskRow::VerticalFirst),
      (I420, _) => (
        ordinal.wrapping_shr(1),
        WeightedMaskRow::VerticalSecond { sign },
      ),
    };
    let start = index.wrapping_mul(mask_extent.width);
    let Ok(range) = indices(start..start.wrapping_add(mask_extent.width));
    let mask_row = mask.get_mut(range).ok_or(Rav1dError::EINVAL)?;
    write_weighted_mask_row(row, mask_row, operation, &rounding)?;
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn h_bilin_16bpc_avx2_inner(
  _token: Desktop64,
  dst: &mut [i32],
  src: &[u16],
  width: u32,
  mx: i32,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let src_2 = &*src;

  let w0 = _mm256_set1_epi32(i32::wrapping_sub(16, mx));
  let w1 = _mm256_set1_epi32(mx);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

  for col in (0..vector_columns).step_by(8) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p0_7 = loadu_128!(checked_value);
    let checked_value_2 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(1)).0..(Wrapping(column_index) + Wrapping(9)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p1_8 = loadu_128!(checked_value_2);

    let p0_lo = _mm256_cvtepu16_epi32(p0_7);
    let p1_lo = _mm256_cvtepu16_epi32(p1_8);

    let term0 = _mm256_mullo_epi32(p0_lo, w0);
    let term1 = _mm256_mullo_epi32(p1_lo, w1);
    let result = _mm256_add_epi32(term0, term1);

    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_256!(checked_value_3, result);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_4 = <[_]>::get(&*src_2, column_index).ok_or(Rav1dError::EINVAL)?;
    let x0 = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_4);
    let checked_value_5 =
      <[_]>::get(&*src_2, (Wrapping(column_index) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
    let x1 = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_5);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = i32::wrapping_add(
      ::core::primitive::i32::wrapping_mul(16, x0),
      i32::wrapping_mul(mx, i32::wrapping_sub(x1, x0)),
    );
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_bilin_16bpc_avx2_inner(
  _token: Desktop64,
  dst: &mut [u16],
  mid: &[[i32; MID_STRIDE]],
  row: MotionRow,
  phase: WordBilinearPhase,
  max: i32,
) -> Rav1dResult {
  let MotionRow { width, ordinal } = row;
  let Ok(y) = console_core_number_conversion::index(ordinal);
  let WordBilinearPhase(my) = phase;
  let sh = WORD_BILINEAR_BUFFERED_SHIFT;
  let dst_2 = &mut *dst;
  let w0 = _mm256_set1_epi32(i32::wrapping_sub(16, my));
  let w1 = _mm256_set1_epi32(my);
  let rnd = _mm256_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1, i32::cast_unsigned(sh)),
    1_u32,
  ));
  let shift_count = _mm_cvtsi32_si128(sh);
  let zero = _mm256_setzero_si256();
  let max_val = _mm256_set1_epi32(max);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

  for col in (0..vector_columns).step_by(8) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      &(*{
        let checked_value_36 = <[_]>::get(&*mid, y).ok_or(Rav1dError::EINVAL)?;
        checked_value_36
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let r0 = loadu_256!(checked_value);
    let checked_value_2 = <[_]>::get(
      &(*{
        let checked_value_37 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_37
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let r1 = loadu_256!(checked_value_2);

    let term0 = _mm256_mullo_epi32(r0, w0);
    let term1 = _mm256_mullo_epi32(r1, w1);
    let sum = _mm256_add_epi32(term0, term1);

    let shifted = _mm256_sra_epi32(_mm256_add_epi32(sum, rnd), shift_count);
    let clamped = _mm256_min_epi32(_mm256_max_epi32(shifted, zero), max_val);

    let packed = _mm256_packus_epi32(clamped, zero);
    let result = _mm256_permute4x64_epi64(packed, 0b00_00_10_00);

    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_128!(checked_value_3, _mm256_castsi256_si128(result));
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_38 = <[_]>::get(
      &(*{
        let checked_value_18 = <[_]>::get(&*mid, y).ok_or(Rav1dError::EINVAL)?;
        checked_value_18
      }),
      column_index,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r0 = *checked_value_38;
    let checked_value_39 = <[_]>::get(
      &(*{
        let checked_value_20 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_20
      }),
      column_index,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r1 = *checked_value_39;
    let pixel = i32::wrapping_add(
      i32::wrapping_mul(16, r0),
      i32::wrapping_mul(my, i32::wrapping_sub(r1, r0)),
    );
    let r =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_shl(1, i32::cast_unsigned(sh)), 1_u32);
    let output_sample =
      i32::wrapping_shr(i32::wrapping_add(pixel, r), i32::cast_unsigned(sh)).clamp(0, max);
    *{
      let checked_value_21 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_21
    } =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(output_sample)
      {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_bilin_16bpc_prep_avx2_inner(
  _token: Desktop64,
  dst: &mut [i16],
  mid: &[[i32; MID_STRIDE]],
  row: MotionRow,
  phase: WordBilinearPhase,
  prep_bias: i32,
) -> Rav1dResult {
  let MotionRow { width, ordinal } = row;
  let Ok(y) = console_core_number_conversion::index(ordinal);
  let WordBilinearPhase(my) = phase;
  let sh = WORD_BILINEAR_BUFFERED_SHIFT;
  let dst_2 = &mut *dst;
  let w0 = _mm256_set1_epi32(i32::wrapping_sub(16, my));
  let w1 = _mm256_set1_epi32(my);
  let rnd = _mm256_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1, i32::cast_unsigned(sh)),
    1_u32,
  ));
  let shift_count = _mm_cvtsi32_si128(sh);
  let bias = _mm256_set1_epi32(prep_bias);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

  for col in (0..vector_columns).step_by(8) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      &(*{
        let checked_value_36 = <[_]>::get(&*mid, y).ok_or(Rav1dError::EINVAL)?;
        checked_value_36
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let r0 = loadu_256!(checked_value);
    let checked_value_2 = <[_]>::get(
      &(*{
        let checked_value_37 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_37
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[i32; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let r1 = loadu_256!(checked_value_2);

    let term0 = _mm256_mullo_epi32(r0, w0);
    let term1 = _mm256_mullo_epi32(r1, w1);
    let sum = _mm256_add_epi32(term0, term1);

    let shifted = _mm256_sra_epi32(_mm256_add_epi32(sum, rnd), shift_count);
    let biased = _mm256_sub_epi32(shifted, bias);

    let packed = _mm256_packs_epi32(biased, biased);
    let result = _mm256_permute4x64_epi64(packed, 0b00_00_10_00);

    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_128!(checked_value_3, _mm256_castsi256_si128(result));
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_38 = <[_]>::get(
      &(*{
        let checked_value_18 = <[_]>::get(&*mid, y).ok_or(Rav1dError::EINVAL)?;
        checked_value_18
      }),
      column_index,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r0 = *checked_value_38;
    let checked_value_39 = <[_]>::get(
      &(*{
        let checked_value_20 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_20
      }),
      column_index,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r1 = *checked_value_39;
    let pixel = i32::wrapping_add(
      i32::wrapping_mul(16, r0),
      i32::wrapping_mul(my, i32::wrapping_sub(r1, r0)),
    );
    let r =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_shl(1, i32::cast_unsigned(sh)), 1_u32);
    let output_sample = i32::wrapping_sub(
      i32::wrapping_shr(i32::wrapping_add(pixel, r), i32::cast_unsigned(sh)),
      prep_bias,
    );
    *{
      let checked_value_21 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_21
    } =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(output_sample)
      {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn h_bilin_16bpc_put_avx2_inner(
  _token: Desktop64,
  dst: &mut [u16],
  src: &[u16],
  width: u32,
  phase: WordBilinearPhase,
  bd_max: i32,
) -> Rav1dResult {
  let WordBilinearPhase(mx) = phase;
  let dst_2 = &mut *dst;
  let src_2 = &*src;
  let w0 = _mm256_set1_epi32(i32::wrapping_sub(16, mx));
  let w1 = _mm256_set1_epi32(mx);
  let rnd = _mm256_set1_epi32(8);
  let shift_count = _mm_cvtsi32_si128(4);
  let zero = _mm256_setzero_si256();
  let max_val = _mm256_set1_epi32(bd_max);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

  for col in (0..vector_columns).step_by(8) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p0 = _mm256_cvtepu16_epi32(loadu_128!(checked_value));
    let checked_value_2 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(1)).0..(Wrapping(column_index) + Wrapping(9)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p1 = _mm256_cvtepu16_epi32(loadu_128!(checked_value_2));

    let term0 = _mm256_mullo_epi32(p0, w0);
    let term1 = _mm256_mullo_epi32(p1, w1);
    let sum = _mm256_add_epi32(term0, term1);

    let shifted = _mm256_sra_epi32(_mm256_add_epi32(sum, rnd), shift_count);
    let clamped = _mm256_min_epi32(_mm256_max_epi32(shifted, zero), max_val);

    let packed = _mm256_packus_epi32(clamped, zero);
    let result = _mm256_permute4x64_epi64(packed, 0b00_00_10_00);

    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_128!(checked_value_3, _mm256_castsi256_si128(result));
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_4 = <[_]>::get(&*src_2, column_index).ok_or(Rav1dError::EINVAL)?;
    let x0 = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_4);
    let checked_value_5 =
      <[_]>::get(&*src_2, (Wrapping(column_index) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
    let x1 = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_5);
    let pixel = i32::wrapping_add(
      i32::wrapping_mul(i32::wrapping_sub(16, mx), x0),
      i32::wrapping_mul(mx, x1),
    );
    let result =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(pixel, 8), 4_u32).clamp(0, bd_max);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(result) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_bilin_16bpc_direct_avx2_inner(
  _token: Desktop64,
  dst: &mut [u16],
  input: BilinearInputRows<'_, u16>,
  width: u32,
  phase: WordBilinearPhase,
  bd_max: i32,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let BilinearInputRows { first, next } = input;
  let WordBilinearPhase(my) = phase;
  let w0 = _mm256_set1_epi32(i32::wrapping_sub(16, my));
  let w1 = _mm256_set1_epi32(my);
  let rnd = _mm256_set1_epi32(8);
  let shift_count = _mm_cvtsi32_si128(4);
  let zero = _mm256_setzero_si256();
  let max_val = _mm256_set1_epi32(bd_max);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

  for col in (0..vector_columns).step_by(8) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      first,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p0 = _mm256_cvtepu16_epi32(loadu_128!(checked_value));
    let checked_value_2 = <[_]>::get(next, column_index..column_index.wrapping_add(8))
      .ok_or(crate::src::error::Rav1dError::EINVAL)
      .and_then(|_explicit_array_source| {
        <&[u16; 8]>::try_from(_explicit_array_source)
          .map_err(|_| crate::src::error::Rav1dError::EINVAL)
      })?;
    let p1 = _mm256_cvtepu16_epi32(loadu_128!(checked_value_2));

    let term0 = _mm256_mullo_epi32(p0, w0);
    let term1 = _mm256_mullo_epi32(p1, w1);
    let sum = _mm256_add_epi32(term0, term1);

    let shifted = _mm256_sra_epi32(_mm256_add_epi32(sum, rnd), shift_count);
    let clamped = _mm256_min_epi32(_mm256_max_epi32(shifted, zero), max_val);

    let packed = _mm256_packus_epi32(clamped, zero);
    let result = _mm256_permute4x64_epi64(packed, 0b00_00_10_00);

    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_128!(checked_value_3, _mm256_castsi256_si128(result));
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_4 = <[_]>::get(first, column_index).ok_or(Rav1dError::EINVAL)?;
    let x0 = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_4);
    let checked_value_5 = <[_]>::get(next, column_index).ok_or(Rav1dError::EINVAL)?;
    let x1 = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_5);
    let pixel = i32::wrapping_add(
      i32::wrapping_mul(i32::wrapping_sub(16, my), x0),
      i32::wrapping_mul(my, x1),
    );
    let result =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(pixel, 8), 4_u32).clamp(0, bd_max);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(result) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn h_bilin_16bpc_prep_direct_avx2_inner(
  _token: Desktop64,
  dst: &mut [i16],
  src: &[u16],
  width: u32,
  phase: WordBilinearPhase,
  prep_bias: i32,
) -> Rav1dResult {
  let WordBilinearPhase(mx) = phase;
  let dst_2 = &mut *dst;
  let src_2 = &*src;
  let w0 = _mm256_set1_epi32(i32::wrapping_sub(16, mx));
  let w1 = _mm256_set1_epi32(mx);
  let rnd = _mm256_set1_epi32(8);
  let shift_count = _mm_cvtsi32_si128(4);
  let bias = _mm256_set1_epi32(prep_bias);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

  for col in (0..vector_columns).step_by(8) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p0 = _mm256_cvtepu16_epi32(loadu_128!(checked_value));
    let checked_value_2 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(1)).0..(Wrapping(column_index) + Wrapping(9)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p1 = _mm256_cvtepu16_epi32(loadu_128!(checked_value_2));

    let term0 = _mm256_mullo_epi32(p0, w0);
    let term1 = _mm256_mullo_epi32(p1, w1);
    let sum = _mm256_add_epi32(term0, term1);

    let shifted = _mm256_sra_epi32(_mm256_add_epi32(sum, rnd), shift_count);
    let biased = _mm256_sub_epi32(shifted, bias);

    let packed = _mm256_packs_epi32(biased, biased);
    let result = _mm256_permute4x64_epi64(packed, 0b00_00_10_00);

    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_128!(checked_value_3, _mm256_castsi256_si128(result));
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_4 = <[_]>::get(&*src_2, column_index).ok_or(Rav1dError::EINVAL)?;
    let x0 = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_4);
    let checked_value_5 =
      <[_]>::get(&*src_2, (Wrapping(column_index) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
    let x1 = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_5);
    let pixel = i32::wrapping_add(
      i32::wrapping_mul(i32::wrapping_sub(16, mx), x0),
      i32::wrapping_mul(mx, x1),
    );
    let result = i32::wrapping_sub(
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(pixel, 8), 4_u32),
      prep_bias,
    );
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(result) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_bilin_16bpc_prep_direct_avx2_inner(
  _token: Desktop64,
  dst: &mut [i16],
  input: BilinearInputRows<'_, u16>,
  width: u32,
  phase: WordBilinearPhase,
  prep_bias: i32,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let BilinearInputRows { first, next } = input;
  let WordBilinearPhase(my) = phase;
  let w0 = _mm256_set1_epi32(i32::wrapping_sub(16, my));
  let w1 = _mm256_set1_epi32(my);
  let rnd = _mm256_set1_epi32(8);
  let shift_count = _mm_cvtsi32_si128(4);
  let bias = _mm256_set1_epi32(prep_bias);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

  for col in (0..vector_columns).step_by(8) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      first,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p0 = _mm256_cvtepu16_epi32(loadu_128!(checked_value));
    let checked_value_2 = <[_]>::get(next, column_index..column_index.wrapping_add(8))
      .ok_or(crate::src::error::Rav1dError::EINVAL)
      .and_then(|_explicit_array_source| {
        <&[u16; 8]>::try_from(_explicit_array_source)
          .map_err(|_| crate::src::error::Rav1dError::EINVAL)
      })?;
    let p1 = _mm256_cvtepu16_epi32(loadu_128!(checked_value_2));

    let term0 = _mm256_mullo_epi32(p0, w0);
    let term1 = _mm256_mullo_epi32(p1, w1);
    let sum = _mm256_add_epi32(term0, term1);

    let shifted = _mm256_sra_epi32(_mm256_add_epi32(sum, rnd), shift_count);
    let biased = _mm256_sub_epi32(shifted, bias);

    let packed = _mm256_packs_epi32(biased, biased);
    let result = _mm256_permute4x64_epi64(packed, 0b00_00_10_00);

    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 8]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_128!(checked_value_3, _mm256_castsi256_si128(result));
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_4 = <[_]>::get(first, column_index).ok_or(Rav1dError::EINVAL)?;
    let x0 = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_4);
    let checked_value_5 = <[_]>::get(next, column_index).ok_or(Rav1dError::EINVAL)?;
    let x1 = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_5);
    let pixel = i32::wrapping_add(
      i32::wrapping_mul(i32::wrapping_sub(16, my), x0),
      i32::wrapping_mul(my, x1),
    );
    let result = i32::wrapping_sub(
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(pixel, 8), 4_u32),
      prep_bias,
    );
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(result) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[cfg(target_arch = "x86_64")]
#[cfg(target_arch = "x86_64")]
#[rite]
fn h_bilin_16bpc_avx512_inner(
  _token: Server64,
  dst: &mut [i32],
  src: &[u16],
  width: u32,
  mx: i32,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let src_2 = &*src;
  let w0 = _mm512_set1_epi32(i32::wrapping_sub(16, mx));
  let w1 = _mm512_set1_epi32(mx);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p0 = _mm512_cvtepu16_epi32(loadu_256!(checked_value));
    let checked_value_2 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(1)).0..(Wrapping(column_index) + Wrapping(17)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p1 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_2));

    let term0 = _mm512_mullo_epi32(p0, w0);
    let term1 = _mm512_mullo_epi32(p1, w1);
    let result = _mm512_add_epi32(term0, term1);

    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    storeu_512!(checked_value_3, [i32; 16], result);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_4 = <[_]>::get(&*src_2, column_index).ok_or(Rav1dError::EINVAL)?;
    let x0 = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_4);
    let checked_value_5 =
      <[_]>::get(&*src_2, (Wrapping(column_index) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
    let x1 = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_5);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = i32::wrapping_add(
      ::core::primitive::i32::wrapping_mul(16, x0),
      i32::wrapping_mul(mx, i32::wrapping_sub(x1, x0)),
    );
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_bilin_16bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u16],
  mid: &[[i32; MID_STRIDE]],
  row: MotionRow,
  phase: WordBilinearPhase,
  max: i32,
) -> Rav1dResult {
  let MotionRow { width, ordinal } = row;
  let Ok(y) = console_core_number_conversion::index(ordinal);
  let WordBilinearPhase(my) = phase;
  let sh = WORD_BILINEAR_BUFFERED_SHIFT;
  let dst_2 = &mut *dst;
  let w0 = _mm512_set1_epi32(i32::wrapping_sub(16, my));
  let w1 = _mm512_set1_epi32(my);
  let rnd = _mm512_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1, i32::cast_unsigned(sh)),
    1_u32,
  ));
  let shift_count = _mm_cvtsi32_si128(sh);
  let zero = _mm512_setzero_si512();
  let max_val = _mm512_set1_epi32(max);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      &(*{
        let checked_value_36 = <[_]>::get(&*mid, y).ok_or(Rav1dError::EINVAL)?;
        checked_value_36
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r0 = loadu_512!(checked_value, [i32; 16]);
    let checked_value_2 = <[_]>::get(
      &(*{
        let checked_value_37 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_37
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r1 = loadu_512!(checked_value_2, [i32; 16]);

    let term0 = _mm512_mullo_epi32(r0, w0);
    let term1 = _mm512_mullo_epi32(r1, w1);
    let sum = _mm512_add_epi32(term0, term1);

    let shifted = _mm512_sra_epi32(_mm512_add_epi32(sum, rnd), shift_count);
    let clamped = _mm512_min_epi32(_mm512_max_epi32(shifted, zero), max_val);

    let packed = _mm512_cvtusepi32_epi16(clamped);
    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_256!(checked_value_3, packed);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_38 = <[_]>::get(
      &(*{
        let checked_value_18 = <[_]>::get(&*mid, y).ok_or(Rav1dError::EINVAL)?;
        checked_value_18
      }),
      column_index,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r0 = *checked_value_38;
    let checked_value_39 = <[_]>::get(
      &(*{
        let checked_value_20 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_20
      }),
      column_index,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r1 = *checked_value_39;
    let pixel = i32::wrapping_add(
      i32::wrapping_mul(16, r0),
      i32::wrapping_mul(my, i32::wrapping_sub(r1, r0)),
    );
    let r =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_shl(1, i32::cast_unsigned(sh)), 1_u32);
    let output_sample =
      i32::wrapping_shr(i32::wrapping_add(pixel, r), i32::cast_unsigned(sh)).clamp(0, max);
    *{
      let checked_value_21 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_21
    } =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(output_sample)
      {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_bilin_16bpc_prep_avx512_inner(
  _token: Server64,
  dst: &mut [i16],
  mid: &[[i32; MID_STRIDE]],
  row: MotionRow,
  phase: WordBilinearPhase,
  prep_bias: i32,
) -> Rav1dResult {
  let MotionRow { width, ordinal } = row;
  let Ok(y) = console_core_number_conversion::index(ordinal);
  let WordBilinearPhase(my) = phase;
  let sh = WORD_BILINEAR_BUFFERED_SHIFT;
  let dst_2 = &mut *dst;
  let w0 = _mm512_set1_epi32(i32::wrapping_sub(16, my));
  let w1 = _mm512_set1_epi32(my);
  let rnd = _mm512_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1, i32::cast_unsigned(sh)),
    1_u32,
  ));
  let shift_count = _mm_cvtsi32_si128(sh);
  let bias = _mm512_set1_epi32(prep_bias);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      &(*{
        let checked_value_36 = <[_]>::get(&*mid, y).ok_or(Rav1dError::EINVAL)?;
        checked_value_36
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r0 = loadu_512!(checked_value, [i32; 16]);
    let checked_value_2 = <[_]>::get(
      &(*{
        let checked_value_37 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_37
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r1 = loadu_512!(checked_value_2, [i32; 16]);

    let term0 = _mm512_mullo_epi32(r0, w0);
    let term1 = _mm512_mullo_epi32(r1, w1);
    let sum = _mm512_add_epi32(term0, term1);

    let shifted = _mm512_sra_epi32(_mm512_add_epi32(sum, rnd), shift_count);
    let biased = _mm512_sub_epi32(shifted, bias);

    let packed = _mm512_cvtsepi32_epi16(biased);
    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_256!(checked_value_3, packed);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_38 = <[_]>::get(
      &(*{
        let checked_value_18 = <[_]>::get(&*mid, y).ok_or(Rav1dError::EINVAL)?;
        checked_value_18
      }),
      column_index,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r0 = *checked_value_38;
    let checked_value_39 = <[_]>::get(
      &(*{
        let checked_value_20 =
          <[_]>::get(&*mid, (Wrapping(y) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
        checked_value_20
      }),
      column_index,
    )
    .ok_or(Rav1dError::EINVAL)?;
    let r1 = *checked_value_39;
    let pixel = i32::wrapping_add(
      i32::wrapping_mul(16, r0),
      i32::wrapping_mul(my, i32::wrapping_sub(r1, r0)),
    );
    let r =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_shl(1, i32::cast_unsigned(sh)), 1_u32);
    let output_sample = i32::wrapping_sub(
      i32::wrapping_shr(i32::wrapping_add(pixel, r), i32::cast_unsigned(sh)),
      prep_bias,
    );
    *{
      let checked_value_21 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_21
    } =
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(output_sample)
      {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn h_bilin_16bpc_put_avx512_inner(
  _token: Server64,
  dst: &mut [u16],
  src: &[u16],
  width: u32,
  phase: WordBilinearPhase,
  bd_max: i32,
) -> Rav1dResult {
  let WordBilinearPhase(mx) = phase;
  let dst_2 = &mut *dst;
  let src_2 = &*src;
  let w0 = _mm512_set1_epi32(i32::wrapping_sub(16, mx));
  let w1 = _mm512_set1_epi32(mx);
  let rnd = _mm512_set1_epi32(8);
  let shift_count = _mm_cvtsi32_si128(4);
  let zero = _mm512_setzero_si512();
  let max_val = _mm512_set1_epi32(bd_max);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p0 = _mm512_cvtepu16_epi32(loadu_256!(checked_value));
    let checked_value_2 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(1)).0..(Wrapping(column_index) + Wrapping(17)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p1 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_2));

    let term0 = _mm512_mullo_epi32(p0, w0);
    let term1 = _mm512_mullo_epi32(p1, w1);
    let sum = _mm512_add_epi32(term0, term1);

    let shifted = _mm512_sra_epi32(_mm512_add_epi32(sum, rnd), shift_count);
    let clamped = _mm512_min_epi32(_mm512_max_epi32(shifted, zero), max_val);

    let packed = _mm512_cvtusepi32_epi16(clamped);
    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_256!(checked_value_3, packed);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_4 = <[_]>::get(&*src_2, column_index).ok_or(Rav1dError::EINVAL)?;
    let x0 = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_4);
    let checked_value_5 =
      <[_]>::get(&*src_2, (Wrapping(column_index) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
    let x1 = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_5);
    let pixel = i32::wrapping_add(
      i32::wrapping_mul(i32::wrapping_sub(16, mx), x0),
      i32::wrapping_mul(mx, x1),
    );
    let result =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(pixel, 8), 4_u32).clamp(0, bd_max);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(result) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_bilin_16bpc_direct_avx512_inner(
  _token: Server64,
  dst: &mut [u16],
  input: BilinearInputRows<'_, u16>,
  width: u32,
  phase: WordBilinearPhase,
  bd_max: i32,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let BilinearInputRows { first, next } = input;
  let WordBilinearPhase(my) = phase;
  let w0 = _mm512_set1_epi32(i32::wrapping_sub(16, my));
  let w1 = _mm512_set1_epi32(my);
  let rnd = _mm512_set1_epi32(8);
  let shift_count = _mm_cvtsi32_si128(4);
  let zero = _mm512_setzero_si512();
  let max_val = _mm512_set1_epi32(bd_max);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      first,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p0 = _mm512_cvtepu16_epi32(loadu_256!(checked_value));
    let checked_value_2 = <[_]>::get(next, column_index..column_index.wrapping_add(16))
      .ok_or(crate::src::error::Rav1dError::EINVAL)
      .and_then(|_explicit_array_source| {
        <&[u16; 16]>::try_from(_explicit_array_source)
          .map_err(|_| crate::src::error::Rav1dError::EINVAL)
      })?;
    let p1 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_2));

    let term0 = _mm512_mullo_epi32(p0, w0);
    let term1 = _mm512_mullo_epi32(p1, w1);
    let sum = _mm512_add_epi32(term0, term1);

    let shifted = _mm512_sra_epi32(_mm512_add_epi32(sum, rnd), shift_count);
    let clamped = _mm512_min_epi32(_mm512_max_epi32(shifted, zero), max_val);

    let packed = _mm512_cvtusepi32_epi16(clamped);
    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_256!(checked_value_3, packed);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_4 = <[_]>::get(first, column_index).ok_or(Rav1dError::EINVAL)?;
    let x0 = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_4);
    let checked_value_5 = <[_]>::get(next, column_index).ok_or(Rav1dError::EINVAL)?;
    let x1 = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_5);
    let pixel = i32::wrapping_add(
      i32::wrapping_mul(i32::wrapping_sub(16, my), x0),
      i32::wrapping_mul(my, x1),
    );
    let result =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(pixel, 8), 4_u32).clamp(0, bd_max);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(result) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn h_bilin_16bpc_prep_direct_avx512_inner(
  _token: Server64,
  dst: &mut [i16],
  src: &[u16],
  width: u32,
  phase: WordBilinearPhase,
  prep_bias: i32,
) -> Rav1dResult {
  let WordBilinearPhase(mx) = phase;
  let dst_2 = &mut *dst;
  let src_2 = &*src;
  let w0 = _mm512_set1_epi32(i32::wrapping_sub(16, mx));
  let w1 = _mm512_set1_epi32(mx);
  let rnd = _mm512_set1_epi32(8);
  let shift_count = _mm_cvtsi32_si128(4);
  let bias = _mm512_set1_epi32(prep_bias);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p0 = _mm512_cvtepu16_epi32(loadu_256!(checked_value));
    let checked_value_2 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(1)).0..(Wrapping(column_index) + Wrapping(17)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p1 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_2));

    let term0 = _mm512_mullo_epi32(p0, w0);
    let term1 = _mm512_mullo_epi32(p1, w1);
    let sum = _mm512_add_epi32(term0, term1);

    let shifted = _mm512_sra_epi32(_mm512_add_epi32(sum, rnd), shift_count);
    let biased = _mm512_sub_epi32(shifted, bias);

    let packed = _mm512_cvtsepi32_epi16(biased);
    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_256!(checked_value_3, packed);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_4 = <[_]>::get(&*src_2, column_index).ok_or(Rav1dError::EINVAL)?;
    let x0 = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_4);
    let checked_value_5 =
      <[_]>::get(&*src_2, (Wrapping(column_index) + Wrapping(1)).0).ok_or(Rav1dError::EINVAL)?;
    let x1 = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_5);
    let pixel = i32::wrapping_add(
      i32::wrapping_mul(i32::wrapping_sub(16, mx), x0),
      i32::wrapping_mul(mx, x1),
    );
    let result = i32::wrapping_sub(
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(pixel, 8), 4_u32),
      prep_bias,
    );
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(result) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn v_bilin_16bpc_prep_direct_avx512_inner(
  _token: Server64,
  dst: &mut [i16],
  input: BilinearInputRows<'_, u16>,
  width: u32,
  phase: WordBilinearPhase,
  prep_bias: i32,
) -> Rav1dResult {
  let dst_2 = &mut *dst;
  let BilinearInputRows { first, next } = input;
  let WordBilinearPhase(my) = phase;
  let w0 = _mm512_set1_epi32(i32::wrapping_sub(16, my));
  let w1 = _mm512_set1_epi32(my);
  let rnd = _mm512_set1_epi32(8);
  let shift_count = _mm_cvtsi32_si128(4);
  let bias = _mm512_set1_epi32(prep_bias);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value = <[_]>::get(
      first,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&[u16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    let p0 = _mm512_cvtepu16_epi32(loadu_256!(checked_value));
    let checked_value_2 = <[_]>::get(next, column_index..column_index.wrapping_add(16))
      .ok_or(crate::src::error::Rav1dError::EINVAL)
      .and_then(|_explicit_array_source| {
        <&[u16; 16]>::try_from(_explicit_array_source)
          .map_err(|_| crate::src::error::Rav1dError::EINVAL)
      })?;
    let p1 = _mm512_cvtepu16_epi32(loadu_256!(checked_value_2));

    let term0 = _mm512_mullo_epi32(p0, w0);
    let term1 = _mm512_mullo_epi32(p1, w1);
    let sum = _mm512_add_epi32(term0, term1);

    let shifted = _mm512_sra_epi32(_mm512_add_epi32(sum, rnd), shift_count);
    let biased = _mm512_sub_epi32(shifted, bias);

    let packed = _mm512_cvtsepi32_epi16(biased);
    let checked_value_3 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(crate::src::error::Rav1dError::EINVAL)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 16]>::try_from(_explicit_array_source)
        .map_err(|_| crate::src::error::Rav1dError::EINVAL)
    })?;
    storeu_256!(checked_value_3, packed);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_4 = <[_]>::get(first, column_index).ok_or(Rav1dError::EINVAL)?;
    let x0 = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_4);
    let checked_value_5 = <[_]>::get(next, column_index).ok_or(Rav1dError::EINVAL)?;
    let x1 = <i32 as ::core::convert::From<::core::primitive::u16>>::from(*checked_value_5);
    let pixel = i32::wrapping_add(
      i32::wrapping_mul(i32::wrapping_sub(16, my), x0),
      i32::wrapping_mul(my, x1),
    );
    let result = i32::wrapping_sub(
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(pixel, 8), 4_u32),
      prep_bias,
    );
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(Rav1dError::EINVAL)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(result) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct BilinearRequest<'a, Pixel, Output> {
  destination: &'a mut [Output],
  stride: i64,
  source: PixelSource<'a, Pixel>,
  sampling: reference::Sampling,
}

#[cfg(target_arch = "x86_64")]
struct BilinearInputRows<'a, Pixel> {
  first: &'a [Pixel],
  next: &'a [Pixel],
}

#[cfg(target_arch = "x86_64")]
struct WordBilinearPhase(i32);

#[cfg(target_arch = "x86_64")]
const WORD_BILINEAR_BUFFERED_SHIFT: i32 = 8;

#[cfg(target_arch = "x86_64")]
#[doc = "Connects the selected word SIMD backend to the shared bilinear row traversal."]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one four-callback definition covers both word bilinear outputs and both word bilinear predictors with their original CPU tokens"
  )
)]
macro_rules! word_bilinear_writer {
  ($token:ident, $output:ty, $scale:expr; $horizontal:path, $vertical:path, $horizontal_direct:path, $vertical_direct:path) => {
    MotionRowWriters {
      horizontal_buffered: |target: &mut [i32], input: &[u16], width, phase| {
        $horizontal($token, target, input, width, phase)
      },
      vertical_buffered: |target: &mut [$output],
                          filtered: &[[i32; MID_STRIDE]],
                          geometry: MotionRow,
                          phase| {
        $vertical(
          $token,
          target,
          filtered,
          geometry,
          WordBilinearPhase(phase),
          $scale,
        )
      },
      horizontal_direct: |target: &mut [$output], input: &[u16], width, phase| {
        $horizontal_direct(
          $token,
          target,
          input,
          width,
          WordBilinearPhase(phase),
          $scale,
        )
      },
      vertical_direct: |target: &mut [$output],
                        input: BilinearInputRows<'_, u16>,
                        width,
                        phase| {
        $vertical_direct(
          $token,
          target,
          input,
          width,
          WordBilinearPhase(phase),
          $scale,
        )
      },
    }
  };
}

#[cfg(target_arch = "x86_64")]
fn write_bilinear<Pixel, Intermediate: Copy + Default + 'static, Output>(
  writer: MotionRowWriters<
    impl Fn(&mut [Intermediate], &[Pixel], u32, i32) -> Rav1dResult,
    impl Fn(&mut [Output], &[[Intermediate; MID_STRIDE]], MotionRow, i32) -> Rav1dResult,
    impl Fn(&mut [Output], &[Pixel], u32, i32) -> Rav1dResult,
    impl Fn(&mut [Output], BilinearInputRows<'_, Pixel>, u32, i32) -> Rav1dResult,
  >,
  request: BilinearRequest<'_, Pixel, Output>,
  pool: &'static MidPool<Intermediate, 130>,
  copy: impl Fn(&mut [Output], &[Pixel]) -> Result<(), Never>,
) -> Rav1dResult {
  use console_core_pixel_buffers::RowLayout;
  let BilinearRequest {
    destination,
    stride,
    source,
    sampling,
  } = request;
  let extent = sampling.checked_size()?;
  let Ok(bytes_per_pixel) =
    console_core_number_conversion::fixed_index(core::mem::size_of::<Pixel>());
  let pitch = source
    .stride_bytes
    .checked_div(bytes_per_pixel.cast_signed())
    .ok_or(Rav1dError::EINVAL)?;

  match (pitch, stride) {
    (1.., 1..) => {}
    _ => return Err(Rav1dError::EINVAL),
  }

  let Point {
    x: horizontal,
    y: vertical,
  } = sampling.phase;
  let reference_extent = Size {
    width: extent.width.wrapping_add(u64::from(horizontal != 0)),
    height: extent.height.wrapping_add(u64::from(vertical != 0)),
  };
  let input =
    RowLayout::new(reference_extent, source.origin, pitch).map_err(|_| Rav1dError::EINVAL)?;
  input
    .validate_len(source.samples.len())
    .map_err(|_| Rav1dError::EINVAL)?;
  let output = RowLayout::new(extent, 0, stride).map_err(|_| Rav1dError::EINVAL)?;
  let rows = output
    .rows_mut(destination)
    .map_err(|_| Rav1dError::EINVAL)?;
  let width = sampling.size.width.cast_unsigned();
  let height = sampling.size.height.cast_unsigned();

  match (horizontal, vertical) {
    (0, 0) => {
      let inputs = input.rows(source.samples).map_err(|_| Rav1dError::EINVAL)?;

      for (target, reference) in rows.zip(inputs) {
        let Ok(()) = copy(target, reference);
      }
    }
    (1.., 0) => {
      for (row, target) in (0..height).zip(rows) {
        let reference = source.row_tail(Point {
          x: 0,
          y: i64::from(row),
        })?;
        (writer.horizontal_direct)(target, reference, width, horizontal)?;
      }
    }
    (0, 1..) => {
      for (row, target) in (0..height).zip(rows) {
        let ordinal = i64::from(row);
        let first = source.row_tail(Point { x: 0, y: ordinal })?;
        let next = source.row_tail(Point {
          x: 0,
          y: ordinal.wrapping_add(1),
        })?;
        let input_2 = BilinearInputRows { first, next };
        (writer.vertical_direct)(target, input_2, width, vertical)?;
      }
    }
    (1.., 1..) => {
      let Ok(mut storage) = take_mid(pool);
      let filtered = storage.as_mut();

      for row in 0..=height {
        let Ok(index) = console_core_number_conversion::index(row);
        let target = filtered.get_mut(index).ok_or(Rav1dError::EINVAL)?;
        let reference = source.row_tail(Point {
          x: 0,
          y: i64::from(row),
        })?;
        (writer.horizontal_buffered)(target, reference, width, horizontal)?;
      }

      for (row, target) in (0..height).zip(rows) {
        let line = MotionRow {
          width,
          ordinal: row,
        };
        (writer.vertical_buffered)(target, filtered, line, vertical)?;
      }

      let Ok(()) = put_mid(pool, storage);
    }
    _ => return Err(Rav1dError::EINVAL),
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn put_bilin_16bpc_avx512_impl_inner(
  token: Server64,
  request: BilinearRequest<'_, u16, u16>,
  maximum: i32,
) -> Rav1dResult {
  let writer = word_bilinear_writer!(
    token, u16, maximum;
    h_bilin_16bpc_avx512_inner,
    v_bilin_16bpc_avx512_inner,
    h_bilin_16bpc_put_avx512_inner,
    v_bilin_16bpc_direct_avx512_inner
  );

  write_bilinear(writer, request, &MID_I32_130, copy_bilinear_pixels)
}

#[cfg(target_arch = "x86_64")]
fn put_bilin_16bpc_dispatch_inner(
  token: Desktop64,
  request: BilinearRequest<'_, u16, u16>,
  maximum: i32,
) -> Rav1dResult {
  let Ok(avx512) = crate::src::cpu::summon_avx512();

  match avx512 {
    Some(wide) => put_bilin_16bpc_avx512_impl_inner(wide, request, maximum),
    None => put_bilin_16bpc_avx2_impl_inner_safe(token, request, maximum),
  }
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn put_bilin_16bpc_avx2_impl_inner_safe(
  token: Desktop64,
  request: BilinearRequest<'_, u16, u16>,
  maximum: i32,
) -> Rav1dResult {
  let writer = word_bilinear_writer!(
    token, u16, maximum;
    h_bilin_16bpc_avx2_inner,
    v_bilin_16bpc_avx2_inner,
    h_bilin_16bpc_put_avx2_inner,
    v_bilin_16bpc_direct_avx2_inner
  );

  write_bilinear(writer, request, &MID_I32_130, copy_bilinear_pixels)
}

#[cfg(target_arch = "x86_64")]
fn copy_bilinear_pixels<Pixel: Copy>(
  output: &mut [Pixel],
  source: &[Pixel],
) -> Result<(), Never> {
  for (target, &pixel) in output.iter_mut().zip(source) {
    *target = pixel;
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn prep_word_bilinear_avx2(
  token: Desktop64,
  request: BilinearRequest<'_, u16, i16>,
) -> Rav1dResult {
  let writer = word_bilinear_writer!(
    token, i16, 8192;
    h_bilin_16bpc_avx2_inner,
    v_bilin_16bpc_prep_avx2_inner,
    h_bilin_16bpc_prep_direct_avx2_inner,
    v_bilin_16bpc_prep_direct_avx2_inner
  );

  write_bilinear(writer, request, &MID_I32_130, |target, reference| {
    prepare_word_identity(target, reference, 0)
  })
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn prep_word_bilinear_avx512(
  token: Server64,
  request: BilinearRequest<'_, u16, i16>,
) -> Rav1dResult {
  let writer = word_bilinear_writer!(
    token, i16, 8192;
    h_bilin_16bpc_avx512_inner,
    v_bilin_16bpc_prep_avx512_inner,
    h_bilin_16bpc_prep_direct_avx512_inner,
    v_bilin_16bpc_prep_direct_avx512_inner
  );

  write_bilinear(writer, request, &MID_I32_130, |target, reference| {
    prepare_word_identity(target, reference, 0)
  })
}

#[cfg(target_arch = "x86_64")]
fn prep_word_bilinear_dispatch(
  token: Desktop64,
  request: BilinearRequest<'_, u16, i16>,
) -> Rav1dResult {
  let Ok(avx512) = crate::src::cpu::summon_avx512();

  match avx512 {
    Some(wide) => prep_word_bilinear_avx512(wide, request),
    None => prep_word_bilinear_avx2(token, request),
  }
}

#[cfg(target_arch = "x86_64")]
#[doc = "Connects the selected byte SIMD backend to the shared bilinear row traversal."]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one four-callback definition covers the four byte bilinear output and predictor backends with their original CPU tokens"
  )
)]
macro_rules! byte_bilinear_writer {
  ($output:ty; $horizontal:expr, $vertical:expr, $horizontal_direct:expr, $vertical_direct:expr) => {
    MotionRowWriters {
      horizontal_buffered: $horizontal,
      vertical_buffered: |target: &mut [$output],
                          filtered: &[[i16; MID_STRIDE]],
                          geometry: MotionRow,
                          phase| {
        let Ok(index) = console_core_number_conversion::index(geometry.ordinal);
        let window = filtered.get(index..).ok_or(Rav1dError::EINVAL)?;
        let [first, next] = window.first_chunk::<2>().ok_or(Rav1dError::EINVAL)?;
        ($vertical)(
          target,
          [first.as_slice(), next.as_slice()],
          geometry.width,
          phase,
        )
      },
      horizontal_direct: $horizontal_direct,
      vertical_direct: |target: &mut [$output],
                        input: BilinearInputRows<'_, u8>,
                        width,
                        phase| {
        ($vertical_direct)(target, input.first, input.next, width, phase)
      },
    }
  };
}

#[cfg(target_arch = "x86_64")]
fn prepare_byte_bilinear_pair<Pixel: Copy + Into<i32>>(
  output: &mut [i16],
  [first, next]: [&[Pixel]; 2],
  phase: i32,
  shift: u32,
) -> Rav1dResult {
  let front = 16_i32.wrapping_sub(phase);

  for (target, (&a, &b)) in output.iter_mut().zip(first.iter().zip(next)) {
    let sum = <_ as ::core::convert::Into<::core::primitive::i32>>::into(a)
      .wrapping_mul(front)
      .wrapping_add(
        <_ as ::core::convert::Into<::core::primitive::i32>>::into(b).wrapping_mul(phase),
      );
    let shifted = console_core_video_interpolation::round_shift(sum, shift);
    let Ok(prediction) = console_core_number_conversion::wrapped::<_, i16>(shifted);
    *target = prediction;
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
fn prepare_byte_bilinear_identity(output: &mut [i16], source: &[u8]) -> Result<(), Never> {
  for (target, &pixel) in output.iter_mut().zip(source) {
    *target = i16::from(pixel).wrapping_shl(4);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn put_byte_bilinear_avx2(token: Desktop64, request: BilinearRequest<'_, u8, u8>) -> Rav1dResult {
  let writer = byte_bilinear_writer!(
    u8;
    |target: &mut [i16], source: &[u8], width, phase| h_filter_bilin_8bpc_avx2_inner(token, target, source, width, phase, 0),
    |target, rows: [&[i16]; 2], width, phase| v_filter_bilin_8bpc_avx2_inner(token, target, &rows, width, phase, 8, 255),
    |target: &mut [u8], source: &[u8], width, phase| h_bilin_8bpc_put_avx2_inner(token, target, source, width, phase),
    |target, first, next, width, phase| v_bilin_8bpc_direct_avx2_inner(token, target, first, next, width, phase)
  );

  write_bilinear(writer, request, &MID_I16_130, copy_bilinear_pixels)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn prep_byte_bilinear_avx2(
  token: Desktop64,
  request: BilinearRequest<'_, u8, i16>,
) -> Rav1dResult {
  let writer = byte_bilinear_writer!(
    i16;
    |target: &mut [i16], source: &[u8], width, phase| h_filter_bilin_8bpc_avx2_inner(token, target, source, width, phase, 0),
    |target, rows: [&[i16]; 2], _width, phase| prepare_byte_bilinear_pair(target, rows, phase, 4),
    |target: &mut [i16], source: &[u8], width, phase| h_filter_bilin_8bpc_avx2_inner(token, target, source, width, phase, 0),
    |target, first, next, _width, phase| prepare_byte_bilinear_pair(target, [first, next], phase, 0)
  );

  write_bilinear(
    writer,
    request,
    &MID_I16_130,
    prepare_byte_bilinear_identity,
  )
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn put_byte_bilinear_avx512(
  token: Server64,
  request: BilinearRequest<'_, u8, u8>,
) -> Rav1dResult {
  let writer = byte_bilinear_writer!(
    u8;
    |target: &mut [i16], source: &[u8], width, phase| h_filter_bilin_8bpc_avx512_inner(token, target, source, width, phase, 0),
    |target, rows: [&[i16]; 2], width, phase| v_filter_bilin_8bpc_avx512_inner(token, target, &rows, width, phase, 8, 255),
    |target: &mut [u8], source: &[u8], width, phase| h_bilin_8bpc_put_avx512_inner(token, target, source, width, phase),
    |target, first, next, width, phase| v_bilin_8bpc_direct_avx512_inner(token, target, first, next, width, phase)
  );

  write_bilinear(writer, request, &MID_I16_130, copy_bilinear_pixels)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn prep_byte_bilinear_avx512(
  token: Server64,
  request: BilinearRequest<'_, u8, i16>,
) -> Rav1dResult {
  let writer = byte_bilinear_writer!(
    i16;
    |target: &mut [i16], source: &[u8], width, phase| h_filter_bilin_8bpc_avx512_inner(token, target, source, width, phase, 0),
    |target, rows: [&[i16]; 2], width, phase| v_filter_bilin_8bpc_prep_avx512_inner(token, target, &rows, width, phase, 4),
    |target: &mut [i16], source: &[u8], width, phase| h_filter_bilin_8bpc_avx512_inner(token, target, source, width, phase, 0),
    |target, first, next, width, phase| v_bilin_8bpc_prep_direct_avx512_inner(token, target, first, next, width, phase)
  );

  write_bilinear(
    writer,
    request,
    &MID_I16_130,
    prepare_byte_bilinear_identity,
  )
}

#[cfg(target_arch = "x86_64")]
fn put_bilin_8bpc_dispatch_inner(
  token: Desktop64,
  request: BilinearRequest<'_, u8, u8>,
) -> Rav1dResult {
  let Ok(avx512) = crate::src::cpu::summon_avx512();

  match avx512 {
    Some(wide) => put_byte_bilinear_avx512(wide, request),
    None => put_byte_bilinear_avx2(token, request),
  }
}

#[cfg(target_arch = "x86_64")]
fn prep_bilin_8bpc_dispatch_inner(
  token: Desktop64,
  request: BilinearRequest<'_, u8, i16>,
) -> Rav1dResult {
  let Ok(avx512) = crate::src::cpu::summon_avx512();

  match avx512 {
    Some(wide) => prep_byte_bilinear_avx512(wide, request),
    None => prep_byte_bilinear_avx2(token, request),
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BlendAxis {
  Horizontal,
  Vertical,
}

#[cfg(target_arch = "x86_64")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MotionFallback {
  MissingCpuFeature,
  BufferLayout,
  AliasedView,
}

#[cfg(target_arch = "x86_64")]
pub(crate) type MotionDispatch = Result<(), MotionFallback>;

#[cfg(target_arch = "x86_64")]
fn with_motion_destination<BD: BitDepth, Context>(
  destination: PicOffset,
  extent: Size<i32>,
  context: Context,
  write: impl FnOnce(PixelDestination<'_>, Context) -> MotionDispatch,
) -> MotionDispatch {
  let Ok(width) = console_core_number_conversion::wrapped_index(extent.width);
  let Ok(height) = console_core_number_conversion::wrapped_index(extent.height);

  crate::include::dav1d::picture::with_pixel_guard_mut::<BD, _>(
    &destination,
    width,
    height,
    |bytes, start, pitch| {
      let Ok(origin) = console_core_number_conversion::fixed_index(start);
      let Ok(stride) = console_core_number_conversion::fixed_offset(pitch);
      write(
        PixelDestination {
          bytes,
          origin,
          stride,
        },
        context,
      )
    },
  )
}

#[cfg(target_arch = "x86_64")]
#[must_use]
pub fn avg_dispatch<BD: BitDepth>(
  dst: PicOffset,
  tmp1: &[i16; COMPINTER_LEN],
  tmp2: &[i16; COMPINTER_LEN],
  block_extent: Size<i32>,
  bd: BD,
) -> MotionDispatch {
  let Ok(avx2) = crate::src::cpu::summon_avx2();
  let _token = match avx2 {
    Some(_token) => _token,
    None => {
      return Err(MotionFallback::MissingCpuFeature);
    }
  };
  with_motion_destination::<BD, _>(dst, block_extent, (), |destination, ()| {
    avg_dispatch_inner::<BD>(destination, tmp1, tmp2, block_extent, bd)
  })
}

#[cfg(target_arch = "x86_64")]
fn avg_dispatch_inner<BD: BitDepth>(
  destination: PixelDestination<'_>,
  tmp1: &[i16; COMPINTER_LEN],
  tmp2: &[i16; COMPINTER_LEN],
  block_extent: Size<i32>,
  bd: BD,
) -> MotionDispatch {
  let Size {
    width: w,
    height: h,
  } = block_extent;
  use crate::include::common::bitdepth::BPC;
  let Ok(avx512_token) = crate::src::cpu::summon_avx512();
  let Ok(avx2) = crate::src::cpu::summon_avx2();
  let token = match avx2 {
    Some(token) => token,
    None => {
      return Err(MotionFallback::MissingCpuFeature);
    }
  };
  let bd_c = bd.into_c();
  let (dst, byte_stride) = destination.forward()?;

  let blended = match BD::BPC {
    BPC::BPC8 => match avx512_token {
      Some(t512) => avg_8bpc_avx512_safe(
        t512,
        dst,
        byte_stride,
        tmp1,
        tmp2,
        Size {
          width: w,
          height: h,
        },
      ),
      None => avg_8bpc_avx2_safe(
        token,
        dst,
        byte_stride,
        tmp1,
        tmp2,
        Size {
          width: w,
          height: h,
        },
      ),
    },
    BPC::BPC16 => match avx512_token {
      Some(t512) => avg_16bpc_avx512_safe(
        t512,
        dst,
        byte_stride,
        tmp1,
        tmp2,
        Size {
          width: w,
          height: h,
        },
        bd_c,
      ),
      None => avg_16bpc_avx2_safe(
        token,
        dst,
        byte_stride,
        tmp1,
        tmp2,
        Size {
          width: w,
          height: h,
        },
        bd_c,
      ),
    },
  };

  blended.map_err(|_invalid_layout| MotionFallback::BufferLayout)
}

#[cfg(target_arch = "x86_64")]
#[must_use]
pub fn w_avg_dispatch<BD: BitDepth>(
  dst: PicOffset,
  tmp1: &[i16; COMPINTER_LEN],
  tmp2: &[i16; COMPINTER_LEN],
  block_extent: Size<i32>,
  weight: i32,
  bd: BD,
) -> MotionDispatch {
  let Ok(avx2) = crate::src::cpu::summon_avx2();
  let _token = match avx2 {
    Some(_token) => _token,
    None => {
      return Err(MotionFallback::MissingCpuFeature);
    }
  };
  with_motion_destination::<BD, _>(dst, block_extent, (), |destination, ()| {
    w_avg_dispatch_inner::<BD>(destination, tmp1, tmp2, block_extent, weight, bd)
  })
}

#[cfg(target_arch = "x86_64")]
fn w_avg_dispatch_inner<BD: BitDepth>(
  destination: PixelDestination<'_>,
  tmp1: &[i16; COMPINTER_LEN],
  tmp2: &[i16; COMPINTER_LEN],
  block_extent: Size<i32>,
  weight: i32,
  bd: BD,
) -> MotionDispatch {
  let Size {
    width: w,
    height: h,
  } = block_extent;
  use crate::include::common::bitdepth::BPC;
  let Ok(avx512_token) = crate::src::cpu::summon_avx512();
  let Ok(avx2) = crate::src::cpu::summon_avx2();
  let token = match avx2 {
    Some(token) => token,
    None => {
      return Err(MotionFallback::MissingCpuFeature);
    }
  };
  let bd_c = bd.into_c();
  let (dst, byte_stride) = destination.forward()?;

  let blended = match BD::BPC {
    BPC::BPC8 => match avx512_token {
      Some(t512) => w_avg_8bpc_avx512_safe(
        t512,
        dst,
        byte_stride,
        tmp1,
        tmp2,
        Size {
          width: w,
          height: h,
        },
        weight,
      ),
      None => w_avg_8bpc_avx2_safe(
        token,
        dst,
        byte_stride,
        tmp1,
        tmp2,
        Size {
          width: w,
          height: h,
        },
        weight,
      ),
    },
    BPC::BPC16 => match avx512_token {
      Some(t512) => w_avg_16bpc_avx512_safe(
        t512,
        dst,
        byte_stride,
        tmp1,
        tmp2,
        Size {
          width: w,
          height: h,
        },
        WeightedAverageScale {
          weight,
          bitdepth_max: bd_c,
        },
      ),
      None => w_avg_16bpc_avx2_safe(
        token,
        dst,
        byte_stride,
        tmp1,
        tmp2,
        Size {
          width: w,
          height: h,
        },
        WeightedAverageScale {
          weight,
          bitdepth_max: bd_c,
        },
      ),
    },
  };

  blended.map_err(|_invalid_layout| MotionFallback::BufferLayout)
}

#[cfg(target_arch = "x86_64")]
#[must_use]
pub fn mask_dispatch<BD: BitDepth>(
  dst: PicOffset,
  tmp1: &[i16; COMPINTER_LEN],
  tmp2: &[i16; COMPINTER_LEN],
  block_extent: Size<i32>,
  mask: &[u8],
  bd: BD,
) -> MotionDispatch {
  let Ok(avx2) = crate::src::cpu::summon_avx2();
  let _token = match avx2 {
    Some(_token) => _token,
    None => {
      return Err(MotionFallback::MissingCpuFeature);
    }
  };
  with_motion_destination::<BD, _>(dst, block_extent, (), |destination, ()| {
    mask_dispatch_inner::<BD>(destination, tmp1, tmp2, block_extent, mask, bd)
  })
}

#[cfg(target_arch = "x86_64")]
fn mask_dispatch_inner<BD: BitDepth>(
  destination: PixelDestination<'_>,
  tmp1: &[i16; COMPINTER_LEN],
  tmp2: &[i16; COMPINTER_LEN],
  block_extent: Size<i32>,
  mask: &[u8],
  bd: BD,
) -> MotionDispatch {
  let Size {
    width: w,
    height: h,
  } = block_extent;
  use crate::include::common::bitdepth::BPC;
  let Ok(avx512_token) = crate::src::cpu::summon_avx512();
  let Ok(avx2) = crate::src::cpu::summon_avx2();
  let token = match avx2 {
    Some(token) => token,
    None => {
      return Err(MotionFallback::MissingCpuFeature);
    }
  };
  let bd_c = bd.into_c();
  let (dst, byte_stride) = destination.forward()?;

  let blended = match BD::BPC {
    BPC::BPC8 => match avx512_token {
      Some(t512) => mask_8bpc_avx512_safe(
        t512,
        dst,
        byte_stride,
        tmp1,
        tmp2,
        Size {
          width: w,
          height: h,
        },
        mask,
      ),
      None => mask_8bpc_avx2_safe(
        token,
        dst,
        byte_stride,
        tmp1,
        tmp2,
        Size {
          width: w,
          height: h,
        },
        mask,
      ),
    },
    BPC::BPC16 => match avx512_token {
      Some(t512) => mask_16bpc_avx512_safe(
        t512,
        dst,
        byte_stride,
        tmp1,
        tmp2,
        Size {
          width: w,
          height: h,
        },
        mask,
        bd_c,
      ),
      None => mask_16bpc_avx2_safe(
        token,
        dst,
        byte_stride,
        tmp1,
        tmp2,
        Size {
          width: w,
          height: h,
        },
        mask,
        bd_c,
      ),
    },
  };

  blended.map_err(|_invalid_layout| MotionFallback::BufferLayout)
}

#[cfg(target_arch = "x86_64")]
#[must_use]
pub fn blend_dispatch<BD: BitDepth>(
  dst: PicOffset,
  tmp: &[BD::Pixel; SCRATCH_INTER_INTRA_BUF_LEN],
  block_extent: Size<i32>,
  mask: &[u8],
) -> MotionDispatch {
  let Ok(avx2) = crate::src::cpu::summon_avx2();
  let _token = match avx2 {
    Some(_token) => _token,
    None => {
      return Err(MotionFallback::MissingCpuFeature);
    }
  };
  use zerocopy::IntoBytes;
  let tmp_bytes = tmp.as_bytes();
  with_motion_destination::<BD, _>(dst, block_extent, (), |destination, ()| {
    blend_dispatch_inner::<BD>(destination, tmp_bytes, block_extent, mask)
  })
}

#[cfg(target_arch = "x86_64")]
fn blend_dispatch_inner<BD: BitDepth>(
  destination: PixelDestination<'_>,
  tmp_bytes: &[u8],
  block_extent: Size<i32>,
  mask: &[u8],
) -> MotionDispatch {
  let Size {
    width: w,
    height: h,
  } = block_extent;
  use crate::include::common::bitdepth::BPC;
  let Ok(avx2) = crate::src::cpu::summon_avx2();
  let token = match avx2 {
    Some(token) => token,
    None => {
      return Err(MotionFallback::MissingCpuFeature);
    }
  };

  let (dst, stride) = destination.forward()?;

  let applied = match BD::BPC {
    BPC::BPC8 => blend_8bpc_avx2_safe(
      token,
      dst,
      stride,
      tmp_bytes,
      Size {
        width: w,
        height: h,
      },
      mask,
    ),
    BPC::BPC16 => blend_16bpc_avx2_safe(
      token,
      dst,
      stride,
      tmp_bytes,
      Size {
        width: w,
        height: h,
      },
      mask,
    ),
  };
  applied.map_err(|_invalid_layout| MotionFallback::BufferLayout)
}

#[cfg(target_arch = "x86_64")]
#[must_use]
pub fn blend_dir_dispatch<BD: BitDepth>(
  axis: BlendAxis,
  dst: PicOffset,
  tmp: &[BD::Pixel; SCRATCH_LAP_LEN],
  block_extent: Size<i32>,
) -> MotionDispatch {
  let Ok(avx2) = crate::src::cpu::summon_avx2();
  let _token = match avx2 {
    Some(_token) => _token,
    None => {
      return Err(MotionFallback::MissingCpuFeature);
    }
  };
  use zerocopy::IntoBytes;
  let tmp_bytes = tmp.as_bytes();
  with_motion_destination::<BD, _>(dst, block_extent, (), |destination, ()| {
    blend_dir_dispatch_inner::<BD>(axis, destination, tmp_bytes, block_extent)
  })
}

#[cfg(target_arch = "x86_64")]
fn blend_dir_dispatch_inner<BD: BitDepth>(
  axis: BlendAxis,
  destination: PixelDestination<'_>,
  tmp_bytes: &[u8],
  block_extent: Size<i32>,
) -> MotionDispatch {
  let Size {
    width: w,
    height: h,
  } = block_extent;
  use crate::include::common::bitdepth::BPC;
  let Ok(avx2) = crate::src::cpu::summon_avx2();
  let token = match avx2 {
    Some(token) => token,
    None => {
      return Err(MotionFallback::MissingCpuFeature);
    }
  };

  let (dst, stride) = destination.forward()?;

  let applied = match (BD::BPC, axis) {
    (BPC::BPC8, BlendAxis::Horizontal) => blend_h_8bpc_avx2_safe(
      token,
      dst,
      stride,
      tmp_bytes,
      Size {
        width: w,
        height: h,
      },
    ),
    (BPC::BPC8, BlendAxis::Vertical) => blend_v_8bpc_avx2_safe(
      token,
      dst,
      stride,
      tmp_bytes,
      Size {
        width: w,
        height: h,
      },
    ),
    (BPC::BPC16, BlendAxis::Horizontal) => blend_h_16bpc_avx2_safe(
      token,
      dst,
      stride,
      tmp_bytes,
      Size {
        width: w,
        height: h,
      },
    ),
    (BPC::BPC16, BlendAxis::Vertical) => blend_v_16bpc_avx2_safe(
      token,
      dst,
      stride,
      tmp_bytes,
      Size {
        width: w,
        height: h,
      },
    ),
  };
  applied.map_err(|_invalid_layout| MotionFallback::BufferLayout)
}

#[cfg(target_arch = "x86_64")]
pub(crate) fn w_mask_dispatch<BD: BitDepth>(
  layout: Rav1dPixelLayoutSubSampled,
  dst: PicOffset,
  tmp1: &[i16; COMPINTER_LEN],
  tmp2: &[i16; COMPINTER_LEN],
  block_extent: Size<i32>,
  mask: &mut [u8; SEG_MASK_LEN],
  sign: i32,
  bd: BD,
) -> MotionDispatch {
  let Ok(avx2) = crate::src::cpu::summon_avx2();

  match avx2 {
    Some(_) => {}
    None => return Err(MotionFallback::MissingCpuFeature),
  }

  let Ok(mask_sign) = console_core_number_conversion::wrapped(sign);
  let mode = WeightedMaskMode {
    layout,
    sign: mask_sign,
    depth: bd,
  };
  with_motion_destination::<BD, _>(dst, block_extent, mask, |destination, mask_2| {
    let (pixels, stride) = destination.forward()?;
    write_weighted_mask(pixels, stride, [tmp1, tmp2], block_extent, mask_2, mode)
      .map_err(|_invalid_layout| MotionFallback::BufferLayout)
  })
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn dispatch_eight_tap<T>(
  token: Desktop64,
  destination: T,
  avx2: impl FnOnce(Desktop64, T) -> Rav1dResult,
  avx512: impl FnOnce(Server64, T) -> Rav1dResult,
) -> Rav1dResult {
  let Ok(wide) = crate::src::cpu::summon_avx512();

  match wide {
    Some(server) => avx512(server, destination),
    None => avx2(token, destination),
  }
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn put_8tap_8bpc_dispatch_inner(
  token: Desktop64,
  dst: &mut [u8],
  dst_stride: i64,
  source: PixelSource<'_, u8>,
  block_extent: Size<i32>,
  motion_phase: Point<i32>,
  filter_axes: Point<Rav1dFilterMode>,
) -> Rav1dResult {
  let output = MotionOutput {
    samples: dst,
    row_stride: dst_stride,
    maximum: 255,
  };
  let sampling = reference::Sampling {
    size: block_extent,
    phase: motion_phase,
  };
  let request = EightTapOutput {
    output,
    source,
    sampling,
    axes: filter_axes,
  };
  dispatch_eight_tap(
    token,
    request,
    |narrow, work| put_8tap_8bpc_avx2_impl_inner(narrow, work),
    |wide, work| put_8tap_8bpc_avx512_impl_inner(wide, work),
  )
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn put_8tap_16bpc_dispatch_inner(
  token: Desktop64,
  dst: &mut [u16],
  dst_stride: i64,
  source: PixelSource<'_, u16>,
  block_extent: Size<i32>,
  motion_phase: Point<i32>,
  bd_c: i32,
  filter_axes: Point<Rav1dFilterMode>,
) -> Rav1dResult {
  let output = MotionOutput {
    samples: dst,
    row_stride: dst_stride.wrapping_div(2),
    maximum: bd_c,
  };
  let sampling = reference::Sampling {
    size: block_extent,
    phase: motion_phase,
  };
  let request = EightTapOutput {
    output,
    source,
    sampling,
    axes: filter_axes,
  };
  dispatch_eight_tap(
    token,
    request,
    |narrow, work| put_8tap_16bpc_avx2_impl_inner(narrow, work),
    |wide, work| put_8tap_16bpc_avx512_impl_inner(wide, work),
  )
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn prep_8tap_8bpc_dispatch_inner(
  token: Desktop64,
  tmp: &mut [i16],
  source: PixelSource<'_, u8>,
  block_extent: Size<i32>,
  motion_phase: Point<i32>,
  filter_axes: Point<Rav1dFilterMode>,
) -> Rav1dResult {
  dispatch_eight_tap(
    token,
    tmp,
    |narrow, target| {
      prep_8tap_8bpc_avx2_impl_inner(
        narrow,
        target,
        source,
        block_extent,
        motion_phase,
        filter_axes,
      )
    },
    |wide, target| {
      prep_8tap_8bpc_avx512_impl_inner(
        wide,
        target,
        source,
        block_extent,
        motion_phase,
        filter_axes,
      )
    },
  )
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn prep_8tap_16bpc_dispatch_inner(
  token: Desktop64,
  tmp: &mut [i16],
  source: PixelSource<'_, u16>,
  block_extent: Size<i32>,
  motion_phase: Point<i32>,
  bitdepth_max: i32,
  filter_axes: Point<Rav1dFilterMode>,
) -> Rav1dResult {
  dispatch_eight_tap(
    token,
    tmp,
    |narrow, target| {
      prep_8tap_16bpc_avx2_impl_inner(
        narrow,
        target,
        source,
        block_extent,
        motion_phase,
        bitdepth_max,
        filter_axes,
      )
    },
    |wide, target| {
      prep_8tap_16bpc_avx512_impl_inner(
        wide,
        target,
        source,
        block_extent,
        motion_phase,
        bitdepth_max,
        filter_axes,
      )
    },
  )
}

#[cfg(target_arch = "x86_64")]
#[must_use]
pub fn mc_put_dispatch<BD: BitDepth>(
  filter: Filter2d,
  dst: PicOffset,
  src: PicOffset,
  block_extent: Size<i32>,
  motion_phase: Point<i32>,
  depth: BD,
) -> MotionDispatch {
  let Ok(avx2) = crate::src::cpu::summon_avx2();

  let token = match avx2 {
    Some(matched_token_2) => matched_token_2,
    ::core::option::Option::None => return Err(MotionFallback::MissingCpuFeature),
  };

  match { ::core::primitive::bool::then_some(dst.data.ref_eq(src.data), ()) } {
    ::core::option::Option::Some(()) => {
      return Err(MotionFallback::AliasedView);
    }
    ::core::option::Option::None => {}
  }

  let sampling = reference::Sampling {
    size: block_extent,
    phase: motion_phase,
  };
  let size = sampling
    .checked_size()
    .map_err(|_| MotionFallback::BufferLayout)?;
  let Ok(origin) = console_core_number_conversion::fixed_index(dst.offset);
  let Ok(pitch) = console_core_number_conversion::fixed_offset(match dst.pixel_stride::<BD>() {
    Ok(samples) => samples,
  });
  let layout = console_core_pixel_buffers::RowLayout::new(size, origin, pitch)
    .map_err(|_| MotionFallback::BufferLayout)?;
  layout
    .validate_len(dst.data.pixel_len::<BD>())
    .map_err(|_| MotionFallback::BufferLayout)?;
  let Ok(width) = console_core_number_conversion::index(size.width);
  let Ok(height) = console_core_number_conversion::index(size.height);
  crate::include::dav1d::picture::with_pixel_guard_mut::<BD, _>(
    &dst,
    width,
    height,
    |bytes, offset, pitch_2| {
      let Ok(origin_2) = console_core_number_conversion::fixed_index(offset);
      let Ok(stride) = console_core_number_conversion::fixed_offset(pitch_2);
      let output = PixelDestination {
        bytes,
        origin: origin_2,
        stride,
      };
      mc_put_dispatch_inner(token, filter, output, src, sampling, depth)
    },
  )
}

#[cfg(target_arch = "x86_64")]
fn mc_put_dispatch_inner<BD: BitDepth>(
  token: Desktop64,
  filter: Filter2d,
  output: PixelDestination<'_>,
  src: PicOffset,
  sampling: reference::Sampling,
  depth: BD,
) -> MotionDispatch {
  use crate::include::common::bitdepth::BPC;
  use zerocopy::IntoBytes;
  let checked = (|destination_view: PixelDestination<'_>| -> Rav1dResult {
    let (guard, origin) = reference::filter_guard::<BD>(src, filter, sampling)?;
    let source = guard.as_bytes();
    let Ok(video_stride) = console_core_number_conversion::fixed_offset(src.stride());
    let Ok(start) = console_core_number_conversion::index(destination_view.origin);
    let destination = destination_view
      .bytes
      .get_mut(start..)
      .ok_or(Rav1dError::EINVAL)?;
    let reference::Sampling { size, phase } = sampling;
    let Ok((horizontal, vertical)) = filter.hv();

    match (BD::BPC, filter) {
      (BPC::BPC8, Filter2d::Bilinear) => {
        let input = PixelSource {
          samples: source,
          origin,
          stride_bytes: video_stride,
        };
        let request = BilinearRequest {
          destination,
          stride: destination_view.stride,
          source: input,
          sampling,
        };
        put_bilin_8bpc_dispatch_inner(token, request)
      }
      (
        BPC::BPC8,
        Filter2d::Regular8Tap
        | Filter2d::RegularSmooth8Tap
        | Filter2d::RegularSharp8Tap
        | Filter2d::SharpRegular8Tap
        | Filter2d::SharpSmooth8Tap
        | Filter2d::Sharp8Tap
        | Filter2d::SmoothRegular8Tap
        | Filter2d::Smooth8Tap
        | Filter2d::SmoothSharp8Tap,
      ) => put_8tap_8bpc_dispatch_inner(
        token,
        destination,
        destination_view.stride,
        PixelSource {
          samples: source,
          origin,
          stride_bytes: video_stride,
        },
        size,
        phase,
        Point {
          x: horizontal,
          y: vertical,
        },
      ),
      (BPC::BPC16, mode) => {
        let input: &[u16] =
          zerocopy::FromBytes::ref_from_bytes(source).map_err(|_| Rav1dError::EINVAL)?;
        let output_2: &mut [u16] =
          zerocopy::FromBytes::mut_from_bytes(destination).map_err(|_| Rav1dError::EINVAL)?;
        let max = depth.into_c();

        match mode {
          Filter2d::Bilinear => {
            let source_2 = PixelSource {
              samples: input,
              origin,
              stride_bytes: video_stride,
            };
            let stride = destination_view.stride.wrapping_div(2);
            let request = BilinearRequest {
              destination: output_2,
              stride,
              source: source_2,
              sampling,
            };
            put_bilin_16bpc_dispatch_inner(token, request, max)
          }
          Filter2d::Regular8Tap
          | Filter2d::RegularSmooth8Tap
          | Filter2d::RegularSharp8Tap
          | Filter2d::SharpRegular8Tap
          | Filter2d::SharpSmooth8Tap
          | Filter2d::Sharp8Tap
          | Filter2d::SmoothRegular8Tap
          | Filter2d::Smooth8Tap
          | Filter2d::SmoothSharp8Tap => put_8tap_16bpc_dispatch_inner(
            token,
            output_2,
            destination_view.stride,
            PixelSource {
              samples: input,
              origin,
              stride_bytes: video_stride,
            },
            size,
            phase,
            max,
            Point {
              x: horizontal,
              y: vertical,
            },
          ),
        }
      }
    }
  })(output);
  checked.map_err(|_invalid_layout| MotionFallback::BufferLayout)
}

#[cfg(target_arch = "x86_64")]
pub fn mct_prep_dispatch<BD: BitDepth>(
  filter: Filter2d,
  predictors: &mut [i16],
  src: PicOffset,
  block_extent: Size<i32>,
  motion_phase: Point<i32>,
  depth: BD,
) -> MotionDispatch {
  use crate::include::common::bitdepth::BPC;
  use zerocopy::IntoBytes;
  let Ok(avx2) = crate::src::cpu::summon_avx2();

  let token = match avx2 {
    Some(matched_token_2) => matched_token_2,
    ::core::option::Option::None => return Err(MotionFallback::MissingCpuFeature),
  };

  let checked = (|prediction_storage: &mut [i16]| -> Rav1dResult {
    let sampling = reference::Sampling {
      size: block_extent,
      phase: motion_phase,
    };
    let size = sampling.checked_size()?;
    let length = size
      .width
      .checked_mul(size.height)
      .ok_or(Rav1dError::EINVAL)?;
    let Ok(end) = console_core_number_conversion::index(length);
    let output = prediction_storage
      .get_mut(..end)
      .ok_or(Rav1dError::EINVAL)?;
    let (guard, origin) = reference::filter_guard::<BD>(src, filter, sampling)?;
    let source = guard.as_bytes();
    let Ok(stride) = console_core_number_conversion::fixed_offset(src.stride());
    let Ok((horizontal, vertical)) = filter.hv();

    match (BD::BPC, filter) {
      (BPC::BPC8, Filter2d::Bilinear) => {
        let input = PixelSource {
          samples: source,
          origin,
          stride_bytes: stride,
        };
        let request = BilinearRequest {
          destination: output,
          stride: i64::from(block_extent.width),
          source: input,
          sampling,
        };
        prep_bilin_8bpc_dispatch_inner(token, request)
      }
      (
        BPC::BPC8,
        Filter2d::Regular8Tap
        | Filter2d::RegularSmooth8Tap
        | Filter2d::RegularSharp8Tap
        | Filter2d::SharpRegular8Tap
        | Filter2d::SharpSmooth8Tap
        | Filter2d::Sharp8Tap
        | Filter2d::SmoothRegular8Tap
        | Filter2d::Smooth8Tap
        | Filter2d::SmoothSharp8Tap,
      ) => prep_8tap_8bpc_dispatch_inner(
        token,
        output,
        PixelSource {
          samples: source,
          origin,
          stride_bytes: stride,
        },
        block_extent,
        motion_phase,
        Point {
          x: horizontal,
          y: vertical,
        },
      ),
      (BPC::BPC16, mode) => {
        let input: &[u16] =
          zerocopy::FromBytes::ref_from_bytes(source).map_err(|_| Rav1dError::EINVAL)?;
        let max = depth.into_c();

        match mode {
          Filter2d::Bilinear => {
            let input_plane = PixelSource {
              samples: input,
              origin,
              stride_bytes: stride,
            };
            let request = BilinearRequest {
              destination: output,
              stride: i64::from(block_extent.width),
              source: input_plane,
              sampling,
            };
            prep_word_bilinear_dispatch(token, request)
          }
          Filter2d::Regular8Tap
          | Filter2d::RegularSmooth8Tap
          | Filter2d::RegularSharp8Tap
          | Filter2d::SharpRegular8Tap
          | Filter2d::SharpSmooth8Tap
          | Filter2d::Sharp8Tap
          | Filter2d::SmoothRegular8Tap
          | Filter2d::Smooth8Tap
          | Filter2d::SmoothSharp8Tap => prep_8tap_16bpc_dispatch_inner(
            token,
            output,
            PixelSource {
              samples: input,
              origin,
              stride_bytes: stride,
            },
            block_extent,
            motion_phase,
            max,
            Point {
              x: horizontal,
              y: vertical,
            },
          ),
        }
      }
    }
  })(predictors);
  checked.map_err(|_invalid_layout| MotionFallback::BufferLayout)
}

#[cfg(target_arch = "x86_64")]
#[derive(Clone, Copy)]
struct PixelSource<'a, Sample> {
  samples: &'a [Sample],
  origin: u64,
  stride_bytes: i64,
}

#[cfg(target_arch = "x86_64")]
impl<'a, Sample> PixelSource<'a, Sample> {
  #[inline]
  fn row_tail(&self, position: Point<i64>) -> Rav1dResult<&'a [Sample]> {
    let Ok(sample_size) =
      console_core_number_conversion::fixed_index(core::mem::size_of::<Sample>());
    let sample_bytes = i64::try_from(sample_size).map_err(|_| Rav1dError::EINVAL)?;
    let stride = self
      .stride_bytes
      .checked_div(sample_bytes)
      .ok_or(Rav1dError::EINVAL)?;
    let delta = position
      .y
      .checked_mul(stride)
      .and_then(|offset| offset.checked_add(position.x))
      .ok_or(Rav1dError::EINVAL)?;
    let offset = self
      .origin
      .checked_add_signed(delta)
      .ok_or(Rav1dError::EINVAL)?;
    let start =
      console_core_number_conversion::checked_index(offset).map_err(|_| Rav1dError::EINVAL)?;
    self.samples.get(start..).ok_or(Rav1dError::EINVAL)
  }
}

#[cfg(target_arch = "x86_64")]
use crate::src::mc::{WarpSampling, warp_coefficients, warp_vertical_sums, write_warp_predictor};

#[cfg(target_arch = "x86_64")]
#[rite]
fn warp_horizontal_dot<BD: BitDepth>(
  _token: Desktop64,
  window: &[u8],
  taps: &[i8; 8],
) -> Rav1dResult<i32> {
  use crate::include::common::bitdepth::BPC;
  let samples = match BD::BPC {
    BPC::BPC8 => {
      let input = window.first_chunk::<8>().ok_or(Rav1dError::EINVAL)?;
      let mut lanes = [0_u8; 16];
      let low = lanes.first_chunk_mut::<8>().ok_or(Rav1dError::EINVAL)?;
      *low = *input;
      let bytes = loadu_128!(&lanes, [u8; 16]);
      _mm_unpacklo_epi8(bytes, _mm_setzero_si128())
    }
    BPC::BPC16 => {
      let input = window.first_chunk::<16>().ok_or(Rav1dError::EINVAL)?;

      loadu_128!(input, [u8; 16])
    }
  };
  let coefficients = taps.map(i16::from);
  let lane_coefficients = loadu_128!(&coefficients, [i16; 8]);
  let products = _mm_madd_epi16(samples, lane_coefficients);
  let partial = _mm_hadd_epi32(products, products);
  Ok(_mm_cvtsi128_si32(_mm_hadd_epi32(partial, partial)))
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn warp_accumulate<BD: BitDepth>(
  token: Desktop64,
  source: PixelSource<'_, u8>,
  sampling: WarpSampling,
  depth: BD,
) -> Rav1dResult<[[i32; 8]; 8]> {
  use console_core_pixel_buffers::RowLayout;
  use console_core_video_interpolation::round_shift;
  let Ok(pixel_bytes) =
    console_core_number_conversion::fixed_index(core::mem::size_of::<BD::Pixel>());
  let byte_size = i64::try_from(pixel_bytes).map_err(|_| Rav1dError::EINVAL)?;
  let margin = source
    .stride_bytes
    .checked_mul(-3)
    .and_then(|rows| rows.checked_sub(byte_size.wrapping_mul(3)))
    .ok_or(Rav1dError::EINVAL)?;
  let start = source
    .origin
    .checked_add_signed(margin)
    .ok_or(Rav1dError::EINVAL)?;
  let size = Size {
    width: pixel_bytes.wrapping_mul(15),
    height: 15,
  };
  let layout =
    RowLayout::new(size, start, source.stride_bytes).map_err(|_| Rav1dError::EINVAL)?;
  let source_rows = layout
    .rows(source.samples)
    .map_err(|_| Rav1dError::EINVAL)?;
  let [alpha, beta, _, _] = sampling.coefficients.map(i32::from);
  let precision = depth.get_intermediate_bits();
  let horizontal_shift = 7_u32.wrapping_sub(u32::from(precision));
  let mut intermediate = [[0_i16; 8]; 15];

  for ((row, pixels), values) in (0_i32..).zip(source_rows).zip(&mut intermediate) {
    let vertical_advance = row.wrapping_mul(beta);
    let mut phase = sampling.phase.x.wrapping_add(vertical_advance);

    for (column, sample) in (0_u64..).zip(values) {
      let taps = warp_coefficients(phase)?;
      let Ok(offset) = console_core_number_conversion::index(column.wrapping_mul(pixel_bytes));
      let window = pixels.get(offset..).ok_or(Rav1dError::EINVAL)?;
      let sum = warp_horizontal_dot::<BD>(token, window, taps)?;
      let Ok(value) =
        console_core_number_conversion::wrapped::<_, i16>(round_shift(sum, horizontal_shift));
      *sample = value;
      phase = phase.wrapping_add(alpha);
    }
  }

  warp_vertical_sums(&intermediate, sampling)
}

#[cfg(target_arch = "x86_64")]
struct PixelDestination<'a> {
  bytes: &'a mut [u8],
  origin: u64,
  stride: i64,
}

#[cfg(target_arch = "x86_64")]
impl<'a> PixelDestination<'a> {
  fn forward(self) -> Result<(&'a mut [u8], u64), MotionFallback> {
    let Ok(start) = console_core_number_conversion::index(self.origin);
    let buffer = self
      .bytes
      .get_mut(start..)
      .ok_or(MotionFallback::BufferLayout)?;
    let stride = u64::try_from(self.stride).map_err(|_| MotionFallback::BufferLayout)?;

    Ok((buffer, stride))
  }
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn warp_put<BD: BitDepth>(
  token: Desktop64,
  source: PixelSource<'_, u8>,
  sampling: WarpSampling,
  output: PixelDestination<'_>,
  depth: BD,
) -> Rav1dResult {
  use console_core_pixel_buffers::RowLayout;
  use console_core_video_interpolation::round_shift;
  use zerocopy::IntoBytes;
  let Ok(pixel_size) =
    console_core_number_conversion::fixed_index(core::mem::size_of::<BD::Pixel>());
  let size = Size {
    width: pixel_size.wrapping_mul(8),
    height: 8,
  };
  let layout =
    RowLayout::new(size, output.origin, output.stride).map_err(|_| Rav1dError::EINVAL)?;
  let rows = layout
    .rows_mut(output.bytes)
    .map_err(|_| Rav1dError::EINVAL)?;
  let samples = warp_accumulate(token, source, sampling, depth)?;
  let shift = 7_u32.wrapping_add(u32::from(depth.get_intermediate_bits()));
  let Ok(pixel_bytes) = console_core_number_conversion::index(pixel_size);

  for (mut line, sums) in rows.zip(samples) {
    for sample in sums {
      let (target, rest) = line
        .split_at_mut_checked(pixel_bytes)
        .ok_or(Rav1dError::EINVAL)?;
      let clipped = depth.iclip_pixel(round_shift(sample, shift));
      let Ok(()) = copy_bilinear_pixels(target, clipped.as_bytes());
      line = rest;
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn warp_prep<BD: BitDepth>(
  token: Desktop64,
  source: PixelSource<'_, u8>,
  sampling: WarpSampling,
  output: &mut [i16],
  stride: u64,
  depth: BD,
) -> Rav1dResult {
  write_warp_predictor::<BD>(output, stride, || {
    warp_accumulate(token, source, sampling, depth)
  })
}

#[cfg(target_arch = "x86_64")]
pub(crate) fn warp8x8_dispatch<BD: BitDepth>(
  dst: PicOffset,
  src: PicOffset,
  sampling: WarpSampling,
  depth: BD,
) -> MotionDispatch {
  use zerocopy::IntoBytes;
  let Ok(avx2) = crate::src::cpu::summon_avx2();

  let token = match avx2 {
    Some(matched_token) => matched_token,
    ::core::option::Option::None => return Err(MotionFallback::MissingCpuFeature),
  };

  match { ::core::primitive::bool::then_some(dst.data.ref_eq(src.data), ()) } {
    ::core::option::Option::Some(()) => {
      return Err(MotionFallback::AliasedView);
    }
    ::core::option::Option::None => {}
  }

  let checked = (|| -> Rav1dResult {
    let Ok(stride) = console_core_number_conversion::fixed_offset(src.stride());
    let Ok(pixel_bytes) =
      console_core_number_conversion::fixed_index(core::mem::size_of::<BD::Pixel>());
    let (guard, base) = reference::warp_guard::<BD>(src)?;
    let origin = base.checked_mul(pixel_bytes).ok_or(Rav1dError::EINVAL)?;
    let reference = PixelSource {
      samples: guard.as_bytes(),
      origin,
      stride_bytes: stride,
    };
    crate::include::dav1d::picture::with_pixel_guard_mut::<BD, _>(
      &dst,
      8,
      8,
      |buffer, offset, pitch| {
        let Ok(origin_2) = console_core_number_conversion::fixed_index(offset);
        let Ok(stride_2) = console_core_number_conversion::fixed_offset(pitch);
        let output = PixelDestination {
          bytes: buffer,
          origin: origin_2,
          stride: stride_2,
        };
        warp_put(token, reference, sampling, output, depth)
      },
    )
  })();
  checked.map_err(|_invalid_layout| MotionFallback::BufferLayout)
}

#[cfg(target_arch = "x86_64")]
pub(crate) fn warp8x8t_dispatch<BD: BitDepth>(
  output: &mut [i16],
  pitch: u64,
  src: PicOffset,
  sampling: WarpSampling,
  depth: BD,
) -> MotionDispatch {
  use zerocopy::IntoBytes;
  let Ok(avx2) = crate::src::cpu::summon_avx2();

  let token = match avx2 {
    Some(matched_token) => matched_token,
    ::core::option::Option::None => return Err(MotionFallback::MissingCpuFeature),
  };

  let checked = (|target: &mut [i16]| -> Rav1dResult {
    let Ok(stride) = console_core_number_conversion::fixed_offset(src.stride());
    let Ok(pixel_bytes) =
      console_core_number_conversion::fixed_index(core::mem::size_of::<BD::Pixel>());
    let (guard, base) = reference::warp_guard::<BD>(src)?;
    let origin = base.checked_mul(pixel_bytes).ok_or(Rav1dError::EINVAL)?;
    let reference = PixelSource {
      samples: guard.as_bytes(),
      origin,
      stride_bytes: stride,
    };
    warp_prep(token, reference, sampling, target, pitch, depth)
  })(output);
  checked.map_err(|_invalid_layout| MotionFallback::BufferLayout)
}

#[cfg(all(test, target_arch = "x86_64"))]
mod tests {
  use super::*;

  #[test]
  fn word_bilinear_refuses_a_short_reference_before_writing() {
    let Ok(avx2) = crate::src::cpu::summon_avx2();
    let token = match avx2 {
      Some(available) => available,
      None => return,
    };
    let input = [117_u16; 128];
    let mut result = [711_u16; 128];
    let source = PixelSource {
      samples: &input,
      origin: 0,
      stride_bytes: 64,
    };
    let sampling = reference::Sampling {
      size: Size {
        width: 23,
        height: 4,
      },
      phase: Point { x: 7, y: 1 },
    };
    let request = BilinearRequest {
      destination: &mut result,
      stride: 32,
      source,
      sampling,
    };

    assert_eq!(
      put_bilin_16bpc_dispatch_inner(token, request, 1023),
      Err(Rav1dError::EINVAL)
    );
    assert!(result.iter().all(|&sample| sample == 711));
  }

  #[test]
  fn short_motion_destinations_are_rejected_before_writing() {
    use crate::include::common::bitdepth::BitDepth8;
    use crate::include::dav1d::picture::Rav1dPictureDataComponent;
    use crate::src::align::Align64;
    use aligned::Aligned;

    let _lock = archmage::testing::lock_token_testing();
    let Ok(avx2) = crate::src::cpu::summon_avx2();

    let _available = match avx2 {
      Some(matched_available) => matched_available,
      ::core::option::Option::None => return,
    };

    let mut reference: Align64<[u8; 4096]> = Aligned([17; 4096]);
    let mut short: Align64<[u8; 64]> = Aligned([39; 64]);
    let input = Rav1dPictureDataComponent::wrap_buf::<BitDepth8>(&mut *reference, 64);
    let output = Rav1dPictureDataComponent::wrap_buf::<BitDepth8>(&mut *short, 64);
    let source = PicOffset {
      data: &input,
      offset: 520,
    };
    let destination = PicOffset {
      data: &output,
      offset: 0,
    };
    let shape = Size {
      width: 8,
      height: 8,
    };
    let phase = Point { x: 3, y: 11 };
    let depth = BitDepth8::new(());
    let mut predictors = [41_i16; 32];

    assert_eq!(
      mc_put_dispatch(
        Filter2d::Regular8Tap,
        destination,
        source,
        shape,
        phase,
        depth
      ),
      Err(MotionFallback::BufferLayout),
    );
    assert_eq!(&*destination.slice::<BitDepth8>(64), &[39; 64]);
    assert_eq!(
      mct_prep_dispatch(
        Filter2d::Regular8Tap,
        &mut predictors,
        source,
        shape,
        phase,
        depth
      ),
      Err(MotionFallback::BufferLayout),
    );
    assert_eq!(predictors, [41; 32]);
  }

  #[test]
  fn a_short_mask_declines_the_blend_before_either_row_is_written() {
    let Ok(token) = crate::src::cpu::summon_avx2();
    let avx2 = match token {
      Some(avx2) => avx2,
      None => return,
    };
    let samples = [0i16; COMPINTER_LEN];
    let mut dst = [71u8; 131];
    let mask = [32u8; 127];

    assert_eq!(
      mask_8bpc_avx2_safe(
        avx2,
        &mut dst,
        67,
        &samples,
        &samples,
        Size {
          width: 64,
          height: 2
        },
        &mask
      ),
      Err(BlendLayoutError::Source),
    );
    assert_eq!(dst, [71; 131]);
  }

  #[test]
  fn high_bit_depth_blend_writes_vectors_and_tails_without_touching_padding() {
    use zerocopy::IntoBytes as _;

    let Ok(token) = crate::src::cpu::summon_avx2();
    let avx2 = match token {
      Some(avx2) => avx2,
      None => return,
    };
    let samples = [0i16; COMPINTER_LEN];
    let mut dst = [u16::MAX; 85];

    assert_eq!(
      avg_16bpc_avx2_safe(
        avx2,
        dst.as_mut_bytes(),
        88,
        &samples,
        &samples,
        Size {
          width: 41,
          height: 2
        },
        1023
      ),
      Ok(()),
    );

    let expect_avg = [512; 41].into_iter().chain([u16::MAX; 3]).chain([512; 41]);
    assert!(dst.into_iter().eq(expect_avg));

    let mask: [u8; 82] = core::array::from_fn(|ordinal| {
      let Ok(position) = console_core_number_conversion::fixed_index(ordinal);
      let Ok(alpha) = console_core_number_conversion::wrapped(position.wrapping_rem(65));
      alpha
    });

    let dark = [-8192i16; COMPINTER_LEN];
    let expect_mask = mask.map(|alpha| 512u16.wrapping_sub(u16::from(alpha).wrapping_mul(8)));
    let [first_row, second_row]: &[[u16; 41]; 2] = zerocopy::transmute_ref!(&expect_mask);
    let expected = first_row
      .iter()
      .copied()
      .chain([u16::MAX; 3])
      .chain(second_row.iter().copied());

    dst.fill(u16::MAX);
    assert_eq!(
      mask_16bpc_avx2_safe(
        avx2,
        dst.as_mut_bytes(),
        88,
        &dark,
        &samples,
        Size {
          width: 41,
          height: 2
        },
        &mask,
        1023
      ),
      Ok(()),
    );
    assert!(dst.into_iter().eq(expected.clone()));

    let Ok(server) = crate::src::cpu::summon_avx512();

    match server {
      Some(avx512) => {
        dst.fill(u16::MAX);
        assert_eq!(
          mask_16bpc_avx512_safe(
            avx512,
            dst.as_mut_bytes(),
            88,
            &dark,
            &samples,
            Size {
              width: 41,
              height: 2
            },
            &mask,
            1023
          ),
          Ok(()),
        );
        assert!(dst.into_iter().eq(expected));
      }
      ::core::option::Option::None => {}
    }
  }

  #[test]
  fn test_avg_8bpc_avx2_matches_scalar() {
    let Ok(avx2) = crate::src::cpu::summon_avx2();
    let token = match avx2 {
      Some(token) => token,
      None => {
        eprintln!("Skipping AVX2 test - CPU doesn't support it or tokens disabled");
        return;
      }
    };

    let test_values: [i16; 18] = [
      0,
      1,
      2,
      127,
      128,
      255,
      256,
      511,
      512,
      1023,
      1024,
      -1,
      -128,
      -256,
      -512,
      -1024,
      i16::MIN,
      i16::MAX,
    ];

    let w = 67i32;
    let h = 2i32;

    let mut tmp1 = [0i16; COMPINTER_LEN];
    let mut tmp2 = [0i16; COMPINTER_LEN];
    let mut dst_avx2 = [0_u8; 134];
    let mut dst_scalar = [0_u8; 134];

    for &v1 in &test_values {
      for &v2 in &test_values {
        let Ok(len) = console_core_number_conversion::wrapped_index(i32::wrapping_mul(w, h));

        for (a, b) in tmp1.iter_mut().zip(&mut tmp2).take(len) {
          *a = v1;
          *b = v2;
        }

        dst_avx2.fill(0);
        dst_scalar.fill(0);

        let golden = ::core::primitive::i32::wrapping_shr(
          i32::from(v1.wrapping_add(v2)).wrapping_add(16),
          5_u32,
        )
        .clamp(0, 255);
        let Ok(expected) = console_core_number_conversion::wrapped::<_, u8>(golden);
        dst_scalar.fill(expected);

        assert_eq!(
          avg_8bpc_avx2_safe(
            token,
            &mut dst_avx2,
            u64::from(w.cast_unsigned()),
            &tmp1,
            &tmp2,
            Size {
              width: w,
              height: h
            }
          ),
          Ok(())
        );

        assert_eq!(
          dst_avx2,
          dst_scalar,
          "Mismatch for v1={}, v2={}: avx2={:?} scalar={:?}",
          v1,
          v2,
          dst_avx2.first_chunk::<8>(),
          dst_scalar.first_chunk::<8>()
        );
      }
    }
  }

  #[test]
  fn test_avg_varying_data() {
    let Ok(avx2) = crate::src::cpu::summon_avx2();
    let token = match avx2 {
      Some(token) => token,
      None => {
        return;
      }
    };

    let w = 128i32;
    let h = 4i32;

    let mut tmp1 = [0i16; COMPINTER_LEN];
    let mut tmp2 = [0i16; COMPINTER_LEN];

    for (i, (a, b)) in tmp1
      .iter_mut()
      .zip(&mut tmp2)
      .take({
        let Ok(numeric_value) =
          console_core_number_conversion::wrapped_index(i32::wrapping_mul(w, h));
        numeric_value
      })
      .enumerate()
    {
      *a = {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(
          i.wrapping_mul(37).wrapping_rem(8192),
        );
        numeric_value
      };
      *b = {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(
          i.wrapping_mul(73).wrapping_add(1000).wrapping_rem(8192),
        );
        numeric_value
      };
    }

    let mut dst_a = [0_u8; 512];
    let mut dst_b = [0_u8; 512];

    assert_eq!(
      avg_8bpc_avx2_safe(
        token,
        &mut dst_a,
        u64::from(w.cast_unsigned()),
        &tmp1,
        &tmp2,
        Size {
          width: w,
          height: h
        }
      ),
      Ok(())
    );

    assert_eq!(
      avg_8bpc_avx2_safe(
        token,
        &mut dst_b,
        u64::from(w.cast_unsigned()),
        &tmp1,
        &tmp2,
        Size {
          width: w,
          height: h
        }
      ),
      Ok(())
    );

    assert_eq!(dst_a, dst_b, "Results differ for varying data");
  }

  #[test]
  fn test_w_avg_8bpc_avx2_matches_scalar() {
    let Ok(avx2) = crate::src::cpu::summon_avx2();
    let token = match avx2 {
      Some(token) => token,
      None => {
        eprintln!("Skipping AVX2 test - CPU doesn't support it or tokens disabled");
        return;
      }
    };

    let test_values: Vec<i16> = vec![
      0, 1, 127, 255, 512, 1023, 2047, 4095, 8191, -1, -128, -512, -1024,
    ];
    let test_weights = [0, 1, 4, 8, 12, 15, 16];

    let w = 67i32;
    let h = 2i32;

    let mut tmp1 = [0i16; COMPINTER_LEN];
    let mut tmp2 = [0i16; COMPINTER_LEN];
    let mut dst_a = [0_u8; 134];
    let mut dst_b = [0_u8; 134];

    for &weight in &test_weights {
      for &v1 in &test_values {
        for &v2 in &test_values {
          let Ok(len) = console_core_number_conversion::wrapped_index(i32::wrapping_mul(w, h));

          for (a, b) in tmp1.iter_mut().zip(&mut tmp2).take(len) {
            *a = v1;
            *b = v2;
          }

          dst_a.fill(0);
          dst_b.fill(0);

          assert_eq!(
            w_avg_8bpc_avx2_safe(
              token,
              &mut dst_a,
              u64::from(w.cast_unsigned()),
              &tmp1,
              &tmp2,
              Size {
                width: w,
                height: h
              },
              weight
            ),
            Ok(())
          );

          let (primary, secondary, factor) = match weight {
            8.. => (i32::from(v1), i32::from(v2), weight.wrapping_sub(16)),
            ..=7 => (i32::from(v2), i32::from(v1), weight.wrapping_neg()),
          };
          let adjustment = ::core::primitive::i32::wrapping_shr(
            primary.wrapping_sub(secondary).wrapping_mul(factor),
            4_u32,
          );
          let golden = ::core::primitive::i32::wrapping_shr(
            primary.wrapping_add(adjustment).wrapping_add(8),
            4_u32,
          )
          .clamp(0, 255);
          let Ok(expected) = console_core_number_conversion::wrapped::<_, u8>(golden);
          dst_b.fill(expected);

          assert_eq!(
            dst_a,
            dst_b,
            "Mismatch for weight={}, v1={}, v2={}: a={:?} b={:?}",
            weight,
            v1,
            v2,
            dst_a.first_chunk::<8>(),
            dst_b.first_chunk::<8>()
          );
        }
      }
    }
  }

  #[test]
  fn test_mask_8bpc_matches_scalar() {
    let Ok(avx2) = crate::src::cpu::summon_avx2();
    let token = match avx2 {
      Some(token) => token,
      None => {
        eprintln!("Skipping AVX2 test - CPU doesn't support it or tokens disabled");
        return;
      }
    };

    let test_values = [0, 127, 255, 512, 1023, 4095, -128, -512];
    let test_masks = [0, 1, 16, 32, 48, 63, 64];

    let w = 67i32;
    let h = 2i32;

    let mut tmp1 = [0i16; COMPINTER_LEN];
    let mut tmp2 = [0i16; COMPINTER_LEN];
    let mut mask = [0_u8; 134];
    let mut dst_a = [0_u8; 134];
    let mut dst_b = [0_u8; 134];

    for &m in &test_masks {
      for &v1 in &test_values {
        for &v2 in &test_values {
          let Ok(len) = console_core_number_conversion::wrapped_index(i32::wrapping_mul(w, h));

          for ((a, b), weight) in tmp1.iter_mut().zip(&mut tmp2).zip(&mut mask).take(len) {
            *a = v1;
            *b = v2;
            *weight = m;
          }

          dst_a.fill(0);
          dst_b.fill(0);

          assert_eq!(
            mask_8bpc_avx2_safe(
              token,
              &mut dst_a,
              u64::from(w.cast_unsigned()),
              &tmp1,
              &tmp2,
              Size {
                width: w,
                height: h
              },
              &mask
            ),
            Ok(())
          );

          let alpha = i32::from(m);
          let left = i32::from(v1).wrapping_mul(alpha);
          let right = i32::from(v2).wrapping_mul(64i32.wrapping_sub(alpha));
          let golden = ::core::primitive::i32::wrapping_shr(
            left.wrapping_add(right).wrapping_add(512),
            10_u32,
          )
          .clamp(0, 255);
          let Ok(expected) = console_core_number_conversion::wrapped::<_, u8>(golden);
          dst_b.fill(expected);

          assert_eq!(
            dst_a,
            dst_b,
            "Mismatch for mask={}, v1={}, v2={}: a={:?} b={:?}",
            m,
            v1,
            v2,
            dst_a.first_chunk::<8>(),
            dst_b.first_chunk::<8>()
          );
        }
      }
    }
  }

  #[test]
  fn test_avg_token_permutations() {
    use archmage::testing::{CompileTimePolicy, for_each_token_permutation};

    let w = 32i32;
    let h = 2i32;
    let Ok(size) = console_core_number_conversion::wrapped_index(i32::wrapping_mul(w, h));

    let mut tmp1 = [0i16; COMPINTER_LEN];
    let mut tmp2 = [0i16; COMPINTER_LEN];

    for (i, (a, b)) in tmp1.iter_mut().zip(&mut tmp2).take(size).enumerate() {
      *a = {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(
          i.wrapping_mul(37).wrapping_rem(4096),
        );
        numeric_value
      };
      *b = {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(
          i.wrapping_mul(73).wrapping_add(500).wrapping_rem(4096),
        );
        numeric_value
      };
    }

    let reference = {
      let Ok(avx2) = crate::src::cpu::summon_avx2();
      let token = match avx2 {
        Some(token) => token,
        None => {
          eprintln!("Skipping: AVX2 not available");
          return;
        }
      };
      let mut dst = [0_u8; 64];
      assert_eq!(
        avg_8bpc_avx2_safe(
          token,
          &mut dst,
          u64::from(w.cast_unsigned()),
          &tmp1,
          &tmp2,
          Size {
            width: w,
            height: h
          }
        ),
        Ok(())
      );
      dst
    };

    let report = for_each_token_permutation(CompileTimePolicy::WarnStderr, |perm| {
      let Ok(avx2) = crate::src::cpu::summon_avx2();

      match avx2 {
        Some(token) => {
          let mut dst = [0_u8; 64];
          assert_eq!(
            avg_8bpc_avx2_safe(
              token,
              &mut dst,
              u64::from(w.cast_unsigned()),
              &tmp1,
              &tmp2,
              Size {
                width: w,
                height: h
              }
            ),
            Ok(())
          );
          assert_eq!(dst, reference, "avg output mismatch at: {perm}");
        }
        None => {}
      }
    });
    eprintln!("MC avg permutations: {}", report.permutations_run);
    assert!(report.permutations_run >= 1);
  }
}
