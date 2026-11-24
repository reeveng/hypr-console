//! Min and max select the right operand for unordered comparisons and ties
//! in the SIMD path, as the x86 pipeline did. Pixel conversion first
//! normalizes and scales each lane to 0..=255. Adding 2^23 rounds to even in a
//! binade where adjacent float bit patterns represent consecutive integers.

#[cfg(not(feature = "std"))]
use console_core_paths::NoStdFloat;
#[cfg(feature = "simd")]
use core::simd::{
  Mask, Select, Simd,
  cmp::{SimdPartialEq, SimdPartialOrd},
  num::SimdFloat,
};
#[cfg(all(feature = "simd", feature = "std"))]
use std::simd::StdFloat;

use crate::math::GammaExponent;
use console_core_never::Never;

use super::{i32x8, lanes::lane_operator, u32x8};

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "the six lane comparisons share the same SIMD and scalar mask construction"
  )
)]
macro_rules! compare {
  ($name:ident, $simd:ident, $comparison:tt) => {
    #[inline]
    pub fn $name(self, rhs: Self) -> Result<FloatMask8, Never> {
      #[cfg(feature = "simd")]
      let lanes = Simd::from_array(self.0).$simd(Simd::from_array(rhs.0));
      #[cfg(not(feature = "simd"))]
      let lanes = super::lanes::combine(self.0, rhs.0, |left, right| left $comparison right);

      Ok(FloatMask8(lanes))
    }
  };
}

#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Zeroable, bytemuck::Pod)]
#[allow(
  non_camel_case_types,
  reason = "the name identifies the lane type and count"
)]
#[repr(C, align(32))]
pub struct f32x8([f32; 8]);

