#![forbid(unsafe_code)]

use super::{f32x8, u16x16};

#[derive(Copy, Clone, Debug, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C, align(32))]
pub struct f32x16(pub f32x8, pub f32x8);

impl Default for f32x16 {
  fn default() -> Self {
    let Ok(splat) = Self::splat(0.0);

    splat
  }
}

impl f32x16 {
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the generic SIMD broadcast preserves the units of its caller's lane value"
    )
  )]
  pub fn splat(n: f32) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(splat) = f32x8::splat(n);

    Ok(Self(splat, {
      let Ok(explicit_value) = f32x8::splat(n);
      explicit_value
    }))
  }

  #[inline]
  pub fn abs(&self) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(low) = self.0.abs();
    let Ok(high) = self.1.abs();

    Ok(Self(low, high))
  }

  pub fn normalize(&self) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(normalize) = self.0.normalize();
    let Ok(own_normalize) = self.1.normalize();

    Ok(Self(normalize, own_normalize))
  }

  pub fn floor(&self) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(low) = self.0.floor();
    let Ok(high) = self.1.floor();

    Ok(Self(low, high))
  }

  pub fn sqrt(&self) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self(
      {
        let Ok(explicit_value) = self.0.sqrt();
        explicit_value
      },
      {
        let Ok(explicit_value) = self.1.sqrt();
        explicit_value
      },
    ))
  }

  pub fn save_to_u16x16(&self, dst: &mut u16x16) -> Result<(), console_core_never::Never> {
    let low = <[f32; 8]>::from(self.0);
    let high = <[f32; 8]>::from(self.1);

    for (output, input) in dst.0.iter_mut().zip(low.into_iter().chain(high)) {
      let Ok(value) = console_core_number_conversion::toward_zero_u16(f64::from(input));
      *output = value;
    }

    Ok(())
  }
}

impl core::ops::Add<f32x16> for f32x16 {
  type Output = Self;

  fn add(self, rhs: Self) -> Self::Output {
    Self(self.0 + rhs.0, self.1 + rhs.1)
  }
}

impl core::ops::Sub<f32x16> for f32x16 {
  type Output = Self;

  fn sub(self, rhs: Self) -> Self::Output {
    Self(self.0 - rhs.0, self.1 - rhs.1)
  }
}

impl core::ops::Mul<f32x16> for f32x16 {
  type Output = Self;

  fn mul(self, rhs: Self) -> Self::Output {
    Self(self.0 * rhs.0, self.1 * rhs.1)
  }
}
