//! The ways a frame is refused: the bitstream ended early, it says something the specification
//! does not allow, or it asks for a reference this decoder was never given.

use std::fmt;

use console_core_video_decoding_bitstream::BitError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
  Bitstream(BitError),
  Invalid(Invalid),
  MissingReference(MissingReference),
}

impl fmt::Display for Error {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Bitstream(value) => write!(to, "bitstream error: {value}"),
      Self::Invalid(value) => write!(to, "invalid bitstream: {value}"),
      Self::MissingReference(value) => write!(to, "missing reference frame: {value}"),
    }
  }
}

impl std::error::Error for Error {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      Self::Bitstream(value) => Some(value),
      Self::Invalid(value) => Some(value),
      Self::MissingReference(value) => Some(value),
    }
  }
}

impl From<BitError> for Error {
  fn from(value: BitError) -> Self {
    Self::Bitstream(value)
  }
}

impl From<Invalid> for Error {
  fn from(value: Invalid) -> Self {
    Self::Invalid(value)
  }
}

impl From<MissingReference> for Error {
  fn from(value: MissingReference) -> Self {
    Self::MissingReference(value)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invalid {
  FrameMarker,
  ReservedProfileBit,
  SyncCode,
  SubsampledInProfile { profile: u8 },
  ReservedColorBit,
  RgbInProfile { profile: u8 },
  EmptyCompressedHeader,
  TooLarge { width: u32, height: u32 },
  CompressedHeaderBeyondData,
  ReferenceFormat,
  TileBeyondData { size: u64 },
  TileDataExhausted,
}

impl fmt::Display for Invalid {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::FrameMarker => write!(to, "invalid frame marker"),
      Self::ReservedProfileBit => write!(to, "reserved bit set after profile"),
      Self::SyncCode => write!(to, "invalid frame sync code"),
      Self::SubsampledInProfile { profile } => {
        write!(to, "4:2:0 subsampling in profile {profile}")
      }
      Self::ReservedColorBit => write!(to, "reserved bit set in color config"),
      Self::RgbInProfile { profile } => write!(to, "RGB color space in profile {profile}"),
      Self::EmptyCompressedHeader => write!(to, "empty compressed header"),
      Self::TooLarge { width, height } => write!(to, "frame size {width}x{height} too large"),
      Self::CompressedHeaderBeyondData => write!(to, "compressed header exceeds frame data"),
      Self::ReferenceFormat => {
        write!(to, "reference frame format differs from the current frame")
      }
      Self::TileBeyondData { size } => write!(to, "tile size {size} beyond frame data"),
      Self::TileDataExhausted => write!(to, "tile data exhausted"),
    }
  }
}

impl std::error::Error for Invalid {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingReference {
  SizeFromEmptySlot { slot: u8 },
  ShownEmptySlot { slot: u8 },
  EmptySlot { slot: u8 },
  Unusable { reference: i8 },
}

impl fmt::Display for MissingReference {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::SizeFromEmptySlot { slot } => write!(to, "frame size from empty slot {slot}"),
      Self::ShownEmptySlot { slot } => write!(to, "show_existing_frame of empty slot {slot}"),
      Self::EmptySlot { slot } => write!(to, "reference slot {slot} is empty"),
      Self::Unusable { reference } => write!(to, "block uses unusable reference {reference}"),
    }
  }
}

impl std::error::Error for MissingReference {}

pub type Result<T> = std::result::Result<T, Error>;
