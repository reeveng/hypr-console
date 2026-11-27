//! NEON row kernels for eight-bit motion compensation.

use archmage::intrinsics::aarch64 as safe_simd;
use archmage::{Arm64, SimdToken, arcane};
use console_core_pixel_buffers::RowLayoutError;
use console_core_video_av1_digital_signal_processing::{
  MotionCompensation, MotionScratch, Prediction, RowFilters, SCRATCH_COLUMNS, VerticalInput,
  apply, copy_pixels, copy_predictors, scalar_vertical_put,
};
use console_core_video_interpolation::convolve8;
use core::arch::aarch64::*;

#[doc = r###"Horizontal 8-tap filter to intermediate buffer (i16)
For ARM NEON: uses multiply-accumulate with 8 coefficients"###]
#[arcane]
fn h_filter_8tap_8bpc_neon(
  _token: Arm64,
  dst: &mut [i16],
  src: &[u8],
  w: usize,
  filter: &[i8; 8],
  sh: u8,
) {
  let rnd = ::core::primitive::i16::wrapping_shr(1i16 << sh, 1_u32);

  let mut col = 0;

  while col + 8 <= w {
    let c0 = <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_explicit_element, ..] => _explicit_element,
    });
    let c1 = <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _explicit_element, ..] => _explicit_element,
    });
    let c2 = <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _explicit_element, ..] => _explicit_element,
    });
    let c3 = <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c4 = <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c5 = <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c6 = <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c7 = <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    });

    let s0 = safe_simd::vld1_u8(src[col..][..8].try_into().unwrap());
    let s1 = safe_simd::vld1_u8(src[col + 1..][..8].try_into().unwrap());
    let s2 = safe_simd::vld1_u8(src[col + 2..][..8].try_into().unwrap());
    let s3 = safe_simd::vld1_u8(src[col + 3..][..8].try_into().unwrap());
    let s4 = safe_simd::vld1_u8(src[col + 4..][..8].try_into().unwrap());
    let s5 = safe_simd::vld1_u8(src[col + 5..][..8].try_into().unwrap());
    let s6 = safe_simd::vld1_u8(src[col + 6..][..8].try_into().unwrap());
    let s7 = safe_simd::vld1_u8(src[col + 7..][..8].try_into().unwrap());

    let s0_16 = vreinterpretq_s16_u16(vmovl_u8(s0));
    let s1_16 = vreinterpretq_s16_u16(vmovl_u8(s1));
    let s2_16 = vreinterpretq_s16_u16(vmovl_u8(s2));
    let s3_16 = vreinterpretq_s16_u16(vmovl_u8(s3));
    let s4_16 = vreinterpretq_s16_u16(vmovl_u8(s4));
    let s5_16 = vreinterpretq_s16_u16(vmovl_u8(s5));
    let s6_16 = vreinterpretq_s16_u16(vmovl_u8(s6));
    let s7_16 = vreinterpretq_s16_u16(vmovl_u8(s7));

    let mut sum = vmulq_n_s16(s0_16, c0);
    sum = vmlaq_n_s16(sum, s1_16, c1);
    sum = vmlaq_n_s16(sum, s2_16, c2);
    sum = vmlaq_n_s16(sum, s3_16, c3);
    sum = vmlaq_n_s16(sum, s4_16, c4);
    sum = vmlaq_n_s16(sum, s5_16, c5);
    sum = vmlaq_n_s16(sum, s6_16, c6);
    sum = vmlaq_n_s16(sum, s7_16, c7);

    let rnd_vec = vdupq_n_s16(rnd);
    let result = vshrq_n_s16::<2>(vaddq_s16(sum, rnd_vec));

    let dst_arr: &mut [i16; 8] = (&mut dst[col..col + 8]).try_into().unwrap();
    safe_simd::vst1q_s16(dst_arr, result);
    col += 8;
  }

  for _ in (::core::ops::Range { start: col, end: w }) {
    let sum = convolve8(filter, |tap| i32::from(src[col.wrapping_add(tap)]));
    dst[col] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
      (sum + i32::from(rnd)) >> sh,
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
    col = (::core::num::Wrapping(col) + ::core::num::Wrapping(1)).0;
  }
}

