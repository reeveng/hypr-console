//! A GIF's first frame, read into the pixels it holds.
//!
//! A GIF is a screen of a given size and frames drawn onto it, each a
//! rectangle of palette indices packed with LZW. What a still is made of -- a
//! thumbnail, a cover, the viewer before anything plays -- is the first frame,
//! so that is the one read, and it is drawn where it sits on the screen with
//! nothing under it: the rest of the screen is clear, which is how a browser
//! shows it. Its palette is its own if it brought one and the screen's if
//! not, and the index a graphic control extension names as transparent is
//! clear.
//!
//! [`decoded`] reads it into RGBA at the size of the screen. [`measured`]
//! says how big the screen is from the first thirteen bytes alone.
//!
//! Nothing here opens a file. It is handed bytes, and a stranger's bytes are
//! what it is for: the memory the screen takes is counted before it is taken,
//! a code the table has not reached and an index past the end of the palette
//! are damage, and a frame whose codes end before its pixels do is refused
//! rather than drawn in part.

mod blocks;
mod lzw;

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::index;

use crate::blocks::{Frame, Lacing, Screen};

const MOST_BYTES: u64 = 1 << 30;

const RGBA: u64 = 4;

const CLEAR: [u8; 4] = [0, 0, 0, 0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GifError {
  NotAGif,
  Truncated,
  Corrupt,
  TooLarge,
}

impl std::fmt::Display for GifError {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      GifError::NotAGif => write!(to, "this is not a GIF"),
      GifError::Truncated => write!(to, "the file ends before the picture in it does"),
      GifError::Corrupt => write!(to, "the picture in here is damaged"),
      GifError::TooLarge => write!(to, "this picture would take more memory than any picture"),
    }
  }
}

impl std::error::Error for GifError {}

pub use console_core_pixel_buffers::PackedPixels as Picture;

pub fn measured(bytes: &[u8]) -> Result<Size<u32>, GifError> {
  let (size, _flags) = blocks::described(bytes)?;

  Ok(size)
}

pub fn decoded(bytes: &[u8]) -> Result<Picture, GifError> {
  decode(bytes, Extent::Canvas)
}

pub fn decoded_frame(bytes: &[u8]) -> Result<Picture, GifError> {
  decode(bytes, Extent::Frame)
}

enum Extent {
  Canvas,
  Frame,
}

fn decode(bytes: &[u8], extent: Extent) -> Result<Picture, GifError> {
  let mut screen = blocks::screen(bytes)?;
  let mut frame = blocks::first_frame(bytes, &screen)?;

  match extent {
    Extent::Canvas => affordable(screen.size)?,
    Extent::Frame => {
      screen.size = frame.size;
      frame.place = console_core_geometry::Point { x: 0, y: 0 };
    }
  }

  affordable(frame.size)?;

  let indices = lzw::unpacked(&frame.packed, frame.least, frame.size)?;
  let rgba = painted(&screen, &frame, &indices)?;

  Ok(Picture {
    size: screen.size,
    bytes: rgba,
  })
}

fn affordable(size: Size<u32>) -> Result<(), GifError> {
  let area = u64::from(size.width).saturating_mul(u64::from(size.height));

  match area.saturating_mul(RGBA) {
    0..=MOST_BYTES => Ok(()),
    _too_large => Err(GifError::TooLarge),
  }
}

fn painted(screen: &Screen<'_>, frame: &Frame<'_>, indices: &[u8]) -> Result<Vec<u8>, GifError> {
  let Ok(area) = index(
    u64::from(screen.size.width)
      .saturating_mul(u64::from(screen.size.height))
      .saturating_mul(RGBA),
  );
  let Ok(line) = index(u64::from(screen.size.width).saturating_mul(RGBA));
  let Ok(wide) = index(frame.size.width);
  let Ok(order) = rows(frame);
  let palette = frame.table.as_chunks::<3>().0;
  let reached = match indices.iter().copied().max() {
    Some(most) => {
      let Ok(most_2) = index(most);

      palette.get(most_2)
    }
    None => palette.first(),
  };

  match reached {
    Some(_) => {}
    None => return Err(GifError::Corrupt),
  }

  let Ok(colors) = colors(palette, frame.clear);
  #[cfg_attr(
    dylint_lib = "explicit088_no_arriving_length",
    expect(
      explicit088_no_arriving_length,
      reason = "decoded checks the screen dimensions with affordable before painting; the RGBA buffer cannot exceed MOST_BYTES"
    )
  )]
  let mut rgba = vec![0u8; area];

  for (row, down) in indices.chunks(wide.max(1)).zip(order) {
    let Ok(down_2) = index(down.saturating_add(frame.place.y));

    match rgba.chunks_exact_mut(line.max(1)).nth(down_2) {
      Some(line_2) => {
        let Ok(()) = row_painted(line_2, (row, frame.place.x), &colors);
      }
      None => {}
    }
  }

  Ok(rgba)
}

enum PaletteEntry {
  Transparent,
  Opaque,
}

impl From<bool> for PaletteEntry {
  fn from(transparent: bool) -> Self {
    match bool::then_some(transparent, ()) {
      Some(()) => Self::Transparent,
      None => Self::Opaque,
    }
  }
}

fn colors(palette: &[[u8; 3]], clear: Option<u8>) -> Result<[[u8; 4]; 256], Never> {
  let mut colors = [CLEAR; 256];

  for ((slot, [red, green, blue]), at) in colors.iter_mut().zip(palette).zip(0u8..=u8::MAX) {
    *slot = match PaletteEntry::from(clear == Some(at)) {
      PaletteEntry::Transparent => CLEAR,
      PaletteEntry::Opaque => [*red, *green, *blue, u8::MAX],
    };
  }

  Ok(colors)
}

fn row_painted(line: &mut [u8], row: (&[u8], u32), colors: &[[u8; 4]; 256]) -> Result<(), Never> {
  let (row_2, left) = row;
  let Ok(left_2) = index(left);

  for (into, at) in line
    .as_chunks_mut::<4>()
    .0
    .iter_mut()
    .skip(left_2)
    .zip(row_2)
  {
    let Ok(at_2) = index(*at);

    *into = match colors.get(at_2) {
      Some(color) => *color,
      None => CLEAR,
    };
  }

  Ok(())
}

fn rows(frame: &Frame<'_>) -> Result<Vec<u32>, Never> {
  let tall = frame.size.height;

  Ok(match frame.lacing {
    Lacing::Straight => (0..tall).collect(),
    Lacing::Interlaced => (0..tall)
      .step_by(8)
      .chain((4..tall).step_by(8))
      .chain((2..tall).step_by(4))
      .chain((1..tall).step_by(2))
      .collect(),
  })
}
