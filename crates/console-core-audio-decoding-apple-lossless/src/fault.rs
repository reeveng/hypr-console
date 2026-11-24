//! The ways an ALAC packet or cookie is refused.
//!
//! The decoder contract answers in the audio crate's `Error`, whose cases
//! carry a sentence and nothing a caller could decide from, so the sentence is
//! written once here as a case of this crate's own and crosses into that error
//! at the one place the contract asks for it.

use std::fmt;
use std::io;

use console_core_audio_decoding_interfaces::errors::Error;
use console_core_never::Never;

#[derive(Debug)]
pub(crate) enum DecodeError {
  CodecType,
  MissingExtraData,
  CookieSize,
  Version,
  BitDepth,
  ChannelCount,
  LayoutSize,
  LayoutId,
  LayoutVersion,
  LayoutTag,
  LayoutChannels,
  LayoutReserved,
  UnusedHeaderBits,
  Shift,
  Mixing,
  Mode,
  Element,
  Channel,
  FrameLength,
  Stream(io::Error),
  Output(Error),
}

impl DecodeError {
  fn sentence(&self) -> Result<&'static str, Never> {
    Ok(match self {
      DecodeError::CodecType => "alac: invalid codec type",
      DecodeError::MissingExtraData => "alac: missing extra data",
      DecodeError::CookieSize => "alac: invalid magic cookie size",
      DecodeError::Version => "alac: not compatible with alac version 0",
      DecodeError::BitDepth => "alac: invalid bit depth",
      DecodeError::ChannelCount => "alac: more than 8 channels",
      DecodeError::LayoutSize => "alac: invalid channel layout info size",
      DecodeError::LayoutId => "alac: invalid channel layout info id",
      DecodeError::LayoutVersion => "alac: invalid channel layout info version",
      DecodeError::LayoutTag => "alac: invalid channel layout tag",
      DecodeError::LayoutChannels => {
        "alac: the number of channels differs from the channel layout"
      }
      DecodeError::LayoutReserved => "alac: reserved values in channel layout info are not 0",
      DecodeError::UnusedHeaderBits => "alac: unused header bits not 0",
      DecodeError::Shift => "alac: invalid shift value",
      DecodeError::Mixing => "alac: invalid mixing information for mono channel",
      DecodeError::Mode => "alac: invalid mode",
      DecodeError::Element => "alac: unsupported element",
      DecodeError::Channel => "alac: element has no channel to decode into",
      DecodeError::FrameLength => "alac: frame does not fit the frame length",
      DecodeError::Stream(_) => "alac: stream could not be read",
      DecodeError::Output(_) => "alac: output buffer could not be rendered",
    })
  }
}

impl fmt::Display for DecodeError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      DecodeError::Stream(source) => source.fmt(f),
      DecodeError::Output(inner) => inner.fmt(f),
      DecodeError::CodecType
      | DecodeError::MissingExtraData
      | DecodeError::CookieSize
      | DecodeError::Version
      | DecodeError::BitDepth
      | DecodeError::ChannelCount
      | DecodeError::LayoutSize
      | DecodeError::LayoutId
      | DecodeError::LayoutVersion
      | DecodeError::LayoutTag
      | DecodeError::LayoutChannels
      | DecodeError::LayoutReserved
      | DecodeError::UnusedHeaderBits
      | DecodeError::Shift
      | DecodeError::Mixing
      | DecodeError::Mode
      | DecodeError::Element
      | DecodeError::Channel
      | DecodeError::FrameLength => {
        let Ok(sentence) = self.sentence();

        f.write_str(sentence)
      }
    }
  }
}

impl From<io::Error> for DecodeError {
  fn from(fault: io::Error) -> Self {
    DecodeError::Stream(fault)
  }
}

impl From<Error> for DecodeError {
  fn from(fault: Error) -> Self {
    DecodeError::Output(fault)
  }
}

impl From<DecodeError> for Error {
  fn from(fault: DecodeError) -> Self {
    let Ok(sentence) = fault.sentence();

    match fault {
      DecodeError::Stream(source) => Error::IoError(source),
      DecodeError::Output(inner) => inner,
      DecodeError::CodecType
      | DecodeError::MissingExtraData
      | DecodeError::CookieSize
      | DecodeError::Version
      | DecodeError::ChannelCount => Error::Unsupported(sentence),
      DecodeError::BitDepth
      | DecodeError::LayoutSize
      | DecodeError::LayoutId
      | DecodeError::LayoutVersion
      | DecodeError::LayoutTag
      | DecodeError::LayoutChannels
      | DecodeError::LayoutReserved
      | DecodeError::UnusedHeaderBits
      | DecodeError::Shift
      | DecodeError::Mixing
      | DecodeError::Mode
      | DecodeError::Element
      | DecodeError::Channel
      | DecodeError::FrameLength => Error::DecodeError(sentence),
    }
  }
}