#[doc = r###"Vertical 8-tap filter from intermediate buffer to output (u8)"###]
#[arcane]
fn v_filter_8tap_8bpc_neon(
  _token: Arm64,
  dst: &mut [u8],
  mid: &[[i16; SCRATCH_COLUMNS]],
  w: usize,
  filter: &[i8; 8],
  sh: u8,
  max: u16,
) {
  let rnd = ::core::primitive::i32::wrapping_shr(1i32 << sh, 1_u32);
  let _ = max;

  let mut col = 0;

  while col + 8 <= w {
    let c0 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_explicit_element, ..] => _explicit_element,
    });
    let c1 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _explicit_element, ..] => _explicit_element,
    });
    let c2 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _explicit_element, ..] => _explicit_element,
    });
    let c3 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c4 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c5 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c6 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c7 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    });

    let r0 = safe_simd::vld1q_s16(mid[0][col..][..8].try_into().unwrap());
    let r1 = safe_simd::vld1q_s16(mid[1][col..][..8].try_into().unwrap());
    let r2 = safe_simd::vld1q_s16(mid[2][col..][..8].try_into().unwrap());
    let r3 = safe_simd::vld1q_s16(mid[3][col..][..8].try_into().unwrap());
    let r4 = safe_simd::vld1q_s16(mid[4][col..][..8].try_into().unwrap());
    let r5 = safe_simd::vld1q_s16(mid[5][col..][..8].try_into().unwrap());
    let r6 = safe_simd::vld1q_s16(mid[6][col..][..8].try_into().unwrap());
    let r7 = safe_simd::vld1q_s16(mid[7][col..][..8].try_into().unwrap());

    let r0_lo = vmovl_s16(vget_low_s16(r0));
    let r0_hi = vmovl_s16(vget_high_s16(r0));
    let r1_lo = vmovl_s16(vget_low_s16(r1));
    let r1_hi = vmovl_s16(vget_high_s16(r1));
    let r2_lo = vmovl_s16(vget_low_s16(r2));
    let r2_hi = vmovl_s16(vget_high_s16(r2));
    let r3_lo = vmovl_s16(vget_low_s16(r3));
    let r3_hi = vmovl_s16(vget_high_s16(r3));
    let r4_lo = vmovl_s16(vget_low_s16(r4));
    let r4_hi = vmovl_s16(vget_high_s16(r4));
    let r5_lo = vmovl_s16(vget_low_s16(r5));
    let r5_hi = vmovl_s16(vget_high_s16(r5));
    let r6_lo = vmovl_s16(vget_low_s16(r6));
    let r6_hi = vmovl_s16(vget_high_s16(r6));
    let r7_lo = vmovl_s16(vget_low_s16(r7));
    let r7_hi = vmovl_s16(vget_high_s16(r7));

    let mut sum_lo = vmulq_n_s32(r0_lo, c0);
    sum_lo = vmlaq_n_s32(sum_lo, r1_lo, c1);
    sum_lo = vmlaq_n_s32(sum_lo, r2_lo, c2);
    sum_lo = vmlaq_n_s32(sum_lo, r3_lo, c3);
    sum_lo = vmlaq_n_s32(sum_lo, r4_lo, c4);
    sum_lo = vmlaq_n_s32(sum_lo, r5_lo, c5);
    sum_lo = vmlaq_n_s32(sum_lo, r6_lo, c6);
    sum_lo = vmlaq_n_s32(sum_lo, r7_lo, c7);

    let mut sum_hi = vmulq_n_s32(r0_hi, c0);
    sum_hi = vmlaq_n_s32(sum_hi, r1_hi, c1);
    sum_hi = vmlaq_n_s32(sum_hi, r2_hi, c2);
    sum_hi = vmlaq_n_s32(sum_hi, r3_hi, c3);
    sum_hi = vmlaq_n_s32(sum_hi, r4_hi, c4);
    sum_hi = vmlaq_n_s32(sum_hi, r5_hi, c5);
    sum_hi = vmlaq_n_s32(sum_hi, r6_hi, c6);
    sum_hi = vmlaq_n_s32(sum_hi, r7_hi, c7);

    let rnd_vec = vdupq_n_s32(rnd);
    sum_lo = vaddq_s32(sum_lo, rnd_vec);
    sum_hi = vaddq_s32(sum_hi, rnd_vec);

    let result_lo = vshrq_n_s32::<10>(sum_lo);
    let result_hi = vshrq_n_s32::<10>(sum_hi);

    let result_16 = vcombine_s16(vqmovn_s32(result_lo), vqmovn_s32(result_hi));

    let result_8 = vqmovun_s16(result_16);

    let dst_arr: &mut [u8; 8] = (&mut dst[col..col + 8]).try_into().unwrap();
    safe_simd::vst1_u8(dst_arr, result_8);
    col += 8;
  }

  for _ in (::core::ops::Range { start: col, end: w }) {
    let sum = convolve8(filter, |tap| i32::from(mid[tap][col]));
    dst[col] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
      ((sum + rnd) >> sh).clamp(0, 255),
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
    col = (::core::num::Wrapping(col) + ::core::num::Wrapping(1)).0;
  }
}

