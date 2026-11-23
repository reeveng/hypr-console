//! The walkthrough's local text renderer. It paints into the lent frame and
//! clips to the visible viewport. The address arrives through a bounded channel.

use alloc::vec::Vec;

use console_core_geometry::Size;
use console_core_urls::DataUrl;
use console_core_web_engine::FrameLayout;
use console_kernel_handles::{Bytes, HandleError};

use crate::screen::{DOTS, glyph};

const SCALE: u64 = 2;
const CELL: u64 = 8_u64.saturating_mul(SCALE);
const MARGIN: u64 = CELL;
const PAPER: [u8; 4] = [255; 4];
const INK: [u8; 4] = [0, 0, 0, 255];

pub(super) fn frame_bytes(size: Size<u32>) -> Result<Bytes, HandleError> {
  match ::core::primitive::bool::then_some(size.width == 0 || size.height == 0, ()) {
    ::core::option::Option::Some(()) => return Err(HandleError::BadState),
    ::core::option::Option::None => {}
  }

  FrameLayout::rgba(size)
    .map(|frame| Bytes(frame.bytes))
    .map_err(|_invalid_layout| HandleError::NoResources)
}

pub(super) fn decode(address: &str) -> Result<Vec<u8>, HandleError> {
  let url = DataUrl::process(address).map_err(|_| HandleError::BadState)?;
  let Ok(mime) = url.mime_type();
  let Ok((media_type, subtype)) = mime.essence();
  let Ok(charset) = mime.get_parameter("charset");

  match ::core::primitive::bool::then_some(media_type != "text" || subtype != "plain", ()) {
    ::core::option::Option::Some(()) => return Err(HandleError::BadState),
    ::core::option::Option::None => {}
  }

  let (body, _fragment) = url.decode_to_vec().map_err(|_| HandleError::BadState)?;

  match charset {
    None => Ok(body),
    Some(name) => match ::core::primitive::bool::then_some(
      name.eq_ignore_ascii_case("utf-8")
        || (name.eq_ignore_ascii_case("us-ascii") && body.is_ascii()),
      (),
    ) {
      ::core::option::Option::Some(()) => Ok(body),
      ::core::option::Option::None => Err(HandleError::BadState),
    },
  }
}

pub(super) fn paint(
  size: Size<u32>,
  text: &str,
  pixels: &mut [[u8; 4]],
) -> Result<(), HandleError> {
  let width = u64::from(size.width);
  let height = u64::from(size.height);
  let length = u64::try_from(pixels.len()).map_err(|_wide| HandleError::NoResources)?;

  match ::core::primitive::bool::then_some(width.checked_mul(height) == Some(length), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(HandleError::BadState),
  }

  pixels.fill(PAPER);
  let columns = width
    .saturating_sub(MARGIN.saturating_mul(2))
    .div_euclid(CELL);
  let rows = height
    .saturating_sub(MARGIN.saturating_mul(2))
    .div_euclid(CELL);

  match (columns, rows) {
    (0, _) | (_, 0) => return Ok(()),
    (_, _) => {}
  }

  let mut column = 0_u64;
  let mut row = 0_u64;

  for character in text.chars() {
    match ::core::primitive::bool::then_some(row >= rows, ()) {
      ::core::option::Option::Some(()) => break,
      ::core::option::Option::None => {}
    }

    match character {
      '\n' => {
        column = 0;
        row = row.saturating_add(1);
        continue;
      }
      '\r' => continue,
      _ => {}
    }

    match ::core::primitive::bool::then_some(column == columns, ()) {
      ::core::option::Option::Some(()) => {
        column = 0;
        row = row.saturating_add(1);
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(row >= rows, ()) {
      ::core::option::Option::Some(()) => break,
      ::core::option::Option::None => {}
    }

    let left = MARGIN.saturating_add(column.saturating_mul(CELL));
    let top = MARGIN.saturating_add(row.saturating_mul(CELL));
    let visible = match character {
      '\t' => ' ',
      other => match ::core::primitive::bool::then_some(other.is_control(), ()) {
        ::core::option::Option::Some(()) => '\u{fffd}',
        ::core::option::Option::None => other,
      },
    };
    let Ok(scanlines) = glyph(visible);
    paint_glyph(pixels, width, Cell { left, top }, scanlines)?;
    column = column.saturating_add(1);
  }

  Ok(())
}

struct Cell {
  left: u64,
  top: u64,
}

fn paint_glyph(
  pixels: &mut [[u8; 4]],
  width: u64,
  cell: Cell,
  scanlines: [u8; 8],
) -> Result<(), HandleError> {
  for (y, scanline) in (0_u64..).zip(scanlines) {
    for (x, _) in (0_u64..).zip(DOTS).filter(|(_, dot)| scanline & dot != 0) {
      let left = cell.left.saturating_add(x.saturating_mul(SCALE));
      let top = cell.top.saturating_add(y.saturating_mul(SCALE));

      for down in 0..SCALE {
        let start = top
          .saturating_add(down)
          .saturating_mul(width)
          .saturating_add(left);
        ink(pixels, start)?;
      }
    }
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the frame's fixed-width pixel offset is converted at the slice lookup"
  )
)]
fn ink(pixels: &mut [[u8; 4]], start: u64) -> Result<(), HandleError> {
  let first = usize::try_from(start).map_err(|_wide| HandleError::NoResources)?;
  let end = first.saturating_add(2);
  let run = pixels.get_mut(first..end).ok_or(HandleError::BadState)?;
  run.fill(INK);

  Ok(())
}
