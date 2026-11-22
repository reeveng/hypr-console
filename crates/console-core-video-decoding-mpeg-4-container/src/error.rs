//! The ways a film's boxes can fail to be read.
//!
//! A part that ends before what it says it holds is `Truncated`, named by the
//! part, and a box whose framing is wrong is `Framing`, which is what the shared
//! box reader said about it. `Invalid` is a box that is all there and says
//! something the format does not allow, each case named for what it said, and
//! `TooLarge` is a count past the ceiling this reader sets before it allocates.
//! A track or a sample asked for by a position the file does not have is its
//! own case, so a caller can tell a short file from a wrong question.

use std::{fmt, io};

use console_core_iso_media_boxes::BoxError;

#[derive(Debug)]
pub enum Error {
  Io(io::Error),
  Truncated(&'static str),
  Framing(BoxError),
  Invalid(Invalid),
  Unsupported(Unsupported),
  TooLarge(&'static str),
  NoMoov,
  NoSuchSample(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invalid {
  TrackWithoutMedia,
  ChunksOutOfOrder,
  CompactSizeField(u32),
  FragmentWithoutHeader,
  Av1MarkerMissing,
  ApvRecordVersion(u8),
  ApvBoxVersion(u8),
  EsdsTag(u8),
  EsdsWithoutDecoderConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
  CompressedMovieHeader,
}

impl fmt::Display for Invalid {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Invalid::TrackWithoutMedia => write!(to, "trak without mdia"),
      Invalid::ChunksOutOfOrder => {
        write!(to, "stsc first_chunk values are not increasing from 1")
      }
      Invalid::CompactSizeField(field) => write!(to, "stz2 field size {field}"),
      Invalid::FragmentWithoutHeader => write!(to, "traf without tfhd"),
      Invalid::Av1MarkerMissing => write!(to, "av1C marker bit not set"),
      Invalid::ApvRecordVersion(version) => write!(
        to,
        "unsupported APVDecoderConfigurationRecord version {version}"
      ),
      Invalid::ApvBoxVersion(version) => write!(to, "unsupported apvC FullBox version {version}"),
      Invalid::EsdsTag(tag) => write!(to, "esds: expected ES_Descriptor, got tag {tag}"),
      Invalid::EsdsWithoutDecoderConfig => write!(to, "esds without DecoderConfigDescriptor"),
    }
  }
}

impl fmt::Display for Unsupported {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Unsupported::CompressedMovieHeader => write!(to, "compressed movie header (cmov)"),
    }
  }
}

impl fmt::Display for Error {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Error::Io(fault) => write!(to, "I/O error: {fault}"),
      Error::Truncated(part) => write!(to, "truncated data while reading {part}"),
      Error::Framing(fault) => write!(to, "invalid data: {fault}"),
      Error::Invalid(said) => write!(to, "invalid data: {said}"),
      Error::Unsupported(what) => write!(to, "unsupported: {what}"),
      Error::TooLarge(what) => write!(to, "size limit exceeded: {what}"),
      Error::NoMoov => write!(to, "no moov box found"),
      Error::NoSuchSample(sample) => write!(to, "sample index {sample} out of range"),
    }
  }
}

impl std::error::Error for Error {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      Error::Io(fault) => Some(fault),
      Error::Framing(fault) => Some(fault),
      Error::Truncated(_)
      | Error::Invalid(_)
      | Error::Unsupported(_)
      | Error::TooLarge(_)
      | Error::NoMoov
      | Error::NoSuchSample(_) => None,
    }
  }
}

impl From<io::Error> for Error {
  fn from(fault: io::Error) -> Self {
    Error::Io(fault)
  }
}

impl From<BoxError> for Error {
  fn from(fault: BoxError) -> Self {
    match fault {
      BoxError::Truncated { .. } => Error::Truncated("box header"),
      BoxError::InvalidSize { .. } | BoxError::ExceedsContainer { .. } => Error::Framing(fault),
    }
  }
}

impl From<Invalid> for Error {
  fn from(said: Invalid) -> Self {
    Error::Invalid(said)
  }
}