#[doc = r###"Horizontal 8-tap filter directly to output (H-only case)"###]
#[arcane]
fn h_filter_8tap_8bpc_put_neon(
  _token: Arm64,
  dst: &mut [u8],
  src: &[u8],
  w: usize,
  filter: &[i8; 8],
) {
  let mut col = 0;

  while col + 8 <= w {
    let c0 = <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_explicit_element, ..] => _explicit_element,
    });
    let c1 = <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _explicit_element, ..] => _explicit_element,
    });
    let c2 = <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _explicit_element, ..] => _explicit_element,
    });
    let c3 = <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c4 = <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c5 = <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c6 = <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c7 = <i16 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    });

    let s0 = safe_simd::vld1_u8(src[col..][..8].try_into().unwrap());
    let s1 = safe_simd::vld1_u8(src[col + 1..][..8].try_into().unwrap());
    let s2 = safe_simd::vld1_u8(src[col + 2..][..8].try_into().unwrap());
    let s3 = safe_simd::vld1_u8(src[col + 3..][..8].try_into().unwrap());
    let s4 = safe_simd::vld1_u8(src[col + 4..][..8].try_into().unwrap());
    let s5 = safe_simd::vld1_u8(src[col + 5..][..8].try_into().unwrap());
    let s6 = safe_simd::vld1_u8(src[col + 6..][..8].try_into().unwrap());
    let s7 = safe_simd::vld1_u8(src[col + 7..][..8].try_into().unwrap());

    let s0_16 = vreinterpretq_s16_u16(vmovl_u8(s0));
    let s1_16 = vreinterpretq_s16_u16(vmovl_u8(s1));
    let s2_16 = vreinterpretq_s16_u16(vmovl_u8(s2));
    let s3_16 = vreinterpretq_s16_u16(vmovl_u8(s3));
    let s4_16 = vreinterpretq_s16_u16(vmovl_u8(s4));
    let s5_16 = vreinterpretq_s16_u16(vmovl_u8(s5));
    let s6_16 = vreinterpretq_s16_u16(vmovl_u8(s6));
    let s7_16 = vreinterpretq_s16_u16(vmovl_u8(s7));

    let mut sum = vmulq_n_s16(s0_16, c0);
    sum = vmlaq_n_s16(sum, s1_16, c1);
    sum = vmlaq_n_s16(sum, s2_16, c2);
    sum = vmlaq_n_s16(sum, s3_16, c3);
    sum = vmlaq_n_s16(sum, s4_16, c4);
    sum = vmlaq_n_s16(sum, s5_16, c5);
    sum = vmlaq_n_s16(sum, s6_16, c6);
    sum = vmlaq_n_s16(sum, s7_16, c7);

    let rnd_vec = vdupq_n_s16(34);
    let result = vshrq_n_s16::<6>(vaddq_s16(sum, rnd_vec));

    let result_8 = vqmovun_s16(result);
    let dst_arr: &mut [u8; 8] = (&mut dst[col..col + 8]).try_into().unwrap();
    safe_simd::vst1_u8(dst_arr, result_8);
    col += 8;
  }

  for _ in (::core::ops::Range { start: col, end: w }) {
    let sum = convolve8(filter, |tap| i32::from(src[col.wrapping_add(tap)]));
    dst[col] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
      ::core::primitive::i32::wrapping_shr(sum + 34, 6_u32).clamp(0, 255),
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
    col = (::core::num::Wrapping(col) + ::core::num::Wrapping(1)).0;
  }
}

#[doc = r###"DotProd (Arm64V2) horizontal 8-tap filter to intermediate i16 buffer.

Bit-exact replacement for [`h_filter_8tap_8bpc_neon`]. The NEON path
computes, per output column `j`:
`sum_j = Σ_{i=0..8} filter[i] * src[j+i]`, then
`dst[j] = (sum_j + rnd) >> sh` with `rnd = (1<<sh)>>1`.

`vdotq_s32` is a *signed* i8×i8 dot product, but `src` is u8 (0..=255), out
of i8 range. We bias each source byte by `-128` so it fits i8, then add back
the exact correction. For the per-column accumulator:
  `Σ filter[i] * (src[j+i] - 128)`
    `= Σ filter[i]*src[j+i]  -  128 * Σ filter[i]`
so `sum_j = vdot_acc_j + 128 * filter_sum`, where `filter_sum = Σ filter[i]`
is a per-filter constant. Because every product and the correction are
computed in exact i32 arithmetic (no intermediate truncation), the recovered
`sum_j` is identical to the NEON i16-accumulated `sum` for all valid AV1
filters (whose partial sums stay within i16/i32 range — the same range the
NEON path already relies on). The round+shift is then byte-identical.

