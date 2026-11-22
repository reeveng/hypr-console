//! Which glyph: an index into a font's glyphs, at the three widths a table may write it.
//!
//! The specification treats them as integers, and they are types here so an index into the glyphs
//! cannot be handed where a count or an offset was meant. `GlyphId` is the width everything is
//! carried in once it is read; `GlyphId16` and `GlyphId24` are what a table writes, and narrowing
//! to either is a conversion that can be refused, by a fault that names the identifier that did not
//! fit.

use crate::Uint24;
use console_core_never::Never;

#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "TryFrom checks narrowing from GlyphId; every u16 is a representable glyph identifier, including the default notdef glyph"
  )
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
  feature = "bytemuck",
  derive(bytemuck::AnyBitPattern, bytemuck::NoUninit)
)]
#[repr(transparent)]
pub struct GlyphId16(u16);

impl GlyphId16 {
  pub const NOTDEF: GlyphId16 = GlyphId16(0);

  pub const fn new(raw: u16) -> Result<Self, Never> {
    Ok(GlyphId16(raw))
  }

  pub const fn to_u16(self) -> Result<u16, Never> {
    Ok(self.0)
  }

  pub const fn to_u32(self) -> Result<u32, Never> {
    let [high, low] = self.0.to_be_bytes();

    Ok(u32::from_be_bytes([0, 0, high, low]))
  }

  pub const fn to_be_bytes(self) -> Result<[u8; 2], Never> {
    Ok(self.0.to_be_bytes())
  }
}

impl Default for GlyphId16 {
  fn default() -> Self {
    GlyphId16::NOTDEF
  }
}

impl From<u16> for GlyphId16 {
  fn from(value: u16) -> Self {
    Self(value)
  }
}

impl core::fmt::Display for GlyphId16 {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "GID_{}", self.0)
  }
}

impl From<GlyphId16> for u32 {
  fn from(value: GlyphId16) -> u32 {
    u32::from(value.0)
  }
}

crate::raw::newtype_scalar!(GlyphId16, [u8; 2]);

#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "TryFrom checks narrowing from GlyphId; Uint24 already constrains the stored value to a representable glyph identifier"
  )
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
  feature = "bytemuck",
  derive(bytemuck::AnyBitPattern, bytemuck::NoUninit)
)]
#[repr(transparent)]
pub struct GlyphId24(Uint24);

impl GlyphId24 {
  pub const NOTDEF: GlyphId24 = GlyphId24(Uint24::MIN);

  pub const fn new(raw: u32) -> Result<Self, Never> {
    let Ok(held) = Uint24::new(raw);

    Ok(GlyphId24(held))
  }

  pub const fn checked_new(raw: u32) -> Result<Option<Self>, Never> {
    let Ok(held) = Uint24::checked_new(raw);

    Ok(match held {
      Some(fits) => Some(GlyphId24(fits)),
      None => None,
    })
  }

  pub const fn to_u32(self) -> Result<u32, Never> {
    self.0.to_u32()
  }

  pub const fn to_be_bytes(self) -> Result<[u8; 3], Never> {
    self.0.to_be_bytes()
  }

  pub const fn from_be_bytes(bytes: [u8; 3]) -> Result<Self, Never> {
    let Ok(held) = Uint24::from_be_bytes(bytes);

    Ok(GlyphId24(held))
  }
}

impl Default for GlyphId24 {
  fn default() -> Self {
    GlyphId24::NOTDEF
  }
}

impl core::fmt::Display for GlyphId24 {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "GID_{}", self.0)
  }
}

impl From<GlyphId24> for u32 {
  fn from(value: GlyphId24) -> u32 {
    u32::from(value.0)
  }
}

crate::raw::newtype_scalar!(GlyphId24, [u8; 3]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
  feature = "bytemuck",
  derive(bytemuck::AnyBitPattern, bytemuck::NoUninit)
)]
#[repr(transparent)]
pub struct GlyphId(u32);

impl GlyphId {
  pub const NOTDEF: GlyphId = GlyphId(0);

  pub const fn new(raw: u32) -> Result<Self, Never> {
    Ok(Self(raw))
  }

  pub const fn to_u32(self) -> Result<u32, Never> {
    Ok(self.0)
  }
}

impl Default for GlyphId {
  fn default() -> Self {
    GlyphId::NOTDEF
  }
}

impl From<u16> for GlyphId {
  fn from(value: u16) -> Self {
    Self(u32::from(value))
  }
}

impl From<u32> for GlyphId {
  fn from(value: u32) -> Self {
    Self(value)
  }
}

impl core::fmt::Display for GlyphId {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "GID_{}", self.0)
  }
}

impl From<GlyphId> for u32 {
  fn from(value: GlyphId) -> u32 {
    value.0
  }
}

impl From<GlyphId16> for GlyphId {
  fn from(value: GlyphId16) -> GlyphId {
    Self(u32::from(value))
  }
}

impl From<GlyphId24> for GlyphId {
  fn from(value: GlyphId24) -> GlyphId {
    Self(u32::from(value))
  }
}

impl PartialEq<GlyphId16> for GlyphId {
  fn eq(&self, other: &GlyphId16) -> bool {
    self.0 == u32::from(other.0)
  }
}

impl PartialOrd<GlyphId16> for GlyphId {
  fn partial_cmp(&self, other: &GlyphId16) -> Option<core::cmp::Ordering> {
    Some(self.0.cmp(&u32::from(other.0)))
  }
}

impl PartialEq<GlyphId> for GlyphId16 {
  fn eq(&self, other: &GlyphId) -> bool {
    u32::from(self.0) == other.0
  }
}

impl PartialOrd<GlyphId> for GlyphId16 {
  fn partial_cmp(&self, other: &GlyphId) -> Option<core::cmp::Ordering> {
    Some(u32::from(self.0).cmp(&other.0))
  }
}

impl TryFrom<GlyphId> for GlyphId16 {
  type Error = TryFromGlyphIdError;

  fn try_from(value: GlyphId) -> Result<Self, Self::Error> {
    let narrow = u16::try_from(value.0).map_err(|_too_wide| TryFromGlyphIdError(value.0))?;

    Ok(Self(narrow))
  }
}

impl TryFrom<GlyphId> for GlyphId24 {
  type Error = TryFromGlyphIdError;

  fn try_from(value: GlyphId) -> Result<Self, Self::Error> {
    let Ok(narrow) = Self::checked_new(value.0);

    narrow.ok_or(TryFromGlyphIdError(value.0))
  }
}

#[derive(Debug)]
pub struct TryFromGlyphIdError(u32);

impl core::fmt::Display for TryFromGlyphIdError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "glyph identifier {} too large for conversion", self.0)
  }
}

impl core::error::Error for TryFromGlyphIdError {}
