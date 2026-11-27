//! What format a picture is in and how big it is, read off its first bytes.
//!
//! A picture says both in a header written before anything is compressed: a
//! signature that names the format, and a width and a height a few bytes
//! after it, or in JPEG's case in whichever segment starts the frame. So a
//! caller reads as much of the file as it is willing to, hands it here, and
//! is told the format or the size, or why the bytes did not say: they are no
//! format this reads, they end before the answer, or they say something no
//! picture would.
//!
//! The size is the one the picture is stored at. A JPEG or a WebP may carry
//! an orientation that turns it on its side when it is shown, and that is a
//! second question with an answer of its own; nothing here reads it, so a
//! caller asking whether a picture is square is told about the stored one.
//!
//! The crate began as imagesize, by Maiddog under MIT, carried in `forks/`
//! for the SVG reader, and it came into the tree under the rules. The fork
//! knew two dozen formats behind features and a reader that seeked; the SVG
//! reader and the application list between them ask about four, PNG, JPEG,
//! GIF and WebP, and the rest was left behind with the seeking. Applications
//! had a parser of its own for PNG and JPEG, and the two told formats apart
//! by different rules, so the rules are now one: see each format's module
//! for where they differed and which was kept.

mod gif;
mod jpeg;
mod png;
mod webp;

use console_core_never::Never;
use console_core_size::Size;
use console_core_words::Words;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum PictureFormat {
  #[words(says = "PNG")]
  Png,
  #[words(says = "JPEG")]
  Jpeg,
  #[words(says = "GIF")]
  Gif,
  #[words(says = "WebP")]
  Webp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeError {
  Unrecognised,
  Truncated,
  Corrupted,
}

impl fmt::Display for SizeError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      SizeError::Unrecognised => write!(to, "these bytes begin no picture format this reads"),
      SizeError::Truncated => {
        write!(to, "the picture's header ends before it says how big it is")
      }
      SizeError::Corrupted => write!(to, "the picture's header says something no picture would"),
    }
  }
}

impl std::error::Error for SizeError {}

pub fn format(head: &[u8]) -> Result<PictureFormat, SizeError> {
  let formats = [
    PictureFormat::Jpeg,
    PictureFormat::Png,
    PictureFormat::Gif,
    PictureFormat::Webp,
  ];

  match formats
    .into_iter()
    .find(|candidate| begins(*candidate, head) == Ok(Signature::Found))
  {
    Some(found) => Ok(found),
    None => Err(SizeError::Unrecognised),
  }
}

fn begins(candidate: PictureFormat, head: &[u8]) -> Result<Signature, Never> {
  match candidate {
    PictureFormat::Png => png::begins(head),
    PictureFormat::Jpeg => jpeg::begins(head),
    PictureFormat::Gif => gif::begins(head),
    PictureFormat::Webp => webp::begins(head),
  }
}

pub fn size(head: &[u8]) -> Result<Size<u32>, SizeError> {
  let found = format(head)?;

  match found {
    PictureFormat::Png => png::size(head),
    PictureFormat::Jpeg => jpeg::size(head),
    PictureFormat::Gif => gif::size(head),
    PictureFormat::Webp => webp::size(head),
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Signature {
  Found,
  Absent,
}

impl From<bool> for Signature {
  fn from(found: bool) -> Self {
    match bool::then_some(found, ()) {
      Some(()) => Self::Found,
      None => Self::Absent,
    }
  }
}

pub(crate) fn signature(head: &[u8], magic: &[u8]) -> Result<Signature, Never> {
  Ok(Signature::from(head.starts_with(magic)))
}

pub(crate) fn two(field: Option<&[u8]>) -> Result<[u8; 2], SizeError> {
  match field.and_then(|bytes| bytes.first_chunk::<2>()) {
    Some(bytes) => Ok(*bytes),
    None => Err(SizeError::Truncated),
  }
}

pub(crate) fn three(field: Option<&[u8]>) -> Result<[u8; 3], SizeError> {
  match field.and_then(|bytes| bytes.first_chunk::<3>()) {
    Some(bytes) => Ok(*bytes),
    None => Err(SizeError::Truncated),
  }
}

pub(crate) fn four(field: Option<&[u8]>) -> Result<[u8; 4], SizeError> {
  match field.and_then(|bytes| bytes.first_chunk::<4>()) {
    Some(bytes) => Ok(*bytes),
    None => Err(SizeError::Truncated),
  }
}
