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
//!
//! A line reserves space for its delimiter before appending. Its fragments
//! survive temporary I/O failures. Clear it after consuming a complete line.

use std::io::{self, BufRead, Read};
use std::string::FromUtf8Error;

use console_core_bytes::Bytes;
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinePart {
  Partial,
  Complete,
  End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
  Required,
  OptionalAtEof,
}

pub struct Line {
  limit: u32,
  ending: LineEnding,
  bytes: Vec<u8>,
}

impl Line {
  pub fn new(limit: u32, ending: LineEnding) -> Result<Self, Never> {
    Ok(Self {
      limit,
      ending,
      bytes: Vec::new(),
    })
  }

  pub fn bytes(&self) -> Result<&[u8], Never> {
    Ok(&self.bytes)
  }

  pub fn clear(&mut self) -> Result<(), Never> {
    self.bytes.clear();

    Ok(())
  }

  pub fn read_part(&mut self, input: &mut impl BufRead) -> Result<LinePart, ReadError> {
    let available = input.fill_buf().map_err(ReadError::Io)?;

    match (available.is_empty(), self.bytes.is_empty(), self.ending) {
      (true, true, _) => return Ok(LinePart::End),
      (true, false, LineEnding::OptionalAtEof) => return Ok(LinePart::Complete),
      (true, false, LineEnding::Required) => {
        return Err(ReadError::Io(io::Error::from(io::ErrorKind::UnexpectedEof)));
      }
      (false, _, _) => {}
    }

    let (incoming, part) = match available.iter().position(|byte| *byte == b'\n') {
      Some(end) => {
        let Ok(size) = fixed_index(end);

        (size, LinePart::Complete)
      }
      None => {
        let Ok(size) = fixed_index(available.len());

        (size, LinePart::Partial)
      }
    };
    let Ok(total) = fixed_index(self.bytes.len());
    let too_long = ReadError::TooLong { limit: self.limit };

    match (total.saturating_add(incoming) < u64::from(self.limit)).then_some(()) {
      Some(()) => {}
      None => return Err(too_long),
    }

    let Ok(stop) = index(incoming);
    let data = available.get(..stop).ok_or(too_long)?;
    self.bytes.extend_from_slice(data);

    match part {
      LinePart::Complete => input.consume(stop.saturating_add(1)),
      LinePart::Partial | LinePart::End => input.consume(stop),
    }

    Ok(part)
  }

  pub fn read(&mut self, input: &mut impl BufRead) -> Result<Option<&[u8]>, ReadError> {
    for _ in 0..=self.limit {
      let part = self.read_part(input)?;

      match part {
        LinePart::Partial => {}
        LinePart::Complete => return Ok(Some(&self.bytes)),
        LinePart::End => return Ok(None),
      }
    }

    Err(ReadError::TooLong { limit: self.limit })
  }
}

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

pub fn bytes(reader: impl Read, limit: u32) -> Result<Vec<u8>, ReadError> {
  let data = payload(reader, limit)?;
  let Ok(buffer) = data.into_vec();

  Ok(buffer)
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  allow(
    explicit057_no_unbounded_wait,
    reason = "the caller owns the reader and its deadline; a generic Read has no timeout setting, and this function bounds only its byte count"
  )
)]
pub fn payload(reader: impl Read, limit: u32) -> Result<Bytes, ReadError> {
  let mut bounded = reader.take(u64::from(limit).saturating_add(1));
  let mut buffer = Vec::new();

  bounded.read_to_end(&mut buffer).map_err(ReadError::Io)?;

  match u32::try_from(buffer.len()) {
    Ok(length) => match InputLength::from(length <= limit) {
      InputLength::WithinLimit => Ok(Bytes::from(buffer)),
      InputLength::TooLong => Err(ReadError::TooLong { limit }),
    },
    Err(_too_wide) => Err(ReadError::TooLong { limit }),
  }
}

pub fn text(reader: impl Read, limit: u32) -> Result<String, ReadError> {
  let buffer = payload(reader, limit)?;

  buffer.into_utf8().map_err(ReadError::NotUtf8)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn fragmented_frames_leave_the_next_line_in_the_reader() -> Result<(), ReadError> {
    let mut input = io::BufReader::with_capacity(2, b"abc\n\nnext\n".as_slice());
    let Ok(mut line) = Line::new(5, LineEnding::Required);

    let part = line.read_part(&mut input)?;
    let Ok(fragment) = line.bytes();
    assert_eq!(part, LinePart::Partial);
    assert_eq!(fragment, b"ab");
    let first = line.read(&mut input)?;
    assert_eq!(first, Some(b"abc".as_slice()));
    let Ok(()) = line.clear();
    let empty = line.read(&mut input)?;
    assert_eq!(empty, Some(b"".as_slice()));
    let second = line.read(&mut input)?;
    assert_eq!(second, Some(b"next".as_slice()));
    let Ok(()) = line.clear();
    let end = line.read(&mut input)?;
    assert_eq!(end, None);

    Ok(())
  }

  #[test]
  fn oversized_and_truncated_frames_never_complete() -> Result<(), ReadError> {
    let Ok(mut line) = Line::new(4, LineEnding::Required);

    assert!(matches!(
      line.read(&mut b"abcd\n".as_slice()),
      Err(ReadError::TooLong { limit: 4 })
    ));
    let Ok(buffered) = line.bytes();
    assert!(buffered.is_empty());
    assert!(matches!(
      line.read(&mut b"abc".as_slice()),
      Err(ReadError::Io(fault)) if fault.kind() == io::ErrorKind::UnexpectedEof
    ));

    Ok(())
  }

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
