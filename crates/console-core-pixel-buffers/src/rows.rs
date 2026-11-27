//! Checked views of rows in borrowed, strided storage.
//!
//! A negative stride puts logical row zero at the highest row start. The
//! stored bounds and step must fit the slice index type before a layout can
//! be constructed.
//!
//! The clamped copy asks its row provider for visible samples only. The copy
//! callback may convert samples, and repeated edge rows reuse the copied
//! samples. The destination layout is checked before any writes.

use console_core_api_compatibility::NativeSize;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{checked_index, fitted, fixed_index, index, indices};
use core::cmp::Ordering;
use core::ops::{Deref, Range};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowLayoutError {
  Empty,
  Stride,
  Bounds,
}

#[derive(Clone, Copy, Debug)]
enum RowOrder {
  Ascending,
  Descending,
}

#[derive(Clone, Debug)]
pub struct RowLayout {
  hull: Range<u64>,
  width: u64,
  height: u64,
  step: u64,
  order: RowOrder,
}

pub(super) fn row_extent(size: Size<u64>, step: u64) -> Result<Option<u64>, Never> {
  let extent = size
    .height
    .saturating_sub(1)
    .checked_mul(step)
    .and_then(|before_last| before_last.checked_add(size.width));

  Ok(extent)
}

impl RowLayout {
  #[inline]
  pub fn new(size: Size<u64>, first: u64, stride: i64) -> Result<Self, RowLayoutError> {
    let step = stride.unsigned_abs();

    match (size.width, size.height) {
      (0, _) | (_, 0) => return Err(RowLayoutError::Empty),
      (1.., 1..) => {}
    }

    match step.cmp(&size.width) {
      Ordering::Less => return Err(RowLayoutError::Stride),
      Ordering::Equal | Ordering::Greater => {}
    }

    let Ok(extent) = row_extent(size, step);
    let extent_bytes = extent.ok_or(RowLayoutError::Bounds)?;
    let order = match stride {
      i64::MIN..=-1 => RowOrder::Descending,
      0.. => RowOrder::Ascending,
    };
    let low = match order {
      RowOrder::Descending => first
        .checked_sub(extent_bytes.wrapping_sub(size.width))
        .ok_or(RowLayoutError::Bounds)?,
      RowOrder::Ascending => first,
    };
    let high = low
      .checked_add(extent_bytes)
      .ok_or(RowLayoutError::Bounds)?;
    checked_index(high).map_err(|_outside_slice_indices| RowLayoutError::Bounds)?;
    checked_index(step).map_err(|_outside_slice_indices| RowLayoutError::Stride)?;

    Ok(Self {
      hull: low..high,
      width: size.width,
      height: size.height,
      step,
      order,
    })
  }

  /// The contiguous span containing the first through last sample.
  #[inline]
  pub fn span(&self) -> Range<u64> {
    self.hull.clone()
  }

  #[inline]
  pub fn validate_len(&self, available: NativeSize) -> Result<(), RowLayoutError> {
    let Ok(available_samples) = fixed_index(available);

    match self.hull.end.cmp(&available_samples) {
      Ordering::Less | Ordering::Equal => Ok(()),
      Ordering::Greater => Err(RowLayoutError::Bounds),
    }
  }

  #[inline]
  pub fn row<'a, T>(&self, data: &'a [T], row: u64) -> Result<&'a [T], RowLayoutError> {
    self.validate_len(data.len())?;

    match row.cmp(&self.height) {
      Ordering::Less => {}
      Ordering::Equal | Ordering::Greater => return Err(RowLayoutError::Bounds),
    }

    let physical_row = match self.order {
      RowOrder::Descending => self.height.wrapping_sub(1).wrapping_sub(row),
      RowOrder::Ascending => row,
    };
    let start = self
      .hull
      .start
      .wrapping_add(physical_row.wrapping_mul(self.step));
    let Ok(range) = indices(start..start.wrapping_add(self.width));

    data.get(range).ok_or(RowLayoutError::Bounds)
  }

  #[inline]
  pub fn rows<'a, T>(
    &self,
    data: &'a [T],
  ) -> Result<impl Iterator<Item = &'a [T]> + use<'a, T>, RowLayoutError> {
    let Ok(width) = index(self.width);
    let Ok(step) = index(self.step);
    let Ok(range) = indices(self.hull.clone());
    let hull = data.get(range).ok_or(RowLayoutError::Bounds)?;
    let rows = InRowOrder {
      chunks: hull.chunks(step.max(1)),
      order: self.order,
    };

    Ok(rows.filter_map(move |row| row.get(..width)))
  }

  #[inline]
  pub fn rows_mut<'a, T>(
    &self,
    data: &'a mut [T],
  ) -> Result<impl Iterator<Item = &'a mut [T]> + use<'a, T>, RowLayoutError> {
    let Ok(width) = index(self.width);
    let Ok(step) = index(self.step);
    let Ok(range) = indices(self.hull.clone());
    let hull = data.get_mut(range).ok_or(RowLayoutError::Bounds)?;
    let rows = InRowOrder {
      chunks: hull.chunks_mut(step.max(1)),
      order: self.order,
    };

    Ok(rows.filter_map(move |row| row.get_mut(..width)))
  }
}

