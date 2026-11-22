//! Range endpoints cross to slice indices together, retaining empty inclusive spans.

use crate::{Ends, index};
use console_core_never::Never;
use core::cmp::Ordering;
use core::ops::{Range, RangeFrom, RangeInclusive, RangeTo, RangeToInclusive};

enum InclusiveSource {
  Empty,
  Nonempty,
}

impl From<bool> for InclusiveSource {
  fn from(empty: bool) -> Self {
    match bool::then_some(empty, ()) {
      Some(()) => Self::Empty,
      None => Self::Nonempty,
    }
  }
}

pub fn indices<F: Ends + PartialOrd + Copy>(range: Range<F>) -> Result<Range<usize>, Never>
where
  usize: TryFrom<F>,
{
  let Ok(start) = index(range.start);
  let Ok(end) = index(range.end);

  Ok(start..end)
}

pub fn indices_from<F: Ends + PartialOrd + Copy>(
  range: RangeFrom<F>,
) -> Result<RangeFrom<usize>, Never>
where
  usize: TryFrom<F>,
{
  let Ok(start) = index(range.start);

  Ok(start..)
}

pub fn indices_to<F: Ends + PartialOrd + Copy>(range: RangeTo<F>) -> Result<RangeTo<usize>, Never>
where
  usize: TryFrom<F>,
{
  let Ok(end) = index(range.end);

  Ok(..end)
}

pub fn indices_inclusive<F: Ends + PartialOrd + Copy>(
  range: RangeInclusive<F>,
) -> Result<RangeInclusive<usize>, Never>
where
  usize: TryFrom<F>,
{
  let source = InclusiveSource::from(range.is_empty());
  let (start, end) = range.into_inner();
  let Ok(start_2) = index(start);
  let Ok(end_2) = index(end);
  let mut converted = start_2..=end_2;

  match (source, start_2.cmp(&end_2)) {
    (InclusiveSource::Empty, Ordering::Equal) => {
      let _past_end = converted.next();
    }
    (InclusiveSource::Empty, Ordering::Less | Ordering::Greater)
    | (InclusiveSource::Nonempty, Ordering::Less | Ordering::Equal | Ordering::Greater) => {}
  }

  Ok(converted)
}

pub fn indices_to_inclusive<F: Ends + PartialOrd + Copy>(
  range: RangeToInclusive<F>,
) -> Result<RangeToInclusive<usize>, Never>
where
  usize: TryFrom<F>,
{
  let Ok(end) = index(range.end);

  Ok(..=end)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn converted_ranges_select_the_same_bytes() -> Result<(), Never> {
    let bytes = [10, 20, 30, 40];
    let Ok(middle) = indices(1u64..3);
    let Ok(tail) = indices_from(2u64..);
    let Ok(prefix) = indices_to(..2u64);
    let Ok(inclusive) = indices_inclusive(1u64..=2);
    let Ok(inclusive_prefix) = indices_to_inclusive(..=1u64);
    let Ok(overflowing) = indices_to_inclusive(..=u64::MAX);

    assert_eq!(bytes.get(middle), Some([20, 30].as_slice()));
    assert_eq!(bytes.get(tail), Some([30, 40].as_slice()));
    assert_eq!(bytes.get(prefix), Some([10, 20].as_slice()));
    assert_eq!(bytes.get(inclusive), Some([20, 30].as_slice()));
    assert_eq!(bytes.get(inclusive_prefix), Some([10, 20].as_slice()));
    assert_eq!(bytes.get(overflowing), None);

    Ok(())
  }

  #[test]
  fn an_exhausted_inclusive_range_remains_empty() -> Result<(), Never> {
    let mut exhausted = 2u64..=2;
    let _last = exhausted.next();
    let Ok(converted) = indices_inclusive(exhausted);

    let bytes = [10, 20, 30];
    let mut native = 2usize..=2;
    let _last = native.next();

    assert_eq!(
      bytes.get(converted).map(<[i32]>::as_ptr),
      bytes.get(native).map(<[i32]>::as_ptr)
    );

    let mut outside = 12u64..=12;
    let _last = outside.next();
    let Ok(converted_outside) = indices_inclusive(outside);

    assert_eq!(bytes.get(converted_outside), None);

    Ok(())
  }
}
