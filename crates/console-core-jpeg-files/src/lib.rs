//! A JPEG, read into the pixels it holds, at the size it is wanted.
//!
//! Every photograph this desktop showed was read by ffmpeg, after ffprobe had
//! been asked how big it was: two programs started for a thumbnail a hundred
//! and twenty-eight pixels across. And most of what a JPEG holds is never
//! needed at that size. The format keeps a picture as blocks of eight by eight
//! frequencies, so a block can be drawn one, two or four pixels a side from its
//! lowest frequencies alone, which is what libvips does and why it is quick: a
//! 48 megapixel photograph drawn into a screen is read at a quarter of its
//! size, and never exists anywhere at the size it was taken.
//!
//! [`decoded`] is asked for the size the picture will be drawn into, and reads
//! it at the smallest of its own sizes that still covers that, upright: the
//! way the camera was held is in the EXIF, and the pixels come back turned to
//! it. [`measured`] says how big that upright picture is without decoding any
//! of it. [`decoded_as_stored`] reads an embedded JPEG at full resolution in scan
//! order. [`decoded_components`] returns interleaved samples with chroma upsampled
//! to that grid. It preserves both channels of a two-band image and the black
//! channel of CMYK and YCCK. [`Components::convert_ycbcr_to_rgb`] converts YCbCr
//! triples in place when the document requests a color transform.
//!
//! Baseline and progressive JPEGs are read, in grey, YCbCr, RGB or the CMYK a
//! print shop's software writes, with any chroma subsampling whose factors
//! divide each other. What nothing this desktop meets writes -- arithmetic
//! coding, twelve bits a sample, the lossless and the hierarchical processes --
//! is refused by name.
//!
//! Nothing here opens a file. It is handed bytes, and a stranger's bytes are
//! what it is for: every length the file states is checked against what is
//! there, and the memory a picture would take is counted before it is taken.
//! A file cut short is drawn as far as it goes, the way a browser draws one.
//!
//! [`decoded_spread`] is the same picture drawn by more than one hand. The bits
//! of a scan can only be read one after another, but the blocks they make and
//! the color of every row can be drawn anywhere, so they come back to the
//! caller as [`Task`]s for it to spread however it spreads work, with the
//! reading of the next round of rows as one of them. Nothing in here starts a
//! thread.
//!
//! [`encoded`] is the other way: RGBA written as the baseline JPEG libjpeg
//! writes when it is handed a [`Quality`] and nothing else, which is the JPEG
//! every reader reads. A quality is the one to a hundred every encoder takes,
//! and a number past either end is the end it is past.

#![cfg_attr(all(not(feature = "std"), not(test)), no_std)]

extern crate alloc;

use alloc::vec::Vec;

mod assembly;
mod bits;
#[cfg(feature = "std")]
mod entropy;
mod exif;
#[cfg(feature = "std")]
mod fdct;
mod frame;
mod huffman;
mod idct;
mod kept;
mod scans;
mod segments;
mod strips;
#[cfg(feature = "std")]
mod writing;

use console_core_geometry::Size;
use console_core_never::Never;

pub use crate::strips::{Spread, Task};
pub use assembly::ColorSpace;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JpegError {
  NotAJpeg,
  Truncated,
  Corrupt,
  TooLarge,
  Unsupported(Unsupported),
  Mismatched,
  TaskBusy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
  ArithmeticCoding,
  Lossless,
  Hierarchical,
  TwelveBits,
  Components(u8),
  Sampling,
  HeightAfterwards,
}

impl core::fmt::Display for JpegError {
  fn fmt(&self, to: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      JpegError::NotAJpeg => write!(to, "this is not a JPEG"),
      JpegError::Truncated => write!(to, "the file ends before the picture in it begins"),
      JpegError::Corrupt => write!(to, "the picture in here is damaged"),
      JpegError::TooLarge => write!(
        to,
        "this picture would take more memory than any photograph"
      ),
      JpegError::Unsupported(how) => {
        write!(to, "this JPEG is written {how}, which is not read here")
      }
      JpegError::Mismatched => write!(
        to,
        "the pixels handed over are not the picture their size says"
      ),
      JpegError::TaskBusy => write!(to, "the JPEG executor started the same task concurrently"),
    }
  }
}

