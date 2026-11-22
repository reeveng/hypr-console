//! Parses decimal or hexadecimal ASCII digits with checked arithmetic.
//!
//! The input has no sign, prefix, or surrounding whitespace. Empty input,
//! invalid digits, overflow, and values outside the destination range return
//! `None`. Signed destinations accept nonnegative magnitudes only.

use console_core_never::Never;

const DECIMAL: u32 = 10;
const HEXADECIMAL: u32 = 16;

pub trait FromBytes: Sized {
  fn from_bytes_dec(b: &[u8]) -> Option<Self>;

  fn from_bytes_hex(b: &[u8]) -> Option<Self>;
}

fn digits<T: TryFrom<u64>>(spelled: &[u8], radix: u32) -> Result<Option<T>, Never> {
  let read = match spelled {
    [] => None,
    [_first, ..] => spelled.iter().try_fold(0u64, |held, &c| {
      let digit = char::from(c).to_digit(radix)?;
      let shifted = held.checked_mul(u64::from(radix))?;

      shifted.checked_add(u64::from(digit))
    }),
  };

  Ok(match read {
    None => None,
    Some(value) => match T::try_from(value) {
      Ok(fits) => Some(fits),
      Err(_too_wide_for_the_type) => None,
    },
  })
}

impl FromBytes for u32 {
  fn from_bytes_dec(b: &[u8]) -> Option<Self> {
    let Ok(read) = digits(b, DECIMAL);
    read
  }

  fn from_bytes_hex(b: &[u8]) -> Option<Self> {
    let Ok(read) = digits(b, HEXADECIMAL);
    read
  }
}

impl FromBytes for i64 {
  fn from_bytes_dec(b: &[u8]) -> Option<Self> {
    let Ok(read) = digits(b, DECIMAL);
    read
  }

  fn from_bytes_hex(b: &[u8]) -> Option<Self> {
    let Ok(read) = digits(b, HEXADECIMAL);
    read
  }
}
