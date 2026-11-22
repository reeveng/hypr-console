//! Fixed-point arithmetic for video subpixel filters.
//!
//! The caller supplies the coefficients and maps taps to sample positions.
//! Each codec chooses the precision, rounding points and output sample range.
//! [`convolve8`] accumulates in signed 32-bit arithmetic with wrapping and
//! calls the sample provider in order from the first tap to the last.
//! [`round_shift`] uses wrapping arithmetic for both the rounding bias and
//! the shift. A shift of zero leaves the input unchanged.

#![no_std]
#![forbid(unsafe_code)]

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "AV1 and VP9 use this scalar convolution in their sample kernels. The primitive return type preserves the shared arithmetic API on every architecture"
  )
)]
#[inline]
#[must_use]
pub fn convolve8<C: Copy + Into<i32>>(
  coefficients: &[C; 8],
  mut sample_at: impl FnMut(::console_core_api_compatibility::NativeSize) -> i32,
) -> i32 {
  let mut sum = 0_i32;

  for (tap, coefficient) in coefficients.iter().copied().enumerate() {
    let weight = <C as Into<i32>>::into(coefficient);
    let sample = sample_at(tap);
    sum = sum.wrapping_add(weight.wrapping_mul(sample));
  }

  sum
}

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the rounding operation is a const scalar primitive used directly by existing AV1 and VP9 arithmetic kernels"
  )
)]
#[inline]
#[must_use]
pub const fn round_shift(value: i32, shift: u32) -> i32 {
  let bias = 1_i32.wrapping_shl(shift).wrapping_shr(1);
  value.wrapping_add(bias).wrapping_shr(shift)
}

#[cfg(test)]
mod tests {
  use super::{convolve8, round_shift};

  #[test]
  fn convolution_keeps_signed_taps_and_sample_order() {
    let coefficients = [-2_i8, 4, -8, 16, -32, 63, -5, 28];
    let taps = core::cell::Cell::new(0_i32);
    let sum = convolve8(&coefficients, |tap| {
      let sample = taps.get();

      assert_eq!(i32::try_from(tap), Ok(sample));
      taps.set(sample.wrapping_add(1));

      sample
    });

    assert_eq!(taps.into_inner(), 8);
    assert_eq!(sum, 389);
    assert_eq!(convolve8(&[128_i16; 8], |_| 4095), 4_193_280);
  }

  #[test]
  fn rounding_preserves_signed_intermediates() {
    assert_eq!(round_shift(-65, 7), -1);
    assert_eq!(round_shift(-64, 7), 0);
    assert_eq!(round_shift(63, 7), 0);
    assert_eq!(round_shift(64, 7), 1);
    assert_eq!(round_shift(i32::MIN, 0), i32::MIN);
  }
}
