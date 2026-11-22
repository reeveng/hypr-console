//! Luma and two chroma planes drawn as red, green and blue.
//!
//! Chroma is kept at half the size each way, so each of its samples sits
//! between four pixels. libwebp's "fancy" upsampling gives each pixel the
//! nearest sample nine sixteenths of its say, the two beside and above or
//! below it three sixteenths each and the far one a sixteenth, with the first
//! and last row and column standing in for the ones past the edge; done the
//! same way here, a row of pixels is a row of chroma three parts nearest to
//! one part next, then three parts nearest to one part beside.
//!
//! Each pixel is then turned into red, green and blue with libwebp's own
//! fixed point weights for BT.601 at studio range, fourteen bits each, so a
//! lossy WebP comes out as libwebp draws it.
//!
//! Only the columns a caller wants are coloured, from chroma mixed along the
//! whole row, so the first and last of them have the neighbours they have in
//! the whole picture. Rows a caller takes as they come are coloured into a
//! single row, handed on before the next is coloured.

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_picture_scaling::{chroma_2x_rows, upsample_chroma_2x_bilinear};
use console_core_pixel_buffers::RowLayout;

use crate::WebpError;
use crate::rounds::Receiver;

const LUMA: i32 = 19077;

const RED_FROM_RED: i32 = 26149;

const GREEN_FROM_BLUE: i32 = 6419;

const GREEN_FROM_RED: i32 = 13320;

const BLUE_FROM_BLUE: i32 = 33050;

const RED_OFFSET: i32 = 14234;

const GREEN_OFFSET: i32 = 8708;

const BLUE_OFFSET: i32 = 17685;

const OPAQUE: u32 = 0xFF00_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Chroma<'a> {
  pub(crate) blue: &'a [u8],
  pub(crate) red: &'a [u8],
  pub(crate) stride: u32,
  pub(crate) origin: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Luma<'a> {
  pub(crate) pixels: &'a [u8],
  pub(crate) stride: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Columns {
  pub(crate) left: u32,
  pub(crate) width: u32,
}

