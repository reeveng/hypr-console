//! Motion-compensation blends check their rectangle before writing. Each row
//! then lends complete SIMD blocks, followed by its remaining scalar pixels.

use core::num::NonZeroU32;

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, indices_to};
use console_core_pixel_buffers::RowLayout;
use zerocopy::{FromBytes, IntoBytes, KnownLayout};

#[derive(Debug, PartialEq, Eq)]
pub(in crate::src::safe_simd) enum BlendLayoutError {
  Dimensions,
  Source,
  Destination,
}

#[derive(Clone, Copy)]
pub(in crate::src::safe_simd) struct BlendSize(Size<NonZeroU32>);

impl BlendSize {
  pub(in crate::src::safe_simd) fn new(size: Size<i32>) -> Result<Self, BlendLayoutError> {
    let columns = u32::try_from(size.width).map_err(|_negative| BlendLayoutError::Dimensions)?;
    let rows = u32::try_from(size.height).map_err(|_negative| BlendLayoutError::Dimensions)?;
    let width = NonZeroU32::new(columns).ok_or(BlendLayoutError::Dimensions)?;
    let height = NonZeroU32::new(rows).ok_or(BlendLayoutError::Dimensions)?;

    Ok(Self(Size { width, height }))
  }

  pub(in crate::src::safe_simd) fn input_rows<T>(
    self,
    source: &[T],
  ) -> Result<impl Iterator<Item = &[T]>, BlendLayoutError> {
    let width = u64::from(self.0.width.get());
    let layout = RowLayout::new(
      Size {
        width,
        height: u64::from(self.0.height.get()),
      },
      0,
      i64::from(self.0.width.get()),
    )
    .map_err(|_layout| BlendLayoutError::Source)?;

    layout
      .rows(source)
      .map_err(|_layout| BlendLayoutError::Source)
  }

  pub(in crate::src::safe_simd) fn destination_rows<
    'a,
    Pixel: FromBytes + IntoBytes + KnownLayout + 'a,
  >(
    self,
    bytes: &'a mut [u8],
    byte_stride: u64,
  ) -> Result<impl Iterator<Item = &'a mut [Pixel]>, BlendLayoutError> {
    let Ok(pixel_bytes) = fixed_index(size_of::<Pixel>());

    match (byte_stride.checked_rem(pixel_bytes) == Some(0)).then_some(()) {
      Some(()) => {}
      None => return Err(BlendLayoutError::Destination),
    }

    let Ok(available) = fixed_index(bytes.len());
    let samples = available
      .checked_div(pixel_bytes)
      .ok_or(BlendLayoutError::Destination)?;
    let Ok(range) = indices_to(..samples.saturating_mul(pixel_bytes));
    let visible = bytes.get_mut(range).ok_or(BlendLayoutError::Destination)?;
    let pixels: &mut [Pixel] =
      FromBytes::mut_from_bytes(visible).map_err(|_alignment| BlendLayoutError::Destination)?;
    let stride_samples = byte_stride
      .checked_div(pixel_bytes)
      .ok_or(BlendLayoutError::Destination)?;
    let stride =
      i64::try_from(stride_samples).map_err(|_too_large| BlendLayoutError::Destination)?;
    let size = Size {
      width: u64::from(self.0.width.get()),
      height: u64::from(self.0.height.get()),
    };
    let layout =
      RowLayout::new(size, 0, stride).map_err(|_layout| BlendLayoutError::Destination)?;

    layout
      .rows_mut(pixels)
      .map_err(|_layout| BlendLayoutError::Destination)
  }

  #[inline]
  pub(in crate::src::safe_simd) fn blend_rows<
    'a,
    Pixel: FromBytes + IntoBytes + KnownLayout + 'a,
  >(
    self,
    [primary, secondary]: [&'a [i16]; 2],
    destination: &'a mut [u8],
    stride: u64,
  ) -> Result<impl Iterator<Item = BlendRow<'a, Pixel>>, BlendLayoutError> {
    let first_rows = self.input_rows(primary)?;
    let second_rows = self.input_rows(secondary)?;
    let destination_rows = self.destination_rows::<Pixel>(destination, stride)?;
    Ok(
      destination_rows
        .zip(first_rows)
        .zip(second_rows)
        .map(|((dst, first), second)| BlendRow { dst, first, second }),
    )
  }
}

pub(in crate::src::safe_simd) struct BlendRow<'a, Pixel> {
  pub(in crate::src::safe_simd) dst: &'a mut [Pixel],
  pub(in crate::src::safe_simd) first: &'a [i16],
  pub(in crate::src::safe_simd) second: &'a [i16],
}

impl<'a, Pixel> BlendRow<'a, Pixel> {
  #[inline]
  pub(in crate::src::safe_simd) fn blocks<const LANES: usize>(
    &mut self,
  ) -> Result<
    impl Iterator<Item = (&'a mut [Pixel; LANES], &'a [i16; LANES], &'a [i16; LANES])>,
    Never,
  > {
    let (dst_blocks, dst_tail) = core::mem::take(&mut self.dst).as_chunks_mut::<LANES>();
    let (first_blocks, first_tail) = self.first.as_chunks::<LANES>();
    let (second_blocks, second_tail) = self.second.as_chunks::<LANES>();
    self.dst = dst_tail;
    self.first = first_tail;
    self.second = second_tail;

    Ok(
      dst_blocks
        .iter_mut()
        .zip(first_blocks)
        .zip(second_blocks)
        .map(|((dst, first), second)| (dst, first, second)),
    )
  }

  #[inline]
  pub(in crate::src::safe_simd) fn pixels(
    self,
  ) -> Result<impl Iterator<Item = (&'a mut Pixel, &'a i16, &'a i16)>, Never> {
    Ok(
      self
        .dst
        .iter_mut()
        .zip(self.first)
        .zip(self.second)
        .map(|((dst, first), second)| (dst, first, second)),
    )
  }
}
