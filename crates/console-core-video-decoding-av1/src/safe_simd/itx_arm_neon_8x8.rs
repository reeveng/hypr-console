//! Safe ARM NEON 8x8 inverse transforms (DCT, ADST, flipADST, identity)
//!
//! Port of the 8x8 inverse transform functions from `src/arm/64/itx.S`
//! (lines 711-942) to safe Rust NEON intrinsics. Uses full 128-bit
//! `int16x8_t` registers (8 lanes) instead of the 64-bit `int16x4_t`
//! used by the 4x4 transforms.
//!
//! The transpose uses interleaves at 16-, 32-, then 64-bit lane widths, matching
//! `transpose_8x8h` in util.S. Widening products preserve the lower and upper
//! halves independently through the 12-bit saturating rounded narrow. The
//! 8-point DCT applies the 4-point DCT to the even inputs before rotating and
//! combining the odd inputs. ADST retains the assembly's signs and output order.
//!
//! The 8x8 multiply-by-two identity cancels the row normalization shift. For
//! the DC-only DCT, fixed coefficient scaling rounds on both sides of the row
//! shift by one, followed by the final shift by four. The square layout has
//! no additional rectangular normalization. Tests compare against scalar
//! transforms with a tolerance for accumulated rounding in the NEON assembly.

#![allow(
  clippy::too_many_arguments,
  reason = "the NEON entry points take the destination, coefficients, transform parameters and SIMD capability explicitly"
)]
#![cfg_attr(not(feature = "unchecked"), forbid(unsafe_code))]
#![cfg_attr(feature = "unchecked", deny(unsafe_code))]

#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;

#[cfg(target_arch = "aarch64")]
use archmage::{Arm64, arcane, rite};

#[cfg(target_arch = "aarch64")]
use archmage::intrinsics::aarch64 as safe_simd;

use super::itx_arm_neon_common::IDCT_COEFFS;

