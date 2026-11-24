#![forbid(unsafe_code)]

#[cfg(feature = "simd")]
use core::simd::{Select, Simd, cmp::SimdOrd, cmp::SimdPartialOrd};

use console_core_never::Never;

#[allow(
  non_camel_case_types,
  reason = "the name identifies the lane type and count, matching core::simd"
)]
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct u16x16(pub [u16; 16]);

impl u16x16 {
  #[cfg(not(feature = "simd"))]
  #[inline]
  fn combined(&self, rhs: &Self, operation: impl Fn(u16, u16) -> u16) -> [u16; 16] {
    core::array::from_fn(|lane| operation(self.0[lane], rhs.0[lane]))
  }

  #[inline]
  pub fn splat(value: u16) -> Result<Self, Never> {
    Ok(Self([value; 16]))
  }

  #[inline]
  pub fn as_slice(&self) -> Result<&[u16; 16], Never> {
    Ok(&self.0)
  }

  #[inline]
  pub fn min(&self, rhs: &Self) -> Result<Self, Never> {
    #[cfg(feature = "simd")]
    let minimum = Simd::from_array(self.0)
      .simd_min(Simd::from_array(rhs.0))
      .to_array();
    #[cfg(not(feature = "simd"))]
    let minimum = self.combined(rhs, u16::min);

    Ok(Self(minimum))
  }

  #[inline]
  pub fn max(&self, rhs: &Self) -> Result<Self, Never> {
    #[cfg(feature = "simd")]
    let maximum = Simd::from_array(self.0)
      .simd_max(Simd::from_array(rhs.0))
      .to_array();
    #[cfg(not(feature = "simd"))]
    let maximum = self.combined(rhs, u16::max);

    Ok(Self(maximum))
  }

  #[inline]
  pub fn cmp_le(&self, rhs: &Self) -> Result<Self, Never> {
    #[cfg(feature = "simd")]
    let mask = Simd::from_array(self.0)
      .simd_le(Simd::from_array(rhs.0))
      .select(Simd::splat(u16::MAX), Simd::splat(0))
      .to_array();
    #[cfg(not(feature = "simd"))]
    let mask = self.combined(rhs, |left, right| match left <= right {
      true => u16::MAX,
      false => 0,
    });

    Ok(Self(mask))
  }

  #[inline]
  pub fn blend(self, yes: Self, no: Self) -> Result<Self, Never> {
    Ok((yes & self) | (no & !self))
  }
}

impl core::ops::Add for u16x16 {
  type Output = Self;

  #[inline]
  fn add(self, rhs: Self) -> Self {
    #[cfg(feature = "simd")]
    let sum = (Simd::from_array(self.0) + Simd::from_array(rhs.0)).to_array();
    #[cfg(not(feature = "simd"))]
    let sum = self.combined(&rhs, u16::wrapping_add);

    Self(sum)
  }
}

impl core::ops::Sub for u16x16 {
  type Output = Self;

  #[inline]
  fn sub(self, rhs: Self) -> Self {
    #[cfg(feature = "simd")]
    let difference = (Simd::from_array(self.0) - Simd::from_array(rhs.0)).to_array();
    #[cfg(not(feature = "simd"))]
    let difference = self.combined(&rhs, u16::wrapping_sub);

    Self(difference)
  }
}

impl core::ops::Mul for u16x16 {
  type Output = Self;

  #[inline]
  fn mul(self, rhs: Self) -> Self {
    #[cfg(feature = "simd")]
    let product = (Simd::from_array(self.0) * Simd::from_array(rhs.0)).to_array();
    #[cfg(not(feature = "simd"))]
    let product = self.combined(&rhs, u16::wrapping_mul);

    Self(product)
  }
}

impl core::ops::BitAnd for u16x16 {
  type Output = Self;

  #[inline]
  fn bitand(self, rhs: Self) -> Self {
    #[cfg(feature = "simd")]
    let bits = (Simd::from_array(self.0) & Simd::from_array(rhs.0)).to_array();
    #[cfg(not(feature = "simd"))]
    let bits = self.combined(&rhs, |left, right| left & right);

    Self(bits)
  }
}

impl core::ops::BitOr for u16x16 {
  type Output = Self;

  #[inline]
  fn bitor(self, rhs: Self) -> Self {
    #[cfg(feature = "simd")]
    let bits = (Simd::from_array(self.0) | Simd::from_array(rhs.0)).to_array();
    #[cfg(not(feature = "simd"))]
    let bits = self.combined(&rhs, |left, right| left | right);

    Self(bits)
  }
}

impl core::ops::Not for u16x16 {
  type Output = Self;

  #[inline]
  fn not(self) -> Self {
    Self(self.0.map(|lane| !lane))
  }
}

impl core::ops::Shr for u16x16 {
  type Output = Self;

  #[inline]
  fn shr(self, rhs: Self) -> Self {
    #[cfg(feature = "simd")]
    let shifted = (Simd::from_array(self.0) >> Simd::from_array(rhs.0)).to_array();
    #[cfg(not(feature = "simd"))]
    let shifted = self.combined(&rhs, |left, right| left.wrapping_shr(u32::from(right)));

    Self(shifted)
  }
}
