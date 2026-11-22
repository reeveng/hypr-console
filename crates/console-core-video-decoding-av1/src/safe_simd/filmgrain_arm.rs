//! Fixed-size NEON film-grain application kernels.

#![forbid(unsafe_code)]

use archmage::intrinsics::aarch64 as neon;
use archmage::{Arm64, arcane};
use core::arch::aarch64::*;
use console_core_never::Never;

pub(crate) struct GrainNeon8<'a> {
  pub source: &'a [u8; 16],
  pub destination: &'a mut [u8; 16],
  pub noise: &'a [i8; 16],
  pub scaling: [i16; 16],
  pub minimum: u8,
  pub maximum: u8,
  pub shift: u8,
}

#[arcane]
pub(crate) fn apply_grain8(_token: Arm64, block: GrainNeon8<'_>) -> Result<(), Never> {
  let pixels = neon::vld1q_u8(block.source);
  let samples = neon::vld1q_s8(block.noise);
  let halves: &[[i16; 8]; 2] = zerocopy::transmute_ref!(&block.scaling);
  let [low_scale, high_scale] = halves;
  let low_product = vmulq_s16(neon::vld1q_s16(low_scale), vmovl_s8(vget_low_s8(samples)));
  let high_product = vmulq_s16(neon::vld1q_s16(high_scale), vmovl_high_s8(samples));
  let power = u32::from(15_u8.wrapping_sub(block.shift));
  let multiplier = vdupq_n_s16(1_i16.wrapping_shl(power));
  let low_noise = vqrdmulhq_s16(low_product, multiplier);
  let high_noise = vqrdmulhq_s16(high_product, multiplier);
  let low_pixels = vreinterpretq_s16_u16(vmovl_u8(vget_low_u8(pixels)));
  let high_pixels = vreinterpretq_s16_u16(vmovl_high_u8(pixels));
  let low = vqmovun_s16(vaddq_s16(low_pixels, low_noise));
  let high = vqmovun_s16(vaddq_s16(high_pixels, high_noise));
  let packed = vcombine_u8(low, high);
  let clipped = vminq_u8(vmaxq_u8(packed, vdupq_n_u8(block.minimum)), vdupq_n_u8(block.maximum));
  neon::vst1q_u8(block.destination, clipped);
  Ok(())
}

pub(crate) struct GrainNeon16<'a> {
  pub source: &'a [u16; 8],
  pub destination: &'a mut [u16; 8],
  pub noise: [i16; 8],
  pub minimum: i16,
  pub maximum: i16,
}

#[arcane]
pub(crate) fn apply_grain16(_token: Arm64, block: GrainNeon16<'_>) -> Result<(), Never> {
  let source = vreinterpretq_s16_u16(neon::vld1q_u16(block.source));
  let noise = neon::vld1q_s16(&block.noise);
  let changed = vaddq_s16(source, noise);
  let minimum = vdupq_n_s16(block.minimum);
  let maximum = vdupq_n_s16(block.maximum);
  let output = vreinterpretq_u16_s16(vminq_s16(vmaxq_s16(changed, minimum), maximum));
  neon::vst1q_u16(block.destination, output);
  Ok(())
}