pub(crate) const IADST8_COEFFS_V0: [i16; 8] = [4076, 401, 3612, 1931, 2598, 3166, 1189, 3920];
pub(crate) const IADST8_COEFFS_V1: [i16; 8] = [2896, 0, 1567, 3784, 0, 0, 0, 0];

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
pub(crate) fn transpose_8x8h(
  r0: int16x8_t,
  r1: int16x8_t,
  r2: int16x8_t,
  r3: int16x8_t,
  r4: int16x8_t,
  r5: int16x8_t,
  r6: int16x8_t,
  r7: int16x8_t,
) -> (
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
) {
  let t8 = vtrn1q_s16(r0, r1);
  let t9 = vtrn2q_s16(r0, r1);
  let a1 = vtrn1q_s16(r2, r3);
  let a3 = vtrn2q_s16(r2, r3);
  let a0 = vtrn1q_s16(r4, r5);
  let a5 = vtrn2q_s16(r4, r5);
  let a2 = vtrn1q_s16(r6, r7);
  let a7 = vtrn2q_s16(r6, r7);

  let a0_32 = vreinterpretq_s32_s16(a0);
  let a2_32 = vreinterpretq_s32_s16(a2);
  let a5_32 = vreinterpretq_s32_s16(a5);
  let a7_32 = vreinterpretq_s32_s16(a7);
  let t9_32 = vreinterpretq_s32_s16(t9);
  let a3_32 = vreinterpretq_s32_s16(a3);
  let t8_32 = vreinterpretq_s32_s16(t8);
  let a1_32 = vreinterpretq_s32_s16(a1);

  let b4_32 = vtrn1q_s32(a0_32, a2_32);
  let b2_32 = vtrn2q_s32(a0_32, a2_32);
  let b6_32 = vtrn1q_s32(a5_32, a7_32);
  let b7_32 = vtrn2q_s32(a5_32, a7_32);
  let b5_32 = vtrn1q_s32(t9_32, a3_32);
  let c9_32 = vtrn2q_s32(t9_32, a3_32);
  let b3_32 = vtrn1q_s32(t8_32, a1_32);
  let c8_32 = vtrn2q_s32(t8_32, a1_32);

  let b3_64 = vreinterpretq_s64_s32(b3_32);
  let b4_64 = vreinterpretq_s64_s32(b4_32);
  let b5_64 = vreinterpretq_s64_s32(b5_32);
  let b6_64 = vreinterpretq_s64_s32(b6_32);
  let c8_64 = vreinterpretq_s64_s32(c8_32);
  let b2_64 = vreinterpretq_s64_s32(b2_32);
  let c9_64 = vreinterpretq_s64_s32(c9_32);
  let b7_64 = vreinterpretq_s64_s32(b7_32);

  let o0_64 = vtrn1q_s64(b3_64, b4_64);
  let o4_64 = vtrn2q_s64(b3_64, b4_64);
  let o1_64 = vtrn1q_s64(b5_64, b6_64);
  let o5_64 = vtrn2q_s64(b5_64, b6_64);
  let o6_64 = vtrn2q_s64(c8_64, b2_64);
  let o2_64 = vtrn1q_s64(c8_64, b2_64);
  let o3_64 = vtrn1q_s64(c9_64, b7_64);
  let o7_64 = vtrn2q_s64(c9_64, b7_64);

  (
    vreinterpretq_s16_s64(o0_64),
    vreinterpretq_s16_s64(o1_64),
    vreinterpretq_s16_s64(o2_64),
    vreinterpretq_s16_s64(o3_64),
    vreinterpretq_s16_s64(o4_64),
    vreinterpretq_s16_s64(o5_64),
    vreinterpretq_s16_s64(o6_64),
    vreinterpretq_s16_s64(o7_64),
  )
}

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
#[inline(always)]
pub(crate) fn smull_smlal_q(
  s0: int16x8_t,
  s1: int16x8_t,
  coeffs: int16x8_t,
  c0_lane: usize,
  c1_lane: usize,
) -> (int32x4_t, int32x4_t) {
  let s0_lo = vget_low_s16(s0);
  let s1_lo = vget_low_s16(s1);

  let (lo, hi) = match (c0_lane, c1_lane) {
    (0, 0) => {
      let lo = vmull_laneq_s16::<0>(s0_lo, coeffs);
      let lo = vmlal_laneq_s16::<0>(lo, s1_lo, coeffs);
      let hi = vmull_high_laneq_s16::<0>(s0, coeffs);
      let hi = vmlal_high_laneq_s16::<0>(hi, s1, coeffs);
      (lo, hi)
    }
    (0, 1) => {
      let lo = vmull_laneq_s16::<0>(s0_lo, coeffs);
      let lo = vmlal_laneq_s16::<1>(lo, s1_lo, coeffs);
      let hi = vmull_high_laneq_s16::<0>(s0, coeffs);
      let hi = vmlal_high_laneq_s16::<1>(hi, s1, coeffs);
      (lo, hi)
    }
    (1, 0) => {
      let lo = vmull_laneq_s16::<1>(s0_lo, coeffs);
      let lo = vmlal_laneq_s16::<0>(lo, s1_lo, coeffs);
      let hi = vmull_high_laneq_s16::<1>(s0, coeffs);
      let hi = vmlal_high_laneq_s16::<0>(hi, s1, coeffs);
      (lo, hi)
    }
    (2, 3) => {
      let lo = vmull_laneq_s16::<2>(s0_lo, coeffs);
      let lo = vmlal_laneq_s16::<3>(lo, s1_lo, coeffs);
      let hi = vmull_high_laneq_s16::<2>(s0, coeffs);
      let hi = vmlal_high_laneq_s16::<3>(hi, s1, coeffs);
      (lo, hi)
    }
    (3, 2) => {
      let lo = vmull_laneq_s16::<3>(s0_lo, coeffs);
      let lo = vmlal_laneq_s16::<2>(lo, s1_lo, coeffs);
      let hi = vmull_high_laneq_s16::<3>(s0, coeffs);
      let hi = vmlal_high_laneq_s16::<2>(hi, s1, coeffs);
      (lo, hi)
    }
    (4, 5) => {
      let lo = vmull_laneq_s16::<4>(s0_lo, coeffs);
      let lo = vmlal_laneq_s16::<5>(lo, s1_lo, coeffs);
      let hi = vmull_high_laneq_s16::<4>(s0, coeffs);
      let hi = vmlal_high_laneq_s16::<5>(hi, s1, coeffs);
      (lo, hi)
    }
    (5, 4) => {
      let lo = vmull_laneq_s16::<5>(s0_lo, coeffs);
      let lo = vmlal_laneq_s16::<4>(lo, s1_lo, coeffs);
      let hi = vmull_high_laneq_s16::<5>(s0, coeffs);
      let hi = vmlal_high_laneq_s16::<4>(hi, s1, coeffs);
      (lo, hi)
    }
    (6, 7) => {
      let lo = vmull_laneq_s16::<6>(s0_lo, coeffs);
      let lo = vmlal_laneq_s16::<7>(lo, s1_lo, coeffs);
      let hi = vmull_high_laneq_s16::<6>(s0, coeffs);
      let hi = vmlal_high_laneq_s16::<7>(hi, s1, coeffs);
      (lo, hi)
    }
    (7, 6) => {
      let lo = vmull_laneq_s16::<7>(s0_lo, coeffs);
      let lo = vmlal_laneq_s16::<6>(lo, s1_lo, coeffs);
      let hi = vmull_high_laneq_s16::<7>(s0, coeffs);
      let hi = vmlal_high_laneq_s16::<6>(hi, s1, coeffs);
      (lo, hi)
    }
    _ => unreachable!(),
  };
  (lo, hi)
}

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
#[inline(always)]
pub(crate) fn smull_smlsl_q(
  s0: int16x8_t,
  s1: int16x8_t,
  coeffs: int16x8_t,
  c0_lane: usize,
  c1_lane: usize,
) -> (int32x4_t, int32x4_t) {
  let s0_lo = vget_low_s16(s0);
  let s1_lo = vget_low_s16(s1);

  let (lo, hi) = match (c0_lane, c1_lane) {
    (0, 0) => {
      let lo = vmull_laneq_s16::<0>(s0_lo, coeffs);
      let lo = vmlsl_laneq_s16::<0>(lo, s1_lo, coeffs);
      let hi = vmull_high_laneq_s16::<0>(s0, coeffs);
      let hi = vmlsl_high_laneq_s16::<0>(hi, s1, coeffs);
      (lo, hi)
    }
    (0, 1) => {
      let lo = vmull_laneq_s16::<0>(s0_lo, coeffs);
      let lo = vmlsl_laneq_s16::<1>(lo, s1_lo, coeffs);
      let hi = vmull_high_laneq_s16::<0>(s0, coeffs);
      let hi = vmlsl_high_laneq_s16::<1>(hi, s1, coeffs);
      (lo, hi)
    }
    (1, 0) => {
      let lo = vmull_laneq_s16::<1>(s0_lo, coeffs);
      let lo = vmlsl_laneq_s16::<0>(lo, s1_lo, coeffs);
      let hi = vmull_high_laneq_s16::<1>(s0, coeffs);
      let hi = vmlsl_high_laneq_s16::<0>(hi, s1, coeffs);
      (lo, hi)
    }
    (2, 3) => {
      let lo = vmull_laneq_s16::<2>(s0_lo, coeffs);
      let lo = vmlsl_laneq_s16::<3>(lo, s1_lo, coeffs);
      let hi = vmull_high_laneq_s16::<2>(s0, coeffs);
      let hi = vmlsl_high_laneq_s16::<3>(hi, s1, coeffs);
      (lo, hi)
    }
    (3, 2) => {
      let lo = vmull_laneq_s16::<3>(s0_lo, coeffs);
      let lo = vmlsl_laneq_s16::<2>(lo, s1_lo, coeffs);
      let hi = vmull_high_laneq_s16::<3>(s0, coeffs);
      let hi = vmlsl_high_laneq_s16::<2>(hi, s1, coeffs);
      (lo, hi)
    }
    (4, 5) => {
      let lo = vmull_laneq_s16::<4>(s0_lo, coeffs);
      let lo = vmlsl_laneq_s16::<5>(lo, s1_lo, coeffs);
      let hi = vmull_high_laneq_s16::<4>(s0, coeffs);
      let hi = vmlsl_high_laneq_s16::<5>(hi, s1, coeffs);
      (lo, hi)
    }
    (5, 4) => {
      let lo = vmull_laneq_s16::<5>(s0_lo, coeffs);
      let lo = vmlsl_laneq_s16::<4>(lo, s1_lo, coeffs);
      let hi = vmull_high_laneq_s16::<5>(s0, coeffs);
      let hi = vmlsl_high_laneq_s16::<4>(hi, s1, coeffs);
      (lo, hi)
    }
    (6, 7) => {
      let lo = vmull_laneq_s16::<6>(s0_lo, coeffs);
      let lo = vmlsl_laneq_s16::<7>(lo, s1_lo, coeffs);
      let hi = vmull_high_laneq_s16::<6>(s0, coeffs);
      let hi = vmlsl_high_laneq_s16::<7>(hi, s1, coeffs);
      (lo, hi)
    }
    (7, 6) => {
      let lo = vmull_laneq_s16::<7>(s0_lo, coeffs);
      let lo = vmlsl_laneq_s16::<6>(lo, s1_lo, coeffs);
      let hi = vmull_high_laneq_s16::<7>(s0, coeffs);
      let hi = vmlsl_high_laneq_s16::<6>(hi, s1, coeffs);
      (lo, hi)
    }
    _ => unreachable!(),
  };
  (lo, hi)
}

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
#[inline(always)]
pub(crate) fn sqrshrn_pair(lo: int32x4_t, hi: int32x4_t) -> int16x8_t {
  let lo_narrow = vqrshrn_n_s32::<12>(lo);
  vqrshrn_high_n_s32::<12>(lo_narrow, hi)
}

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
pub(crate) fn idct_4_q(
  r0: int16x8_t,
  r1: int16x8_t,
  r2: int16x8_t,
  r3: int16x8_t,
) -> (int16x8_t, int16x8_t, int16x8_t, int16x8_t) {
  let coeffs = safe_simd::vld1q_s16(<&[i16; 8]>::try_from(&IDCT_COEFFS[0..8]).unwrap());

  let (v6_lo, v6_hi) = smull_smlal_q(r1, r3, coeffs, 3, 2);

  let (v4_lo, v4_hi) = smull_smlsl_q(r1, r3, coeffs, 2, 3);

  let (v2_lo, v2_hi) = smull_smlal_q(r0, r2, coeffs, 0, 0);

  let t3a = sqrshrn_pair(v6_lo, v6_hi);
  let t2a = sqrshrn_pair(v4_lo, v4_hi);

  let (v4_lo, v4_hi) = smull_smlsl_q(r0, r2, coeffs, 0, 0);

  let t0 = sqrshrn_pair(v2_lo, v2_hi);
  let t1 = sqrshrn_pair(v4_lo, v4_hi);

  let out0 = vqaddq_s16(t0, t3a);
  let out3 = vqsubq_s16(t0, t3a);
  let out1 = vqaddq_s16(t1, t2a);
  let out2 = vqsubq_s16(t1, t2a);

  (out0, out1, out2, out3)
}

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
pub(crate) fn idct_8_q(
  r0: int16x8_t,
  r1: int16x8_t,
  r2: int16x8_t,
  r3: int16x8_t,
  r4: int16x8_t,
  r5: int16x8_t,
  r6: int16x8_t,
  r7: int16x8_t,
) -> (
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
) {
  let (e0, e1, e2, e3) = idct_4_q(r0, r2, r4, r6);

  let coeffs = safe_simd::vld1q_s16(<&[i16; 8]>::try_from(&IDCT_COEFFS[0..8]).unwrap());

  let (v2_lo, v2_hi) = smull_smlsl_q(r1, r7, coeffs, 4, 5);

  let (v4_lo, v4_hi) = smull_smlal_q(r1, r7, coeffs, 5, 4);

  let (v6_lo, v6_hi) = smull_smlsl_q(r5, r3, coeffs, 6, 7);

  let t4a = sqrshrn_pair(v2_lo, v2_hi);
  let t7a = sqrshrn_pair(v4_lo, v4_hi);

  let (v2_lo, v2_hi) = smull_smlal_q(r5, r3, coeffs, 7, 6);

  let t5a = sqrshrn_pair(v6_lo, v6_hi);
  let t6a = sqrshrn_pair(v2_lo, v2_hi);

  let t4 = vqaddq_s16(t4a, t5a);
  let t5a_new = vqsubq_s16(t4a, t5a);

  let t7 = vqaddq_s16(t7a, t6a);
  let t6a_new = vqsubq_s16(t7a, t6a);

  let (v4_lo, v4_hi) = smull_smlsl_q(t6a_new, t5a_new, coeffs, 0, 0);
  let (v6_lo, v6_hi) = smull_smlal_q(t6a_new, t5a_new, coeffs, 0, 0);
  let t5 = sqrshrn_pair(v4_lo, v4_hi);
  let t6 = sqrshrn_pair(v6_lo, v6_hi);

  let out0 = vqaddq_s16(e0, t7);
  let out7 = vqsubq_s16(e0, t7);
  let out1 = vqaddq_s16(e1, t6);
  let out6 = vqsubq_s16(e1, t6);
  let out2 = vqaddq_s16(e2, t5);
  let out5 = vqsubq_s16(e2, t5);
  let out3 = vqaddq_s16(e3, t4);
  let out4 = vqsubq_s16(e3, t4);

  (out0, out1, out2, out3, out4, out5, out6, out7)
}

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
pub(crate) fn iadst_8_q(
  in0: int16x8_t,
  in1: int16x8_t,
  in2: int16x8_t,
  in3: int16x8_t,
  in4: int16x8_t,
  in5: int16x8_t,
  in6: int16x8_t,
  in7: int16x8_t,
) -> (
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
) {
  let c0 = safe_simd::vld1q_s16(<&[i16; 8]>::try_from(&IADST8_COEFFS_V0[..]).unwrap());
  let c1 = safe_simd::vld1q_s16(<&[i16; 8]>::try_from(&IADST8_COEFFS_V1[..]).unwrap());

  let (lo, hi) = smull_smlal_q(in7, in0, c0, 0, 1);
  let t0a = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(in7, in0, c0, 1, 0);
  let t1a = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(in5, in2, c0, 2, 3);
  let t2a = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(in5, in2, c0, 3, 2);
  let t3a = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(in3, in4, c0, 4, 5);
  let t4a = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(in3, in4, c0, 5, 4);
  let t5a = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(in1, in6, c0, 6, 7);
  let t6a = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(in1, in6, c0, 7, 6);
  let t7a = sqrshrn_pair(lo, hi);

  let t0 = vqaddq_s16(t0a, t4a);
  let t4 = vqsubq_s16(t0a, t4a);
  let t1 = vqaddq_s16(t1a, t5a);
  let t5 = vqsubq_s16(t1a, t5a);
  let t2 = vqaddq_s16(t2a, t6a);
  let t6 = vqsubq_s16(t2a, t6a);
  let t3 = vqaddq_s16(t3a, t7a);
  let t7 = vqsubq_s16(t3a, t7a);

  let (lo, hi) = smull_smlal_q(t4, t5, c1, 3, 2);
  let t4a = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(t4, t5, c1, 2, 3);
  let t5a = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(t7, t6, c1, 3, 2);
  let t6a = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(t7, t6, c1, 2, 3);
  let t7a = sqrshrn_pair(lo, hi);

  let o0 = vqaddq_s16(t0, t2);

  let x2 = vqsubq_s16(t0, t2);

  let o7 = vqnegq_s16(vqaddq_s16(t1, t3));

  let x3 = vqsubq_s16(t1, t3);

  let o1 = vqnegq_s16(vqaddq_s16(t4a, t6a));

  let x6 = vqsubq_s16(t4a, t6a);

  let o6 = vqaddq_s16(t5a, t7a);

  let x7 = vqsubq_s16(t5a, t7a);

  let (lo, hi) = smull_smlal_q(x2, x3, c1, 0, 0);
  let o3_pre = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(x2, x3, c1, 0, 0);
  let o4_pre = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlsl_q(x6, x7, c1, 0, 0);
  let o5_pre = sqrshrn_pair(lo, hi);

  let (lo, hi) = smull_smlal_q(x6, x7, c1, 0, 0);
  let o2 = sqrshrn_pair(lo, hi);

  let o4 = o4_pre;

  let o3 = vqnegq_s16(o3_pre);
  let o5 = vqnegq_s16(o5_pre);

  (o0, o1, o2, o3, o4, o5, o6, o7)
}

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
pub(crate) fn identity_8_q(
  v0: int16x8_t,
  v1: int16x8_t,
  v2: int16x8_t,
  v3: int16x8_t,
  v4: int16x8_t,
  v5: int16x8_t,
  v6: int16x8_t,
  v7: int16x8_t,
) -> (
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
) {
  (
    vqshlq_n_s16::<1>(v0),
    vqshlq_n_s16::<1>(v1),
    vqshlq_n_s16::<1>(v2),
    vqshlq_n_s16::<1>(v3),
    vqshlq_n_s16::<1>(v4),
    vqshlq_n_s16::<1>(v5),
    vqshlq_n_s16::<1>(v6),
    vqshlq_n_s16::<1>(v7),
  )
}

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
pub(crate) fn add_to_dst_8x8_8bpc(
  dst: &mut [u8],
  dst_base: usize,
  stride: isize,
  v0: int16x8_t,
  v1: int16x8_t,
  v2: int16x8_t,
  v3: int16x8_t,
  v4: int16x8_t,
  v5: int16x8_t,
  v6: int16x8_t,
  v7: int16x8_t,
) {
  let rows = [v0, v1, v2, v3, v4, v5, v6, v7];

  for (i, &row) in rows.iter().enumerate() {
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

#[cfg(target_arch = "aarch64")]
#[derive(Clone, Copy)]
pub(crate) enum TxType8 {
  Dct,
  Adst,
  FlipAdst,
  Identity,
}

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn apply_tx8(
  tx: TxType8,
  v0: int16x8_t,
  v1: int16x8_t,
  v2: int16x8_t,
  v3: int16x8_t,
  v4: int16x8_t,
  v5: int16x8_t,
  v6: int16x8_t,
  v7: int16x8_t,
) -> (
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
  int16x8_t,
) {
  match tx {
    TxType8::Dct => idct_8_q(v0, v1, v2, v3, v4, v5, v6, v7),
    TxType8::Adst => iadst_8_q(v0, v1, v2, v3, v4, v5, v6, v7),
    TxType8::FlipAdst => {
      let (o0, o1, o2, o3, o4, o5, o6, o7) = iadst_8_q(v0, v1, v2, v3, v4, v5, v6, v7);
      (o7, o6, o5, o4, o3, o2, o1, o0)
    }
    TxType8::Identity => identity_8_q(v0, v1, v2, v3, v4, v5, v6, v7),
  }
}

#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_8x8_8bpc_neon(
  _token: Arm64,
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  eob: i32,
  _bitdepth_max: i32,
  row_tx: TxType8,
  col_tx: TxType8,
) {
  match {
    ::core::primitive::bool::then_some(
      matches!(row_tx, TxType8::Dct) && matches!(col_tx, TxType8::Dct) && eob == 0,
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      dc_only_8x8_8bpc(dst, dst_base, dst_stride, coeff);
      return;
    }
    ::core::option::Option::None => {}
  }

  let v16 = safe_simd::vld1q_s16(<&[i16; 8]>::try_from(&coeff[0..8]).unwrap());
  let v17 = safe_simd::vld1q_s16(<&[i16; 8]>::try_from(&coeff[8..16]).unwrap());
  let v18 = safe_simd::vld1q_s16(<&[i16; 8]>::try_from(&coeff[16..24]).unwrap());
  let v19 = safe_simd::vld1q_s16(<&[i16; 8]>::try_from(&coeff[24..32]).unwrap());
  let v20 = safe_simd::vld1q_s16(<&[i16; 8]>::try_from(&coeff[32..40]).unwrap());
  let v21 = safe_simd::vld1q_s16(<&[i16; 8]>::try_from(&coeff[40..48]).unwrap());
  let v22 = safe_simd::vld1q_s16(<&[i16; 8]>::try_from(&coeff[48..56]).unwrap());
  let v23 = safe_simd::vld1q_s16(<&[i16; 8]>::try_from(&coeff[56..64]).unwrap());

  coeff[0..64].fill(0);

  let is_identity_row = matches!(row_tx, TxType8::Identity);

  let (v16, v17, v18, v19, v20, v21, v22, v23) =
    match ::core::primitive::bool::then_some(is_identity_row, ()) {
      ::core::option::Option::Some(()) => (v16, v17, v18, v19, v20, v21, v22, v23),
      ::core::option::Option::None => {
        let (v16, v17, v18, v19, v20, v21, v22, v23) =
          apply_tx8(row_tx, v16, v17, v18, v19, v20, v21, v22, v23);

        let v16 = vrshrq_n_s16::<1>(v16);
        let v17 = vrshrq_n_s16::<1>(v17);
        let v18 = vrshrq_n_s16::<1>(v18);
        let v19 = vrshrq_n_s16::<1>(v19);
        let v20 = vrshrq_n_s16::<1>(v20);
        let v21 = vrshrq_n_s16::<1>(v21);
        let v22 = vrshrq_n_s16::<1>(v22);
        let v23 = vrshrq_n_s16::<1>(v23);

        (v16, v17, v18, v19, v20, v21, v22, v23)
      }
    };

  let (v16, v17, v18, v19, v20, v21, v22, v23) =
    transpose_8x8h(v16, v17, v18, v19, v20, v21, v22, v23);

  let (v16, v17, v18, v19, v20, v21, v22, v23) =
    apply_tx8(col_tx, v16, v17, v18, v19, v20, v21, v22, v23);

  add_to_dst_8x8_8bpc(
    dst, dst_base, dst_stride, v16, v17, v18, v19, v20, v21, v22, v23,
  );
}

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn dc_only_8x8_8bpc(dst: &mut [u8], dst_base: usize, dst_stride: isize, coeff: &mut [i16]) {
  let dc = coeff[0];
  coeff[0] = 0;

  let scale = vdupq_n_s16(
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
      ::core::primitive::i32::wrapping_mul(2896, 8),
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    },
  );

  let v16 = vdupq_n_s16(dc);

  let v16 = vqrdmulhq_s16(v16, scale);

  let v16 = vrshrq_n_s16::<1>(v16);

  let v16 = vqrdmulhq_s16(v16, scale);

  let v16 = vrshrq_n_s16::<4>(v16);

  for i in 0..8 {
    let row_off = dst_base.wrapping_add_signed(
      (match ::console_core_number_conversion::wrapped_offset(i) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      }) * dst_stride,
    );

    let dst_bytes: [u8; 8] = dst[row_off..row_off + 8].try_into().unwrap();
    let dst_u8 = safe_simd::vld1_u8(&dst_bytes);

    let sum = vreinterpretq_s16_u16(vaddw_u8(vreinterpretq_u16_s16(v16), dst_u8));
    let result = vqmovun_s16(sum);

    let mut out = [0u8; 8];
    safe_simd::vst1_u8(&mut out, result);
    dst[row_off..row_off + 8].copy_from_slice(&out);
  }
}

