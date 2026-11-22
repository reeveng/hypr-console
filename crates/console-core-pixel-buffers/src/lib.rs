//! Checked strided rows and owned, row-major color pixels. Each owned pixel contains four eight-bit channels
//! in red, green, blue, alpha order. With straight alpha, the color channels
//! store their original intensities independently of opacity.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
#[cfg(test)]
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

pub mod prediction;
mod rows;
pub use rows::{RowLayout, RowLayoutError, copy_clamped_window};

pub const BYTES_PER_PIXEL: u32 = 4;

/// The red, green, blue and alpha channels of one pixel, in byte order.
pub type Rgba8 = [u8; 4];

/// Borrows the complete pixels in a packed RGBA byte slice.
pub fn rgba_pixels(bytes: &[u8]) -> Result<&[Rgba8], Never> {
  Ok(bytes.as_chunks().0)
}

/// Mutably borrows the complete pixels in a packed RGBA byte slice.
pub fn rgba_pixels_mut(bytes: &mut [u8]) -> Result<&mut [Rgba8], Never> {
  Ok(bytes.as_chunks_mut().0)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedPixels {
  pub size: Size<u32>,
  pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Packing {
  Complete,
  Mismatched,
}

impl PackedPixels {
  pub fn packing(&self) -> Result<Packing, Never> {
    let Ok(available) = fitted::<_, u64>(self.bytes.len());
    let area = ::core::primitive::u64::wrapping_mul(
      u64::from(self.size.width),
      u64::from(self.size.height),
    );
    let required = area
      .checked_mul(u64::from(BYTES_PER_PIXEL))
      .filter(|required| *required == available);

    Ok(match (area, required) {
      (0, _) | (_, None) => Packing::Mismatched,
      (_, Some(_)) => Packing::Complete,
    })
  }

  pub fn pixels(&self) -> Result<&[Rgba8], Never> {
    rgba_pixels(&self.bytes)
  }

  pub fn pixel_at(&self, point: Point<u32>) -> Result<Option<Rgba8>, Never> {
    match (
      point.x.cmp(&self.size.width),
      point.y.cmp(&self.size.height),
    ) {
      (Ordering::Less, Ordering::Less) => {}
      (Ordering::Equal | Ordering::Greater, _)
      | (Ordering::Less, Ordering::Equal | Ordering::Greater) => return Ok(None),
    }

    let offset = ::core::primitive::u64::wrapping_add(
      ::core::primitive::u64::wrapping_mul(u64::from(point.y), u64::from(self.size.width)),
      u64::from(point.x),
    );
    let Ok(offset_2) = index(offset);
    let Ok(pixels) = self.pixels();

    Ok(pixels.get(offset_2).copied())
  }

  pub fn into_strided(self) -> Result<StridedPixels, Never> {
    StridedPixels::packed(self.size, self.bytes)
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StridedPixels {
  pub width: u32,
  pub height: u32,
  pub stride: u32,
  pub bytes: Vec<u8>,
}

impl StridedPixels {
  pub fn packed(size: Size<u32>, bytes: Vec<u8>) -> Result<Self, Never> {
    let stride = size.width.saturating_mul(BYTES_PER_PIXEL);

    Ok(Self {
      width: size.width,
      height: size.height,
      stride,
      bytes,
    })
  }

  pub fn size(&self) -> Result<Size<u32>, Never> {
    Ok(Size {
      width: self.width,
      height: self.height,
    })
  }

  pub fn required_bytes(size: Size<u32>, stride: u32) -> Result<Option<u64>, Never> {
    let width = u64::from(size.width).wrapping_mul(u64::from(BYTES_PER_PIXEL));
    let height = u64::from(size.height);
    rows::row_extent(Size { width, height }, u64::from(stride))
  }

  pub fn rows(&self) -> Result<Option<impl Iterator<Item = &[Rgba8]>>, Never> {
    let width = u64::from(self.width).wrapping_mul(u64::from(BYTES_PER_PIXEL));
    let size = Size {
      width,
      height: u64::from(self.height),
    };
    let rows =
      RowLayout::new(size, 0, i64::from(self.stride)).and_then(|layout| layout.rows(&self.bytes));

    Ok(match rows {
      Ok(visible) => Some(visible.map(|row| {
        let Ok(pixels) = rgba_pixels(row);
        pixels
      })),
      Err(_invalid_or_incomplete_layout) => None,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn borrowing_rows_skips_padding_even_when_the_final_row_has_none() {
    let mut buffer = StridedPixels {
      width: 1,
      height: 2,
      stride: 6,
      bytes: vec![1, 2, 3, 4, 99, 99, 5, 6, 7, 8],
    };
    let Ok(rows) = buffer.rows();
    let visible = rows.map(|row_views| row_views.collect::<Vec<_>>());

    assert_eq!(
      visible,
      Some(vec![[[1, 2, 3, 4]].as_slice(), [[5, 6, 7, 8]].as_slice()])
    );

    buffer.bytes.truncate(9);
    assert!(matches!(buffer.rows(), Ok(None)));

    buffer.stride = 3;
    assert!(matches!(buffer.rows(), Ok(None)));
  }

  #[test]
  fn a_packed_buffer_checks_pixel_coordinates_and_transfers_its_allocation() {
    let buffer = PackedPixels {
      size: Size {
        width: 1,
        height: 2,
      },
      bytes: vec![1, 2, 3, 4, 5, 6, 7, 8],
    };
    let address = buffer.bytes.as_ptr();

    assert_eq!(buffer.packing(), Ok(Packing::Complete));
    assert_eq!(
      buffer.pixel_at(Point { x: 0, y: 1 }),
      Ok(Some([5, 6, 7, 8]))
    );
    assert_eq!(buffer.pixel_at(Point { x: 1, y: 0 }), Ok(None));

    let Ok(strided) = buffer.into_strided();
    assert_eq!(strided.stride, 4);
    assert_eq!(strided.bytes.as_ptr(), address);
  }
}
