#![forbid(unsafe_code)]

#[cfg(feature = "simd")]
use core::simd::{Select, Simd, cmp::SimdPartialEq};

use console_core_never::Never;

use super::f32x8;

#[allow(non_camel_case_types, reason = "matches the lane names in core::simd")]
#[derive(Clone, Copy, Debug, Default, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C, align(32))]
pub struct u32x8([u32; 8]);

impl u32x8 {
  #[inline]
  pub fn splat(value: u32) -> Result<Self, Never> {
    Ok(Self([value; 8]))
  }

  #[inline]
  pub fn to_f32x8_bitcast(self) -> Result<f32x8, Never> {
    Ok(bytemuck::cast(self))
  }

  #[inline]
  pub fn cmp_eq(self, rhs: Self) -> Result<Self, Never> {
    #[cfg(feature = "simd")]
    let mask = Simd::from_array(self.0)
      .simd_eq(Simd::from_array(rhs.0))
      .select(Simd::splat(u32::MAX), Simd::splat(0))
      .to_array();
    #[cfg(not(feature = "simd"))]
    let mask = core::array::from_fn(|lane| match self.0[lane] == rhs.0[lane] {
      true => u32::MAX,
      false => 0,
    });

    Ok(Self(mask))
  }

  #[inline]
  pub fn shl<const SHIFT: u32>(self) -> Result<Self, Never> {
    const { assert!(SHIFT < u32::BITS) };

    #[cfg(feature = "simd")]
    let shifted = (Simd::from_array(self.0) << SHIFT).to_array();
    #[cfg(not(feature = "simd"))]
    let shifted = self.0.map(|lane| lane << SHIFT);

    Ok(Self(shifted))
  }
}

impl core::ops::Not for u32x8 {
  type Output = Self;

  #[inline]
  fn not(self) -> Self {
    #[cfg(feature = "simd")]
    let bits = (!Simd::from_array(self.0)).to_array();
    #[cfg(not(feature = "simd"))]
    let bits = self.0.map(|lane| !lane);

    Self(bits)
  }
}

impl core::ops::Add for u32x8 {
  type Output = Self;

  #[inline]
  fn add(self, rhs: Self) -> Self {
    #[cfg(feature = "simd")]
    let sum = (Simd::from_array(self.0) + Simd::from_array(rhs.0)).to_array();
    #[cfg(not(feature = "simd"))]
    let sum = core::array::from_fn(|lane| self.0[lane].wrapping_add(rhs.0[lane]));

    Self(sum)
  }
}

impl core::ops::BitAnd for u32x8 {
  type Output = Self;

  #[inline]
  fn bitand(self, rhs: Self) -> Self {
    #[cfg(feature = "simd")]
    let bits = (Simd::from_array(self.0) & Simd::from_array(rhs.0)).to_array();
    #[cfg(not(feature = "simd"))]
    let bits = core::array::from_fn(|lane| self.0[lane] & rhs.0[lane]);

    Self(bits)
  }
}

impl core::ops::BitOr for u32x8 {
  type Output = Self;

  #[inline]
  fn bitor(self, rhs: Self) -> Self {
    #[cfg(feature = "simd")]
    let bits = (Simd::from_array(self.0) | Simd::from_array(rhs.0)).to_array();
    #[cfg(not(feature = "simd"))]
    let bits = core::array::from_fn(|lane| self.0[lane] | rhs.0[lane]);

    Self(bits)
  }
}
