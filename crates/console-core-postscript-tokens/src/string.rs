//! Where a string begins and ends, and the bytes it holds when they are asked
//! for.
//!
//! A literal string closes at the parenthesis that balances its opening one,
//! with a backslash taking the byte after it out of the count; a hexadecimal
//! string at the first `>`; an ASCII85 string at the first `~`, which must be
//! followed by `>`. Finding the end reads nothing inside, so a string that
//! nobody decodes costs one walk over it and a string that will not decode
//! says so only when its bytes are wanted.

use console_core_number_conversion::fitted;

use crate::ScanError;
use crate::bytes::Walk;
use crate::decoding;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Encoding {
  Literal,
  Hexadecimal,
  Ascii85,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct String<'a> {
  body: &'a [u8],
  encoding: Encoding,
}

impl String<'_> {
  pub fn decode_into(&self, out: &mut Vec<u8>) -> Result<(), ScanError> {
    out.clear();

    match self.encoding {
      Encoding::Literal => decoding::literal(self.body, out),
      Encoding::Hexadecimal => decoding::hexadecimal(self.body, out),
      Encoding::Ascii85 => decoding::ascii85(self.body, out),
    }
  }

  pub fn decode(&self) -> Result<Vec<u8>, ScanError> {
    let mut decoded = Vec::new();

    self.decode_into(&mut decoded)?;

    Ok(decoded)
  }
}

pub(crate) fn delimited(
  after_open: &[u8],
  encoding: Encoding,
) -> Result<(String<'_>, &[u8]), ScanError> {
  let parts = match encoding {
    Encoding::Literal => literal(after_open),
    Encoding::Hexadecimal => hexadecimal(after_open),
    Encoding::Ascii85 => ascii85(after_open),
  };

  parts.map(|(body, rest)| (String { body, encoding }, rest))
}

fn literal(after_open: &[u8]) -> Result<(&[u8], &[u8]), ScanError> {
  let Ok(bound) = fitted::<_, u64>(after_open.len());
  let mut depth = 1u32;
  let mut rest = after_open;

  for _byte in 0..=bound {
    match rest {
      [] | [b'\\'] => return Err(ScanError::SyntaxError),
      [b'\\', _escaped, after @ ..] => rest = after,
      [b'(', after @ ..] => {
        depth = depth.saturating_add(1);
        rest = after;
      }
      [b')', after @ ..] => {
        depth = depth.saturating_sub(1);

        match depth {
          0 => {
            let Ok(body) = (Walk {
              from: after_open,
              rest,
            })
            .walked();

            return Ok((body, after));
          }
          _open => rest = after,
        }
      }
      [_byte, after @ ..] => rest = after,
    }
  }

  Err(ScanError::SyntaxError)
}

fn closed_by(bytes: &[u8], close: u8) -> Result<(&[u8], &[u8]), ScanError> {
  match bytes
    .iter()
    .position(|byte| *byte == close)
    .and_then(|end| bytes.split_at_checked(end))
  {
    Some(parts) => Ok(parts),
    None => Err(ScanError::SyntaxError),
  }
}

fn hexadecimal(after_open: &[u8]) -> Result<(&[u8], &[u8]), ScanError> {
  let (body, closed) = closed_by(after_open, b'>')?;

  match closed {
    [_close, rest @ ..] => Ok((body, rest)),
    [] => Err(ScanError::SyntaxError),
  }
}

fn ascii85(after_open: &[u8]) -> Result<(&[u8], &[u8]), ScanError> {
  let (body, closed) = closed_by(after_open, b'~')?;

  match closed {
    [b'~', b'>', rest @ ..] => Ok((body, rest)),
    _unclosed => Err(ScanError::SyntaxError),
  }
}

pub(crate) fn skipped(after_open: &[u8], encoding: Encoding) -> Result<&[u8], ScanError> {
  let (_string, rest) = delimited(after_open, encoding)?;

  Ok(rest)
}
