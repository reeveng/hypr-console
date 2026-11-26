//! Streaming forgiving Base64 decoding, with invalid input and writer failures
//! kept distinct. Padding is accepted only after the alphabet has ended.
//! Unpadded decoding requires only alphabet characters. Display streams
//! standard padded Base64 directly to the formatter.

use alloc::vec::Vec;
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};
use core::fmt;

use crate::Bytes;

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

impl core::error::Error for InvalidBase64 {}

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

impl<E: core::error::Error> core::error::Error for DecodeError<E> {}

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
  let bytes = decode(input)?;
  let Ok(output) = bytes.into_vec();

  Ok(output)
}

pub fn decode(input: &[u8]) -> Result<Bytes, InvalidBase64> {
  decode_with(input, Whitespace::Forgiving)
}

pub fn decode_unpadded(input: &[u8]) -> Result<Bytes, InvalidBase64> {
  decode_with(input, Whitespace::Reject)
}

enum Whitespace {
  Forgiving,
  Reject,
}

fn decode_with(input: &[u8], mode: Whitespace) -> Result<Bytes, InvalidBase64> {
  let mut bytes = Vec::new();
  let output = &mut bytes;
  let Ok(mut decoder) = Decoder::new(move |part| {
    output.extend_from_slice(part);

    Ok::<(), Never>(())
  });

  match mode {
    Whitespace::Forgiving => decoder.feed(input)?,
    Whitespace::Reject => {
      for &byte in input {
        decoder.symbol(byte)?;
      }
    }
  }

  decoder.finish()?;

  Ok(Bytes::from(bytes))
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
        b'=' => self.padding_symbols = self.padding_symbols.saturating_add(1),
        b' ' | b'\t' | b'\n' | b'\r' | b'\x0C' => {}
        symbol => self.symbol(symbol)?,
      }
    }

    Ok(())
  }

  fn symbol(&mut self, byte: u8) -> Result<(), DecodeError<E>> {
    let value = match byte {
      b'A'..=b'Z' => byte.wrapping_sub(b'A'),
      b'a'..=b'z' => byte.wrapping_sub(b'a').wrapping_add(26),
      b'0'..=b'9' => byte.wrapping_sub(b'0').wrapping_add(52),
      b'+' => 62,
      b'/' => 63,
      other => {
        return Err(DecodeError::from(InvalidBase64Details::UnexpectedSymbol(
          other,
        )));
      }
    };

    self.alphabet(value)
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

#[must_use]
pub fn display(bytes: &[u8]) -> Result<impl fmt::Display + '_, Never> {
  Ok(Encoded(bytes))
}

struct Encoded<'a>(&'a [u8]);

impl fmt::Display for Encoded<'_> {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    let (groups, tail) = self.0.as_chunks::<3>();

    let mut buffer = [0; 1024];
    let (output, _) = buffer.as_chunks_mut::<4>();

    for block in groups.chunks(256) {
      for (&group, slot) in block.iter().zip(output.iter_mut()) {
        let Ok(encoded) = quartet(group, Padding::None);
        *slot = encoded;
      }

      let Ok(group_count) = fixed_index(block.len());
      let Ok(written) = index(group_count.saturating_mul(4));
      let portion = output.as_flattened().get(..written).ok_or(fmt::Error)?;

      write_ascii(to, portion)?;
    }

    let Ok(last) = match tail {
      [a] => quartet([*a, 0, 0], Padding::Two),
      [a, b] => quartet([*a, *b, 0], Padding::One),
      [] | [_, _, _, ..] => return Ok(()),
    };

    write_ascii(to, &last)
  }
}

enum Padding {
  None,
  One,
  Two,
}

#[must_use]
fn quartet([a, b, c]: [u8; 3], padding: Padding) -> Result<[u8; 4], Never> {
  let Ok(first) = encoded_symbol(a.wrapping_shr(2));
  let Ok(second) = encoded_symbol((a & 3).wrapping_shl(4) | b.wrapping_shr(4));
  let Ok(third) = encoded_symbol((b & 15).wrapping_shl(2) | c.wrapping_shr(6));
  let Ok(fourth) = encoded_symbol(c & 63);

  Ok(match padding {
    Padding::None => [first, second, third, fourth],
    Padding::One => [first, second, third, b'='],
    Padding::Two => [first, second, b'=', b'='],
  })
}

#[must_use]
fn encoded_symbol(value: u8) -> Result<u8, Never> {
  Ok(match value {
    0..=25 => b'A'.wrapping_add(value),
    26..=51 => b'a'.wrapping_add(value.wrapping_sub(26)),
    52..=61 => b'0'.wrapping_add(value.wrapping_sub(52)),
    62 => b'+',
    _ => b'/',
  })
}

fn write_ascii(to: &mut fmt::Formatter<'_>, group: &[u8]) -> fmt::Result {
  match core::str::from_utf8(group) {
    Ok(text) => to.write_str(text),
    Err(_outside_alphabet) => Err(fmt::Error),
  }
}
