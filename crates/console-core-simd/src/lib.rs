//! Dispatch for SIMD operations. Each kernel writes a whole vector prefix and
//! returns its length so callers can finish with their scalar implementation.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

use core::convert::Infallible;

mod floats;
pub use floats::{AffineGain, AffineOffset, ComponentTransform, FusedF32, WaveletLift};

#[derive(Debug, Clone, Copy)]
enum Backend {
  Scalar,
  #[cfg(all(feature = "std", target_arch = "x86_64"))]
  Avx2(console_core_simd_x86::Avx2),
  #[cfg(all(feature = "std", target_arch = "x86_64"))]
  Avx512(console_core_simd_x86::Avx512),
  #[cfg(all(feature = "std", target_arch = "aarch64"))]
  Neon(console_core_simd_aarch64::Neon),
}

#[derive(Debug, Clone, Copy)]
pub struct Simd(Backend);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RgbaCompositing {
  Applied,
  Unavailable,
}

#[derive(Clone, Copy)]
pub struct FilterTaps {
  pub step: u64,
  pub bias: u16,
  pub maximum: u16,
}

impl Simd {
  pub const SCALAR: Self = Self(Backend::Scalar);

  pub fn detected() -> Result<Self, Infallible> {
    let Ok(available) = Self::available();
    Ok(match available.last() {
      Some(backend) => backend,
      None => Self::SCALAR,
    })
  }

  #[cfg(all(feature = "std", target_arch = "x86_64"))]
  pub fn avx2() -> Result<Option<Self>, Infallible> {
    let Ok(capability) = console_core_simd_x86::Avx2::detect();
    Ok(capability.map(|token| Self(Backend::Avx2(token))))
  }

  #[cfg(all(feature = "std", target_arch = "x86_64"))]
  pub fn avx512() -> Result<Option<Self>, Infallible> {
    let Ok(capability) = console_core_simd_x86::Avx512::detect();
    Ok(capability.map(|token| Self(Backend::Avx512(token))))
  }

  #[doc = "Backends supported by this process, starting with the scalar fallback."]
  pub fn available() -> Result<impl Iterator<Item = Self>, Infallible> {
    let scalar = core::iter::once(Self::SCALAR);

    #[cfg(all(feature = "std", target_arch = "x86_64"))]
    {
      let Ok(avx2) = Self::avx2();
      let Ok(avx512) = Self::avx512();
      Ok(scalar.chain(avx2).chain(avx512))
    }

    #[cfg(all(feature = "std", target_arch = "aarch64"))]
    {
      let Ok(neon) = console_core_simd_aarch64::Neon::detect();
      Ok(scalar.chain(neon.map(|token| Self(Backend::Neon(token)))))
    }

    #[cfg(not(all(feature = "std", any(target_arch = "x86_64", target_arch = "aarch64"))))]
    {
      Ok(scalar)
    }
  }

  #[doc = "Composites sixteen pixels from planar u16 RGBA lanes onto packed RGBA. Preserves the raster pipeline's wrapping arithmetic and its `(destination * (255 - alpha) + 255) >> 8` rounding rule. Returns Unavailable and leaves the destination untouched on scalar backends."]
  #[inline]
  pub fn source_over_rgba16(
    self,
    source: [&[u16; 16]; 4],
    pixels: &mut [[u8; 4]; 16],
  ) -> Result<RgbaCompositing, Infallible> {
    #[cfg(not(all(feature = "std", any(target_arch = "x86_64", target_arch = "aarch64"))))]
    let _ = (source, pixels);

    Ok(match self.0 {
      Backend::Scalar => RgbaCompositing::Unavailable,
      #[cfg(all(feature = "std", target_arch = "x86_64"))]
      Backend::Avx2(cpu) => {
        let Ok(()) = cpu.source_over_rgba16(source, pixels);
        RgbaCompositing::Applied
      }
      #[cfg(all(feature = "std", target_arch = "x86_64"))]
      Backend::Avx512(cpu) => {
        let Ok(()) = cpu.source_over_rgba16(source, pixels);
        RgbaCompositing::Applied
      }
      #[cfg(all(feature = "std", target_arch = "aarch64"))]
      Backend::Neon(cpu) => {
        let Ok(()) = cpu.source_over_rgba16(source, pixels);
        RgbaCompositing::Applied
      }
    })
  }

