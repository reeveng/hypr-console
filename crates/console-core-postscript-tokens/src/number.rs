//! A number: an integer, a real, or an integer in a base of its own.
//!
//! A sign, digits, a point and more digits, and an exponent, each where it
//! may be and none required but a digit on one side of the point, and then a
//! boundary, or the token is no number. `8#777` is an integer written in the
//! base before the `#`, from two to thirty-six, with no sign. What the token
//! is worth is the standard library's reading of it, as it was the fork's: a
//! real is an `f32`, and an integer that does not fit an `i32` is a fault.

use console_core_never::Never;
use console_core_number_conversion::toward_zero_i32;

use crate::ScanError;
use crate::bytes::{Follows, Walk, split_while, what_follows};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Number {
  Integer(i32),
  Real(f32),
}

impl Number {
  pub fn as_i32(self) -> Result<i32, Never> {
    match self {
      Number::Integer(value) => Ok(value),
      Number::Real(value) => toward_zero_i32(f64::from(value)),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
  Written,
  Unwritten,
}

#[derive(Debug, Clone, Copy)]
struct Lead<'a> {
  token: &'a [u8],
  digits: &'a [u8],
  rest: &'a [u8],
}

#[derive(Debug, Clone, Copy)]
struct Radix<'a> {
  base: &'a [u8],
  after_hash: &'a [u8],
}

pub(crate) fn read(token: &[u8]) -> Result<(Number, &[u8]), ScanError> {
  let (sign, unsigned) = match token {
    [b'+' | b'-', after @ ..] => (Part::Written, after),
    unsigned => (Part::Unwritten, unsigned),
  };
  let Ok((digits, rest)) = split_while(unsigned, |byte| Follows::from(byte.is_ascii_digit()));

  match (sign, digits, rest) {
    (Part::Unwritten, [_digit, ..], [b'#', after_hash @ ..]) => radix(Radix {
      base: digits,
      after_hash,
    }),
    (Part::Unwritten | Part::Written, _digits, _rest) => decimal(Lead {
      token,
      digits,
      rest,
    }),
  }
}

fn decimal(lead: Lead<'_>) -> Result<(Number, &[u8]), ScanError> {
  let Ok((point, after_fraction)) = fraction(lead.rest);

  match (lead.digits, point) {
    ([], Part::Unwritten) => return Err(ScanError::SyntaxError),
    ([], Part::Written) => {}
    ([_digit, ..], Part::Written | Part::Unwritten) => {}
  }

  let Ok((exponent, rest)) = exponent(after_fraction);

  match what_follows(rest) {
    Ok(Follows::Boundary) => {}
    Ok(Follows::More) => return Err(ScanError::SyntaxError),
  }

  let Ok(written) = (Walk {
    from: lead.token,
    rest,
  })
  .walked();
  let text = std::str::from_utf8(written).map_err(|_not_text| ScanError::SyntaxError)?;

  let number = match (point, exponent) {
    (Part::Unwritten, Part::Unwritten) => text
      .parse::<i32>()
      .map(Number::Integer)
      .map_err(|_unread| ScanError::SyntaxError),
    (Part::Written, Part::Written | Part::Unwritten) | (Part::Unwritten, Part::Written) => text
      .parse::<f32>()
      .map(Number::Real)
      .map_err(|_unread| ScanError::SyntaxError),
  };

  number.map(|found| (found, rest))
}

fn fraction(rest: &[u8]) -> Result<(Part, &[u8]), Never> {
  Ok(match rest {
    [b'.', after @ ..] => {
      let Ok((_fraction, after_fraction)) =
        split_while(after, |byte| Follows::from(byte.is_ascii_digit()));

      (Part::Written, after_fraction)
    }
    unpointed => (Part::Unwritten, unpointed),
  })
}

fn exponent(rest: &[u8]) -> Result<(Part, &[u8]), Never> {
  Ok(match rest {
    [b'e' | b'E', after @ ..] => {
      let unsigned = match after {
        [b'+' | b'-', digits @ ..] => digits,
        digits => digits,
      };
      let Ok((_exponent, after_exponent)) =
        split_while(unsigned, |byte| Follows::from(byte.is_ascii_digit()));

      (Part::Written, after_exponent)
    }
    unraised => (Part::Unwritten, unraised),
  })
}

fn radix(written: Radix<'_>) -> Result<(Number, &[u8]), ScanError> {
  let base_text =
    std::str::from_utf8(written.base).map_err(|_not_text| ScanError::SyntaxError)?;
  let base = base_text
    .parse::<u32>()
    .map_err(|_unread| ScanError::SyntaxError)?;

  match base {
    2..=36 => {}
    _outside => return Err(ScanError::SyntaxError),
  }

  let Ok((value, rest)) = split_while(written.after_hash, |byte| {
    Follows::from(byte.is_ascii_alphanumeric())
  });

  match (value, what_follows(rest)) {
    ([_digit, ..], Ok(Follows::Boundary)) => {}
    ([], Ok(Follows::Boundary | Follows::More)) => return Err(ScanError::SyntaxError),
    ([_digit, ..], Ok(Follows::More)) => return Err(ScanError::SyntaxError),
  }

  let value_text = std::str::from_utf8(value).map_err(|_not_text| ScanError::SyntaxError)?;
  let integer =
    i32::from_str_radix(value_text, base).map_err(|_unread| ScanError::SyntaxError)?;

  Ok((Number::Integer(integer), rest))
}
