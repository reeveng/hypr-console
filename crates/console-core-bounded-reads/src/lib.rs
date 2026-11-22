//! A stream read to its end with a ceiling chosen by its caller.
//!
//! A socket reply, a bookmark import and a compressed image are different
//! formats with the same question at their door: how much may be read before
//! the input is refused? A `take(limit)` alone silently accepts a prefix.
//! Reading one extra byte distinguishes an input that fits exactly from one
//! that does not, without reading the rest of an oversized stream. The limit
//! is a byte count, including for text, and an invalid UTF-8 sequence is a fault.
//! A read error is preserved; no partial buffer is returned as a complete input.
//! The caller also owns the wait: a socket's timeout is set before it is handed
//! in, because a generic reader has no deadline setting.

use std::io::{self, Read};
use std::string::FromUtf8Error;

enum InputLength {
  WithinLimit,
  TooLong,
}

impl From<bool> for InputLength {
  fn from(fits: bool) -> Self {
    match bool::then_some(fits, ()) {
      Some(()) => Self::WithinLimit,
      None => Self::TooLong,
    }
  }
}

#[derive(Debug)]
pub enum ReadError {
  Io(io::Error),
  TooLong { limit: u32 },
  NotUtf8(FromUtf8Error),
}

impl std::fmt::Display for ReadError {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Io(fault) => write!(to, "{fault}"),
      Self::TooLong { limit } => write!(to, "the input exceeds its {limit}-byte limit"),
      Self::NotUtf8(fault) => write!(to, "the input is not UTF-8: {fault}"),
    }
  }
}

impl std::error::Error for ReadError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      Self::Io(fault) => Some(fault),
      Self::TooLong { .. } => None,
      Self::NotUtf8(fault) => Some(fault),
    }
  }
}

impl From<ReadError> for io::Error {
  fn from(fault: ReadError) -> io::Error {
    match fault {
      ReadError::Io(cause) => cause,
      ReadError::TooLong { .. } | ReadError::NotUtf8(_) => {
        io::Error::new(io::ErrorKind::InvalidData, fault)
      }
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  allow(
    explicit057_no_unbounded_wait,
    reason = "the caller owns the reader and its deadline; a generic Read has no timeout setting, and this function bounds only its byte count"
  )
)]
pub fn bytes(reader: impl Read, limit: u32) -> Result<Vec<u8>, ReadError> {
  let mut bounded = reader.take(u64::from(limit).saturating_add(1));
  let mut buffer = Vec::new();

  bounded.read_to_end(&mut buffer).map_err(ReadError::Io)?;

  match u32::try_from(buffer.len()) {
    Ok(length) => match InputLength::from(length <= limit) {
      InputLength::WithinLimit => Ok(buffer),
      InputLength::TooLong => Err(ReadError::TooLong { limit }),
    },
    Err(_too_wide) => Err(ReadError::TooLong { limit }),
  }
}

pub fn text(reader: impl Read, limit: u32) -> Result<String, ReadError> {
  let buffer = bytes(reader, limit)?;

  String::from_utf8(buffer).map_err(ReadError::NotUtf8)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn empty_short_and_exactly_fitting_inputs_are_complete() -> Result<(), ReadError> {
    let empty = bytes(b"".as_slice(), 0)?;
    let short = bytes(b"ab".as_slice(), 3)?;
    let exact = bytes(b"abc".as_slice(), 3)?;

    assert!(empty.is_empty());
    assert_eq!(short, b"ab");
    assert_eq!(exact, b"abc");

    Ok(())
  }

  #[test]
  fn an_oversized_input_is_refused_after_one_extra_byte() {
    let mut input = b"abcdef".as_slice();

    assert!(matches!(
      bytes(&mut input, 3),
      Err(ReadError::TooLong { limit: 3 })
    ));
    assert_eq!(input, b"ef");
    assert!(matches!(
      bytes(b"x".as_slice(), 0),
      Err(ReadError::TooLong { limit: 0 })
    ));
  }

  #[test]
  fn a_source_that_never_ends_is_still_bounded() {
    assert!(matches!(
      bytes(io::repeat(b'x'), 16),
      Err(ReadError::TooLong { limit: 16 })
    ));
  }

  #[test]
  fn text_is_counted_in_bytes_and_invalid_text_is_refused() -> Result<(), ReadError> {
    let decoded = text("é".as_bytes(), 2)?;

    assert_eq!(decoded, "é");
    assert!(matches!(
      text("é".as_bytes(), 1),
      Err(ReadError::TooLong { limit: 1 })
    ));
    assert!(matches!(
      text(b"\xff".as_slice(), 1),
      Err(ReadError::NotUtf8(_))
    ));

    Ok(())
  }

  struct Failing;

  impl Read for Failing {
    #[cfg_attr(
      dylint_lib = "explicit051_no_machine_width",
      allow(
        explicit051_no_machine_width,
        reason = "the Read trait requires its byte count to be usize"
      )
    )]
    fn read(&mut self, _into: &mut [u8]) -> io::Result<usize> {
      Err(io::Error::from(io::ErrorKind::PermissionDenied))
    }
  }

  #[test]
  fn read_errors_keep_their_original_kind() {
    let fault = bytes(b"ab".as_slice().chain(Failing), 3).unwrap_err();
    let io_fault = io::Error::from(fault);

    assert_eq!(io_fault.kind(), io::ErrorKind::PermissionDenied);
  }

  #[test]
  fn oversized_input_is_invalid_data_at_an_io_boundary() {
    let fault = bytes(b"abcd".as_slice(), 3).unwrap_err();
    let io_fault = io::Error::from(fault);

    assert_eq!(io_fault.kind(), io::ErrorKind::InvalidData);
    assert!(
      io_fault
        .get_ref()
        .is_some_and(|cause| cause.is::<ReadError>())
    );
  }
}