  #[doc = "Writes Q8 channels from adjacent pairs of four-channel byte pixels. Each offset starts a two-pixel window in `source`. Its second weight is in 0..=256. The first pixel has weight 256 minus that value. Returns the number of output pixels written. Processing stops before the first vector that uses an offset without two pixels in `source`."]
  pub fn weighted_pairs<const SWAP_RED_BLUE: bool>(
    self,
    source: &[[u8; 4]],
    offsets: &[u32],
    weights: &[u16],
    output: &mut [[u16; 4]],
  ) -> Result<u64, Infallible> {
    #[cfg(not(all(feature = "std", any(target_arch = "x86_64", target_arch = "aarch64"))))]
    let _ = (source, offsets, weights, output);

    match self.0 {
      Backend::Scalar => Ok(0),
      #[cfg(all(feature = "std", target_arch = "x86_64"))]
      Backend::Avx2(cpu) => cpu.weighted_pairs::<SWAP_RED_BLUE>(source, offsets, weights, output),
      #[cfg(all(feature = "std", target_arch = "x86_64"))]
      Backend::Avx512(cpu) => {
        cpu.weighted_pairs::<SWAP_RED_BLUE>(source, offsets, weights, output)
      }
      #[cfg(all(feature = "std", target_arch = "aarch64"))]
      Backend::Neon(cpu) => cpu.weighted_pairs::<SWAP_RED_BLUE>(source, offsets, weights, output),
    }
  }

  #[doc = "Blends two unsigned fixed-point rows and rounds to bytes. The weight for the bottom row is in 0..=256 and the top weight is its complement to 256. `SHIFT` is 12 for Q4 inputs (0..=4080) or 16 for Q8 inputs (0..=65280). Returns the number of channels written."]
  #[inline]
  pub fn blend_rows<const SHIFT: i32>(
    self,
    top: &[u16],
    bottom: &[u16],
    weight: u16,
    output: &mut [u8],
  ) -> Result<u64, Infallible> {
    const { assert!(SHIFT == 12 || SHIFT == 16) };

    #[cfg(not(all(feature = "std", any(target_arch = "x86_64", target_arch = "aarch64"))))]
    let _ = (top, bottom, weight, output);

    match self.0 {
      Backend::Scalar => Ok(0),
      #[cfg(all(feature = "std", target_arch = "x86_64"))]
      Backend::Avx2(cpu) => cpu.blend_rows::<SHIFT>(top, bottom, weight, output),
      #[cfg(all(feature = "std", target_arch = "x86_64"))]
      Backend::Avx512(cpu) => cpu.blend_rows::<SHIFT>(top, bottom, weight, output),
      #[cfg(all(feature = "std", target_arch = "aarch64"))]
      Backend::Neon(cpu) => cpu.blend_rows::<SHIFT>(top, bottom, weight, output),
    }
  }

