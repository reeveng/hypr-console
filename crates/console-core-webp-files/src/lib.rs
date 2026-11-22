//! A WebP's first picture, read into the pixels it holds.
//!
//! A WebP is a RIFF container around a picture coded one of two ways, lossy
//! as in VP8 or lossless as in VP8L, or around an animation whose frames are
//! each one of those. What a still is made of is the first frame, so that is
//! the one read, drawn where it sits on the canvas with nothing under it: the
//! rest of the canvas is clear, which is how libwebp's own animation decoder
//! starts.
//!
//! [`decoded`] reads either kind into RGBA at the size of the canvas, a lossy
//! picture's transparency with it. [`decoded_spread`] reads the same, with the
//! work of a lossy picture handed in rounds to a function of the caller's that
//! may do each round's tasks on as many threads as it has. [`measured`] says
//! how large the canvas is from the first thirty bytes alone.
//!
//! [`decoded_in_bands`] hands the rows of the part of a picture its caller
//! names to the caller's [`Receiver`], a row at a time as each is coloured,
//! so a lossy picture made into a thumbnail is never held whole. It is a trait
//! rather than a closure because what it writes is the receiver itself, and a
//! closure would hold the permission to write it out of sight of the call. Anything else
//! is decoded whole and handed over the same way, a row of the part at a time.
//!
//! Nothing here opens a file. It is handed bytes, and a stranger's bytes are
//! what it is for: the memory the canvas, the frame's pixels as they are
//! decoded and the codes take is counted before it is taken, a code that is
//! not complete, a copy from before the first pixel and a transform sent twice
//! are damage, and a bitstream that ends before its pixels do is refused
//! rather than drawn in part.

mod alpha;
#[cfg(test)]
mod boolean_encoder;
mod booleans;
mod chances;
mod colours;
mod container;
mod drawing;
#[cfg(test)]
mod forward;
mod inverse;
mod loop_filter;
mod lossless;
mod lossy;
#[cfg(test)]
mod mode_coding;
mod prediction;
mod prefix;
#[cfg(test)]
mod residual_coding;
mod rounds;
mod transforms;

use core::cmp::Ordering;

