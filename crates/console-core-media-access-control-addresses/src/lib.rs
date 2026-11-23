//! A hardware address: six bytes, written as six pairs of hexadecimal digits
//! with one mark between each pair.
//!
//! The mark is the only part that changes with who is writing. bluez says a
//! device's address with colons, names a device that never said its own name
//! after the same address with hyphens, and puts it into an object path with
//! underscores, because a path cannot hold a colon or a hyphen. The settings
//! tab compared a name with an address by swapping one mark for the other in a
//! string, and the bluez conversation would have needed the third spelling of
//! the same six bytes. So the address is the bytes, and the mark is said where it
//! is written or read.
//!
//! BlueR carried this as `macaddr`, a crate of its own, and that is the
//! argument for this one: an address is not Bluetooth's. A Wi-Fi access point
//! and a network card are named the same way, and whichever of them needs one
//! next reads it here rather than splitting a string again.
//!
//! Reading is strict. Each pair is exactly two digits, because a reader that
//! took `+F` as a byte -- which the standard library's own radix parse will --
//! would find an address in text that does not hold one. Writing is always
//! upper case, which is what bluez writes and what a reader comparing two
//! spellings has to agree on.

#![cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "the fixed-size array enforces the entire Address invariant; only the text representation can fail to parse"
  )
)]

use std::fmt;
use std::str::FromStr;

use console_core_never::Never;
use console_core_words::Words;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Address(pub [u8; 6]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Separator {
  #[words(written = ":")]
  Colon,
  #[words(written = "-")]
  Hyphen,
  #[words(written = "_")]
  Underscore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
  Length(String),
  Digits(String),
}

impl fmt::Display for ParseError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      ParseError::Length(written) => write!(to, "{written:?} is not six pairs of digits"),
      ParseError::Digits(written) => write!(
        to,
        "{written:?} has a pair that is not two hexadecimal digits"
      ),
    }
  }
}

impl std::error::Error for ParseError {}

impl Address {
  pub fn read(written: &str, separator: Separator) -> Result<Address, ParseError> {
    let Ok(mark) = separator.written();
    let Ok(bytes) = written
      .split(mark)
      .map(byte)
      .collect::<Result<Option<Vec<u8>>, Never>>();

    let read = match bytes {
      Some(read) => read,
      None => return Err(ParseError::Digits(written.to_string())),
    };

    match <[u8; 6]>::try_from(read) {
      Ok(six) => Ok(Address(six)),
      Err(_counted_wrong) => Err(ParseError::Length(written.to_string())),
    }
  }

  pub fn written(self, separator: Separator) -> Result<String, Never> {
    let Ok(mark) = separator.written();
    let pairs: Vec<String> = self.0.iter().map(|byte| format!("{byte:02X}")).collect();

    Ok(pairs.join(mark))
  }
}

enum HexPair {
  TwoDigits,
  Invalid,
}

impl From<bool> for HexPair {
  fn from(valid: bool) -> Self {
    match bool::then_some(valid, ()) {
      Some(()) => Self::TwoDigits,
      None => Self::Invalid,
    }
  }
}

fn byte(pair: &str) -> Result<Option<u8>, Never> {
  let spelling =
    HexPair::from(pair.len() == 2 && pair.bytes().all(|digit| digit.is_ascii_hexdigit()));

  Ok(match spelling {
    HexPair::TwoDigits => match u8::from_str_radix(pair, 16) {
      Ok(byte) => Some(byte),
      Err(_not_a_byte) => None,
    },
    HexPair::Invalid => None,
  })
}

impl fmt::Display for Address {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    let Ok(written) = self.written(Separator::Colon);

    write!(to, "{written}")
  }
}

impl FromStr for Address {
  type Err = ParseError;

  fn from_str(written: &str) -> Result<Address, ParseError> {
    Address::read(written, Separator::Colon)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const MS218: Address = Address([0xE0, 0xC1, 0x02, 0x00, 0xB0, 0xC3]);

  #[test]
  fn one_address_is_read_back_from_each_of_the_spellings_bluez_writes() {
    assert_eq!("E0:C1:02:00:B0:C3".parse::<Address>(), Ok(MS218));
    assert_eq!(
      Address::read("E0-C1-02-00-B0-C3", Separator::Hyphen),
      Ok(MS218)
    );
    assert_eq!(
      Address::read("E0_C1_02_00_B0_C3", Separator::Underscore),
      Ok(MS218)
    );
    assert_eq!(
      Address::read("e0:c1:02:00:b0:c3", Separator::Colon),
      Ok(MS218),
      "lower case is the same address"
    );
  }

  #[test]
  fn an_address_is_written_in_upper_case_with_the_mark_it_is_asked_for() {
    assert_eq!(MS218.to_string(), "E0:C1:02:00:B0:C3");
    assert_eq!(
      MS218.written(Separator::Hyphen),
      Ok("E0-C1-02-00-B0-C3".to_string())
    );
    assert_eq!(
      MS218.written(Separator::Underscore),
      Ok("E0_C1_02_00_B0_C3".to_string())
    );
  }

  #[test]
  fn text_that_only_looks_like_an_address_is_not_read_as_one() {
    assert_eq!(
      "E0:C1:02:00:B0".parse::<Address>(),
      Err(ParseError::Length("E0:C1:02:00:B0".to_string()))
    );
    assert_eq!(
      "E0:C1:02:00:B0:C3:11".parse::<Address>(),
      Err(ParseError::Length("E0:C1:02:00:B0:C3:11".to_string()))
    );
    assert_eq!(
      "+F:C1:02:00:B0:C3".parse::<Address>(),
      Err(ParseError::Digits("+F:C1:02:00:B0:C3".to_string()))
    );
    assert_eq!(
      "E0-C1-02-00-B0-C3".parse::<Address>(),
      Err(ParseError::Digits("E0-C1-02-00-B0-C3".to_string()))
    );
    assert_eq!(
      "".parse::<Address>(),
      Err(ParseError::Digits(String::new()))
    );
  }
}