#[inline]
pub fn copy_clamped_window<Sample, Pixel: Copy, Row: Deref<Target = [Sample]>>(
  source: Size<NativeSize>,
  origin: Point<i64>,
  destination: (&RowLayout, &mut [Pixel]),
  mut read: impl FnMut(NativeSize, Range<NativeSize>) -> Option<Row>,
  mut copy: impl FnMut(&[Sample], &mut [Pixel]),
) -> Result<(), RowLayoutError> {
  match (source.width, source.height) {
    (0, _) | (_, 0) => return Err(RowLayoutError::Empty),
    (1.., 1..) => {}
  }

  let (layout, pixels) = destination;
  let Ok(source_width) = fixed_index(source.width);
  let Ok(source_height) = fixed_index(source.height);
  let bound = |value| i64::try_from(value).map_err(|_outside_coordinates| RowLayoutError::Bounds);
  let last_x = bound(source_width.wrapping_sub(1))?;
  let last_y = bound(source_height.wrapping_sub(1))?;
  let last_column = bound(layout.width.wrapping_sub(1))?;
  let last_row = bound(layout.height.wrapping_sub(1))?;
  let first = origin.x.clamp(0, last_x);
  let last = origin.x.saturating_add(last_column).clamp(0, last_x);
  let Ok(last_source_column) = fitted::<_, u64>(last);
  let Ok(first_source_column) = fitted::<_, u64>(first);
  let Ok(columns) = indices(first_source_column..last_source_column.wrapping_add(1));
  let Ok(left) = index(origin.x.saturating_neg().clamp(0, last_column));
  let mut previous: Option<(i64, &mut [Pixel])> = None;

  let rows_mut = layout.rows_mut(pixels)?;

  for (dy, out) in (0..=last_row).zip(rows_mut) {
    let y = origin.y.saturating_add(dy).clamp(0, last_y);

    match previous.as_ref().filter(|(previous_y, _)| y == *previous_y) {
      Some((_, previous_row)) => {
        match out.len().cmp(&previous_row.len()) {
          Ordering::Equal => {}
          Ordering::Less | Ordering::Greater => return Err(RowLayoutError::Bounds),
        }

        for (target, sample) in out.iter_mut().zip(previous_row.iter()) {
          *target = *sample;
        }
      }
      None => {
        let Ok(source_row) = index(y);
        let row = read(source_row, columns.clone()).ok_or(RowLayoutError::Bounds)?;

        match row.len().cmp(&columns.len()) {
          Ordering::Equal => {}
          Ordering::Less | Ordering::Greater => return Err(RowLayoutError::Bounds),
        }

        let (prefix, rest) = out
          .split_at_mut_checked(left)
          .ok_or(RowLayoutError::Bounds)?;
        let (middle, suffix) = rest
          .split_at_mut_checked(row.len())
          .ok_or(RowLayoutError::Bounds)?;
        copy(&row, middle);
        let first_2 =
          ::core::result::Result::<&_, _>::copied(middle.first().ok_or(RowLayoutError::Bounds))?;
        let last_2 =
          ::core::result::Result::<&_, _>::copied(middle.last().ok_or(RowLayoutError::Bounds))?;
        prefix.fill(first_2);
        suffix.fill(last_2);
      }
    }

    previous = Some((y, out));
  }

  Ok(())
}

struct InRowOrder<I> {
  chunks: I,
  order: RowOrder,
}

