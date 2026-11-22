//! A date as a font writes it: seconds since midnight at the start of 1904, in UTC, which is
//! where the Macintosh clock began. It can be negative, for a date before then.

use console_core_never::Never;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bytemuck", derive(bytemuck::AnyBitPattern))]
#[repr(transparent)]
pub struct LongDateTime(i64);

impl LongDateTime {
  pub const fn new(secs: i64) -> Result<Self, Never> {
    Ok(Self(secs))
  }

  pub const fn as_secs(&self) -> Result<i64, Never> {
    Ok(self.0)
  }

  pub const fn to_be_bytes(self) -> Result<[u8; 8], Never> {
    Ok(self.0.to_be_bytes())
  }
}

crate::raw::newtype_scalar!(LongDateTime, [u8; 8]);
