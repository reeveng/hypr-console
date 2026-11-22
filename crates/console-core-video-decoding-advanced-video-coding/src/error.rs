use std::fmt;

use console_core_video_decoding_bitstream::BitError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
  Bitstream(BitError),
  Invalid(String),
  Unsupported(String),
  MissingParameterSet(String),
}

impl fmt::Display for Error {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Bitstream(value) => write!(to, "bitstream error: {value}"),
      Self::Invalid(value) => write!(to, "invalid bitstream: {value}"),
      Self::Unsupported(value) => write!(to, "unsupported: {value}"),
      Self::MissingParameterSet(value) => write!(to, "missing parameter set: {value}"),
    }
  }
}

impl std::error::Error for Error {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      Self::Bitstream(value) => Some(value),
      Self::Invalid(_) | Self::Unsupported(_) | Self::MissingParameterSet(_) => None,
    }
  }
}

impl From<BitError> for Error {
  fn from(value: BitError) -> Self {
    Self::Bitstream(value)
  }
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn invalid<T>(msg: impl Into<String>) -> Result<T> {
  Err(Error::Invalid(<_ as ::core::convert::Into<
    ::std::string::String,
  >>::into(msg)))
}

pub(crate) fn unsupported<T>(msg: impl Into<String>) -> Result<T> {
  Err(Error::Unsupported(<_ as ::core::convert::Into<
    ::std::string::String,
  >>::into(msg)))
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "A syntax check returns the typed bitstream error at the caller without evaluating its message on success"
  )
)]
macro_rules! ensure {
    ($cond:expr, $($arg:tt)+) => {
        if !$cond {
            return Err($crate::error::Error::Invalid(format!($($arg)+)));
        }
    };
}
pub(crate) use ensure;
