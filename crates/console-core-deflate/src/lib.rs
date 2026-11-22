//! Shared DEFLATE compression and bounded decompression.
//!
//! ZIP entries, PNG images, and embedded glyph bitmaps use the same compression
//! format. The encoder and decoder live here independently of those containers.
//! Raw RFC 1951 streams use [`deflate`] and [`inflate`]; [`zlib`] adds RFC 1950
//! headers and verifies the Adler-32 checksum. Every decode has an output limit.

mod deflate;
mod inflate;
pub mod zlib;

pub use deflate::deflate;
pub use inflate::inflate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeflateError {
  Truncated,
  Corrupt,
  OutputLimitExceeded,
  TooLarge,
  Uncoded,
  UnsupportedDictionary,
  ChecksumMismatch,
}

impl std::fmt::Display for DeflateError {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Truncated => write!(to, "the compressed stream ends before it is complete"),
      Self::Corrupt => write!(to, "the compressed stream is damaged"),
      Self::OutputLimitExceeded => write!(to, "the stream exceeds its output limit"),
      Self::TooLarge => write!(to, "the stream is larger than four gigabytes"),
      Self::Uncoded => write!(to, "a symbol has no compression code"),
      Self::UnsupportedDictionary => {
        write!(to, "preset compression dictionaries are unsupported")
      }
      Self::ChecksumMismatch => write!(to, "the decompressed bytes do not match their checksum"),
    }
  }
}

impl std::error::Error for DeflateError {}