impl<I: DoubleEndedIterator> Iterator for InRowOrder<I> {
  type Item = I::Item;

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    match self.order {
      RowOrder::Descending => self.chunks.next_back(),
      RowOrder::Ascending => self.chunks.next(),
    }
  }

  #[inline]
  fn size_hint(&self) -> (NativeSize, Option<NativeSize>) {
    self.chunks.size_hint()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use core::cell::Cell;

  #[test]
  fn clamped_windows_match_per_pixel_clamping_in_bottom_up_storage() -> Result<(), RowLayoutError>
  {
    const UNWRITTEN: i32 = 99;
    let source = [[1u16, 2, 3], [4, 5, 6]];
    let layout = RowLayout::new(
      Size {
        width: 5,
        height: 4,
      },
      19,
      -6,
    )?;
    let origins = (i64::MIN..=i64::MIN)
      .chain(-5..=5)
      .chain(i64::MAX..=i64::MAX);

    for x in origins.clone() {
      for y in origins.clone() {
        let mut output = [UNWRITTEN; 24];
        copy_clamped_window(
          Size {
            width: 3,
            height: 2,
          },
          Point { x, y },
          (&layout, &mut output),
          |row_index, columns| {
            let row = source.get(row_index)?;

            row.get(columns)
          },
          |row, into| {
            for (&sample, target) in row.iter().zip(into) {
              *target = i32::from(sample);
            }
          },
        )?;
        let rows = layout.rows(&output)?;

        for (dy, row) in (0..4).zip(rows) {
          let Ok(sy) = index(y.saturating_add(dy).clamp(0, 1));
          let source_row = source.get(sy).ok_or(RowLayoutError::Bounds)?;
          let expected = (0..5).map(|dx| {
            let Ok(sx) = index(x.saturating_add(dx).clamp(0, 2));

            source_row.get(sx).copied().map(i32::from)
          });
          assert!(row.iter().copied().map(Some).eq(expected));
        }

        assert_eq!(
          output.iter().filter(|&&sample| sample == UNWRITTEN).count(),
          4
        );
      }
    }

    Ok(())
  }

  #[test]
  fn a_short_destination_is_rejected_before_visiting_source_rows()
  -> ::core::result::Result<(), crate::rows::RowLayoutError> {
    let layout = RowLayout::new(
      Size {
        width: 2,
        height: 2,
      },
      0,
      4,
    )?;
    let source = [1u8, 2];
    let mut output = [9; 5];
    let reads = Cell::new(0u32);
    assert_eq!(
      copy_clamped_window(
        Size {
          width: 2,
          height: 2,
        },
        Point { x: 0, y: 0 },
        (&layout, &mut output),
        |_, range| {
          reads.set(reads.get().wrapping_add(1));
          source.get(range)
        },
        |row, into| {
          for (&sample, target) in row.iter().zip(into) {
            *target = sample;
          }
        },
      ),
      Err(RowLayoutError::Bounds)
    );
    assert_eq!(reads.get(), 0);
    assert_eq!(output, [9; 5]);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn bottom_up_samples_follow_picture_order_and_preserve_padding()
  -> ::core::result::Result<(), crate::rows::RowLayoutError> {
    let layout = RowLayout::new(
      Size {
        width: 2,
        height: 2,
      },
      5,
      -4,
    )?;
    let mut samples = [91u16, 1, 2, 91, 91, 3, 4, 91];
    let [bottom, top] = [[1, 2], [3, 4]];
    let rows = layout.rows(&samples)?;

    assert!(rows.eq([top.as_slice(), bottom.as_slice()]));
    assert_eq!(layout.row(&samples, 0), Ok(top.as_slice()));
    assert_eq!(layout.row(&samples, 2), Err(RowLayoutError::Bounds));

    let rows_mut = layout.rows_mut(&mut samples)?;

    for (row, value) in rows_mut.zip([10, 20]) {
      row.fill(value);
    }

    assert_eq!(samples, [91, 20, 20, 91, 91, 10, 10, 91]);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn rejects_overlapping_or_unrepresentable_rows_and_short_storage()
  -> ::core::result::Result<(), crate::rows::RowLayoutError> {
    let size = Size {
      width: 2,
      height: 2,
    };
    assert!(matches!(
      RowLayout::new(size, 0, 1),
      Err(RowLayoutError::Stride)
    ));
    assert!(matches!(
      RowLayout::new(size, 3, -4),
      Err(RowLayoutError::Bounds)
    ));
    assert!(matches!(
      RowLayout::new(size, u64::MAX, 4),
      Err(RowLayoutError::Bounds)
    ));
    let layout = RowLayout::new(size, 0, 4)?;
    let mut truncated = [7u8; 5];
    assert_eq!(layout.row(&truncated, 0), Err(RowLayoutError::Bounds));
    assert!(matches!(
      layout.rows_mut(&mut truncated),
      Err(RowLayoutError::Bounds)
    ));
    assert_eq!(truncated, [7; 5]);
    ::core::result::Result::Ok(())
  }
}
