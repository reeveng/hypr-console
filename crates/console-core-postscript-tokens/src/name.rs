//! A name, literal after a slash or executable without one, and the bytes it
//! stands for once its `#` escapes are read.
//!
//! A name is every byte up to the next white space or delimiter. A literal
//! name may be empty, as `/` alone is; an executable one is at least a byte,
//! because nothing at all is not a token. `#` and two hexadecimal digits is
//! the byte they spell, the way PDF writes a name, and `#` without them is a
//! fault rather than itself.

use console_core_never::Never;
use console_core_number_conversion::fitted;
use std::str::Utf8Error;

use crate::ScanError;
use crate::bytes::{Class, Follows, class, split_while};
use crate::decoding::hex_digit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameKind {
  Literal,
  Executable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Name<'a> {
  body: &'a [u8],
  kind: NameKind,
}

impl<'a> Name<'a> {
  pub fn kind(&self) -> Result<NameKind, Never> {
    Ok(self.kind)
  }

  pub fn as_str(&self) -> Result<&'a str, Utf8Error> {
    std::str::from_utf8(self.body)
  }

  pub fn decode(&self) -> Result<Vec<u8>, ScanError> {
    let Ok(bound) = fitted::<_, u64>(self.body.len());
    let mut decoded = Vec::with_capacity(self.body.len());
    let mut rest = self.body;

    for _byte in 0..=bound {
      match rest {
        [] => return Ok(decoded),
        [b'#', high, low, after @ ..] => {
          let high_digit = hex_digit(*high)?;
          let low_digit = hex_digit(*low)?;

          decoded.push(high_digit.wrapping_shl(4) | low_digit);
          rest = after;
        }
        [b'#', ..] => return Err(ScanError::SyntaxError),
        [byte, after @ ..] => {
          decoded.push(*byte);
          rest = after;
        }
      }
    }

    Ok(decoded)
  }
}

pub(crate) fn literal(after_slash: &[u8]) -> Result<(Name<'_>, &[u8]), Never> {
  let Ok((body, rest)) = split_while(after_slash, |byte| {
    Follows::from(class(byte) == Ok(Class::Regular))
  });

  Ok((
    Name {
      body,
      kind: NameKind::Literal,
    },
    rest,
  ))
}

pub(crate) fn executable(start: &[u8]) -> Result<(Name<'_>, &[u8]), ScanError> {
  let Ok((body, rest)) = split_while(start, |byte| {
    Follows::from(class(byte) == Ok(Class::Regular))
  });

  match body {
    [] => Err(ScanError::SyntaxError),
    [_first, ..] => Ok((
      Name {
        body,
        kind: NameKind::Executable,
      },
      rest,
    )),
  }
}