4 output columns are produced per `vdotq` group; two `vdotq` accumulate taps
0..4 and 4..8 respectively. The source is gathered so each 4-byte lane of the
`b` operand holds `src[j..j+4]` for output column `j` (the dav1d DotProd MC
source layout)."###]
#[cfg(all(target_arch = "aarch64", rav1d_arm_dotprod))]
#[arcane]
fn h_filter_8tap_8bpc_dotprod(
  _token: archmage::Arm64V2Token,
  dst: &mut [i16],
  src: &[u8],
  w: usize,
  filter: &[i8; 8],
  sh: u8,
) {
  let rnd = ((1i32 << sh) >> 1) as i32;

  let filter_sum: i32 = filter.iter().map(|&c| c as i32).sum();
  let bias_corr = 128i32 * filter_sum;

  let f_lo: [i8; 16] = [
    filter[0], filter[1], filter[2], filter[3], filter[0], filter[1], filter[2], filter[3],
    filter[0], filter[1], filter[2], filter[3], filter[0], filter[1], filter[2], filter[3],
  ];
  let f_hi: [i8; 16] = [
    filter[4], filter[5], filter[6], filter[7], filter[4], filter[5], filter[6], filter[7],
    filter[4], filter[5], filter[6], filter[7], filter[4], filter[5], filter[6], filter[7],
  ];
  let vf_lo = safe_simd::vld1q_s8(&f_lo);
  let vf_hi = safe_simd::vld1q_s8(&f_hi);
  let bias = vdupq_n_s8(-128i8);
  let rnd_vec = vdupq_n_s32(rnd + bias_corr);

  let mut col = 0;

  while col + 4 <= w && col + 7 < src.len() {
    let mut win_lo = [0u8; 16];
    let mut win_hi = [0u8; 16];
    for l in 0..4 {
      win_lo[l * 4..l * 4 + 4].copy_from_slice(&src[col + l..col + l + 4]);
      win_hi[l * 4..l * 4 + 4].copy_from_slice(&src[col + l + 4..col + l + 8]);
    }

    let s_lo = vaddq_s8(vreinterpretq_s8_u8(safe_simd::vld1q_u8(&win_lo)), bias);
    let s_hi = vaddq_s8(vreinterpretq_s8_u8(safe_simd::vld1q_u8(&win_hi)), bias);

    let acc = vdotq_s32(vdupq_n_s32(0), vf_lo, s_lo);
    let acc = vdotq_s32(acc, vf_hi, s_hi);

    let sum = vaddq_s32(acc, rnd_vec);
    let res32 = vshrq_n_s32_dyn(sum, sh);
    let res16 = vqmovn_s32(res32);

    let out_arr: &mut [i16; 4] = (&mut dst[col..col + 4]).try_into().unwrap();
    safe_simd::vst1_s16(out_arr, res16);
    col += 4;
  }

  while col < w {
    let sum = convolve8(filter, |tap| i32::from(src[col.wrapping_add(tap)]));
    dst[col] = ((sum + rnd) >> sh) as i16;
    col += 1;
  }
}

#[doc = r###"I8MM (Arm64V3) horizontal 8-tap filter to intermediate i16 buffer.

Bit-exact replacement for [`h_filter_8tap_8bpc_neon`], same as
[`h_filter_8tap_8bpc_dotprod`] but using `vusdotq_s32` (u8×i8 dot product),
which removes the `-128` source bias entirely: the accumulator already holds
the exact `Σ filter[i] * src[j+i]` because `src` is consumed as unsigned and
`filter` as signed. No correction term is needed; round+shift is identical."###]
#[cfg(all(target_arch = "aarch64", rav1d_arm_i8mm))]
#[arcane]
fn h_filter_8tap_8bpc_i8mm(
  _token: archmage::Arm64V3Token,
  dst: &mut [i16],
  src: &[u8],
  w: usize,
  filter: &[i8; 8],
  sh: u8,
) {
  let rnd = ((1i32 << sh) >> 1) as i32;

  let f_lo: [i8; 16] = [
    filter[0], filter[1], filter[2], filter[3], filter[0], filter[1], filter[2], filter[3],
    filter[0], filter[1], filter[2], filter[3], filter[0], filter[1], filter[2], filter[3],
  ];
  let f_hi: [i8; 16] = [
    filter[4], filter[5], filter[6], filter[7], filter[4], filter[5], filter[6], filter[7],
    filter[4], filter[5], filter[6], filter[7], filter[4], filter[5], filter[6], filter[7],
  ];
  let vf_lo = safe_simd::vld1q_s8(&f_lo);
  let vf_hi = safe_simd::vld1q_s8(&f_hi);
  let rnd_vec = vdupq_n_s32(rnd);

  let mut col = 0;
  while col + 4 <= w && col + 7 < src.len() {
    let mut win_lo = [0u8; 16];
    let mut win_hi = [0u8; 16];
    for l in 0..4 {
      win_lo[l * 4..l * 4 + 4].copy_from_slice(&src[col + l..col + l + 4]);
      win_hi[l * 4..l * 4 + 4].copy_from_slice(&src[col + l + 4..col + l + 8]);
    }
    let s_lo = safe_simd::vld1q_u8(&win_lo);
    let s_hi = safe_simd::vld1q_u8(&win_hi);

    let acc = vusdotq_s32(vdupq_n_s32(0), s_lo, vf_lo);
    let acc = vusdotq_s32(acc, s_hi, vf_hi);

    let sum = vaddq_s32(acc, rnd_vec);
    let res32 = vshrq_n_s32_dyn(sum, sh);
    let res16 = vqmovn_s32(res32);
    let out_arr: &mut [i16; 4] = (&mut dst[col..col + 4]).try_into().unwrap();
    safe_simd::vst1_s16(out_arr, res16);
    col += 4;
  }

  while col < w {
    let sum = convolve8(filter, |tap| i32::from(src[col.wrapping_add(tap)]));
    dst[col] = ((sum + rnd) >> sh) as i16;
    col += 1;
  }
}

