use console_core_never::Never;

use super::lanes::lane_operator;

#[derive(Clone, Copy, Debug)]
#[allow(
  non_camel_case_types,
  reason = "the name identifies the lane type and count"
)]
pub struct f32x4([f32; 4]);

impl f32x4 {
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the generic SIMD broadcast preserves the units of its caller's lane value"
    )
  )]
  pub fn splat(value: f32) -> Result<Self, Never> {
    Ok(Self([value; 4]))
  }
}

impl From<[f32; 4]> for f32x4 {
  fn from(lanes: [f32; 4]) -> Self {
    Self(lanes)
  }
}

impl From<f32x4> for [f32; 4] {
  fn from(vector: f32x4) -> Self {
    vector.0
  }
}

lane_operator!(f32x4, Add, add, +, |left, right| left + right);
lane_operator!(f32x4, Sub, sub, -, |left, right| left - right);
lane_operator!(f32x4, Mul, mul, *, |left, right| left * right);

impl core::ops::AddAssign for f32x4 {
  #[inline]
  fn add_assign(&mut self, rhs: Self) {
    *self = *self + rhs;
  }
}
