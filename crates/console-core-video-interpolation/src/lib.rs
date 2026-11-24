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

use console_core_number_conversion::{checked_index, fitted, fixed_index, index};
use console_core_simd::Simd;

pub use console_core_simd::FilterTaps;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TapSpanOutsideSource;

#[doc = "Filters a row of samples in 0..=4095. The first tap for each successive output starts one sample farther into `source`. `taps.step` is the distance between taps. A short input leaves the output untouched."]
#[doc = "Use a shift of 6 or 7 and supply the rounding bias and clip limit for the codec's filter. Returns an error if any tap would be out of bounds."]
#[inline]
pub fn filter_8tap_u16_row<const SHIFT: i32>(
  simd: Simd,
  source: &[u16],
  coefficients: &[i16; 8],
  taps: FilterTaps,
  output: &mut [u16],
) -> Result<(), TapSpanOutsideSource> {
  match output {
    [] => return Ok(()),
    [_, ..] => {}
  }

  let Ok(width) = fixed_index(output.len());
  let span = taps
    .step
    .checked_mul(7)
    .and_then(|reach| reach.checked_add(width))
    .ok_or(TapSpanOutsideSource)?;
  let end = checked_index(span).map_err(|_| TapSpanOutsideSource)?;
  let readable = source.get(..end).ok_or(TapSpanOutsideSource)?;
  let Ok(written) = simd.filter_8tap_u16::<SHIFT>(readable, coefficients, taps, output);
  let Ok(start) = index(written);
  let Ok(shift) = fitted::<_, u32>(SHIFT);

  for (column, sample) in output.iter_mut().enumerate().skip(start) {
    let Ok(x) = fixed_index(column);
    let mut sum = 0_i32;

    for (tap, &weight) in (0u64..).zip(coefficients) {
      let Ok(position) = index(x.wrapping_add(tap.wrapping_mul(taps.step)));
      let pixel = readable
        .get(position)
        .copied()
        .ok_or(TapSpanOutsideSource)?;
      sum = sum.wrapping_add(i32::from(weight).wrapping_mul(i32::from(pixel)));
    }

    let clipped = sum
      .wrapping_add(i32::from(taps.bias))
      .wrapping_shr(shift)
      .clamp(0, i32::from(taps.maximum));
    let Ok(value) = fitted(clipped);
    *sample = value;
  }

  Ok(())
}

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
  use super::*;

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

  #[test]
  fn eight_tap_rows_match_scalar_for_both_codec_precisions() -> Result<(), TapSpanOutsideSource> {
    compare_rows::<6>([34, 1023])?;
    compare_rows::<6>([40, 4095])?;
    compare_rows::<7>([64, 4095])
  }

  fn compare_rows<const SHIFT: i32>(
    [bias, maximum]: [u16; 2],
  ) -> Result<(), TapSpanOutsideSource> {
    let source: [u16; 578] = core::array::from_fn(|position| {
      let Ok(i) = fixed_index(position);
      let value = i
        .wrapping_mul(137)
        .wrapping_add(i.wrapping_div(7).wrapping_mul(31));
      let Ok(pixel) = fitted(value & 4095);
      pixel
    });
    let Ok(shift) = fitted::<_, u32>(SHIFT);

    for step in [1_u64, 73] {
      let Ok(stride) = index(step);

      for width in [4_u64, 8, 9, 16, 23, 32, 63, 65] {
        let length = step.wrapping_mul(7).wrapping_add(width);
        let Ok(end) = index(length.wrapping_add(1));
        let row = source.get(1..end).ok_or(TapSpanOutsideSource)?;
        let Ok(columns) = index(width);

        for taps in [
          [-2, 6, -15, 95, 54, -10, 4, -4],
          [0, 0, 0, 128, 0, 0, 0, 0],
          [i16::MIN; 8],
          [i16::MAX; 8],
        ] {
          let mut expected = [4321; 65];

          for (x, target) in expected.iter_mut().take(columns).enumerate() {
            let samples = row.iter().skip(x).step_by(stride.max(1));
            let sum = taps
              .iter()
              .zip(samples)
              .fold(i32::from(bias), |sum, (&tap, &sample)| {
                sum.wrapping_add(i32::from(tap).wrapping_mul(i32::from(sample)))
              });
            let clipped = sum.wrapping_shr(shift).clamp(0, i32::from(maximum));
            let Ok(pixel) = fitted(clipped);
            *target = pixel;
          }

          let Ok(available) = Simd::available();

          for simd in available {
            let mut actual = [4321; 65];
            let destination = actual.get_mut(..columns).ok_or(TapSpanOutsideSource)?;
            filter_8tap_u16_row::<SHIFT>(
              simd,
              row,
              &taps,
              FilterTaps {
                step,
                bias,
                maximum,
              },
              destination,
            )?;
            assert_eq!(
              actual, expected,
              "{simd:?}, step {step}, width {width}, taps {taps:?}"
            );
          }
        }
      }
    }

    Ok(())
  }

  #[test]
  fn eight_tap_rows_validate_the_full_tap_span_before_writing() {
    let mut output = [42; 17];
    let Ok(simd) = Simd::detected();

    for step in [8, u64::MAX] {
      assert_eq!(
        filter_8tap_u16_row::<7>(
          simd,
          &[1023; 72],
          &[16; 8],
          FilterTaps {
            step,
            bias: 64,
            maximum: 1023
          },
          &mut output
        ),
        Err(TapSpanOutsideSource)
      );
      assert_eq!(output, [42; 17]);
    }
  }
}