impl f32x8 {
  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the generic SIMD broadcast preserves the units of its caller's lane value"
    )
  )]
  pub fn splat(value: f32) -> Result<Self, Never> {
    Ok(Self([value; 8]))
  }

  #[inline]
  pub fn floor(self) -> Result<Self, Never> {
    #[cfg(all(feature = "simd", feature = "std"))]
    let lanes = Simd::from_array(self.0).floor().to_array();
    #[cfg(all(not(feature = "simd"), feature = "std"))]
    let lanes = self.0.map(f32::floor);
    #[cfg(not(feature = "std"))]
    let lanes = self.0.map(<f32 as NoStdFloat>::floor);

    Ok(Self(lanes))
  }

  #[inline]
  pub fn fract(self) -> Result<Self, Never> {
    let Ok(floor) = self.floor();

    Ok(self - floor)
  }

  #[inline]
  pub fn normalize(self) -> Result<Self, Never> {
    let Ok(nonnegative) = self.max(Self::default());
    let Ok(one) = Self::splat(1.0);

    nonnegative.min(one)
  }

  #[inline]
  pub fn abs(self) -> Result<Self, Never> {
    #[cfg(feature = "simd")]
    let lanes = Simd::from_array(self.0).abs().to_array();
    #[cfg(not(feature = "simd"))]
    let lanes = self.0.map(f32::abs);

    Ok(Self(lanes))
  }

  #[inline]
  pub fn max(self, rhs: Self) -> Result<Self, Never> {
    #[cfg(feature = "simd")]
    let lanes = {
      let left = Simd::from_array(self.0);
      let right = Simd::from_array(rhs.0);
      left.simd_gt(right).select(left, right).to_array()
    };
    #[cfg(not(feature = "simd"))]
    let lanes = super::lanes::combine(self.0, rhs.0, |left, right| match left < right {
      true => right,
      false => left,
    });

    Ok(Self(lanes))
  }

  #[inline]
  pub fn min(self, rhs: Self) -> Result<Self, Never> {
    #[cfg(feature = "simd")]
    let lanes = {
      let left = Simd::from_array(self.0);
      let right = Simd::from_array(rhs.0);
      left.simd_lt(right).select(left, right).to_array()
    };
    #[cfg(not(feature = "simd"))]
    let lanes = super::lanes::combine(self.0, rhs.0, |left, right| match right < left {
      true => right,
      false => left,
    });

    Ok(Self(lanes))
  }

  compare!(cmp_eq, simd_eq, ==);
  compare!(cmp_ne, simd_ne, !=);
  compare!(cmp_ge, simd_ge, >=);
  compare!(cmp_gt, simd_gt, >);
  compare!(cmp_le, simd_le, <=);
  compare!(cmp_lt, simd_lt, <);

  #[inline]
  pub fn round(self) -> Result<Self, Never> {
    #[cfg(all(feature = "simd", feature = "std"))]
    let lanes = Simd::from_array(self.0).round_ties_even().to_array();
    #[cfg(all(not(feature = "simd"), feature = "std"))]
    let lanes = self.0.map(f32::round_ties_even);
    #[cfg(not(feature = "std"))]
    let lanes = self.0.map(round_ties_even);

    Ok(Self(lanes))
  }

  #[inline]
  pub fn round_int(self) -> Result<i32x8, Never> {
    let Ok(rounded) = self.round();

    rounded.trunc_int()
  }

  #[inline]
  pub fn to_pixel_channels(self) -> Result<i32x8, Never> {
    let Ok(normalized) = self.normalize();
    let Ok(scale) = Self::splat(255.0);
    let scaled = normalized * scale;

    #[cfg(feature = "simd")]
    {
      const ROUNDING_BIAS: f32 = 8_388_608.0;
      let Ok(bias) = Self::splat(ROUNDING_BIAS);
      let Ok(rounded_bits) = (scaled + bias).to_i32x8_bitcast();
      let Ok(bias_bits) = bias.to_i32x8_bitcast();

      Ok(rounded_bits - bias_bits)
    }

    #[cfg(not(feature = "simd"))]
    scaled.round_int()
  }

  #[inline]
  pub fn trunc_int(self) -> Result<i32x8, Never> {
    #[cfg(feature = "simd")]
    let lanes = Simd::from_array(self.0).cast::<i32>().to_array();
    #[cfg(not(feature = "simd"))]
    let lanes = self.0.map(|lane| {
      let Ok(converted) = console_core_number_conversion::toward_zero_i32(f64::from(lane));
      converted
    });

    Ok(i32x8(lanes))
  }

  #[inline]
  pub fn reciprocal(self) -> Result<Self, Never> {
    let Ok(one) = Self::splat(1.0);

    Ok(one / self)
  }

  #[inline]
  pub fn sqrt(self) -> Result<Self, Never> {
    #[cfg(all(feature = "simd", feature = "std"))]
    let lanes = Simd::from_array(self.0).sqrt().to_array();
    #[cfg(all(not(feature = "simd"), feature = "std"))]
    let lanes = self.0.map(f32::sqrt);
    #[cfg(not(feature = "std"))]
    let lanes = self.0.map(<f32 as NoStdFloat>::sqrt);

    Ok(Self(lanes))
  }

  #[inline]
  fn to_i32x8_bitcast(self) -> Result<i32x8, Never> {
    Ok(bytemuck::cast(self))
  }

  #[inline]
  pub fn to_u32x8_bitcast(self) -> Result<u32x8, Never> {
    Ok(bytemuck::cast(self))
  }

  pub fn powf(self, GammaExponent(exponent): GammaExponent) -> Result<Self, Never> {
    let Ok(logarithm) = self.approximate_log2();
    let Ok(power) = Self::splat(exponent);
    let Ok(value) = (logarithm * power).approximate_exp2();
    let Ok(zero) = Self::splat(0.0);
    let Ok(one) = Self::splat(1.0);
    let Ok(is_zero) = self.cmp_eq(zero);
    let Ok(is_one) = self.cmp_eq(one);

    (is_zero | is_one).blend(self, value)
  }

  fn approximate_log2(self) -> Result<Self, Never> {
    let Ok(encoded) = self.to_i32x8_bitcast();
    let Ok(exponent_scale) = Self::splat(1.0 / 8_388_608.0);
    let Ok(integer_bits) = encoded.to_f32x8();
    let exponent = integer_bits * exponent_scale;
    let Ok(bits) = self.to_u32x8_bitcast();
    let Ok(significand_bits) = u32x8::splat(0x007f_ffff);
    let Ok(normalized_exponent) = u32x8::splat(0x3f00_0000);
    let Ok(significand) = ((bits & significand_bits) | normalized_exponent).to_f32x8_bitcast();
    let Ok(bias) = Self::splat(124.225_52);
    let Ok(linear) = Self::splat(1.498_030_3);
    let Ok(numerator) = Self::splat(1.725_88);
    let Ok(denominator) = Self::splat(0.352_088_72);

    Ok(exponent - bias - linear * significand - numerator / (denominator + significand))
  }

  fn approximate_exp2(self) -> Result<Self, Never> {
    let Ok(floor) = self.floor();
    let fraction = self - floor;
    let Ok(bias) = Self::splat(121.274_055);
    let Ok(linear) = Self::splat(1.490_129_1);
    let Ok(numerator) = Self::splat(27.728_024);
    let Ok(denominator) = Self::splat(4.842_525_5);
    let Ok(exponent_scale) = Self::splat(8_388_608.0);
    let approx =
      (self + bias - fraction * linear + numerator / (denominator - fraction)) * exponent_scale;
    let Ok(infinity_bits) =
      console_core_number_conversion::nearest_f32_from_u64(u64::from(f32::INFINITY.to_bits()));
    let Ok(maximum) = Self::splat(infinity_bits);
    let Ok(nonnegative) = approx.max(Self::default());
    let Ok(limited) = nonnegative.min(maximum);
    let Ok(rounded) = limited.round_int();

    rounded.to_f32x8_bitcast()
  }
}