use console_core_geometry::{Point, Rectangle, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_prefix_codes::{CodeError, PastTheEnd, TooLong};

use crate::container::{Coding, Found, Image};

pub use crate::rounds::{Receiver, Spread, Task};

use crate::rounds::Destination;

const MOST_BYTES: u64 = 1 << 30;

const RGBA: u64 = 4;

const ARGB: u64 = 4;

enum CanvasCoverage {
  FullCanvas,
  PlacedFrame,
}

impl From<bool> for CanvasCoverage {
  fn from(full_canvas: bool) -> Self {
    match bool::then_some(full_canvas, ()) {
      Some(()) => Self::FullCanvas,
      None => Self::PlacedFrame,
    }
  }
}

impl Image<'_> {
  fn coverage(&self, canvas: Size<u32>) -> Result<CanvasCoverage, Never> {
    Ok(CanvasCoverage::from(
      self.place == (Point { x: 0, y: 0 }) && self.size == canvas,
    ))
  }

  fn check_size(&self, decoded: Size<u32>) -> Result<(), WebpError> {
    match (decoded.width, decoded.height).cmp(&(self.size.width, self.size.height)) {
      Ordering::Equal => Ok(()),
      Ordering::Less | Ordering::Greater => Err(WebpError::Corrupt),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebpError {
  NotAWebp,
  Truncated,
  Corrupt,
  TooLarge,
}

impl std::fmt::Display for WebpError {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      WebpError::NotAWebp => write!(to, "this is not a WebP"),
      WebpError::Truncated => write!(to, "the file ends before the picture in it does"),
      WebpError::Corrupt => write!(to, "the picture in here is damaged"),
      WebpError::TooLarge => write!(to, "this picture would take more memory than any picture"),
    }
  }
}

impl std::error::Error for WebpError {}

impl From<PastTheEnd> for WebpError {
  fn from(_cut: PastTheEnd) -> WebpError {
    WebpError::Truncated
  }
}

impl From<console_core_boolean_decoding::PastTheEnd> for WebpError {
  fn from(_cut: console_core_boolean_decoding::PastTheEnd) -> WebpError {
    WebpError::Truncated
  }
}

impl From<TooLong> for WebpError {
  fn from(_long: TooLong) -> WebpError {
    WebpError::TooLarge
  }
}

impl From<CodeError> for WebpError {
  fn from(_damaged: CodeError) -> WebpError {
    WebpError::Corrupt
  }
}

pub use console_core_pixel_buffers::PackedPixels as Picture;

pub fn measured(bytes: &[u8]) -> Result<Size<u32>, WebpError> {
  container::measured(bytes)
}

pub fn decoded(bytes: &[u8]) -> Result<Picture, WebpError> {
  decoded_spread(bytes, &rounds::one_after_another)
}

pub fn decoded_spread(bytes: &[u8], spread: Spread<'_>) -> Result<Picture, WebpError> {
  let found = container::found(bytes)?;

  affordable((found.canvas, found.image.size))?;

  let rgba = whole(&found, spread)?;

  Ok(Picture {
    size: found.canvas,
    bytes: rgba,
  })
}

pub fn decoded_in_bands(
  bytes: &[u8],
  spread: Spread<'_>,
  wanted: (Rectangle<u32>, &mut dyn Receiver),
) -> Result<Size<u32>, WebpError> {
  let (kept, receiver) = wanted;
  let found = container::found(bytes)?;

  affordable((found.canvas, found.image.size))?;

  let Ok(kept_2) = inside(kept, found.canvas);
  let Ok(coverage) = found.image.coverage(found.canvas);

  match (found.image.coding, coverage) {
    (Coding::Lossy, CanvasCoverage::FullCanvas) => {
      let (size, _) = rounds::decoded(
        found.image.bitstream,
        found.image.alpha,
        (spread, Destination::Bands(receiver, kept_2)),
      )?;

      found.image.check_size(size)?;

      Ok(found.canvas)
    }
    (Coding::Lossy, CanvasCoverage::PlacedFrame)
    | (Coding::Lossless, CanvasCoverage::FullCanvas | CanvasCoverage::PlacedFrame) => {
      let rgba = whole(&found, spread)?;
      let Ok(()) = cropped(&rgba, (found.canvas, kept_2), receiver);

      Ok(found.canvas)
    }
  }
}

fn inside(kept: Rectangle<u32>, canvas: Size<u32>) -> Result<Rectangle<u32>, Never> {
  let x = kept.origin.x.min(canvas.width);
  let y = kept.origin.y.min(canvas.height);
  let width = kept.size.width.min(canvas.width.saturating_sub(x));
  let height = kept.size.height.min(canvas.height.saturating_sub(y));

  Ok(Rectangle {
    origin: Point { x, y },
    size: Size { width, height },
  })
}

fn cropped(
  rgba: &[u8],
  cut: (Size<u32>, Rectangle<u32>),
  receiver: &mut dyn Receiver,
) -> Result<(), Never> {
  let (canvas, kept) = cut;
  let Ok(line) = index(u64::from(canvas.width).saturating_mul(RGBA));
  let Ok(left) = index(u64::from(kept.origin.x).saturating_mul(RGBA));
  let Ok(wide) = index(u64::from(kept.size.width).saturating_mul(RGBA));
  let Ok(top) = index(kept.origin.y);
  let Ok(tall) = index(kept.size.height);

  for row in rgba.chunks_exact(line.max(1)).skip(top).take(tall) {
    match row.get(left..).and_then(|row_2| row_2.get(..wide)) {
      Some(kept_2) => {
        let Ok(()) = receiver.received(kept_2);
      }
      None => {}
    }
  }

  Ok(())
}

fn whole(found: &Found<'_>, spread: Spread<'_>) -> Result<Vec<u8>, WebpError> {
  match found.image.coding {
    Coding::Lossless => {
      let (size, pixels) = lossless::decoded(found.image.bitstream)?;

      found.image.check_size(size)?;

      painted(found.canvas, &found.image, &pixels)
    }
    Coding::Lossy => {
      let (size, rgba) = rounds::decoded(
        found.image.bitstream,
        found.image.alpha,
        (spread, Destination::Picture),
      )?;

      found.image.check_size(size)?;

      placed(found.canvas, &found.image, rgba)
    }
  }
}

fn affordable(sizes: (Size<u32>, Size<u32>)) -> Result<(), WebpError> {
  let (canvas, frame) = sizes;
  let painted = u64::from(canvas.width)
    .saturating_mul(u64::from(canvas.height))
    .saturating_mul(RGBA);
  let decoded = u64::from(frame.width)
    .saturating_mul(u64::from(frame.height))
    .saturating_mul(ARGB);

  match painted.saturating_add(decoded).cmp(&MOST_BYTES) {
    Ordering::Less | Ordering::Equal => Ok(()),
    Ordering::Greater => Err(WebpError::TooLarge),
  }
}

#[cfg_attr(
  dylint_lib = "explicit088_no_arriving_length",
  expect(
    explicit088_no_arriving_length,
    reason = "the requested entry count multiplied by its element size is checked against MOST_BYTES before reserve"
  )
)]
fn reserve_to<T>(storage: &mut Vec<T>, count: u64) -> Result<(), WebpError> {
  let Ok(bytes_per_item) = fitted::<_, u64>(core::mem::size_of::<T>());

  match count.saturating_mul(bytes_per_item.max(1)).cmp(&MOST_BYTES) {
    Ordering::Greater => return Err(WebpError::TooLarge),
    Ordering::Less | Ordering::Equal => {}
  }

  let Ok(needed) = index(count);
  storage
    .try_reserve_exact(needed.saturating_sub(storage.len()))
    .map_err(|_could_not_reserve| WebpError::TooLarge)
}

