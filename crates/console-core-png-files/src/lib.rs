//! A PNG, read into the pixels it holds.
//!
//! A screenshot is a PNG, and so is most of what a person saves off a page or
//! is sent in a chat. Every one of them was read by ffmpeg, after ffprobe had
//! been asked how big it was: two programs started to read a format that is a
//! few chunks and a deflate stream, which `console-core-deflate` already
//! undoes for the books.
//!
//! [`decoded`] reads the whole picture into RGBA, eight bits a channel,
//! whatever the file keeps: grey, grey with alpha, RGB, RGBA or a palette, at
//! any bit depth the format allows, interlaced or not. A palette's alpha and
//! the one transparent grey or colour a `tRNS` chunk names come back as alpha.
//! Sixteen bits a channel is rounded to the nearest of eight. [`measured`] says
//! how big the picture is from its header alone.
//!
//! What a PNG says about colour -- its gamma, its chromaticities, an ICC
//! profile -- is not applied, as ffmpeg did not apply it either. An animated
//! PNG is its default image, which is what a reader that knows nothing of
//! animation draws.
//!
//! Nothing here opens a file. It is handed bytes, and a stranger's bytes are
//! what it is for: every chunk is checked against the CRC32 it carries, every
//! length against what is there, and the memory a picture would take is
//! counted before it is taken. A file cut short is refused rather than drawn
//! in part.
//!
//! [`encoded`] is the other way: RGBA written as a PNG at eight bits a channel,
//! with only the channels the picture uses, each row filtered whichever way
//! leaves it smallest and the whole deflated by `console-core-deflate`. It
//! is what a thumbnail and an edited photograph are saved as, where ffmpeg and
//! the image crate wrote them before.

mod chunks;
mod filters;
mod pixels;

use console_core_deflate::{DeflateError, zlib};
use console_core_geometry::Size;
use console_core_number_conversion::index;
use console_core_pixel_buffers::{BYTES_PER_PIXEL, Packing};

use crate::chunks::{Header, Lacing};

const MOST_BYTES: u64 = 1 << 30;

const EIGHT_BITS: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PngError {
  NotGreyscale,
  NotAPng,
  Truncated,
  Corrupt,
  TooLarge,
  Mismatched,
  InvalidText,
  Deflate(DeflateError),
}

impl std::fmt::Display for PngError {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      PngError::NotAPng => write!(to, "this is not a PNG"),
      PngError::Truncated => write!(to, "the file ends before the picture in it does"),
      PngError::Corrupt => write!(to, "the picture in here is damaged"),
      PngError::TooLarge => write!(to, "this picture would take more memory than any picture"),
      PngError::Mismatched => write!(
        to,
        "the pixels handed over are not the picture their size says"
      ),
      PngError::NotGreyscale => write!(to, "expected a greyscale PNG without transparency"),
      PngError::InvalidText => write!(to, "the text is not a PNG keyword and Latin-1 value"),
      PngError::Deflate(why) => write!(to, "the pixels could not be deflated: {why}"),
    }
  }
}

impl std::error::Error for PngError {}

pub use console_core_pixel_buffers::PackedPixels as Picture;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Text<'a> {
  pub keyword: &'a str,
  pub value: &'a str,
}

pub fn measured(bytes: &[u8]) -> Result<Size<u32>, PngError> {
  let header = chunks::header(bytes)?;

  Ok(header.size)
}

pub fn decoded(bytes: &[u8]) -> Result<Picture, PngError> {
  decoded_rgba(bytes, Reduction::Nearest)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Reduction {
  Nearest,
  HighByte,
}

pub fn decoded_rgba(bytes: &[u8], reduction: Reduction) -> Result<Picture, PngError> {
  let read = chunks::read(bytes)?;
  decoded_pixels(read, reduction)
}

pub fn decoded_greyscale(bytes: &[u8]) -> Result<(Size<u32>, Vec<u8>), PngError> {
  let read = chunks::read(bytes)?;

  match (read.header.color, read.key) {
    (chunks::Color::Grey, None) => {}
    _ => return Err(PngError::NotGreyscale),
  }

  let picture = decoded_pixels(read, Reduction::HighByte)?;
  let grey = picture
    .bytes
    .as_chunks::<4>()
    .0
    .iter()
    .map(|[grey, ..]| *grey)
    .collect();
  Ok((picture.size, grey))
}

fn decoded_pixels(mut read: chunks::Read, reduction: Reduction) -> Result<Picture, PngError> {
  let filtered = filters::filtered_length(&read.header)?;

  let rgba_bytes = affordable(&read.header, filtered)?;
  let unpacked = unpacked(std::mem::take(&mut read.packed), filtered)?;
  let Ok(length) = index(rgba_bytes);
  #[cfg_attr(
    dylint_lib = "explicit088_no_arriving_length",
    expect(
      explicit088_no_arriving_length,
      reason = "affordable checks the combined byte count of the RGBA output and inflated rows against MOST_BYTES and returns the checked RGBA length"
    )
  )]
  let mut rgba = vec![0u8; length];

  pixels::painted(&read, unpacked, &mut rgba, reduction)?;

  Ok(Picture {
    size: read.header.size,
    bytes: rgba,
  })
}

