//! Common NEON utilities for inverse transforms
//!
//! Shared constants, transpose, and add-to-destination helpers used by all
//! transform sizes. Ported from assembly macros in `src/arm/64/itx.S`.

#![allow(clippy::too_many_arguments)]
#![cfg_attr(not(feature = "unchecked"), forbid(unsafe_code))]
#![cfg_attr(feature = "unchecked", deny(unsafe_code))]

#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;

#[cfg(target_arch = "aarch64")]
use archmage::rite;

#[cfg(target_arch = "aarch64")]
use archmage::intrinsics::aarch64 as safe_simd;





/// IDCT coefficients: [2896, 2896*8, 1567, 3784, 799, 4017, 3406, 2276, ...]
pub(crate) const IDCT_COEFFS: [i16; 32] = [
  2896,
  ::console_core_number_conversion::const_wrapped_integer!(signed ::core::primitive::i32 => ::core::primitive::i16, 2896 * 8),
  1567,
  3784,

  799,
  4017,
  3406,
  2276,

  401,
  4076,
  3166,
  2598,

  1931,
  3612,
  3920,
  1189,

  201,
  4091,
  3035,
  2751,

  1751,
  3703,
  3857,
  1380,

  995,
  3973,
  3513,
  2106,

  2440,
  3290,
  4052,
  601,
];

/// IADST4 coefficients: [1321, 3803, 2482, 3344, 3344, 0]
/// Note: h[4..5] can be interpreted as s[2] = 3344 (32-bit)
pub(crate) const IADST4_COEFFS: [i16; 8] = [1321, 3803, 2482, 3344, 3344, 0, 0, 0];

/// The identity scale factor: (5793 - 4096) * 8 = 13576
/// Used by the identity transform: x * 5793/4096 via sqrdmulh + sqadd trick.
pub(crate) const IDENTITY_SCALE: i16 = ::console_core_number_conversion::const_wrapped_integer!(signed ::core::primitive::i32 => ::core::primitive::i16, (5793 - 4096) * 8);





/// 4x4 transpose for int16x4_t vectors.
///
/// Matches the `transpose_4x4h` macro from util.S lines 230-239.
/// Uses trn1/trn2 at 16-bit level, then trn1/trn2 at 32-bit level.
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
pub(crate) fn transpose_4x4h(
  r0: int16x4_t,
  r1: int16x4_t,
  r2: int16x4_t,
  r3: int16x4_t,
) -> (int16x4_t, int16x4_t, int16x4_t, int16x4_t) {

  let t4 = vtrn1_s16(r0, r1);
  let t5 = vtrn2_s16(r0, r1);
  let t6 = vtrn1_s16(r2, r3);
  let t7 = vtrn2_s16(r2, r3);


  let t4_32 = vreinterpret_s32_s16(t4);
  let t5_32 = vreinterpret_s32_s16(t5);
  let t6_32 = vreinterpret_s32_s16(t6);
  let t7_32 = vreinterpret_s32_s16(t7);

  let o0_32 = vtrn1_s32(t4_32, t6_32);
  let o2_32 = vtrn2_s32(t4_32, t6_32);
  let o1_32 = vtrn1_s32(t5_32, t7_32);
  let o3_32 = vtrn2_s32(t5_32, t7_32);

  let o0 = vreinterpret_s16_s32(o0_32);
  let o1 = vreinterpret_s16_s32(o1_32);
  let o2 = vreinterpret_s16_s32(o2_32);
  let o3 = vreinterpret_s16_s32(o3_32);

  (o0, o1, o2, o3)
}





/// Add transform output to destination pixels for 4x4 block (8bpc).
///
/// Mirrors the `L(itx_4x4_end)` label in itx.S lines 652-663.
/// The transform output is in 4 int16x4_t vectors (one per row).
/// We combine them into pairs of int16x8_t, apply rounding shift >>4,
/// add to u8 destination pixels with unsigned saturation, and store.
///
/// When `shift` is true, applies srshr>>4 (for non-WHT transforms).
/// WHT transforms skip the shift (it's already absorbed).
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
pub(crate) fn add_to_dst_4x4_8bpc(
  dst: &mut [u8],
  dst_base: usize,
  stride: isize,
  v16: int16x4_t,
  v17: int16x4_t,
  v18: int16x4_t,
  v19: int16x4_t,
  apply_shift: bool,
) {

  let v16_wide = vcombine_s16(v16, v17);
  let v18_wide = vcombine_s16(v18, v19);


  let v16_wide = match ::core::primitive::bool::then_some(apply_shift, ()) {
      ::core::option::Option::Some(()) => {
        vrshrq_n_s16::<4>(v16_wide)
      },
      ::core::option::Option::None => {
        v16_wide
      },
  };
  let v18_wide = match ::core::primitive::bool::then_some(apply_shift, ()) {
      ::core::option::Option::Some(()) => {
        vrshrq_n_s16::<4>(v18_wide)
      },
      ::core::option::Option::None => {
        v18_wide
      },
  };


  let row0_off = dst_base;
  let row1_off = dst_base.wrapping_add_signed(stride);
  let row2_off = dst_base.wrapping_add_signed(stride * 2);
  let row3_off = dst_base.wrapping_add_signed(stride * 3);


  let mut dst_bytes_01 = [0u8; 8];
  dst_bytes_01[0..4].copy_from_slice(&dst[row0_off..row0_off + 4]);
  dst_bytes_01[4..8].copy_from_slice(&dst[row1_off..row1_off + 4]);
  let v0 = safe_simd::vld1_u8(&dst_bytes_01);


  let mut dst_bytes_23 = [0u8; 8];
  dst_bytes_23[0..4].copy_from_slice(&dst[row2_off..row2_off + 4]);
  dst_bytes_23[4..8].copy_from_slice(&dst[row3_off..row3_off + 4]);
  let v1 = safe_simd::vld1_u8(&dst_bytes_23);


  let sum_01 = vreinterpretq_s16_u16(vaddw_u8(vreinterpretq_u16_s16(v16_wide), v0));
  let sum_23 = vreinterpretq_s16_u16(vaddw_u8(vreinterpretq_u16_s16(v18_wide), v1));


  let result_01 = vqmovun_s16(sum_01);
  let result_23 = vqmovun_s16(sum_23);


  let mut out_01 = [0u8; 8];
  safe_simd::vst1_u8(&mut out_01, result_01);
  dst[row0_off..row0_off + 4].copy_from_slice({ let [_explicit_part @ .., _, _, _, _] = &out_01; let _explicit_slice: &[_] = _explicit_part; _explicit_slice });
  dst[row1_off..row1_off + 4].copy_from_slice({ let [_, _, _, _, _explicit_part @ ..] = &out_01; let _explicit_slice: &[_] = _explicit_part; _explicit_slice });

  let mut out_23 = [0u8; 8];
  safe_simd::vst1_u8(&mut out_23, result_23);
  dst[row2_off..row2_off + 4].copy_from_slice({ let [_explicit_part @ .., _, _, _, _] = &out_23; let _explicit_slice: &[_] = _explicit_part; _explicit_slice });
  dst[row3_off..row3_off + 4].copy_from_slice({ let [_, _, _, _, _explicit_part @ ..] = &out_23; let _explicit_slice: &[_] = _explicit_part; _explicit_slice });
}
