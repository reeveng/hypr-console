//! Safe SIMD implementations of motion compensation functions for ARM NEON
#![cfg_attr(not(feature = "unchecked"), forbid(unsafe_code))]
#![cfg_attr(feature = "unchecked", deny(unsafe_code))]
//!
//! These use archmage tokens to safely invoke NEON intrinsics.
//! The extern "C" wrappers are used for FFI compatibility with rav1d's dispatch system.

#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;

#[cfg(target_arch = "aarch64")]
use archmage::{Arm64, SimdToken, arcane};

#[cfg(target_arch = "aarch64")]
use archmage::intrinsics::aarch64 as safe_simd;

use crate::include::common::bitdepth::BitDepth;
use crate::include::dav1d::headers::Rav1dFilterMode;
#[cfg(target_arch = "aarch64")]
use crate::include::dav1d::headers::Rav1dPixelLayoutSubSampled;
use crate::include::dav1d::picture::PicOffset;
use crate::src::internal::CompoundPrediction;
use crate::src::internal::MAX_COMPOUND_SAMPLES;
#[cfg(target_arch = "aarch64")]
use crate::src::internal::SegmentMask;
use crate::src::levels::Filter2d;
#[cfg(target_arch = "aarch64")]
use crate::src::mc::get_filter as get_filter_coeff;
use crate::src::safe_simd::pixel_access::Flex;
use crate::src::strided::Strided as _;
#[cfg(target_arch = "aarch64")]
use console_core_video_interpolation::convolve8;

use crate::src::safe_simd::mc::blend_rows::{BlendLayoutError, BlendRow, BlendSize};
use crate::src::safe_simd::mc::{WordTapDirection, filter_word_tap_row};
use console_core_geometry::Size;
use console_core_number_conversion::wrapped;
use console_core_simd::Simd;
use zerocopy::{FromBytes, IntoBytes, KnownLayout};

struct CompoundBlock<'a> {
  output: &'a mut [u8],
  predictors: [&'a [i16]; 2],
  extent: Size<i32>,
  pitch: u64,
}

impl<'a> CompoundBlock<'a> {
  fn rows<Pixel: FromBytes + IntoBytes + KnownLayout + 'a>(
    self,
  ) -> Result<impl Iterator<Item = BlendRow<'a, Pixel>>, BlendLayoutError> {
    let shape = BlendSize::new(self.extent)?;
    shape.blend_rows::<Pixel>(self.predictors, self.output, self.pitch)
  }
}

#[arcane]
fn avg_8bpc_inner(_token: Arm64, block: CompoundBlock<'_>) -> Result<(), BlendLayoutError> {
  let rows = block.rows::<u8>()?;

  for mut row in rows {
    let Ok(wide) = row.blocks::<16>();

    for (output, left, right) in wide {
      let [left_low, left_high]: &[[i16; 8]; 2] = zerocopy::transmute_ref!(left);
      let [right_low, right_high]: &[[i16; 8]; 2] = zerocopy::transmute_ref!(right);
      let low = vaddq_s16(
        safe_simd::vld1q_s16(left_low),
        safe_simd::vld1q_s16(right_low),
      );
      let high = vaddq_s16(
        safe_simd::vld1q_s16(left_high),
        safe_simd::vld1q_s16(right_high),
      );
      let low_bytes = vqmovun_s16(vqrdmulhq_n_s16(low, 1024));
      let high_bytes = vqmovun_s16(vqrdmulhq_n_s16(high, 1024));
      safe_simd::vst1q_u8(output, vcombine_u8(low_bytes, high_bytes));
    }

    let Ok(narrow) = row.blocks::<8>();

    for (output, left, right) in narrow {
      let sum = vaddq_s16(safe_simd::vld1q_s16(left), safe_simd::vld1q_s16(right));
      safe_simd::vst1_u8(output, vqmovun_s16(vqrdmulhq_n_s16(sum, 1024)));
    }

    let Ok(pixels) = row.pixels();

    for (output, &left, &right) in pixels {
      let sum = i32::from(left).wrapping_add(i32::from(right));
      let rounded = sum.wrapping_mul(1024).wrapping_add(16384).wrapping_shr(15);
      let Ok(sample) = wrapped(rounded.clamp(0, 255));
      *output = sample;
    }
  }

  Ok(())
}

#[arcane]
fn avg_16bpc_inner(
  _token: Arm64,
  block: CompoundBlock<'_>,
  maximum: i32,
) -> Result<(), BlendLayoutError> {
  let Ok(intermediate) = intermediate_bits_16bpc(maximum);
  let precision = intermediate.cast_unsigned();
  let shift = precision.wrapping_add(1);
  let rounding = 1_i32
    .wrapping_shl(precision)
    .wrapping_add(PREP_BIAS_16BPC.wrapping_mul(2));
  let bias = vdupq_n_s32(rounding);
  let right_shift = vdupq_n_s32(shift.cast_signed().wrapping_neg());
  let zero = vdupq_n_s32(0);
  let maximum_vector = vdupq_n_s32(maximum);
  let rows = block.rows::<u16>()?;

  for mut row in rows {
    let Ok(vectors) = row.blocks::<8>();

    for (output, left, right) in vectors {
      let first = safe_simd::vld1q_s16(left);
      let second = safe_simd::vld1q_s16(right);
      let first_low = vmovl_s16(vget_low_s16(first));
      let first_high = vmovl_s16(vget_high_s16(first));
      let second_low = vmovl_s16(vget_low_s16(second));
      let second_high = vmovl_s16(vget_high_s16(second));
      let mean_low = vshlq_s32(
        vaddq_s32(vaddq_s32(first_low, second_low), bias),
        right_shift,
      );
      let mean_high = vshlq_s32(
        vaddq_s32(vaddq_s32(first_high, second_high), bias),
        right_shift,
      );
      let low = vmaxq_s32(vminq_s32(mean_low, maximum_vector), zero);
      let high = vmaxq_s32(vminq_s32(mean_high, maximum_vector), zero);
      let words = vcombine_u16(
        vmovn_u32(vreinterpretq_u32_s32(low)),
        vmovn_u32(vreinterpretq_u32_s32(high)),
      );
      safe_simd::vst1q_u16(output, words);
    }

    let Ok(pixels) = row.pixels();

    for (output, &left, &right) in pixels {
      let sum = i32::from(left).wrapping_add(i32::from(right));
      let rounded = sum.wrapping_add(rounding).wrapping_shr(shift);
      let Ok(sample) = wrapped(rounded.clamp(0, maximum));
      *output = sample;
    }
  }

  Ok(())
}

#[arcane]
fn w_avg_8bpc_inner(
  _token: Arm64,
  block: CompoundBlock<'_>,
  weight: i32,
) -> Result<(), BlendLayoutError> {
  const ROUNDING: i32 = 128;
  let scale = vdupq_n_s32(weight);
  let bias = vdupq_n_s32(ROUNDING);
  let other_weight = 16_i32.wrapping_sub(weight);
  let rows = block.rows::<u8>()?;

  for mut row in rows {
    let Ok(vectors) = row.blocks::<8>();

    for (output, left, right) in vectors {
      let first = safe_simd::vld1q_s16(left);
      let second = safe_simd::vld1q_s16(right);
      let difference = vsubq_s16(first, second);
      let diff_low = vmovl_s16(vget_low_s16(difference));
      let diff_high = vmovl_s16(vget_high_s16(difference));
      let second_low = vmovl_s16(vget_low_s16(second));
      let second_high = vmovl_s16(vget_high_s16(second));
      let sum_low = vaddq_s32(
        vaddq_s32(vmulq_s32(diff_low, scale), vshlq_n_s32::<4>(second_low)),
        bias,
      );
      let sum_high = vaddq_s32(
        vaddq_s32(vmulq_s32(diff_high, scale), vshlq_n_s32::<4>(second_high)),
        bias,
      );
      let low = vmovn_s32(vshrq_n_s32::<8>(sum_low));
      let high = vmovn_s32(vshrq_n_s32::<8>(sum_high));
      safe_simd::vst1_u8(output, vqmovun_s16(vcombine_s16(low, high)));
    }

    let Ok(pixels) = row.pixels();

    for (output, &left, &right) in pixels {
      let first = i32::from(left).wrapping_mul(weight);
      let second = i32::from(right).wrapping_mul(other_weight);
      let rounded = first
        .wrapping_add(second)
        .wrapping_add(ROUNDING)
        .wrapping_shr(8);
      let Ok(sample) = wrapped(rounded.clamp(0, 255));
      *output = sample;
    }
  }

  Ok(())
}

struct WordAverageScale {
  weight: i32,
  maximum: i32,
}

#[arcane]
fn w_avg_16bpc_inner(
  _token: Arm64,
  block: CompoundBlock<'_>,
  scale: WordAverageScale,
) -> Result<(), BlendLayoutError> {
  let WordAverageScale { weight, maximum } = scale;
  let Ok(intermediate) = intermediate_bits_16bpc(maximum);
  let precision = intermediate.cast_unsigned();
  let shift = precision.wrapping_add(4);
  let rounding = 8_i32
    .wrapping_shl(precision)
    .wrapping_add(PREP_BIAS_16BPC.wrapping_mul(16));
  let other_weight = 16_i32.wrapping_sub(weight);
  let first_weight = vdupq_n_s32(weight);
  let second_weight = vdupq_n_s32(other_weight);
  let right_shift = vdupq_n_s32(shift.cast_signed().wrapping_neg());
  let bias = vdupq_n_s32(rounding);
  let zero = vdupq_n_s32(0);
  let maximum_vector = vdupq_n_s32(maximum);
  let rows = block.rows::<u16>()?;

  for mut row in rows {
    let Ok(vectors) = row.blocks::<4>();

    for (output, left, right) in vectors {
      let first = vmovl_s16(safe_simd::vld1_s16(left));
      let second = vmovl_s16(safe_simd::vld1_s16(right));
      let sum = vaddq_s32(
        vaddq_s32(
          vmulq_s32(first, first_weight),
          vmulq_s32(second, second_weight),
        ),
        bias,
      );
      let weighted = vshlq_s32(sum, right_shift);
      let pixels = vmaxq_s32(vminq_s32(weighted, maximum_vector), zero);
      safe_simd::vst1_u16(output, vmovn_u32(vreinterpretq_u32_s32(pixels)));
    }

    let Ok(pixels) = row.pixels();

    for (output, &left, &right) in pixels {
      let first = i32::from(left).wrapping_mul(weight);
      let second = i32::from(right).wrapping_mul(other_weight);
      let rounded = first
        .wrapping_add(second)
        .wrapping_add(rounding)
        .wrapping_shr(shift);
      let Ok(sample) = wrapped(rounded.clamp(0, maximum));
      *output = sample;
    }
  }

  Ok(())
}

#[doc = r###"`14 - bitdepth`: 4 at 10bpc, 2 at 12bpc. `bitdepth_max` is 1023 or 4095."###]
#[cfg(target_arch = "aarch64")]
#[inline(always)]
#[must_use]
fn intermediate_bits_16bpc(bitdepth_max: i32) -> Result<i32, console_core_never::Never> {
  let Ok(maximum) = wrapped::<_, u16>(bitdepth_max);
  let missing_bits = maximum.leading_zeros().cast_signed();
  Ok(missing_bits.wrapping_sub(2))
}

#[arcane]
fn mask_8bpc_inner(
  _token: Arm64,
  block: CompoundBlock<'_>,
  weights: &[u8],
) -> Result<(), BlendLayoutError> {
  const ROUNDING: i32 = 512;
  let bias = vdupq_n_s32(ROUNDING);
  let shape = BlendSize::new(block.extent)?;
  let alphas = shape.input_rows(weights)?;
  let rows = block.rows::<u8>()?;

  for (mut row, mask_row) in rows.zip(alphas) {
    let Ok(vectors) = row.blocks::<8>();
    let (weight_vectors, weight_tail) = mask_row.as_chunks::<8>();

    for ((output, left, right), weights_block) in vectors.zip(weight_vectors) {
      let first = safe_simd::vld1q_s16(left);
      let second = safe_simd::vld1q_s16(right);
      let difference = vsubq_s16(first, second);
      let weights_short = vreinterpretq_s16_u16(vmovl_u8(safe_simd::vld1_u8(weights_block)));
      let diff_low = vmovl_s16(vget_low_s16(difference));
      let diff_high = vmovl_s16(vget_high_s16(difference));
      let weight_low = vmovl_s16(vget_low_s16(weights_short));
      let weight_high = vmovl_s16(vget_high_s16(weights_short));
      let second_low = vmovl_s16(vget_low_s16(second));
      let second_high = vmovl_s16(vget_high_s16(second));
      let sum_low = vaddq_s32(
        vaddq_s32(
          vmulq_s32(diff_low, weight_low),
          vshlq_n_s32::<6>(second_low),
        ),
        bias,
      );
      let sum_high = vaddq_s32(
        vaddq_s32(
          vmulq_s32(diff_high, weight_high),
          vshlq_n_s32::<6>(second_high),
        ),
        bias,
      );
      let low = vmovn_s32(vshrq_n_s32::<10>(sum_low));
      let high = vmovn_s32(vshrq_n_s32::<10>(sum_high));
      safe_simd::vst1_u8(output, vqmovun_s16(vcombine_s16(low, high)));
    }

    let Ok(pixels) = row.pixels();

    for ((output, &left, &right), &weight) in pixels.zip(weight_tail) {
      let alpha = i32::from(weight);
      let first = i32::from(left).wrapping_mul(alpha);
      let second = i32::from(right).wrapping_mul(64_i32.wrapping_sub(alpha));
      let rounded = first
        .wrapping_add(second)
        .wrapping_add(ROUNDING)
        .wrapping_shr(10);
      let Ok(sample) = wrapped(rounded.clamp(0, 255));
      *output = sample;
    }
  }

  Ok(())
}