#[doc = r###"Dynamic right-shift for `int32x4_t` by a runtime amount, expressed via the
const-generic `vshrq_n_s32` (which requires an immediate). MC H-filter `sh`
is always 2 (`6 - intermediate_bits`); branch on the values rav1d uses so
every arm passes a const. Kept `#[rite]` so it inlines into the caller's
target-feature region."###]
#[cfg(all(target_arch = "aarch64", any(rav1d_arm_dotprod, rav1d_arm_i8mm)))]
#[archmage::rite(neon)]
fn vshrq_n_s32_dyn(v: int32x4_t, sh: u8) -> int32x4_t {
  match sh {
    2 => vshrq_n_s32::<2>(v),
    4 => vshrq_n_s32::<4>(v),
    6 => vshrq_n_s32::<6>(v),
    _ => vshlq_s32(v, vdupq_n_s32(-(sh as i32))),
  }
}

#[doc = r###"Vertical 8-tap filter directly from source (V-only case)"###]
#[arcane]
fn v_filter_8tap_8bpc_direct_neon(
  _token: Arm64,
  dst: &mut [u8],
  src: &[u8],
  src_stride: usize,
  w: usize,
  filter: &[i8; 8],
) {
  let mut col = 0;

  while col + 8 <= w {
    let c0 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_explicit_element, ..] => _explicit_element,
    });
    let c1 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _explicit_element, ..] => _explicit_element,
    });
    let c2 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _explicit_element, ..] => _explicit_element,
    });
    let c3 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c4 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c5 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c6 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c7 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    });

    let r0 = safe_simd::vld1_u8(src[col..][..8].try_into().unwrap());
    let r1 = safe_simd::vld1_u8(src[col + src_stride..][..8].try_into().unwrap());
    let r2 = safe_simd::vld1_u8(src[col + 2 * src_stride..][..8].try_into().unwrap());
    let r3 = safe_simd::vld1_u8(src[col + 3 * src_stride..][..8].try_into().unwrap());
    let r4 = safe_simd::vld1_u8(src[col + 4 * src_stride..][..8].try_into().unwrap());
    let r5 = safe_simd::vld1_u8(src[col + 5 * src_stride..][..8].try_into().unwrap());
    let r6 = safe_simd::vld1_u8(src[col + 6 * src_stride..][..8].try_into().unwrap());
    let r7 = safe_simd::vld1_u8(src[col + 7 * src_stride..][..8].try_into().unwrap());

    let r0_16 = vreinterpretq_s16_u16(vmovl_u8(r0));
    let r1_16 = vreinterpretq_s16_u16(vmovl_u8(r1));
    let r2_16 = vreinterpretq_s16_u16(vmovl_u8(r2));
    let r3_16 = vreinterpretq_s16_u16(vmovl_u8(r3));
    let r4_16 = vreinterpretq_s16_u16(vmovl_u8(r4));
    let r5_16 = vreinterpretq_s16_u16(vmovl_u8(r5));
    let r6_16 = vreinterpretq_s16_u16(vmovl_u8(r6));
    let r7_16 = vreinterpretq_s16_u16(vmovl_u8(r7));

    let r0_lo = vmovl_s16(vget_low_s16(r0_16));
    let r1_lo = vmovl_s16(vget_low_s16(r1_16));
    let r2_lo = vmovl_s16(vget_low_s16(r2_16));
    let r3_lo = vmovl_s16(vget_low_s16(r3_16));
    let r4_lo = vmovl_s16(vget_low_s16(r4_16));
    let r5_lo = vmovl_s16(vget_low_s16(r5_16));
    let r6_lo = vmovl_s16(vget_low_s16(r6_16));
    let r7_lo = vmovl_s16(vget_low_s16(r7_16));

    let r0_hi = vmovl_s16(vget_high_s16(r0_16));
    let r1_hi = vmovl_s16(vget_high_s16(r1_16));
    let r2_hi = vmovl_s16(vget_high_s16(r2_16));
    let r3_hi = vmovl_s16(vget_high_s16(r3_16));
    let r4_hi = vmovl_s16(vget_high_s16(r4_16));
    let r5_hi = vmovl_s16(vget_high_s16(r5_16));
    let r6_hi = vmovl_s16(vget_high_s16(r6_16));
    let r7_hi = vmovl_s16(vget_high_s16(r7_16));

    let mut sum_lo = vmulq_n_s32(r0_lo, c0);
    sum_lo = vmlaq_n_s32(sum_lo, r1_lo, c1);
    sum_lo = vmlaq_n_s32(sum_lo, r2_lo, c2);
    sum_lo = vmlaq_n_s32(sum_lo, r3_lo, c3);
    sum_lo = vmlaq_n_s32(sum_lo, r4_lo, c4);
    sum_lo = vmlaq_n_s32(sum_lo, r5_lo, c5);
    sum_lo = vmlaq_n_s32(sum_lo, r6_lo, c6);
    sum_lo = vmlaq_n_s32(sum_lo, r7_lo, c7);

    let mut sum_hi = vmulq_n_s32(r0_hi, c0);
    sum_hi = vmlaq_n_s32(sum_hi, r1_hi, c1);
    sum_hi = vmlaq_n_s32(sum_hi, r2_hi, c2);
    sum_hi = vmlaq_n_s32(sum_hi, r3_hi, c3);
    sum_hi = vmlaq_n_s32(sum_hi, r4_hi, c4);
    sum_hi = vmlaq_n_s32(sum_hi, r5_hi, c5);
    sum_hi = vmlaq_n_s32(sum_hi, r6_hi, c6);
    sum_hi = vmlaq_n_s32(sum_hi, r7_hi, c7);

    let rnd_vec = vdupq_n_s32(32);
    sum_lo = vshrq_n_s32::<6>(vaddq_s32(sum_lo, rnd_vec));
    sum_hi = vshrq_n_s32::<6>(vaddq_s32(sum_hi, rnd_vec));

    let result_16 = vcombine_s16(vqmovn_s32(sum_lo), vqmovn_s32(sum_hi));
    let result_8 = vqmovun_s16(result_16);
    let dst_arr: &mut [u8; 8] = (&mut dst[col..col + 8]).try_into().unwrap();
    safe_simd::vst1_u8(dst_arr, result_8);
    col += 8;
  }

  for _ in (::core::ops::Range { start: col, end: w }) {
    let sum = convolve8(filter, |tap| {
      i32::from(src[col.wrapping_add(tap.wrapping_mul(src_stride))])
    });
    dst[col] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
      ::core::primitive::i32::wrapping_shr(sum + 32, 6_u32).clamp(0, 255),
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
    col = (::core::num::Wrapping(col) + ::core::num::Wrapping(1)).0;
  }
}

