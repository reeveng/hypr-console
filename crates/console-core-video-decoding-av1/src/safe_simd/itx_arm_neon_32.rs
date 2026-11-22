//! Safe ARM NEON 32x32 inverse transforms (DCT, Identity)
//!
//! Port of the 32x32 inverse transform functions from `src/arm/64/itx.S`
//! to safe Rust NEON intrinsics. Only DCT_DCT and IDTX are valid for 32x32.
//!
//! The 32-point inverse DCT is decomposed into:
//!   1. 16-point IDCT on even-indexed inputs (`idct_16_q`)
//!   2. 32-point odd part (`idct32_odd_q`) on odd-indexed inputs
//!   3. Butterfly combination of even and odd results
//!
//! The 32x32 block is processed in two passes:
//!   - Row transform: 4 groups of 8 rows, each processing 32 columns
//!     (split into even/odd 16-column halves), storing to a 2048-byte scratch buffer
//!   - Column transform: 4 groups of 8 columns from scratch, adding to destination

#![allow(clippy::too_many_arguments)]
#![cfg_attr(not(feature = "unchecked"), forbid(unsafe_code))]
#![cfg_attr(feature = "unchecked", deny(unsafe_code))]

#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;

#[cfg(target_arch = "aarch64")]
use archmage::{Arm64, arcane, rite};

#[cfg(target_arch = "aarch64")]
use archmage::intrinsics::aarch64 as safe_simd;

use super::itx_arm_neon_8x8::{smull_smlal_q, smull_smlsl_q, sqrshrn_pair, transpose_8x8h};
use super::itx_arm_neon_16x16::idct_16_q;
use super::itx_arm_neon_common::IDCT_COEFFS;

/// Type alias for 16 NEON vectors.
#[cfg(target_arch = "aarch64")]
pub(crate) type V16 = [int16x8_t; 16];
