#[arcane]
fn mask_16bpc_inner(
  _token: Arm64,
  block: CompoundBlock<'_>,
  weights: &[u8],
  maximum: i32,
) -> Result<(), BlendLayoutError> {
  let Ok(intermediate) = intermediate_bits_16bpc(maximum);
  let precision = intermediate.cast_unsigned();
  let shift = precision.wrapping_add(6);
  let rounding = 32_i32
    .wrapping_shl(precision)
    .wrapping_add(PREP_BIAS_16BPC.wrapping_mul(64));
  let right_shift = vdupq_n_s32(shift.cast_signed().wrapping_neg());
  let bias = vdupq_n_s32(rounding);
  let full = vdupq_n_s32(64);
  let zero = vdupq_n_s32(0);
  let maximum_vector = vdupq_n_s32(maximum);
  let shape = BlendSize::new(block.extent)?;
  let alphas = shape.input_rows(weights)?;
  let rows = block.rows::<u16>()?;

  for (mut row, mask_row) in rows.zip(alphas) {
    let Ok(vectors) = row.blocks::<4>();
    let (weight_vectors, weight_tail) = mask_row.as_chunks::<4>();

    for ((output, left, right), &[a, b, c, d]) in vectors.zip(weight_vectors) {
      let first = vmovl_s16(safe_simd::vld1_s16(left));
      let second = vmovl_s16(safe_simd::vld1_s16(right));
      let weights_bytes = [a, b, c, d, 0, 0, 0, 0];
      let weights_words = vmovl_u8(safe_simd::vld1_u8(&weights_bytes));
      let first_weight = vreinterpretq_s32_u32(vmovl_u16(vget_low_u16(weights_words)));
      let second_weight = vsubq_s32(full, first_weight);
      let sum = vaddq_s32(
        vaddq_s32(
          vmulq_s32(first, first_weight),
          vmulq_s32(second, second_weight),
        ),
        bias,
      );
      let weighted = vshlq_s32(sum, right_shift);
      let samples = vmaxq_s32(vminq_s32(weighted, maximum_vector), zero);
      safe_simd::vst1_u16(output, vmovn_u32(vreinterpretq_u32_s32(samples)));
    }

    let Ok(pixels) = row.pixels();

    for ((output, &left, &right), &weight) in pixels.zip(weight_tail) {
      let alpha = i32::from(weight);
      let first = i32::from(left).wrapping_mul(alpha);
      let second = i32::from(right).wrapping_mul(64_i32.wrapping_sub(alpha));
      let rounded = first
        .wrapping_add(second)
        .wrapping_add(rounding)
        .wrapping_shr(shift);
      let Ok(sample) = wrapped(rounded.clamp(0, maximum));
      *output = sample;
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  #[test]
  fn test_arm_token_available() {
    #[cfg(target_arch = "aarch64")]
    {
      use archmage::{Arm64, SimdToken};

      assert!(Arm64::summon().is_some());
    }
  }

  #[doc = r###"Verify ARM token permutations work correctly.
On aarch64, NEON is always available but the permutation system should
still be able to disable/re-enable tokens."###]
  #[test]
  #[cfg(target_arch = "aarch64")]
  fn test_arm_token_permutations() {
    use archmage::testing::{CompileTimePolicy, for_each_token_permutation};
    use archmage::{Arm64, SimdToken};

    let Ok(_guard) = crate::src::safe_simd::token_test_lock();

    let mut had_enabled = false;
    let mut had_disabled = false;

    let report = for_each_token_permutation(CompileTimePolicy::WarnStderr, |_perm| {
      match { ::core::primitive::bool::then_some(Arm64::summon().is_some(), ()) } {
        ::core::option::Option::Some(()) => {
          had_enabled = true;
        }
        ::core::option::Option::None => {
          had_disabled = true;
        }
      }
    });
    eprintln!("ARM permutations: {}", report.permutations_run);
    assert!(report.permutations_run >= 1);

    assert!(had_enabled, "token was never enabled");
  }

  #[doc = r###"R1 equivalence guard (runs on ALL targets, no intrinsics): proves the
DotProd `-128` source-bias correction reconstructs the exact NEON
accumulator `Σ filter[i]*src[i]`. The I8MM path needs no correction
(u8×i8 is exact), so this guards the only non-trivial arithmetic.
If this ever fails, the `bias_corr = 128*Σfilter` term in
`h_filter_8tap_8bpc_dotprod` is wrong."###]
  #[test]
  fn test_dotprod_bias_correction_bit_exact() {
    #[must_use]
    fn neon_sum(f: &[i8; 8], s: &[u8; 8]) -> i32 {
      (0..8)
        .map(|i| {
          ::core::primitive::i32::wrapping_mul(
            <i32 as ::core::convert::From<::core::primitive::i8>>::from(f[i]),
            <i32 as ::core::convert::From<::core::primitive::u8>>::from(s[i]),
          )
        })
        .sum()
    }

    #[must_use]
    fn dotprod_sum(f: &[i8; 8], s: &[u8; 8]) -> i32 {
      let dot: i32 = (0..8)
        .map(|i| {
          ::core::primitive::i32::wrapping_mul(
            <i32 as ::core::convert::From<::core::primitive::i8>>::from(f[i]),
            ::core::primitive::i32::wrapping_sub(
              <i32 as ::core::convert::From<::core::primitive::u8>>::from(s[i]),
              128,
            ),
          )
        })
        .sum();
      let corr = 128i32
        * f
          .iter()
          .map(|&c| <i32 as ::core::convert::From<::core::primitive::i8>>::from(c))
          .sum::<i32>();
      dot + corr
    }

    let filters: [[i8; 8]; 4] = [
      [0, 1, -3, 63, 4, -1, 0, 0],
      [-1, 2, -5, 126, 8, -3, 1, 0],
      [-2, 2, -6, 126, 8, -2, 2, 0],
      [3, -6, 15, 113, -9, 4, -2, 0],
    ];
    let mut rng: u64 = 0x1234_5678_9abc_def0;
    let mut next = || {
      rng ^= ::core::primitive::u64::wrapping_shl(rng, 13_u32);
      rng ^= ::core::primitive::u64::wrapping_shr(rng, 7_u32);
      rng ^= ::core::primitive::u64::wrapping_shl(rng, 17_u32);
      rng
    };

    for f in &filters {
      for _ in 0..200_000 {
        let mut s = [0u8; 8];

        for x in &mut s {
          *x = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
            (next() & 0xff),
          ) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          };
        }

        assert_eq!(neon_sum(f, &s), dotprod_sum(f, &s));
      }

      for v in [0u8, 255] {
        let s = [v; 8];
        assert_eq!(neon_sum(f, &s), dotprod_sum(f, &s));
      }
    }
  }

  #[doc = r###"R1 CI bit-exactness: compare the DotProd / I8MM H-filter kernels against
the baseline NEON kernel on seeded input. Only compiled when the
higher-tier cfgs are enabled (the underlying intrinsics are nightly-only;
see the DotProd/I8MM section in this module). Runs on real aarch64 in CI;
the host (x86) cross-compiles but cannot execute it."###]
  #[test]
  #[cfg(all(target_arch = "aarch64", any(rav1d_arm_dotprod, rav1d_arm_i8mm)))]
  fn test_arm_h_filter_8tap_matches_neon() {
    use super::{MID_STRIDE, h_filter_8tap_8bpc_neon};
    use archmage::{Arm64, SimdToken};

    let Ok(_lock) = crate::src::safe_simd::token_test_lock();
    let token = Arm64::summon().expect("NEON always available on aarch64");
    let filters: [[i8; 8]; 4] = [
      [0, 1, -3, 63, 4, -1, 0, 0],
      [-1, 2, -5, 126, 8, -3, 1, 0],
      [-2, 2, -6, 126, 8, -2, 2, 0],
      [3, -6, 15, 113, -9, 4, -2, 0],
    ];
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

        let mut ref_out = [0i16; MID_STRIDE];
        h_filter_8tap_8bpc_neon(token, &mut ref_out[..w], &src, w, f, sh);

        #[cfg(rav1d_arm_dotprod)]
        {
          let Ok(tier) = crate::src::cpu::summon_arm64v2();

          match tier {
            Some(available) => {
              let mut got = [0i16; MID_STRIDE];
              super::h_filter_8tap_8bpc_dotprod(available, &mut got[..w], &src, w, f, sh);
              assert_eq!(&got[..w], &ref_out[..w], "dotprod mismatch w={w} f={f:?}");
            }
            None => {}
          }
        }
        #[cfg(rav1d_arm_i8mm)]
        {
          let Ok(tier) = crate::src::cpu::summon_arm64v3();

          match tier {
            Some(available) => {
              let mut got = [0i16; MID_STRIDE];
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

#[doc = r###"Core w_mask implementation for 8bpc
SS_HOR and SS_VER control subsampling: 444=(false,false), 422=(true,false), 420=(true,true)"###]
#[cfg(target_arch = "aarch64")]
#[arcane]
fn w_mask_8bpc_inner(
  _token: Arm64,
  dst: &mut [u8],
  dst_stride: usize,
  tmp1: &[i16],
  tmp2: &[i16],
  w: usize,
  h: usize,
  mask: &mut [u8],
  sign: u8,
  ss_hor: bool,
  ss_ver: bool,
) {
  let mut dst = dst.flex_mut();
  let tmp1 = tmp1.flex();
  let tmp2 = tmp2.flex();
  let mut mask = mask.flex_mut();

  const SH: i32 = 4 + 6;
  const RND: i32 = (32 << 4) + 0 * 64;
  const MASK_SH: u32 = ::core::primitive::i32::cast_unsigned((8 + 4 - 4));
  const MASK_RND: u16 = 1 << (MASK_SH - 5);

  let mask_w = match ::core::primitive::bool::then_some(ss_hor, ()) {
    ::core::option::Option::Some(()) => w.wrapping_shr(1_u32),
    ::core::option::Option::None => w,
  };

  let mut mrow = [0u8; 128];

  for y in 0..h {
    let tmp1_row = &tmp1[y * w..][..w];
    let tmp2_row = &tmp2[y * w..][..w];
    let dst_row = &mut dst[y * dst_stride..][..w];

    let mut col = 0;

    while col + 8 <= w {
      let t1 = safe_simd::vld1q_s16(tmp1_row[col..][..8].try_into().unwrap());
      let t2 = safe_simd::vld1q_s16(tmp2_row[col..][..8].try_into().unwrap());

      let diff = vsubq_s16(t1, t2);
      let abs_diff = vabsq_s16(diff);

      let abs_32_lo = vmovl_s16(vget_low_s16(abs_diff));
      let abs_32_hi = vmovl_s16(vget_high_s16(abs_diff));

      let mask_rnd_vec =
        vdupq_n_s32(<i32 as ::core::convert::From<::core::primitive::u16>>::from(MASK_RND));
      let m_lo = vaddq_s32(abs_32_lo, mask_rnd_vec);
      let m_hi = vaddq_s32(abs_32_hi, mask_rnd_vec);

      let m_shifted_lo = vshrq_n_s32::<8>(m_lo);
      let m_shifted_hi = vshrq_n_s32::<8>(m_hi);

      let m_lo = vminq_s32(vaddq_s32(m_shifted_lo, vdupq_n_s32(38)), vdupq_n_s32(64));
      let m_hi = vminq_s32(vaddq_s32(m_shifted_hi, vdupq_n_s32(38)), vdupq_n_s32(64));

      let m_16 = vcombine_s16(vmovn_s32(m_lo), vmovn_s32(m_hi));

      let inv_m = vsubq_s16(vdupq_n_s16(64), m_16);

      let t1_lo = vmovl_s16(vget_low_s16(t1));
      let t1_hi = vmovl_s16(vget_high_s16(t1));
      let t2_lo = vmovl_s16(vget_low_s16(t2));
      let t2_hi = vmovl_s16(vget_high_s16(t2));
      let m_lo_32 = vmovl_s16(vget_low_s16(m_16));
      let m_hi_32 = vmovl_s16(vget_high_s16(m_16));
      let inv_m_lo_32 = vmovl_s16(vget_low_s16(inv_m));
      let inv_m_hi_32 = vmovl_s16(vget_high_s16(inv_m));

      let rnd_vec = vdupq_n_s32(RND);
      let blend_lo = vaddq_s32(
        vaddq_s32(vmulq_s32(t1_lo, m_lo_32), vmulq_s32(t2_lo, inv_m_lo_32)),
        rnd_vec,
      );
      let blend_hi = vaddq_s32(
        vaddq_s32(vmulq_s32(t1_hi, m_hi_32), vmulq_s32(t2_hi, inv_m_hi_32)),
        rnd_vec,
      );

      let result_lo = vshrq_n_s32::<10>(blend_lo);
      let result_hi = vshrq_n_s32::<10>(blend_hi);

      let zero = vdupq_n_s32(0);
      let max_val = vdupq_n_s32(255);
      let result_lo = vmaxq_s32(vminq_s32(result_lo, max_val), zero);
      let result_hi = vmaxq_s32(vminq_s32(result_hi, max_val), zero);

      let narrow_16 = vcombine_s16(vmovn_s32(result_lo), vmovn_s32(result_hi));
      let result_u8 = vqmovun_s16(narrow_16);

      let dst_arr: &mut [u8; 8] = (&mut dst_row[col..col + 8]).try_into().unwrap();
      safe_simd::vst1_u8(dst_arr, result_u8);

      let m_narrow = vqmovun_s16(m_16);
      let m_arr: &mut [u8; 8] = (&mut mrow[col..col + 8]).try_into().unwrap();
      safe_simd::vst1_u8(m_arr, m_narrow);

      col += 8;
    }

    for _ in (::core::ops::Range { start: col, end: w }) {
      let t1 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(tmp1_row[col]);
      let t2 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(tmp2_row[col]);
      let m = cmp_min_i32(
        38 + ::core::primitive::u32::cast_signed(
          (t1
            .abs_diff(t2)
            .saturating_add(
              <u32 as ::core::convert::From<::core::primitive::u16>>::from(MASK_RND),
            )
            >> MASK_SH),
        ),
        64,
      );

      let blend = (t1 * m + t2 * (64 - m) + RND) >> SH;
      dst_row[col] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        blend.clamp(0, 255),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
      mrow[col] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(m) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };

      col = (::core::num::Wrapping(col) + ::core::num::Wrapping(1)).0;
    }

    let mask_y = match ::core::primitive::bool::then_some(ss_ver, ()) {
      ::core::option::Option::Some(()) => y.wrapping_shr(1_u32),
      ::core::option::Option::None => y,
    };
    let mask_row = &mut mask[mask_y * mask_w..][..mask_w];

    if !ss_hor {
      mask_row[..w].copy_from_slice(&mrow[..w]);
    } else {
      let mut x = 0;

      while x + 1 < w {
        let m = mrow[x];
        let n = mrow[x + 1];
        let sum = m + n;
        mask_row[::core::primitive::usize::wrapping_shr(x, 1_u32)] = if ss_ver && (y & 1) != 0 {
          match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
            ::core::primitive::u16::wrapping_shr(
              ::core::primitive::u16::wrapping_add(
                <u16 as ::core::convert::From<::core::primitive::u8>>::from((sum + 2 - sign)),
                <u16 as ::core::convert::From<::core::primitive::u8>>::from(
                  mask_row[::core::primitive::usize::wrapping_shr(x, 1_u32)],
                ),
              ),
              2_u32,
            ),
          ) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          }
        } else if ss_ver {
          sum
        } else {
          ::core::primitive::u8::wrapping_shr(sum + 1 - sign, 1_u32)
        };
        x += 2;
      }
    }
  }
}

#[doc = r###"`i32::min` as a free function, so the kernel body reads like the reference's
`cmp::min` without pulling `std::cmp` into this module's namespace."###]
#[cfg(target_arch = "aarch64")]
#[inline(always)]
#[must_use]
fn cmp_min_i32(a: i32, b: i32) -> i32 {
  match { ::core::primitive::bool::then_some(a < b, ()) } {
    ::core::option::Option::Some(()) => a,
    ::core::option::Option::None => b,
  }
}

use crate::src::error::{Rav1dError, Rav1dResult};
use console_core_geometry::Point;
use console_core_never::Never;
use console_core_pixel_buffers::RowLayout;

struct BilinearBlock<'a, Pixel, Output> {
  output: &'a mut [Output],
  source: &'a [Pixel],
  output_pitch: i64,
  source_pitch: i64,
  extent: Size<u64>,
  phase: Point<i32>,
}

#[derive(Clone, Copy)]
struct BilinearWeights {
  first: i32,
  second: i32,
}

impl BilinearWeights {
  fn new(fraction: i32) -> Result<Self, Never> {
    Ok(Self {
      first: 16_i32.wrapping_sub(fraction),
      second: fraction,
    })
  }

  #[inline]
  fn sum(self, [first, second]: [i32; 2]) -> Result<i32, Never> {
    Ok(
      first
        .wrapping_mul(self.first)
        .wrapping_add(second.wrapping_mul(self.second)),
    )
  }
}

enum BilinearLine<'a, Pixel> {
  Identity(&'a [Pixel]),
  Single {
    samples: [&'a [Pixel]; 2],
    weights: BilinearWeights,
  },
  Filtered {
    samples: [&'a [i32]; 2],
    weights: BilinearWeights,
  },
}

enum HorizontalPrecision {
  Exact,
  Narrowed { shift: u32 },
}

fn round_bilinear(sample: i32, shift: u32) -> Result<i32, Never> {
  let bias = 1_i32.wrapping_shl(shift).wrapping_shr(1);
  Ok(sample.wrapping_add(bias).wrapping_shr(shift))
}

fn horizontal_bilinear<Pixel: Copy + Into<i32>>(
  output: &mut [i32],
  source: &[Pixel],
  weights: BilinearWeights,
  precision: &HorizontalPrecision,
) -> Rav1dResult {
  let (_, next) = source.split_first().ok_or(Rav1dError::EINVAL)?;

  for ((output_sample, &first), &second) in output.iter_mut().zip(source).zip(next) {
    let Ok(sum) = weights.sum([Into::<i32>::into(first), Into::<i32>::into(second)]);
    *output_sample = match precision {
      HorizontalPrecision::Exact => sum,
      HorizontalPrecision::Narrowed { shift } => {
        let Ok(rounded) = round_bilinear(sum, *shift);
        let Ok(narrow) = wrapped::<_, i16>(rounded);
        i32::from(narrow)
      }
    };
  }

  Ok(())
}

fn write_bilinear_rows<Pixel: Copy + Into<i32>, Output>(
  block: BilinearBlock<'_, Pixel, Output>,
  horizontal_precision: HorizontalPrecision,
  write: impl Fn(&mut [Output], BilinearLine<'_, Pixel>) -> Rav1dResult,
) -> Rav1dResult {
  let BilinearBlock {
    output,
    source,
    output_pitch,
    source_pitch,
    extent,
    phase: Point { x, y },
  } = block;

  match (x, y) {
    (0..=15, 0..=15) => {}
    _ => return Err(Rav1dError::EINVAL),
  }

  let source_size = Size {
    width: extent.width.wrapping_add(u64::from(x != 0)),
    height: extent.height.wrapping_add(u64::from(y != 0)),
  };
  let source_layout =
    RowLayout::new(source_size, 0, source_pitch).map_err(|_| Rav1dError::EINVAL)?;
  let mut source_rows = source_layout.rows(source).map_err(|_| Rav1dError::EINVAL)?;
  let output_layout = RowLayout::new(extent, 0, output_pitch).map_err(|_| Rav1dError::EINVAL)?;
  let target_rows = output_layout
    .rows_mut(output)
    .map_err(|_| Rav1dError::EINVAL)?;
  let Ok(horizontal) = BilinearWeights::new(x);
  let Ok(vertical) = BilinearWeights::new(y);

  match (x, y) {
    (0, 0) => {
      for (target, input) in target_rows.zip(source_rows) {
        write(target, BilinearLine::Identity(input))?;
      }
    }
    (_, 0) => {
      for (target, input) in target_rows.zip(source_rows) {
        let (_, next) = input.split_first().ok_or(Rav1dError::EINVAL)?;
        write(
          target,
          BilinearLine::Single {
            samples: [input, next],
            weights: horizontal,
          },
        )?;
      }
    }
    (0, _) => {
      let mut top = source_rows.next().ok_or(Rav1dError::EINVAL)?;

      for (target, bottom) in target_rows.zip(source_rows) {
        write(
          target,
          BilinearLine::Single {
            samples: [top, bottom],
            weights: vertical,
          },
        )?;
        top = bottom;
      }
    }
    (_, _) => {
      let mut top_storage = [0_i32; 128];
      let mut bottom_storage = [0_i32; 128];
      let Ok(width) = console_core_number_conversion::index(extent.width);
      let mut top = top_storage.get_mut(..width).ok_or(Rav1dError::EINVAL)?;
      let mut bottom = bottom_storage.get_mut(..width).ok_or(Rav1dError::EINVAL)?;
      let first = source_rows.next().ok_or(Rav1dError::EINVAL)?;
      horizontal_bilinear(top, first, horizontal, &horizontal_precision)?;

      for (target, input) in target_rows.zip(source_rows) {
        horizontal_bilinear(bottom, input, horizontal, &horizontal_precision)?;
        write(
          target,
          BilinearLine::Filtered {
            samples: [top, bottom],
            weights: vertical,
          },
        )?;
        core::mem::swap(&mut top, &mut bottom);
      }
    }
  }

  Ok(())
}

struct BilinearRounding {
  identity: u32,
  single: u32,
  filtered: u32,
}

fn write_bilinear_word<Output>(
  output: &mut [Output],
  line: BilinearLine<'_, u16>,
  rounding: &BilinearRounding,
  store: impl Fn(&mut Output, i32) -> Result<(), Never>,
) -> Rav1dResult {
  match line {
    BilinearLine::Identity(source) => {
      for (target, &sample) in output.iter_mut().zip(source) {
        let Ok(()) = store(target, i32::from(sample).wrapping_shl(rounding.identity));
      }
    }
    BilinearLine::Single {
      samples: [first, second],
      weights,
    } => {
      for ((target, &low), &high) in output.iter_mut().zip(first).zip(second) {
        let Ok(sum) = weights.sum([i32::from(low), i32::from(high)]);
        let Ok(value) = round_bilinear(sum, rounding.single);
        let Ok(()) = store(target, value);
      }
    }
    BilinearLine::Filtered {
      samples: [top, bottom],
      weights,
    } => {
      for ((target, &low), &high) in output.iter_mut().zip(top).zip(bottom) {
        let Ok(sum) = weights.sum([low, high]);
        let Ok(value) = round_bilinear(sum, rounding.filtered);
        let Ok(()) = store(target, value);
      }
    }
  }

  Ok(())
}

#[arcane]
fn byte_bilinear_pair(
  _token: Arm64,
  input: [&[u8; 8]; 2],
  weights: BilinearWeights,
  shift: u32,
) -> Result<int16x8_t, Never> {
  let [first, second] = input;
  let low = vreinterpretq_s16_u16(vmovl_u8(safe_simd::vld1_u8(first)));
  let high = vreinterpretq_s16_u16(vmovl_u8(safe_simd::vld1_u8(second)));
  let Ok(left) = wrapped::<_, i16>(weights.first);
  let Ok(right) = wrapped::<_, i16>(weights.second);
  let sum = vaddq_s16(
    vmulq_s16(low, vdupq_n_s16(left)),
    vmulq_s16(high, vdupq_n_s16(right)),
  );
  let Ok(bias) = wrapped::<_, i16>(1_i32.wrapping_shl(shift).wrapping_shr(1));
  let Ok(shift_right) = wrapped::<_, i16>(shift.cast_signed().wrapping_neg());
  let shifted = vshlq_s16(vaddq_s16(sum, vdupq_n_s16(bias)), vdupq_n_s16(shift_right));
  Ok(shifted)
}

#[arcane]
fn byte_bilinear_filtered(
  _token: Arm64,
  input: [&[i32; 8]; 2],
  weights: BilinearWeights,
  shift: u32,
) -> Result<int16x8_t, Never> {
  let [first, second] = input;
  let [first_low, first_high]: &[[i32; 4]; 2] = zerocopy::transmute_ref!(first);
  let [second_low, second_high]: &[[i32; 4]; 2] = zerocopy::transmute_ref!(second);
  let c0 = vdupq_n_s32(weights.first);
  let c1 = vdupq_n_s32(weights.second);
  let bias = vdupq_n_s32(1_i32.wrapping_shl(shift).wrapping_shr(1));
  let right = vdupq_n_s32(shift.cast_signed().wrapping_neg());
  let first_a = vmulq_s32(safe_simd::vld1q_s32(first_low), c0);
  let first_b = vmulq_s32(safe_simd::vld1q_s32(first_high), c0);
  let second_a = vmulq_s32(safe_simd::vld1q_s32(second_low), c1);
  let second_b = vmulq_s32(safe_simd::vld1q_s32(second_high), c1);
  let lower = vshlq_s32(vaddq_s32(vaddq_s32(first_a, second_a), bias), right);
  let upper = vshlq_s32(vaddq_s32(vaddq_s32(first_b, second_b), bias), right);
  Ok(vcombine_s16(vmovn_s32(lower), vmovn_s32(upper)))
}

#[arcane]
fn write_bilinear_byte<Output>(
  token: Arm64,
  destination: &mut [Output],
  line: BilinearLine<'_, u8>,
  rounding: &BilinearRounding,
  scalar: impl Fn(&mut Output, i32) -> Result<(), Never>,
  vector: impl Fn(Arm64, &mut [Output; 8], int16x8_t) -> Result<(), Never>,
) -> Rav1dResult {
  match line {
    BilinearLine::Identity(source) => {
      for (target, &sample) in destination.iter_mut().zip(source) {
        let Ok(()) = scalar(target, i32::from(sample).wrapping_shl(rounding.identity));
      }
    }
    BilinearLine::Single {
      samples: [first, second],
      weights,
    } => {
      let first_row = first.get(..destination.len()).ok_or(Rav1dError::EINVAL)?;
      let second_row = second.get(..destination.len()).ok_or(Rav1dError::EINVAL)?;
      let (blocks, tail) = destination.as_chunks_mut::<8>();
      let (first_vectors, first_tail) = first_row.as_chunks::<8>();
      let (second_vectors, second_tail) = second_row.as_chunks::<8>();

      for ((target, low), high) in blocks.iter_mut().zip(first_vectors).zip(second_vectors) {
        let Ok(sum) = byte_bilinear_pair(token, [low, high], weights, rounding.single);
        let Ok(()) = vector(token, target, sum);
      }

      for ((target, &low), &high) in tail.iter_mut().zip(first_tail).zip(second_tail) {
        let Ok(sum) = weights.sum([i32::from(low), i32::from(high)]);
        let Ok(value) = round_bilinear(sum, rounding.single);
        let Ok(()) = scalar(target, value);
      }
    }
    BilinearLine::Filtered {
      samples: [top, bottom],
      weights,
    } => {
      let (blocks, tail) = destination.as_chunks_mut::<8>();
      let (top_vectors, top_tail) = top.as_chunks::<8>();
      let (bottom_vectors, bottom_tail) = bottom.as_chunks::<8>();

      for ((target, low), high) in blocks.iter_mut().zip(top_vectors).zip(bottom_vectors) {
        let Ok(sum) = byte_bilinear_filtered(token, [low, high], weights, rounding.filtered);
        let Ok(()) = vector(token, target, sum);
      }

      for ((target, &low), &high) in tail.iter_mut().zip(top_tail).zip(bottom_tail) {
        let Ok(sum) = weights.sum([low, high]);
        let Ok(value) = round_bilinear(sum, rounding.filtered);
        let Ok(()) = scalar(target, value);
      }
    }
  }

  Ok(())
}

#[arcane]
fn store_bilinear_pixels(
  _token: Arm64,
  output: &mut [u8; 8],
  input: int16x8_t,
) -> Result<(), Never> {
  safe_simd::vst1_u8(output, vqmovun_s16(input));
  Ok(())
}

#[arcane]
fn store_bilinear_predictors(
  _token: Arm64,
  output: &mut [i16; 8],
  input: int16x8_t,
) -> Result<(), Never> {
  safe_simd::vst1q_s16(output, input);
  Ok(())
}

#[arcane]
fn put_bilin_8bpc_inner(token: Arm64, block: BilinearBlock<'_, u8, u8>) -> Rav1dResult {
  let scale = BilinearRounding {
    identity: 0,
    single: 4,
    filtered: 8,
  };
  write_bilinear_rows(block, HorizontalPrecision::Exact, |output, line| {
    write_bilinear_byte(
      token,
      output,
      line,
      &scale,
      |target, value| {
        let Ok(sample) = wrapped(value.clamp(0, 255));
        *target = sample;
        Ok(())
      },
      |available, target, lanes| store_bilinear_pixels(available, target, lanes),
    )
  })
}

#[arcane]
fn prep_bilin_8bpc_inner(token: Arm64, block: BilinearBlock<'_, u8, i16>) -> Rav1dResult {
  let scale = BilinearRounding {
    identity: 4,
    single: 0,
    filtered: 4,
  };
  write_bilinear_rows(block, HorizontalPrecision::Exact, |output, line| {
    write_bilinear_byte(
      token,
      output,
      line,
      &scale,
      |target, value| {
        let Ok(sample) = wrapped(value);
        *target = sample;
        Ok(())
      },
      |available, target, lanes| store_bilinear_predictors(available, target, lanes),
    )
  })
}

fn put_bilin_16bpc_inner(block: BilinearBlock<'_, u16, u16>, maximum: i32) -> Rav1dResult {
  let scale = BilinearRounding {
    identity: 0,
    single: 4,
    filtered: 8,
  };
  write_bilinear_rows(
    block,
    HorizontalPrecision::Exact,
    |output, line| match line {
      BilinearLine::Identity(input) => {
        for (target, &sample) in output.iter_mut().zip(input) {
          *target = sample;
        }

        Ok(())
      }
      BilinearLine::Single { .. } | BilinearLine::Filtered { .. } => {
        write_bilinear_word(output, line, &scale, |target, value| {
          let Ok(sample) = wrapped(value.clamp(0, maximum));
          *target = sample;
          Ok(())
        })
      }
    },
  )
}

fn prep_bilin_16bpc_inner(block: BilinearBlock<'_, u16, i16>, maximum: i32) -> Rav1dResult {
  let Ok(intermediate_bits) = intermediate_bits_16bpc(maximum);
  let identity = intermediate_bits.cast_unsigned();
  let shift = 4_u32.wrapping_sub(identity);
  let scale = BilinearRounding {
    identity,
    single: shift,
    filtered: 4,
  };
  write_bilinear_rows(
    block,
    HorizontalPrecision::Narrowed { shift },
    |output, line| {
      write_bilinear_word(output, line, &scale, |target, value| {
        let Ok(sample) = wrapped(value.wrapping_sub(PREP_BIAS_16BPC));
        *target = sample;
        Ok(())
      })
    },
  )
}

const PREP_BIAS_16BPC: i32 = 8192;

const MID_STRIDE: usize = 128 + 16;

#[doc = r###"Horizontal 8-tap filter to intermediate buffer (i16)
For ARM NEON: uses multiply-accumulate with 8 coefficients"###]
#[cfg(target_arch = "aarch64")]
#[arcane]
fn h_filter_8tap_8bpc_neon(
  _token: Arm64,
  dst: &mut [i16],
  src: &[u8],
  w: usize,
  filter: &[i8; 8],
  sh: u8,
) {
  let mut dst = dst.flex_mut();
  let src = src.flex();
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
      ((sum + <i32 as ::core::convert::From<::core::primitive::i16>>::from(rnd)) >> sh),
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
    col = (::core::num::Wrapping(col) + ::core::num::Wrapping(1)).0;
  }
}

#[doc = r###"Vertical 8-tap filter from intermediate buffer to output (u8)"###]
#[cfg(target_arch = "aarch64")]
#[arcane]
fn v_filter_8tap_8bpc_neon(
  _token: Arm64,
  dst: &mut [u8],
  mid: &[[i16; MID_STRIDE]],
  w: usize,
  filter: &[i8; 8],
  sh: u8,
  max: u16,
) {
  let mut dst = dst.flex_mut();
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
#[cfg(target_arch = "aarch64")]
#[arcane]
fn h_filter_8tap_8bpc_put_neon(
  _token: Arm64,
  dst: &mut [u8],
  src: &[u8],
  w: usize,
  filter: &[i8; 8],
) {
  let mut dst = dst.flex_mut();
  let src = src.flex();
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
  let mut dst = dst.flex_mut();
  let src = src.flex();
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
  let mut dst = dst.flex_mut();
  let src = src.flex();
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
#[archmage::rite]
fn vshrq_n_s32_dyn(v: int32x4_t, sh: u8) -> int32x4_t {
  match sh {
    2 => vshrq_n_s32::<2>(v),
    4 => vshrq_n_s32::<4>(v),
    6 => vshrq_n_s32::<6>(v),
    _ => vshlq_s32(v, vdupq_n_s32(-(sh as i32))),
  }
}

#[doc = r###"Vertical 8-tap filter directly from source (V-only case)"###]
#[cfg(target_arch = "aarch64")]
#[arcane]
fn v_filter_8tap_8bpc_direct_neon(
  _token: Arm64,
  dst: &mut [u8],
  src: &[u8],
  src_stride: usize,
  w: usize,
  filter: &[i8; 8],
) {
  let mut dst = dst.flex_mut();
  let src = src.flex();
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
#[cfg(target_arch = "aarch64")]
#[doc = r###"Tier-selecting wrapper for the horizontal 8-tap → i16 filter.

Prefers the I8MM (`Arm64V3`) path, then DotProd (`Arm64V2`), then baseline
NEON. Each higher tier is gated behind its rustc cfg (`rav1d_arm_i8mm` /
`rav1d_arm_dotprod`) which is OFF until the underlying `core::arch`
intrinsics stabilize on the project's stable toolchain (see the DotProd /
I8MM section above). On the default stable build this compiles to exactly
the NEON call — zero behavioural change. `#[rite]` so it inlines into the
`#[arcane]` caller's target-feature region."###]
#[cfg(target_arch = "aarch64")]
#[archmage::rite]
fn dispatch_h_filter_8tap_8bpc(
  token: Arm64,
  dst: &mut [i16],
  src: &[u8],
  w: usize,
  filter: &[i8; 8],
  sh: u8,
) {
  #[cfg(rav1d_arm_i8mm)]
  {
    let Ok(tier) = crate::src::cpu::summon_arm64v3();

    match tier {
      Some(available) => {
        h_filter_8tap_8bpc_i8mm(available, dst, src, w, filter, sh);
        return;
      }
      None => {}
    }
  }
  #[cfg(rav1d_arm_dotprod)]
  {
    let Ok(tier) = crate::src::cpu::summon_arm64v2();

    match tier {
      Some(available) => {
        h_filter_8tap_8bpc_dotprod(available, dst, src, w, filter, sh);
        return;
      }
      None => {}
    }
  }
  h_filter_8tap_8bpc_neon(token, dst, src, w, filter, sh);
}

#[arcane]
fn put_8tap_8bpc_inner(
  token: Arm64,
  dst: &mut [u8],
  dst_stride: usize,
  src: &[u8],
  src_base: usize,
  src_stride: usize,
  w: usize,
  h: usize,
  mx: usize,
  my: usize,
  h_filter_type: Rav1dFilterMode,
  v_filter_type: Rav1dFilterMode,
) {
  let mut dst = dst.flex_mut();
  let src = src.flex();
  let intermediate_bits = 4u8;

  let Ok(fh) = get_filter_coeff(mx, w, h_filter_type);
  let Ok(fv) = get_filter_coeff(my, h, v_filter_type);

  match (fh, fv) {
    (Some(fh), Some(fv)) => {
      let tmp_h = h + 7;
      let mut mid = [[0i16; MID_STRIDE]; 135];

      for y in 0..tmp_h {
        let src_off =
          src_base.wrapping_add_signed((y.cast_signed() - 3) * src_stride.cast_signed() - 3);
        let src_row = &src[src_off..];
        dispatch_h_filter_8tap_8bpc(
          token,
          &mut mid[y][..w],
          src_row,
          w,
          fh,
          ::core::primitive::u8::wrapping_sub(6, intermediate_bits),
        );
      }

      for y in 0..h {
        let dst_row = &mut dst[y * dst_stride..][..w];
        v_filter_8tap_8bpc_neon(
          token,
          dst_row,
          &mid[y..],
          w,
          fv,
          ::core::primitive::u8::wrapping_add(6, intermediate_bits),
          255,
        );
      }
    }
    (Some(fh), None) => {
      for y in 0..h {
        let src_off =
          src_base.wrapping_add_signed(y.cast_signed() * src_stride.cast_signed() - 3);
        let src_row = &src[src_off..];
        let dst_row = &mut dst[y * dst_stride..][..w];
        h_filter_8tap_8bpc_put_neon(token, dst_row, src_row, w, fh);
      }
    }
    (None, Some(fv)) => {
      for y in 0..h {
        let src_off =
          src_base.wrapping_add_signed((y.cast_signed() - 3) * src_stride.cast_signed());
        let src_row = &src[src_off..];
        let dst_row = &mut dst[y * dst_stride..][..w];
        v_filter_8tap_8bpc_direct_neon(token, dst_row, src_row, src_stride, w, fv);
      }
    }
    (None, None) => {
      for y in 0..h {
        let src_row = &src[src_base + y * src_stride..][..w];
        let dst_row = &mut dst[y * dst_stride..][..w];
        dst_row.copy_from_slice(src_row);
      }
    }
  }
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
#[must_use]
fn get_h_filter_type(filter: Filter2d) -> Rav1dFilterMode {
  match filter {
    Filter2d::Regular8Tap | Filter2d::RegularSmooth8Tap | Filter2d::RegularSharp8Tap => {
      Rav1dFilterMode::Regular8Tap
    }
    Filter2d::Smooth8Tap | Filter2d::SmoothRegular8Tap | Filter2d::SmoothSharp8Tap => {
      Rav1dFilterMode::Smooth8Tap
    }
    Filter2d::Sharp8Tap | Filter2d::SharpRegular8Tap | Filter2d::SharpSmooth8Tap => {
      Rav1dFilterMode::Sharp8Tap
    }
    Filter2d::Bilinear => Rav1dFilterMode::Regular8Tap,
  }
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
#[must_use]
fn get_v_filter_type(filter: Filter2d) -> Rav1dFilterMode {
  match filter {
    Filter2d::Regular8Tap | Filter2d::SmoothRegular8Tap | Filter2d::SharpRegular8Tap => {
      Rav1dFilterMode::Regular8Tap
    }
    Filter2d::Smooth8Tap | Filter2d::RegularSmooth8Tap | Filter2d::SharpSmooth8Tap => {
      Rav1dFilterMode::Smooth8Tap
    }
    Filter2d::Sharp8Tap | Filter2d::RegularSharp8Tap | Filter2d::SmoothSharp8Tap => {
      Rav1dFilterMode::Sharp8Tap
    }
    Filter2d::Bilinear => Rav1dFilterMode::Regular8Tap,
  }
}

#[doc = r###"Vertical 8-tap filter from intermediate buffer to i16 output"###]
#[cfg(target_arch = "aarch64")]
#[arcane]
fn v_filter_8tap_to_i16_neon(
  _token: Arm64,
  dst: &mut [i16],
  mid: &[[i16; MID_STRIDE]],
  w: usize,
  filter: &[i8; 8],
  sh: u8,
) {
  let mut dst = dst.flex_mut();
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
      ((sum + rnd) >> sh),
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
    col = (::core::num::Wrapping(col) + ::core::num::Wrapping(1)).0;
  }
}

#[doc = r###"Main 8-tap prep function for 8bpc"###]
#[cfg(target_arch = "aarch64")]
#[arcane]
fn prep_8tap_8bpc_inner(
  token: Arm64,
  tmp: &mut [i16],
  src: &[u8],
  src_base: usize,
  src_stride: usize,
  w: usize,
  h: usize,
  mx: usize,
  my: usize,
  h_filter_type: Rav1dFilterMode,
  v_filter_type: Rav1dFilterMode,
) {
  let mut tmp = tmp.flex_mut();
  let src = src.flex();
  let intermediate_bits = 4u8;

  let Ok(fh) = get_filter_coeff(mx, w, h_filter_type);
  let Ok(fv) = get_filter_coeff(my, h, v_filter_type);

  match (fh, fv) {
    (Some(fh), Some(fv)) => {
      let tmp_h = h + 7;
      let mut mid = [[0i16; MID_STRIDE]; 135];

      for y in 0..tmp_h {
        let src_off =
          src_base.wrapping_add_signed((y.cast_signed() - 3) * src_stride.cast_signed() - 3);
        let src_row = &src[src_off..];
        dispatch_h_filter_8tap_8bpc(
          token,
          &mut mid[y][..w],
          src_row,
          w,
          fh,
          ::core::primitive::u8::wrapping_sub(6, intermediate_bits),
        );
      }

      for y in 0..h {
        let out_row = &mut tmp[y * w..][..w];

        v_filter_8tap_to_i16_neon(token, out_row, &mid[y..], w, fv, 6);
      }
    }
    (Some(fh), None) => {
      for y in 0..h {
        let src_off =
          src_base.wrapping_add_signed(y.cast_signed() * src_stride.cast_signed() - 3);
        let src_row = &src[src_off..];
        let out_row = &mut tmp[y * w..][..w];
        dispatch_h_filter_8tap_8bpc(
          token,
          out_row,
          src_row,
          w,
          fh,
          ::core::primitive::u8::wrapping_sub(6, intermediate_bits),
        );
      }
    }
    (None, Some(fv)) => {
      for y in 0..h {
        let out_row = &mut tmp[y * w..][..w];

        let mut mid = [[0i16; MID_STRIDE]; 8];

        for i in 0..8 {
          let src_off = src_base.wrapping_add_signed(
            (y.cast_signed() + ::core::primitive::usize::cast_signed(i) - 3)
              * src_stride.cast_signed(),
          );

          for x in 0..w {
            mid[i][x] = ::core::primitive::i16::wrapping_shl(
              <i16 as ::core::convert::From<::core::primitive::u8>>::from(src[src_off + x]),
              ::core::primitive::u32::from(intermediate_bits),
            );
          }
        }

        v_filter_8tap_to_i16_neon(token, out_row, &mid, w, fv, 6);
      }
    }
    (None, None) => {
      for y in 0..h {
        let src_row = &src[src_base + y * src_stride..][..w];
        let out_row = &mut tmp[y * w..][..w];

        for x in 0..w {
          out_row[x] = ::core::primitive::i16::wrapping_shl(
            <i16 as ::core::convert::From<::core::primitive::u8>>::from(src_row[x]),
            ::core::primitive::u32::from(intermediate_bits),
          );
        }
      }
    }
  }
}

#[doc = r###"Horizontal 8-tap filter to intermediate buffer (i32) for 16bpc"###]
#[cfg(target_arch = "aarch64")]
#[arcane]
fn h_filter_8tap_16bpc_neon(
  _token: Arm64,
  dst: &mut [i32],
  src: &[u16],
  w: usize,
  filter: &[i8; 8],
  sh: u8,
) {
  let mut dst = dst.flex_mut();
  let src = src.flex();
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

    let s0 = safe_simd::vld1_u16(src[col..][..4].try_into().unwrap());
    let s1 = safe_simd::vld1_u16(src[col + 1..][..4].try_into().unwrap());
    let s2 = safe_simd::vld1_u16(src[col + 2..][..4].try_into().unwrap());
    let s3 = safe_simd::vld1_u16(src[col + 3..][..4].try_into().unwrap());
    let s4 = safe_simd::vld1_u16(src[col + 4..][..4].try_into().unwrap());
    let s5 = safe_simd::vld1_u16(src[col + 5..][..4].try_into().unwrap());
    let s6 = safe_simd::vld1_u16(src[col + 6..][..4].try_into().unwrap());
    let s7 = safe_simd::vld1_u16(src[col + 7..][..4].try_into().unwrap());

    let s0_32 = vreinterpretq_s32_u32(vmovl_u16(s0));
    let s1_32 = vreinterpretq_s32_u32(vmovl_u16(s1));
    let s2_32 = vreinterpretq_s32_u32(vmovl_u16(s2));
    let s3_32 = vreinterpretq_s32_u32(vmovl_u16(s3));
    let s4_32 = vreinterpretq_s32_u32(vmovl_u16(s4));
    let s5_32 = vreinterpretq_s32_u32(vmovl_u16(s5));
    let s6_32 = vreinterpretq_s32_u32(vmovl_u16(s6));
    let s7_32 = vreinterpretq_s32_u32(vmovl_u16(s7));

    let mut sum = vmulq_n_s32(s0_32, c0);
    sum = vmlaq_n_s32(sum, s1_32, c1);
    sum = vmlaq_n_s32(sum, s2_32, c2);
    sum = vmlaq_n_s32(sum, s3_32, c3);
    sum = vmlaq_n_s32(sum, s4_32, c4);
    sum = vmlaq_n_s32(sum, s5_32, c5);
    sum = vmlaq_n_s32(sum, s6_32, c6);
    sum = vmlaq_n_s32(sum, s7_32, c7);

    let rnd_vec = vdupq_n_s32(rnd);

    let result = vshlq_s32(
      vaddq_s32(sum, rnd_vec),
      vdupq_n_s32(::core::primitive::i32::wrapping_neg(
        <i32 as ::core::convert::From<::core::primitive::u8>>::from(sh),
      )),
    );

    let dst_arr: &mut [i32; 4] = (&mut dst[col..col + 4]).try_into().unwrap();
    safe_simd::vst1q_s32(dst_arr, result);
    col += 4;
  }

  for _ in (::core::ops::Range { start: col, end: w }) {
    let sum = convolve8(filter, |tap| i32::from(src[col.wrapping_add(tap)]));
    dst[col] = (sum + rnd) >> sh;
    col = (::core::num::Wrapping(col) + ::core::num::Wrapping(1)).0;
  }
}

#[doc = r###"Vertical 8-tap filter from i32 intermediate to u16 output for 16bpc"###]
#[cfg(target_arch = "aarch64")]
#[arcane]
fn v_filter_8tap_16bpc_neon(
  _token: Arm64,
  dst: &mut [u16],
  mid: &[[i32; MID_STRIDE]],
  w: usize,
  filter: &[i8; 8],
  sh: u8,
  max: u16,
) {
  let mut dst = dst.flex_mut();
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

    let r0 = safe_simd::vld1q_s32(mid[0][col..][..4].try_into().unwrap());
    let r1 = safe_simd::vld1q_s32(mid[1][col..][..4].try_into().unwrap());
    let r2 = safe_simd::vld1q_s32(mid[2][col..][..4].try_into().unwrap());
    let r3 = safe_simd::vld1q_s32(mid[3][col..][..4].try_into().unwrap());
    let r4 = safe_simd::vld1q_s32(mid[4][col..][..4].try_into().unwrap());
    let r5 = safe_simd::vld1q_s32(mid[5][col..][..4].try_into().unwrap());
    let r6 = safe_simd::vld1q_s32(mid[6][col..][..4].try_into().unwrap());
    let r7 = safe_simd::vld1q_s32(mid[7][col..][..4].try_into().unwrap());

    let mut sum = vmulq_n_s32(r0, c0);
    sum = vmlaq_n_s32(sum, r1, c1);
    sum = vmlaq_n_s32(sum, r2, c2);
    sum = vmlaq_n_s32(sum, r3, c3);
    sum = vmlaq_n_s32(sum, r4, c4);
    sum = vmlaq_n_s32(sum, r5, c5);
    sum = vmlaq_n_s32(sum, r6, c6);
    sum = vmlaq_n_s32(sum, r7, c7);

    let rnd_vec = vdupq_n_s32(rnd);
    sum = vaddq_s32(sum, rnd_vec);

    let result = vshlq_s32(
      sum,
      vdupq_n_s32(::core::primitive::i32::wrapping_neg(
        <i32 as ::core::convert::From<::core::primitive::u8>>::from(sh),
      )),
    );

    let max_vec = vdupq_n_s32(<i32 as ::core::convert::From<::core::primitive::u16>>::from(max));
    let zero = vdupq_n_s32(0);
    let clamped = vminq_s32(vmaxq_s32(result, zero), max_vec);

    let narrow = vqmovun_s32(clamped);
    let dst_arr: &mut [u16; 4] = (&mut dst[col..col + 4]).try_into().unwrap();
    safe_simd::vst1_u16(dst_arr, narrow);
    col += 4;
  }

  for _ in (::core::ops::Range { start: col, end: w }) {
    let mut sum = 0i64;

    for (i, filter_element) in <[_]>::iter(filter).enumerate() {
      sum += ::core::primitive::i64::wrapping_mul(
        <i64 as ::core::convert::From<::core::primitive::i8>>::from(*filter_element),
        <i64 as ::core::convert::From<::core::primitive::i32>>::from(mid[i][col]),
      );
    }

    dst[col] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(
        ((sum + <i64 as ::core::convert::From<::core::primitive::i32>>::from(rnd)) >> sh),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      }
      .clamp(
        0,
        <i32 as ::core::convert::From<::core::primitive::u16>>::from(max),
      ),
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
    col = (::core::num::Wrapping(col) + ::core::num::Wrapping(1)).0;
  }
}

#[doc = r###"Main 8-tap put function for 16bpc"###]
#[cfg(target_arch = "aarch64")]
#[arcane]
fn put_8tap_16bpc_inner(
  token: Arm64,
  dst: &mut [u16],
  dst_stride: usize,
  src: &[u16],
  src_base: usize,
  src_stride: usize,
  w: usize,
  h: usize,
  mx: usize,
  my: usize,
  h_filter_type: Rav1dFilterMode,
  v_filter_type: Rav1dFilterMode,
  bitdepth_max: u16,
) {
  let mut dst = dst.flex_mut();
  let src = src.flex();

  let intermediate_bits =
    (match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
      bitdepth_max.leading_zeros(),
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    }) - 2;

  let Ok(fh) = get_filter_coeff(mx, w, h_filter_type);
  let Ok(fv) = get_filter_coeff(my, h, v_filter_type);

  match (fh, fv) {
    (Some(fh), Some(fv)) => {
      let tmp_h = h + 7;
      let mut mid = [[0i32; MID_STRIDE]; 135];

      for y in 0..tmp_h {
        let src_off =
          src_base.wrapping_add_signed((y.cast_signed() - 3) * src_stride.cast_signed() - 3);
        let src_row = &src[src_off..];
        h_filter_8tap_16bpc_neon(
          token,
          &mut mid[y][..w],
          src_row,
          w,
          fh,
          6 - intermediate_bits,
        );
      }

      for y in 0..h {
        let dst_row = &mut dst[y * dst_stride..][..w];
        v_filter_8tap_16bpc_neon(
          token,
          dst_row,
          &mid[y..],
          w,
          fv,
          6 + intermediate_bits,
          bitdepth_max,
        );
      }
    }
    (Some(fh), None) => {
      let Ok(simd) = Simd::detected();

      for y in 0..h {
        let src_off =
          src_base.wrapping_add_signed(y.cast_signed() * src_stride.cast_signed() - 3);
        let src_row = &src[src_off..];
        let dst_row = &mut dst[y * dst_stride..][..w];

        match filter_word_tap_row(
          simd,
          dst_row,
          src_row,
          WordTapDirection::Horizontal,
          fh,
          bitdepth_max,
        ) {
          Ok(()) => {}
          Err(_invalid_filter_row) => return,
        }
      }
    }
    (None, Some(fv)) => {
      let Ok(simd) = Simd::detected();
      let Ok(pitch) = console_core_number_conversion::fixed_index(src_stride);

      for y in 0..h {
        let src_off =
          src_base.wrapping_add_signed((y.cast_signed() - 3) * src_stride.cast_signed());
        let src_row = &src[src_off..];
        let dst_row = &mut dst[y * dst_stride..][..w];

        match filter_word_tap_row(
          simd,
          dst_row,
          src_row,
          WordTapDirection::Vertical(pitch),
          fv,
          bitdepth_max,
        ) {
          Ok(()) => {}
          Err(_invalid_filter_row) => return,
        }
      }
    }
    (None, None) => {
      for y in 0..h {
        let src_row = &src[src_base + y * src_stride..][..w];
        let dst_row = &mut dst[y * dst_stride..][..w];
        dst_row.copy_from_slice(src_row);
      }
    }
  }
}

#[doc = r###"Vertical 8-tap filter from i32 intermediate to i16 output for 16bpc prep"###]
#[cfg(target_arch = "aarch64")]
#[arcane]
fn v_filter_8tap_16bpc_to_i16_neon(
  _token: Arm64,
  dst: &mut [i16],
  mid: &[[i32; MID_STRIDE]],
  w: usize,
  filter: &[i8; 8],
  sh: u8,
  bias: i32,
) {
  let mut dst = dst.flex_mut();
  let rnd = ::core::primitive::i32::wrapping_shr(1i32 << sh, 1_u32);
  let bias_vec = vdupq_n_s32(bias);

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

    let r0 = safe_simd::vld1q_s32(mid[0][col..][..4].try_into().unwrap());
    let r1 = safe_simd::vld1q_s32(mid[1][col..][..4].try_into().unwrap());
    let r2 = safe_simd::vld1q_s32(mid[2][col..][..4].try_into().unwrap());
    let r3 = safe_simd::vld1q_s32(mid[3][col..][..4].try_into().unwrap());
    let r4 = safe_simd::vld1q_s32(mid[4][col..][..4].try_into().unwrap());
    let r5 = safe_simd::vld1q_s32(mid[5][col..][..4].try_into().unwrap());
    let r6 = safe_simd::vld1q_s32(mid[6][col..][..4].try_into().unwrap());
    let r7 = safe_simd::vld1q_s32(mid[7][col..][..4].try_into().unwrap());

    let mut sum = vmulq_n_s32(r0, c0);
    sum = vmlaq_n_s32(sum, r1, c1);
    sum = vmlaq_n_s32(sum, r2, c2);
    sum = vmlaq_n_s32(sum, r3, c3);
    sum = vmlaq_n_s32(sum, r4, c4);
    sum = vmlaq_n_s32(sum, r5, c5);
    sum = vmlaq_n_s32(sum, r6, c6);
    sum = vmlaq_n_s32(sum, r7, c7);

    let rnd_vec = vdupq_n_s32(rnd);

    sum = vshlq_s32(
      vaddq_s32(sum, rnd_vec),
      vdupq_n_s32(::core::primitive::i32::wrapping_neg(
        <i32 as ::core::convert::From<::core::primitive::u8>>::from(sh),
      )),
    );

    sum = vsubq_s32(sum, bias_vec);

    let result = vqmovn_s32(sum);
    let dst_arr: &mut [i16; 4] = (&mut dst[col..col + 4]).try_into().unwrap();
    safe_simd::vst1_s16(dst_arr, result);
    col += 4;
  }

  for _ in (::core::ops::Range { start: col, end: w }) {
    let mut sum = 0i64;

    for (i, filter_element) in <[_]>::iter(filter).enumerate() {
      sum += ::core::primitive::i64::wrapping_mul(
        <i64 as ::core::convert::From<::core::primitive::i8>>::from(*filter_element),
        <i64 as ::core::convert::From<::core::primitive::i32>>::from(mid[i][col]),
      );
    }

    dst[col] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
      (((sum + <i64 as ::core::convert::From<::core::primitive::i32>>::from(rnd)) >> sh)
        - <i64 as ::core::convert::From<::core::primitive::i32>>::from(bias)),
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
    col = (::core::num::Wrapping(col) + ::core::num::Wrapping(1)).0;
  }
}

#[doc = r###"Main 8-tap prep function for 16bpc"###]
#[cfg(target_arch = "aarch64")]
#[arcane]
fn prep_8tap_16bpc_inner(
  token: Arm64,
  tmp: &mut [i16],
  src: &[u16],
  src_base: usize,
  src_stride: usize,
  w: usize,
  h: usize,
  mx: usize,
  my: usize,
  h_filter_type: Rav1dFilterMode,
  v_filter_type: Rav1dFilterMode,
  bitdepth_max: u16,
) {
  let mut tmp = tmp.flex_mut();
  let src = src.flex();

  let intermediate_bits =
    (match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
      bitdepth_max.leading_zeros(),
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    }) - 2;

  let bias = PREP_BIAS_16BPC;

  let h_sh = 6 - intermediate_bits;

  let Ok(fh) = get_filter_coeff(mx, w, h_filter_type);
  let Ok(fv) = get_filter_coeff(my, h, v_filter_type);

  match (fh, fv) {
    (Some(fh), Some(fv)) => {
      let tmp_h = h + 7;
      let mut mid = [[0i32; MID_STRIDE]; 135];

      for y in 0..tmp_h {
        let src_off =
          src_base.wrapping_add_signed((y.cast_signed() - 3) * src_stride.cast_signed() - 3);
        let src_row = &src[src_off..];
        h_filter_8tap_16bpc_neon(token, &mut mid[y][..w], src_row, w, fh, h_sh);
      }

      for y in 0..h {
        let out_row = &mut tmp[y * w..][..w];

        v_filter_8tap_16bpc_to_i16_neon(token, out_row, &mid[y..], w, fv, 6, bias);
      }
    }
    (Some(fh), None) => {
      let mut mid = [0i32; MID_STRIDE];

      for y in 0..h {
        let src_off =
          src_base.wrapping_add_signed(y.cast_signed() * src_stride.cast_signed() - 3);
        let src_row = &src[src_off..];
        h_filter_8tap_16bpc_neon(token, &mut mid[..w], src_row, w, fh, h_sh);
        let out_row = &mut tmp[y * w..][..w];

        for x in 0..w {
          out_row[x] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
            (mid[x] - bias),
          ) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          };
        }
      }
    }
    (None, Some(fv)) => {
      for y in 0..h {
        let out_row = &mut tmp[y * w..][..w];

        let mut mid = [[0i32; MID_STRIDE]; 8];

        for i in 0..8 {
          let src_off = src_base.wrapping_add_signed(
            (y.cast_signed() + ::core::primitive::usize::cast_signed(i) - 3)
              * src_stride.cast_signed(),
          );

          for x in 0..w {
            mid[i][x] =
              <i32 as ::core::convert::From<::core::primitive::u16>>::from(src[src_off + x])
                << intermediate_bits;
          }
        }

        v_filter_8tap_16bpc_to_i16_neon(token, out_row, &mid, w, fv, 6, bias);
      }
    }
    (None, None) => {
      for y in 0..h {
        let src_row = &src[src_base + y * src_stride..][..w];
        let out_row = &mut tmp[y * w..][..w];

        for x in 0..w {
          out_row[x] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
            ((<i32 as ::core::convert::From<::core::primitive::u16>>::from(src_row[x])
              << intermediate_bits)
              - bias),
          ) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          };
        }
      }
    }
  }
}