fn reserved<T>(count: u64) -> Result<Vec<T>, WebpError> {
  let mut storage = Vec::new();
  reserve_to(&mut storage, count)?;

  Ok(storage)
}

fn resized<T: Clone>(storage: &mut Vec<T>, count: u64, element: T) -> Result<(), WebpError> {
  reserve_to(storage, count)?;

  let Ok(needed) = index(count);
  storage.truncate(needed);
  storage.extend(std::iter::repeat_n(
    element,
    needed.saturating_sub(storage.len()),
  ));

  Ok(())
}

fn filled<T: Clone>(count: u64, element: T) -> Result<Vec<T>, WebpError> {
  let mut storage = Vec::new();
  resized(&mut storage, count, element)?;

  Ok(storage)
}

fn painted(canvas: Size<u32>, image: &Image<'_>, pixels: &[u32]) -> Result<Vec<u8>, WebpError> {
  let area = u64::from(canvas.width)
    .saturating_mul(u64::from(canvas.height))
    .saturating_mul(RGBA);

  let mut rgba = filled(area, 0u8)?;
  let Ok(coverage) = image.coverage(canvas);

  match coverage {
    CanvasCoverage::FullCanvas => {
      for (into, argb) in rgba.as_chunks_mut::<4>().0.iter_mut().zip(pixels) {
        *into = argb.rotate_left(8).to_be_bytes();
      }

      return Ok(rgba);
    }
    CanvasCoverage::PlacedFrame => {}
  }

  let Ok(line) = index(u64::from(canvas.width).saturating_mul(RGBA));
  let Ok(width) = index(image.size.width);
  let Ok(left) = index(image.place.x);
  let Ok(top) = index(image.place.y);

  for (into, row) in rgba
    .chunks_exact_mut(line.max(1))
    .skip(top)
    .zip(pixels.chunks_exact(width.max(1)))
  {
    for (into_2, argb) in into.as_chunks_mut::<4>().0.iter_mut().skip(left).zip(row) {
      *into_2 = argb.rotate_left(8).to_be_bytes();
    }
  }

  Ok(rgba)
}

fn placed(canvas: Size<u32>, image: &Image<'_>, frame: Vec<u8>) -> Result<Vec<u8>, WebpError> {
  let Ok(coverage) = image.coverage(canvas);

  match coverage {
    CanvasCoverage::FullCanvas => return Ok(frame),
    CanvasCoverage::PlacedFrame => {}
  }

  let area = u64::from(canvas.width)
    .saturating_mul(u64::from(canvas.height))
    .saturating_mul(RGBA);
  let Ok(line) = index(u64::from(canvas.width).saturating_mul(RGBA));
  let Ok(frame_line) = index(u64::from(image.size.width).saturating_mul(RGBA));
  let Ok(left) = index(u64::from(image.place.x).saturating_mul(RGBA));
  let Ok(top) = index(image.place.y);

  let mut rgba = filled(area, 0u8)?;

  for (into, row) in rgba
    .chunks_exact_mut(line.max(1))
    .skip(top)
    .zip(frame.chunks_exact(frame_line.max(1)))
  {
    match into
      .get_mut(left..)
      .and_then(|into_2| into_2.get_mut(..row.len()))
    {
      Some(into_3) => {
        for (slot, value) in into_3.iter_mut().zip(row) {
          *slot = *value;
        }
      }
      None => {}
    }
  }

  Ok(rgba)
}

#[cfg(test)]
mod allocation_tests {
  use super::*;

  #[test]
  fn the_limit_counts_bytes_before_reserving_elements() {
    let count = MOST_BYTES.wrapping_div(4).saturating_add(1);

    assert_eq!(reserved::<u32>(count), Err(WebpError::TooLarge));
  }
}
