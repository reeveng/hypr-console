//! AArch64 SIMD kernels. The capability token guards the NEON entry points.

#![cfg(target_arch = "aarch64")]

use console_core_number_conversion::{fixed_index, index};
use core::arch::aarch64::*;
use core::convert::Infallible;

mod floats;

#[derive(Clone, Copy)]
pub struct FilterTaps {
  pub step: u64,
  pub bias: u16,
  pub maximum: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct Neon(());

impl Neon {
  pub fn detect() -> Result<Option<Self>, Infallible> {
    Ok(std::arch::is_aarch64_feature_detected!("neon").then_some(Self(())))
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "Neon::detect is the only token constructor and checks the NEON target feature before this call"
    )
  )]
  pub fn source_over_rgba16(
    self,
    source: [&[u16; 16]; 4],
    pixels: &mut [[u8; 4]; 16],
  ) -> Result<(), Infallible> {
    // SAFETY: This token guarantees NEON.
    unsafe { source_over_rgba16(source, pixels) }
  }

  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "Neon::detect is the only token constructor and checks the NEON target feature before this call"
    )
  )]
  pub fn filter_8tap_u16<const SHIFT: i32>(
    self,
    source: &[u16],
    coefficients: &[i16; 8],
    taps: FilterTaps,
    output: &mut [u16],
  ) -> Result<u64, Infallible> {
    // SAFETY: This token guarantees NEON.
    unsafe { filter_8tap_neon::<SHIFT>(source, coefficients, taps, output) }
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "Neon::detect is the only token constructor and checks the NEON target feature before this call"
    )
  )]
  pub fn weighted_pairs<const SWAP: bool>(
    self,
    source: &[[u8; 4]],
    offsets: &[u32],
    weights: &[u16],
    output: &mut [[u16; 4]],
  ) -> Result<u64, Infallible> {
    // SAFETY: Only runtime detection can construct this token.
    unsafe { pairs_neon::<SWAP>(source, offsets, weights, output) }
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "Neon::detect is the only token constructor and checks the NEON target feature before this call"
    )
  )]
  pub fn blend_rows<const SHIFT: i32>(
    self,
    top: &[u16],
    bottom: &[u16],
    weight: u16,
    output: &mut [u8],
  ) -> Result<u64, Infallible> {
    // SAFETY: Only runtime detection can construct this token.
    unsafe { rows_neon::<SHIFT>(top, bottom, weight, output) }
  }
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "Each source and target comes from a full eight-pixel chunk, so the NEON loads and stores fit their arrays"
  )
)]
#[target_feature(enable = "neon")]
fn source_over_rgba16(
  source: [&[u16; 16]; 4],
  pixels: &mut [[u8; 4]; 16],
) -> Result<(), Infallible> {
  let byte = vdupq_n_u16(255);
  let [red, green, blue, alpha] = source.map(|plane| plane.as_chunks::<8>().0);

  for ((target, (red_chunk, green_chunk)), (blue_chunk, alpha_chunk)) in pixels
    .as_chunks_mut::<8>()
    .0
    .iter_mut()
    .zip(red.iter().zip(green))
    .zip(blue.iter().zip(alpha))
  {
    let [r, g, b, a] = [red_chunk, green_chunk, blue_chunk, alpha_chunk].map(|lanes| {
      // SAFETY: Each source chunk holds eight u16 lanes.
      unsafe { vld1q_u16(lanes.as_ptr()) }
    });
    // SAFETY: Eight RGBA pixels have 32 readable bytes.
    let uint8x8x4_t(dr, dg, db, da) = unsafe { vld4_u8(target.as_ptr().cast()) };
    let inverse = vsubq_u16(byte, a);
    let blend = |src, dst| {
      let scaled = vshrq_n_u16::<8>(vaddq_u16(vmulq_u16(vmovl_u8(dst), inverse), byte));
      vmovn_u16(vaddq_u16(src, scaled))
    };
    let out = uint8x8x4_t(blend(r, dr), blend(g, dg), blend(b, db), blend(a, da));

    // SAFETY: The same eight-pixel group has space for all 32 bytes.
    unsafe { vst4_u8(target.as_mut_ptr().cast(), out) };
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "The checked source and destination lengths bound every complete tap span and each partial-vector store"
  )
)]
#[target_feature(enable = "neon")]
fn filter_8tap_neon<const SHIFT: i32>(
  source: &[u16],
  coefficients: &[i16; 8],
  taps: FilterTaps,
  output: &mut [u16],
) -> Result<u64, Infallible> {
  let Ok(source_count) = fixed_index(source.len());
  let Ok(output_count) = fixed_index(output.len());
  let available = source_count
    .saturating_sub(taps.step.saturating_mul(7))
    .min(output_count);
  let count = available & !7;
  let bias = vdupq_n_s32(i32::from(taps.bias));
  let maximum = vdupq_n_u16(taps.maximum);

  for (column, target) in (0..count).step_by(8).zip(output.chunks_exact_mut(8)) {
    let mut lo = bias;
    let mut hi = bias;

    for (tap, &weight) in (0_u64..).zip(coefficients) {
      let Ok(at) = index(column.wrapping_add(tap.wrapping_mul(taps.step)));
      // SAFETY: The tap span and eight samples fit at every column.
      let unsigned = unsafe { vld1q_u16(source.as_ptr().add(at)) };
      let samples = vreinterpretq_s16_u16(unsigned);
      lo = vmlal_n_s16(lo, vget_low_s16(samples), weight);
      hi = vmlal_high_n_s16(hi, samples, weight);
    }

    let lower = vqmovun_s32(vshrq_n_s32::<SHIFT>(lo));
    let upper = vqmovun_s32(vshrq_n_s32::<SHIFT>(hi));
    let samples = vminq_u16(vcombine_u16(lower, upper), maximum);

    // SAFETY: The destination holds eight samples.
    unsafe { vst1q_u16(target.as_mut_ptr(), samples) };
  }

  match available.wrapping_sub(count) {
    4.. => {
      let mut sum = bias;

      for (tap, &weight) in (0_u64..).zip(coefficients) {
        let Ok(at) = index(count.wrapping_add(tap.wrapping_mul(taps.step)));
        // SAFETY: At least four samples remain in every tap.
        let samples = unsafe { vld1_u16(source.as_ptr().add(at)) };
        sum = vmlal_n_s16(sum, vreinterpret_s16_u16(samples), weight);
      }

      let samples = vmin_u16(
        vqmovun_s32(vshrq_n_s32::<SHIFT>(sum)),
        vget_low_u16(maximum),
      );
      let Ok(at) = index(count);

      // SAFETY: The destination has four remaining samples.
      unsafe { vst1_u16(output.as_mut_ptr().add(at), samples) };

      Ok(count.wrapping_add(4))
    }
    _ => Ok(count),
  }
}