pub fn encoded(picture: &Picture) -> Result<Vec<u8>, PngError> {
  encoded_with_text(picture, &[])
}

pub fn encoded_with_text(picture: &Picture, text: &[Text<'_>]) -> Result<Vec<u8>, PngError> {
  let Ok(packing) = picture.packing();

  match packing {
    Packing::Complete => {}
    Packing::Mismatched => return Err(PngError::Mismatched),
  }

  let Ok(kept) = pixels::fewest(&picture.bytes);
  let Ok(color) = kept.color();
  let header = encoding_header(picture.size, color)?;
  let Ok(raw) = pixels::unpainted(&picture.bytes, kept);
  write_channels(&header, &raw, text)
}

pub fn encoded_rgba(size: Size<u32>, bytes: &[u8]) -> Result<Vec<u8>, PngError> {
  let header = encoding_header(size, chunks::Color::Rgba)?;
  write_channels(&header, bytes, &[])
}

pub fn encoded_greyscale(size: Size<u32>, bytes: &[u8]) -> Result<Vec<u8>, PngError> {
  let header = encoding_header(size, chunks::Color::Grey)?;
  write_channels(&header, bytes, &[])
}

fn encoding_header(size: Size<u32>, color: chunks::Color) -> Result<Header, PngError> {
  let header = Header {
    size,
    depth: EIGHT_BITS,
    color,
    lacing: Lacing::Straight,
  };
  let filtered = filters::filtered_length(&header)?;

  let _rgba_bytes = affordable(&header, filtered)?;

  Ok(header)
}

fn write_channels(header: &Header, raw: &[u8], text: &[Text<'_>]) -> Result<Vec<u8>, PngError> {
  let size = header.size;
  let Ok(row_bytes) = filters::row_length(header, size.width);
  let Ok(length) = console_core_number_conversion::fixed_index(raw.len());
  let expected = row_bytes.checked_mul(u64::from(size.height));

  match ::core::primitive::bool::then_some(
    expected == Some(length) && size.width > 0 && size.height > 0,
    (),
  ) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(PngError::Mismatched),
  }

  let Ok(filtered) = filters::filtered(raw, header);
  let packed = packed(&filtered)?;

  chunks::written(header, &packed, text)
}

fn packed(filtered: &[u8]) -> Result<Vec<u8>, PngError> {
  zlib::encode(filtered).map_err(PngError::Deflate)
}

fn affordable(header: &Header, filtered: u64) -> Result<u64, PngError> {
  let area = u64::from(header.size.width).saturating_mul(u64::from(header.size.height));
  let rgba = area.saturating_mul(u64::from(BYTES_PER_PIXEL));

  match ::core::primitive::bool::then_some(rgba.saturating_add(filtered) <= MOST_BYTES, ()) {
    ::core::option::Option::Some(()) => Ok(rgba),
    ::core::option::Option::None => Err(PngError::TooLarge),
  }
}

fn unpacked(packed: Vec<u8>, filtered: u64) -> Result<Vec<u8>, PngError> {
  let size = match u32::try_from(filtered) {
    Ok(size) => size,
    Err(_more_than_four_gigabytes) => return Err(PngError::TooLarge),
  };
  let unpacked = match zlib::decode(&packed, size) {
    Ok(unpacked) => unpacked,
    Err(DeflateError::Truncated) => return Err(PngError::Truncated),
    Err(_damaged) => return Err(PngError::Corrupt),
  };

  match ::core::primitive::bool::then_some(u64::try_from(unpacked.len()) == Ok(filtered), ()) {
    ::core::option::Option::Some(()) => Ok(unpacked),
    ::core::option::Option::None => Err(PngError::Truncated),
  }
}
