use alloc::{string::FromUtf8Error, string::String, vec::Vec};
use core::fmt;
use core::ops::Deref;

use console_core_never::Never;

#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct Bytes(Vec<u8>);

impl Bytes {
  #[must_use]
  pub fn copy_from_slice(bytes: &[u8]) -> Result<Self, Never> {
    Ok(Self(bytes.to_vec()))
  }

  #[must_use]
  pub fn into_vec(self) -> Result<Vec<u8>, Never> {
    Ok(self.0)
  }

  pub fn into_utf8(self) -> Result<String, FromUtf8Error> {
    String::from_utf8(self.0)
  }
}

impl From<Vec<u8>> for Bytes {
  fn from(bytes: Vec<u8>) -> Self {
    Self(bytes)
  }
}

impl From<String> for Bytes {
  fn from(text: String) -> Self {
    Self(text.into_bytes())
  }
}

impl AsRef<[u8]> for Bytes {
  fn as_ref(&self) -> &[u8] {
    &self.0
  }
}

impl Deref for Bytes {
  type Target = [u8];

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl fmt::Debug for Bytes {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    to.debug_struct("Bytes")
      .field("len", &self.0.len())
      .finish()
  }
}
