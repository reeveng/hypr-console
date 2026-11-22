//! A JBIG2 image, the bi-level coding scanned pages in a PDF are kept in, decoded into its pixels.
//!
//! ITU-T T.88 cuts a page into segments: a page information segment saying how large it is, then
//! regions of four kinds laid on it, generic ones coded pixel by pixel from the ones around them,
//! refinements of another bitmap, text made of the bitmaps in symbol dictionaries, and halftones made
//! of the patterns in pattern dictionaries, and the Huffman tables the text and the dictionaries may
//! ask for. A PDF stream is the embedded organisation, segments one after another with the segments
//! shared between its images in a second stream, which is the only one [`Image`] reads.
//!
//! The crate began as hayro-jbig2, by Laurenz Stampfl under MIT or Apache-2.0, carried in `forks/` for
//! the PDF reader, and it came into the tree under the rules. What the fork did for speed went, and
//! what was left is one bitmap that holds a pixel to a byte, so a decoded region, a symbol and a
//! pattern are all the same thing and a region is laid on another with the one `combine`, where the
//! fork packed words, kept a path for a region that covers a pristine page and shifted every pattern
//! thirty-two ways ahead of time. A context is still not gathered again for every pixel:
//! [`neighbourhood`] works out which of its bits the pixel before had a column to the left and shifts
//! those. Every walk now says where it stops. The fork ran for ever on a symbol dictionary of empty
//! height classes and on export runs of nothing, because it asked the stream when to end and an
//! exhausted arithmetic decoder never says so; a dictionary now has as many height classes and
//! export runs as the symbols it declares can need.
//!
//! The standard Huffman tables are the lines of Annex B, ordered as their codes are handed out, and
//! made into a tree when a segment asks for one rather than written out as fifteen trees by a
//! script. The file organisations, the `image` crate's decoder trait, the SIMD paths and the context
//! the fork let a caller reuse between pages were left behind: nothing here opens a standalone file,
//! and a page is decoded once.
//!
//! Pixels are handed to a [`Decoder`] as the fax decoder hands them, a colour at a time and in runs
//! of eight wherever a run starts on a byte, which is what both the PDF's JBIG2 filter and the
//! reader's CCITT filter already implement.

mod arithmetic;
mod bitmap;
mod error;
mod generic;
mod halftone;
mod huffman;
mod integers;
mod mmr;
mod neighbourhood;
mod page;
mod reading;
mod refinement;
mod regions;
mod segments;
mod standard;
mod symbols;
mod text;

pub use console_core_fax_images::{Color, Decoder};

pub use crate::error::DecodeError;

use console_core_geometry::{Point, Size};
use console_core_never::Never;

use crate::bitmap::Bitmap;
use crate::segments::{Kind, Segment};

#[derive(Debug)]
pub struct Image<'a> {
  segments: Vec<Segment<'a>>,
  size: Size<u32>,
  default_pixel: Color,
}

impl<'a> Image<'a> {
  pub fn new_embedded(
    data: &'a [u8],
    globals: Option<&'a [u8]>,
  ) -> Result<Image<'a>, DecodeError> {
    let mut segments = Vec::new();

    match globals {
      Some(shared) => segments::parse_all(shared, &mut segments)?,
      None => {}
    }

    segments::parse_all(data, &mut segments)?;
    segments.sort_by_key(|segment| segment.header.number);

    Image::from_segments(segments)
  }

  fn from_segments(segments: Vec<Segment<'a>>) -> Result<Image<'a>, DecodeError> {
    let stripes = segments
      .iter()
      .filter(|segment| segment.header.kind == Kind::EndOfStripe)
      .filter_map(|segment| {
        let Ok(height) = stripe_height(segment.data);

        height
      })
      .max();
    let information = segments
      .iter()
      .find(|segment| segment.header.kind == Kind::PageInformation)
      .ok_or(DecodeError::MissingPageInformation)?;
    let page = segments::parse_page(information.data)?;
    let height = match page.size.height {
      0xFFFF_FFFF => stripes.ok_or(DecodeError::UnknownPageHeight)?,
      known => known,
    };

    match (page.size.width, height) {
      (0, _) | (_, 0) => Err(DecodeError::EmptyPage),
      (width, height_2) => Ok(Image {
        segments,
        size: Size {
          width,
          height: height_2,
        },
        default_pixel: page.default_pixel,
      }),
    }
  }

  pub fn size(&self) -> Result<Size<u32>, Never> {
    Ok(self.size)
  }

  pub fn decode(&self, decoder: &mut impl Decoder) -> Result<(), DecodeError> {
    let page = Bitmap::new(self.size, Point { x: 0, y: 0 }, self.default_pixel)?;
    let page_2 = page::decode(&self.segments, page)?;
    let Ok(()) = page_2.emit(decoder);

    Ok(())
  }
}

fn stripe_height(data: &[u8]) -> Result<Option<u32>, Never> {
  Ok(match <[u8; 4]>::try_from(data) {
    Ok(bytes) => u32::from_be_bytes(bytes).checked_add(1),
    Err(_short) => None,
  })
}