struct Mixing {
  blue_row: Vec<[u8; 2]>,
  red_row: Vec<[u8; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Row<'a> {
  luma: &'a [u8],
  alpha: Option<&'a [u8]>,
}

pub(crate) enum Rows<'a> {
  Picture(&'a mut [u8]),
  Streamed(&'a mut [u8], &'a mut dyn Receiver),
}

pub(crate) fn coloured(
  into: &mut Rows<'_>,
  planes: (Luma<'_>, Chroma<'_>, &[u8]),
  rows: (Size<u32>, u32, Columns),
) -> Result<(), WebpError> {
  let (luma, chroma, alpha) = planes;
  let (size, first, columns) = rows;
  let Ok(line) = index(columns.width.saturating_mul(4));
  let Ok(wide) = index(size.width);
  let Ok(luma_stride) = index(luma.stride);
  let chroma_width = size.width.div_ceil(2);
  let chroma_wide = u64::from(chroma_width);
  let chroma_height = size.height.div_ceil(2);

  let blue_row = crate::filled(chroma_wide, [0; 2])?;
  let red_row = crate::filled(chroma_wide, [0; 2])?;
  let mut mixing = Mixing { blue_row, red_row };

  let luma_rows = luma.pixels.chunks_exact(luma_stride.max(1));
  let alpha_rows = alpha.chunks_exact(wide.max(1));
  let no_alpha = std::iter::repeat_n(None, luma_rows.len().saturating_sub(alpha_rows.len()));
  let alphas = alpha_rows.map(Some).chain(no_alpha);
  let sources = (first..).zip(luma_rows.zip(alphas).map(|(luma_2, alpha_2)| Row {
    luma: luma_2,
    alpha: alpha_2,
  }));

  let each = |mixing_2: &mut Mixing, row: u32, out: (&mut [u8], Row<'_>)| -> Result<(), Never> {
    let (out_2, from) = out;
    let (near, far) = chroma_2x_rows(row, chroma_height);

    let Ok(()) = mixed(
      mixing_2,
      &chroma,
      (
        near.saturating_sub(chroma.origin),
        far.saturating_sub(chroma.origin),
        chroma_width,
      ),
    );

    row_coloured(out_2, from, mixing_2, columns)
  };

  match into {
    Rows::Picture(rgba) => {
      for (out, (row, from)) in rgba.chunks_exact_mut(line.max(1)).zip(sources) {
        let Ok(()) = each(&mut mixing, row, (out, from));
      }
    }
    Rows::Streamed(out, receiver) => {
      for (row, from) in sources {
        let Ok(()) = each(&mut mixing, row, (&mut **out, from));
        let Ok(()) = receiver.received(out);
      }
    }
  }

  Ok(())
}

fn row_coloured(
  out: &mut [u8],
  from: Row<'_>,
  mixing: &Mixing,
  columns: Columns,
) -> Result<(), Never> {
  let Row {
    luma: luma_row,
    alpha: alphas,
  } = from;
  let Ok(left) = index(columns.left);
  let Ok(kept) = index(columns.width);

  let (luma_row_3, blue_row, red_row) = match (
    luma_row.get(left..),
    mixing.blue_row.as_flattened().get(left..),
    mixing.red_row.as_flattened().get(left..),
  ) {
    (Some(luma_row_2), Some(blue_row), Some(red_row)) => (luma_row_2, blue_row, red_row),
    (_, _, _) => return Ok(()),
  };

  let pixels = out
    .as_chunks_mut::<4>()
    .0
    .iter_mut()
    .zip(luma_row_3.iter().take(kept))
    .zip(blue_row.iter().zip(red_row));

  for ((pixel, luma), (blue, red)) in pixels {
    let Ok(colour) = rgb((*luma, *blue, *red));

    *pixel = colour.to_le_bytes();
  }

  match alphas.and_then(|alphas_2| alphas_2.get(left..)) {
    Some(alphas_3) => {
      for (pixel, alpha) in out
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(alphas_3.iter().take(kept))
      {
        let [red, green, blue, _] = *pixel;

        *pixel = [red, green, blue, *alpha];
      }
    }
    None => {}
  }

  Ok(())
}

fn mixed(mixing: &mut Mixing, chroma: &Chroma<'_>, rows: (u32, u32, u32)) -> Result<(), Never> {
  let (near, far, wide) = rows;
  let size = Size {
    width: u64::from(wide),
    height: u64::from(near.max(far)).wrapping_add(1),
  };
  let layout = RowLayout::new(size, 0, i64::from(chroma.stride));
  let [[near_blue, far_blue], [near_red, far_red]] = [chroma.blue, chroma.red].map(|plane| {
    [near, far].map(|at| match &layout {
      Ok(geometry) => match geometry.row(plane, u64::from(at)) {
        Ok(present) => present,
        Err(_row_outside_plane) => &[],
      },
      Err(_invalid_chroma_layout) => &[],
    })
  });

  for (output, [near_2, far_2]) in [
    (&mut mixing.blue_row, [near_blue, far_blue]),
    (&mut mixing.red_row, [near_red, far_red]),
  ] {
    let _upsampled = upsample_chroma_2x_bilinear(near_2, far_2, output.as_flattened_mut());
  }

  Ok(())
}

#[inline(always)]
fn high(value: u8, weight: i32) -> Result<i32, Never> {
  Ok(i32::from(value).wrapping_mul(weight).wrapping_shr(8))
}

#[inline(always)]
fn channel(value: i32) -> Result<u32, Never> {
  Ok(u32::from_le_bytes(
    value.wrapping_shr(6).clamp(0, 255).to_le_bytes(),
  ))
}

#[inline(always)]
fn rgb(sample: (u8, u8, u8)) -> Result<u32, Never> {
  let (luma, blue, red) = sample;
  let Ok(bright) = high(luma, LUMA);
  let Ok(red_red) = high(red, RED_FROM_RED);
  let Ok(green_blue) = high(blue, GREEN_FROM_BLUE);
  let Ok(green_red) = high(red, GREEN_FROM_RED);
  let Ok(blue_blue) = high(blue, BLUE_FROM_BLUE);
  let Ok(r) = channel(bright.wrapping_add(red_red).wrapping_sub(RED_OFFSET));
  let Ok(g) = channel(
    bright
      .wrapping_sub(green_blue)
      .wrapping_sub(green_red)
      .wrapping_add(GREEN_OFFSET),
  );
  let Ok(b) = channel(bright.wrapping_add(blue_blue).wrapping_sub(BLUE_OFFSET));

  Ok(r | g.wrapping_shl(8) | b.wrapping_shl(16) | OPAQUE)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn grey_at_the_middle_of_studio_range_is_grey() {
    assert_eq!(
      rgb((126, 128, 128)).map(u32::to_le_bytes),
      Ok([128, 128, 128, 255])
    );
  }
}