pub(crate) enum CompoundBlendFallback {
  Disabled,
  MissingCpuFeature,
  InvalidLayout,
}

pub(crate) fn compound_blend_dispatch<BD: BitDepth>(
  destination: PicOffset,
  predictors: [&[i16]; 2],
  extent: Size<i32>,
  operation: crate::src::mc::PredictorBlend<'_>,
  depth: BD,
) -> Result<(), CompoundBlendFallback> {
  use crate::include::common::bitdepth::BPC;
  use crate::src::mc::PredictorBlend;
  use console_core_number_conversion::{fixed_index, fixed_offset, index, wrapped_index};

  match crate::src::ablate::is_off(crate::src::ablate::Family::McOther).then_some(()) {
    Some(()) => return Err(CompoundBlendFallback::Disabled),
    None => {}
  }

  let token = Arm64::summon().ok_or(CompoundBlendFallback::MissingCpuFeature)?;
  let Ok(width) = wrapped_index(extent.width);
  let Ok(height) = wrapped_index(extent.height);
  let mut block = destination.block_mut::<BD>(width, height);
  let Ok(pixel_bytes) = fixed_index(core::mem::size_of::<BD::Pixel>());
  let Ok(pixel_origin) = fixed_index(block.base());
  let Ok(start) = index(pixel_origin.wrapping_mul(pixel_bytes));
  let Ok(signed_pitch) = fixed_offset(block.byte_stride());
  let pitch = u64::try_from(signed_pitch).map_err(|_| CompoundBlendFallback::InvalidLayout)?;
  let output = block
    .as_mut_bytes()
    .get_mut(start..)
    .ok_or(CompoundBlendFallback::InvalidLayout)?;
  let view = CompoundBlock {
    output,
    predictors,
    extent,
    pitch,
  };
  let bound = depth.into_c();

  match (BD::BPC, operation) {
    (BPC::BPC8, PredictorBlend::Average) => avg_8bpc_inner(token, view),
    (BPC::BPC16, PredictorBlend::Average) => avg_16bpc_inner(token, view, bound),
    (BPC::BPC8, PredictorBlend::Weighted(weight)) => w_avg_8bpc_inner(token, view, weight),
    (BPC::BPC16, PredictorBlend::Weighted(weight)) => w_avg_16bpc_inner(
      token,
      view,
      WordAverageScale {
        weight,
        maximum: bound,
      },
    ),
    (BPC::BPC8, PredictorBlend::Mask(weights)) => mask_8bpc_inner(token, view, weights),
    (BPC::BPC16, PredictorBlend::Mask(weights)) => mask_16bpc_inner(token, view, weights, bound),
  }
  .map_err(|_| CompoundBlendFallback::InvalidLayout)
}

