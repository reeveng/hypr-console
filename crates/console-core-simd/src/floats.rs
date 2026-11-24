use super::{Backend, Simd};
use core::convert::Infallible;

#[derive(Clone, Copy, Debug)]
enum FloatBackend {
  #[cfg(all(feature = "std", target_arch = "x86_64"))]
  X86(console_core_simd_x86::FusedF32),
  #[cfg(all(feature = "std", target_arch = "aarch64"))]
  Neon(console_core_simd_aarch64::Neon),
}

#[doc = "A vector backend with hardware fused multiply-add."]
#[derive(Clone, Copy, Debug)]
pub struct FusedF32(FloatBackend);

#[derive(Clone, Copy)]
pub struct AffineGain(u32);

impl From<f32> for AffineGain {
  fn from(value: f32) -> Self {
    Self(value.to_bits())
  }
}

#[derive(Clone, Copy)]
pub struct AffineOffset(u32);

impl From<f32> for AffineOffset {
  fn from(value: f32) -> Self {
    Self(value.to_bits())
  }
}

#[derive(Clone, Copy, Debug)]
pub enum WaveletLift {
  ReversibleUpdate,
  ReversiblePredict,
  Irreversible(f32),
}

#[derive(Clone, Copy, Debug)]
pub enum ComponentTransform {
  Reversible,
  Ycbcr {
    offset: f32,
    #[doc = "Multipliers for Cr to red, Cb to green, Cr to green, and Cb to blue."]
    gains: [f32; 4],
    maximum: Option<f32>,
  },
}

impl Simd {
  #[doc = "Selects the hardware backend for fused floating-point operations."]
  pub fn fused_f32(self) -> Result<Option<FusedF32>, Infallible> {
    Ok(match self.0 {
      Backend::Scalar => None,
      #[cfg(all(feature = "std", target_arch = "x86_64"))]
      Backend::Avx2(_) | Backend::Avx512(_) => {
        let Ok(capability) = console_core_simd_x86::FusedF32::detect();
        capability.map(|cpu| FusedF32(FloatBackend::X86(cpu)))
      }
      #[cfg(all(feature = "std", target_arch = "aarch64"))]
      Backend::Neon(cpu) => Some(FusedF32(FloatBackend::Neon(cpu))),
    })
  }
}

impl FusedF32 {
  #[doc = "Writes alternating samples from two rows and returns the pair count."]
  pub fn interleave(
    self,
    first: &[f32],
    second: &[f32],
    output: &mut [[f32; 2]],
  ) -> Result<u64, Infallible> {
    #[cfg(not(all(feature = "std", any(target_arch = "x86_64", target_arch = "aarch64"))))]
    let _ = (first, second, output);

    match self.0 {
      #[cfg(all(feature = "std", target_arch = "x86_64"))]
      FloatBackend::X86(cpu) => cpu.interleave(first, second, output),
      #[cfg(all(feature = "std", target_arch = "aarch64"))]
      FloatBackend::Neon(cpu) => cpu.interleave(first, second, output),
    }
  }

  #[doc = "Applies one inverse wavelet lifting step and returns the prefix length."]
  pub fn lift(
    self,
    step: WaveletLift,
    current: &mut [f32],
    before: &[f32],
    after: &[f32],
  ) -> Result<u64, Infallible> {
    match step {
      WaveletLift::ReversibleUpdate => self.lift_mode::<0>(current, before, after, 0),
      WaveletLift::ReversiblePredict => self.lift_mode::<1>(current, before, after, 0),
      WaveletLift::Irreversible(gain) => {
        self.lift_mode::<2>(current, before, after, gain.to_bits())
      }
    }
  }

  fn lift_mode<const MODE: u8>(
    self,
    current: &mut [f32],
    before: &[f32],
    after: &[f32],
    gain_bits: u32,
  ) -> Result<u64, Infallible> {
    #[cfg(not(all(feature = "std", any(target_arch = "x86_64", target_arch = "aarch64"))))]
    let _ = (current, before, after, gain_bits);

    match self.0 {
      #[cfg(all(feature = "std", target_arch = "x86_64"))]
      FloatBackend::X86(cpu) => cpu.lift::<MODE>(current, before, after, gain_bits),
      #[cfg(all(feature = "std", target_arch = "aarch64"))]
      FloatBackend::Neon(cpu) => cpu.lift::<MODE>(current, before, after, gain_bits),
    }
  }

  #[doc = "Applies fused gain and offset in place and returns the prefix length."]
  pub fn affine(
    self,
    samples: &mut [f32],
    AffineGain(gain): AffineGain,
    AffineOffset(offset): AffineOffset,
  ) -> Result<u64, Infallible> {
    #[cfg(not(all(feature = "std", any(target_arch = "x86_64", target_arch = "aarch64"))))]
    let _ = (samples, gain, offset);

    match self.0 {
      #[cfg(all(feature = "std", target_arch = "x86_64"))]
      FloatBackend::X86(cpu) => cpu.affine(samples, [gain, offset]),
      #[cfg(all(feature = "std", target_arch = "aarch64"))]
      FloatBackend::Neon(cpu) => cpu.affine(samples, [gain, offset]),
    }
  }

  #[doc = "Converts three sample planes to RGB in place and returns the prefix length."]
  pub fn color(
    self,
    transform: ComponentTransform,
    planes: [&mut [f32]; 3],
  ) -> Result<u64, Infallible> {
    match transform {
      ComponentTransform::Reversible => self.color_mode::<true>(planes, 0, [0.0; 4], None),
      ComponentTransform::Ycbcr {
        offset,
        gains,
        maximum,
      } => self.color_mode::<false>(planes, offset.to_bits(), gains, maximum),
    }
  }