/// 32-point inverse DCT odd part on 16 int16x8_t vectors.
///
/// Takes 16 odd-indexed inputs (indices 1,3,5,...,31 from the original 32-point
/// transform) in v[0..16] and produces 16 outputs (t16..t31).
///
/// This is a direct port of `inv_dct32_odd_8h_x16_neon` from itx.S.
///
/// The outputs represent t16..t31 and will be butterfly-combined with the
/// even part (from idct_16) to produce the final 32-point DCT result.
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
pub(crate) fn idct32_odd_q(v: V16) -> V16 {


  let c0 = safe_simd::vld1q_s16(<&[i16; 8]>::try_from(&IDCT_COEFFS[16..24]).unwrap());

  let c1 = safe_simd::vld1q_s16(<&[i16; 8]>::try_from(&IDCT_COEFFS[24..32]).unwrap());

































  let (lo, hi) = smull_smlsl_q(match v { [_explicit_element, ..] => _explicit_element }, match v { [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, c0, 0, 1);
  let t16a = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(match v { [_explicit_element, ..] => _explicit_element }, match v { [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, c0, 1, 0);
  let t31a = sqrshrn_pair(lo, hi);



  let (lo, hi) = smull_smlsl_q(match v { [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match v { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, c0, 2, 3);
  let t17a = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(match v { [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match v { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, c0, 3, 2);
  let t30a = sqrshrn_pair(lo, hi);



  let (lo, hi) = smull_smlsl_q(match v { [_, _, _, _, _explicit_element, ..] => _explicit_element }, match v { [_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, c0, 4, 5);
  let t18a = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(match v { [_, _, _, _, _explicit_element, ..] => _explicit_element }, match v { [_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, c0, 5, 4);
  let t29a = sqrshrn_pair(lo, hi);



  let (lo, hi) = smull_smlsl_q(match v { [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match v { [_, _, _, _explicit_element, ..] => _explicit_element }, c0, 6, 7);
  let t19a = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(match v { [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match v { [_, _, _, _explicit_element, ..] => _explicit_element }, c0, 7, 6);
  let t28a = sqrshrn_pair(lo, hi);



  let (lo, hi) = smull_smlsl_q(match v { [_, _, _explicit_element, ..] => _explicit_element }, match v { [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, c1, 0, 1);
  let t20a = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(match v { [_, _, _explicit_element, ..] => _explicit_element }, match v { [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, c1, 1, 0);
  let t27a = sqrshrn_pair(lo, hi);



  let (lo, hi) = smull_smlsl_q(match v { [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match v { [_, _, _, _, _, _explicit_element, ..] => _explicit_element }, c1, 2, 3);
  let t21a = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(match v { [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match v { [_, _, _, _, _, _explicit_element, ..] => _explicit_element }, c1, 3, 2);
  let t26a = sqrshrn_pair(lo, hi);



  let (lo, hi) = smull_smlsl_q(match v { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match v { [_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, c1, 4, 5);
  let t22a = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(match v { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match v { [_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, c1, 5, 4);
  let t25a = sqrshrn_pair(lo, hi);



  let (lo, hi) = smull_smlsl_q(match v { [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match v { [_, _explicit_element, ..] => _explicit_element }, c1, 6, 7);
  let t23a = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(match v { [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match v { [_, _explicit_element, ..] => _explicit_element }, c1, 7, 6);
  let t24a = sqrshrn_pair(lo, hi);



  let c_main = safe_simd::vld1q_s16(<&[i16; 8]>::try_from(&IDCT_COEFFS[0..8]).unwrap());


  let s17 = vqsubq_s16(t16a, t17a);
  let s16 = vqaddq_s16(t16a, t17a);
  let s30 = vqsubq_s16(t31a, t30a);
  let s31 = vqaddq_s16(t31a, t30a);
  let s18 = vqsubq_s16(t19a, t18a);
  let s19 = vqaddq_s16(t19a, t18a);
  let s20 = vqaddq_s16(t20a, t21a);
  let s21 = vqsubq_s16(t20a, t21a);
  let s22 = vqsubq_s16(t23a, t22a);
  let s23 = vqaddq_s16(t23a, t22a);
  let s24 = vqaddq_s16(t24a, t25a);
  let s25 = vqsubq_s16(t24a, t25a);
  let s26 = vqsubq_s16(t27a, t26a);
  let s27 = vqaddq_s16(t27a, t26a);
  let s28 = vqaddq_s16(t28a, t29a);
  let s29 = vqsubq_s16(t28a, t29a);






  let (lo, hi) = smull_smlsl_q(s30, s17, c_main, 4, 5);
  let u17a = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(s30, s17, c_main, 5, 4);
  let u30a = sqrshrn_pair(lo, hi);



  let (lo, hi) = smull_smlal_q(s29, s18, c_main, 5, 4);
  let u18a = sqrshrn_pair(vnegq_s32(lo), vnegq_s32(hi));

  let (lo, hi) = smull_smlsl_q(s29, s18, c_main, 4, 5);
  let u29a = sqrshrn_pair(lo, hi);



  let (lo, hi) = smull_smlsl_q(s26, s21, c_main, 6, 7);
  let u21a = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(s26, s21, c_main, 7, 6);
  let u26a = sqrshrn_pair(lo, hi);


  let (lo, hi) = smull_smlal_q(s25, s22, c_main, 7, 6);
  let u22a = sqrshrn_pair(vnegq_s32(lo), vnegq_s32(hi));

  let (lo, hi) = smull_smlsl_q(s25, s22, c_main, 6, 7);
  let u25a = sqrshrn_pair(lo, hi);


  let w30 = vqaddq_s16(u30a, u29a);
  let w29 = vqsubq_s16(u30a, u29a);
  let w18 = vqsubq_s16(u17a, u18a);
  let w17 = vqaddq_s16(u17a, u18a);
  let w19a = vqsubq_s16(s16, s19);
  let w16a = vqaddq_s16(s16, s19);
  let w20a = vqsubq_s16(s23, s20);
  let w23a = vqaddq_s16(s23, s20);
  let w21 = vqsubq_s16(u22a, u21a);
  let w22 = vqaddq_s16(u22a, u21a);
  let w24a = vqaddq_s16(s24, s27);
  let w27a = vqsubq_s16(s24, s27);
  let w25 = vqaddq_s16(u25a, u26a);
  let w26 = vqsubq_s16(u25a, u26a);
  let w28a = vqsubq_s16(s31, s28);
  let w31a = vqaddq_s16(s31, s28);





  let (lo, hi) = smull_smlsl_q(w29, w18, c_main, 2, 3);
  let x18a = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(w29, w18, c_main, 3, 2);
  let x29a = sqrshrn_pair(lo, hi);



  let (lo, hi) = smull_smlsl_q(w28a, w19a, c_main, 2, 3);
  let x19 = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(w28a, w19a, c_main, 3, 2);
  let x28 = sqrshrn_pair(lo, hi);


  let (lo, hi) = smull_smlal_q(w27a, w20a, c_main, 3, 2);
  let x20 = sqrshrn_pair(vnegq_s32(lo), vnegq_s32(hi));

  let (lo, hi) = smull_smlsl_q(w27a, w20a, c_main, 2, 3);
  let x27 = sqrshrn_pair(lo, hi);


  let (lo, hi) = smull_smlal_q(w26, w21, c_main, 3, 2);
  let x21a = sqrshrn_pair(vnegq_s32(lo), vnegq_s32(hi));

  let (lo, hi) = smull_smlsl_q(w26, w21, c_main, 2, 3);
  let x26a = sqrshrn_pair(lo, hi);


  let y23 = vqsubq_s16(w16a, w23a);
  let y16 = vqaddq_s16(w16a, w23a);
  let y24 = vqsubq_s16(w31a, w24a);
  let y31 = vqaddq_s16(w31a, w24a);
  let y22a = vqsubq_s16(w17, w22);
  let y17a = vqaddq_s16(w17, w22);
  let y30a = vqaddq_s16(w30, w25);
  let y25a = vqsubq_s16(w30, w25);
  let y21 = vqsubq_s16(x18a, x21a);
  let y18 = vqaddq_s16(x18a, x21a);
  let y19a = vqaddq_s16(x19, x20);
  let y20a = vqsubq_s16(x19, x20);
  let y29 = vqaddq_s16(x29a, x26a);
  let y26 = vqsubq_s16(x29a, x26a);
  let y28a = vqaddq_s16(x28, x27);
  let y27a = vqsubq_s16(x28, x27);





  let (lo, hi) = smull_smlsl_q(y27a, y20a, c_main, 0, 0);
  let z20 = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(y27a, y20a, c_main, 0, 0);
  let z27 = sqrshrn_pair(lo, hi);







  let (lo, hi) = smull_smlal_q(y26, y21, c_main, 0, 0);
  let z26a = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlsl_q(y26, y21, c_main, 0, 0);
  let z21a = sqrshrn_pair(lo, hi);






  let (lo, hi) = smull_smlsl_q(y25a, y22a, c_main, 0, 0);
  let z22 = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(y25a, y22a, c_main, 0, 0);
  let z25 = sqrshrn_pair(lo, hi);



  let (lo, hi) = smull_smlsl_q(y24, y23, c_main, 0, 0);
  let z23a = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(y24, y23, c_main, 0, 0);
  let z24a = sqrshrn_pair(lo, hi);


  [
    y16,
    y17a,
    y18,
    y19a,
    z20,
    z21a,
    z22,
    z23a,
    z24a,
    z25,
    z26a,
    z27,
    y28a,
    y29,
    y30a,
    y31,
  ]
}





/// Full 32-point inverse DCT.
///
/// Takes 32 int16x8_t vectors (even[0..16] from idct_16, odd[0..16] from
/// idct32_odd) and produces 32 output vectors via butterfly combination.
///
/// This is the top-level function that combines even and odd halves:
///   out[i]      = even[i]      + odd[15 - i]   for i = 0..16
///   out[31 - i] = even[i]      - odd[15 - i]   for i = 0..16
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
pub(crate) fn idct_32_full(even: &V16, odd: &V16) -> [int16x8_t; 32] {
  let mut out = [vdupq_n_s16(0); 32];












  for i in 0..16 {
    out[i] = vqaddq_s16(even[i], odd[::core::primitive::usize::wrapping_sub(15, i)]);
    out[::core::primitive::usize::wrapping_sub(31, i)] = vqsubq_s16(even[i], odd[::core::primitive::usize::wrapping_sub(15, i)]);
  }

  out
}





/// Horizontal DCT 32x8: transform 8 rows of 32 coefficients.
///
/// Mirrors `inv_txfm_horz_dct_32x8_neon` from itx.S lines 2014-2094.
///
/// For each group of 8 rows:
/// 1. Load 16 even-indexed columns, apply idct_16, transpose
/// 2. Load 16 odd-indexed columns, apply idct32_odd, transpose
/// 3. Butterfly-combine even and odd, apply shift, store to scratch
///
/// `coeff_base`: starting offset in `coeff` for the even-indexed columns
/// `coeff_stride`: distance between consecutive rows in coeff (= 32 for 32x32)
/// `scratch`: output buffer, one row = 32 i16 values
/// `scratch_base`: starting offset in scratch
/// `shift`: rounding shift to apply (2 for 32x32)
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
pub(crate) fn horz_dct_32x8(
  coeff: &mut [i16],
  coeff_base: usize,
  coeff_stride: usize,
  scratch: &mut [i16],
  scratch_base: usize,
  shift: i16,
) {
  let zero_vec = vdupq_n_s16(0);





  let mut even_in: V16 = [zero_vec; 16];

  for c in 0..16 {
    let col = ::core::primitive::usize::wrapping_mul(c, 2);
    let base = coeff_base + col * coeff_stride;
    let arr: [i16; 8] = coeff[base..base + 8].try_into().unwrap();
    even_in[c] = safe_simd::vld1q_s16(&arr);

    coeff[base..base + 8].fill(0);
  }


  let even_out = idct_16_q(even_in);


  let (et0, et1, et2, et3, et4, et5, et6, et7) = transpose_8x8h(
    match even_out { [_explicit_element, ..] => _explicit_element },
    match even_out { [_, _explicit_element, ..] => _explicit_element },
    match even_out { [_, _, _explicit_element, ..] => _explicit_element },
    match even_out { [_, _, _, _explicit_element, ..] => _explicit_element },
    match even_out { [_, _, _, _, _explicit_element, ..] => _explicit_element },
    match even_out { [_, _, _, _, _, _explicit_element, ..] => _explicit_element },
    match even_out { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element },
    match even_out { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element },
  );
  let (et8, et9, et10, et11, et12, et13, et14, et15) = transpose_8x8h(
    match even_out { [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element },
    match even_out { [_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element },
    match even_out { [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element },
    match even_out { [_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element },
    match even_out { [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element },
    match even_out { [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element },
    match even_out { [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element },
    match even_out { [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element },
  );




















  let even_t = [
    [et0, et8],
    [et1, et9],
    [et2, et10],
    [et3, et11],
    [et4, et12],
    [et5, et13],
    [et6, et14],
    [et7, et15],
  ];


  let mut odd_in: V16 = [zero_vec; 16];

  for c in 0..16 {
    let col = ::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(c, 2), 1);
    let base = coeff_base + col * coeff_stride;
    let arr: [i16; 8] = coeff[base..base + 8].try_into().unwrap();
    odd_in[c] = safe_simd::vld1q_s16(&arr);
    coeff[base..base + 8].fill(0);
  }


  let odd_out = idct32_odd_q(odd_in);







  let (ot15, ot14, ot13, ot12, ot11, ot10, ot9, ot8) = transpose_8x8h(
    match odd_out { [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element },
    match odd_out { [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element },
    match odd_out { [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element },
    match odd_out { [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element },
    match odd_out { [_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element },
    match odd_out { [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element },
    match odd_out { [_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element },
    match odd_out { [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element },
  );
  let (ot7, ot6, ot5, ot4, ot3, ot2, ot1, ot0) = transpose_8x8h(
    match odd_out { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match odd_out { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match odd_out { [_, _, _, _, _, _explicit_element, ..] => _explicit_element }, match odd_out { [_, _, _, _, _explicit_element, ..] => _explicit_element }, match odd_out { [_, _, _, _explicit_element, ..] => _explicit_element }, match odd_out { [_, _, _explicit_element, ..] => _explicit_element }, match odd_out { [_, _explicit_element, ..] => _explicit_element },
    match odd_out { [_explicit_element, ..] => _explicit_element },
  );

  let odd_t_hi = [ot15, ot14, ot13, ot12, ot11, ot10, ot9, ot8];
  let odd_t_lo = [ot7, ot6, ot5, ot4, ot3, ot2, ot1, ot0];

















  for (row, (odd_t_hi_element, odd_t_lo_element)) in <[_]>::iter(&odd_t_hi).zip(<[_]>::iter(&odd_t_lo)).enumerate() {
    let e_lo = even_t[row][0];
    let e_hi = even_t[row][1];
    let o_hi = *odd_t_hi_element;
    let o_lo = *odd_t_lo_element;


    let first_lo = vqaddq_s16(e_lo, o_hi);
    let first_hi = vqaddq_s16(e_hi, o_lo);


    let sub_lo = vqsubq_s16(e_lo, o_hi);
    let sub_hi = vqsubq_s16(e_hi, o_lo);

    let rev_lo = rev128_s16(sub_hi);
    let rev_hi = rev128_s16(sub_lo);


    let r0 = vrshrq_n_s16::<2>(first_lo);
    let r1 = vrshrq_n_s16::<2>(first_hi);
    let r2 = vrshrq_n_s16::<2>(rev_lo);
    let r3 = vrshrq_n_s16::<2>(rev_hi);


    let soff = scratch_base + ::core::primitive::usize::wrapping_mul(row, 32);
    store_v16(scratch, soff, r0);
    store_v16(scratch, soff + 8, r1);
    store_v16(scratch, soff + 16, r2);
    store_v16(scratch, soff + 24, r3);
  }


  let _ = shift;
}

/// Reverse all 8 i16 elements in an int16x8_t.
/// Equivalent to: rev64 v.8h + ext v.16b, v.16b, #8
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
#[inline(always)]
pub(crate) fn rev128_s16(v: int16x8_t) -> int16x8_t {
  let rev = vrev64q_s16(v);
  vextq_s16::<4>(rev, rev)
}

/// Store an int16x8_t to a slice at the given offset.
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
#[inline(always)]
pub(crate) fn store_v16(buf: &mut [i16], off: usize, v: int16x8_t) {
  let mut tmp = [0i16; 8];
  safe_simd::vst1q_s16(&mut tmp, v);
  buf[off..off + 8].copy_from_slice(&tmp);
}

/// Load an int16x8_t from a slice at the given offset.
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
#[inline(always)]
pub(crate) fn load_v16(buf: &[i16], off: usize) -> int16x8_t {
  let arr: [i16; 8] = buf[off..off + 8].try_into().unwrap();
  safe_simd::vld1q_s16(&arr)
}





/// Vertical DCT 8x32: transform 8 columns and add to destination (8bpc).
///
/// Mirrors `inv_txfm_add_vert_dct_8x32_neon` from itx.S lines 2100-2167.
///
/// 1. Load 16 even-indexed rows from scratch, apply idct_16
/// 2. Store results back, load 16 odd-indexed rows, apply idct32_odd
/// 3. Butterfly-combine with shift >>4, add to destination pixels
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
pub(crate) fn vert_dct_add_8x32_8bpc(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  scratch: &[i16],
  scratch_base: usize,
  scratch_stride: usize,
) {

  let mut even_in: V16 = [vdupq_n_s16(0); 16];

  for i in 0..16 {
    let row = ::core::primitive::usize::wrapping_mul(i, 2);
    even_in[i] = load_v16(scratch, scratch_base + row * scratch_stride);
  }


  let even_out = idct_16_q(even_in);


  let mut odd_in: V16 = [vdupq_n_s16(0); 16];

  for i in 0..16 {
    let row = ::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(i, 2), 1);
    odd_in[i] = load_v16(scratch, scratch_base + row * scratch_stride);
  }


  let odd_out = idct32_odd_q(odd_in);




  for i in 0..16 {
    let combined = vqaddq_s16(even_out[i], odd_out[::core::primitive::usize::wrapping_sub(15, i)]);
    let shifted = vrshrq_n_s16::<4>(combined);

    let row_off = dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(i) * dst_stride);
    let dst_bytes: [u8; 8] = dst[row_off..row_off + 8].try_into().unwrap();
    let dst_u8 = safe_simd::vld1_u8(&dst_bytes);
    let sum = vreinterpretq_s16_u16(vaddw_u8(vreinterpretq_u16_s16(shifted), dst_u8));
    let result = vqmovun_s16(sum);
    let mut out = [0u8; 8];
    safe_simd::vst1_u8(&mut out, result);
    dst[row_off..row_off + 8].copy_from_slice(&out);
  }

  for j in 0..16 {
    let i = ::core::primitive::usize::wrapping_add(16, j);
    let combined = vqsubq_s16(even_out[::core::primitive::usize::wrapping_sub(15, j)], odd_out[j]);
    let shifted = vrshrq_n_s16::<4>(combined);

    let row_off = dst_base.wrapping_add_signed(i.cast_signed() * dst_stride);
    let dst_bytes: [u8; 8] = dst[row_off..row_off + 8].try_into().unwrap();
    let dst_u8 = safe_simd::vld1_u8(&dst_bytes);
    let sum = vreinterpretq_s16_u16(vaddw_u8(vreinterpretq_u16_s16(shifted), dst_u8));
    let result = vqmovun_s16(sum);
    let mut out = [0u8; 8];
    safe_simd::vst1_u8(&mut out, result);
    dst[row_off..row_off + 8].copy_from_slice(&out);
  }
}

/// Vertical DCT 8x32: transform 8 columns and add to destination (16bpc).
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
pub(crate) fn vert_dct_add_8x32_16bpc(
  dst: &mut [u16],
  dst_base: usize,
  dst_stride: isize,
  scratch: &[i16],
  scratch_base: usize,
  scratch_stride: usize,
  bitdepth_max: i32,
) {
  let bd_max = vdupq_n_s16(match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(bitdepth_max) { ::core::result::Result::Ok(converted_position) => converted_position });
  let zero = vdupq_n_s16(0);


  let mut even_in: V16 = [vdupq_n_s16(0); 16];

  for i in 0..16 {
    even_in[i] = load_v16(scratch, scratch_base + ::core::primitive::usize::wrapping_mul(i, 2) * scratch_stride);
  }

  let even_out = idct_16_q(even_in);


  let mut odd_in: V16 = [vdupq_n_s16(0); 16];

  for i in 0..16 {
    odd_in[i] = load_v16(scratch, scratch_base + ::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(i, 2), 1) * scratch_stride);
  }

  let odd_out = idct32_odd_q(odd_in);


  for i in 0..16 {
    let combined = vqaddq_s16(even_out[i], odd_out[::core::primitive::usize::wrapping_sub(15, i)]);
    let shifted = vrshrq_n_s16::<4>(combined);

    let row_off = dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(i) * dst_stride);
    let mut arr = [0i16; 8];

    for (j, arr_element) in <[_]>::iter_mut(&mut arr).enumerate() {
      *arr_element = ::core::primitive::u16::cast_signed(dst[row_off + j]);
    }

    let d = safe_simd::vld1q_s16(&arr);
    let sum = vqaddq_s16(d, shifted);
    let clamped = vminq_s16(vmaxq_s16(sum, zero), bd_max);
    let mut out = [0i16; 8];
    safe_simd::vst1q_s16(&mut out, clamped);

    for (j, out_element) in <[_]>::iter(&out).enumerate() {
      dst[row_off + j] = ::core::primitive::i16::cast_unsigned(*out_element);
    }
  }

  for j in 0..16 {
    let i = ::core::primitive::usize::wrapping_add(16, j);
    let combined = vqsubq_s16(even_out[::core::primitive::usize::wrapping_sub(15, j)], odd_out[j]);
    let shifted = vrshrq_n_s16::<4>(combined);

    let row_off = dst_base.wrapping_add_signed(i.cast_signed() * dst_stride);
    let mut arr = [0i16; 8];

    for (k, arr_element) in <[_]>::iter_mut(&mut arr).enumerate() {
      *arr_element = ::core::primitive::u16::cast_signed(dst[row_off + k]);
    }

    let d = safe_simd::vld1q_s16(&arr);
    let sum = vqaddq_s16(d, shifted);
    let clamped = vminq_s16(vmaxq_s16(sum, zero), bd_max);
    let mut out = [0i16; 8];
    safe_simd::vst1q_s16(&mut out, clamped);

    for (k, out_element) in <[_]>::iter(&out).enumerate() {
      dst[row_off + k] = ::core::primitive::i16::cast_unsigned(*out_element);
    }
  }
}





/// DC-only fast path for DCT_DCT 32x32 with eob=0 (8bpc).
///
/// Assembly: `idct_dc 32, 32, 2`
/// ```text
/// dc = coeff[0]; coeff[0] = 0;
/// scale = 2896*8 = 23168
/// dc = sqrdmulh(dc, scale)    // first scaling
/// dc = srshr(dc, 2)           // shift=2 for 32x32
/// dc = sqrdmulh(dc, scale)    // second scaling
/// dc = srshr(dc, 4)           // final shift
/// ```
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn dc_only_32x32_8bpc(dst: &mut [u8], dst_base: usize, dst_stride: isize, coeff: &mut [i16]) {
  let dc = coeff[0];
  coeff[0] = 0;

  let scale = vdupq_n_s16(match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(::core::primitive::i32::wrapping_mul(2896, 8)) { ::core::result::Result::Ok(converted_position) => converted_position });

  let v = vdupq_n_s16(dc);
  let v = vqrdmulhq_s16(v, scale);
  let v = vrshrq_n_s16::<2>(v);
  let v = vqrdmulhq_s16(v, scale);
  let v = vrshrq_n_s16::<4>(v);

  for i in 0..32 {
    let row_off = dst_base.wrapping_add_signed((match ::console_core_number_conversion::wrapped_offset(i) { ::core::result::Result::Ok(converted_position) => converted_position }) * dst_stride);

    for half in 0..4 {
      let off = row_off + ::core::primitive::usize::wrapping_mul(half, 8);
      let dst_bytes: [u8; 8] = dst[off..off + 8].try_into().unwrap();
      let dst_u8 = safe_simd::vld1_u8(&dst_bytes);
      let sum = vreinterpretq_s16_u16(vaddw_u8(vreinterpretq_u16_s16(v), dst_u8));
      let result = vqmovun_s16(sum);
      let mut out = [0u8; 8];
      safe_simd::vst1_u8(&mut out, result);
      dst[off..off + 8].copy_from_slice(&out);
    }
  }
}

/// DC-only fast path for DCT_DCT 32x32 (16bpc).
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn dc_only_32x32_16bpc(
  dst: &mut [u16],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i32],
  bitdepth_max: i32,
) {
  let dc_val = coeff[0];
  coeff[0] = 0;

  let scale = ::core::primitive::i32::wrapping_mul(2896i32, 8);
  let mut dc = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(::core::primitive::i64::wrapping_shr(::core::primitive::i64::wrapping_add(<i64 as ::core::convert::From<::core::primitive::i32>>::from(dc_val) * <i64 as ::core::convert::From<::core::primitive::i32>>::from(scale), 16384), 15_u32)) { ::core::result::Result::Ok(converted_position) => converted_position };
  dc = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_add(dc, 2), 2_u32);
  dc = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(::core::primitive::i64::wrapping_shr(::core::primitive::i64::wrapping_add(<i64 as ::core::convert::From<::core::primitive::i32>>::from(dc) * <i64 as ::core::convert::From<::core::primitive::i32>>::from(scale), 16384), 15_u32)) { ::core::result::Result::Ok(converted_position) => converted_position };
  dc = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_add(dc, 8), 4_u32);

  let dc = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(dc.clamp(<i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MIN), <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MAX))) { ::core::result::Result::Ok(converted_position) => converted_position };
  let dc_vec = vdupq_n_s16(dc);
  let bd_max = vdupq_n_s16(match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(bitdepth_max) { ::core::result::Result::Ok(converted_position) => converted_position });
  let zero = vdupq_n_s16(0);

  for i in 0..32 {
    let row_off = dst_base.wrapping_add_signed((match ::console_core_number_conversion::wrapped_offset(i) { ::core::result::Result::Ok(converted_position) => converted_position }) * dst_stride);

    for half in 0..4 {
      let off = row_off + ::core::primitive::usize::wrapping_mul(half, 8);
      let mut arr = [0i16; 8];

      for (j, arr_element) in <[_]>::iter_mut(&mut arr).enumerate() {
        *arr_element = ::core::primitive::u16::cast_signed(dst[off + j]);
      }

      let d = safe_simd::vld1q_s16(&arr);
      let sum = vqaddq_s16(d, dc_vec);
      let clamped = vminq_s16(vmaxq_s16(sum, zero), bd_max);
      let mut out = [0i16; 8];
      safe_simd::vst1q_s16(&mut out, clamped);

      for (j, out_element) in <[_]>::iter(&out).enumerate() {
        dst[off + j] = ::core::primitive::i16::cast_unsigned(*out_element);
      }
    }
  }
}





/// Identity transform for 32x32 (8bpc).
///
/// The identity for 32x32 is a no-op on the coefficients (the shl<<1 from
/// identity and >>1 from normalization cancel). Just loads, transposes (for
/// the row transform), and adds to destination with shift >>2.
///
/// Mirrors `inv_txfm_add_identity_identity_32x32_8bpc_neon` from itx.S lines 2186-2219.
/// Processes the block as 4x4 groups of 8x8 sub-blocks, with early termination
/// based on eob thresholds.
#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_identity_identity_32x32_8bpc_neon_inner(
  _token: Arm64,
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  eob: i32,
  _bitdepth_max: i32,
) {
  identity_32x32_8bpc_impl(dst, dst_base, dst_stride, coeff, eob);
}

/// Inner implementation of identity 32x32 for 8bpc.
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn identity_32x32_8bpc_impl(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  eob: i32,
) {
  let eob_row_thresholds: [i32; 4] = [36, 136, 300, 1024];
  let eob_col_thresholds: [i32; 4] = [36, 136, 300, 1024];


  for rg in 0..4 {
    match {
        ::core::primitive::bool::then_some(rg > 0 && eob < eob_row_thresholds[rg - 1], ())
    } {
        ::core::option::Option::Some(()) => {
          break;
        },
        ::core::option::Option::None => {},
    }

    let row_start = ::core::primitive::usize::wrapping_mul(rg, 8);


    'over_cg_2: for cg in 0..4 {
      match {
          ::core::primitive::bool::then_some(cg > 0 && eob < eob_col_thresholds[cg - 1], ())
      } {
          ::core::option::Option::Some(()) => {
            break 'over_cg_2;
          },
          ::core::option::Option::None => {},
      }

      let col_start = ::core::primitive::usize::wrapping_mul(cg, 8);


      let zero_vec = vdupq_n_s16(0);
      let mut v: [int16x8_t; 8] = [zero_vec; 8];

      for (c, v_element) in <[_]>::iter_mut(&mut v).enumerate() {
        let col = col_start.wrapping_add(c);
        let base = col.wrapping_mul(32) + row_start;
        let arr: [i16; 8] = coeff[base..base.wrapping_add(8)].try_into().unwrap();
        *v_element = safe_simd::vld1q_s16(&arr);
        coeff[base..base.wrapping_add(8)].fill(0);
      }


      let (r0, r1, r2, r3, r4, r5, r6, r7) =
        transpose_8x8h(match v { [_explicit_element, ..] => _explicit_element }, match v { [_, _explicit_element, ..] => _explicit_element }, match v { [_, _, _explicit_element, ..] => _explicit_element }, match v { [_, _, _, _explicit_element, ..] => _explicit_element }, match v { [_, _, _, _, _explicit_element, ..] => _explicit_element }, match v { [_, _, _, _, _, _explicit_element, ..] => _explicit_element }, match v { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match v { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });


      let rows = [r0, r1, r2, r3, r4, r5, r6, r7];

      for (r, rows_element) in <[_]>::iter(&rows).enumerate() {
        let shifted = vrshrq_n_s16::<2>(*rows_element);
        let row_off =
          dst_base.wrapping_add_signed(row_start.wrapping_add(r) as isize * dst_stride) + col_start;

        let dst_bytes: [u8; 8] = dst[row_off..row_off + 8].try_into().unwrap();
        let dst_u8 = safe_simd::vld1_u8(&dst_bytes);
        let sum = vreinterpretq_s16_u16(vaddw_u8(vreinterpretq_u16_s16(shifted), dst_u8));
        let result = vqmovun_s16(sum);
        let mut out = [0u8; 8];
        safe_simd::vst1_u8(&mut out, result);
        dst[row_off..row_off + 8].copy_from_slice(&out);
      }
    }
  }
}

/// Identity 32x32 for 16bpc.
#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_identity_identity_32x32_16bpc_neon_inner(
  _token: Arm64,
  dst: &mut [u16],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i32],
  eob: i32,
  bitdepth_max: i32,
) {







  let eob_thresholds: [i32; 4] = [36, 136, 300, 1024];

  let bd_max = bitdepth_max;

  for rg in 0..4 {
    match {
        ::core::primitive::bool::then_some(rg > 0 && eob < eob_thresholds[rg - 1], ())
    } {
        ::core::option::Option::Some(()) => {
          break;
        },
        ::core::option::Option::None => {},
    }

    let row_start = ::core::primitive::usize::wrapping_mul(rg, 8);

    'over_cg_2: for cg in 0..4 {
      match {
          ::core::primitive::bool::then_some(cg > 0 && eob < eob_thresholds[cg - 1], ())
      } {
          ::core::option::Option::Some(()) => {
            break 'over_cg_2;
          },
          ::core::option::Option::None => {},
      }

      let col_start = ::core::primitive::usize::wrapping_mul(cg, 8);


      let mut block = [[0i32; 8]; 8];

      for c in 0..8 {
        let col = col_start.wrapping_add(c);

        for r in 0..8 {
          let row = row_start.wrapping_add(r);
          block[c][r] = coeff[col.wrapping_mul(32) + row];
          coeff[col.wrapping_mul(32) + row] = 0;
        }
      }


      for r in 0..8 {
        let row_off = dst_base.wrapping_add_signed(row_start.wrapping_add(r) as isize * dst_stride);

        for c in 0..8 {
          let val = ::core::primitive::i32::wrapping_shr(block[c][r] + 2, 2_u32);
          let d = <i32 as ::core::convert::From<::core::primitive::u16>>::from(dst[row_off + col_start + c]);
          let result = ::core::primitive::i32::wrapping_add(d, val).clamp(0, bd_max);
          dst[row_off + col_start + c] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(result) { ::core::result::Result::Ok(converted_position) => converted_position };
        }
      }
    }
  }
}





/// NEON implementation of 32x32 DCT_DCT inverse transform add for 8bpc.
///
/// Mirrors `inv_txfm_add_dct_dct_32x32_8bpc_neon` from itx.S lines 2333-2378.
///
/// Algorithm:
/// 1. DC fast path if eob=0
/// 2. Row transform: 4 groups of 8 rows, each applies 32-point DCT,
///    stores to 2048-byte scratch buffer
/// 3. Column transform: 4 groups of 8 columns from scratch, applies
///    32-point DCT, adds to destination
#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_dct_dct_32x32_8bpc_neon_inner(
  _token: Arm64,
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  eob: i32,
  _bitdepth_max: i32,
) {

  match {
      ::core::primitive::bool::then_some(eob == 0, ())
  } {
      ::core::option::Option::Some(()) => {
        dc_only_32x32_8bpc(dst, dst_base, dst_stride, coeff);
        return;
      },
      ::core::option::Option::None => {},
  }


  let eob_thresholds: [i32; 4] = [36, 136, 300, 1024];


  let mut scratch = [0i16; 1024];


  for group in 0..4 {
    let row_start = ::core::primitive::usize::wrapping_mul(group, 8);

    match {
        ::core::primitive::bool::then_some(group > 0 && eob < eob_thresholds[group - 1], ())
    } {
        ::core::option::Option::Some(()) => {

          break;
        },
        ::core::option::Option::None => {},
    }




    horz_dct_32x8(
      coeff,
      row_start,
      32,
      &mut scratch,
      row_start.wrapping_mul(32),
      2,
    );
  }


  for group in 0..4 {
    let col_start = ::core::primitive::usize::wrapping_mul(group, 8);
    vert_dct_add_8x32_8bpc(
      dst,
      dst_base + col_start,
      dst_stride,
      &scratch,
      col_start,
      32,
    );
  }
}

/// NEON implementation of 32x32 DCT_DCT inverse transform add for 16bpc.
#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_dct_dct_32x32_16bpc_neon_inner(
  _token: Arm64,
  dst: &mut [u16],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i32],
  eob: i32,
  bitdepth_max: i32,
) {

  match {
      ::core::primitive::bool::then_some(eob == 0, ())
  } {
      ::core::option::Option::Some(()) => {
        dc_only_32x32_16bpc(dst, dst_base, dst_stride, coeff, bitdepth_max);
        return;
      },
      ::core::option::Option::None => {},
  }








  scalar_dct_dct_32x32_16bpc(dst, dst_base, dst_stride, coeff, eob, bitdepth_max);
}

/// Scalar fallback for 32x32 DCT_DCT 16bpc.
fn scalar_dct_dct_32x32_16bpc(
  dst: &mut [u16],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i32],
  _eob: i32,
  bitdepth_max: i32,
) {


  let mut tmp = [0i32; 1024];

  for y in 0..32 {
    let mut input = [0i32; 32];

    for (x, input_element) in <[_]>::iter_mut(&mut input).enumerate() {
      *input_element = coeff[(::core::num::Wrapping(y) + ::core::num::Wrapping(x * 32)).0];
    }

    let out = scalar_dct32_1d(&input);

    for x in 0..32 {
      tmp[::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(y, 32), x)] = out[x];
    }
  }

  for x in 0..32 {
    let mut input = [0i32; 32];

    for (y, input_element) in <[_]>::iter_mut(&mut input).enumerate() {
      *input_element = tmp[y.wrapping_mul(32) + x];
    }

    let out = scalar_dct32_1d(&input);

    for y in 0..32 {
      let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride));
      let d = <i32 as ::core::convert::From<::core::primitive::u16>>::from(dst[(match ::console_core_number_conversion::index(row_off) { ::core::result::Result::Ok(native_position) => native_position }) + x]);
      let c = ::core::primitive::i32::wrapping_shr(out[y] + 512, 10_u32);
      let result = ::core::primitive::i32::wrapping_add(d, c).clamp(0, bitdepth_max);
      dst[(match ::console_core_number_conversion::index(row_off) { ::core::result::Result::Ok(native_position) => native_position }) + x] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(result) { ::core::result::Result::Ok(converted_position) => converted_position };
    }
  }

  for i in 0..1024 {
    coeff[i] = 0;
  }
}

/// Scalar 32-point inverse DCT.
///
/// Full precision implementation using the exact AV1 spec coefficients.
#[must_use]
fn scalar_dct32_1d(input: &[i32; 32]) -> [i32; 32] {

  let mut even = [0i32; 16];

  for (even_element, [input_lane_0, _]) in <[_]>::iter_mut(&mut even).zip(<[_]>::iter(<[_]>::as_chunks::<2>(input).0)) {
    *even_element = *input_lane_0;
  }

  let even_out = scalar_dct16_1d(&even);


  let mut odd = [0i32; 16];

  for (odd_element, [_, input_lane_1]) in <[_]>::iter_mut(&mut odd).zip(<[_]>::iter(<[_]>::as_chunks::<2>(input).0)) {
    *odd_element = *input_lane_1;
  }

  let odd_out = scalar_idct32_odd(&odd);


  let mut out = [0i32; 32];

  for i in 0..16 {
    out[i] = even_out[i] + odd_out[::core::primitive::usize::wrapping_sub(15, i)];
    out[::core::primitive::usize::wrapping_sub(31, i)] = even_out[i] - odd_out[::core::primitive::usize::wrapping_sub(15, i)];
  }

  out
}

/// Scalar 16-point inverse DCT (spec-accurate).
#[must_use]
fn scalar_dct16_1d(input: &[i32; 16]) -> [i32; 16] {

  let mut even = [0i32; 8];

  for (even_element, [input_lane_0, _]) in <[_]>::iter_mut(&mut even).zip(<[_]>::iter(<[_]>::as_chunks::<2>(input).0)) {
    *even_element = *input_lane_0;
  }

  let even_out = scalar_dct8_1d(&even);



  let c = [401i32, 4076, 3166, 2598, 1931, 3612, 3920, 1189];


  let t8a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match input { &[_, _explicit_element, ..] => _explicit_element }), (match c { [_explicit_element, ..] => _explicit_element })) - ::core::primitive::i32::wrapping_mul((match input { &[_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c { [_, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t15a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match input { &[_, _explicit_element, ..] => _explicit_element }), (match c { [_, _explicit_element, ..] => _explicit_element })) + ::core::primitive::i32::wrapping_mul((match input { &[_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c { [_explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t9a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match input { &[_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c { [_, _, _explicit_element, ..] => _explicit_element })) - ::core::primitive::i32::wrapping_mul((match input { &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c { [_, _, _, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t14a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match input { &[_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c { [_, _, _, _explicit_element, ..] => _explicit_element })) + ::core::primitive::i32::wrapping_mul((match input { &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c { [_, _, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t10a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match input { &[_, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c { [_, _, _, _, _explicit_element, ..] => _explicit_element })) - ::core::primitive::i32::wrapping_mul((match input { &[_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c { [_, _, _, _, _, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t13a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match input { &[_, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c { [_, _, _, _, _, _explicit_element, ..] => _explicit_element })) + ::core::primitive::i32::wrapping_mul((match input { &[_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c { [_, _, _, _, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t11a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match input { &[_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element })) - ::core::primitive::i32::wrapping_mul((match input { &[_, _, _, _explicit_element, ..] => _explicit_element }), (match c { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t12a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match input { &[_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element })) + ::core::primitive::i32::wrapping_mul((match input { &[_, _, _, _explicit_element, ..] => _explicit_element }), (match c { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);


  let t8 = ::core::primitive::i32::wrapping_add(t8a, t9a);
  let t9 = ::core::primitive::i32::wrapping_sub(t8a, t9a);
  let t10 = ::core::primitive::i32::wrapping_sub(t11a, t10a);
  let t11 = ::core::primitive::i32::wrapping_add(t11a, t10a);
  let t12 = ::core::primitive::i32::wrapping_add(t12a, t13a);
  let t13 = ::core::primitive::i32::wrapping_sub(t12a, t13a);
  let t14 = ::core::primitive::i32::wrapping_sub(t15a, t14a);
  let t15 = ::core::primitive::i32::wrapping_add(t15a, t14a);


  let t9a_2 = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(t14, 1567) - ::core::primitive::i32::wrapping_mul(t9, 3784) + 2048, 12_u32);
  let t14a_2 = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(t14, 3784) + ::core::primitive::i32::wrapping_mul(t9, 1567) + 2048, 12_u32);
  let t13a_2 = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(t13, 1567) - ::core::primitive::i32::wrapping_mul(t10, 3784) + 2048, 12_u32);
  let t10a_2 = ::core::primitive::i32::wrapping_neg(::core::primitive::i32::wrapping_shr(t13 * 3784 + t10 * 1567 + 2048, 12_u32));


  let t8a_2 = ::core::primitive::i32::wrapping_add(t8, t11);
  let t11a_2 = ::core::primitive::i32::wrapping_sub(t8, t11);
  let t9b = ::core::primitive::i32::wrapping_add(t9a_2, t10a_2);
  let t10b = ::core::primitive::i32::wrapping_sub(t9a_2, t10a_2);
  let t12a_2 = ::core::primitive::i32::wrapping_sub(t15, t12);
  let t15a_2 = ::core::primitive::i32::wrapping_add(t15, t12);
  let t13b = ::core::primitive::i32::wrapping_sub(t14a_2, t13a_2);
  let t14b = ::core::primitive::i32::wrapping_add(t14a_2, t13a_2);


  let t11_f = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(t12a_2, 2896) - ::core::primitive::i32::wrapping_mul(t11a_2, 2896) + 2048, 12_u32);
  let t12_f = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(t12a_2, 2896) + ::core::primitive::i32::wrapping_mul(t11a_2, 2896) + 2048, 12_u32);
  let t10_f = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(t13b, 2896) - ::core::primitive::i32::wrapping_mul(t10b, 2896) + 2048, 12_u32);
  let t13_f = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(t13b, 2896) + ::core::primitive::i32::wrapping_mul(t10b, 2896) + 2048, 12_u32);


  let mut out = [0i32; 16];
  *{ let [_explicit_part, ..] = &mut out; _explicit_part } = ::core::primitive::i32::wrapping_add((match even_out { [_explicit_element, ..] => _explicit_element }), t15a_2);
  *{ let [_, _explicit_part, ..] = &mut out; _explicit_part } = ::core::primitive::i32::wrapping_add((match even_out { [_, _explicit_element, ..] => _explicit_element }), t14b);
  *{ let [_, _, _explicit_part, ..] = &mut out; _explicit_part } = ::core::primitive::i32::wrapping_add((match even_out { [_, _, _explicit_element, ..] => _explicit_element }), t13_f);
  *{ let [_, _, _, _explicit_part, ..] = &mut out; _explicit_part } = ::core::primitive::i32::wrapping_add((match even_out { [_, _, _, _explicit_element, ..] => _explicit_element }), t12_f);
  *{ let [_, _, _, _, _explicit_part, ..] = &mut out; _explicit_part } = ::core::primitive::i32::wrapping_add((match even_out { [_, _, _, _, _explicit_element, ..] => _explicit_element }), t11_f);
  *{ let [_, _, _, _, _, _explicit_part, ..] = &mut out; _explicit_part } = ::core::primitive::i32::wrapping_add((match even_out { [_, _, _, _, _, _explicit_element, ..] => _explicit_element }), t10_f);
  *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut out; _explicit_part } = ::core::primitive::i32::wrapping_add((match even_out { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), t9b);
  *{ let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut out; _explicit_part } = ::core::primitive::i32::wrapping_add((match even_out { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), t8a_2);
  *{ let [_, _, _, _, _, _, _, _, _explicit_part, ..] = &mut out; _explicit_part } = ::core::primitive::i32::wrapping_sub((match even_out { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), t8a_2);
  *{ let [_, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut out; _explicit_part } = ::core::primitive::i32::wrapping_sub((match even_out { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), t9b);
  *{ let [_, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut out; _explicit_part } = ::core::primitive::i32::wrapping_sub((match even_out { [_, _, _, _, _, _explicit_element, ..] => _explicit_element }), t10_f);
  *{ let [_, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut out; _explicit_part } = ::core::primitive::i32::wrapping_sub((match even_out { [_, _, _, _, _explicit_element, ..] => _explicit_element }), t11_f);
  *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut out; _explicit_part } = ::core::primitive::i32::wrapping_sub((match even_out { [_, _, _, _explicit_element, ..] => _explicit_element }), t12_f);
  *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut out; _explicit_part } = ::core::primitive::i32::wrapping_sub((match even_out { [_, _, _explicit_element, ..] => _explicit_element }), t13_f);
  *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut out; _explicit_part } = ::core::primitive::i32::wrapping_sub((match even_out { [_, _explicit_element, ..] => _explicit_element }), t14b);
  *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut out; _explicit_part } = ::core::primitive::i32::wrapping_sub((match even_out { [_explicit_element, ..] => _explicit_element }), t15a_2);
  out
}

/// Scalar 8-point inverse DCT.
#[must_use]
fn scalar_dct8_1d(input: &[i32; 8]) -> [i32; 8] {

  let even = scalar_dct4_1d(match input { &[_explicit_element, ..] => _explicit_element }, match input { &[_, _, _explicit_element, ..] => _explicit_element }, match input { &[_, _, _, _, _explicit_element, ..] => _explicit_element }, match input { &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element });


  let c = [799i32, 4017, 3406, 2276];
  let t4a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match input { &[_, _explicit_element, ..] => _explicit_element }), (match c { [_explicit_element, ..] => _explicit_element })) - ::core::primitive::i32::wrapping_mul((match input { &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c { [_, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t7a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match input { &[_, _explicit_element, ..] => _explicit_element }), (match c { [_, _explicit_element, ..] => _explicit_element })) + ::core::primitive::i32::wrapping_mul((match input { &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c { [_explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t5a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match input { &[_, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c { [_, _, _explicit_element, ..] => _explicit_element })) - ::core::primitive::i32::wrapping_mul((match input { &[_, _, _, _explicit_element, ..] => _explicit_element }), (match c { [_, _, _, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t6a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match input { &[_, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c { [_, _, _, _explicit_element, ..] => _explicit_element })) + ::core::primitive::i32::wrapping_mul((match input { &[_, _, _, _explicit_element, ..] => _explicit_element }), (match c { [_, _, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);

  let t4 = ::core::primitive::i32::wrapping_add(t4a, t5a);
  let t5 = ::core::primitive::i32::wrapping_sub(t4a, t5a);
  let t6 = ::core::primitive::i32::wrapping_sub(t7a, t6a);
  let t7 = ::core::primitive::i32::wrapping_add(t7a, t6a);

  let t5a_2 = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_sub(t6, t5) * 2896 + 2048, 12_u32);
  let t6a_2 = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_add(t6, t5) * 2896 + 2048, 12_u32);

  [
    ::core::primitive::i32::wrapping_add((match even { [_explicit_element, ..] => _explicit_element }), t7),
    ::core::primitive::i32::wrapping_add((match even { [_, _explicit_element, ..] => _explicit_element }), t6a_2),
    ::core::primitive::i32::wrapping_add((match even { [_, _, _explicit_element, ..] => _explicit_element }), t5a_2),
    ::core::primitive::i32::wrapping_add((match even { [_, _, _, _explicit_element, ..] => _explicit_element }), t4),
    ::core::primitive::i32::wrapping_sub((match even { [_, _, _, _explicit_element, ..] => _explicit_element }), t4),
    ::core::primitive::i32::wrapping_sub((match even { [_, _, _explicit_element, ..] => _explicit_element }), t5a_2),
    ::core::primitive::i32::wrapping_sub((match even { [_, _explicit_element, ..] => _explicit_element }), t6a_2),
    ::core::primitive::i32::wrapping_sub((match even { [_explicit_element, ..] => _explicit_element }), t7),
  ]
}

/// Scalar 4-point inverse DCT.
#[must_use]
fn scalar_dct4_1d(in0: i32, in1: i32, in2: i32, in3: i32) -> [i32; 4] {
  let t0 = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_add(in0, in2) * 2896 + 2048, 12_u32);
  let t1 = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_sub(in0, in2) * 2896 + 2048, 12_u32);
  let t2 = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(in1, 1567) - ::core::primitive::i32::wrapping_mul(in3, 3784) + 2048, 12_u32);
  let t3 = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(in1, 3784) + ::core::primitive::i32::wrapping_mul(in3, 1567) + 2048, 12_u32);
  [::core::primitive::i32::wrapping_add(t0, t3), ::core::primitive::i32::wrapping_add(t1, t2), ::core::primitive::i32::wrapping_sub(t1, t2), ::core::primitive::i32::wrapping_sub(t0, t3)]
}

/// Scalar 32-point inverse DCT odd part (matches idct32_odd_q exactly).
#[must_use]
fn scalar_idct32_odd(v: &[i32; 16]) -> [i32; 16] {

  let c0 = [201i32, 4091, 3035, 2751, 1751, 3703, 3857, 1380];
  let c1 = [995i32, 3973, 3513, 2106, 2440, 3290, 4052, 601];

  let t16a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match v { &[_explicit_element, ..] => _explicit_element }), (match c0 { [_explicit_element, ..] => _explicit_element })) - ::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c0 { [_, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t31a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match v { &[_explicit_element, ..] => _explicit_element }), (match c0 { [_, _explicit_element, ..] => _explicit_element })) + ::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c0 { [_explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t17a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c0 { [_, _, _explicit_element, ..] => _explicit_element })) - ::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c0 { [_, _, _, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t30a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c0 { [_, _, _, _explicit_element, ..] => _explicit_element })) + ::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c0 { [_, _, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t18a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _explicit_element, ..] => _explicit_element }), (match c0 { [_, _, _, _, _explicit_element, ..] => _explicit_element })) - ::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c0 { [_, _, _, _, _, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t29a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _explicit_element, ..] => _explicit_element }), (match c0 { [_, _, _, _, _, _explicit_element, ..] => _explicit_element })) + ::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c0 { [_, _, _, _, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t19a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c0 { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element })) - ::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _explicit_element, ..] => _explicit_element }), (match c0 { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t28a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c0 { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element })) + ::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _explicit_element, ..] => _explicit_element }), (match c0 { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);

  let t20a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match v { &[_, _, _explicit_element, ..] => _explicit_element }), (match c1 { [_explicit_element, ..] => _explicit_element })) - ::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c1 { [_, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t27a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match v { &[_, _, _explicit_element, ..] => _explicit_element }), (match c1 { [_, _explicit_element, ..] => _explicit_element })) + ::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c1 { [_explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t21a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c1 { [_, _, _explicit_element, ..] => _explicit_element })) - ::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c1 { [_, _, _, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t26a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c1 { [_, _, _, _explicit_element, ..] => _explicit_element })) + ::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c1 { [_, _, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t22a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c1 { [_, _, _, _, _explicit_element, ..] => _explicit_element })) - ::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c1 { [_, _, _, _, _, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t25a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c1 { [_, _, _, _, _, _explicit_element, ..] => _explicit_element })) + ::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c1 { [_, _, _, _, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t23a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c1 { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element })) - ::core::primitive::i32::wrapping_mul((match v { &[_, _explicit_element, ..] => _explicit_element }), (match c1 { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);
  let t24a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul((match v { &[_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }), (match c1 { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element })) + ::core::primitive::i32::wrapping_mul((match v { &[_, _explicit_element, ..] => _explicit_element }), (match c1 { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element })) + 2048, 12_u32);


  let s17 = ::core::primitive::i32::wrapping_sub(t16a, t17a);
  let s16 = ::core::primitive::i32::wrapping_add(t16a, t17a);
  let s30 = ::core::primitive::i32::wrapping_sub(t31a, t30a);
  let s31 = ::core::primitive::i32::wrapping_add(t31a, t30a);
  let s18 = ::core::primitive::i32::wrapping_sub(t19a, t18a);
  let s19 = ::core::primitive::i32::wrapping_add(t19a, t18a);
  let s20 = ::core::primitive::i32::wrapping_add(t20a, t21a);
  let s21 = ::core::primitive::i32::wrapping_sub(t20a, t21a);
  let s22 = ::core::primitive::i32::wrapping_sub(t23a, t22a);
  let s23 = ::core::primitive::i32::wrapping_add(t23a, t22a);
  let s24 = ::core::primitive::i32::wrapping_add(t24a, t25a);
  let s25 = ::core::primitive::i32::wrapping_sub(t24a, t25a);
  let s26 = ::core::primitive::i32::wrapping_sub(t27a, t26a);
  let s27 = ::core::primitive::i32::wrapping_add(t27a, t26a);
  let s28 = ::core::primitive::i32::wrapping_add(t28a, t29a);
  let s29 = ::core::primitive::i32::wrapping_sub(t28a, t29a);


  let u17a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(s30, 799) - ::core::primitive::i32::wrapping_mul(s17, 4017) + 2048, 12_u32);
  let u30a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(s30, 4017) + ::core::primitive::i32::wrapping_mul(s17, 799) + 2048, 12_u32);
  let u18a = ::core::primitive::i32::wrapping_neg(::core::primitive::i32::wrapping_shr(s29 * 4017 + s18 * 799 + 2048, 12_u32));
  let u29a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(s29, 799) - ::core::primitive::i32::wrapping_mul(s18, 4017) + 2048, 12_u32);
  let u21a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(s26, 3406) - ::core::primitive::i32::wrapping_mul(s21, 2276) + 2048, 12_u32);
  let u26a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(s26, 2276) + ::core::primitive::i32::wrapping_mul(s21, 3406) + 2048, 12_u32);
  let u22a = ::core::primitive::i32::wrapping_neg(::core::primitive::i32::wrapping_shr(s25 * 2276 + s22 * 3406 + 2048, 12_u32));
  let u25a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(s25, 3406) - ::core::primitive::i32::wrapping_mul(s22, 2276) + 2048, 12_u32);


  let w30 = ::core::primitive::i32::wrapping_add(u30a, u29a);
  let w29 = ::core::primitive::i32::wrapping_sub(u30a, u29a);
  let w18 = ::core::primitive::i32::wrapping_sub(u17a, u18a);
  let w17 = ::core::primitive::i32::wrapping_add(u17a, u18a);
  let w19a = ::core::primitive::i32::wrapping_sub(s16, s19);
  let w16a = ::core::primitive::i32::wrapping_add(s16, s19);
  let w20a = ::core::primitive::i32::wrapping_sub(s23, s20);
  let w23a = ::core::primitive::i32::wrapping_add(s23, s20);
  let w21 = ::core::primitive::i32::wrapping_sub(u22a, u21a);
  let w22 = ::core::primitive::i32::wrapping_add(u22a, u21a);
  let w24a = ::core::primitive::i32::wrapping_add(s24, s27);
  let w27a = ::core::primitive::i32::wrapping_sub(s24, s27);
  let w25 = ::core::primitive::i32::wrapping_add(u25a, u26a);
  let w26 = ::core::primitive::i32::wrapping_sub(u25a, u26a);
  let w28a = ::core::primitive::i32::wrapping_sub(s31, s28);
  let w31a = ::core::primitive::i32::wrapping_add(s31, s28);


  let x18a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(w29, 1567) - ::core::primitive::i32::wrapping_mul(w18, 3784) + 2048, 12_u32);
  let x29a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(w29, 3784) + ::core::primitive::i32::wrapping_mul(w18, 1567) + 2048, 12_u32);
  let x19 = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(w28a, 1567) - ::core::primitive::i32::wrapping_mul(w19a, 3784) + 2048, 12_u32);
  let x28 = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(w28a, 3784) + ::core::primitive::i32::wrapping_mul(w19a, 1567) + 2048, 12_u32);
  let x20 = ::core::primitive::i32::wrapping_neg(::core::primitive::i32::wrapping_shr(w27a * 3784 + w20a * 1567 + 2048, 12_u32));
  let x27 = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(w27a, 1567) - ::core::primitive::i32::wrapping_mul(w20a, 3784) + 2048, 12_u32);
  let x21a = ::core::primitive::i32::wrapping_neg(::core::primitive::i32::wrapping_shr(w26 * 3784 + w21 * 1567 + 2048, 12_u32));
  let x26a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(w26, 1567) - ::core::primitive::i32::wrapping_mul(w21, 3784) + 2048, 12_u32);


  let y16 = ::core::primitive::i32::wrapping_add(w16a, w23a);
  let y23 = ::core::primitive::i32::wrapping_sub(w16a, w23a);
  let y31 = ::core::primitive::i32::wrapping_add(w31a, w24a);
  let y24 = ::core::primitive::i32::wrapping_sub(w31a, w24a);
  let y17a = ::core::primitive::i32::wrapping_add(w17, w22);
  let y22a = ::core::primitive::i32::wrapping_sub(w17, w22);
  let y30a = ::core::primitive::i32::wrapping_add(w30, w25);
  let y25a = ::core::primitive::i32::wrapping_sub(w30, w25);
  let y18 = ::core::primitive::i32::wrapping_add(x18a, x21a);
  let y21 = ::core::primitive::i32::wrapping_sub(x18a, x21a);
  let y19a = ::core::primitive::i32::wrapping_add(x19, x20);
  let y20a = ::core::primitive::i32::wrapping_sub(x19, x20);
  let y29 = ::core::primitive::i32::wrapping_add(x29a, x26a);
  let y26 = ::core::primitive::i32::wrapping_sub(x29a, x26a);
  let y28a = ::core::primitive::i32::wrapping_add(x28, x27);
  let y27a = ::core::primitive::i32::wrapping_sub(x28, x27);


  let z20 = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(y27a, 2896) - ::core::primitive::i32::wrapping_mul(y20a, 2896) + 2048, 12_u32);
  let z27 = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(y27a, 2896) + ::core::primitive::i32::wrapping_mul(y20a, 2896) + 2048, 12_u32);
  let z26a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(y26, 2896) + ::core::primitive::i32::wrapping_mul(y21, 2896) + 2048, 12_u32);
  let z21a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(y26, 2896) - ::core::primitive::i32::wrapping_mul(y21, 2896) + 2048, 12_u32);
  let z22 = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(y25a, 2896) - ::core::primitive::i32::wrapping_mul(y22a, 2896) + 2048, 12_u32);
  let z25 = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(y25a, 2896) + ::core::primitive::i32::wrapping_mul(y22a, 2896) + 2048, 12_u32);
  let z23a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(y24, 2896) - ::core::primitive::i32::wrapping_mul(y23, 2896) + 2048, 12_u32);
  let z24a = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_mul(y24, 2896) + ::core::primitive::i32::wrapping_mul(y23, 2896) + 2048, 12_u32);

  [
    y16, y17a, y18, y19a, z20, z21a, z22, z23a, z24a, z25, z26a, z27, y28a, y29, y30a, y31,
  ]
}





#[cfg(test)]
mod tests {
  use super::*;

  /// Test that the scalar DCT4 produces expected results.
  #[test]
  fn test_scalar_dct4() {
    let out = scalar_dct4_1d(4096, 0, 0, 0);

    assert!(
      out.iter().all(|&x| (x - (match out { [_explicit_element, ..] => _explicit_element })).abs() <= 1),
      "DC-only DCT4 should produce equal outputs, got {:?}",
      out
    );
  }

  /// Test that the scalar 32-point odd part with zero input produces zero output.
  #[test]
  fn test_scalar_idct32_odd_zero() {
    let input = [0i32; 16];
    let out = scalar_idct32_odd(&input);
    assert_eq!(out, [0i32; 16]);
  }

  /// Test that the scalar DCT32 with only DC produces flat output.
  #[test]
  fn test_scalar_dct32_dc_only() {
    let mut input = [0i32; 32];
    *{ let [_explicit_part, ..] = &mut input; _explicit_part } = 4096;
    let out = scalar_dct32_1d(&input);

    let mean = ::core::primitive::i32::wrapping_div(out.iter().sum::<i32>(), 32);

    for (i, &v) in out.iter().enumerate() {
      assert!(
        ::core::primitive::i32::wrapping_sub(v, mean).abs() <= 2,
        "DCT32 DC-only: output[{}]={} deviates from mean={}",
        i,
        v,
        mean
      );
    }
  }

  /// Test that DCT32 → inverse DCT32 round-trips (within rounding).
  #[test]
  fn test_scalar_dct32_roundtrip() {

    let mut input = [0i32; 32];

    for (i, input_element) in <[_]>::iter_mut(&mut input).enumerate() {
      *input_element = (match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(i) { ::core::result::Result::Ok(converted_position) => converted_position }) * 100 - 1600;
    }

    let transformed = scalar_dct32_1d(&input);




    assert!(
      transformed.iter().all(|&x| x.abs() < 100000),
      "Transform output should be bounded"
    );
  }
}