#[cfg(target_arch = "aarch64")]
pub fn w_mask_dispatch<BD: BitDepth>(
  layout: Rav1dPixelLayoutSubSampled,
  dst: PicOffset,
  tmp1: &CompoundPrediction,
  tmp2: &CompoundPrediction,
  w: i32,
  h: i32,
  mask: &mut SegmentMask,
  sign: i32,
  bd: BD,
) -> bool {
  match {
    ::core::primitive::bool::then_some(
      crate::src::ablate::is_off(crate::src::ablate::Family::McOther),
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      return false;
    }
    ::core::option::Option::None => {}
  }

  let ::core::result::Result::Ok(pixel_size) =
    ::console_core_number_conversion::fixed_index(std::mem::size_of::<BD::Pixel>());

  let mut block = dst.block_mut::<BD>(
    match ::console_core_number_conversion::wrapped_index(w) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    },
    match ::console_core_number_conversion::wrapped_index(h) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    },
  );
  let ::core::result::Result::Ok(dst_stride) =
    ::console_core_number_conversion::fixed_offset(block.byte_stride());
  let ::core::result::Result::Ok(dst_offset) = ::console_core_number_conversion::fixed_index(
    block.base()
      * match ::console_core_number_conversion::index(pixel_size) {
        ::core::result::Result::Ok(native_position) => native_position,
      },
  );
  let dst_bytes = block.as_mut_bytes();
  w_mask_dispatch_inner::<BD>(
    layout,
    dst_bytes,
    match ::console_core_number_conversion::index(dst_offset) {
      ::core::result::Result::Ok(native_position_) => native_position_,
    },
    match ::console_core_number_conversion::wrapped_offset(dst_stride) {
      ::core::result::Result::Ok(native_position) => native_position,
    },
    tmp1,
    tmp2,
    w,
    h,
    mask,
    sign,
    bd,
  )
}

