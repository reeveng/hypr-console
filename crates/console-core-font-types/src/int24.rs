//! A signed integer three bytes wide, the width a variation delta and a `COLR` offset are written
//! in.
//!
//! It is held in an `i32` and held at either end of 24 bits by `new`; reading three bytes widens
//! the top bit into the fourth, so a negative value reads back negative.

use console_core_never::Never;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bytemuck", derive(bytemuck::AnyBitPattern))]
#[repr(transparent)]
pub struct Int24(i32);

impl Int24 {
  const MIN_VALUE: i32 = -0x0080_0000;
  const MAX_VALUE: i32 = 0x007f_ffff;

  pub const MIN: Self = Int24(Self::MIN_VALUE);

  pub const MAX: Self = Int24(Self::MAX_VALUE);

  pub const fn new(raw: i32) -> Result<Int24, Never> {
    Ok(match raw {
      ..Self::MIN_VALUE => Self::MIN,
      Self::MIN_VALUE..=Self::MAX_VALUE => Self(raw),
      _ => Self::MAX,
    })
  }

  pub const fn checked_new(raw: i32) -> Result<Option<Int24>, Never> {
    Ok(match raw {
      Self::MIN_VALUE..=Self::MAX_VALUE => Some(Int24(raw)),
      _ => None,
    })
  }

  pub const fn to_i32(self) -> Result<i32, Never> {
    Ok(self.0)
  }

  pub const fn to_be_bytes(self) -> Result<[u8; 3], Never> {
    let [_, first, second, third] = self.0.to_be_bytes();

    Ok([first, second, third])
  }

  pub const fn from_be_bytes(bytes: [u8; 3]) -> Result<Self, Never> {
    let [first, second, third] = bytes;
    let sign = match first & 0x80 {
      0 => 0,
      _negative => 0xff,
    };

    Ok(Self(i32::from_be_bytes([sign, first, second, third])))
  }
}

impl From<Int24> for i32 {
  fn from(src: Int24) -> i32 {
    src.0
  }
}

impl core::fmt::Display for Int24 {
  fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
    self.0.fmt(f)
  }
}
