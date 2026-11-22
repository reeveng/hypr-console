//! Where a table is: a count of bytes from the start of whatever holds the offset, at the three
//! widths a font writes one in.
//!
//! Zero is where a table would be if it were the thing holding the offset, so zero is the null
//! offset -- the table is not there. Some fields may be null and some may not, and the field's type
//! says which: a [`Nullable`] offset is one the specification allows to be zero, and a bare one is
//! read as though it cannot be, which is the reader's to check. Whether one is null is asked as a
//! [`Nullness`], so the caller names the case it is in. The three widths are written out rather
//! than generated, because each is a few lines and a macro would be the longer of the two.

use crate::{Scalar, Uint24};
use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bytemuck", derive(bytemuck::AnyBitPattern))]
#[repr(transparent)]
pub struct Nullable<T>(T);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Nullness {
  Null,
  NonNull,
}

impl From<bool> for Nullness {
  fn from(zero: bool) -> Self {
    match zero.then_some(()) {
      Some(()) => Self::Null,
      None => Self::NonNull,
    }
  }
}

impl Nullness {
  const fn of(offset: u32) -> Result<Nullness, Never> {
    Ok(match offset {
      0 => Nullness::Null,
      1.. => Nullness::NonNull,
    })
  }
}

trait NullValue {
  const NULL: Self;
}

impl<T: Scalar> Scalar for Nullable<T> {
  type Raw = T::Raw;

  #[inline]
  fn from_raw(raw: Self::Raw) -> Result<Self, Never> {
    let Ok(offset) = T::from_raw(raw);

    Ok(Self(offset))
  }

  #[inline]
  fn to_raw(self) -> Result<Self::Raw, Never> {
    self.0.to_raw()
  }
}

impl<T> Nullable<T> {
  #[inline]
  pub fn offset(&self) -> Result<&T, Never> {
    Ok(&self.0)
  }
}

impl<T: PartialEq<u32>> Nullable<T> {
  #[inline]
  pub fn nullness(&self) -> Result<Nullness, Never> {
    Ok(Nullness::from(self.0 == 0))
  }
}

impl<T: PartialEq<u32>> PartialEq<u32> for Nullable<T> {
  #[inline]
  fn eq(&self, other: &u32) -> bool {
    self.0 == *other
  }
}

impl<T: NullValue> Default for Nullable<T> {
  fn default() -> Self {
    Self(T::NULL)
  }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bytemuck", derive(bytemuck::AnyBitPattern))]
#[repr(transparent)]
pub struct Offset16(u16);

impl Offset16 {
  #[inline]
  pub const fn new(raw: u16) -> Result<Self, Never> {
    Ok(Self(raw))
  }

  #[inline]
  pub fn nullness(self) -> Result<Nullness, Never> {
    Nullness::of(u32::from(self.0))
  }

  #[inline]
  pub fn to_u32(self) -> Result<u32, Never> {
    Ok(u32::from(self.0))
  }
}

impl Scalar for Offset16 {
  type Raw = [u8; 2];

  fn from_raw(raw: Self::Raw) -> Result<Self, Never> {
    Ok(Self(u16::from_be_bytes(raw)))
  }

  fn to_raw(self) -> Result<Self::Raw, Never> {
    Ok(self.0.to_be_bytes())
  }
}

impl PartialEq<u32> for Offset16 {
  fn eq(&self, other: &u32) -> bool {
    u32::from(self.0) == *other
  }
}

impl NullValue for Offset16 {
  const NULL: Offset16 = Offset16(0);
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bytemuck", derive(bytemuck::AnyBitPattern))]
#[repr(transparent)]
pub struct Offset24(Uint24);

impl Offset24 {
  #[inline]
  pub const fn new(raw: Uint24) -> Result<Self, Never> {
    Ok(Self(raw))
  }

  #[inline]
  pub fn nullness(self) -> Result<Nullness, Never> {
    Nullness::of(u32::from(self.0))
  }

  #[inline]
  pub fn to_u32(self) -> Result<u32, Never> {
    Ok(u32::from(self.0))
  }
}

impl Scalar for Offset24 {
  type Raw = [u8; 3];

  fn from_raw(raw: Self::Raw) -> Result<Self, Never> {
    let Ok(held) = Uint24::from_be_bytes(raw);

    Ok(Self(held))
  }

  fn to_raw(self) -> Result<Self::Raw, Never> {
    self.0.to_be_bytes()
  }
}

impl PartialEq<u32> for Offset24 {
  fn eq(&self, other: &u32) -> bool {
    u32::from(self.0) == *other
  }
}

impl NullValue for Offset24 {
  const NULL: Offset24 = Offset24(Uint24::MIN);
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bytemuck", derive(bytemuck::AnyBitPattern))]
#[repr(transparent)]
pub struct Offset32(u32);

impl Offset32 {
  #[inline]
  pub const fn new(raw: u32) -> Result<Self, Never> {
    Ok(Self(raw))
  }

  #[inline]
  pub fn nullness(self) -> Result<Nullness, Never> {
    Nullness::of(self.0)
  }

  #[inline]
  pub fn to_u32(self) -> Result<u32, Never> {
    Ok(self.0)
  }
}

impl Scalar for Offset32 {
  type Raw = [u8; 4];

  fn from_raw(raw: Self::Raw) -> Result<Self, Never> {
    Ok(Self(u32::from_be_bytes(raw)))
  }

  fn to_raw(self) -> Result<Self::Raw, Never> {
    Ok(self.0.to_be_bytes())
  }
}

impl PartialEq<u32> for Offset32 {
  fn eq(&self, other: &u32) -> bool {
    self.0 == *other
  }
}

impl NullValue for Offset32 {
  const NULL: Offset32 = Offset32(0);
}
