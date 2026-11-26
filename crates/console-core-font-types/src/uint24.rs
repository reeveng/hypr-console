//! An unsigned integer three bytes wide, the width OpenType writes a 24-bit offset and a large
//! glyph identifier in.
//!
//! It is held in a `u32`, and a `u32` past the top of 24 bits is held at the top by `new` and
//! refused by `checked_new`, so nothing outside the three bytes is ever kept.

use console_core_never::Never;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
  feature = "bytemuck",
  derive(bytemuck::AnyBitPattern, bytemuck::NoUninit)
)]
#[repr(transparent)]
pub struct Uint24(u32);

impl Uint24 {
  const MAX_VALUE: u32 = 0x00ff_ffff;

  pub const MIN: Self = Uint24(0);

  pub const MAX: Self = Uint24(Self::MAX_VALUE);

  pub const fn new(raw: u32) -> Result<Uint24, Never> {
    Ok(match raw {
      ..=Self::MAX_VALUE => Self(raw),
      _ => Self::MAX,
    })
  }

  pub const fn checked_new(raw: u32) -> Result<Option<Uint24>, Never> {
    Ok(match raw {
      ..=Self::MAX_VALUE => Some(Uint24(raw)),
      _ => None,
    })
  }

  pub const fn to_u32(self) -> Result<u32, Never> {
    Ok(self.0)
  }

  pub const fn to_be_bytes(self) -> Result<[u8; 3], Never> {
    let [_, first, second, third] = self.0.to_be_bytes();

    Ok([first, second, third])
  }

  pub const fn from_be_bytes(bytes: [u8; 3]) -> Result<Self, Never> {
    let Ok(value) = console_core_bytes::endian::u24_be(bytes);

    Ok(Self(value))
  }
}

impl From<Uint24> for u32 {
  fn from(src: Uint24) -> u32 {
    src.0
  }
}

impl core::fmt::Display for Uint24 {
  fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
    self.0.fmt(f)
  }
}
