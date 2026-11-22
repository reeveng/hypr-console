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
pub(crate) enum Fault {
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

impl Fault {
  fn sentence(&self) -> Result<&'static str, Never> {
    Ok(match self {
      Fault::CodecType => "alac: invalid codec type",
      Fault::MissingExtraData => "alac: missing extra data",
      Fault::CookieSize => "alac: invalid magic cookie size",
      Fault::Version => "alac: not compatible with alac version 0",
      Fault::BitDepth => "alac: invalid bit depth",
      Fault::ChannelCount => "alac: more than 8 channels",
      Fault::LayoutSize => "alac: invalid channel layout info size",
      Fault::LayoutId => "alac: invalid channel layout info id",
      Fault::LayoutVersion => "alac: invalid channel layout info version",
      Fault::LayoutTag => "alac: invalid channel layout tag",
      Fault::LayoutChannels => "alac: the number of channels differs from the channel layout",
      Fault::LayoutReserved => "alac: reserved values in channel layout info are not 0",
      Fault::UnusedHeaderBits => "alac: unused header bits not 0",
      Fault::Shift => "alac: invalid shift value",
      Fault::Mixing => "alac: invalid mixing information for mono channel",
      Fault::Mode => "alac: invalid mode",
      Fault::Element => "alac: unsupported element",
      Fault::Channel => "alac: element has no channel to decode into",
      Fault::FrameLength => "alac: frame does not fit the frame length",
      Fault::Stream(_) => "alac: stream could not be read",
      Fault::Output(_) => "alac: output buffer could not be rendered",
    })
  }
}

impl fmt::Display for Fault {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Fault::Stream(source) => source.fmt(f),
      Fault::Output(inner) => inner.fmt(f),
      Fault::CodecType
      | Fault::MissingExtraData
      | Fault::CookieSize
      | Fault::Version
      | Fault::BitDepth
      | Fault::ChannelCount
      | Fault::LayoutSize
      | Fault::LayoutId
      | Fault::LayoutVersion
      | Fault::LayoutTag
      | Fault::LayoutChannels
      | Fault::LayoutReserved
      | Fault::UnusedHeaderBits
      | Fault::Shift
      | Fault::Mixing
      | Fault::Mode
      | Fault::Element
      | Fault::Channel
      | Fault::FrameLength => {
        let Ok(sentence) = self.sentence();

        f.write_str(sentence)
      }
    }
  }
}

impl From<io::Error> for Fault {
  fn from(fault: io::Error) -> Self {
    Fault::Stream(fault)
  }
}

impl From<Error> for Fault {
  fn from(fault: Error) -> Self {
    Fault::Output(fault)
  }
}

impl From<Fault> for Error {
  fn from(fault: Fault) -> Self {
    let Ok(sentence) = fault.sentence();

    match fault {
      Fault::Stream(source) => Error::IoError(source),
      Fault::Output(inner) => inner,
      Fault::CodecType
      | Fault::MissingExtraData
      | Fault::CookieSize
      | Fault::Version
      | Fault::ChannelCount => Error::Unsupported(sentence),
      Fault::BitDepth
      | Fault::LayoutSize
      | Fault::LayoutId
      | Fault::LayoutVersion
      | Fault::LayoutTag
      | Fault::LayoutChannels
      | Fault::LayoutReserved
      | Fault::UnusedHeaderBits
      | Fault::Shift
      | Fault::Mixing
      | Fault::Mode
      | Fault::Element
      | Fault::Channel
      | Fault::FrameLength => Error::DecodeError(sentence),
    }
  }
}