#[doc = r###"Inner w_mask dispatch — operates on pre-acquired byte slice.
Can be called directly with scheduler-owned guards."###]
#[cfg(target_arch = "aarch64")]
pub(crate) fn w_mask_dispatch_inner<BD: BitDepth>(
  layout: Rav1dPixelLayoutSubSampled,
  dst_bytes: &mut [u8],
  dst_offset: usize,
  dst_stride: isize,
  tmp1: &CompoundPrediction,
  tmp2: &CompoundPrediction,
  w: i32,
  h: i32,
  mask: &mut SegmentMask,
  sign: i32,
  bd: BD,
) -> bool {
  use crate::include::common::bitdepth::BPC;

  {
    let token = {
      let optional_token = Arm64::summon();

      match optional_token {
        Some(matched_token) => matched_token,
        ::core::option::Option::None => return false,
      }
    };

    let ::core::result::Result::Ok(w_u) = ::console_core_number_conversion::fixed_index(
      match ::console_core_number_conversion::wrapped_index(w) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      },
    );
    let ::core::result::Result::Ok(h_u) = ::console_core_number_conversion::fixed_index(
      match ::console_core_number_conversion::wrapped_index(h) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      },
    );
    let ::core::result::Result::Ok(dst_stride_u) =
      ::console_core_number_conversion::fixed_index(dst_stride.cast_unsigned());

    match BD::BPC {
      BPC::BPC8 => {
        let dst_slice = &mut dst_bytes[dst_offset..];

        match layout {
          Rav1dPixelLayoutSubSampled::I420 => w_mask_8bpc_inner(
            token,
            dst_slice,
            match ::console_core_number_conversion::index(dst_stride_u) {
              ::core::result::Result::Ok(native_position_) => native_position_,
            },
            {
              let _explicit_part = &*tmp1;
              let _explicit_slice: &[_] = _explicit_part;
              _explicit_slice
            },
            {
              let _explicit_part = &*tmp2;
              let _explicit_slice: &[_] = _explicit_part;
              _explicit_slice
            },
            match ::console_core_number_conversion::index(w_u) {
              ::core::result::Result::Ok(native_position_) => native_position_,
            },
            match ::console_core_number_conversion::index(h_u) {
              ::core::result::Result::Ok(native_position_) => native_position_,
            },
            {
              let _explicit_part = &mut *mask;
              let _explicit_slice: &mut [_] = _explicit_part;
              _explicit_slice
            },
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(sign) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            },
            true,
            true,
          ),
          Rav1dPixelLayoutSubSampled::I422 => w_mask_8bpc_inner(
            token,
            dst_slice,
            match ::console_core_number_conversion::index(dst_stride_u) {
              ::core::result::Result::Ok(native_position_) => native_position_,
            },
            {
              let _explicit_part = &*tmp1;
              let _explicit_slice: &[_] = _explicit_part;
              _explicit_slice
            },
            {
              let _explicit_part = &*tmp2;
              let _explicit_slice: &[_] = _explicit_part;
              _explicit_slice
            },
            match ::console_core_number_conversion::index(w_u) {
              ::core::result::Result::Ok(native_position_) => native_position_,
            },
            match ::console_core_number_conversion::index(h_u) {
              ::core::result::Result::Ok(native_position_) => native_position_,
            },
            {
              let _explicit_part = &mut *mask;
              let _explicit_slice: &mut [_] = _explicit_part;
              _explicit_slice
            },
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(sign) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            },
            true,
            false,
          ),
          Rav1dPixelLayoutSubSampled::I444 => w_mask_8bpc_inner(
            token,
            dst_slice,
            match ::console_core_number_conversion::index(dst_stride_u) {
              ::core::result::Result::Ok(native_position_) => native_position_,
            },
            {
              let _explicit_part = &*tmp1;
              let _explicit_slice: &[_] = _explicit_part;
              _explicit_slice
            },
            {
              let _explicit_part = &*tmp2;
              let _explicit_slice: &[_] = _explicit_part;
              _explicit_slice
            },
            match ::console_core_number_conversion::index(w_u) {
              ::core::result::Result::Ok(native_position_) => native_position_,
            },
            match ::console_core_number_conversion::index(h_u) {
              ::core::result::Result::Ok(native_position_) => native_position_,
            },
            {
              let _explicit_part = &mut *mask;
              let _explicit_slice: &mut [_] = _explicit_part;
              _explicit_slice
            },
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(sign) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            },
            false,
            false,
          ),
        }
      }
      BPC::BPC16 => {
        use crate::src::safe_simd::mc::{WeightedMaskMode, write_weighted_mask};
        use console_core_geometry::Size;

        let bytes = match dst_bytes.get_mut(dst_offset..) {
          Some(samples) => samples,
          None => return false,
        };
        let pitch = match u64::try_from(dst_stride) {
          Ok(stride) => stride,
          Err(_invalid_stride) => return false,
        };
        let Ok(polarity) = console_core_number_conversion::wrapped(sign);
        let mode = WeightedMaskMode {
          layout,
          sign: polarity,
          depth: bd,
        };

        match write_weighted_mask(
          bytes,
          pitch,
          [tmp1, tmp2],
          Size {
            width: w,
            height: h,
          },
          mask,
          mode,
        ) {
          Ok(()) => return true,
          Err(_invalid_layout) => return false,
        }
      }
    }
  }

  true
}

