use crate::floating_point::float_bits_in_value_order;

pub const SCALAR_MAX: f32 = f32::MAX;
pub const SCALAR_NEARLY_ZERO: f32 = 1.0 / 4096.0;
pub const SCALAR_ROOT_2_OVER_2: f32 = core::f32::consts::FRAC_1_SQRT_2;

#[doc = "Result of comparing two scalar values with a tolerance."]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarProximity {
  WithinTolerance,
  OutsideTolerance,
}

impl From<bool> for ScalarProximity {
  fn from(within_tolerance: bool) -> Self {
    match within_tolerance.then_some(()) {
      Some(()) => Self::WithinTolerance,
      None => Self::OutsideTolerance,
    }
  }
}

#[expect(
  clippy::wrong_self_convention,
  reason = "the trait is implemented for f32, which is Copy, so asking a number by value is asking it by reference"
)]
pub trait Scalar {
  fn half(self) -> Self;

  fn ave(self, other: Self) -> Self;

  fn sqr(self) -> Self;

  fn invert(self) -> Self;

  fn bound(self, min: Self, max: Self) -> Self;

  fn is_nearly_equal(self, other: Self) -> ScalarProximity;

  fn is_nearly_equal_within_tolerance(self, other: Self, tolerance: Self) -> ScalarProximity;

  fn is_nearly_zero(self) -> ScalarProximity;

  fn is_nearly_zero_within_tolerance(self, tolerance: Self) -> ScalarProximity;

  fn almost_dequal_ulps(self, other: Self) -> ScalarProximity;
}

impl Scalar for f32 {
  fn half(self) -> f32 {
    self * 0.5
  }

  fn ave(self, other: Self) -> f32 {
    (self + other) * 0.5
  }

  fn sqr(self) -> f32 {
    self * self
  }

  fn invert(self) -> f32 {
    1.0 / self
  }

  fn bound(self, min: Self, max: Self) -> Self {
    max.min(self).max(min)
  }

  fn is_nearly_equal(self, other: Self) -> ScalarProximity {
    ScalarProximity::from((self - other).abs() <= SCALAR_NEARLY_ZERO)
  }

  fn is_nearly_equal_within_tolerance(self, other: Self, tolerance: Self) -> ScalarProximity {
    ScalarProximity::from((self - other).abs() <= tolerance)
  }

  fn is_nearly_zero(self) -> ScalarProximity {
    self.is_nearly_zero_within_tolerance(SCALAR_NEARLY_ZERO)
  }

  fn is_nearly_zero_within_tolerance(self, tolerance: Self) -> ScalarProximity {
    ScalarProximity::from(self.abs() <= tolerance)
  }

  fn almost_dequal_ulps(self, other: Self) -> ScalarProximity {
    const ULPS_EPSILON: i32 = 16;
    let Ok(a_bits) = float_bits_in_value_order(self.to_bits());
    let Ok(b_bits) = float_bits_in_value_order(other.to_bits());

    ScalarProximity::from(
      a_bits < i32::wrapping_add(b_bits, ULPS_EPSILON)
        && b_bits < i32::wrapping_add(a_bits, ULPS_EPSILON),
    )
  }
}

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
pub trait NoStdFloat {
  fn trunc(self) -> Self;

  fn sqrt(self) -> Self;

  fn abs(self) -> Self;

  fn sin(self) -> Self;

  fn cos(self) -> Self;

  fn ceil(self) -> Self;

  fn floor(self) -> Self;

  fn round(self) -> Self;

  fn powf(self, y: Self) -> Self;

  fn acos(self) -> Self;
}

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
impl NoStdFloat for f32 {
  fn trunc(self) -> Self {
    libm::truncf(self)
  }

  fn sqrt(self) -> Self {
    libm::sqrtf(self)
  }

  fn abs(self) -> Self {
    libm::fabsf(self)
  }

  fn sin(self) -> Self {
    libm::sinf(self)
  }

  fn cos(self) -> Self {
    libm::cosf(self)
  }

  fn ceil(self) -> Self {
    libm::ceilf(self)
  }

  fn floor(self) -> Self {
    libm::floorf(self)
  }

  fn round(self) -> Self {
    libm::roundf(self)
  }

  fn powf(self, y: Self) -> Self {
    libm::powf(self, y)
  }

  fn acos(self) -> Self {
    libm::acosf(self)
  }
}

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
impl NoStdFloat for f64 {
  fn trunc(self) -> Self {
    libm::trunc(self)
  }

  fn sqrt(self) -> Self {
    libm::sqrt(self)
  }

  fn abs(self) -> Self {
    libm::fabs(self)
  }

  fn sin(self) -> Self {
    libm::sin(self)
  }

  fn cos(self) -> Self {
    libm::cos(self)
  }

  fn ceil(self) -> Self {
    libm::ceil(self)
  }

  fn floor(self) -> Self {
    libm::floor(self)
  }

  fn round(self) -> Self {
    libm::round(self)
  }

  fn powf(self, y: Self) -> Self {
    libm::pow(self, y)
  }

  fn acos(self) -> Self {
    libm::acos(self)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn bound() {
    assert_eq!(f32::NAN.bound(0.0, 1.0), 1.0);
    assert_eq!(f32::INFINITY.bound(0.0, 1.0), 1.0);
    assert_eq!(f32::NEG_INFINITY.bound(0.0, 1.0), 0.0);
    assert_eq!(f32::EPSILON.bound(0.0, 1.0), f32::EPSILON);
    assert_eq!(0.5.bound(0.0, 1.0), 0.5);
    assert_eq!((-1.0).bound(0.0, 1.0), 0.0);
    assert_eq!(2.0.bound(0.0, 1.0), 1.0);
  }
}