#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_dct_dct_8x8_8bpc_neon_inner(
  token: Arm64,
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  eob: i32,
  bitdepth_max: i32,
) {
  inv_txfm_add_8x8_8bpc_neon(
    token,
    dst,
    dst_base,
    dst_stride,
    coeff,
    eob,
    bitdepth_max,
    TxType8::Dct,
    TxType8::Dct,
  );
}

#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_identity_identity_8x8_8bpc_neon_inner(
  token: Arm64,
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  eob: i32,
  bitdepth_max: i32,
) {
  inv_txfm_add_8x8_8bpc_neon(
    token,
    dst,
    dst_base,
    dst_stride,
    coeff,
    eob,
    bitdepth_max,
    TxType8::Identity,
    TxType8::Identity,
  );
}

#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_adst_adst_8x8_8bpc_neon_inner(
  token: Arm64,
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  eob: i32,
  bitdepth_max: i32,
) {
  inv_txfm_add_8x8_8bpc_neon(
    token,
    dst,
    dst_base,
    dst_stride,
    coeff,
    eob,
    bitdepth_max,
    TxType8::Adst,
    TxType8::Adst,
  );
}

#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_dct_adst_8x8_8bpc_neon_inner(
  token: Arm64,
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  eob: i32,
  bitdepth_max: i32,
) {
  inv_txfm_add_8x8_8bpc_neon(
    token,
    dst,
    dst_base,
    dst_stride,
    coeff,
    eob,
    bitdepth_max,
    TxType8::Dct,
    TxType8::Adst,
  );
}