#[cfg(target_arch = "aarch64")]
#[must_use]
pub fn mc_put_dispatch<BD: BitDepth>(
  filter: Filter2d,
  dst: PicOffset,
  src: PicOffset,
  w: i32,
  h: i32,
  mx: i32,
  my: i32,
  bd: BD,
) -> bool {
  match {
    ::core::primitive::bool::then_some(
      crate::src::ablate::is_off(crate::src::ablate::Family::McPut),
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      return false;
    }
    ::core::option::Option::None => {}
  }

  match { ::core::primitive::bool::then_some(dst.data.ref_eq(src.data), ()) } {
    ::core::option::Option::Some(()) => {
      return false;
    }
    ::core::option::Option::None => {}
  }

  let ::core::result::Result::Ok(pixel_size) =
    ::console_core_number_conversion::fixed_index(std::mem::size_of::<BD::Pixel>());

  let mut block = dst.block_mut::<BD>(
    match ::console_core_number_conversion::wrapped_index(w) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    },
    match ::console_core_number_conversion::wrapped_index(h) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    },
  );
  let ::core::result::Result::Ok(dst_stride) =
    ::console_core_number_conversion::fixed_offset(block.byte_stride());
  let ::core::result::Result::Ok(dst_offset) = ::console_core_number_conversion::fixed_index(
    block.base()
      * match ::console_core_number_conversion::index(pixel_size) {
        ::core::result::Result::Ok(native_position) => native_position,
      },
  );
  let dst_bytes = block.as_mut_bytes();
  mc_put_dispatch_inner::<BD>(
    filter,
    dst_bytes,
    match ::console_core_number_conversion::index(dst_offset) {
      ::core::result::Result::Ok(native_position_) => native_position_,
    },
    match ::console_core_number_conversion::wrapped_offset(dst_stride) {
      ::core::result::Result::Ok(native_position) => native_position,
    },
    src,
    w,
    h,
    mx,
    my,
    bd,
  )
}

#[doc = r###"Inner mc_put dispatch — operates on pre-acquired dst byte slice.
src guard is acquired inside since src is from a different frame's DisjointMut.
Can be called directly with scheduler-owned dst guards."###]
#[cfg(target_arch = "aarch64")]
pub(crate) fn mc_put_dispatch_inner<BD: BitDepth>(
  filter: Filter2d,
  dst_bytes: &mut [u8],
  dst_offset: usize,
  dst_stride: isize,
  src: PicOffset,
  w: i32,
  h: i32,
  mx: i32,
  my: i32,
  bd: BD,
) -> bool {
  use crate::include::common::bitdepth::BPC;
  use Filter2d::*;

  {
    let token = {
      let optional_token = Arm64::summon();

      match optional_token {
        Some(matched_token) => matched_token,
        ::core::option::Option::None => return false,
      }
    };

    let w_u = match ::console_core_number_conversion::wrapped_index(w) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
    let h_u = match ::console_core_number_conversion::wrapped_index(h) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
    let mx_u = match ::console_core_number_conversion::wrapped_index(mx) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
    let my_u = match ::console_core_number_conversion::wrapped_index(my) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
    let (src_guard, src_base) = src.full_guard::<BD>();
    let src_stride_raw = src.stride();

    match BD::BPC {
      BPC::BPC8 => {
        use zerocopy::IntoBytes;
        let dst_slice = &mut dst_bytes[dst_offset..];
        let src_bytes = src_guard.as_bytes();
        let dst_stride_u = dst_stride.cast_unsigned();
        let src_stride_u = src_stride_raw.cast_unsigned();

        match { ::core::primitive::bool::then_some(filter == Bilinear, ()) } {
          ::core::option::Option::Some(()) => {
            let source = match src_bytes.get(src_base..) {
              Some(source) => source,
              None => return false,
            };
            let Ok(source_pitch) = console_core_number_conversion::fixed_index(src_stride_u);
            let Ok(output_pitch) = console_core_number_conversion::fixed_index(dst_stride_u);
            let block = BilinearBlock {
              output: dst_slice,
              source,
              source_pitch: source_pitch.cast_signed(),
              output_pitch: output_pitch.cast_signed(),
              extent: Size {
                width: u64::from(w.cast_unsigned()),
                height: u64::from(h.cast_unsigned()),
              },
              phase: Point { x: mx, y: my },
            };
            return matches!(put_bilin_8bpc_inner(token, block), Ok(()));
          }
          ::core::option::Option::None => {
            put_8tap_8bpc_inner(
              token,
              dst_slice,
              dst_stride_u,
              src_bytes,
              src_base,
              src_stride_u,
              w_u,
              h_u,
              mx_u,
              my_u,
              get_h_filter_type(filter),
              get_v_filter_type(filter),
            );
          }
        }
      }
      BPC::BPC16 => {
        use zerocopy::{FromBytes, IntoBytes};
        let src_bytes = src_guard.as_bytes();
        let dst_stride_u16 = dst_stride.cast_unsigned().wrapping_div(2);
        let src_stride_u16 = src_stride_raw.cast_unsigned().wrapping_div(2);
        let dst_start = dst_offset;

        let dst_byte_len = (h_u.saturating_sub(1) * dst_stride_u16 + w_u) * 2;
        let dst_u16: &mut [u16] =
          FromBytes::mut_from_bytes(&mut dst_bytes[dst_start..dst_start + dst_byte_len]).unwrap();

        match { ::core::primitive::bool::then_some(filter == Bilinear, ()) } {
          ::core::option::Option::Some(()) => {
            let source = match src_bytes
              .get(src_base.wrapping_mul(2)..)
              .and_then(|bytes| FromBytes::ref_from_bytes(bytes).ok())
            {
              Some(source) => source,
              None => return false,
            };
            let Ok(source_pitch) = console_core_number_conversion::fixed_index(src_stride_u16);
            let Ok(output_pitch) = console_core_number_conversion::fixed_index(dst_stride_u16);
            let block = BilinearBlock {
              output: dst_u16,
              source,
              source_pitch: source_pitch.cast_signed(),
              output_pitch: output_pitch.cast_signed(),
              extent: Size {
                width: u64::from(w.cast_unsigned()),
                height: u64::from(h.cast_unsigned()),
              },
              phase: Point { x: mx, y: my },
            };
            return matches!(put_bilin_16bpc_inner(block, bd.into_c()), Ok(()));
          }
          ::core::option::Option::None => {
            let src_u16: &[u16] = FromBytes::ref_from_bytes(src_bytes).unwrap();
            put_8tap_16bpc_inner(
              token,
              dst_u16,
              dst_stride_u16,
              src_u16,
              src_base,
              src_stride_u16,
              w_u,
              h_u,
              mx_u,
              my_u,
              get_h_filter_type(filter),
              get_v_filter_type(filter),
              match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
                bd.into_c(),
              ) {
                ::core::result::Result::Ok(converted_position) => converted_position,
              },
            );
          }
        }
      }
    }
  }

  true
}

#[cfg(target_arch = "aarch64")]
pub fn mct_prep_dispatch<BD: BitDepth>(
  filter: Filter2d,
  tmp: &mut [i16],
  src: PicOffset,
  w: i32,
  h: i32,
  mx: i32,
  my: i32,
  bd: BD,
) -> bool {
  match {
    ::core::primitive::bool::then_some(
      crate::src::ablate::is_off(crate::src::ablate::Family::McPrep),
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      return false;
    }
    ::core::option::Option::None => {}
  }

  use crate::include::common::bitdepth::BPC;
  use Filter2d::*;

  {
    let token = {
      let optional_token = Arm64::summon();

      match optional_token {
        Some(matched_token) => matched_token,
        ::core::option::Option::None => return false,
      }
    };

    let w_u = match ::console_core_number_conversion::wrapped_index(w) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
    let h_u = match ::console_core_number_conversion::wrapped_index(h) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
    let mx_u = match ::console_core_number_conversion::wrapped_index(mx) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
    let my_u = match ::console_core_number_conversion::wrapped_index(my) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
    let tmp_slice = &mut tmp[..(w_u * h_u)];
    let (src_guard, src_base) = src.full_guard::<BD>();
    let src_stride_raw = src.stride();

    match BD::BPC {
      BPC::BPC8 => {
        use zerocopy::IntoBytes;
        let src_bytes = src_guard.as_bytes();
        let src_stride_u = src_stride_raw.cast_unsigned();

        match { ::core::primitive::bool::then_some(filter == Bilinear, ()) } {
          ::core::option::Option::Some(()) => {
            let source = match src_bytes.get(src_base..) {
              Some(source) => source,
              None => return false,
            };
            let Ok(source_pitch) = console_core_number_conversion::fixed_index(src_stride_u);
            let block = BilinearBlock {
              output: tmp_slice,
              source,
              source_pitch: source_pitch.cast_signed(),
              output_pitch: i64::from(w),
              extent: Size {
                width: u64::from(w.cast_unsigned()),
                height: u64::from(h.cast_unsigned()),
              },
              phase: Point { x: mx, y: my },
            };
            return matches!(prep_bilin_8bpc_inner(token, block), Ok(()));
          }
          ::core::option::Option::None => {
            prep_8tap_8bpc_inner(
              token,
              tmp_slice,
              src_bytes,
              src_base,
              src_stride_u,
              w_u,
              h_u,
              mx_u,
              my_u,
              get_h_filter_type(filter),
              get_v_filter_type(filter),
            );
          }
        }
      }
      BPC::BPC16 => {
        use zerocopy::{FromBytes, IntoBytes};
        let src_bytes = src_guard.as_bytes();
        let src_stride_u16 = src_stride_raw.cast_unsigned().wrapping_div(2);

        match { ::core::primitive::bool::then_some(filter == Bilinear, ()) } {
          ::core::option::Option::Some(()) => {
            let source = match src_bytes
              .get(src_base.wrapping_mul(2)..)
              .and_then(|bytes| FromBytes::ref_from_bytes(bytes).ok())
            {
              Some(source) => source,
              None => return false,
            };
            let Ok(source_pitch) = console_core_number_conversion::fixed_index(src_stride_u16);
            let block = BilinearBlock {
              output: tmp_slice,
              source,
              source_pitch: source_pitch.cast_signed(),
              output_pitch: i64::from(w),
              extent: Size {
                width: u64::from(w.cast_unsigned()),
                height: u64::from(h.cast_unsigned()),
              },
              phase: Point { x: mx, y: my },
            };
            return matches!(prep_bilin_16bpc_inner(block, bd.into_c()), Ok(()));
          }
          ::core::option::Option::None => {
            let src_u16: &[u16] = FromBytes::ref_from_bytes(src_bytes).unwrap();
            prep_8tap_16bpc_inner(
              token,
              tmp_slice,
              src_u16,
              src_base,
              src_stride_u16,
              w_u,
              h_u,
              mx_u,
              my_u,
              get_h_filter_type(filter),
              get_v_filter_type(filter),
              match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u16>(
                bd.into_c(),
              ) {
                ::core::result::Result::Ok(converted_position) => converted_position,
              },
            );
          }
        }
      }
    }
  }

  true
}

#[cfg(all(test, target_arch = "aarch64"))]
mod compound_parity {
  use super::*;
  use crate::include::common::bitdepth::{BitDepth, BitDepth8, BitDepth16};
  use std::cmp;

  #[doc = r###"xorshift64*, so a failure reproduces from its seed."###]
  struct Rng(u64);

  impl Rng {
    fn in_range(&mut self, lo: i32, hi: i32) -> i32 {
      let Ok(seeded) = crate::src::safe_simd::next_fixture_sample(&mut self.0);
      lo + match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(
        (seeded
          % match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u64>(
            (hi - lo + 1),
          ) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          }),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      }
    }
  }

  #[must_use]
  fn bd_max_of(bitdepth: u8) -> i32 {
    (1i32 << bitdepth) - 1
  }

  #[doc = r###"`intermediate_bits` as the spec defines it: 4 at 8 and 10 bits, **2 at
12 bits**. A kernel that writes a literal `4` is right at 8 and 10 and
wrong at 12 — the bug shape this module hunts."###]
  #[must_use]
  fn intermediate_bits(bitdepth: u8) -> u8 {
    match { ::core::primitive::bool::then_some(bitdepth == 8, ()) } {
      ::core::option::Option::Some(()) => 4,
      ::core::option::Option::None => 14 - bitdepth,
    }
  }

  #[doc = r###"Result accumulator: which parameter cells ran, which diverged."###]
  #[derive(Default)]
  struct Report {
    cells: usize,
    bad: Vec<String>,
    first: Option<String>,
  }

  impl Report {
    fn record(&mut self, label: &str, ok: bool, detail: impl FnOnce() -> String) {
      self.cells += 1;

      match { ::core::primitive::bool::then_some(!ok, ()) } {
        ::core::option::Option::Some(()) => {
          match { ::core::primitive::bool::then_some(self.first.is_none(), ()) } {
            ::core::option::Option::Some(()) => {
              self.first = Some(format!("{label}: {}", detail()));
            }
            ::core::option::Option::None => {}
          }

          self.bad.push(label.to_string());
        }
        ::core::option::Option::None => {}
      }
    }

    #[doc = r###"Fail on any divergence, AND on a vacuous run.

`min_cells` is the liveness half. Without it, a refactor that made
the sweep skip every case (an emptied size list, a `continue` that
swallows the interesting shapes) would leave the parity assert
trivially true and the test permanently, uselessly green."###]
    fn finish(self, what: &str, min_cells: usize) {
      assert!(
        self.cells >= min_cells,
        "{what}: only {} parameter cells ran (expected >= {min_cells}) \
                 — the sweep is not reaching the kernel",
        self.cells
      );
      assert!(
        self.bad.is_empty(),
        "{what}: {} of {} parameter cells diverge from the scalar \
                 reference.\n  first: {}\n  cells: {:?}",
        self.bad.len(),
        self.cells,
        self.first.unwrap_or_default(),
        self.bad
      );
    }
  }

  #[must_use]
  fn avg8_oracle(Avg8OracleArguments { t1, t2 }: Avg8OracleArguments) -> i32 {
    ::core::primitive::i32::wrapping_shr(
      t1 + t2 + ::core::primitive::i32::wrapping_shl(1, 4_u32),
      5_u32,
    )
    .clamp(0, 255)
  }

  struct Avg8OracleArguments {
    t1: i32,
    t2: i32,
  }

  #[must_use]
  fn w_avg8_oracle(WAvg8OracleArguments { t1, t2, weight }: WAvg8OracleArguments) -> i32 {
    ::core::primitive::i32::wrapping_shr(
      t1 * weight + t2 * (16 - weight) + ::core::primitive::i32::wrapping_shl(8, 4_u32),
      8_u32,
    )
    .clamp(0, 255)
  }

  struct WAvg8OracleArguments {
    t1: i32,
    t2: i32,
    weight: i32,
  }

  #[must_use]
  fn mask8_oracle(Mask8OracleArguments { t1, t2, m }: Mask8OracleArguments) -> i32 {
    ::core::primitive::i32::wrapping_shr(
      t1 * m + t2 * (64 - m) + ::core::primitive::i32::wrapping_shl(32, 4_u32),
      10_u32,
    )
    .clamp(0, 255)
  }

  struct Mask8OracleArguments {
    t1: i32,
    t2: i32,
    m: i32,
  }

  #[doc = r###"The segmentation mask `w_mask_rust` writes for one row, given that
row's per-pixel weights and (for 4:2:0) the previous row's stored
values. Transcribed from `src/mc.rs::w_mask_rust`'s store arm.