#[cfg(not(feature = "std"))]
#[inline]
fn round_ties_even(value: f32) -> f32 {
  let magnitude = value.abs();
  let integer_spacing = 1.0 / f32::EPSILON;

  match magnitude < integer_spacing {
    true => (magnitude + integer_spacing - integer_spacing).copysign(value),
    false => value,
  }
}

impl From<[f32; 8]> for f32x8 {
  fn from(lanes: [f32; 8]) -> Self {
    Self(lanes)
  }
}

impl From<f32x8> for [f32; 8] {
  fn from(vector: f32x8) -> Self {
    vector.0
  }
}

lane_operator!(f32x8, Add, add, +, |left, right| left + right);
lane_operator!(f32x8, Sub, sub, -, |left, right| left - right);
lane_operator!(f32x8, Mul, mul, *, |left, right| left * right);
lane_operator!(f32x8, Div, div, /, |left, right| left / right);

impl core::ops::AddAssign for f32x8 {
  #[inline]
  fn add_assign(&mut self, rhs: Self) {
    *self = *self + rhs;
  }
}

impl core::ops::MulAssign for f32x8 {
  #[inline]
  fn mul_assign(&mut self, rhs: Self) {
    *self = *self * rhs;
  }
}

impl core::ops::Neg for f32x8 {
  type Output = Self;

  #[inline]
  fn neg(self) -> Self {
    Self(self.0.map(core::ops::Neg::neg))
  }
}

#[derive(Clone, Copy)]
pub struct FloatMask8(
  #[cfg(feature = "simd")] Mask<i32, 8>,
  #[cfg(not(feature = "simd"))] [bool; 8],
);

impl FloatMask8 {
  #[inline]
  pub fn to_u32x8_mask(self) -> Result<u32x8, Never> {
    #[cfg(feature = "simd")]
    let lanes = self
      .0
      .select(Simd::splat(u32::MAX), Simd::splat(0))
      .to_array();
    #[cfg(not(feature = "simd"))]
    let lanes = self.0.map(|set| match set {
      true => u32::MAX,
      false => 0,
    });

    Ok(bytemuck::cast(lanes))
  }

  #[inline]
  pub fn blend(self, yes: f32x8, no: f32x8) -> Result<f32x8, Never> {
    #[cfg(feature = "simd")]
    let lanes = self
      .0
      .select(Simd::from_array(yes.0), Simd::from_array(no.0))
      .to_array();
    #[cfg(not(feature = "simd"))]
    let lanes = {
      let mut selected = no.0;

      for ((lane, mask), when_set) in selected.iter_mut().zip(self.0).zip(yes.0) {
        match mask {
          true => *lane = when_set,
          false => {}
        }
      }

      selected
    };

    Ok(f32x8(lanes))
  }
}

impl core::ops::BitOr for FloatMask8 {
  type Output = Self;

  #[inline]
  fn bitor(self, rhs: Self) -> Self {
    #[cfg(feature = "simd")]
    let lanes = self.0 | rhs.0;
    #[cfg(not(feature = "simd"))]
    let lanes = super::lanes::combine(self.0, rhs.0, |left, right| left | right);

    Self(lanes)
  }
}

#[cfg(test)]
mod tests {
  use super::f32x8;

  #[test]
  fn nearest_integer_uses_ties_to_even() {
    let input = f32x8::from([-2.5, -1.5, -0.5, -0.0, 0.5, 1.5, 2.5, 3.5]);
    let Ok(rounded) = input.round();
    let Ok(integers) = input.round_int();

    assert_eq!(
      <[f32; 8]>::from(rounded).map(f32::to_bits),
      [-2.0_f32, -2.0, -0.0, -0.0, 0.0, 2.0, 2.0, 4.0].map(f32::to_bits)
    );
    assert_eq!(<[i32; 8]>::from(integers), [-2, -2, 0, 0, 0, 2, 2, 4]);
  }

  #[test]
  fn pixel_channels_clamp_and_round_to_even() {
    let inputs = f32x8::from([-1.0, 0.1, 0.3, 0.5, 0.7, 1.0, 2.0, f32::NAN]);
    let Ok(pixels) = inputs.to_pixel_channels();

    assert_eq!(<[i32; 8]>::from(pixels), [0, 26, 76, 128, 178, 255, 255, 0]);
  }

  #[test]
  fn floor_keeps_signed_zero_and_large_whole_values() {
    let input = f32x8::from([
      -2.5,
      -0.5,
      -0.0,
      0.0,
      2.5,
      4_294_967_296.0,
      f32::INFINITY,
      f32::NEG_INFINITY,
    ]);
    let Ok(floor) = input.floor();

    assert_eq!(
      <[f32; 8]>::from(floor).map(f32::to_bits),
      [
        -3.0_f32,
        -1.0,
        -0.0,
        0.0,
        2.0,
        4_294_967_296.0,
        f32::INFINITY,
        f32::NEG_INFINITY
      ]
      .map(f32::to_bits)
    );
  }
}
