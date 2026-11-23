//! What can go wrong reading a Matroska file, as cases a caller can tell apart.
//!
//! The shared `Error` says a malformed stream with a `&'static str`, and a fault
//! said as a sentence is one nothing can decide about. Inside this crate every
//! call fails as a `Fault`, whose cases name what was missing or wrong, and the
//! words the shared `Error` always carried are written once, on the case, and
//! handed over at the one place a fault leaves: the `From` into `Error` the
//! `FormatReader` methods meet with `?`.

use std::io;

use console_core_audio_decoding_interfaces::errors::{Error, SeekErrorKind};
use console_core_iteration::Endless;
use console_core_words::Words;

#[derive(Debug, Clone, Copy, Words)]
pub(crate) enum MissingElement {
  #[words(said = "mkv: missing vorbis identification packet")]
  VorbisIdentification,
  #[words(said = "mkv: missing vorbis setup packet")]
  VorbisSetup,
  #[words(said = "mkv (flac): missing flac stream marker")]
  FlacMarker,
  #[words(said = "mkv: missing Tracks element")]
  Tracks,
  #[words(said = "mkv: missing Info element")]
  Info,
  #[words(said = "mkv: missing track number")]
  TrackNumber,
  #[words(said = "mkv: missing track UID")]
  TrackUid,
  #[words(said = "mkv: missing codec id")]
  CodecId,
  #[words(said = "mkv: missing seek track id")]
  SeekId,
  #[words(said = "mkv: missing seek track pos")]
  SeekPosition,
  #[words(said = "mkv: missing time in cue")]
  CueTime,
  #[words(said = "mkv: missing positions in cue")]
  CuePositions,
  #[words(said = "mkv: missing track in cue track positions")]
  CueTrack,
  #[words(said = "mkv: missing position in cue track positions")]
  CueClusterPosition,
  #[words(said = "mkv: missing block inside block group")]
  Block,
  #[words(said = "mkv: missing tag name")]
  TagName,
  #[words(said = "mkv: missing tag value")]
  TagValue,
}

#[derive(Debug, Clone, Copy, Words)]
pub(crate) enum Invalid {
  #[words(said = "mkv: unexpected EBML element")]
  UnexpectedElement,
  #[words(said = "mkv: invalid EBML element ID")]
  ElementId,
  #[words(said = "mkv: invalid EBML variable integer")]
  VariableInteger,
  #[words(said = "mkv: invalid block header")]
  BlockHeader,
  #[words(said = "mkv: invalid block lacing")]
  Lacing,
  #[words(said = "mkv: invalid vorbis packet lacing")]
  VorbisLacing,
  #[words(said = "mkv: track number exceeds u32")]
  TrackNumber,
  #[words(said = "mkv: invalid timestamp scale")]
  TimestampScale,
  #[words(said = "mkv: element runs past the end of the stream")]
  Length,
  #[words(said = "mkv: element read out of place")]
  Position,
  #[words(said = "mkv: the walk over the stream never ended")]
  Endless,
}

#[derive(Debug, Clone, Copy, Words)]
pub(crate) enum Unsupported {
  #[words(said = "mkv: not a matroska / webm file")]
  NotMatroska,
  #[words(said = "mkv: missing segment element")]
  MissingSegment,
  #[words(said = "mkv: invalid ebml file")]
  InvalidEbml,
}

#[derive(Debug)]
pub(crate) enum DecodeError {
  Io(io::Error),
  Foreign(Error),
  Seek(SeekErrorKind),
  EndOfStream,
  Absent(MissingElement),
  Invalid(Invalid),
  Unsupported(Unsupported),
}

impl From<io::Error> for DecodeError {
  fn from(fault: io::Error) -> Self {
    DecodeError::Io(fault)
  }
}

impl From<Error> for DecodeError {
  fn from(fault: Error) -> Self {
    DecodeError::Foreign(fault)
  }
}

impl From<Endless> for DecodeError {
  fn from(_never_ended: Endless) -> Self {
    DecodeError::Invalid(Invalid::Endless)
  }
}

impl From<DecodeError> for Error {
  fn from(fault: DecodeError) -> Self {
    match fault {
      DecodeError::Io(fault_2) => Error::IoError(fault_2),
      DecodeError::Foreign(fault_3) => fault_3,
      DecodeError::Seek(kind) => Error::SeekError(kind),
      DecodeError::EndOfStream => Error::IoError(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "end of stream",
      )),
      DecodeError::Absent(missing) => {
        let Ok(said) = missing.said();

        Error::DecodeError(said)
      }
      DecodeError::Invalid(invalid) => {
        let Ok(said) = invalid.said();

        Error::DecodeError(said)
      }
      DecodeError::Unsupported(unsupported) => {
        let Ok(said) = unsupported.said();

        Error::Unsupported(said)
      }
    }
  }
}