#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_dct_flipadst_8x8_8bpc_neon_inner(
  token: Arm64,
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  eob: i32,
  bitdepth_max: i32,
) {
  inv_txfm_add_8x8_8bpc_neon(
    token,
    dst,
    dst_base,
    dst_stride,
    coeff,
    eob,
    bitdepth_max,
    TxType8::Dct,
    TxType8::FlipAdst,
  );
}

#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_dct_identity_8x8_8bpc_neon_inner(
  token: Arm64,
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  eob: i32,
  bitdepth_max: i32,
) {
  inv_txfm_add_8x8_8bpc_neon(
    token,
    dst,
    dst_base,
    dst_stride,
    coeff,
    eob,
    bitdepth_max,
    TxType8::Dct,
    TxType8::Identity,
  );
}

#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_adst_dct_8x8_8bpc_neon_inner(
  token: Arm64,
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  eob: i32,
  bitdepth_max: i32,
) {
  inv_txfm_add_8x8_8bpc_neon(
    token,
    dst,
    dst_base,
    dst_stride,
    coeff,
    eob,
    bitdepth_max,
    TxType8::Adst,
    TxType8::Dct,
  );
}

#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_adst_flipadst_8x8_8bpc_neon_inner(
  token: Arm64,
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  eob: i32,
  bitdepth_max: i32,
) {
  inv_txfm_add_8x8_8bpc_neon(
    token,
    dst,
    dst_base,
    dst_stride,
    coeff,
    eob,
    bitdepth_max,
    TxType8::Adst,
    TxType8::FlipAdst,
  );
}

