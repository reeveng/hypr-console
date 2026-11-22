//! The bytes a string holds, out of the three ways PostScript writes them.
//!
//! A literal string says its bytes as they are, with a backslash before the
//! ones it cannot: a letter for a control character, up to three octal digits
//! for any byte, and a line ending for nothing at all. A line ending without
//! one is a line feed, and a run of line endings is one, as it was in the
//! fork. A hexadecimal string is two digits a byte with white space anywhere,
//! and a last digit alone is the high half of a byte. ASCII85 is five digits
//! of base 85 for four bytes, `z` for four zeros, and a short group at the end
//! for fewer, padded out with the largest digit.
//!
//! Each is lenient where the fork was and PDF readers are: an octal escape
//! past a byte says nothing, an ASCII85 string may end without its `~`, and a
//! `z` may cut short the group before it. The PDF reader decodes the same
//! three, in its strings and its ASCII filters, and asks for them here; what
//! a string is written into is the caller's, so each takes anything a byte
//! can be added to rather than a vector of its own.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::ScanError;
use crate::bytes::{Class, Follows, Line, class, line, split_while};

const BASE_85: u64 = 85;

const LARGEST_85: u64 = 84;

const GROUP_85: u32 = 5;

pub fn literal(body: &[u8], out: &mut impl Extend<u8>) -> Result<(), ScanError> {
  let Ok(bound) = fitted::<_, u64>(body.len());
  let mut rest = body;

  for _byte in 0..=bound {
    match rest {
      [] => return Ok(()),
      [b'\\', after @ ..] => {
        let after_escape = escape(after, out)?;

        rest = after_escape;
      }
      [b'\n' | b'\r', after @ ..] => {
        let Ok(line_on) = line_ends_skipped(after);

        out.extend(std::iter::once(b'\n'));
        rest = line_on;
      }
      [byte, after @ ..] => {
        out.extend([*byte]);
        rest = after;
      }
    }
  }

  Ok(())
}

fn escape<'a>(
  after_backslash: &'a [u8],
  out: &mut impl Extend<u8>,
) -> Result<&'a [u8], ScanError> {
  match after_backslash {
    [] => Err(ScanError::SyntaxError),
    [
      high @ b'0'..=b'7',
      middle @ b'0'..=b'7',
      low @ b'0'..=b'7',
      rest @ ..,
    ] => {
      let Ok(()) = octal([*high, *middle, *low].as_slice(), out);

      Ok(rest)
    }
    [high @ b'0'..=b'7', low @ b'0'..=b'7', rest @ ..] => {
      let Ok(()) = octal([*high, *low].as_slice(), out);

      Ok(rest)
    }
    [digit @ b'0'..=b'7', rest @ ..] => {
      let Ok(()) = octal([*digit].as_slice(), out);

      Ok(rest)
    }
    [b'\n' | b'\r', rest @ ..] => {
      let Ok(line_on) = line_ends_skipped(rest);

      Ok(line_on)
    }
    [named, rest @ ..] => {
      let Ok(byte) = escaped(*named);

      out.extend([byte]);

      Ok(rest)
    }
  }
}

fn escaped(named: u8) -> Result<u8, Never> {
  Ok(match named {
    b'n' => 0x0a,
    b'r' => 0x0d,
    b't' => 0x09,
    b'b' => 0x08,
    b'f' => 0x0c,
    _itself => named,
  })
}

fn octal(digits: &[u8], out: &mut impl Extend<u8>) -> Result<(), Never> {
  let byte = digits.iter().try_fold(0u8, |value, digit| {
    value
      .checked_mul(8)
      .and_then(|high| high.checked_add(digit.wrapping_sub(b'0')))
  });

  match byte {
    Some(said) => out.extend([said]),
    None => {}
  }

  Ok(())
}

fn line_ends_skipped(rest: &[u8]) -> Result<&[u8], Never> {
  let Ok((_line_ends, line_on)) =
    split_while(rest, |byte| Follows::from(line(byte) == Ok(Line::Ends)));

  Ok(line_on)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Nibble {
  Awaited,
  Held(u8),
}

pub fn hexadecimal(body: &[u8], out: &mut impl Extend<u8>) -> Result<(), ScanError> {
  let mut nibble = Nibble::Awaited;

  for byte in body
    .iter()
    .filter(|byte| class(**byte) != Ok(Class::WhiteSpace))
  {
    let digit = hex_digit(*byte)?;

    nibble = match nibble {
      Nibble::Awaited => Nibble::Held(digit),
      Nibble::Held(high) => {
        out.extend([high.wrapping_shl(4) | digit]);

        Nibble::Awaited
      }
    };
  }

  match nibble {
    Nibble::Held(high) => out.extend([high.wrapping_shl(4)]),
    Nibble::Awaited => {}
  }

  Ok(())
}

pub(crate) fn hex_digit(byte: u8) -> Result<u8, ScanError> {
  match byte {
    b'0'..=b'9' => Ok(byte.wrapping_sub(b'0')),
    b'A'..=b'F' => Ok(byte.wrapping_sub(b'A').wrapping_add(10)),
    b'a'..=b'f' => Ok(byte.wrapping_sub(b'a').wrapping_add(10)),
    _other => Err(ScanError::SyntaxError),
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Group {
  value: u64,
  count: u32,
}

const EMPTY: Group = Group { value: 0, count: 0 };

impl Group {
  fn with(self, digit: u8) -> Result<Group, Never> {
    Ok(Group {
      value: self
        .value
        .wrapping_mul(BASE_85)
        .wrapping_add(u64::from(digit.wrapping_sub(b'!'))),
      count: self.count.saturating_add(1),
    })
  }

  fn flush(self, out: &mut impl Extend<u8>) -> Result<(), ScanError> {
    match self.count {
      0 => Ok(()),
      1 => Err(ScanError::SyntaxError),
      written => {
        let padded = (written..GROUP_85).fold(self.value, |value, _pad| {
          value.wrapping_mul(BASE_85).wrapping_add(LARGEST_85)
        });
        let word = u32::try_from(padded).map_err(|_past_four_bytes| ScanError::SyntaxError)?;
        let Ok(kept) = index(written.saturating_sub(1));

        out.extend(word.to_be_bytes().into_iter().take(kept));

        Ok(())
      }
    }
  }
}

pub fn ascii85(body: &[u8], out: &mut impl Extend<u8>) -> Result<(), ScanError> {
  let mut group = EMPTY;

  for byte in body
    .iter()
    .filter(|byte| class(**byte) != Ok(Class::WhiteSpace))
  {
    match byte {
      b'!'..=b'u' => {
        let Ok(grown) = group.with(*byte);

        group = match grown.count {
          GROUP_85 => {
            grown.flush(out)?;

            EMPTY
          }
          _short => grown,
        };
      }
      b'z' => {
        group.flush(out)?;
        group = EMPTY;
        out.extend([0; 4]);
      }
      b'~' => return group.flush(out),
      _other => return Err(ScanError::SyntaxError),
    }
  }

  group.flush(out)
}
