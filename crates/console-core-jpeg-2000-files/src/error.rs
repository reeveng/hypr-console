//! Error types for JPEG 2000 decoding.

use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
  Format(FormatError),

  Marker(MarkerError),

  Tile(TileError),

  Validation(ValidationError),

  Decoding(DecodingError),

  Color(ColorError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatError {
  InvalidSignature,

  InvalidFileType,

  InvalidBox,

  MissingCodestream,

  Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerError {
  Invalid,

  Unsupported,

  Expected(&'static str),

  Missing(&'static str),

  ParseFailure(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileError {
  Invalid,

  InvalidIndex,

  InvalidOffsets,

  PpmPptConflict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationError {
  InvalidDimensions,

  ImageTooLarge,

  TooManyChannels,

  InvalidComponentMetadata,

  InvalidProgressionOrder,

  InvalidTransformation,

  InvalidQuantizationStyle,

  MissingPrecinctExponents,

  InsufficientExponents,

  MissingStepSize,

  InvalidExponents,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodingError {
  CodeBlockDecodeFailure,

  TooManyBitplanes,

  TooManyCodingPasses,

  InvalidBitplaneCount,

  InvalidPrecinct,

  InvalidProgressionIterator,

  UnexpectedEof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorError {
  Mct,

  PaletteResolutionFailed,

  SyccConversionFailed,

  LabConversionFailed,
}

impl fmt::Display for DecodeError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Format(e) => write!(f, "{e}"),
      Self::Marker(e) => write!(f, "{e}"),
      Self::Tile(e) => write!(f, "{e}"),
      Self::Validation(e) => write!(f, "{e}"),
      Self::Decoding(e) => write!(f, "{e}"),
      Self::Color(e) => write!(f, "{e}"),
    }
  }
}

impl fmt::Display for FormatError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::InvalidSignature => write!(f, "invalid JP2 signature"),
      Self::InvalidFileType => write!(f, "invalid JP2 file type"),
      Self::InvalidBox => write!(f, "invalid JP2 box"),
      Self::MissingCodestream => write!(f, "missing codestream data"),
      Self::Unsupported => write!(f, "unsupported JP2 image"),
    }
  }
}

impl fmt::Display for MarkerError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Invalid => write!(f, "invalid marker"),
      Self::Unsupported => write!(f, "unsupported marker"),
      Self::Expected(marker) => write!(f, "expected {marker} marker"),
      Self::Missing(marker) => write!(f, "missing {marker} marker"),
      Self::ParseFailure(marker) => write!(f, "failed to parse {marker} marker"),
    }
  }
}

impl fmt::Display for TileError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Invalid => write!(f, "image contains no tiles"),
      Self::InvalidIndex => write!(f, "invalid tile index in tile-part header"),
      Self::InvalidOffsets => write!(f, "invalid tile offsets"),
      Self::PpmPptConflict => {
        write!(
          f,
          "PPT marker present when PPM marker exists in main header"
        )
      }
    }
  }
}

impl fmt::Display for ValidationError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::InvalidDimensions => write!(f, "invalid image dimensions"),
      Self::ImageTooLarge => write!(f, "image is too large"),
      Self::TooManyChannels => write!(f, "image has too many channels"),
      Self::InvalidComponentMetadata => write!(f, "invalid component metadata"),
      Self::InvalidProgressionOrder => write!(f, "invalid progression order"),
      Self::InvalidTransformation => write!(f, "invalid transformation type"),
      Self::InvalidQuantizationStyle => write!(f, "invalid quantization style"),
      Self::MissingPrecinctExponents => {
        write!(f, "missing exponents for precinct sizes")
      }
      Self::InsufficientExponents => {
        write!(f, "not enough exponents provided in header")
      }
      Self::MissingStepSize => write!(f, "missing exponent step size"),
      Self::InvalidExponents => write!(f, "invalid quantization exponents"),
    }
  }
}

impl fmt::Display for DecodingError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::CodeBlockDecodeFailure => write!(f, "failed to decode code-block"),
      Self::TooManyBitplanes => write!(f, "number of bitplanes is too large"),
      Self::TooManyCodingPasses => {
        write!(f, "code-block contains too many coding passes")
      }
      Self::InvalidBitplaneCount => write!(f, "invalid number of bitplanes"),
      Self::InvalidPrecinct => write!(f, "a precinct was invalid"),
      Self::InvalidProgressionIterator => {
        write!(f, "a progression iterator was invalid")
      }
      Self::UnexpectedEof => write!(f, "unexpected end of data"),
    }
  }
}

impl fmt::Display for ColorError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Mct => write!(f, "multi-component transform failed"),
      Self::PaletteResolutionFailed => write!(f, "failed to resolve palette indices"),
      Self::SyccConversionFailed => write!(f, "failed to convert from sYCC to RGB"),
      Self::LabConversionFailed => write!(f, "failed to convert from LAB to RGB"),
    }
  }
}

impl core::error::Error for DecodeError {}

impl core::error::Error for FormatError {}

impl core::error::Error for MarkerError {}

impl core::error::Error for TileError {}

impl core::error::Error for ValidationError {}

impl core::error::Error for DecodingError {}

impl core::error::Error for ColorError {}

impl From<FormatError> for DecodeError {
  fn from(e: FormatError) -> Self {
    Self::Format(e)
  }
}

impl From<MarkerError> for DecodeError {
  fn from(e: MarkerError) -> Self {
    Self::Marker(e)
  }
}

impl From<TileError> for DecodeError {
  fn from(e: TileError) -> Self {
    Self::Tile(e)
  }
}

impl From<ValidationError> for DecodeError {
  fn from(e: ValidationError) -> Self {
    Self::Validation(e)
  }
}

impl From<DecodingError> for DecodeError {
  fn from(e: DecodingError) -> Self {
    Self::Decoding(e)
  }
}

impl From<ColorError> for DecodeError {
  fn from(e: ColorError) -> Self {
    Self::Color(e)
  }
}

pub type Result<T> = core::result::Result<T, DecodeError>;
