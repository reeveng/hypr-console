//! The ways a Matroska file can fail to be read, and the damage it can be read
//! past.
//!
//! A fault ends the open: the source would not answer, the file is not
//! Matroska or WebM, it has no Segment, or it needs something this reader does
//! not do. Damage inside a Segment does not end it -- a cluster that stops
//! decoding is skipped to the next element that looks like the start of one,
//! and an element that claims more than its parent is cut -- and each of those
//! is a `Warning` on the file rather than a fault, named for what was found and
//! where, so a caller can tell a short file from a damaged one.
//!
//! Reading a sample after the open fails for one more pair of reasons: a track
//! or a sample asked for by a position the file does not have. Both carry the
//! position that was asked, at the width it is asked at -- a track as a `u64`,
//! and a sample as the `u32` the keyframe table and the MPEG-4 reader's sample
//! tables already hold one at.
//!
//! The file is walked a header at a time with `console-core-iteration`, whose
//! one answer that cannot arrive -- the steps running out without one of them
//! saying it was done -- is still a case, and it is `Invalid::Endless` here.

use std::{fmt, io};

use console_core_iteration::Endless;

#[derive(Debug)]
pub enum Error {
  Io(io::Error),
  NotMatroska(NotMatroska),
  Invalid(Invalid),
  Unsupported(Unsupported),
  NoSuchTrack(u64),
  NoSuchSample(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotMatroska {
  NoHeader,
  DamagedHeader,
  DocType(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invalid {
  NoSegment,
  UnknownSizeBeforeSegment,
  Endless,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
  LongIds,
  LongSizes,
  EncodedFrames { track: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Warning {
  SegmentPastEnd,
  Resynced { from: u64, to: u64 },
  OversizedElement { id: u32, at: u64, to: u64 },
  DamagedCluster { cluster: u64, at: u64 },
  OverrunningBlock { cluster: u64, id: u32, at: u64 },
  MissedSeek { id: u32, at: u64 },
  CrcMismatch { at: u64 },
  TruncatedElement { id: u32, at: u64 },
  TooLarge { id: u32, at: u64, size: u64 },
}

impl fmt::Display for NotMatroska {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      NotMatroska::NoHeader => write!(to, "no EBML header"),
      NotMatroska::DamagedHeader => write!(to, "bad EBML header"),
      NotMatroska::DocType(said) => write!(to, "DocType {said:?}"),
    }
  }
}

impl fmt::Display for Invalid {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Invalid::NoSegment => write!(to, "no Segment"),
      Invalid::UnknownSizeBeforeSegment => write!(to, "unknown-size element before Segment"),
      Invalid::Endless => write!(to, "the walk over the file never ended"),
    }
  }
}

impl fmt::Display for Unsupported {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Unsupported::LongIds => write!(to, "EBMLMaxIDLength > 4"),
      Unsupported::LongSizes => write!(to, "EBMLMaxSizeLength > 8"),
      Unsupported::EncodedFrames { track } => write!(
        to,
        "content encoding of track {track} (compression/encryption)"
      ),
    }
  }
}

impl fmt::Display for Error {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Error::Io(fault) => write!(to, "I/O error: {fault}"),
      Error::NotMatroska(why) => write!(to, "not a Matroska/WebM file: {why}"),
      Error::Invalid(said) => write!(to, "invalid data: {said}"),
      Error::Unsupported(what) => write!(to, "unsupported: {what}"),
      Error::NoSuchTrack(track) => write!(to, "track index {track} out of range"),
      Error::NoSuchSample(sample) => write!(to, "sample index {sample} out of range"),
    }
  }
}

impl fmt::Display for Warning {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Warning::SegmentPastEnd => write!(to, "Segment extends past end of file (truncated)"),
      Warning::Resynced { from, to: found } => {
        write!(to, "undecodable element at {from}; resynced at {found}")
      }
      Warning::OversizedElement { id, at, to: found } => write!(
        to,
        "oversized element 0x{id:X} at {at}; resynced at {found}"
      ),
      Warning::DamagedCluster { cluster, at } => write!(
        to,
        "damaged cluster at {cluster}: undecodable element at {at}"
      ),
      Warning::OverrunningBlock { cluster, id, at } => write!(
        to,
        "damaged cluster at {cluster}: element 0x{id:X} at {at} overruns"
      ),
      Warning::MissedSeek { id, at } => write!(
        to,
        "SeekHead entry 0x{id:X} -> {at} does not point at that element"
      ),
      Warning::CrcMismatch { at } => write!(to, "CRC-32 mismatch in element at {at}"),
      Warning::TruncatedElement { id, at } => write!(to, "element 0x{id:X} at {at} truncated"),
      Warning::TooLarge { id, at, size } => write!(
        to,
        "element 0x{id:X} at {at} too large ({size} bytes); skipped"
      ),
    }
  }
}

impl std::error::Error for Error {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      Error::Io(fault) => Some(fault),
      Error::NotMatroska(_)
      | Error::Invalid(_)
      | Error::Unsupported(_)
      | Error::NoSuchTrack(_)
      | Error::NoSuchSample(_) => None,
    }
  }
}

impl From<io::Error> for Error {
  fn from(fault: io::Error) -> Self {
    Error::Io(fault)
  }
}

impl From<Endless> for Error {
  fn from(_never_ended: Endless) -> Self {
    Error::Invalid(Invalid::Endless)
  }
}