#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_flipadst_dct_8x8_8bpc_neon_inner(
  token: Arm64,
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  eob: i32,
  bitdepth_max: i32,
) {
  inv_txfm_add_8x8_8bpc_neon(
    token,
    dst,
    dst_base,
    dst_stride,
    coeff,
    eob,
    bitdepth_max,
    TxType8::FlipAdst,
    TxType8::Dct,
  );
}

#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_flipadst_adst_8x8_8bpc_neon_inner(
  token: Arm64,
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  eob: i32,
  bitdepth_max: i32,
) {
  inv_txfm_add_8x8_8bpc_neon(
    token,
    dst,
    dst_base,
    dst_stride,
    coeff,
    eob,
    bitdepth_max,
    TxType8::FlipAdst,
    TxType8::Adst,
  );
}

#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_flipadst_flipadst_8x8_8bpc_neon_inner(
  token: Arm64,
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  eob: i32,
  bitdepth_max: i32,
) {
  inv_txfm_add_8x8_8bpc_neon(
    token,
    dst,
    dst_base,
    dst_stride,
    coeff,
    eob,
    bitdepth_max,
    TxType8::FlipAdst,
    TxType8::FlipAdst,
  );
}

#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_identity_dct_8x8_8bpc_neon_inner(
  token: Arm64,
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  eob: i32,
  bitdepth_max: i32,
) {
  inv_txfm_add_8x8_8bpc_neon(
    token,
    dst,
    dst_base,
    dst_stride,
    coeff,
    eob,
    bitdepth_max,
    TxType8::Identity,
    TxType8::Dct,
  );
}

#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_adst_identity_8x8_8bpc_neon_inner(
  token: Arm64,
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  eob: i32,
  bitdepth_max: i32,
) {
  inv_txfm_add_8x8_8bpc_neon(
    token,
    dst,
    dst_base,
    dst_stride,
    coeff,
    eob,
    bitdepth_max,
    TxType8::Adst,
    TxType8::Identity,
  );
}

#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_flipadst_identity_8x8_8bpc_neon_inner(
  token: Arm64,
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  eob: i32,
  bitdepth_max: i32,
) {
  inv_txfm_add_8x8_8bpc_neon(
    token,
    dst,
    dst_base,
    dst_stride,
    coeff,
    eob,
    bitdepth_max,
    TxType8::FlipAdst,
    TxType8::Identity,
  );
}

#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_identity_adst_8x8_8bpc_neon_inner(
  token: Arm64,
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  eob: i32,
  bitdepth_max: i32,
) {
  inv_txfm_add_8x8_8bpc_neon(
    token,
    dst,
    dst_base,
    dst_stride,
    coeff,
    eob,
    bitdepth_max,
    TxType8::Identity,
    TxType8::Adst,
  );
}

#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_identity_flipadst_8x8_8bpc_neon_inner(
  token: Arm64,
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  eob: i32,
  bitdepth_max: i32,
) {
  inv_txfm_add_8x8_8bpc_neon(
    token,
    dst,
    dst_base,
    dst_stride,
    coeff,
    eob,
    bitdepth_max,
    TxType8::Identity,
    TxType8::FlipAdst,
  );
}

#[cfg(test)]
#[cfg(target_arch = "aarch64")]
mod tests {
  use super::*;
  use archmage::SimdToken;

  const MAX_DIFF: i32 = 40;

  fn scalar_dct_dct_8x8(
    dst: &mut [u8],
    stride: isize,
    coeff: &mut [i16],
  ) -> ::core::result::Result<(), ::console_core_never::Never> {
    let mut tmp = [0i32; 64];

    for y in 0..8 {
      let mut input = [0i32; 8];

      for (x, input_element) in <[_]>::iter_mut(&mut input).enumerate() {
        *input_element =
          <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + x * 8]);
      }

      let ::core::result::Result::Ok(out) = scalar_dct8_1d(&input);

