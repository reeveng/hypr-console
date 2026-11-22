//! What reading a file and writing one can fail with.
//!
//! A file is read or it is not, and a caller that shows the fault shows one
//! sentence naming the format, so `Display` says only that much and the cause
//! is what `source` hands on: the read that ran short, or the part of the file
//! that made no sense. Inside the crate a reader answers `ParseError`, which
//! has no format yet, and `read` puts the format on it once, where the format
//! was chosen.

use std::error::Error;
use std::fmt;

use crate::FileType;

#[derive(Debug)]
pub enum FileParseError {
  UnknownFormat,
  Open(std::io::Error),
  Read(FileType, std::io::Error),
  Parse(FileType, Malformed),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Malformed {
  Short,
  Signature,
  NoAudio,
  Properties,
  TagVersion,
  TagSize,
  Frame,
  Encoding,
  Text,
  Timestamp,
  Picture,
  Item,
  Atom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileEncodingError {
  NotWave,
  SizeMismatch,
  TooLarge,
  Timestamp,
}

#[derive(Debug)]
pub(crate) enum ParseError {
  Read(std::io::Error),
  Parse(Malformed),
}

impl ParseError {
  pub(crate) fn of(self, format: FileType) -> Result<FileParseError, console_core_never::Never> {
    Ok(match self {
      ParseError::Read(fault) => FileParseError::Read(format, fault),
      ParseError::Parse(malformed) => FileParseError::Parse(format, malformed),
    })
  }
}

impl From<std::io::Error> for ParseError {
  fn from(fault: std::io::Error) -> ParseError {
    ParseError::Read(fault)
  }
}

impl From<Malformed> for ParseError {
  fn from(malformed: Malformed) -> ParseError {
    ParseError::Parse(malformed)
  }
}

impl From<console_core_ogg_pages::PageError> for ParseError {
  fn from(fault: console_core_ogg_pages::PageError) -> ParseError {
    match fault {
      console_core_ogg_pages::PageError::Io(fault_2) => ParseError::Read(fault_2),
      console_core_ogg_pages::PageError::MissingCapturePattern
      | console_core_ogg_pages::PageError::UnknownVersion
      | console_core_ogg_pages::PageError::NoSegments
      | console_core_ogg_pages::PageError::TooFewPackets
      | console_core_ogg_pages::PageError::NoSuchPacket => {
        ParseError::Parse(Malformed::Signature)
      }
    }
  }
}

impl fmt::Display for FileParseError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      FileParseError::UnknownFormat | FileParseError::Open(_) => {
        write!(to, "failed to parse file")
      }
      FileParseError::Read(format, _fault) => write!(to, "failed to parse {format:?} file"),
      FileParseError::Parse(format, _malformed) => write!(to, "failed to parse {format:?} file"),
    }
  }
}

impl Error for FileParseError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    match self {
      FileParseError::UnknownFormat => None,
      FileParseError::Open(fault) => Some(fault),
      FileParseError::Read(_format, fault) => Some(fault),
      FileParseError::Parse(_format, malformed) => Some(malformed),
    }
  }
}

impl fmt::Display for Malformed {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Malformed::Short => write!(
        to,
        "a part of the file ends before the size it gives itself"
      ),
      Malformed::Signature => write!(
        to,
        "the file does not begin the way its format says it must"
      ),
      Malformed::NoAudio => write!(to, "the file holds no audio this format describes"),
      Malformed::Properties => write!(
        to,
        "the audio is described with values the format does not allow"
      ),
      Malformed::TagVersion => write!(to, "the tag is written in a version nothing reads"),
      Malformed::TagSize => write!(to, "a size in the file does not fit the file"),
      Malformed::Frame => write!(to, "a frame of the tag cannot be read"),
      Malformed::Encoding => write!(to, "a frame names a text encoding there is not"),
      Malformed::Text => write!(to, "a text is not written in the encoding it names"),
      Malformed::Timestamp => write!(to, "a date in the tag is not a date"),
      Malformed::Picture => write!(to, "a picture is in a format the tag does not allow"),
      Malformed::Item => write!(
        to,
        "an item of the tag has a key or a kind the format does not allow"
      ),
      Malformed::Atom => write!(to, "an atom of the file is not the size it says"),
    }
  }
}

impl Error for Malformed {}

impl fmt::Display for FileEncodingError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      FileEncodingError::NotWave
      | FileEncodingError::SizeMismatch
      | FileEncodingError::TooLarge
      | FileEncodingError::Timestamp => write!(to, "failed to write Wav file"),
    }
  }
}

impl Error for FileEncodingError {}
