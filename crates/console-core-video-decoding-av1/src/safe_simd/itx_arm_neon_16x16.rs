//! Safe ARM NEON 16x16 inverse transforms (DCT, ADST, flipADST, identity)
//!
//! Port of the 16x16 inverse transform functions from `src/arm/64/itx.S`
//! (lines 1033-1480) to safe Rust NEON intrinsics. Operates on 16 `int16x8_t`
//! vectors (8 elements each), processing 8 columns in parallel.
//!
//! Each transform half processes 16 `int16x8_t` vectors where:
//! - For the row transform: v[col] holds 8 row values at that column
//!   (8 row transforms in parallel across 16 columns)
//! - For the column transform: v[row] holds 8 column values at that row
//!   (8 column transforms in parallel across 16 rows)
//!
//! The row transform runs twice (rows 0-7, then rows 8-15), storing
//! transposed results to a 512-byte intermediate buffer. The column
//! transform then runs twice (cols 0-7, then cols 8-15), adding results
//! to the destination.

#![allow(clippy::too_many_arguments)]
#![cfg_attr(not(feature = "unchecked"), forbid(unsafe_code))]
#![cfg_attr(feature = "unchecked", deny(unsafe_code))]

#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;

#[cfg(target_arch = "aarch64")]
use archmage::{Arm64, arcane, rite};

#[cfg(target_arch = "aarch64")]
use archmage::intrinsics::aarch64 as safe_simd;

use super::itx_arm_neon_8x8::{
  idct_8_q, smull_smlal_q, smull_smlsl_q, sqrshrn_pair, transpose_8x8h,
};
use super::itx_arm_neon_common::IDCT_COEFFS;

/// Type alias for 16 NEON vectors (one full 16-point transform state).
#[cfg(target_arch = "aarch64")]
type V16 = [int16x8_t; 16];

/// IADST16 coefficients packed as two `int16x8_t` vectors.
///
/// v0: [4091, 201, 3973, 995, 3703, 1751, 3290, 2440]
/// v1: [2751, 3035, 2106, 3513, 1380, 3857, 601, 4052]
#[cfg(target_arch = "aarch64")]
pub(crate) const IADST16_COEFFS_V0: [i16; 8] = [4091, 201, 3973, 995, 3703, 1751, 3290, 2440];
#[cfg(target_arch = "aarch64")]
pub(crate) const IADST16_COEFFS_V1: [i16; 8] = [2751, 3035, 2106, 3513, 1380, 3857, 601, 4052];