      for x in 0..8 {
        tmp[::core::primitive::usize::wrapping_add(
          ::core::primitive::usize::wrapping_mul(y, 8),
          x,
        )] = out[x];
      }
    }

    for x in 0..8 {
      let mut input = [0i32; 8];

      for (y, input_element) in <[_]>::iter_mut(&mut input).enumerate() {
        *input_element = tmp[y.wrapping_mul(8) + x];
      }

      let ::core::result::Result::Ok(out) = scalar_dct8_1d(&input);

      for y in 0..8 {
        let ::core::result::Result::Ok(row_off) =
          ::console_core_number_conversion::fixed_index(::core::primitive::isize::cast_unsigned(
            (::core::primitive::usize::cast_signed(y) * stride),
          ));
        let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
          dst[(match ::console_core_number_conversion::index(row_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          }) + x],
        );
        let c = ::core::primitive::i32::wrapping_shr(out[y] + 32, 6_u32);
        let result = ::core::primitive::i32::wrapping_add(d, c).clamp(0, 255);
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x] =
          match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          };
      }
    }

    coeff[0..64].fill(0);
    ::core::result::Result::Ok(())
  }

  #[must_use]
  fn scalar_dct8_1d(
    input: &[i32; 8],
  ) -> ::core::result::Result<[i32; 8], ::console_core_never::Never> {
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
    let ::core::result::Result::Ok(even_out) = scalar_dct4_1d(&even);

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

    ::core::result::Result::Ok([
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
    ])
  }

  #[must_use]
  fn scalar_dct4_1d(
    input: &[i32; 4],
  ) -> ::core::result::Result<[i32; 4], ::console_core_never::Never> {
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

    ::core::result::Result::Ok([
      ::core::primitive::i32::wrapping_add(t0, t3a),
      ::core::primitive::i32::wrapping_add(t1, t2a),
      ::core::primitive::i32::wrapping_sub(t1, t2a),
      ::core::primitive::i32::wrapping_sub(t0, t3a),
    ])
  }

  #[must_use]
  fn scalar_adst8_1d(
    input: &[i32; 8],
  ) -> ::core::result::Result<[i32; 8], ::console_core_never::Never> {
    let &[_, _, _, _, _, _, _, x0, ..] = input;
    let &[x1, ..] = input;
    let &[_, _, _, _, _, x2, ..] = input;
    let &[_, _, x3, ..] = input;
    let &[_, _, _, x4, ..] = input;
    let &[_, _, _, _, x5, ..] = input;
    let &[_, x6, ..] = input;
    let &[_, _, _, _, _, _, x7, ..] = input;

    let s0 = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_add(x0 * 4076, x1 * 401) + 2048,
      12_u32,
    );
    let s1 = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_sub(x0 * 401, x1 * 4076) + 2048,
      12_u32,
    );
    let s2 = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_add(x2 * 3612, x3 * 1931) + 2048,
      12_u32,
    );
    let s3 = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_sub(x2 * 1931, x3 * 3612) + 2048,
      12_u32,
    );
    let s4 = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_add(x4 * 2598, x5 * 3166) + 2048,
      12_u32,
    );
    let s5 = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_sub(x4 * 3166, x5 * 2598) + 2048,
      12_u32,
    );
    let s6 = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_add(x6 * 1189, x7 * 3920) + 2048,
      12_u32,
    );
    let s7 = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_sub(x6 * 3920, x7 * 1189) + 2048,
      12_u32,
    );

    let x0_2 = ::core::primitive::i32::wrapping_add(s0, s4);
    let x1_2 = ::core::primitive::i32::wrapping_add(s1, s5);
    let x2_2 = ::core::primitive::i32::wrapping_add(s2, s6);
    let x3_2 = ::core::primitive::i32::wrapping_add(s3, s7);
    let x4_2 = ::core::primitive::i32::wrapping_sub(s0, s4);
    let x5_2 = ::core::primitive::i32::wrapping_sub(s1, s5);
    let x6_2 = ::core::primitive::i32::wrapping_sub(s2, s6);
    let x7_2 = ::core::primitive::i32::wrapping_sub(s3, s7);

    let t4a = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_add(x4_2 * 3784, x5_2 * 1567) + 2048,
      12_u32,
    );
    let t5a = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_sub(x4_2 * 1567, x5_2 * 3784) + 2048,
      12_u32,
    );
    let t6a = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_sub(x7_2 * 3784, x6_2 * 1567) + 2048,
      12_u32,
    );
    let t7a = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_add(x7_2 * 1567, x6_2 * 3784) + 2048,
      12_u32,
    );

    let a0 = ::core::primitive::i32::wrapping_add(x0_2, x2_2);
    let a1 = ::core::primitive::i32::wrapping_add(x1_2, x3_2);
    let a2 = ::core::primitive::i32::wrapping_sub(x0_2, x2_2);
    let a3 = ::core::primitive::i32::wrapping_sub(x1_2, x3_2);
    let a4 = ::core::primitive::i32::wrapping_add(t4a, t6a);
    let a5 = ::core::primitive::i32::wrapping_add(t5a, t7a);
    let a6 = ::core::primitive::i32::wrapping_sub(t4a, t6a);
    let a7 = ::core::primitive::i32::wrapping_sub(t5a, t7a);

    let o0 = a0;
    let o7 = ::core::primitive::i32::wrapping_neg((a1));
    let o1 = ::core::primitive::i32::wrapping_neg((a4));
    let o6 = a5;

    let b2 = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_add(a2 * 2896, a3 * 2896) + 2048,
      12_u32,
    );
    let b3 = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_sub(a2 * 2896, a3 * 2896) + 2048,
      12_u32,
    );
    let b6 = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_add(a6 * 2896, a7 * 2896) + 2048,
      12_u32,
    );
    let b7 = ::core::primitive::i32::wrapping_shr(
      ::core::primitive::i32::wrapping_sub(a6 * 2896, a7 * 2896) + 2048,
      12_u32,
    );

    ::core::result::Result::Ok([
      o0,
      o1,
      b6,
      ::core::primitive::i32::wrapping_neg(b2),
      b3,
      ::core::primitive::i32::wrapping_neg(b7),
      o6,
      o7,
    ])
  }

  fn scalar_adst_adst_8x8(
    dst: &mut [u8],
    stride: isize,
    coeff: &mut [i16],
  ) -> ::core::result::Result<(), ::console_core_never::Never> {
    let mut tmp = [0i32; 64];

    for y in 0..8 {
      let mut input = [0i32; 8];

      for (x, input_element) in <[_]>::iter_mut(&mut input).enumerate() {
        *input_element =
          <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + x * 8]);
      }

      let ::core::result::Result::Ok(out) = scalar_adst8_1d(&input);

      for x in 0..8 {
        tmp[::core::primitive::usize::wrapping_add(
          ::core::primitive::usize::wrapping_mul(y, 8),
          x,
        )] = out[x];
      }
    }

    for x in 0..8 {
      let mut input = [0i32; 8];

      for (y, input_element) in <[_]>::iter_mut(&mut input).enumerate() {
        *input_element = tmp[y.wrapping_mul(8) + x];
      }

      let ::core::result::Result::Ok(out) = scalar_adst8_1d(&input);

      for y in 0..8 {
        let ::core::result::Result::Ok(row_off) =
          ::console_core_number_conversion::fixed_index(::core::primitive::isize::cast_unsigned(
            (::core::primitive::usize::cast_signed(y) * stride),
          ));
        let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
          dst[(match ::console_core_number_conversion::index(row_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          }) + x],
        );
        let c = ::core::primitive::i32::wrapping_shr(out[y] + 32, 6_u32);
        let result = ::core::primitive::i32::wrapping_add(d, c).clamp(0, 255);
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x] =
          match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          };
      }
    }

    coeff[0..64].fill(0);
    ::core::result::Result::Ok(())
  }

  fn scalar_identity_identity_8x8(
    dst: &mut [u8],
    stride: isize,
    coeff: &mut [i16],
  ) -> ::core::result::Result<(), ::console_core_never::Never> {
    for y in 0..8 {
      let ::core::result::Result::Ok(row_off) =
        ::console_core_number_conversion::fixed_index(::core::primitive::isize::cast_unsigned(
          (::core::primitive::usize::cast_signed(y) * stride),
        ));

      for x in 0..8 {
        let c = <i32 as ::core::convert::From<::core::primitive::i16>>::from(
          coeff[::core::primitive::usize::wrapping_add(y, x * 8)],
        );

        let val = ::core::primitive::i32::wrapping_shr(
          ::core::primitive::i32::wrapping_mul(c, 2) + 8,
          4_u32,
        );
        let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
          dst[(match ::console_core_number_conversion::index(row_off) {
            ::core::result::Result::Ok(native_position) => native_position,
          }) + x],
        );
        let result = ::core::primitive::i32::wrapping_add(d, val).clamp(0, 255);
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x] =
          match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          };
      }
    }

    coeff[0..64].fill(0);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn test_dct_dct_8x8_neon_vs_scalar() {
    let token = match Arm64::summon() {
      Some(t) => t,
      None => return,
    };

    let stride: isize = 16;
    let mut coeff_neon = [0i16; 64];
    let mut coeff_scalar = [0i16; 64];

    for (i, (coeff_neon_element, coeff_scalar_element)) in <[_]>::iter_mut(&mut coeff_neon)
      .zip(<[_]>::iter_mut(&mut coeff_scalar))
      .enumerate()
    {
      let val = ::core::primitive::i16::wrapping_rem(
        (match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(i) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        }) * 37
          + 13,
        200,
      ) - 100;
      *coeff_neon_element = val;
      *coeff_scalar_element = val;
    }

    let mut dst_neon = vec![128u8; stride.cast_unsigned() * 8];
    let mut dst_scalar = dst_neon.clone();

    inv_txfm_add_dct_dct_8x8_8bpc_neon_inner(
      token,
      &mut dst_neon,
      0,
      stride,
      &mut coeff_neon,
      63,
      255,
    );

    let ::core::result::Result::Ok(()) =
      scalar_dct_dct_8x8(&mut dst_scalar, stride, &mut coeff_scalar);

    for y in 0..8 {
      for x in 0..8 {
        let off = (::core::num::Wrapping(y)
          * ::core::num::Wrapping(::core::primitive::isize::cast_unsigned(stride)))
        .0 + x;
        let diff = ::core::primitive::i32::wrapping_sub(
          <i32 as ::core::convert::From<::core::primitive::u8>>::from(dst_neon[off]),
          <i32 as ::core::convert::From<::core::primitive::u8>>::from(dst_scalar[off]),
        )
        .abs();
        assert!(
          diff <= MAX_DIFF,
          "DCT_DCT 8x8 mismatch at ({}, {}): neon={}, scalar={}, diff={} (max {})",
          x,
          y,
          dst_neon[off],
          dst_scalar[off],
          diff,
          MAX_DIFF,
        );
      }
    }

    assert!(coeff_neon.iter().all(|&c| c == 0));
  }

  #[test]
  fn test_dct_dct_8x8_dc_only() {
    let token = match Arm64::summon() {
      Some(t) => t,
      None => return,
    };

    let stride: isize = 16;
    let mut coeff_neon = [0i16; 64];
    let mut coeff_full = [0i16; 64];
    *{
      let [_explicit_part, ..] = &mut coeff_neon;
      _explicit_part
    } = 100;
    *{
      let [_explicit_part, ..] = &mut coeff_full;
      _explicit_part
    } = 100;

    let mut dst_dc = vec![128u8; stride.cast_unsigned() * 8];
    let mut dst_full = dst_dc.clone();

    inv_txfm_add_dct_dct_8x8_8bpc_neon_inner(
      token,
      &mut dst_dc,
      0,
      stride,
      &mut coeff_neon,
      0,
      255,
    );

    inv_txfm_add_dct_dct_8x8_8bpc_neon_inner(
      token,
      &mut dst_full,
      0,
      stride,
      &mut coeff_full,
      1,
      255,
    );

    let first_val = dst_dc[0];

    for y in 0..8 {
      for x in 0..8 {
        let off = (::core::num::Wrapping(y)
          * ::core::num::Wrapping(::core::primitive::isize::cast_unsigned(stride)))
        .0 + x;
        assert_eq!(
          dst_dc[off], first_val,
          "DC-only should be uniform at ({}, {})",
          x, y,
        );
      }
    }

    for y in 0..8 {
      for x in 0..8 {
        let off = (::core::num::Wrapping(y)
          * ::core::num::Wrapping(::core::primitive::isize::cast_unsigned(stride)))
        .0 + x;
        let diff = ::core::primitive::i32::wrapping_sub(
          <i32 as ::core::convert::From<::core::primitive::u8>>::from(dst_dc[off]),
          <i32 as ::core::convert::From<::core::primitive::u8>>::from(dst_full[off]),
        )
        .abs();
        assert!(
          diff <= 1,
          "DC fast path mismatch at ({}, {}): dc={}, full={}, diff={}",
          x,
          y,
          dst_dc[off],
          dst_full[off],
          diff,
        );
      }
    }
  }

  #[test]
  fn test_adst_adst_8x8_neon_vs_scalar() {
    let token = match Arm64::summon() {
      Some(t) => t,
      None => return,
    };

    let stride: isize = 16;
    let mut coeff_neon = [0i16; 64];
    let mut coeff_scalar = [0i16; 64];

    for (i, (coeff_neon_element, coeff_scalar_element)) in <[_]>::iter_mut(&mut coeff_neon)
      .zip(<[_]>::iter_mut(&mut coeff_scalar))
      .enumerate()
    {
      let val = ::core::primitive::i16::wrapping_rem(
        (match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(i) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        }) * 23
          + 7,
        120,
      ) - 60;
      *coeff_neon_element = val;
      *coeff_scalar_element = val;
    }

    let mut dst_neon = vec![128u8; stride.cast_unsigned() * 8];
    let mut dst_scalar = dst_neon.clone();

    inv_txfm_add_adst_adst_8x8_8bpc_neon_inner(
      token,
      &mut dst_neon,
      0,
      stride,
      &mut coeff_neon,
      63,
      255,
    );

    let ::core::result::Result::Ok(()) =
      scalar_adst_adst_8x8(&mut dst_scalar, stride, &mut coeff_scalar);

    for y in 0..8 {
      for x in 0..8 {
        let off = (::core::num::Wrapping(y)
          * ::core::num::Wrapping(::core::primitive::isize::cast_unsigned(stride)))
        .0 + x;
        let diff = ::core::primitive::i32::wrapping_sub(
          <i32 as ::core::convert::From<::core::primitive::u8>>::from(dst_neon[off]),
          <i32 as ::core::convert::From<::core::primitive::u8>>::from(dst_scalar[off]),
        )
        .abs();
        assert!(
          diff <= MAX_DIFF,
          "ADST_ADST 8x8 mismatch at ({}, {}): neon={}, scalar={}, diff={} (max {})",
          x,
          y,
          dst_neon[off],
          dst_scalar[off],
          diff,
          MAX_DIFF,
        );
      }
    }
  }

  #[test]
  fn test_identity_identity_8x8_neon_vs_scalar() {
    let token = match Arm64::summon() {
      Some(t) => t,
      None => return,
    };

    let stride: isize = 16;
    let mut coeff_neon = [0i16; 64];
    let mut coeff_scalar = [0i16; 64];

    for (i, (coeff_neon_element, coeff_scalar_element)) in <[_]>::iter_mut(&mut coeff_neon)
      .zip(<[_]>::iter_mut(&mut coeff_scalar))
      .enumerate()
    {
      let val = ::core::primitive::i16::wrapping_rem(
        (match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(i) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        }) * 11
          + 3,
        80,
      ) - 40;
      *coeff_neon_element = val;
      *coeff_scalar_element = val;
    }

    let mut dst_neon = vec![128u8; stride.cast_unsigned() * 8];
    let mut dst_scalar = dst_neon.clone();

    inv_txfm_add_identity_identity_8x8_8bpc_neon_inner(
      token,
      &mut dst_neon,
      0,
      stride,
      &mut coeff_neon,
      63,
      255,
    );

    let ::core::result::Result::Ok(()) =
      scalar_identity_identity_8x8(&mut dst_scalar, stride, &mut coeff_scalar);

    for y in 0..8 {
      for x in 0..8 {
        let off = (::core::num::Wrapping(y)
          * ::core::num::Wrapping(::core::primitive::isize::cast_unsigned(stride)))
        .0 + x;
        let diff = ::core::primitive::i32::wrapping_sub(
          <i32 as ::core::convert::From<::core::primitive::u8>>::from(dst_neon[off]),
          <i32 as ::core::convert::From<::core::primitive::u8>>::from(dst_scalar[off]),
        )
        .abs();
        assert!(
          diff <= 1,
          "IDENTITY 8x8 mismatch at ({}, {}): neon={}, scalar={}, diff={}",
          x,
          y,
          dst_neon[off],
          dst_scalar[off],
          diff,
        );
      }
    }
  }

  #[test]
  fn test_dct_adst_8x8_neon_vs_scalar() {
    let token = match Arm64::summon() {
      Some(t) => t,
      None => return,
    };

    let stride: isize = 16;
    let mut coeff_neon = [0i16; 64];
    let mut coeff_scalar = [0i16; 64];

    for (i, (coeff_neon_element, coeff_scalar_element)) in <[_]>::iter_mut(&mut coeff_neon)
      .zip(<[_]>::iter_mut(&mut coeff_scalar))
      .enumerate()
    {
      let val = ::core::primitive::i16::wrapping_rem(
        (match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(i) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        }) * 19
          + 5,
        100,
      ) - 50;
      *coeff_neon_element = val;
      *coeff_scalar_element = val;
    }

    let mut dst_neon = vec![128u8; stride.cast_unsigned() * 8];
    let mut dst_scalar = dst_neon.clone();

    inv_txfm_add_dct_adst_8x8_8bpc_neon_inner(
      token,
      &mut dst_neon,
      0,
      stride,
      &mut coeff_neon,
      63,
      255,
    );

    {
      let mut tmp = [0i32; 64];

      for y in 0..8 {
        let mut input = [0i32; 8];

        for (x, input_element) in <[_]>::iter_mut(&mut input).enumerate() {
          *input_element =
            <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff_scalar[y + x * 8]);
        }

        let ::core::result::Result::Ok(out) = scalar_dct8_1d(&input);

        for x in 0..8 {
          tmp[::core::primitive::usize::wrapping_add(
            ::core::primitive::usize::wrapping_mul(y, 8),
            x,
          )] = out[x];
        }
      }

      for x in 0..8 {
        let mut input = [0i32; 8];

        for (y, input_element) in <[_]>::iter_mut(&mut input).enumerate() {
          *input_element = tmp[y.wrapping_mul(8) + x];
        }

        let ::core::result::Result::Ok(out) = scalar_adst8_1d(&input);

        for y in 0..8 {
          let row_off = (::core::primitive::usize::cast_signed(y)).wrapping_mul(stride) as usize;
          let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
            dst_scalar[::core::primitive::usize::wrapping_add(row_off, x)],
          );
          let c = ::core::primitive::i32::wrapping_shr(out[y] + 32, 6_u32);
          let result = ::core::primitive::i32::wrapping_add(d, c).clamp(0, 255);
          dst_scalar[row_off.wrapping_add(x)] =
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            };
        }
      }

      coeff_scalar[0..64].fill(0);
    }

    for y in 0..8 {
      for x in 0..8 {
        let off = (::core::num::Wrapping(y)
          * ::core::num::Wrapping(::core::primitive::isize::cast_unsigned(stride)))
        .0 + x;
        let diff = ::core::primitive::i32::wrapping_sub(
          <i32 as ::core::convert::From<::core::primitive::u8>>::from(dst_neon[off]),
          <i32 as ::core::convert::From<::core::primitive::u8>>::from(dst_scalar[off]),
        )
        .abs();
        assert!(
          diff <= MAX_DIFF,
          "DCT_ADST 8x8 mismatch at ({}, {}): neon={}, scalar={}, diff={} (max {})",
          x,
          y,
          dst_neon[off],
          dst_scalar[off],
          diff,
          MAX_DIFF,
        );
      }
    }
  }

  #[arcane]
  fn test_transpose_8x8h_inner(_token: Arm64, rows_in: &[[i16; 8]; 8]) -> [[i16; 8]; 8] {
    let r0 = safe_simd::vld1q_s16({
      let [_explicit_part, ..] = &*rows_in;
      _explicit_part
    });
    let r1 = safe_simd::vld1q_s16({
      let [_, _explicit_part, ..] = &*rows_in;
      _explicit_part
    });
    let r2 = safe_simd::vld1q_s16({
      let [_, _, _explicit_part, ..] = &*rows_in;
      _explicit_part
    });
    let r3 = safe_simd::vld1q_s16({
      let [_, _, _, _explicit_part, ..] = &*rows_in;
      _explicit_part
    });
    let r4 = safe_simd::vld1q_s16({
      let [_, _, _, _, _explicit_part, ..] = &*rows_in;
      _explicit_part
    });
    let r5 = safe_simd::vld1q_s16({
      let [_, _, _, _, _, _explicit_part, ..] = &*rows_in;
      _explicit_part
    });
    let r6 = safe_simd::vld1q_s16({
      let [_, _, _, _, _, _, _explicit_part, ..] = &*rows_in;
      _explicit_part
    });
    let r7 = safe_simd::vld1q_s16({
      let [_, _, _, _, _, _, _, _explicit_part, ..] = &*rows_in;
      _explicit_part
    });

    let (o0, o1, o2, o3, o4, o5, o6, o7) = transpose_8x8h(r0, r1, r2, r3, r4, r5, r6, r7);

    let mut rows_out = [[0i16; 8]; 8];
    safe_simd::vst1q_s16(
      {
        let [_explicit_part, ..] = &mut rows_out;
        _explicit_part
      },
      o0,
    );
    safe_simd::vst1q_s16(
      {
        let [_, _explicit_part, ..] = &mut rows_out;
        _explicit_part
      },
      o1,
    );
    safe_simd::vst1q_s16(
      {
        let [_, _, _explicit_part, ..] = &mut rows_out;
        _explicit_part
      },
      o2,
    );
    safe_simd::vst1q_s16(
      {
        let [_, _, _, _explicit_part, ..] = &mut rows_out;
        _explicit_part
      },
      o3,
    );
    safe_simd::vst1q_s16(
      {
        let [_, _, _, _, _explicit_part, ..] = &mut rows_out;
        _explicit_part
      },
      o4,
    );
    safe_simd::vst1q_s16(
      {
        let [_, _, _, _, _, _explicit_part, ..] = &mut rows_out;
        _explicit_part
      },
      o5,
    );
    safe_simd::vst1q_s16(
      {
        let [_, _, _, _, _, _, _explicit_part, ..] = &mut rows_out;
        _explicit_part
      },
      o6,
    );
    safe_simd::vst1q_s16(
      {
        let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut rows_out;
        _explicit_part
      },
      o7,
    );
    rows_out
  }

  #[test]
  fn test_transpose_8x8h() {
    let token = match Arm64::summon() {
      Some(t) => t,
      None => return,
    };

    let mut rows_in = [[0i16; 8]; 8];

    for y in 0..8 {
      for x in 0..8 {
        rows_in[y][x] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
          ::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(y, 8), x),
        ) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
      }
    }

    let rows_out = test_transpose_8x8h_inner(token, &rows_in);

    for y in 0..8 {
      for x in 0..8 {
        assert_eq!(
          rows_out[y][x], rows_in[x][y],
          "Transpose mismatch at ({}, {}): got {}, expected {}",
          x, y, rows_out[y][x], rows_in[x][y],
        );
      }
    }
  }
}