#[doc = r###"Main 8-tap put function for 8bpc"###]
#[doc = r###"Tier-selecting wrapper for the horizontal 8-tap → i16 filter.

Prefers the I8MM (`Arm64V3`) path, then DotProd (`Arm64V2`), then baseline
NEON. Each higher tier is gated behind its rustc cfg (`rav1d_arm_i8mm` /
`rav1d_arm_dotprod`) which is OFF until the underlying `core::arch`
intrinsics stabilize on the project's stable toolchain (see the DotProd /
I8MM section above). On the default stable build this compiles to exactly
the NEON call — zero behavioural change. `#[rite]` so it inlines into the
`#[arcane]` caller's target-feature region."###]
#[doc = r###"Vertical 8-tap filter from intermediate buffer to i16 output"###]
#[arcane]
fn v_filter_8tap_to_i16_neon(
  _token: Arm64,
  dst: &mut [i16],
  mid: &[[i16; SCRATCH_COLUMNS]],
  w: usize,
  filter: &[i8; 8],
  sh: u8,
) {
  let rnd = ::core::primitive::i32::wrapping_shr(1i32 << sh, 1_u32);

  let mut col = 0;

  while col + 4 <= w {
    let c0 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_explicit_element, ..] => _explicit_element,
    });
    let c1 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _explicit_element, ..] => _explicit_element,
    });
    let c2 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _explicit_element, ..] => _explicit_element,
    });
    let c3 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c4 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c5 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c6 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    });
    let c7 = <i32 as ::core::convert::From<::core::primitive::i8>>::from(match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    });

    let r0 = safe_simd::vld1_s16(mid[0][col..][..4].try_into().unwrap());
    let r1 = safe_simd::vld1_s16(mid[1][col..][..4].try_into().unwrap());
    let r2 = safe_simd::vld1_s16(mid[2][col..][..4].try_into().unwrap());
    let r3 = safe_simd::vld1_s16(mid[3][col..][..4].try_into().unwrap());
    let r4 = safe_simd::vld1_s16(mid[4][col..][..4].try_into().unwrap());
    let r5 = safe_simd::vld1_s16(mid[5][col..][..4].try_into().unwrap());
    let r6 = safe_simd::vld1_s16(mid[6][col..][..4].try_into().unwrap());
    let r7 = safe_simd::vld1_s16(mid[7][col..][..4].try_into().unwrap());

    let r0_32 = vmovl_s16(r0);
    let r1_32 = vmovl_s16(r1);
    let r2_32 = vmovl_s16(r2);
    let r3_32 = vmovl_s16(r3);
    let r4_32 = vmovl_s16(r4);
    let r5_32 = vmovl_s16(r5);
    let r6_32 = vmovl_s16(r6);
    let r7_32 = vmovl_s16(r7);

    let mut sum = vmulq_n_s32(r0_32, c0);
    sum = vmlaq_n_s32(sum, r1_32, c1);
    sum = vmlaq_n_s32(sum, r2_32, c2);
    sum = vmlaq_n_s32(sum, r3_32, c3);
    sum = vmlaq_n_s32(sum, r4_32, c4);
    sum = vmlaq_n_s32(sum, r5_32, c5);
    sum = vmlaq_n_s32(sum, r6_32, c6);
    sum = vmlaq_n_s32(sum, r7_32, c7);

    let rnd_vec = vdupq_n_s32(rnd);

    sum = vshlq_s32(
      vaddq_s32(sum, rnd_vec),
      vdupq_n_s32(::core::primitive::i32::wrapping_neg(
        <i32 as ::core::convert::From<::core::primitive::u8>>::from(sh),
      )),
    );

    let result = vqmovn_s32(sum);
    let dst_arr: &mut [i16; 4] = (&mut dst[col..col + 4]).try_into().unwrap();
    safe_simd::vst1_s16(dst_arr, result);
    col += 4;
  }

  for _ in (::core::ops::Range { start: col, end: w }) {
    let sum = convolve8(filter, |tap| i32::from(mid[tap][col]));
    dst[col] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
      (sum + rnd) >> sh,
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
    col = (::core::num::Wrapping(col) + ::core::num::Wrapping(1)).0;
  }
}