`sign` lives HERE and only here. The pixel blend uses `m` unmodified —
getting that wrong is invisible at `sign = 0` and wrong on half the
wedge blocks."###]
  #[must_use]
  fn w_mask_store_oracle(
    mrow: &[u8],
    prev: &[u8],
    y: usize,
    sign: u8,
    ss_hor: bool,
    ss_ver: bool,
  ) -> Vec<u8> {
    let w = mrow.len();

    match { ::core::primitive::bool::then_some(!ss_hor, ()) } {
      ::core::option::Option::Some(()) => {
        return mrow.to_vec();
      }
      ::core::option::Option::None => {}
    }

    let mut out = vec![0u8; w.wrapping_shr(1_u32)];
    let mut x = 0;

    while x + 1 < w {
      let (m, n) = (mrow[x], mrow[x + 1]);
      out[::core::primitive::usize::wrapping_shr(x, 1_u32)] = if ss_ver && (y & 1) != 0 {
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
          ::core::primitive::u16::wrapping_shr(
            ::core::primitive::u16::wrapping_add(
              <u16 as ::core::convert::From<::core::primitive::u8>>::from((m + n + 2 - sign)),
              <u16 as ::core::convert::From<::core::primitive::u8>>::from(
                prev[::core::primitive::usize::wrapping_shr(x, 1_u32)],
              ),
            ),
            2_u32,
          ),
        ) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        }
      } else if ss_ver {
        m + n
      } else {
        ::core::primitive::u8::wrapping_shr(m + n + 1 - sign, 1_u32)
      };
      x += 2;
    }

    out
  }

  #[doc = r###"`w_mask`'s per-pixel weight; identical at every bit depth except for
`mask_sh`, and bias-invariant because it reads `|t1 - t2|`."###]
  #[must_use]
  fn w_mask_m_oracle(
    WMaskMOracleArguments { t1, t2 }: WMaskMOracleArguments,
    bitdepth: u8,
  ) -> u8 {
    let mask_sh = bitdepth + intermediate_bits(bitdepth) - 4;
    let mask_rnd = 1u16 << (mask_sh - 5);

    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(cmp::min(
      38 + (t1.abs_diff(t2).saturating_add(mask_rnd) >> mask_sh),
      64,
    )) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    }
  }

  struct WMaskMOracleArguments {
    t1: i16,
    t2: i16,
  }

  #[must_use]
  fn w_mask8_px_oracle(WMask8PxOracleArguments { t1, t2, m }: WMask8PxOracleArguments) -> i32 {
    ::core::primitive::i32::wrapping_shr(
      t1 * m + t2 * (64 - m) + ::core::primitive::i32::wrapping_shl(32, 4_u32),
      10_u32,
    )
    .clamp(0, 255)
  }

  struct WMask8PxOracleArguments {
    t1: i32,
    t2: i32,
    m: i32,
  }

  #[doc = r###"The legal range of one 8bpc `prep` output, from `BitDepth8::PREP_BIAS`'s
own doc comment (`[-5132, 9212]`). Sampling the real interval rather
than all of `i16` is what keeps a wrong `>>` visible instead of clamped
away at both ends."###]
  const T8_LO: i32 = -5132;
  const T8_HI: i32 = 9212;

  #[doc = r###"Every (w, h) an AV1 inter block can be."###]
  const SIZES: &[(usize, usize)] = &[
    (4, 4),
    (4, 8),
    (8, 4),
    (8, 8),
    (8, 16),
    (16, 8),
    (16, 16),
    (16, 32),
    (32, 16),
    (32, 32),
    (32, 64),
    (64, 32),
    (64, 64),
    (128, 128),
  ];

  fn compound_fixture<'a, Pixel: IntoBytes + FromBytes>(
    output: &'a mut [Pixel],
    predictors: [&'a [i16]; 2],
    layout: FindBadArguments,
  ) -> Result<CompoundBlock<'a>, console_core_never::Never> {
    use console_core_number_conversion::fixed_index;

    let FindBadArguments { stride, w, h } = layout;
    let Ok(width) = wrapped(w);
    let Ok(height) = wrapped(h);
    let Ok(samples_per_row) = fixed_index(stride);
    let Ok(pixel_bytes) = fixed_index(core::mem::size_of::<Pixel>());
    Ok(CompoundBlock {
      output: output.as_mut_bytes(),
      predictors,
      extent: Size { width, height },
      pitch: samples_per_row.wrapping_mul(pixel_bytes),
    })
  }

  #[test]
  fn compound_8bpc_matches_scalar() {
    let Ok(_lock) = crate::src::safe_simd::token_test_lock();
    let token = Arm64::summon().expect("NEON is mandatory on aarch64");
    let mut rep = Report::default();

    for &(w, h) in SIZES {
      let Ok(capacity) = console_core_number_conversion::index(MAX_COMPOUND_SAMPLES);

      match { ::core::primitive::bool::then_some(w * h > capacity, ()) } {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      let mut rng = Rng(
        0x1234_5678_9ABC_DEF0
          ^ match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u64>((w * h))
          {
            ::core::result::Result::Ok(converted_position) => converted_position,
          },
      );
      let t1: Vec<i16> = (0..w * h)
        .map(|_| {
          match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
            rng.in_range(T8_LO, T8_HI),
          ) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          }
        })
        .collect();
      let t2: Vec<i16> = (0..w * h)
        .map(|_| {
          match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
            rng.in_range(T8_LO, T8_HI),
          ) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          }
        })
        .collect();

      let m: Vec<u8> = (0..w * h)
        .map(|_| {
          match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
            rng.in_range(0, 64),
          ) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          }
        })
        .collect();
      let stride = w + 7;

      let mut got = vec![0u8; h * stride];
      let Ok(view) = compound_fixture(&mut got, [&t1, &t2], FindBadArguments { stride, w, h });
      assert_eq!(avg_8bpc_inner(token, view), Ok(()));
      let bad = find_bad(&got, self::FindBadArguments { stride, w, h }, |x, y| {
        ::core::primitive::i32::cast_unsigned(avg8_oracle(self::Avg8OracleArguments {
          t1: <i32 as ::core::convert::From<::core::primitive::i16>>::from(t1[y * w + x]),
          t2: <i32 as ::core::convert::From<::core::primitive::i16>>::from(t2[y * w + x]),
        }))
      });
      rep.record(&format!("avg8 {w}x{h}"), bad.is_none(), || fmt_bad(bad));

      for &weight in &[0i32, 1, 4, 8, 12, 15, 16] {
        let mut got_2 = vec![0u8; h * stride];
        let Ok(view_2) =
          compound_fixture(&mut got_2, [&t1, &t2], FindBadArguments { stride, w, h });
        assert_eq!(w_avg_8bpc_inner(token, view_2, weight), Ok(()));
        let bad_2 = find_bad(&got_2, self::FindBadArguments { stride, w, h }, |x, y| {
          ::core::primitive::i32::cast_unsigned(w_avg8_oracle(self::WAvg8OracleArguments {
            t1: <i32 as ::core::convert::From<::core::primitive::i16>>::from(t1[y * w + x]),
            t2: <i32 as ::core::convert::From<::core::primitive::i16>>::from(t2[y * w + x]),
            weight,
          }))
        });
        rep.record(
          &format!("w_avg8 wt={weight} {w}x{h}"),
          bad_2.is_none(),
          || fmt_bad(bad_2),
        );
      }

      let mut got_3 = vec![0u8; h * stride];
      let Ok(view_3) =
        compound_fixture(&mut got_3, [&t1, &t2], FindBadArguments { stride, w, h });
      assert_eq!(mask_8bpc_inner(token, view_3, &m), Ok(()));
      let bad_3 = find_bad(&got_3, self::FindBadArguments { stride, w, h }, |x, y| {
        ::core::primitive::i32::cast_unsigned(mask8_oracle(self::Mask8OracleArguments {
          t1: <i32 as ::core::convert::From<::core::primitive::i16>>::from(t1[y * w + x]),
          t2: <i32 as ::core::convert::From<::core::primitive::i16>>::from(t2[y * w + x]),
          m: <i32 as ::core::convert::From<::core::primitive::u8>>::from(m[y * w + x]),
        }))
      });
      rep.record(&format!("mask8 {w}x{h}"), bad_3.is_none(), || {
        fmt_bad(bad_3)
      });

      for &(ss_hor, ss_ver) in &[(false, false), (true, false), (true, true)] {
        for &sign in &[0u8, 1] {
          let mask_w = match ::core::primitive::bool::then_some(ss_hor, ()) {
            ::core::option::Option::Some(()) => ::core::primitive::usize::wrapping_shr(w, 1_u32),
            ::core::option::Option::None => w,
          };
          let mask_h = match ::core::primitive::bool::then_some(ss_ver, ()) {
            ::core::option::Option::Some(()) => ::core::primitive::usize::wrapping_shr(h, 1_u32),
            ::core::option::Option::None => h,
          };
          let mut seg = vec![0u8; mask_w * mask_h];
          let mut got_4 = vec![0u8; h * stride];
          w_mask_8bpc_inner(
            token, &mut got_4, stride, &t1, &t2, w, h, &mut seg, sign, ss_hor, ss_ver,
          );

          let bad_4 = find_bad(&got_4, self::FindBadArguments { stride, w, h }, |x, y| {
            let (a, b) = (t1[y * w + x], t2[y * w + x]);
            let mm = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
              w_mask_m_oracle(self::WMaskMOracleArguments { t1: a, t2: b }, 8),
            );
            ::core::primitive::i32::cast_unsigned(w_mask8_px_oracle(
              self::WMask8PxOracleArguments {
                t1: <i32 as ::core::convert::From<::core::primitive::i16>>::from(a),
                t2: <i32 as ::core::convert::From<::core::primitive::i16>>::from(b),
                m: mm,
              },
            ))
          });
          rep.record(
            &format!("w_mask8 px sign={sign} ss=({ss_hor},{ss_ver}) {w}x{h}"),
            bad_4.is_none(),
            || fmt_bad(bad_4),
          );

          let mut want = vec![0u8; mask_w * mask_h];

          for y in 0..h {
            let mrow: Vec<u8> = (0..w)
              .map(|x| {
                w_mask_m_oracle(
                  self::WMaskMOracleArguments {
                    t1: t1[y * w + x],
                    t2: t2[y * w + x],
                  },
                  8,
                )
              })
              .collect();
            let ::core::result::Result::Ok(my) = ::console_core_number_conversion::fixed_index(
              match ::core::primitive::bool::then_some(ss_ver, ()) {
                ::core::option::Option::Some(()) => {
                  ::core::primitive::usize::wrapping_shr(y, 1_u32)
                }
                ::core::option::Option::None => y,
              },
            );
            let prev = want[(match ::console_core_number_conversion::index(my) {
              ::core::result::Result::Ok(native_position) => native_position,
            }) * mask_w..][..mask_w]
              .to_vec();
            let row = w_mask_store_oracle(&mrow, &prev, y, sign, ss_hor, ss_ver);
            want[(match ::console_core_number_conversion::index(my) {
              ::core::result::Result::Ok(native_position) => native_position,
            }) * mask_w..][..mask_w]
              .copy_from_slice(&row);
          }

          let bad_seg = (0..mask_h)
            .flat_map(|y| (0..mask_w).map(move |x| (x, y)))
            .find(|&(x, y)| seg[y * mask_w + x] != want[y * mask_w + x])
            .map(|(x, y)| {
              (
                x,
                y,
                <u32 as ::core::convert::From<::core::primitive::u8>>::from(seg[y * mask_w + x]),
                <u32 as ::core::convert::From<::core::primitive::u8>>::from(want[y * mask_w + x]),
              )
            });
          rep.record(
            &format!("w_mask8 seg sign={sign} ss=({ss_hor},{ss_ver}) {w}x{h}"),
            bad_seg.is_none(),
            || fmt_bad(bad_seg),
          );
        }
      }
    }

    rep.finish("aarch64 8bpc compound kernels", 150);
  }

  #[must_use]
  fn find_bad(
    got: &[impl Copy + Into<u32>],
    FindBadArguments { stride, w, h }: FindBadArguments,
    want: impl Fn(usize, usize) -> u32,
  ) -> Option<(usize, usize, u32, u32)> {
    for y in 0..h {
      for x in 0..w {
        let g: u32 = got[y * stride + x].into();
        let wv = want(x, y);

        match { ::core::primitive::bool::then_some(g != wv, ()) } {
          ::core::option::Option::Some(()) => {
            return Some((x, y, g, wv));
          }
          ::core::option::Option::None => {}
        }
      }
    }

    None
  }

  struct FindBadArguments {
    stride: usize,
    w: usize,
    h: usize,
  }

  #[must_use]
  fn fmt_bad(bad: Option<(usize, usize, u32, u32)>) -> String {
    let (x, y, g, w) = bad.expect("only called on a failing cell");
    format!("at ({x},{y}) neon={g} scalar={w}")
  }

  #[doc = r###"Recover the right shift a kernel actually applies, without knowing its
additive constant.

This is the convention-free half. `out = clamp((k * t + C) >> sh)` for
some unknown `C`: sweeping `t` and measuring how far it must move to
change `out` by one recovers `2^sh / k` regardless of `C`. So it tests
the `intermediate_bits` term — the thing that must vary with bit depth —
without touching the `PREP_BIAS` term, which the aarch64 seam defines
differently from the scalar one (see the module comment).

Returns the modal step, which is robust to the clamped ends."###]
  #[must_use]
  fn measure_input_step(mut eval: impl FnMut(i32) -> u32) -> Option<i32> {
    let mut transitions = Vec::new();
    let mut prev = eval(-30000);

    for t in -30000i32..30000 {
      let cur = eval(t);

      match { ::core::primitive::bool::then_some(cur != prev, ()) } {
        ::core::option::Option::Some(()) => {
          transitions.push(t);
          prev = cur;
        }
        ::core::option::Option::None => {}
      }
    }

    match { ::core::primitive::bool::then_some(transitions.len() < 8, ()) } {
      ::core::option::Option::Some(()) => {
        return None;
      }
      ::core::option::Option::None => {}
    }

    let mut gaps: Vec<i32> = transitions.windows(2).map(|w| w[1] - w[0]).collect();
    gaps.sort_unstable();
    Some(gaps[gaps.len().wrapping_div(2)])
  }

  #[test]
  fn compound_16bpc_shift_tracks_bitdepth() {
    let Ok(_lock) = crate::src::safe_simd::token_test_lock();
    let token = Arm64::summon().expect("NEON is mandatory on aarch64");
    let mut rep = Report::default();

    for &bitdepth in &[10u8, 12] {
      let bd_max = bd_max_of(bitdepth);
      let ib = intermediate_bits(bitdepth);

      let want = 1i32 << (ib + 1);
      let got = measure_input_step(|t| {
        let mut dst = [0u16; 1];
        let Ok(sample) = wrapped::<_, i16>(t);
        let predictors = [[sample], [0_i16]];
        let [first, second] = &predictors;
        let Ok(view) = compound_fixture(
          &mut dst,
          [first, second],
          FindBadArguments {
            stride: 1,
            w: 1,
            h: 1,
          },
        );
        assert_eq!(avg_16bpc_inner(token, view, bd_max), Ok(()));
        <u32 as ::core::convert::From<::core::primitive::u16>>::from(match dst {
          [_explicit_element, ..] => _explicit_element,
        })
      });
      rep.record(
        &format!("avg16 bd={bitdepth} shift"),
        got == Some(want),
        || format!("input step {got:?}, expected {want} (= 2^(intermediate_bits+1))"),
      );

      let want_2 = 1i32 << ib;
      let got_2 = measure_input_step(|t| {
        let mut dst = [0u16; 1];
        let Ok(sample) = wrapped::<_, i16>(t);
        let predictors = [[sample], [0_i16]];
        let [first, second] = &predictors;
        let Ok(view) = compound_fixture(
          &mut dst,
          [first, second],
          FindBadArguments {
            stride: 1,
            w: 1,
            h: 1,
          },
        );
        assert_eq!(
          w_avg_16bpc_inner(
            token,
            view,
            WordAverageScale {
              weight: 16,
              maximum: bd_max
            }
          ),
          Ok(())
        );
        <u32 as ::core::convert::From<::core::primitive::u16>>::from(match dst {
          [_explicit_element, ..] => _explicit_element,
        })
      });
      rep.record(
        &format!("w_avg16 bd={bitdepth} shift"),
        got_2 == Some(want_2),
        || {
          format!(
            "input step {got:?}, expected {want} (= 2^intermediate_bits)",
            want = want_2,
            got = got_2
          )
        },
      );

      let want_3 = 1i32 << ib;
      let got_3 = measure_input_step(|t| {
        let mut dst = [0u16; 1];
        let Ok(sample) = wrapped::<_, i16>(t);
        let predictors = [[sample], [0_i16]];
        let [first, second] = &predictors;
        let Ok(view) = compound_fixture(
          &mut dst,
          [first, second],
          FindBadArguments {
            stride: 1,
            w: 1,
            h: 1,
          },
        );
        assert_eq!(mask_16bpc_inner(token, view, &[64], bd_max), Ok(()));
        <u32 as ::core::convert::From<::core::primitive::u16>>::from(match dst {
          [_explicit_element, ..] => _explicit_element,
        })
      });
      rep.record(
        &format!("mask16 bd={bitdepth} shift"),
        got_3 == Some(want_3),
        || {
          format!(
            "input step {got:?}, expected {want} (= 2^intermediate_bits)",
            want = want_3,
            got = got_3
          )
        },
      );
    }

    rep.finish("aarch64 16bpc compound shift", 6);
  }

  fn word_mask_case(
    output: &mut [u16],
    predictors: [&[i16]; 2],
    size: console_core_geometry::Size<u64>,
    stride: u64,
    mask: &mut [u8],
    mode: crate::src::safe_simd::mc::WeightedMaskMode<
      crate::include::common::bitdepth::BitDepth16,
    >,
  ) -> crate::src::error::Rav1dResult {
    use crate::src::error::Rav1dError;
    use console_core_geometry::Size;
    use zerocopy::IntoBytes;

    let width = i32::try_from(size.width).map_err(|_| Rav1dError::EINVAL)?;
    let height = i32::try_from(size.height).map_err(|_| Rav1dError::EINVAL)?;
    crate::src::safe_simd::mc::write_weighted_mask(
      output.as_mut_bytes(),
      stride.wrapping_mul(2),
      predictors,
      Size { width, height },
      mask,
      mode,
    )
  }

  #[doc = r###"`w_mask`'s segmentation mask output, which is bias-invariant.

