//! Streaming forgiving Base64 decoding, with invalid input and writer failures
//! kept distinct. Padding is accepted only after the alphabet has ended.

use alloc::vec::Vec;
use console_core_never::Never;
use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidBase64(InvalidBase64Details);

impl fmt::Display for InvalidBase64 {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self.0 {
      InvalidBase64Details::UnexpectedSymbol(symbol) => {
        write!(to, "symbol with codepoint {symbol} not expected")
      }
      InvalidBase64Details::AlphabetSymbolAfterPadding => {
        write!(to, "alphabet symbol present after padding")
      }
      InvalidBase64Details::LoneAlphabetSymbol => write!(to, "lone alphabet symbol present"),
      InvalidBase64Details::Padding => write!(to, "incorrect padding"),
    }
  }
}

#[cfg(feature = "std")]
impl std::error::Error for InvalidBase64 {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InvalidBase64Details {
  UnexpectedSymbol(u8),
  AlphabetSymbolAfterPadding,
  LoneAlphabetSymbol,
  Padding,
}

#[derive(Debug)]
pub enum DecodeError<E> {
  InvalidBase64(InvalidBase64),
  WriteError(E),
}

impl<E: fmt::Display> fmt::Display for DecodeError<E> {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::InvalidBase64(inner) => write!(to, "base64 not valid: {inner}"),
      Self::WriteError(fault) => write!(to, "write error: {fault}"),
    }
  }
}

#[cfg(feature = "std")]
impl<E: std::error::Error> std::error::Error for DecodeError<E> {}

impl<E> From<InvalidBase64Details> for DecodeError<E> {
  fn from(details: InvalidBase64Details) -> Self {
    Self::InvalidBase64(InvalidBase64(details))
  }
}

impl From<DecodeError<Never>> for InvalidBase64 {
  fn from(fault: DecodeError<Never>) -> Self {
    match fault {
      DecodeError::InvalidBase64(invalid) => invalid,
      DecodeError::WriteError(never) => never.absurd(),
    }
  }
}

pub fn decode_to_vec(input: &[u8]) -> Result<Vec<u8>, InvalidBase64> {
  let mut bytes = Vec::new();
  let output = &mut bytes;
  let Ok(mut decoder) = Decoder::new(move |part| {
    output.extend_from_slice(part);

    Ok::<(), Never>(())
  });

  decoder.feed(input)?;
  decoder.finish()?;

  Ok(bytes)
}

pub struct Decoder<F, E>
where
  F: FnMut(&[u8]) -> Result<(), E>,
{
  write_bytes: F,
  bit_buffer: u32,
  buffer_bit_length: u8,
  padding_symbols: u8,
}

impl<F, E> Decoder<F, E>
where
  F: FnMut(&[u8]) -> Result<(), E>,
{
  pub fn new(write_bytes: F) -> Result<Self, Never> {
    Ok(Self {
      write_bytes,
      bit_buffer: 0,
      buffer_bit_length: 0,
      padding_symbols: 0,
    })
  }

  pub fn feed(&mut self, input: &[u8]) -> Result<(), DecodeError<E>> {
    for &byte in input {
      match byte {
        b'A'..=b'Z' => self.alphabet(byte.wrapping_sub(b'A'))?,
        b'a'..=b'z' => self.alphabet(byte.wrapping_sub(b'a').wrapping_add(26))?,
        b'0'..=b'9' => self.alphabet(byte.wrapping_sub(b'0').wrapping_add(52))?,
        b'+' => self.alphabet(62)?,
        b'/' => self.alphabet(63)?,
        b'=' => self.padding_symbols = self.padding_symbols.saturating_add(1),
        b' ' | b'\t' | b'\n' | b'\r' | b'\x0C' => {}
        other => {
          return Err(DecodeError::from(InvalidBase64Details::UnexpectedSymbol(
            other,
          )));
        }
      }
    }

    Ok(())
  }

  fn alphabet(&mut self, value: u8) -> Result<(), DecodeError<E>> {
    match self.padding_symbols {
      0 => {}
      _padding => {
        return Err(DecodeError::from(
          InvalidBase64Details::AlphabetSymbolAfterPadding,
        ));
      }
    }

    self.bit_buffer = self.bit_buffer.wrapping_shl(6) | u32::from(value);

    match self.buffer_bit_length {
      ..18 => self.buffer_bit_length = self.buffer_bit_length.saturating_add(6),
      18.. => {
        let [_high, first, second, third] = self.bit_buffer.to_be_bytes();

        (self.write_bytes)(&[first, second, third]).map_err(DecodeError::WriteError)?;

        self.buffer_bit_length = 0;
      }
    }

    Ok(())
  }

  pub fn finish(mut self) -> Result<(), DecodeError<E>> {
    match (self.buffer_bit_length, self.padding_symbols) {
      (0, 0) => {}
      (12, 2) | (12, 0) => {
        let [_first, _second, _third, last] = self.bit_buffer.wrapping_shr(4).to_be_bytes();

        (self.write_bytes)(&[last]).map_err(DecodeError::WriteError)?;
      }
      (18, 1) | (18, 0) => {
        let [_first, _second, _third, first] = self.bit_buffer.wrapping_shr(10).to_be_bytes();
        let [_first, _second, _third, second] = self.bit_buffer.wrapping_shr(2).to_be_bytes();

        (self.write_bytes)(&[first, second]).map_err(DecodeError::WriteError)?;
      }
      (6, _) => return Err(DecodeError::from(InvalidBase64Details::LoneAlphabetSymbol)),
      _invalid_padding => return Err(DecodeError::from(InvalidBase64Details::Padding)),
    }

    Ok(())
  }
}
