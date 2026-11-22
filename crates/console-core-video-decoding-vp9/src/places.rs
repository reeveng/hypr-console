//! A value asked for by its place in a list: a sample, a coefficient, a probability, an entry of
//! one of the specification's tables.
//!
//! The specification never reads past the end of a table or a frame for a stream it describes,
//! and the decoder this came from indexed, so a stream that lied about its own sizes stopped the
//! program. Here a place past the end reads as nought and a write to one goes nowhere: a damaged
//! stream gives a damaged picture rather than taking the player down with it, and a stream the
//! specification describes never meets either, so nothing it decodes changes.
//!
//! A place is held at sixty-four bits, which is the width a position in memory has on every
//! machine the decoder runs on, and it meets the standard library's width here and nowhere else,
//! as it is handed to a `get`.

use std::ops::Range;

use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, indices};

use crate::frame::Mv;

pub(crate) trait Nought: Copy {
  const NOUGHT: Self;
}

impl Nought for Mv {
  const NOUGHT: Self = Self::ZERO;
}

impl Nought for u8 {
  const NOUGHT: Self = 0;
}

impl Nought for i8 {
  const NOUGHT: Self = 0;
}

impl Nought for u16 {
  const NOUGHT: Self = 0;
}

impl Nought for i16 {
  const NOUGHT: Self = 0;
}

impl Nought for u32 {
  const NOUGHT: Self = 0;
}

impl Nought for u64 {
  const NOUGHT: Self = 0;
}

impl Nought for i32 {
  const NOUGHT: Self = 0;
}

impl Nought for i64 {
  const NOUGHT: Self = 0;
}

pub(crate) fn read<T: Nought>(values: &[T], place: u64) -> Result<T, Never> {
  let Ok(at) = index(place);

  Ok(match values.get(at) {
    Some(value) => *value,
    None => T::NOUGHT,
  })
}

pub(crate) fn write<T>(values: &mut [T], place: u64, value: T) -> Result<(), Never> {
  let Ok(at) = index(place);

  match values.get_mut(at) {
    Some(slot) => *slot = value,
    None => {}
  }

  Ok(())
}

pub(crate) fn counted(counts: &mut [u32], place: u64) -> Result<(), Never> {
  let Ok(at) = index(place);

  match counts.get_mut(at) {
    Some(count) => *count = count.wrapping_add(1),
    None => {}
  }

  Ok(())
}

pub(crate) fn span<T>(values: &[T], range: Range<u64>) -> Result<&[T], Never> {
  let Ok(at) = indices(range);

  Ok(match values.get(at) {
    Some(found) => found,
    None => &[],
  })
}

pub(crate) fn span_mut<T>(values: &mut [T], range: Range<u64>) -> Result<&mut [T], Never> {
  let Ok(at) = indices(range);

  Ok(match values.get_mut(at) {
    Some(found) => found,
    None => &mut [],
  })
}

pub(crate) fn after<T>(values: &[T], start: u64) -> Result<&[T], Never> {
  let Ok(at) = index(start);

  Ok(match values.get(at..) {
    Some(found) => found,
    None => &[],
  })
}

pub(crate) fn after_mut<T>(values: &mut [T], start: u64) -> Result<&mut [T], Never> {
  let Ok(at) = index(start);

  Ok(match values.get_mut(at..) {
    Some(found) => found,
    None => &mut [],
  })
}

pub(crate) fn row<T, const N: usize>(rows: &[[T; N]], place: u64) -> Result<&[T], Never> {
  let Ok(at) = index(place);

  Ok(match rows.get(at) {
    Some(found) => found,
    None => &[],
  })
}

pub(crate) fn row_mut<T, const N: usize>(
  rows: &mut [[T; N]],
  place: u64,
) -> Result<&mut [T], Never> {
  let Ok(at) = index(place);

  Ok(match rows.get_mut(at) {
    Some(found) => found,
    None => &mut [],
  })
}

pub(crate) fn length<T>(values: &[T]) -> Result<u64, Never> {
  fixed_index(values.len())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_place_past_the_end_reads_as_nought_and_takes_no_write() {
    let mut values = [3u8, 5, 7];
    let Ok(inside) = read(&values, 2);
    let Ok(outside) = read(&values, 3);

    assert_eq!((inside, outside), (7, 0));

    let Ok(()) = write(&mut values, 9, 1);
    let Ok(()) = write(&mut values, 0, 1);

    assert_eq!(values, [1, 5, 7]);
  }

  #[test]
  fn a_span_past_the_end_is_empty() {
    let values = [1u16, 2, 3, 4];
    let Ok(inside) = span(&values, 1..3);
    let Ok(outside) = span(&values, 3..9);
    let Ok(rest) = after(&values, 2);
    let Ok(nothing) = after(&values, 5);

    assert_eq!(inside, [2, 3]);
    assert!(outside.is_empty());
    assert_eq!(rest, [3, 4]);
    assert!(nothing.is_empty());
  }
}
