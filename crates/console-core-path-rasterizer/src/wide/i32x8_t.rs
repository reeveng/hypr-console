#[cfg(feature = "simd")]
use core::simd::{Simd, num::SimdInt};

use console_core_never::Never;

use super::{f32x8, lanes::lane_operator, u32x8};

#[derive(Clone, Copy, Debug, bytemuck::Zeroable, bytemuck::Pod)]
#[allow(
  non_camel_case_types,
  reason = "the name identifies the lane type and count"
)]
#[repr(C, align(32))]
pub struct i32x8(pub(super) [i32; 8]);

impl i32x8 {
  #[inline]
  pub fn splat(value: i32) -> Result<Self, Never> {
    Ok(Self([value; 8]))
  }

  #[inline]
  pub fn to_f32x8(self) -> Result<f32x8, Never> {
    #[cfg(feature = "simd")]
    let lanes = Simd::from_array(self.0).cast::<f32>().to_array();
    #[cfg(not(feature = "simd"))]
    let lanes = self.0.map(|lane| {
      let Ok(converted) = console_core_number_conversion::nearest_f32_from_i64(i64::from(lane));
      converted
    });

    Ok(f32x8::from(lanes))
  }

  #[inline]
  pub fn to_f32x8_bitcast(self) -> Result<f32x8, Never> {
    Ok(bytemuck::cast(self))
  }

  #[inline]
  pub fn to_u32x8_bitcast(self) -> Result<u32x8, Never> {
    Ok(bytemuck::cast(self))
  }
}

impl From<i32x8> for [i32; 8] {
  fn from(vector: i32x8) -> Self {
    vector.0
  }
}

lane_operator!(i32x8, Add, add, +, i32::wrapping_add);
#[cfg(feature = "simd")]
lane_operator!(i32x8, Sub, sub, -, i32::wrapping_sub);
lane_operator!(i32x8, Mul, mul, *, i32::wrapping_mul);