#[doc = r###"Main 8-tap prep function for 8bpc"###]
#[cfg(all(test, any(rav1d_arm_dotprod, rav1d_arm_i8mm)))]
mod tests {
  #[doc = r###"R1 CI bit-exactness: compare the DotProd / I8MM H-filter kernels against
the baseline NEON kernel on seeded input. Only compiled when the
higher-tier cfgs are enabled (the underlying intrinsics are nightly-only;
see the DotProd/I8MM section in this module). Runs on real aarch64 in CI;
the host (x86) cross-compiles but cannot execute it."###]
  #[test]
  #[cfg(all(target_arch = "aarch64", any(rav1d_arm_dotprod, rav1d_arm_i8mm)))]
  fn test_arm_h_filter_8tap_matches_neon() {
    use super::{SCRATCH_COLUMNS, h_filter_8tap_8bpc_neon};
    use archmage::{Arm64, SimdToken};

    let token = Arm64::summon().expect("NEON always available on aarch64");
    use console_core_video_av1_digital_signal_processing::{TapTable, taps};

    let filters = [
      (TapTable::Regular, 1),
      (TapTable::Sharp, 8),
      (TapTable::Smooth, 6),
      (TapTable::NarrowRegular, 12),
    ]
    .map(|(table, phase)| {
      let Ok(taps) = taps(table, phase);
      *taps.expect("fractional phase")
    });
    let mut rng: u64 = 0xdead_beef_cafe_0001;
    let mut next = || {
      rng ^= rng << 13;
      rng ^= rng >> 7;
      rng ^= rng << 17;
      (rng & 0xff) as u8
    };

    for &w in &[4usize, 8, 16, 32, 64, 128] {
      for f in &filters {
        let src_len = w + 7 + 8;
        let src: Vec<u8> = (0..src_len).map(|_| next()).collect();
        let sh = 2u8;

        let mut ref_out = [0i16; SCRATCH_COLUMNS];
        h_filter_8tap_8bpc_neon(token, &mut ref_out[..w], &src, w, f, sh);

        #[cfg(rav1d_arm_dotprod)]
        {
          let tier = archmage::Arm64V2Token::summon();

          match tier {
            Some(available) => {
              let mut got = [0i16; SCRATCH_COLUMNS];
              super::h_filter_8tap_8bpc_dotprod(available, &mut got[..w], &src, w, f, sh);
              assert_eq!(&got[..w], &ref_out[..w], "dotprod mismatch w={w} f={f:?}");
            }
            None => {}
          }
        }
        #[cfg(rav1d_arm_i8mm)]
        {
          let tier = archmage::Arm64V3Token::summon();

          match tier {
            Some(available) => {
              let mut got = [0i16; SCRATCH_COLUMNS];
              super::h_filter_8tap_8bpc_i8mm(available, &mut got[..w], &src, w, f, sh);
              assert_eq!(&got[..w], &ref_out[..w], "i8mm mismatch w={w} f={f:?}");
            }
            None => {}
          }
        }
      }
    }
  }
}

enum HorizontalFilter {
  Neon(Arm64),
  #[cfg(rav1d_arm_dotprod)]
  DotProduct(archmage::Arm64V2Token),
  #[cfg(rav1d_arm_i8mm)]
  IntegerMatrixMultiply(archmage::Arm64V3Token),
}

impl HorizontalFilter {
  fn apply(
    &self,
    output: &mut [i16],
    input: &[u8],
    taps: &[i8; 8],
  ) -> Result<(), RowLayoutError> {
    let width = output.len();
    match *self {
      Self::Neon(token) => h_filter_8tap_8bpc_neon(token, output, input, width, taps, 2),
      #[cfg(rav1d_arm_dotprod)]
      Self::DotProduct(token) => h_filter_8tap_8bpc_dotprod(token, output, input, width, taps, 2),
      #[cfg(rav1d_arm_i8mm)]
      Self::IntegerMatrixMultiply(token) => {
        h_filter_8tap_8bpc_i8mm(token, output, input, width, taps, 2)
      }
    }
    Ok(())
  }
}

