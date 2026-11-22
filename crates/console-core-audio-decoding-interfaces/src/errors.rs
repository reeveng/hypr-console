//! The ways a decode, a seek or a probe fails, and the sentence each one says.
//!
//! A fault is a case a caller can match on: the payload of the first four is
//! only the reason a person reads, so nothing decides anything from it.

use std::fmt;
use std::io;

#[derive(Debug)]
pub enum SeekErrorKind {
  Unseekable,
  ForwardOnly,
  OutOfRange,
  InvalidTrack,
}

#[derive(Debug)]
pub enum Error {
  IoError(io::Error),
  DecodeError(&'static str),
  SeekError(SeekErrorKind),
  Unsupported(&'static str),
  LimitError(&'static str),
  ResetRequired,
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      Error::IoError(err) => fmt::Display::fmt(err, f),
      Error::DecodeError(msg) => write!(f, "malformed stream: {msg}"),
      Error::SeekError(kind) => write!(
        f,
        "seek error: {}",
        match kind {
          SeekErrorKind::Unseekable => "stream is not seekable",
          SeekErrorKind::ForwardOnly => "stream can only be seeked forward",
          SeekErrorKind::OutOfRange => "requested seek timestamp is out-of-range for stream",
          SeekErrorKind::InvalidTrack => "invalid track id",
        }
      ),
      Error::Unsupported(feature) => write!(f, "unsupported feature: {feature}"),
      Error::LimitError(constraint) => write!(f, "limit reached: {constraint}"),
      Error::ResetRequired => write!(f, "decoder needs to be reset"),
    }
  }
}

impl std::error::Error for Error {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      Error::IoError(err) => Some(err),
      Error::DecodeError(_)
      | Error::SeekError(_)
      | Error::Unsupported(_)
      | Error::LimitError(_)
      | Error::ResetRequired => None,
    }
  }
}

impl From<io::Error> for Error {
  fn from(err: io::Error) -> Error {
    Error::IoError(err)
  }
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn decode_error<T>(desc: &'static str) -> Result<T> {
  Err(Error::DecodeError(desc))
}

pub fn seek_error<T>(kind: SeekErrorKind) -> Result<T> {
  Err(Error::SeekError(kind))
}

pub fn unsupported_error<T>(feature: &'static str) -> Result<T> {
  Err(Error::Unsupported(feature))
}

pub fn limit_error<T>(constraint: &'static str) -> Result<T> {
  Err(Error::LimitError(constraint))
}

pub fn reset_error<T>() -> Result<T> {
  Err(Error::ResetRequired)
}

pub fn end_of_stream_error<T>() -> Result<T> {
  Err(Error::IoError(io::Error::new(
    io::ErrorKind::UnexpectedEof,
    "end of stream",
  )))
}
