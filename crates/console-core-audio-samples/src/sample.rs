//! The sample contract and the two 24-bit types the primitives lack.
//!
//! A `u24` or an `i24` holds its value in a 32-bit integer and promises
//! nothing about the range: a decoder may hand one a raw word, and `clamped`
//! is how the valid range is asked for. Arithmetic on a sample is not offered,
//! because nothing here scales or mixes one; the float formats are what a
//! signal is worked in.

use core::fmt;

#[derive(Copy, Clone, Debug)]
pub enum SampleFormat {
  U8,
  U16,
  U24,
  U32,
  S8,
  S16,
  S24,
  S32,
  F32,
  F64,
}

pub trait Sample: Copy + Clone + Default + PartialOrd + PartialEq + Sized {
  const MID: Self;

  fn clamped(self) -> Self;
}

#[allow(
  non_camel_case_types,
  reason = "24-bit samples follow the primitive integer naming convention"
)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct u24(pub u32);

#[allow(
  non_camel_case_types,
  reason = "24-bit samples follow the primitive integer naming convention"
)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct i24(pub i32);

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one definition states the same midpoint and identity clamp for each primitive integer sample, whose valid range is the whole of the type"
  )
)]
macro_rules! whole_range {
  ($kind:ty, $mid:literal) => {
    impl Sample for $kind {
      const MID: $kind = $mid;

      #[inline(always)]
      fn clamped(self) -> Self {
        self
      }
    }
  };
}

whole_range!(u8, 128);
whole_range!(i8, 0);
whole_range!(u16, 32_768);
whole_range!(i16, 0);
whole_range!(u32, 2_147_483_648);
whole_range!(i32, 0);

impl Sample for u24 {
  const MID: u24 = u24(8_388_608);

  #[inline(always)]
  fn clamped(self) -> Self {
    u24(self.0.min(u24::MAX.0))
  }
}

impl Sample for i24 {
  const MID: i24 = i24(0);

  #[inline(always)]
  fn clamped(self) -> Self {
    i24(self.0.clamp(i24::MIN.0, i24::MAX.0))
  }
}

impl Sample for f32 {
  const MID: f32 = 0.0;

  #[inline(always)]
  fn clamped(self) -> Self {
    self.clamp(-1.0, 1.0)
  }
}

impl Sample for f64 {
  const MID: f64 = 0.0;

  #[inline(always)]
  fn clamped(self) -> Self {
    self.clamp(-1.0, 1.0)
  }
}

impl i24 {
  pub const MAX: i24 = i24(8_388_607);
  pub const MIN: i24 = i24(-8_388_608);
}

impl u24 {
  pub const MAX: u24 = u24(16_777_215);
  pub const MIN: u24 = u24(0);
}

impl fmt::Display for i24 {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl fmt::Display for u24 {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl From<i32> for i24 {
  fn from(val: i32) -> Self {
    i24(val).clamped()
  }
}

impl From<i16> for i24 {
  fn from(val: i16) -> Self {
    i24(i32::from(val))
  }
}

impl From<i8> for i24 {
  fn from(val: i8) -> Self {
    i24(i32::from(val))
  }
}

impl From<u32> for u24 {
  fn from(val: u32) -> Self {
    u24(val).clamped()
  }
}

impl From<u16> for u24 {
  fn from(val: u16) -> Self {
    u24(u32::from(val))
  }
}

impl From<u8> for u24 {
  fn from(val: u8) -> Self {
    u24(u32::from(val))
  }
}
