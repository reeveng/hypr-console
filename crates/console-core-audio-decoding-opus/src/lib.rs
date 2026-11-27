//! Clean-room pure-Rust Opus decoder (RFC 6716, updated by RFC 8251).
//!
//! An `OpusHead` packet from an Ogg or Matroska file makes a [`Decoder`], and
//! each packet after it comes back as planar samples at 48 kHz. The decoder
//! the crate was carried as also made decoders for other output rates, read
//! `dOps` boxes, wrote headers back, decoded the in-band redundancy of a lost
//! packet and reported the range coder's final state for conformance runs;
//! nothing here asks for any of it, and what nothing reaches is gone.

mod celt;
mod decoder;
mod header;
mod multistream;
mod packet;
mod quantities;
mod range;
mod silk;

pub use multistream::Decoder;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
  EmptyPacket,
  MalformedFraming,
  TruncatedMultistream,
  DurationsDiffer,
  CeltFrameTooLong,
  CeltFrameSize,
  MissingMagic,
  ZeroChannels,
  TooManyChannelsForSingleStream,
  TruncatedMapping,
  BadStreamCounts,
  SurroundChannels,
  MappingOutOfRange,
  MajorVersion,
  ProjectionFamily,
}

impl Error {
  pub fn description(self) -> Result<&'static str, console_core_never::Never> {
    Ok(match self {
      Error::EmptyPacket => "invalid packet: empty packet",
      Error::MalformedFraming => "invalid packet: malformed packet framing",
      Error::TruncatedMultistream => "invalid packet: truncated multistream packet",
      Error::DurationsDiffer => "invalid packet: multistream frame durations differ",
      Error::CeltFrameTooLong => "invalid packet: CELT frame too long",
      Error::CeltFrameSize => "invalid argument: CELT frame size",
      Error::MissingMagic => "invalid header: missing OpusHead magic",
      Error::ZeroChannels => "invalid header: zero channels",
      Error::TooManyChannelsForSingleStream => {
        "invalid header: family 0 allows at most 2 channels"
      }
      Error::TruncatedMapping => "invalid header: truncated channel mapping table",
      Error::BadStreamCounts => "invalid header: bad stream counts",
      Error::SurroundChannels => "invalid header: family 1 allows 1 to 8 channels",
      Error::MappingOutOfRange => "invalid header: channel mapping out of range",
      Error::MajorVersion => "unsupported: OpusHead major version",
      Error::ProjectionFamily => "unsupported: channel mapping family 3 (projection)",
    })
  }
}

impl std::fmt::Display for Error {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let Ok(sentence) = self.description();
    to.write_str(sentence)
  }
}

impl std::error::Error for Error {}
