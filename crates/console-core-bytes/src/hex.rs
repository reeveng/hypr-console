//! Strict ASCII hexadecimal digits, pairs and whole byte sequences.

#[cfg(feature = "alloc")]
use alloc::{string::String, vec::Vec};
use core::fmt;

use console_core_never::Never;

#[cfg(feature = "alloc")]
use crate::Bytes;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
  Lower,
  Upper,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidDigit(pub u8);

impl fmt::Display for InvalidDigit {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      to,
      "byte {} is outside the ASCII hexadecimal alphabet",
      self.0
    )
  }
}

impl core::error::Error for InvalidDigit {}

pub const fn decode_digit(byte: u8) -> Result<u8, InvalidDigit> {
  match byte {
    b'0'..=b'9' => Ok(byte.wrapping_sub(b'0')),
    b'A'..=b'F' => Ok(byte.wrapping_sub(b'A').wrapping_add(10)),
    b'a'..=b'f' => Ok(byte.wrapping_sub(b'a').wrapping_add(10)),
    _ => Err(InvalidDigit(byte)),
  }
}

pub fn decode_pair([high, low]: [u8; 2]) -> Result<u8, InvalidDigit> {
  let high_nibble = decode_digit(high)?;
  let low_nibble = decode_digit(low)?;

  Ok(high_nibble.wrapping_shl(4) | low_nibble)
}

#[must_use]
pub const fn encode_byte(byte: u8, case: Case) -> Result<[u8; 2], Never> {
  let Ok(high) = digit(byte.wrapping_shr(4), case);
  let Ok(low) = digit(byte & 0x0f, case);

  Ok([high, low])
}

#[must_use]
const fn digit(nibble: u8, case: Case) -> Result<u8, Never> {
  let letters = match case {
    Case::Lower => b'a',
    Case::Upper => b'A',
  };

  Ok(match nibble {
    0..=9 => b'0'.wrapping_add(nibble),
    _ => letters.wrapping_add(nibble.wrapping_sub(10)),
  })
}

#[cfg(feature = "alloc")]
pub fn append(output: &mut String, bytes: &[u8], case: Case) -> Result<(), Never> {
  for &byte in bytes {
    let Ok([high, low]) = encode_byte(byte, case);
    output.push(char::from(high));
    output.push(char::from(low));
  }

  Ok(())
}

#[cfg(feature = "alloc")]
#[must_use]
pub fn encode(bytes: &[u8], case: Case) -> Result<String, Never> {
  let mut output = String::with_capacity(bytes.len().saturating_mul(2));
  let Ok(()) = append(&mut output, bytes, case);

  Ok(output)
}

#[cfg(feature = "alloc")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
  OddLength,
  Digit(InvalidDigit),
}

#[cfg(feature = "alloc")]
impl fmt::Display for DecodeError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::OddLength => to.write_str("a hexadecimal byte needs two digits"),
      Self::Digit(fault) => fault.fmt(to),
    }
  }
}

#[cfg(feature = "alloc")]
impl core::error::Error for DecodeError {}

#[cfg(feature = "alloc")]
pub fn decode(input: &[u8]) -> Result<Bytes, DecodeError> {
  let (pairs, tail) = input.as_chunks::<2>();

  match tail {
    [] => {}
    [_] | [_, _, ..] => return Err(DecodeError::OddLength),
  }

  let mut output = Vec::with_capacity(pairs.len());

  for &pair in pairs {
    let byte = decode_pair(pair).map_err(DecodeError::Digit)?;

    output.push(byte);
  }

  Ok(Bytes::from(output))
}
