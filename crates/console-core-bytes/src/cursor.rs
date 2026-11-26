//! Checked borrowed reads. A failed read leaves the remaining slice intact.

use core::fmt;

use console_core_never::Never;
use console_core_number_conversion::checked_index;

use crate::endian;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Truncated;

impl fmt::Display for Truncated {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    to.write_str("the input ended before the requested bytes")
  }
}

impl core::error::Error for Truncated {}

#[derive(Clone, Copy, Debug)]
pub struct Cursor<'a> {
  rest: &'a [u8],
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "paired endian readers all advance through the same checked fixed-width read"
  )
)]
macro_rules! readers {
  ($(($big:ident, $little:ident, $number:ty)),+ $(,)?) => {
    $(
      pub fn $big(&mut self) -> Result<$number, Truncated> {
        let bytes = self.array()?;

        Ok(<$number>::from_be_bytes(bytes))
      }

      pub fn $little(&mut self) -> Result<$number, Truncated> {
        let bytes = self.array()?;

        Ok(<$number>::from_le_bytes(bytes))
      }
    )+
  };
}

impl<'a> Cursor<'a> {
  #[must_use]
  pub const fn new(bytes: &'a [u8]) -> Result<Self, Never> {
    Ok(Self { rest: bytes })
  }

  #[must_use]
  pub const fn rest(&self) -> Result<&'a [u8], Never> {
    Ok(self.rest)
  }

  #[must_use]
  pub fn take_rest(&mut self) -> Result<&'a [u8], Never> {
    Ok(core::mem::take(&mut self.rest))
  }

  pub fn take(&mut self, count: u64) -> Result<&'a [u8], Truncated> {
    let at = checked_index(count).map_err(|_outside_slice_range| Truncated)?;
    let (taken, after) = self.rest.split_at_checked(at).ok_or(Truncated)?;

    self.rest = after;

    Ok(taken)
  }

  pub fn take_until(&mut self, delimiter: u8) -> Result<&'a [u8], Truncated> {
    let (taken, tail) = self
      .rest
      .iter()
      .position(|byte| *byte == delimiter)
      .and_then(|end| self.rest.split_at_checked(end))
      .ok_or(Truncated)?;
    let (_, after) = tail.split_first().ok_or(Truncated)?;

    self.rest = after;

    Ok(taken)
  }

  pub fn array<const WIDTH: usize>(&mut self) -> Result<[u8; WIDTH], Truncated> {
    let (taken, after) = self.rest.split_first_chunk().ok_or(Truncated)?;

    self.rest = after;

    Ok(*taken)
  }

  pub fn byte(&mut self) -> Result<u8, Truncated> {
    let [byte] = self.array()?;

    Ok(byte)
  }

  readers!(
    (u16_be, u16_le, u16),
    (i16_be, i16_le, i16),
    (u32_be, u32_le, u32),
    (i32_be, i32_le, i32),
    (u64_be, u64_le, u64),
    (i64_be, i64_le, i64),
    (f32_be, f32_le, f32),
    (f64_be, f64_le, f64),
  );

  pub fn u24_be(&mut self) -> Result<u32, Truncated> {
    let bytes = self.array()?;
    let Ok(value) = endian::u24_be(bytes);

    Ok(value)
  }

  pub fn u24_le(&mut self) -> Result<u32, Truncated> {
    let bytes = self.array()?;
    let Ok(value) = endian::u24_le(bytes);

    Ok(value)
  }

  pub fn i24_be(&mut self) -> Result<i32, Truncated> {
    let bytes = self.array()?;
    let Ok(value) = endian::i24_be(bytes);

    Ok(value)
  }

  pub fn i24_le(&mut self) -> Result<i32, Truncated> {
    let bytes = self.array()?;
    let Ok(value) = endian::i24_le(bytes);

    Ok(value)
  }
}