#[target_feature(enable = "neon")]
fn weighted_pair(pixels: uint16x8_t, weight: u16) -> Result<uint16x4_t, Infallible> {
  let coefficients = vcombine_u16(vdup_n_u16(256_u16.wrapping_sub(weight)), vdup_n_u16(weight));
  let product = vmulq_u16(pixels, coefficients);
  Ok(vadd_u16(vget_low_u16(product), vget_high_u16(product)))
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "Checked adjacent-pixel windows each supply eight source bytes and each target group has two four-channel u16 pixels"
  )
)]
#[target_feature(enable = "neon")]
fn pairs_neon<const SWAP: bool>(
  source: &[[u8; 4]],
  offsets: &[u32],
  weights: &[u16],
  output: &mut [[u16; 4]],
) -> Result<u64, Infallible> {
  let first_indices = u64::from_le_bytes([2, 1, 0, 3, 6, 5, 4, 7]);
  let second_indices = u64::from_le_bytes([10, 9, 8, 11, 14, 13, 12, 15]);
  let order = vcombine_u8(vcreate_u8(first_indices), vcreate_u8(second_indices));
  let mut written = 0_u64;

  for ((&[x0, x1], &[w0, w1]), target) in offsets
    .as_chunks::<2>()
    .0
    .iter()
    .zip(weights.as_chunks::<2>().0)
    .zip(output.as_chunks_mut::<2>().0)
  {
    let (first, second) = match (adjacent(source, x0), adjacent(source, x1)) {
      (Ok(first), Ok(second)) => (first, second),
      _ => break,
    };

    // SAFETY: A pair holds eight contiguous bytes, with no alignment requirement.
    let pixels = unsafe {
      vcombine_u8(
        vld1_u8(first.as_ptr().cast()),
        vld1_u8(second.as_ptr().cast()),
      )
    };
    let ordered = match SWAP.then_some(()) {
      Some(()) => vqtbl1q_u8(pixels, order),
      None => pixels,
    };
    let Ok(a) = weighted_pair(vmovl_u8(vget_low_u8(ordered)), w0);
    let Ok(b) = weighted_pair(vmovl_high_u8(ordered), w1);

    // SAFETY: Two four-channel u16 pixels cover the entire 16-byte store.
    unsafe { vst1q_u16(target.as_mut_ptr().cast(), vcombine_u16(a, b)) };

    written = written.wrapping_add(2);
  }

  Ok(written)
}

struct PairOutsideSource;

#[inline]
fn adjacent(source: &[[u8; 4]], at: u32) -> Result<&[[u8; 4]; 2], PairOutsideSource> {
  let Ok(start) = index(at);
  source
    .get(start..)
    .and_then(<[[u8; 4]]>::first_chunk::<2>)
    .ok_or(PairOutsideSource)
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "Zipped complete input and output chunks bound every NEON vector load and store"
  )
)]
#[target_feature(enable = "neon")]
fn rows_neon<const SHIFT: i32>(
  top: &[u16],
  bottom: &[u16],
  weight: u16,
  output: &mut [u8],
) -> Result<u64, Infallible> {
  let first = vdup_n_u16(256_u16.wrapping_sub(weight));
  let second = vdup_n_u16(weight);
  let groups = top
    .as_chunks::<8>()
    .0
    .iter()
    .zip(bottom.as_chunks::<8>().0)
    .zip(output.as_chunks_mut::<8>().0);
  let Ok(group_count) = fixed_index(groups.len());
  let count = group_count.wrapping_mul(8);

  for ((a, b), target) in groups {
    // SAFETY: Both sources hold a full vector of eight u16 channels.
    let (upper, lower) = unsafe { (vld1q_u16(a.as_ptr()), vld1q_u16(b.as_ptr())) };
    let lo = vmlal_u16(
      vmull_u16(vget_low_u16(upper), first),
      vget_low_u16(lower),
      second,
    );
    let hi = vmlal_u16(
      vmull_u16(vget_high_u16(upper), first),
      vget_high_u16(lower),
      second,
    );
    let channels = vcombine_u16(vrshrn_n_u32::<SHIFT>(lo), vrshrn_n_u32::<SHIFT>(hi));

    // SAFETY: The destination is a complete eight-byte chunk.
    unsafe { vst1_u8(target.as_mut_ptr(), vmovn_u16(channels)) };
  }

  Ok(count)
}