  #[doc = "Applies eight signed taps to samples in 0..=4095, with tap `k` for output `x` at `source[x + k * tap_step]`. Each sum is biased by `rounding`, shifted right by `SHIFT`, and clamped to 0..=`maximum`. Returns the number of outputs written as complete vectors."]
  #[inline]
  pub fn filter_8tap_u16<const SHIFT: i32>(
    self,
    source: &[u16],
    coefficients: &[i16; 8],
    taps: FilterTaps,
    output: &mut [u16],
  ) -> Result<u64, Infallible> {
    const { assert!(SHIFT == 6 || SHIFT == 7) };

    #[cfg(not(all(feature = "std", any(target_arch = "x86_64", target_arch = "aarch64"))))]
    let _ = (source, coefficients, taps, output);

    match self.0 {
      Backend::Scalar => Ok(0),
      #[cfg(all(feature = "std", target_arch = "x86_64"))]
      Backend::Avx2(cpu) => cpu.filter_8tap_u16::<SHIFT>(
        source,
        coefficients,
        console_core_simd_x86::FilterTaps {
          step: taps.step,
          bias: taps.bias,
          maximum: taps.maximum,
        },
        output,
      ),
      #[cfg(all(feature = "std", target_arch = "x86_64"))]
      Backend::Avx512(cpu) => cpu.filter_8tap_u16::<SHIFT>(
        source,
        coefficients,
        console_core_simd_x86::FilterTaps {
          step: taps.step,
          bias: taps.bias,
          maximum: taps.maximum,
        },
        output,
      ),
      #[cfg(all(feature = "std", target_arch = "aarch64"))]
      Backend::Neon(cpu) => cpu.filter_8tap_u16::<SHIFT>(
        source,
        coefficients,
        console_core_simd_aarch64::FilterTaps {
          step: taps.step,
          bias: taps.bias,
          maximum: taps.maximum,
        },
        output,
      ),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{RgbaCompositing, Simd};
  use console_core_number_conversion::index;

  #[derive(Debug)]
  enum FixtureError {
    PairPrefix,
    MissingPair,
    ShortOutput,
    RowPrefix,
  }

  #[test]
  fn rgba_source_over_matches_the_rasterizer_rounding_and_wrap() {
    let mut source = [[0_u16; 16]; 4];

    for (plane, channel) in source.iter_mut().zip(0_u16..) {
      for (value, pixel) in plane.iter_mut().zip(0_u16..) {
        let seed = pixel
          .wrapping_mul(353)
          .wrapping_add(channel.wrapping_mul(147));
        *value = seed & 1023;
      }
    }

    let mut destination = [[0_u8; 4]; 16];

    for (pixel, position) in destination.iter_mut().zip(0_u8..) {
      for (value, channel) in pixel.iter_mut().zip(0_u8..) {
        *value = position
          .wrapping_mul(181)
          .wrapping_add(channel.wrapping_mul(83));
      }
    }

    let [red, green, blue, alpha] = &source;
    let channels = red.iter().zip(green).zip(blue.iter().zip(alpha));
    let mut expected = destination;

    for (pixel, ((&r, &g), (&b, &a))) in expected.iter_mut().zip(channels) {
      let inverse = 255_u16.wrapping_sub(a);

      for (value, channel) in pixel.iter_mut().zip([r, g, b, a]) {
        let product = u16::from(*value).wrapping_mul(inverse);
        let scaled = product.wrapping_add(255).wrapping_shr(8);
        let [low, _] = channel.wrapping_add(scaled).to_le_bytes();
        *value = low;
      }
    }

    let Ok(available) = Simd::available();

    for simd in available {
      let mut output = destination;

      let Ok(compositing) = simd.source_over_rgba16(source.each_ref(), &mut output);

      match compositing {
        RgbaCompositing::Applied => assert_eq!(output, expected, "{simd:?}"),
        RgbaCompositing::Unavailable => assert_eq!(output, destination),
      }
    }
  }

  #[test]
  fn pair_kernels_leave_the_tail_and_preserve_each_channel() -> Result<(), FixtureError> {
    let input = [[1, 20, 30, 4], [56, 70, 80, 9], [100, 110, 120, 130]];
    let offsets = [0, 1, 0, 1, 1, 0, 1];
    let weights = [0, 256, 128, 13, 233, 17, 111];

    let Ok(available) = Simd::available();

    for simd in available {
      let mut output = [[42; 4]; 7];
      let Ok(written_count) =
        simd.weighted_pairs::<true>(&input, &offsets, &weights, &mut output);
      let Ok(count) = index(written_count);
      let (written, tail) = output
        .split_at_checked(count)
        .ok_or(FixtureError::PairPrefix)?;

      for ((&offset, &weight), pixel) in offsets.iter().zip(&weights).zip(written) {
        let [first, second, third] = input;
        let [[ar, ag, ab, aa], [br, bg, bb, ba]] = match offset {
          0 => [first, second],
          1 => [second, third],
          _ => return Err(FixtureError::MissingPair),
        };
        let mix = |a: u8, b: u8| {
          let left = u16::from(a).wrapping_mul(256_u16.wrapping_sub(weight));
          let right = u16::from(b).wrapping_mul(weight);
          left.wrapping_add(right)
        };
        let expected = [mix(ab, bb), mix(ag, bg), mix(ar, br), mix(aa, ba)];
        assert_eq!(*pixel, expected, "{simd:?}");
      }

      assert!(tail.iter().all(|pixel| *pixel == [42; 4]));
    }

    Ok(())
  }

  #[test]
  fn pair_kernels_stop_before_a_vector_with_an_out_of_bounds_offset() {
    let input = [[1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12]];
    let valid_offsets = [0, 1, 0, 1, 0, 1];
    let offsets_with_invalid_pair = [0, 1, 0, 1, 0, 1, 2, 0];
    let weights = [128; 8];

    let Ok(available) = Simd::available();

    for simd in available {
      let mut expected = [[42; 4]; 8];
      let mut output = expected;
      assert_eq!(
        simd.weighted_pairs::<false>(&input, &offsets_with_invalid_pair, &weights, &mut output),
        simd.weighted_pairs::<false>(&input, &valid_offsets, &weights, &mut expected),
      );
      assert_eq!(output, expected, "{simd:?}");
    }
  }

  #[test]
  fn row_kernels_round_all_weights_like_the_scalar_expression() -> Result<(), FixtureError> {
    check_row_kernels::<16>(65280)?;
    check_row_kernels::<12>(4080)
  }

  fn check_row_kernels<const SHIFT: i32>(maximum: u16) -> Result<(), FixtureError> {
    let mut top = [0; 67];
    let mut bottom = [0; 67];

    for ((a, b), lane) in top.iter_mut().zip(&mut bottom).zip(0_u16..) {
      *a = lane.wrapping_mul(maximum.wrapping_div(66));
      *b = maximum.wrapping_sub(lane.wrapping_mul(maximum.wrapping_div(73)));
    }

    let shift = SHIFT.cast_unsigned();
    let rounding = 1_u32.wrapping_shl(shift.wrapping_sub(1));

    let Ok(available) = Simd::available();

    for simd in available {
      for len in [0, 4, 8, 13, 16, 23, 32, 67] {
        for weight in 0..=256 {
          let mut output = [42; 67];
          let requested = output.get_mut(..len).ok_or(FixtureError::ShortOutput)?;
          let Ok(written_count) = simd.blend_rows::<SHIFT>(&top, &bottom, weight, requested);
          let Ok(count) = index(written_count);
          let (written, _) = requested
            .split_at_checked(count)
            .ok_or(FixtureError::RowPrefix)?;

          for ((&a, &b), &actual) in top.iter().zip(&bottom).zip(written) {
            let left = u32::from(a).wrapping_mul(u32::from(256_u16.wrapping_sub(weight)));
            let right = u32::from(b).wrapping_mul(u32::from(weight));
            let sum = left.wrapping_add(right);
            let expected = sum.wrapping_add(rounding).wrapping_shr(shift);
            assert_eq!(u32::from(actual), expected, "{simd:?} weight {weight}");
          }

          assert!(output.iter().skip(count).all(|&value| value == 42));
        }
      }
    }

    Ok(())
  }
}
