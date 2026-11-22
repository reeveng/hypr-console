//! A distance in font units, the grid a font's outlines are drawn on: signed as `FWord`, for a
//! coordinate or a bearing, and unsigned as `UfWord`, for an advance.
//!
//! How many units make an em is the font's own `unitsPerEm`, so a value here is a count of grid
//! steps rather than a size. A variation delta moves one by a 16.16 amount, which can carry a
//! fraction, so the answer is an `f32`.

use super::{F48Dot16, Fixed};
use console_core_never::Never;
use console_core_number_conversion::narrow_f32;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bytemuck", derive(bytemuck::AnyBitPattern))]
#[repr(transparent)]
pub struct FWord(i16);

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bytemuck", derive(bytemuck::AnyBitPattern))]
#[repr(transparent)]
pub struct UfWord(u16);

impl FWord {
  pub const fn new(raw: i16) -> Result<Self, Never> {
    Ok(Self(raw))
  }

  pub const fn to_i16(self) -> Result<i16, Never> {
    Ok(self.0)
  }

  pub fn to_fixed(self) -> Result<Fixed, Never> {
    Fixed::from_i32(i32::from(self.0))
  }

  #[inline(always)]
  pub fn apply_delta(self, delta: F48Dot16) -> Result<f32, Never> {
    let Ok(wide) = delta.to_f64();
    let Ok(step) = narrow_f32(wide);

    Ok(f32::from(self.0) + step)
  }

  pub const fn to_be_bytes(self) -> Result<[u8; 2], Never> {
    Ok(self.0.to_be_bytes())
  }
}

impl UfWord {
  pub const fn new(raw: u16) -> Result<Self, Never> {
    Ok(Self(raw))
  }

  pub const fn to_u16(self) -> Result<u16, Never> {
    Ok(self.0)
  }

  pub fn to_fixed(self) -> Result<Fixed, Never> {
    Fixed::from_i32(i32::from(self.0))
  }

  #[inline(always)]
  pub fn apply_delta(self, delta: F48Dot16) -> Result<f32, Never> {
    let Ok(wide) = delta.to_f64();
    let Ok(step) = narrow_f32(wide);

    Ok(f32::from(self.0) + step)
  }

  pub const fn to_be_bytes(self) -> Result<[u8; 2], Never> {
    Ok(self.0.to_be_bytes())
  }
}

impl core::fmt::Display for FWord {
  fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
    self.0.fmt(f)
  }
}

impl core::fmt::Display for UfWord {
  fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
    self.0.fmt(f)
  }
}

impl From<u16> for UfWord {
  fn from(src: u16) -> Self {
    UfWord(src)
  }
}

impl From<i16> for FWord {
  fn from(src: i16) -> Self {
    FWord(src)
  }
}

impl From<FWord> for i16 {
  fn from(src: FWord) -> Self {
    src.0
  }
}

impl From<UfWord> for u16 {
  fn from(src: UfWord) -> Self {
    src.0
  }
}

crate::raw::newtype_scalar!(FWord, [u8; 2]);
crate::raw::newtype_scalar!(UfWord, [u8; 2]);