The written mask is `min(38 + (|t1-t2| + rnd >> mask_sh), 64)` — it
reads a DIFFERENCE, so whatever bias convention the tmp buffers carry
cancels. That makes it checkable at 10 and 12 bits with no ambiguity,
and it exercises `mask_sh = bitdepth + intermediate_bits - 4`, the other
place the 12-bit `intermediate_bits` term appears."###]
  #[test]
  fn w_mask_16bpc_segmentation_mask_matches_scalar() {
    let Ok(_lock) = crate::src::safe_simd::token_test_lock();
    let mut rep = Report::default();

    for &bitdepth in &[10u8, 12] {
      for &sign in &[0u8, 1] {
        for &(w, h) in &[(8usize, 8usize), (16, 16), (32, 32)] {
          let mut rng = Rng(
            0xC0FF_EE00_1234_5678
              ^ ::core::primitive::u64::wrapping_shl(
                <u64 as ::core::convert::From<::core::primitive::u8>>::from(bitdepth),
                40_u32,
              )
              ^ <u64 as ::core::convert::From<::core::primitive::u8>>::from(sign),
          );

          let t1: Vec<i16> = (0..w * h)
            .map(|_| {
              match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
                rng.in_range(-20602, ::core::primitive::i32::wrapping_sub(36983, 16384)),
              ) {
                ::core::result::Result::Ok(converted_position) => converted_position,
              }
            })
            .collect();
          let t2: Vec<i16> = (0..w * h)
            .map(|_| {
              match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
                rng.in_range(-20602, ::core::primitive::i32::wrapping_sub(36983, 16384)),
              ) {
                ::core::result::Result::Ok(converted_position) => converted_position,
              }
            })
            .collect();
          let stride = w + 5;
          let mut got_px = vec![0u16; h * stride];
          let mut seg = vec![0u8; w * h];
          let Ok(width) = console_core_number_conversion::fixed_index(w);
          let Ok(height) = console_core_number_conversion::fixed_index(h);
          let Ok(pitch) = console_core_number_conversion::fixed_index(stride);
          let Ok(maximum) = console_core_number_conversion::wrapped(bd_max_of(bitdepth));
          let mode = crate::src::safe_simd::mc::WeightedMaskMode {
            layout: Rav1dPixelLayoutSubSampled::I444,
            sign,
            depth: crate::include::common::bitdepth::BitDepth16::new(maximum),
          };
          assert_eq!(
            word_mask_case(
              &mut got_px,
              [&t1, &t2],
              console_core_geometry::Size { width, height },
              pitch,
              &mut seg,
              mode,
            ),
            Ok(())
          );
          let mut bad = None;

          for y in 0..h {
            for x in 0..w {
              let want = w_mask_m_oracle(
                self::WMaskMOracleArguments {
                  t1: t1[y * w + x],
                  t2: t2[y * w + x],
                },
                bitdepth,
              );

              match {
                ::core::primitive::bool::then_some(seg[y * w + x] != want && bad.is_none(), ())
              } {
                ::core::option::Option::Some(()) => {
                  bad = Some((
                    x,
                    y,
                    <u32 as ::core::convert::From<::core::primitive::u8>>::from(seg[y * w + x]),
                    <u32 as ::core::convert::From<::core::primitive::u8>>::from(want),
                  ));
                }
                ::core::option::Option::None => {}
              }
            }
          }

          rep.record(
            &format!("w_mask16 seg bd={bitdepth} sign={sign} {w}x{h}"),
            bad.is_none(),
            || fmt_bad(bad),
          );
        }
      }
    }

    rep.finish("w_mask_16bpc_inner segmentation mask", 12);
  }

  #[doc = r###"Full VALUE parity for the 16bpc compound kernels, at both bit depths.

This is what the module could not assert before: the aarch64 16bpc seam
ran two different `PREP_BIAS` conventions at once (`prep_8tap` and
avg/w_avg/mask omitted it; `prep_bilin` and `w_mask` used it), so an
isolated value test against a scalar-convention oracle reported one
`PREP_BIAS` of seam artefact. All five now speak the scalar convention —
the only one a decoder that falls back to `prep_8tap_rust` for uncovered
shapes can survive — so the oracle is simply `src/mc.rs`'s formulas with
`BitDepth16::PREP_BIAS` in place."###]
  #[test]
  fn compound_16bpc_values_match_scalar() {
    let Ok(_lock) = crate::src::safe_simd::token_test_lock();
    let token = Arm64::summon().expect("NEON is mandatory on aarch64");
    let mut rep = Report::default();

    const T16_LO: i32 = -20602 - 8192;
    const T16_HI: i32 = 36983 - 8192;

    for &bitdepth in &[10u8, 12] {
      let bd_max = bd_max_of(bitdepth);
      let ib =
        <i32 as ::core::convert::From<::core::primitive::u8>>::from(intermediate_bits(bitdepth));
      let bias = i32::from(BitDepth16::PREP_BIAS);

      'inner_for_2: for &(w, h) in SIZES {
        let Ok(capacity) = console_core_number_conversion::index(MAX_COMPOUND_SAMPLES);

        match { ::core::primitive::bool::then_some(w * h > capacity, ()) } {
          ::core::option::Option::Some(()) => {
            continue 'inner_for_2;
          }
          ::core::option::Option::None => {}
        }

        let mut rng = Rng(
          0xABCD_1234_0000_0001
            ^ ::core::primitive::u64::wrapping_shl(
              <u64 as ::core::convert::From<::core::primitive::u8>>::from(bitdepth),
              40_u32,
            )
            ^ match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u64>(
              (w * h),
            ) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            },
        );
        let t1: Vec<i16> = (0..w * h)
          .map(|_| {
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
              rng.in_range(T16_LO, T16_HI),
            ) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            }
          })
          .collect();
        let t2: Vec<i16> = (0..w * h)
          .map(|_| {
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
              rng.in_range(T16_LO, T16_HI),
            ) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            }
          })
          .collect();
        let m: Vec<u8> = (0..w * h)
          .map(|_| {
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
              rng.in_range(0, 64),
            ) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            }
          })
          .collect();
        let stride = w + 5;

        let mut got = vec![0u16; h * stride];
        let Ok(view) = compound_fixture(&mut got, [&t1, &t2], FindBadArguments { stride, w, h });
        assert_eq!(avg_16bpc_inner(token, view, bd_max), Ok(()));
        let bad = find_bad(&got, self::FindBadArguments { stride, w, h }, |x, y| {
          let (a, b) = (
            <i32 as ::core::convert::From<::core::primitive::i16>>::from(t1[y * w + x]),
            <i32 as ::core::convert::From<::core::primitive::i16>>::from(t2[y * w + x]),
          );
          ::core::primitive::i32::cast_unsigned(
            ((a + b + (1 << ib) + bias * 2) >> ::core::primitive::i32::wrapping_add(ib, 1))
              .clamp(0, bd_max),
          )
        });
        rep.record(
          &format!("avg16 bd={bitdepth} {w}x{h}"),
          bad.is_none(),
          || fmt_bad(bad),
        );

        for &weight in &[0i32, 1, 4, 8, 12, 15, 16] {
          let mut got_2 = vec![0u16; h * stride];
          let Ok(view_2) =
            compound_fixture(&mut got_2, [&t1, &t2], FindBadArguments { stride, w, h });
          assert_eq!(
            w_avg_16bpc_inner(
              token,
              view_2,
              WordAverageScale {
                weight,
                maximum: bd_max
              }
            ),
            Ok(())
          );
          let bad_2 = find_bad(&got_2, self::FindBadArguments { stride, w, h }, |x, y| {
            let (a, b) = (
              <i32 as ::core::convert::From<::core::primitive::i16>>::from(t1[y * w + x]),
              <i32 as ::core::convert::From<::core::primitive::i16>>::from(t2[y * w + x]),
            );
            ::core::primitive::i32::cast_unsigned(
              ((a * weight + b * (16 - weight) + (8 << ib) + bias * 16)
                >> ::core::primitive::i32::wrapping_add(ib, 4))
              .clamp(0, bd_max),
            )
          });
          rep.record(
            &format!("w_avg16 bd={bitdepth} wt={weight} {w}x{h}"),
            bad_2.is_none(),
            || fmt_bad(bad_2),
          );
        }

        let mut got_3 = vec![0u16; h * stride];
        let Ok(view_3) =
          compound_fixture(&mut got_3, [&t1, &t2], FindBadArguments { stride, w, h });
        assert_eq!(mask_16bpc_inner(token, view_3, &m, bd_max), Ok(()));
        let bad_3 = find_bad(&got_3, self::FindBadArguments { stride, w, h }, |x, y| {
          let (a, b) = (
            <i32 as ::core::convert::From<::core::primitive::i16>>::from(t1[y * w + x]),
            <i32 as ::core::convert::From<::core::primitive::i16>>::from(t2[y * w + x]),
          );
          let mm = <i32 as ::core::convert::From<::core::primitive::u8>>::from(m[y * w + x]);
          ::core::primitive::i32::cast_unsigned(
            ((a * mm + b * (64 - mm) + (32 << ib) + bias * 64)
              >> ::core::primitive::i32::wrapping_add(ib, 6))
            .clamp(0, bd_max),
          )
        });
        rep.record(
          &format!("mask16 bd={bitdepth} {w}x{h}"),
          bad_3.is_none(),
          || fmt_bad(bad_3),
        );

        for &(ss_hor, ss_ver) in &[(false, false), (true, false), (true, true)] {
          for &sign in &[0u8, 1] {
            let mask_w = match ::core::primitive::bool::then_some(ss_hor, ()) {
              ::core::option::Option::Some(()) => {
                ::core::primitive::usize::wrapping_shr(w, 1_u32)
              }
              ::core::option::Option::None => w,
            };
            let mask_h = match ::core::primitive::bool::then_some(ss_ver, ()) {
              ::core::option::Option::Some(()) => {
                ::core::primitive::usize::wrapping_shr(h, 1_u32)
              }
              ::core::option::Option::None => h,
            };
            let mut seg = vec![0u8; mask_w * mask_h];
            let mut got_4 = vec![0u16; h * stride];
            let Ok(width) = console_core_number_conversion::fixed_index(w);
            let Ok(height) = console_core_number_conversion::fixed_index(h);
            let Ok(pitch) = console_core_number_conversion::fixed_index(stride);
            let Ok(maximum) = console_core_number_conversion::wrapped(bd_max);
            let layout = match (ss_hor, ss_ver) {
              (true, true) => Rav1dPixelLayoutSubSampled::I420,
              (true, false) => Rav1dPixelLayoutSubSampled::I422,
              (false, _) => Rav1dPixelLayoutSubSampled::I444,
            };
            let mode = crate::src::safe_simd::mc::WeightedMaskMode {
              layout,
              sign,
              depth: crate::include::common::bitdepth::BitDepth16::new(maximum),
            };
            assert_eq!(
              word_mask_case(
                &mut got_4,
                [&t1, &t2],
                console_core_geometry::Size { width, height },
                pitch,
                &mut seg,
                mode,
              ),
              Ok(())
            );

            let bad_4 = find_bad(&got_4, self::FindBadArguments { stride, w, h }, |x, y| {
              let (a, b) = (t1[y * w + x], t2[y * w + x]);
              let mm = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
                w_mask_m_oracle(self::WMaskMOracleArguments { t1: a, t2: b }, bitdepth),
              );
              ::core::primitive::i32::cast_unsigned(
                ((<i32 as ::core::convert::From<::core::primitive::i16>>::from(a) * mm
                  + <i32 as ::core::convert::From<::core::primitive::i16>>::from(b) * (64 - mm)
                  + (32 << ib)
                  + bias * 64)
                  >> ::core::primitive::i32::wrapping_add(ib, 6))
                .clamp(0, bd_max),
              )
            });
            rep.record(
              &format!("w_mask16 px bd={bitdepth} sign={sign} ss=({ss_hor},{ss_ver}) {w}x{h}"),
              bad_4.is_none(),
              || fmt_bad(bad_4),
            );

            let mut want = vec![0u8; mask_w * mask_h];

            for y in 0..h {
              let mr: Vec<u8> = (0..w)
                .map(|x| {
                  w_mask_m_oracle(
                    self::WMaskMOracleArguments {
                      t1: t1[y * w + x],
                      t2: t2[y * w + x],
                    },
                    bitdepth,
                  )
                })
                .collect();
              let ::core::result::Result::Ok(my) = ::console_core_number_conversion::fixed_index(
                match ::core::primitive::bool::then_some(ss_ver, ()) {
                  ::core::option::Option::Some(()) => {
                    ::core::primitive::usize::wrapping_shr(y, 1_u32)
                  }
                  ::core::option::Option::None => y,
                },
              );
              let prev = want[(match ::console_core_number_conversion::index(my) {
                ::core::result::Result::Ok(native_position) => native_position,
              }) * mask_w..][..mask_w]
                .to_vec();
              let row = w_mask_store_oracle(&mr, &prev, y, sign, ss_hor, ss_ver);
              want[(match ::console_core_number_conversion::index(my) {
                ::core::result::Result::Ok(native_position) => native_position,
              }) * mask_w..][..mask_w]
                .copy_from_slice(&row);
            }

            let bad_seg = (0..mask_h)
              .flat_map(|y| (0..mask_w).map(move |x| (x, y)))
              .find(|&(x, y)| seg[y * mask_w + x] != want[y * mask_w + x])
              .map(|(x, y)| {
                (
                  x,
                  y,
                  <u32 as ::core::convert::From<::core::primitive::u8>>::from(
                    seg[y * mask_w + x],
                  ),
                  <u32 as ::core::convert::From<::core::primitive::u8>>::from(
                    want[y * mask_w + x],
                  ),
                )
              });
            rep.record(
              &format!("w_mask16 seg bd={bitdepth} sign={sign} ss=({ss_hor},{ss_ver}) {w}x{h}"),
              bad_seg.is_none(),
              || fmt_bad(bad_seg),
            );
          }
        }
      }
    }

    rep.finish("aarch64 16bpc compound values", 300);
  }

  #[doc = r###"The oracles' own check: their constants must come from `BitDepth`, not
from a literal copied out of the kernel under test.

If this fails alongside the kernel tests, suspect the oracle. If only
the kernel tests fail, the kernel is what moved."###]
  #[test]
  fn oracle_constants_come_from_the_bitdepth_not_a_literal() {
    assert_eq!(BitDepth8::new(()).get_intermediate_bits(), 4);
    assert_eq!(BitDepth16::new(1023).get_intermediate_bits(), 4);
    assert_eq!(
      BitDepth16::new(4095).get_intermediate_bits(),
      2,
      "12-bit intermediate_bits is 2, not 4 — a kernel that writes a \
             literal 4 is correct at 8 and 10 bits and wrong here"
    );
    assert_eq!(intermediate_bits(8), 4);
    assert_eq!(intermediate_bits(10), 4);
    assert_eq!(intermediate_bits(12), 2);
    assert_eq!(i32::from(BitDepth8::PREP_BIAS), 0);
    assert_eq!(i32::from(BitDepth16::PREP_BIAS), 8192);

    assert_eq!(8 + intermediate_bits(8) - 4, 8);
    assert_eq!(10 + intermediate_bits(10) - 4, 10);
    assert_eq!(12 + intermediate_bits(12) - 4, 10);
  }
}
