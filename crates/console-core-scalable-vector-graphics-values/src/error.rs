use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
  UnexpectedEndOfStream,

  UnexpectedData(u64),

  InvalidValue,

  InvalidIdent,

  InvalidChar(Vec<u8>, u64),

  InvalidString(Vec<String>, u64),

  InvalidNumber(u64),
}

impl core::fmt::Display for Error {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match *self {
      Self::UnexpectedEndOfStream => {
        write!(f, "unexpected end of stream")
      }
      Self::UnexpectedData(pos) => {
        write!(f, "unexpected data at position {pos}")
      }
      Self::InvalidValue => {
        write!(f, "invalid value")
      }
      Self::InvalidIdent => {
        write!(f, "invalid ident")
      }
      Self::InvalidChar(ref chars, pos) => match chars.split_first() {
        Some((actual, expected)) => {
          let list: Vec<String> = expected
            .iter()
            .map(|byte| String::from(char::from(*byte)))
            .collect();
          let actual_2 = char::from(*actual);

          write!(
            f,
            "expected '{}' not '{actual}' at position {pos}",
            list.join("', '"),
            actual = actual_2
          )
        }
        None => write!(f, "invalid character at position {pos}"),
      },
      Self::InvalidString(ref strings, pos) => match strings.split_first() {
        Some((actual, expected)) => {
          write!(
            f,
            "expected '{}' not '{actual}' at position {pos}",
            expected.join("', '")
          )
        }
        None => write!(f, "invalid string at position {pos}"),
      },
      Self::InvalidNumber(pos) => {
        write!(f, "invalid number at position {pos}")
      }
    }
  }
}

impl core::error::Error for Error {
  fn description(&self) -> &str {
    "an SVG data parsing error"
  }
}