impl core::fmt::Display for Unsupported {
  fn fmt(&self, to: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Unsupported::ArithmeticCoding => write!(to, "with arithmetic coding"),
      Unsupported::Lossless => write!(to, "losslessly"),
      Unsupported::Hierarchical => write!(to, "hierarchically"),
      Unsupported::TwelveBits => write!(to, "at twelve bits a sample"),
      Unsupported::Components(count) => write!(to, "with {count} components"),
      Unsupported::Sampling => write!(to, "with sampling factors that do not divide each other"),
      Unsupported::HeightAfterwards => write!(to, "with its height after the first scan"),
    }
  }
}

impl core::error::Error for JpegError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quality(u8);

impl Quality {
  pub fn percent(percent: u8) -> Result<Quality, Never> {
    Ok(Quality(percent.clamp(1, 100)))
  }
}

pub use console_core_pixel_buffers::PackedPixels as Picture;

pub fn measured(bytes: &[u8]) -> Result<Size<u32>, JpegError> {
  let read = segments::read(
    bytes,
    segments::Purpose::Measuring,
    &strips::one_after_another,
  )?;

  match read.stage {
    segments::Stage::Framed(frame) => {
      let Ok(upright) = read.orientation.upright(frame.size);

      Ok(upright)
    }
    segments::Stage::Opening | segments::Stage::Drawing(_) => Err(JpegError::Truncated),
  }
}

#[cfg(feature = "std")]
pub fn encoded(picture: &Picture, quality: Quality) -> Result<Vec<u8>, JpegError> {
  writing::encoded(picture, quality)
}

pub fn decoded(bytes: &[u8], covering: Size<u32>) -> Result<Picture, JpegError> {
  decoded_spread(bytes, covering, &strips::one_after_another)
}

pub fn decoded_as_stored(bytes: &[u8]) -> Result<Picture, JpegError> {
  decode(bytes, segments::Purpose::Stored, &strips::one_after_another)
}

pub fn decoded_spread(
  bytes: &[u8],
  covering: Size<u32>,
  spread: Spread<'_>,
) -> Result<Picture, JpegError> {
  decode(bytes, segments::Purpose::Drawing(covering), spread)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Components {
  pub size: Size<u32>,
  pub color: ColorSpace,
  pub bytes: Vec<u8>,
}

impl Components {
  pub fn convert_ycbcr_to_rgb(&mut self) -> Result<(), Never> {
    match self.color {
      ColorSpace::YCbCr => {
        for triple in self.bytes.as_chunks_mut::<3>().0 {
          let Ok(rgb) = assembly::converted(*triple);
          *triple = rgb;
        }

        self.color = ColorSpace::Rgb;
      }
      ColorSpace::Grey
      | ColorSpace::Rgb
      | ColorSpace::Cmyk
      | ColorSpace::Ycck
      | ColorSpace::TwoComponent => {}
    }

    Ok(())
  }
}

pub fn decoded_components(jpeg: &[u8]) -> Result<Components, JpegError> {
  let (color, size, bytes) = raster(
    jpeg,
    segments::Purpose::Components,
    &strips::one_after_another,
  )?;
  Ok(Components { color, size, bytes })
}

fn decode(
  jpeg: &[u8],
  purpose: segments::Purpose,
  spread: Spread<'_>,
) -> Result<Picture, JpegError> {
  let (_, size, bytes) = raster(jpeg, purpose, spread)?;
  Ok(Picture { size, bytes })
}

fn raster(
  bytes: &[u8],
  purpose: segments::Purpose,
  spread: Spread<'_>,
) -> Result<(ColorSpace, Size<u32>, Vec<u8>), JpegError> {
  let read = segments::read(bytes, purpose, spread)?;
  let segments::Reading {
    stage,
    tables,
    adobe,
    ..
  } = read;

  let image = match stage {
    segments::Stage::Drawing(image) => image,
    segments::Stage::Opening | segments::Stage::Framed(_) => return Err(JpegError::Truncated),
  };

  let colors = assembly::colors(&image.frame, adobe)?;

  let (size, pixels) = scans::finished(image, &tables, colors, spread)?;
  Ok((colors, size, pixels))
}
