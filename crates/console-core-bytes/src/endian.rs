//! Three-byte integers, extended to their signed or unsigned 32-bit form.

use console_core_never::Never;

#[must_use]
pub const fn u24_be([high, middle, low]: [u8; 3]) -> Result<u32, Never> {
  Ok(u32::from_be_bytes([0, high, middle, low]))
}

#[must_use]
pub const fn u24_le([low, middle, high]: [u8; 3]) -> Result<u32, Never> {
  Ok(u32::from_le_bytes([low, middle, high, 0]))
}

#[must_use]
pub const fn i24_be([high, middle, low]: [u8; 3]) -> Result<i32, Never> {
  Ok(i32::from_be_bytes([high, middle, low, 0]).wrapping_shr(8))
}

#[must_use]
pub const fn i24_le([low, middle, high]: [u8; 3]) -> Result<i32, Never> {
  Ok(i32::from_le_bytes([0, low, middle, high]).wrapping_shr(8))
}
