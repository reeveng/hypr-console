//! How a lookup walks the glyphs it is applied to: which kinds of glyph it skips, whether a
//! cursive chain ends on the baseline, and which marks it leaves alone.
//!
//! It is nearly a bit enumeration and not quite one, because its top byte is a number -- the
//! class of marks it attaches to -- rather than eight more flags, so it is written here by hand
//! rather than generated. Whether it holds a flag is asked as a [`Presence`], and the bits the
//! specification leaves reserved are dropped when the flags are built from a raw value.

use console_core_integer_sets::Presence;
use console_core_never::Never;
use core::ops::{BitOr, BitOrAssign, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LookupFlag(u16);

impl BitOr for LookupFlag {
  type Output = Self;

  fn bitor(self, rhs: Self) -> Self::Output {
    Self(self.0 | rhs.0)
  }
}

impl BitOrAssign for LookupFlag {
  fn bitor_assign(&mut self, rhs: Self) {
    self.0 |= rhs.0
  }
}

impl Sub for LookupFlag {
  type Output = Self;

  #[inline]
  fn sub(self, rhs: Self) -> Self {
    Self(self.0 & !rhs.0)
  }
}

impl LookupFlag {
  pub const RIGHT_TO_LEFT: Self = LookupFlag(0x0001);
  pub const IGNORE_BASE_GLYPHS: Self = LookupFlag(0x002);
  pub const IGNORE_LIGATURES: Self = LookupFlag(0x004);
  pub const IGNORE_MARKS: Self = LookupFlag(0x008);
  pub const USE_MARK_FILTERING_SET: Self = LookupFlag(0x010);

  const FLAG_MASK: Self = LookupFlag(0x1F);

  pub fn empty() -> Result<Self, Never> {
    Ok(Self(0))
  }

  pub fn from_bits_truncate(bits: u16) -> Result<Self, Never> {
    const VALID_BITS: u16 = !0x00E0;

    Ok(Self(bits & VALID_BITS))
  }

  pub fn to_bits(self) -> Result<u16, Never> {
    Ok(self.0)
  }

  #[inline]
  pub const fn contains(&self, other: Self) -> Result<Presence, Never> {
    let other_2 = other.0 & Self::FLAG_MASK.0;

    Ok(match other_2 & !self.0 {
      0 => Presence::Present,
      _missing => Presence::Absent,
    })
  }

  pub fn mark_attachment_class(self) -> Result<Option<u16>, Never> {
    let val = self.0 & 0xff00;

    Ok(match val {
      0 => None,
      _attached => Some(::core::primitive::u16::wrapping_shr(val, 8_u32)),
    })
  }

  pub fn set_mark_attachment_class(&mut self, val: u16) -> Result<(), Never> {
    let val_2 = ::core::primitive::u16::wrapping_shl(val & 0xff, 8_u32);
    self.0 = (self.0 & 0xff) | val_2;

    Ok(())
  }
}

impl types::Scalar for LookupFlag {
  type Raw = <u16 as types::Scalar>::Raw;

  fn to_raw(self) -> Result<Self::Raw, Never> {
    self.0.to_raw()
  }

  fn from_raw(raw: Self::Raw) -> Result<Self, Never> {
    let Ok(bits) = <u16>::from_raw(raw);

    Ok(Self(bits))
  }
}
