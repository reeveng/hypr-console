//! Four-character identifiers carried by base media boxes.
//!
//! A code names a box's type, a sample entry's format, a brand or a handler, and it is kept as
//! the four bytes the file wrote, so comparing one is comparing bytes and nothing is decoded
//! on the way in. Text is made only where a person reads it: `©` (0xA9), which the iTunes item
//! names begin with, becomes U+00A9, and every other byte outside printable ASCII becomes `?`.
use alloc::string::String;
use core::fmt;

use console_core_never::Never;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct FourCharacterCode(pub [u8; 4]);

impl FourCharacterCode {
  pub const fn new(code: &[u8; 4]) -> Result<Self, Never> {
    Ok(FourCharacterCode(*code))
  }

  pub fn as_u32(self) -> Result<u32, Never> {
    Ok(u32::from_be_bytes(self.0))
  }

  pub fn to_string_lossy(self) -> Result<String, Never> {
    Ok(
      self
        .0
        .into_iter()
        .map(|byte| match byte {
          0xA9 => '©',
          0x20..=0x7E => char::from(byte),
          _unprintable => '?',
        })
        .collect::<String>(),
    )
  }
}

impl fmt::Display for FourCharacterCode {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    let Ok(shown) = self.to_string_lossy();

    to.write_str(&shown)
  }
}

impl fmt::Debug for FourCharacterCode {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    let Ok(shown) = self.to_string_lossy();

    write!(to, "FourCharacterCode({shown:?})")
  }
}

impl From<&[u8; 4]> for FourCharacterCode {
  fn from(code: &[u8; 4]) -> Self {
    FourCharacterCode(*code)
  }
}

impl PartialEq<[u8; 4]> for FourCharacterCode {
  fn eq(&self, other: &[u8; 4]) -> bool {
    &self.0 == other
  }
}

impl PartialEq<&[u8; 4]> for FourCharacterCode {
  fn eq(&self, other: &&[u8; 4]) -> bool {
    &self.0 == *other
  }
}

impl PartialEq<&&[u8; 4]> for FourCharacterCode {
  fn eq(&self, other: &&&[u8; 4]) -> bool {
    &self.0 == **other
  }
}