fn put(
  token: Arm64,
  horizontal: HorizontalFilter,
  prediction: Prediction<'_, u8>,
  scratch: &mut MotionScratch,
) -> Result<(), RowLayoutError> {
  apply(
    prediction,
    scratch,
    RowFilters {
      copy: copy_pixels,
      horizontal: |output: &mut [u8], input: &[u8], taps: &[i8; 8]| {
        h_filter_8tap_8bpc_put_neon(token, output, input, output.len(), taps);
        Ok(())
      },
      intermediate: |output: &mut [i16], input: &[u8], taps: &[i8; 8]| {
        horizontal.apply(output, input, taps)
      },
      vertical: |output: &mut [u8],
                 input: VerticalInput<'_>,
                 taps: &[i8; 8],
                 _: &mut [[i16; SCRATCH_COLUMNS]; 8]| {
        match input.forward_span()? {
          Some((pixels, pitch)) => {
            let pitch = usize::try_from(pitch).map_err(|_| RowLayoutError::Stride)?;
            v_filter_8tap_8bpc_direct_neon(token, output, pixels, pitch, output.len(), taps);
            Ok(())
          }
          None => scalar_vertical_put(output, input, taps),
        }
      },
      separable: |output: &mut [u8], input: &[[i16; SCRATCH_COLUMNS]], taps: &[i8; 8]| {
        v_filter_8tap_8bpc_neon(token, output, input, output.len(), taps, 10, 255);
        Ok(())
      },
    },
  )
}

fn prep(
  token: Arm64,
  horizontal: HorizontalFilter,
  prediction: Prediction<'_, i16>,
  scratch: &mut MotionScratch,
) -> Result<(), RowLayoutError> {
  let horizontal =
    |output: &mut [i16], input: &[u8], taps: &[i8; 8]| horizontal.apply(output, input, taps);
  apply(
    prediction,
    scratch,
    RowFilters {
      copy: copy_predictors,
      horizontal,
      intermediate: horizontal,
      vertical: |output: &mut [i16],
                 input: VerticalInput<'_>,
                 taps: &[i8; 8],
                 intermediate: &mut [[i16; SCRATCH_COLUMNS]; 8]| {
        for (line, source) in intermediate.iter_mut().zip(input.rows()?) {
          copy_predictors(line, source)?;
        }
        v_filter_8tap_to_i16_neon(token, output, intermediate, output.len(), taps, 6);
        Ok(())
      },
      separable: |output: &mut [i16], input: &[[i16; SCRATCH_COLUMNS]], taps: &[i8; 8]| {
        v_filter_8tap_to_i16_neon(token, output, input, output.len(), taps, 6);
        Ok(())
      },
    },
  )
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "the three AArch64 tiers install the same row operations with different feature tokens"
  )
)]
macro_rules! motion_compensation {
  ($install:ident, $put:ident, $prep:ident, $token:ty, $horizontal:ident) => {
    pub fn $install(table: &mut MotionCompensation) {
      match (Arm64::summon(), <$token>::summon()) {
        (Some(_), Some(_)) => {
          *table = MotionCompensation {
            put: $put,
            prep: $prep,
          }
        }
        _ => {}
      }
    }

    fn $put(
      prediction: Prediction<'_, u8>,
      scratch: &mut MotionScratch,
    ) -> Result<(), RowLayoutError> {
      match (Arm64::summon(), <$token>::summon()) {
        (Some(neon), Some(tier)) => put(
          neon,
          HorizontalFilter::$horizontal(tier),
          prediction,
          scratch,
        ),
        _ => (MotionCompensation::default().put)(prediction, scratch),
      }
    }

    fn $prep(
      prediction: Prediction<'_, i16>,
      scratch: &mut MotionScratch,
    ) -> Result<(), RowLayoutError> {
      match (Arm64::summon(), <$token>::summon()) {
        (Some(neon), Some(tier)) => prep(
          neon,
          HorizontalFilter::$horizontal(tier),
          prediction,
          scratch,
        ),
        _ => (MotionCompensation::default().prep)(prediction, scratch),
      }
    }
  };
}

motion_compensation!(install_neon, put_neon, prep_neon, Arm64, Neon);
#[cfg(rav1d_arm_dotprod)]
motion_compensation!(
  install_dot_product,
  put_dot_product,
  prep_dot_product,
  archmage::Arm64V2Token,
  DotProduct
);
#[cfg(rav1d_arm_i8mm)]
motion_compensation!(
  install_integer_matrix_multiply,
  put_integer_matrix_multiply,
  prep_integer_matrix_multiply,
  archmage::Arm64V3Token,
  IntegerMatrixMultiply
);