/// 16-point inverse DCT on 16 int16x8_t vectors.
///
/// Calls `idct_8_q` on even-indexed inputs, then processes odd-indexed inputs
/// through butterfly stages. Uses IDCT coefficients from v0 and v1:
///
/// v0: [2896, 23168, 1567, 3784, 799, 4017, 3406, 2276]  (idct4+idct8)
/// v1: [401, 4076, 3166, 2598, 1931, 3612, 3920, 1189]    (idct16)
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
pub(crate) fn idct_16_q(v: V16) -> V16 {
  let (e0, e1, e2, e3, e4, e5, e6, e7) = idct_8_q(
    match v {
      [_explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _explicit_element,
        ..,
      ] => _explicit_element,
    },
  );

  let c1 = safe_simd::vld1q_s16(<&[i16; 8]>::try_from(&IDCT_COEFFS[8..16]).unwrap());

  let (lo, hi) = smull_smlsl_q(
    match v {
      [_, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _explicit_element,
        ..,
      ] => _explicit_element,
    },
    c1,
    0,
    1,
  );
  let t8a = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(
    match v {
      [_, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _explicit_element,
        ..,
      ] => _explicit_element,
    },
    c1,
    1,
    0,
  );
  let t15a = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(
    match v {
      [_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    c1,
    2,
    3,
  );
  let t9a = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(
    match v {
      [_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    c1,
    3,
    2,
  );
  let t14a = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(
    match v {
      [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    c1,
    4,
    5,
  );
  let t10a = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(
    match v {
      [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    c1,
    5,
    4,
  );
  let t13a = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(
    match v {
      [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _, _explicit_element, ..] => _explicit_element,
    },
    c1,
    6,
    7,
  );
  let t11a = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(
    match v {
      [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _, _explicit_element, ..] => _explicit_element,
    },
    c1,
    7,
    6,
  );
  let t12a = sqrshrn_pair(lo, hi);

  let t9 = vqsubq_s16(t8a, t9a);
  let t8 = vqaddq_s16(t8a, t9a);
  let t14 = vqsubq_s16(t15a, t14a);
  let t15 = vqaddq_s16(t15a, t14a);
  let t10 = vqsubq_s16(t11a, t10a);
  let t11 = vqaddq_s16(t11a, t10a);
  let t12 = vqaddq_s16(t12a, t13a);
  let t13 = vqsubq_s16(t12a, t13a);

  let c0 = safe_simd::vld1q_s16(<&[i16; 8]>::try_from(&IDCT_COEFFS[0..8]).unwrap());

  let (lo, hi) = smull_smlsl_q(t14, t9, c0, 2, 3);
  let t9a = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(t14, t9, c0, 3, 2);
  let t14a = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(t13, t10, c0, 2, 3);
  let t13a = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(t13, t10, c0, 3, 2);

  let t10a = sqrshrn_pair(vnegq_s32(lo), vnegq_s32(hi));

  let t11a = vqsubq_s16(t8, t11);
  let t8a = vqaddq_s16(t8, t11);
  let t12a = vqsubq_s16(t15, t12);
  let t15a = vqaddq_s16(t15, t12);
  let t9b = vqaddq_s16(t9a, t10a);
  let t10b = vqsubq_s16(t9a, t10a);
  let t13b = vqsubq_s16(t14a, t13a);
  let t14b = vqaddq_s16(t14a, t13a);

  let (lo, hi) = smull_smlsl_q(t12a, t11a, c0, 0, 0);
  let t11_final = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(t12a, t11a, c0, 0, 0);
  let t12_final = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(t13b, t10b, c0, 0, 0);
  let t10a_final = sqrshrn_pair(lo, hi);
  let (lo, hi) = smull_smlal_q(t13b, t10b, c0, 0, 0);
  let t13a_final = sqrshrn_pair(lo, hi);

  [
    vqaddq_s16(e0, t15a),
    vqaddq_s16(e1, t14b),
    vqaddq_s16(e2, t13a_final),
    vqaddq_s16(e3, t12_final),
    vqaddq_s16(e4, t11_final),
    vqaddq_s16(e5, t10a_final),
    vqaddq_s16(e6, t9b),
    vqaddq_s16(e7, t8a),
    vqsubq_s16(e7, t8a),
    vqsubq_s16(e6, t9b),
    vqsubq_s16(e5, t10a_final),
    vqsubq_s16(e4, t11_final),
    vqsubq_s16(e3, t12_final),
    vqsubq_s16(e2, t13a_final),
    vqsubq_s16(e1, t14b),
    vqsubq_s16(e0, t15a),
  ]
}

/// 16-point inverse ADST on 16 int16x8_t vectors.
///
/// Direct port of the `iadst_16` assembly macro. This is the most complex
/// transform, with 4 stages of butterfly operations.
///
/// Input mapping from assembly: v[0]..v[15] map to v16..v31
/// Output mapping: o[0]..o[15] map to the output indices.
///
/// For normal ADST, outputs are in natural order.
/// For flipADST, the caller reverses the output order.
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
pub(crate) fn iadst_16_q(v: V16) -> V16 {
  let c0 = safe_simd::vld1q_s16(<&[i16; 8]>::try_from(&IADST16_COEFFS_V0[..]).unwrap());
  let c1 = safe_simd::vld1q_s16(<&[i16; 8]>::try_from(&IADST16_COEFFS_V1[..]).unwrap());

  let (lo, hi) = smull_smlal_q(
    match v {
      [
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _explicit_element,
        ..,
      ] => _explicit_element,
    },
    match v {
      [_explicit_element, ..] => _explicit_element,
    },
    c0,
    0,
    1,
  );
  let t0 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(
    match v {
      [
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _explicit_element,
        ..,
      ] => _explicit_element,
    },
    match v {
      [_explicit_element, ..] => _explicit_element,
    },
    c0,
    1,
    0,
  );
  let t1 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(
    match v {
      [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _explicit_element, ..] => _explicit_element,
    },
    c0,
    2,
    3,
  );
  let t2 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(
    match v {
      [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _explicit_element, ..] => _explicit_element,
    },
    c0,
    3,
    2,
  );
  let t3 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(
    match v {
      [_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    c0,
    4,
    5,
  );
  let t4 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(
    match v {
      [_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    c0,
    5,
    4,
  );
  let t5 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(
    match v {
      [_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    c0,
    6,
    7,
  );
  let t6 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(
    match v {
      [_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    c0,
    7,
    6,
  );
  let t7 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(
    match v {
      [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    c1,
    0,
    1,
  );
  let t8 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(
    match v {
      [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    c1,
    1,
    0,
  );
  let t9 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(
    match v {
      [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    c1,
    2,
    3,
  );
  let t10 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(
    match v {
      [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    c1,
    3,
    2,
  );
  let t11 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(
    match v {
      [_, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    c1,
    4,
    5,
  );
  let t12 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(
    match v {
      [_, _, _, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    c1,
    5,
    4,
  );
  let t13 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(
    match v {
      [_, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _explicit_element,
        ..,
      ] => _explicit_element,
    },
    c1,
    6,
    7,
  );
  let t14 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(
    match v {
      [_, _explicit_element, ..] => _explicit_element,
    },
    match v {
      [
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _explicit_element,
        ..,
      ] => _explicit_element,
    },
    c1,
    7,
    6,
  );
  let t15 = sqrshrn_pair(lo, hi);

  let ci = safe_simd::vld1q_s16(<&[i16; 8]>::try_from(&IDCT_COEFFS[0..8]).unwrap());

  let s8a = vqsubq_s16(t0, t8);
  let s0a = vqaddq_s16(t0, t8);
  let s9a = vqsubq_s16(t1, t9);
  let s1a = vqaddq_s16(t1, t9);
  let s2a = vqaddq_s16(t2, t10);
  let s10a = vqsubq_s16(t2, t10);
  let s3a = vqaddq_s16(t3, t11);
  let s11a = vqsubq_s16(t3, t11);
  let s4a = vqaddq_s16(t4, t12);
  let s12a = vqsubq_s16(t4, t12);
  let s5a = vqaddq_s16(t5, t13);
  let s13a = vqsubq_s16(t5, t13);
  let s6a = vqaddq_s16(t6, t14);
  let s14a = vqsubq_s16(t6, t14);
  let s7a = vqaddq_s16(t7, t15);
  let s15a = vqsubq_s16(t7, t15);

  let (lo, hi) = smull_smlal_q(s8a, s9a, ci, 5, 4);
  let u8_ = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(s8a, s9a, ci, 4, 5);
  let u9 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(s10a, s11a, ci, 7, 6);
  let u10 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(s10a, s11a, ci, 6, 7);
  let u11 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(s13a, s12a, ci, 5, 4);
  let u12 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(s13a, s12a, ci, 4, 5);
  let u13 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(s15a, s14a, ci, 7, 6);
  let u14 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(s15a, s14a, ci, 6, 7);
  let u15 = sqrshrn_pair(lo, hi);

  let w4 = vqsubq_s16(s0a, s4a);
  let w0 = vqaddq_s16(s0a, s4a);
  let w5 = vqsubq_s16(s1a, s5a);
  let w1 = vqaddq_s16(s1a, s5a);
  let w2 = vqaddq_s16(s2a, s6a);
  let w6 = vqsubq_s16(s2a, s6a);
  let w3 = vqaddq_s16(s3a, s7a);
  let w7 = vqsubq_s16(s3a, s7a);
  let w8a = vqaddq_s16(u8_, u12);
  let w12a = vqsubq_s16(u8_, u12);
  let w9a = vqaddq_s16(u9, u13);
  let w13a = vqsubq_s16(u9, u13);
  let w10a = vqaddq_s16(u10, u14);
  let w14a = vqsubq_s16(u10, u14);
  let w11a = vqaddq_s16(u11, u15);
  let w15a = vqsubq_s16(u11, u15);

  let (lo, hi) = smull_smlal_q(w4, w5, ci, 3, 2);
  let x4a = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(w4, w5, ci, 2, 3);
  let x5a = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(w7, w6, ci, 3, 2);
  let x6a = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(w7, w6, ci, 2, 3);
  let x7a = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(w12a, w13a, ci, 3, 2);
  let x12 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(w12a, w13a, ci, 2, 3);
  let x13 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(w15a, w14a, ci, 3, 2);
  let x14 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(w15a, w14a, ci, 2, 3);
  let x15 = sqrshrn_pair(lo, hi);

  let t2a = vqsubq_s16(w0, w2);

  let o0 = vqaddq_s16(w0, w2);

  let t3a = vqsubq_s16(w1, w3);

  let o15 = vqnegq_s16(vqaddq_s16(w1, w3));

  let t15a = vqsubq_s16(x13, x15);

  let o13 = vqnegq_s16(vqaddq_s16(x13, x15));

  let o2 = vqaddq_s16(x12, x14);

  let t14a = vqsubq_s16(x12, x14);

  let o1 = vqnegq_s16(vqaddq_s16(w8a, w10a));

  let y10 = vqsubq_s16(w8a, w10a);

  let o14 = vqaddq_s16(w9a, w11a);

  let y11 = vqsubq_s16(w9a, w11a);

  let o3 = vqnegq_s16(vqaddq_s16(x4a, x6a));

  let y6 = vqsubq_s16(x4a, x6a);

  let o12 = vqaddq_s16(x5a, x7a);

  let y7 = vqsubq_s16(x5a, x7a);

  let (lo, hi) = smull_smlsl_q(t2a, t3a, ci, 0, 0);
  let o8 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(t2a, t3a, ci, 0, 0);
  let o7_pre = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(t14a, t15a, ci, 0, 0);
  let o5_pre = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(t14a, t15a, ci, 0, 0);
  let o10 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(y6, y7, ci, 0, 0);
  let o4 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(y6, y7, ci, 0, 0);
  let o11_pre = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(y10, y11, ci, 0, 0);
  let o6 = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(y10, y11, ci, 0, 0);
  let o9_pre = sqrshrn_pair(lo, hi);

  let o7 = vqnegq_s16(o7_pre);
  let o5 = vqnegq_s16(o5_pre);
  let o11 = vqnegq_s16(o11_pre);
  let o9 = vqnegq_s16(o9_pre);

  [
    o0, o1, o2, o3, o4, o5, o6, o7, o8, o9, o10, o11, o12, o13, o14, o15,
  ]
}

/// 16-point identity transform on 16 int16x8_t vectors.
///
/// For 16x16 blocks, the identity transform multiplies by 2*sqrt(2).
/// This is implemented as: x * 2 + (x * ((5793-4096)*2*8) + 0x4000) >> 15
///
/// Assembly:
/// ```text
/// mov w16, #2*(5793-4096)*8   // 27152
/// dup v0.4h, w16
/// sqrdmulh v2.8h, vi.8h, v0.h[0]
/// sqadd    vi.8h, vi.8h, vi.8h    // vi *= 2
/// sqadd    vi.8h, vi.8h, v2.8h    // vi += round(vi * scale)
/// ```
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
pub(crate) fn identity_16_q(mut v: V16) -> V16 {
  let scale = vdupq_n_s16(
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
      ::core::primitive::i32::wrapping_mul(
        ::core::primitive::i32::wrapping_sub(5793, 4096) * 2,
        8,
      ),
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    },
  );

  for vi in v.iter_mut() {
    let t = vqrdmulhq_s16(*vi, scale);
    *vi = vqaddq_s16(*vi, *vi);
    *vi = vqaddq_s16(*vi, t);
  }

  v
}

/// Transpose a 16x16 matrix stored as two halves.
///
/// The 16x16 block is split into:
///   top:    rows 0-7    (8 vectors)
///   bottom: rows 8-15   (8 vectors)
///
/// After transpose:
///   left:   columns 0-7  → new rows 0-7
///   right:  columns 8-15 → new rows 8-15
///
/// This uses 4 blocks of 8x8 transpose:
///   TL = transpose(top[0..8].lo)     → rows 0-7, cols 0-7
///   TR = transpose(top[0..8].hi)     → rows 0-7, cols 8-15
///   BL = transpose(bottom[0..8].lo)  → rows 8-15, cols 0-7
///   BR = transpose(bottom[0..8].hi)  → rows 8-15, cols 8-15
///
/// After transpose, swap TR and BL (off-diagonal blocks).
///
/// But actually, in the assembly the 16x16 is stored row-interleaved
/// across the two halves. The horizontal transform processes 16 values
/// per row, stored as two 8x8 groups: rows 0-7 of both halves are
/// processed, then rows 8-15.
///
/// For the 16x16 case specifically, the transpose happens on the
/// intermediate buffer, working with 16-element rows stored as
/// two consecutive int16x8_t vectors.
///
/// This function takes two arrays of 8 vectors each (representing
/// the top and bottom halves) and produces two new arrays.
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn transpose_16x16_half(
  a: [int16x8_t; 8],
  b: [int16x8_t; 8],
) -> ([int16x8_t; 8], [int16x8_t; 8]) {
  let (a0, a1, a2, a3, a4, a5, a6, a7) = transpose_8x8h(
    match a {
      [_explicit_element, ..] => _explicit_element,
    },
    match a {
      [_, _explicit_element, ..] => _explicit_element,
    },
    match a {
      [_, _, _explicit_element, ..] => _explicit_element,
    },
    match a {
      [_, _, _, _explicit_element, ..] => _explicit_element,
    },
    match a {
      [_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match a {
      [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match a {
      [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match a {
      [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  );
  let (b0, b1, b2, b3, b4, b5, b6, b7) = transpose_8x8h(
    match b {
      [_explicit_element, ..] => _explicit_element,
    },
    match b {
      [_, _explicit_element, ..] => _explicit_element,
    },
    match b {
      [_, _, _explicit_element, ..] => _explicit_element,
    },
    match b {
      [_, _, _, _explicit_element, ..] => _explicit_element,
    },
    match b {
      [_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match b {
      [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match b {
      [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match b {
      [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  );
  (
    [a0, a1, a2, a3, a4, a5, a6, a7],
    [b0, b1, b2, b3, b4, b5, b6, b7],
  )
}

/// Add 16 rows of 8 pixels to destination for 8bpc.
///
/// For each of 16 rows:
///   1. Apply rounding shift right by 4
///   2. Load 8 dst u8 pixels, zero-extend to u16
///   3. Add i16 transform result to u16 pixels
///   4. Saturate i16→u8
///   5. Store 8 bytes
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn add_to_dst_8x16_8bpc(dst: &mut [u8], dst_base: usize, stride: isize, v: V16) {
  for (i, &row) in v.iter().enumerate() {
    let row_off = dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(i) * stride);

    let shifted = vrshrq_n_s16::<4>(row);

    let dst_bytes: [u8; 8] = dst[row_off..row_off + 8].try_into().unwrap();
    let dst_u8 = safe_simd::vld1_u8(&dst_bytes);

    let sum = vreinterpretq_s16_u16(vaddw_u8(vreinterpretq_u16_s16(shifted), dst_u8));

    let result = vqmovun_s16(sum);

    let mut out = [0u8; 8];
    safe_simd::vst1_u8(&mut out, result);
    dst[row_off..row_off + 8].copy_from_slice(&out);
  }
}

/// Add 16 rows of 8 pixels to destination for 16bpc.
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn add_to_dst_8x16_16bpc(
  dst: &mut [u16],
  dst_base: usize,
  stride: isize,
  v: V16,
  bitdepth_max: i32,
) {
  let bd_max = vdupq_n_s16(
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(bitdepth_max) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    },
  );
  let zero = vdupq_n_s16(0);

  for (i, &row) in v.iter().enumerate() {
    let row_off = dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(i) * stride);

    let shifted = vrshrq_n_s16::<4>(row);

    let dst_arr: [i16; 8] = {
      let mut tmp = [0i16; 8];

      for (j, tmp_element) in <[_]>::iter_mut(&mut tmp).enumerate() {
        *tmp_element = ::core::primitive::u16::cast_signed(dst[row_off + j]);
      }

      tmp
    };
    let dst_vals = safe_simd::vld1q_s16(&dst_arr);

    let sum = vqaddq_s16(dst_vals, shifted);

    let clamped = vminq_s16(vmaxq_s16(sum, zero), bd_max);

    let mut out = [0i16; 8];
    safe_simd::vst1q_s16(&mut out, clamped);

    for (j, out_element) in <[_]>::iter(&out).enumerate() {
      dst[row_off + j] = ::core::primitive::i16::cast_unsigned(*out_element);
    }
  }
}

/// Row/column transform type for 16x16 blocks.
#[cfg(target_arch = "aarch64")]
#[derive(Clone, Copy)]
pub(crate) enum TxType16 {
  Dct,
  Adst,
  FlipAdst,
  Identity,
}

/// Apply a 1D 16-point transform to 16 int16x8_t vectors.
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn apply_tx16(tx: TxType16, v: V16) -> V16 {
  match tx {
    TxType16::Dct => idct_16_q(v),
    TxType16::Adst => iadst_16_q(v),
    TxType16::FlipAdst => {
      let mut out = iadst_16_q(v);

      out.reverse();
      out
    }
    TxType16::Identity => identity_16_q(v),
  }
}

/// DC-only fast path for DCT_DCT 16x16 with eob=0.
///
/// Algorithm (from assembly `idct_dc 16, 16, 2`):
/// ```text
/// dc = coeff[0]
/// coeff[0] = 0
/// scale = 2896*8 = 23168
/// dc = sqrdmulh(dc, scale)     // first scaling
/// dc = srshr(dc, 2)            // shift=2 for 16x16
/// dc = sqrdmulh(dc, scale)     // second scaling
/// dc = srshr(dc, 4)            // final shift
/// ```
/// Then broadcast to all 16x16 pixels and add.
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn dc_only_16x16_8bpc(dst: &mut [u8], dst_base: usize, dst_stride: isize, coeff: &mut [i16]) {
  let dc = coeff[0];
  coeff[0] = 0;

  let scale = vdupq_n_s16(
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
      ::core::primitive::i32::wrapping_mul(2896, 8),
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    },
  );

  let v = vdupq_n_s16(dc);
  let v = vqrdmulhq_s16(v, scale);
  let v = vrshrq_n_s16::<2>(v);
  let v = vqrdmulhq_s16(v, scale);
  let v = vrshrq_n_s16::<4>(v);

  for i in 0..16 {
    let row_off = dst_base.wrapping_add_signed(
      (match ::console_core_number_conversion::wrapped_offset(i) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      }) * dst_stride,
    );

    let dst_bytes: [u8; 8] = dst[row_off..row_off + 8].try_into().unwrap();
    let dst_u8 = safe_simd::vld1_u8(&dst_bytes);
    let sum = vreinterpretq_s16_u16(vaddw_u8(vreinterpretq_u16_s16(v), dst_u8));
    let result = vqmovun_s16(sum);
    let mut out = [0u8; 8];
    safe_simd::vst1_u8(&mut out, result);
    dst[row_off..row_off + 8].copy_from_slice(&out);

    let dst_bytes: [u8; 8] = dst[row_off + 8..row_off + 16].try_into().unwrap();
    let dst_u8 = safe_simd::vld1_u8(&dst_bytes);
    let sum = vreinterpretq_s16_u16(vaddw_u8(vreinterpretq_u16_s16(v), dst_u8));
    let result = vqmovun_s16(sum);
    let mut out = [0u8; 8];
    safe_simd::vst1_u8(&mut out, result);
    dst[row_off + 8..row_off + 16].copy_from_slice(&out);
  }
}

/// DC-only fast path for DCT_DCT 16x16 16bpc.
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn dc_only_16x16_16bpc(
  dst: &mut [u16],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i32],
  bitdepth_max: i32,
) {
  let dc_val = coeff[0];
  coeff[0] = 0;

  let scale = ::core::primitive::i32::wrapping_mul(2896i32, 8);
  let mut dc = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(
    ::core::primitive::i64::wrapping_shr(
      ::core::primitive::i64::wrapping_add(
        <i64 as ::core::convert::From<::core::primitive::i32>>::from(dc_val)
          * <i64 as ::core::convert::From<::core::primitive::i32>>::from(scale),
        16384,
      ),
      15_u32,
    ),
  ) {
    ::core::result::Result::Ok(converted_position) => converted_position,
  };
  dc = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_add(dc, 2), 2_u32);
  dc = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(
    ::core::primitive::i64::wrapping_shr(
      ::core::primitive::i64::wrapping_add(
        <i64 as ::core::convert::From<::core::primitive::i32>>::from(dc)
          * <i64 as ::core::convert::From<::core::primitive::i32>>::from(scale),
        16384,
      ),
      15_u32,
    ),
  ) {
    ::core::result::Result::Ok(converted_position) => converted_position,
  };
  dc = ::core::primitive::i32::wrapping_shr(::core::primitive::i32::wrapping_add(dc, 8), 4_u32);

  let dc = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(dc.clamp(
    <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MIN),
    <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MAX),
  )) {
    ::core::result::Result::Ok(converted_position) => converted_position,
  };
  let dc_vec = vdupq_n_s16(dc);
  let bd_max = vdupq_n_s16(
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(bitdepth_max) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    },
  );
  let zero = vdupq_n_s16(0);

  for i in 0..16 {
    let row_off = dst_base.wrapping_add_signed(
      (match ::console_core_number_conversion::wrapped_offset(i) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      }) * dst_stride,
    );

    for half in 0..2 {
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

/// `eob` below which a 16x16 transform's rows 8..15 are guaranteed zero.
///
/// From `src/arm/64/itx.S`, `def_fn_16x16 <txfm1>, <txfm2>, <eob_half>`:
/// 36 for every pairing of {dct, adst, flipadst} plus identity/identity, and
/// **8** for `dct, identity` (H_DCT) and `identity, dct` (V_DCT).
///
/// Being too LOW is always safe — it just transforms an all-zero row group,
/// which contributes nothing. Being too HIGH silently discards coefficients.
/// So the four pairings AV1 does not permit at 16x16 (H_ADST, V_ADST,
/// H_FLIPADST, V_FLIPADST — dav1d has no 16x16 kernel for them at all) take
/// the conservative 8 rather than an invented value.
#[cfg(target_arch = "aarch64")]
#[must_use]
fn eob_half_16x16(row_tx: TxType16, col_tx: TxType16) -> i32 {
  use TxType16::Identity;

  match (row_tx, col_tx) {
    (Identity, Identity) => 36,

    (Identity, _) | (_, Identity) => 8,
    _ => 36,
  }
}

/// NEON implementation of generic 16x16 inverse transform add for 8bpc.
///
/// Mirrors `inv_txfm_add_16x16_neon` from itx.S lines 1433-1464.
///
/// Algorithm:
/// 1. Process two 8-column halves of the 16x16 coefficient block:
///    a. Load 16 rows x 8 columns into V16
///    b. Clear coefficient buffer
///    c. Apply row transform (or identity)
///    d. For non-identity: shift right by 2
///    e. For identity: apply identity_8x16_shift2 (sqrdmulh + sshr + srhadd)
///    f. Transpose two 8x8 blocks within each half
///    g. Store interleaved to 512-byte temp buffer
///
/// 2. Process two 8-column halves of the transposed result:
///    a. Load 16 rows from temp buffer
///    b. Apply column transform
///    c. Add to destination
#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_16x16_8bpc_neon(
  _token: Arm64,
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  eob: i32,
  _bitdepth_max: i32,
  row_tx: TxType16,
  col_tx: TxType16,
) {
  match {
    ::core::primitive::bool::then_some(
      matches!(row_tx, TxType16::Dct) && matches!(col_tx, TxType16::Dct) && eob == 0,
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      dc_only_16x16_8bpc(dst, dst_base, dst_stride, coeff);
      return;
    }
    ::core::option::Option::None => {}
  }

  let is_identity_row = matches!(row_tx, TxType16::Identity);

  let mut tmp = [0i16; 256];

  let eob_half = eob_half_16x16(row_tx, col_tx);

  for half in 0..2usize {
    match { ::core::primitive::bool::then_some(half == 1 && eob < eob_half, ()) } {
      ::core::option::Option::Some(()) => {
        break;
      }
      ::core::option::Option::None => {}
    }

    let row_offset = half.wrapping_mul(8);

    let zero_vec = vdupq_n_s16(0);
    let mut v: V16 = [zero_vec; 16];

    for col in 0..16 {
      let base = ::core::primitive::usize::wrapping_add(
        ::core::primitive::usize::wrapping_mul(col, 16),
        row_offset,
      );
      let arr: [i16; 8] = coeff[base..base.wrapping_add(8)].try_into().unwrap();
      v[col] = safe_simd::vld1q_s16(&arr);
    }

    for col in 0..16 {
      let base = ::core::primitive::usize::wrapping_add(
        ::core::primitive::usize::wrapping_mul(col, 16),
        row_offset,
      );
      coeff[base..base.wrapping_add(8)].fill(0);
    }

    match ::core::primitive::bool::then_some(is_identity_row, ()) {
      ::core::option::Option::Some(()) => {
        let scale =
          vdupq_n_s16(::core::primitive::i32::wrapping_mul((5793 - 4096) * 2, 8) as i16);

        for vi in v.iter_mut() {
          let t = vqrdmulhq_s16(*vi, scale);
          let t = vshrq_n_s16::<1>(t);
          *vi = vrhaddq_s16(*vi, t);
        }
      }
      ::core::option::Option::None => {
        v = apply_tx16(row_tx, v);

        for vi in v.iter_mut() {
          *vi = vrshrq_n_s16::<2>(*vi);
        }
      }
    }

    let top = [
      match v {
        [_explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
    ];
    let bot = [
      match v {
        [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _explicit_element,
          ..,
        ] => _explicit_element,
      },
      match v {
        [
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _explicit_element,
          ..,
        ] => _explicit_element,
      },
    ];

    let (top_t, bot_t) = transpose_16x16_half(top, bot);

    for i in 0..8 {
      let row = half.wrapping_mul(8) + i;
      let mut arr = [0i16; 8];

      safe_simd::vst1q_s16(&mut arr, top_t[i]);
      tmp[row.wrapping_mul(16)..row.wrapping_mul(16) + 8].copy_from_slice(&arr);

      safe_simd::vst1q_s16(&mut arr, bot_t[i]);
      tmp[row.wrapping_mul(16) + 8..row.wrapping_mul(16) + 16].copy_from_slice(&arr);
    }
  }

  for half in 0..2usize {
    let col_offset = half.wrapping_mul(8);
    let zero_vec = vdupq_n_s16(0);
    let mut v: V16 = [zero_vec; 16];

    for row in 0..16 {
      let off = ::core::primitive::usize::wrapping_add(
        ::core::primitive::usize::wrapping_mul(row, 16),
        col_offset,
      );
      let arr: [i16; 8] = tmp[off..off.wrapping_add(8)].try_into().unwrap();
      v[row] = safe_simd::vld1q_s16(&arr);
    }

    v = apply_tx16(col_tx, v);

    add_to_dst_8x16_8bpc(dst, dst_base + col_offset, dst_stride, v);
  }
}

/// NEON implementation of generic 16x16 inverse transform add for 16bpc.
#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_16x16_16bpc_neon(
  _token: Arm64,
  dst: &mut [u16],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i32],
  eob: i32,
  bitdepth_max: i32,
  row_tx: TxType16,
  col_tx: TxType16,
) {
  match {
    ::core::primitive::bool::then_some(
      matches!(row_tx, TxType16::Dct) && matches!(col_tx, TxType16::Dct) && eob == 0,
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      dc_only_16x16_16bpc(dst, dst_base, dst_stride, coeff, bitdepth_max);
      return;
    }
    ::core::option::Option::None => {}
  }

  let is_identity_row = matches!(row_tx, TxType16::Identity);

  let mut tmp = [0i16; 256];

  let eob_half = eob_half_16x16(row_tx, col_tx);

  for half in 0..2usize {
    match { ::core::primitive::bool::then_some(half == 1 && eob < eob_half, ()) } {
      ::core::option::Option::Some(()) => {
        break;
      }
      ::core::option::Option::None => {}
    }

    let row_offset = half.wrapping_mul(8);

    let zero_vec = vdupq_n_s16(0);
    let mut v: V16 = [zero_vec; 16];

    for col in 0..16 {
      let mut arr = [0i16; 8];

      for (r, arr_element) in <[_]>::iter_mut(&mut arr).enumerate() {
        let c = coeff[::core::primitive::usize::wrapping_add(
          ::core::primitive::usize::wrapping_mul(col, 16),
          row_offset,
        ) + r];
        *arr_element =
          match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(c.clamp(
            <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MIN),
            <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MAX),
          )) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          };
      }

      v[col] = safe_simd::vld1q_s16(&arr);
    }

    for col in 0..16 {
      for r in 0..8 {
        coeff[::core::primitive::usize::wrapping_add(
          ::core::primitive::usize::wrapping_mul(col, 16),
          row_offset,
        ) + r] = 0;
      }
    }

    match ::core::primitive::bool::then_some(is_identity_row, ()) {
      ::core::option::Option::Some(()) => {
        let scale =
          vdupq_n_s16(::core::primitive::i32::wrapping_mul((5793 - 4096) * 2, 8) as i16);

        for vi in v.iter_mut() {
          let t = vqrdmulhq_s16(*vi, scale);
          let t = vshrq_n_s16::<1>(t);
          *vi = vrhaddq_s16(*vi, t);
        }
      }
      ::core::option::Option::None => {
        v = apply_tx16(row_tx, v);

        for vi in v.iter_mut() {
          *vi = vrshrq_n_s16::<2>(*vi);
        }
      }
    }

    let top = [
      match v {
        [_explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
    ];
    let bot = [
      match v {
        [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match v {
        [
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _explicit_element,
          ..,
        ] => _explicit_element,
      },
      match v {
        [
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _explicit_element,
          ..,
        ] => _explicit_element,
      },
    ];
    let (top_t, bot_t) = transpose_16x16_half(top, bot);

    for i in 0..8 {
      let row = half.wrapping_mul(8) + i;
      let mut arr = [0i16; 8];

      safe_simd::vst1q_s16(&mut arr, top_t[i]);
      tmp[row.wrapping_mul(16)..row.wrapping_mul(16) + 8].copy_from_slice(&arr);

      safe_simd::vst1q_s16(&mut arr, bot_t[i]);
      tmp[row.wrapping_mul(16) + 8..row.wrapping_mul(16) + 16].copy_from_slice(&arr);
    }
  }

  for half in 0..2usize {
    let col_offset = half.wrapping_mul(8);
    let zero_vec = vdupq_n_s16(0);
    let mut v: V16 = [zero_vec; 16];

    for row in 0..16 {
      let off = ::core::primitive::usize::wrapping_add(
        ::core::primitive::usize::wrapping_mul(row, 16),
        col_offset,
      );
      let arr: [i16; 8] = tmp[off..off.wrapping_add(8)].try_into().unwrap();
      v[row] = safe_simd::vld1q_s16(&arr);
    }

    v = apply_tx16(col_tx, v);

    let col_off = half.wrapping_mul(8);
    add_to_dst_8x16_16bpc(dst, dst_base + col_off, dst_stride, v, bitdepth_max);
  }
}

macro_rules! def_16x16_8bpc {
  ($name:ident, $row:expr, $col:expr) => {
    #[cfg(target_arch = "aarch64")]
    #[arcane]
    pub(crate) fn $name(
      token: Arm64,
      dst: &mut [u8],
      dst_base: usize,
      dst_stride: isize,
      coeff: &mut [i16],
      eob: i32,
      bitdepth_max: i32,
    ) {
      inv_txfm_add_16x16_8bpc_neon(
        token,
        dst,
        dst_base,
        dst_stride,
        coeff,
        eob,
        bitdepth_max,
        $row,
        $col,
      );
    }
  };
}

macro_rules! def_16x16_16bpc {
  ($name:ident, $row:expr, $col:expr) => {
    #[cfg(target_arch = "aarch64")]
    #[arcane]
    pub(crate) fn $name(
      token: Arm64,
      dst: &mut [u16],
      dst_base: usize,
      dst_stride: isize,
      coeff: &mut [i32],
      eob: i32,
      bitdepth_max: i32,
    ) {
      inv_txfm_add_16x16_16bpc_neon(
        token,
        dst,
        dst_base,
        dst_stride,
        coeff,
        eob,
        bitdepth_max,
        $row,
        $col,
      );
    }
  };
}

def_16x16_8bpc!(
  inv_txfm_add_dct_dct_16x16_8bpc_neon_inner,
  TxType16::Dct,
  TxType16::Dct
);
def_16x16_16bpc!(
  inv_txfm_add_dct_dct_16x16_16bpc_neon_inner,
  TxType16::Dct,
  TxType16::Dct
);

def_16x16_8bpc!(
  inv_txfm_add_identity_identity_16x16_8bpc_neon_inner,
  TxType16::Identity,
  TxType16::Identity
);
def_16x16_16bpc!(
  inv_txfm_add_identity_identity_16x16_16bpc_neon_inner,
  TxType16::Identity,
  TxType16::Identity
);

def_16x16_8bpc!(
  inv_txfm_add_adst_adst_16x16_8bpc_neon_inner,
  TxType16::Adst,
  TxType16::Adst
);
def_16x16_16bpc!(
  inv_txfm_add_adst_adst_16x16_16bpc_neon_inner,
  TxType16::Adst,
  TxType16::Adst
);

def_16x16_8bpc!(
  inv_txfm_add_dct_adst_16x16_8bpc_neon_inner,
  TxType16::Dct,
  TxType16::Adst
);
def_16x16_16bpc!(
  inv_txfm_add_dct_adst_16x16_16bpc_neon_inner,
  TxType16::Dct,
  TxType16::Adst
);

def_16x16_8bpc!(
  inv_txfm_add_adst_dct_16x16_8bpc_neon_inner,
  TxType16::Adst,
  TxType16::Dct
);
def_16x16_16bpc!(
  inv_txfm_add_adst_dct_16x16_16bpc_neon_inner,
  TxType16::Adst,
  TxType16::Dct
);

def_16x16_8bpc!(
  inv_txfm_add_dct_flipadst_16x16_8bpc_neon_inner,
  TxType16::Dct,
  TxType16::FlipAdst
);
def_16x16_16bpc!(
  inv_txfm_add_dct_flipadst_16x16_16bpc_neon_inner,
  TxType16::Dct,
  TxType16::FlipAdst
);

def_16x16_8bpc!(
  inv_txfm_add_flipadst_dct_16x16_8bpc_neon_inner,
  TxType16::FlipAdst,
  TxType16::Dct
);
def_16x16_16bpc!(
  inv_txfm_add_flipadst_dct_16x16_16bpc_neon_inner,
  TxType16::FlipAdst,
  TxType16::Dct
);

def_16x16_8bpc!(
  inv_txfm_add_flipadst_flipadst_16x16_8bpc_neon_inner,
  TxType16::FlipAdst,
  TxType16::FlipAdst
);
def_16x16_16bpc!(
  inv_txfm_add_flipadst_flipadst_16x16_16bpc_neon_inner,
  TxType16::FlipAdst,
  TxType16::FlipAdst
);

def_16x16_8bpc!(
  inv_txfm_add_adst_flipadst_16x16_8bpc_neon_inner,
  TxType16::Adst,
  TxType16::FlipAdst
);
def_16x16_16bpc!(
  inv_txfm_add_adst_flipadst_16x16_16bpc_neon_inner,
  TxType16::Adst,
  TxType16::FlipAdst
);

def_16x16_8bpc!(
  inv_txfm_add_flipadst_adst_16x16_8bpc_neon_inner,
  TxType16::FlipAdst,
  TxType16::Adst
);
def_16x16_16bpc!(
  inv_txfm_add_flipadst_adst_16x16_16bpc_neon_inner,
  TxType16::FlipAdst,
  TxType16::Adst
);

def_16x16_8bpc!(
  inv_txfm_add_dct_identity_16x16_8bpc_neon_inner,
  TxType16::Dct,
  TxType16::Identity
);
def_16x16_16bpc!(
  inv_txfm_add_dct_identity_16x16_16bpc_neon_inner,
  TxType16::Dct,
  TxType16::Identity
);

def_16x16_8bpc!(
  inv_txfm_add_identity_dct_16x16_8bpc_neon_inner,
  TxType16::Identity,
  TxType16::Dct
);
def_16x16_16bpc!(
  inv_txfm_add_identity_dct_16x16_16bpc_neon_inner,
  TxType16::Identity,
  TxType16::Dct
);

def_16x16_8bpc!(
  inv_txfm_add_adst_identity_16x16_8bpc_neon_inner,
  TxType16::Adst,
  TxType16::Identity
);
def_16x16_16bpc!(
  inv_txfm_add_adst_identity_16x16_16bpc_neon_inner,
  TxType16::Adst,
  TxType16::Identity
);

def_16x16_8bpc!(
  inv_txfm_add_identity_adst_16x16_8bpc_neon_inner,
  TxType16::Identity,
  TxType16::Adst
);
def_16x16_16bpc!(
  inv_txfm_add_identity_adst_16x16_16bpc_neon_inner,
  TxType16::Identity,
  TxType16::Adst
);

def_16x16_8bpc!(
  inv_txfm_add_flipadst_identity_16x16_8bpc_neon_inner,
  TxType16::FlipAdst,
  TxType16::Identity
);
def_16x16_16bpc!(
  inv_txfm_add_flipadst_identity_16x16_16bpc_neon_inner,
  TxType16::FlipAdst,
  TxType16::Identity
);

def_16x16_8bpc!(
  inv_txfm_add_identity_flipadst_16x16_8bpc_neon_inner,
  TxType16::Identity,
  TxType16::FlipAdst
);
def_16x16_16bpc!(
  inv_txfm_add_identity_flipadst_16x16_16bpc_neon_inner,
  TxType16::Identity,
  TxType16::FlipAdst
);

#[cfg(test)]
#[cfg(target_arch = "aarch64")]
mod tests {
  use super::*;
  use crate::include::common::intops::iclip;
  use archmage::SimdToken;

  /// Maximum per-pixel difference allowed between NEON and scalar.
  /// NEON matches the dav1d assembly (the reference); scalar Rust has known
  /// rounding differences that scale with coefficient magnitude and accumulate
  /// across stages. Larger transforms have more stages.
  const MAX_DIFF: i32 = 40;

  #[must_use]
  fn scalar_dct4_1d(input: &[i32; 4]) -> [i32; 4] {
    let t3a = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_add(
        (match input {
          &[_, _explicit_element, ..] => _explicit_element,
        }) * 3784,
        (match input {
          &[_, _, _, _explicit_element, ..] => _explicit_element,
        }) * 1567,
      ) + 2048,
      12_u32,
    );
    let t2a = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_sub(
        (match input {
          &[_, _explicit_element, ..] => _explicit_element,
        }) * 1567,
        (match input {
          &[_, _, _, _explicit_element, ..] => _explicit_element,
        }) * 3784,
      ) + 2048,
      12_u32,
    );
    let t0 = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_add(
        (match input {
          &[_explicit_element, ..] => _explicit_element,
        }) * 2896,
        (match input {
          &[_, _, _explicit_element, ..] => _explicit_element,
        }) * 2896,
      ) + 2048,
      12_u32,
    );
    let t1 = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_sub(
        (match input {
          &[_explicit_element, ..] => _explicit_element,
        }) * 2896,
        (match input {
          &[_, _, _explicit_element, ..] => _explicit_element,
        }) * 2896,
      ) + 2048,
      12_u32,
    );
    [
      ::core::primitive::i32::wrapping_add(t0, t3a),
      ::core::primitive::i32::wrapping_add(t1, t2a),
      ::core::primitive::i32::wrapping_sub(t1, t2a),
      ::core::primitive::i32::wrapping_sub(t0, t3a),
    ]
  }

  #[must_use]
  fn scalar_dct8_1d(input: &[i32; 8]) -> [i32; 8] {
    let even = [
      match input {
        &[_explicit_element, ..] => _explicit_element,
      },
      match input {
        &[_, _, _explicit_element, ..] => _explicit_element,
      },
      match input {
        &[_, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match input {
        &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
    ];
    let even_out = scalar_dct4_1d(&even);

    let t4a = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_sub(
        (match input {
          &[_, _explicit_element, ..] => _explicit_element,
        }) * 799,
        (match input {
          &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }) * 4017,
      ) + 2048,
      12_u32,
    );
    let t7a = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_add(
        (match input {
          &[_, _explicit_element, ..] => _explicit_element,
        }) * 4017,
        (match input {
          &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }) * 799,
      ) + 2048,
      12_u32,
    );
    let t5a = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_sub(
        (match input {
          &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }) * 3406,
        (match input {
          &[_, _, _, _explicit_element, ..] => _explicit_element,
        }) * 2276,
      ) + 2048,
      12_u32,
    );
    let t6a = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_add(
        (match input {
          &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }) * 2276,
        (match input {
          &[_, _, _, _explicit_element, ..] => _explicit_element,
        }) * 3406,
      ) + 2048,
      12_u32,
    );

    let t4 = ::core::primitive::i32::wrapping_add(t4a, t5a);
    let t5_tmp = ::core::primitive::i32::wrapping_sub(t4a, t5a);
    let t7 = ::core::primitive::i32::wrapping_add(t7a, t6a);
    let t6_tmp = ::core::primitive::i32::wrapping_sub(t7a, t6a);

    let t5 = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_sub(t6_tmp * 2896, t5_tmp * 2896) + 2048,
      12_u32,
    );
    let t6 = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_add(t6_tmp * 2896, t5_tmp * 2896) + 2048,
      12_u32,
    );

    [
      ::core::primitive::i32::wrapping_add(
        (match even_out {
          [_explicit_element, ..] => _explicit_element,
        }),
        t7,
      ),
      ::core::primitive::i32::wrapping_add(
        (match even_out {
          [_, _explicit_element, ..] => _explicit_element,
        }),
        t6,
      ),
      ::core::primitive::i32::wrapping_add(
        (match even_out {
          [_, _, _explicit_element, ..] => _explicit_element,
        }),
        t5,
      ),
      ::core::primitive::i32::wrapping_add(
        (match even_out {
          [_, _, _, _explicit_element, ..] => _explicit_element,
        }),
        t4,
      ),
      ::core::primitive::i32::wrapping_sub(
        (match even_out {
          [_, _, _, _explicit_element, ..] => _explicit_element,
        }),
        t4,
      ),
      ::core::primitive::i32::wrapping_sub(
        (match even_out {
          [_, _, _explicit_element, ..] => _explicit_element,
        }),
        t5,
      ),
      ::core::primitive::i32::wrapping_sub(
        (match even_out {
          [_, _explicit_element, ..] => _explicit_element,
        }),
        t6,
      ),
      ::core::primitive::i32::wrapping_sub(
        (match even_out {
          [_explicit_element, ..] => _explicit_element,
        }),
        t7,
      ),
    ]
  }

  #[must_use]
  fn scalar_dct16_1d(input: &[i32; 16]) -> [i32; 16] {
    let even: [i32; 8] = [
      match input {
        &[_explicit_element, ..] => _explicit_element,
      },
      match input {
        &[_, _, _explicit_element, ..] => _explicit_element,
      },
      match input {
        &[_, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match input {
        &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match input {
        &[_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match input {
        &[_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match input {
        &[_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      },
      match input {
        &[
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _explicit_element,
          ..,
        ] => _explicit_element,
      },
    ];
    let even_out = scalar_dct8_1d(&even);

    let t8a = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_sub(
        (match input {
          &[_, _explicit_element, ..] => _explicit_element,
        }) * 401,
        (match input {
          &[
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _explicit_element,
            ..,
          ] => _explicit_element,
        }) * 4076,
      ) + 2048,
      12_u32,
    );
    let t15a = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_add(
        (match input {
          &[_, _explicit_element, ..] => _explicit_element,
        }) * 4076,
        (match input {
          &[
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _explicit_element,
            ..,
          ] => _explicit_element,
        }) * 401,
      ) + 2048,
      12_u32,
    );
    let t9a = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_sub(
        (match input {
          &[_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }) * 3166,
        (match input {
          &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }) * 2598,
      ) + 2048,
      12_u32,
    );
    let t14a = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_add(
        (match input {
          &[_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }) * 2598,
        (match input {
          &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }) * 3166,
      ) + 2048,
      12_u32,
    );
    let t10a = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_sub(
        (match input {
          &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }) * 1931,
        (match input {
          &[_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }) * 3612,
      ) + 2048,
      12_u32,
    );
    let t13a = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_add(
        (match input {
          &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }) * 3612,
        (match input {
          &[_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }) * 1931,
      ) + 2048,
      12_u32,
    );
    let t11a = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_sub(
        (match input {
          &[_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }) * 3920,
        (match input {
          &[_, _, _, _explicit_element, ..] => _explicit_element,
        }) * 1189,
      ) + 2048,
      12_u32,
    );
    let t12a = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_add(
        (match input {
          &[_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }) * 1189,
        (match input {
          &[_, _, _, _explicit_element, ..] => _explicit_element,
        }) * 3920,
      ) + 2048,
      12_u32,
    );

    let t9 = ::core::primitive::i32::wrapping_sub(t8a, t9a);
    let t8 = ::core::primitive::i32::wrapping_add(t8a, t9a);
    let t14 = ::core::primitive::i32::wrapping_sub(t15a, t14a);
    let t15 = ::core::primitive::i32::wrapping_add(t15a, t14a);
    let t10 = ::core::primitive::i32::wrapping_sub(t11a, t10a);
    let t11 = ::core::primitive::i32::wrapping_add(t11a, t10a);
    let t12 = ::core::primitive::i32::wrapping_add(t12a, t13a);
    let t13 = ::core::primitive::i32::wrapping_sub(t12a, t13a);

    let t9a_2 = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_sub(t14 * 1567, t9 * 3784) + 2048,
      12_u32,
    );
    let t14a_2 = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_add(t14 * 3784, t9 * 1567) + 2048,
      12_u32,
    );
    let t13a_2 = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_sub(t13 * 1567, t10 * 3784) + 2048,
      12_u32,
    );
    let t10a_2 = ::core::primitive::i32::wrapping_neg(::core::primitive::i32::wrapping_shr(
      (t13 * 3784 + t10 * 1567) + 2048,
      12_u32,
    ));

    let t11a_2 = ::core::primitive::i32::wrapping_sub(t8, t11);
    let t8a_2 = ::core::primitive::i32::wrapping_add(t8, t11);
    let t12a_2 = ::core::primitive::i32::wrapping_sub(t15, t12);
    let t15a_2 = ::core::primitive::i32::wrapping_add(t15, t12);
    let t9b = ::core::primitive::i32::wrapping_add(t9a_2, t10a_2);
    let t10b = ::core::primitive::i32::wrapping_sub(t9a_2, t10a_2);
    let t13b = ::core::primitive::i32::wrapping_sub(t14a_2, t13a_2);
    let t14b = ::core::primitive::i32::wrapping_add(t14a_2, t13a_2);

    let t11f = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_sub(t12a_2 * 2896, t11a_2 * 2896) + 2048,
      12_u32,
    );
    let t12f = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_add(t12a_2 * 2896, t11a_2 * 2896) + 2048,
      12_u32,
    );
    let t10af = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_sub(t13b * 2896, t10b * 2896) + 2048,
      12_u32,
    );
    let t13af = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_add(t13b * 2896, t10b * 2896) + 2048,
      12_u32,
    );

    [
      ::core::primitive::i32::wrapping_add(
        (match even_out {
          [_explicit_element, ..] => _explicit_element,
        }),
        t15a_2,
      ),
      ::core::primitive::i32::wrapping_add(
        (match even_out {
          [_, _explicit_element, ..] => _explicit_element,
        }),
        t14b,
      ),
      ::core::primitive::i32::wrapping_add(
        (match even_out {
          [_, _, _explicit_element, ..] => _explicit_element,
        }),
        t13af,
      ),
      ::core::primitive::i32::wrapping_add(
        (match even_out {
          [_, _, _, _explicit_element, ..] => _explicit_element,
        }),
        t12f,
      ),
      ::core::primitive::i32::wrapping_add(
        (match even_out {
          [_, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
        t11f,
      ),
      ::core::primitive::i32::wrapping_add(
        (match even_out {
          [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
        t10af,
      ),
      ::core::primitive::i32::wrapping_add(
        (match even_out {
          [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
        t9b,
      ),
      ::core::primitive::i32::wrapping_add(
        (match even_out {
          [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
        t8a_2,
      ),
      ::core::primitive::i32::wrapping_sub(
        (match even_out {
          [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
        t8a_2,
      ),
      ::core::primitive::i32::wrapping_sub(
        (match even_out {
          [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
        t9b,
      ),
      ::core::primitive::i32::wrapping_sub(
        (match even_out {
          [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
        t10af,
      ),
      ::core::primitive::i32::wrapping_sub(
        (match even_out {
          [_, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
        t11f,
      ),
      ::core::primitive::i32::wrapping_sub(
        (match even_out {
          [_, _, _, _explicit_element, ..] => _explicit_element,
        }),
        t12f,
      ),
      ::core::primitive::i32::wrapping_sub(
        (match even_out {
          [_, _, _explicit_element, ..] => _explicit_element,
        }),
        t13af,
      ),
      ::core::primitive::i32::wrapping_sub(
        (match even_out {
          [_, _explicit_element, ..] => _explicit_element,
        }),
        t14b,
      ),
      ::core::primitive::i32::wrapping_sub(
        (match even_out {
          [_explicit_element, ..] => _explicit_element,
        }),
        t15a_2,
      ),
    ]
  }

  /// Full scalar reference for DCT_DCT 16x16 8bpc
  fn scalar_dct_dct_16x16(dst: &mut [u8], stride: isize, coeff: &mut [i16]) {
    let mut tmp = [0i32; 256];

    for y in 0..16 {
      let mut input = [0i32; 16];

      for (x, input_element) in <[_]>::iter_mut(&mut input).enumerate() {
        *input_element =
          <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + x * 16]);
      }

      let out = scalar_dct16_1d(&input);

      for x in 0..16 {
        tmp[::core::primitive::usize::wrapping_add(
          ::core::primitive::usize::wrapping_mul(y, 16),
          x,
        )] = ::core::primitive::i32::wrapping_shr(out[x] + 2, 2_u32);
      }
    }

    for x in 0..16 {
      let mut input = [0i32; 16];

      for (y, input_element) in <[_]>::iter_mut(&mut input).enumerate() {
        *input_element = tmp[y.wrapping_mul(16) + x];
      }

      let out = scalar_dct16_1d(&input);

      for y in 0..16 {
        let ::core::result::Result::Ok(row_off) =
          ::console_core_number_conversion::fixed_index(::core::primitive::isize::cast_unsigned(
            (::core::primitive::usize::cast_signed(y) * stride),
          ));
        let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
          dst[(match ::console_core_number_conversion::index(row_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          }) + x],
        );
        let c = ::core::primitive::i32::wrapping_shr(out[y] + 8, 4_u32);
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
          iclip(::core::primitive::i32::wrapping_add(d, c), 0, 255),
        ) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
      }
    }

    coeff[0..256].fill(0);
  }

  /// Full scalar reference for IDENTITY_IDENTITY 16x16 8bpc
  fn scalar_identity_identity_16x16(dst: &mut [u8], stride: isize, coeff: &mut [i16]) {
    let sqrt2x2 = 5793i32;

    for y in 0..16 {
      let ::core::result::Result::Ok(row_off) =
        ::console_core_number_conversion::fixed_index(::core::primitive::isize::cast_unsigned(
          (::core::primitive::usize::cast_signed(y) * stride),
        ));

      for x in 0..16 {
        let c = <i32 as ::core::convert::From<::core::primitive::i16>>::from(
          coeff[::core::primitive::usize::wrapping_add(y, x * 16)],
        );

        let t = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(
          ::core::primitive::i64::wrapping_shr(
            <i64 as ::core::convert::From<::core::primitive::i32>>::from(c) * 27152 + 16384,
            15_u32,
          ),
        ) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
        let t_2 = ::core::primitive::i32::wrapping_shr(t, 1_u32);
        let row_result = ::core::primitive::i32::wrapping_shr(
          ::core::primitive::i32::wrapping_add(c, t_2) + 1,
          1_u32,
        );

        let t2 = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(
          ::core::primitive::i64::wrapping_shr(
            <i64 as ::core::convert::From<::core::primitive::i32>>::from(row_result) * 27152
              + 16384,
            15_u32,
          ),
        ) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
        let col_result = row_result.saturating_mul(2).saturating_add(t2);

        let final_val = ::core::primitive::i32::wrapping_shr(col_result + 8, 4_u32);
        let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
          dst[(match ::console_core_number_conversion::index(row_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          }) + x],
        );
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
          iclip(::core::primitive::i32::wrapping_add(d, final_val), 0, 255),
        ) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
      }
    }

    coeff[0..256].fill(0);
  }

  #[test]
  fn test_dct_dct_16x16_neon_vs_scalar() {
    let Ok(_lock) = crate::src::safe_simd::token_test_lock();
    let token = archmage::Arm64::summon().expect("NEON must be available");

    for pattern in 0..3 {
      let mut coeff_neon = [0i16; 256];
      let mut coeff_scalar = [0i16; 256];

      match pattern {
        0 => {
          *{
            let [_explicit_part, ..] = &mut coeff_neon;
            _explicit_part
          } = 1000;
          *{
            let [_explicit_part, ..] = &mut coeff_scalar;
            _explicit_part
          } = 1000;
        }
        1 => {
          for (i, (coeff_neon_element, coeff_scalar_element)) in <[_]>::iter_mut(&mut coeff_neon)
            .take(16)
            .zip(<[_]>::iter_mut(&mut coeff_scalar).take(16))
            .enumerate()
          {
            *coeff_neon_element = ::core::primitive::i16::wrapping_rem(
              ::core::primitive::i16::wrapping_mul(i as i16 + 1, 100),
              2000,
            ) - 1000;
            *coeff_scalar_element = *coeff_neon_element;
          }
        }
        2 => {
          for (i, (coeff_neon_element, coeff_scalar_element)) in <[_]>::iter_mut(&mut coeff_neon)
            .zip(<[_]>::iter_mut(&mut coeff_scalar))
            .enumerate()
          {
            let val = (i * 37 + 13).wrapping_rem(2001) as i16 - 1000;
            *coeff_neon_element = val;
            *coeff_scalar_element = val;
          }
        }
        _ => unreachable!(),
      }

      let stride = 16isize;
      let mut dst_neon = [128u8; 16 * 16];
      let mut dst_scalar = [128u8; 16 * 16];

      inv_txfm_add_16x16_8bpc_neon(
        token,
        &mut dst_neon,
        0,
        stride,
        &mut coeff_neon,
        255,
        255,
        TxType16::Dct,
        TxType16::Dct,
      );

      scalar_dct_dct_16x16(&mut dst_scalar, stride, &mut coeff_scalar);

      let mut max_diff_seen = 0i32;

      for (dst_neon_element, dst_scalar_element) in
        <[_]>::iter(&dst_neon).zip(<[_]>::iter(&dst_scalar))
      {
        let diff = ::core::primitive::i32::wrapping_sub(
          *dst_neon_element as i32,
          *dst_scalar_element as i32,
        )
        .abs();
        max_diff_seen = max_diff_seen.max(diff);
      }

      assert!(
        max_diff_seen <= MAX_DIFF,
        "DCT_DCT 16x16 pattern {pattern}: max diff = {max_diff_seen} (expected <= {MAX_DIFF})"
      );
    }
  }

  #[test]
  fn test_identity_identity_16x16_neon_vs_scalar() {
    let Ok(_lock) = crate::src::safe_simd::token_test_lock();
    let token = archmage::Arm64::summon().expect("NEON must be available");

    for pattern in 0..3 {
      let mut coeff_neon = [0i16; 256];
      let mut coeff_scalar = [0i16; 256];

      match pattern {
        0 => {
          *{
            let [_explicit_part, ..] = &mut coeff_neon;
            _explicit_part
          } = 500;
          *{
            let [_explicit_part, ..] = &mut coeff_scalar;
            _explicit_part
          } = 500;
        }
        1 => {
          for (i, (coeff_neon_element, coeff_scalar_element)) in <[_]>::iter_mut(&mut coeff_neon)
            .take(32)
            .zip(<[_]>::iter_mut(&mut coeff_scalar).take(32))
            .enumerate()
          {
            *coeff_neon_element = ::core::primitive::i16::wrapping_rem(
              ::core::primitive::i16::wrapping_mul(i as i16 + 1, 50),
              1000,
            ) - 500;
            *coeff_scalar_element = *coeff_neon_element;
          }
        }
        2 => {
          for (i, (coeff_neon_element, coeff_scalar_element)) in <[_]>::iter_mut(&mut coeff_neon)
            .zip(<[_]>::iter_mut(&mut coeff_scalar))
            .enumerate()
          {
            let val = (i * 31 + 7).wrapping_rem(1001) as i16 - 500;
            *coeff_neon_element = val;
            *coeff_scalar_element = val;
          }
        }
        _ => unreachable!(),
      }

      let stride = 16isize;
      let mut dst_neon = [128u8; 16 * 16];
      let mut dst_scalar = [128u8; 16 * 16];

      inv_txfm_add_16x16_8bpc_neon(
        token,
        &mut dst_neon,
        0,
        stride,
        &mut coeff_neon,
        255,
        255,
        TxType16::Identity,
        TxType16::Identity,
      );

      scalar_identity_identity_16x16(&mut dst_scalar, stride, &mut coeff_scalar);

      let mut max_diff_seen = 0i32;

      for (dst_neon_element, dst_scalar_element) in
        <[_]>::iter(&dst_neon).zip(<[_]>::iter(&dst_scalar))
      {
        let diff = ::core::primitive::i32::wrapping_sub(
          *dst_neon_element as i32,
          *dst_scalar_element as i32,
        )
        .abs();
        max_diff_seen = max_diff_seen.max(diff);
      }

      assert!(
        max_diff_seen <= MAX_DIFF,
        "IDENTITY_IDENTITY 16x16 pattern {pattern}: max diff = {max_diff_seen} (expected <= {MAX_DIFF})"
      );
    }
  }

  #[test]
  fn test_dc_only_16x16() {
    let Ok(_lock) = crate::src::safe_simd::token_test_lock();
    let token = archmage::Arm64::summon().expect("NEON must be available");

    let mut coeff = [0i16; 256];
    *{
      let [_explicit_part, ..] = &mut coeff;
      _explicit_part
    } = 1000;

    let stride = 16isize;
    let mut dst = [128u8; 16 * 16];

    inv_txfm_add_16x16_8bpc_neon(
      token,
      &mut dst,
      0,
      stride,
      &mut coeff,
      0,
      255,
      TxType16::Dct,
      TxType16::Dct,
    );

    assert_eq!(coeff[0], 0);

    let [first, ..] = dst;

    for i in 1..256 {
      assert_eq!(dst[i], first, "DC output not uniform at pixel {i}");
    }
  }
}