  fn color_mode<const REVERSIBLE: bool>(
    self,
    planes: [&mut [f32]; 3],
    offset_bits: u32,
    gains: [f32; 4],
    maximum: Option<f32>,
  ) -> Result<u64, Infallible> {
    #[cfg(not(all(feature = "std", any(target_arch = "x86_64", target_arch = "aarch64"))))]
    let _ = (planes, offset_bits, gains, maximum);

    match self.0 {
      #[cfg(all(feature = "std", target_arch = "x86_64"))]
      FloatBackend::X86(cpu) => cpu.color::<REVERSIBLE>(planes, offset_bits, gains, maximum),
      #[cfg(all(feature = "std", target_arch = "aarch64"))]
      FloatBackend::Neon(cpu) => cpu.color::<REVERSIBLE>(planes, offset_bits, gains, maximum),
    }
  }
}

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::{ComponentTransform, Simd, WaveletLift};
  use console_core_number_conversion::index;

  #[derive(Debug)]
  enum FixtureError {
    InterleavePrefix,
    LiftPrefix,
  }

  #[test]
  fn wavelet_lifts_preserve_the_arithmetic_and_scalar_tail() -> Result<(), FixtureError> {
    let mut before = [0.0; 19];
    let mut after = [0.0; 19];

    for ((left, right), lane) in before.iter_mut().zip(&mut after).zip(0_u16..) {
      *left = f32::from(lane.wrapping_mul(37)) * 0.3 - 120.0;
      *right = f32::from(lane.wrapping_mul(61)) * 0.7 - 108.0;
    }

    let Ok(available) = Simd::available();

    for backend in available.filter_map(|simd| {
      let Ok(capability) = simd.fused_f32();
      capability
    }) {
      let mut pairs = [[0.0; 2]; 19];
      let Ok(interleaved) = backend.interleave(&before, &after, &mut pairs);
      assert!(interleaved >= 16);
      let Ok(length) = index(interleaved);
      let (written, tail) = pairs
        .split_at_checked(length)
        .ok_or(FixtureError::InterleavePrefix)?;

      for (&pair, (&left, &right)) in written.iter().zip(before.iter().zip(&after)) {
        assert_eq!(pair, [left, right]);
      }

      assert!(tail.iter().all(|pair| *pair == [0.0; 2]));

      for step in [
        WaveletLift::ReversibleUpdate,
        WaveletLift::ReversiblePredict,
        WaveletLift::Irreversible(-0.443_506_87),
      ] {
        let mut current = [35.2_f32; 19];
        let Ok(lifted) = backend.lift(step, &mut current, &before, &after);
        assert!(lifted >= 16);
        let Ok(lifted_length) = index(lifted);
        let (prefix, remainder) = current
          .split_at_checked(lifted_length)
          .ok_or(FixtureError::LiftPrefix)?;

        for (value, (&left, &right)) in prefix.iter().zip(before.iter().zip(&after)) {
          let sum = left + right;
          let expected = match step {
            WaveletLift::ReversibleUpdate => 35.2 - ((sum + 2.0) * 0.25).floor(),
            WaveletLift::ReversiblePredict => 35.2 + (sum * 0.5).floor(),
            WaveletLift::Irreversible(gain) => sum.mul_add(gain, 35.2),
          };
          assert_eq!(value.to_bits(), expected.to_bits());
        }

        assert!(
          remainder
            .iter()
            .all(|sample| sample.to_bits() == 35.2_f32.to_bits())
        );
      }
    }

    Ok(())
  }

  #[test]
  fn s_ycc_transform_uses_fused_math_and_clips_all_three_planes() {
    let luma = [119.25f32; 16];
    let mut blue = [0.0; 16];

    for (value, lane) in blue.iter_mut().zip(0_u16..) {
      *value = f32::from(lane.wrapping_mul(17)) + 0.75;
    }

    let red = blue.map(|value| 255.0 - value);
    let [cr_r, cb_g, cr_g, cb_b] = [1.402, -0.344136, -0.714136, 1.772];

    let Ok(available) = Simd::available();

    for backend in available.filter_map(|simd| {
      let Ok(capability) = simd.fused_f32();
      capability
    }) {
      let [mut r, mut g, mut b] = [luma, blue, red];
      let transform = ComponentTransform::Ycbcr {
        offset: 128.0,
        gains: [cr_r, cb_g, cr_g, cb_b],
        maximum: Some(255.0),
      };
      assert_eq!(backend.color(transform, [&mut r, &mut g, &mut b]), Ok(16));

      let input = luma.iter().zip(blue.iter().zip(&red));
      let output = r.iter().zip(g.iter().zip(&b));

      for ((&y, (&blue_sample, &red_sample)), (&red_output, (&green_output, &blue_output))) in
        input.zip(output)
      {
        let cb = blue_sample - 128.0;
        let cr = red_sample - 128.0;
        let expected = [
          cr.mul_add(cr_r, y),
          cr.mul_add(cr_g, cb.mul_add(cb_g, y)),
          cb.mul_add(cb_b, y),
        ]
        .map(|value| value.clamp(0.0, 255.0).to_bits());
        assert_eq!(
          [red_output, green_output, blue_output].map(f32::to_bits),
          expected
        );
      }
    }
  }
}
