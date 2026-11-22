//! Iteration clips stored pages and walks gaps within the declared domain.

use super::{Continuity, Domain, Emptiness, InDomain, Presence, U32Set};
use alloc::collections::VecDeque;
use console_core_never::Never;
use core::cmp::Ordering;
use core::ops::{Bound, RangeBounds, RangeInclusive};

#[derive(Clone, Copy)]
pub(super) enum Selection {
  Members,
  Exceptions,
}

pub(super) enum Visibility {
  Included,
  Excluded,
}

pub(super) struct Bounds<T> {
  typed: RangeInclusive<T>,
  raw: RangeInclusive<u32>,
}

impl<T: Domain> Bounds<T> {
  pub(super) fn new(range: impl RangeBounds<T>) -> Result<Option<Self>, Never> {
    let Ok(mut domain) = T::ordered_values();
    let first = match domain.next() {
      Some(value) => value,
      None => return Ok(None),
    };
    let last = match domain.next_back() {
      Some(value) => value,
      None => first,
    };
    let Ok(domain_first) = T::from_u32(InDomain(first));
    let Ok(domain_last) = T::from_u32(InDomain(last));
    let start = match range.start_bound() {
      Bound::Included(value) | Bound::Excluded(value) => *value,
      Bound::Unbounded => domain_first,
    };
    let end = match range.end_bound() {
      Bound::Included(value) | Bound::Excluded(value) => *value,
      Bound::Unbounded => domain_last,
    };
    let Ok(start_raw) = start.to_u32();
    let Ok(end_raw) = end.to_u32();
    let first_2 = match range.start_bound() {
      Bound::Excluded(_) => start_raw.checked_add(1),
      Bound::Included(_) | Bound::Unbounded => Some(start_raw),
    };
    let last_2 = match range.end_bound() {
      Bound::Excluded(_) => end_raw.checked_sub(1),
      Bound::Included(_) | Bound::Unbounded => Some(end_raw),
    };

    Ok(match (first_2, last_2) {
      (Some(first_3), Some(last_3)) => match first_3.cmp(&last_3) {
        Ordering::Less | Ordering::Equal => Some(Self {
          typed: start..=end,
          raw: first_3..=last_3,
        }),
        Ordering::Greater => None,
      },
      (None, Some(_)) | (Some(_), None) | (None, None) => None,
    })
  }
}

pub(super) fn values<T: Domain>(
  set: &U32Set,
  selection: Selection,
  bounds: Option<Bounds<T>>,
) -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
  let Ok(continuity) = T::continuity();
  let raw = bounds.as_ref().map(|bounds_2| bounds_2.raw.clone());

  Ok(match (selection, continuity) {
    (Selection::Members, Continuity::Continuous | Continuity::Discontinuous) => {
      let Ok(stored) = set.range(raw);

      Values::Stored(stored)
    }
    (Selection::Exceptions, Continuity::Continuous) => {
      let Ok(stored) = set.iter_ranges();
      let Ok(gaps) = Complement::new(stored, raw);
      let mut ranges = VecDeque::new();

      for range in gaps {
        ranges.push_back(range);
      }

      Values::Gaps(GapValues { ranges })
    }
    (Selection::Exceptions, Continuity::Discontinuous) => {
      let domain = bounds
        .into_iter()
        .flat_map(|bounds_3| {
          let Ok(values) = T::ordered_values_range(bounds_3.typed);

          values.filter(move |value| bounds_3.raw.contains(value))
        })
        .filter(move |value| matches!(set.contains(*value), Ok(Presence::Absent)));

      Values::Domain(domain)
    }
  })
}

enum Values<S, D> {
  Stored(S),
  Gaps(GapValues),
  Domain(D),
}

impl<S: Iterator<Item = u32>, D: Iterator<Item = u32>> Iterator for Values<S, D> {
  type Item = u32;

  fn next(&mut self) -> Option<Self::Item> {
    match self {
      Self::Stored(values) => values.next(),
      Self::Gaps(values) => values.next(),
      Self::Domain(values) => values.next(),
    }
  }
}

impl<S: DoubleEndedIterator<Item = u32>, D: DoubleEndedIterator<Item = u32>> DoubleEndedIterator
  for Values<S, D>
{
  fn next_back(&mut self) -> Option<Self::Item> {
    match self {
      Self::Stored(values) => values.next_back(),
      Self::Gaps(values) => values.next_back(),
      Self::Domain(values) => values.next_back(),
    }
  }
}

struct GapValues {
  ranges: VecDeque<RangeInclusive<u32>>,
}

impl Iterator for GapValues {
  type Item = u32;

  fn next(&mut self) -> Option<Self::Item> {
    let range = self.ranges.front_mut()?;
    let value = range.next();

    match Emptiness::from(range.is_empty()) {
      Emptiness::Empty => {
        let _exhausted = self.ranges.pop_front();
      }
      Emptiness::NonEmpty => {}
    }

    value
  }
}

impl DoubleEndedIterator for GapValues {
  fn next_back(&mut self) -> Option<Self::Item> {
    let range = self.ranges.back_mut()?;
    let value = range.next_back();

    match Emptiness::from(range.is_empty()) {
      Emptiness::Empty => {
        let _exhausted = self.ranges.pop_back();
      }
      Emptiness::NonEmpty => {}
    }

    value
  }
}

pub(super) fn ranges<T: Domain>(
  set: &U32Set,
  selection: Selection,
) -> Result<impl Iterator<Item = RangeInclusive<u32>>, Never> {
  let Ok(continuity) = T::continuity();
  let Ok(stored) = set.iter_ranges();

  Ok(match (selection, continuity) {
    (Selection::Members, Continuity::Continuous) => Ranges::Stored(stored),
    (Selection::Exceptions, Continuity::Continuous) => {
      let Ok(bounds) = Bounds::<T>::new(..);

      let Ok(complement) = Complement::new(stored, bounds.map(|bounds_2| bounds_2.raw));

      Ranges::Complement(complement)
    }
    (selection_2, Continuity::Discontinuous) => {
      let Ok(values) = T::ordered_values();

      Ranges::Domain(DomainRanges {
        values,
        set,
        selection: selection_2,
      })
    }
  })
}

enum Ranges<'a, R, D> {
  Stored(R),
  Complement(Complement<R>),
  Domain(DomainRanges<'a, D>),
}

impl<R: Iterator<Item = RangeInclusive<u32>>, D: Iterator<Item = u32>> Iterator
  for Ranges<'_, R, D>
{
  type Item = RangeInclusive<u32>;

  fn next(&mut self) -> Option<Self::Item> {
    match self {
      Self::Stored(ranges) => ranges.next(),
      Self::Complement(ranges) => ranges.next(),
      Self::Domain(ranges) => ranges.next(),
    }
  }
}

struct Complement<I> {
  ranges: I,
  remaining: Option<RangeInclusive<u32>>,
}

impl<I> Complement<I> {
  fn new(ranges: I, remaining: Option<RangeInclusive<u32>>) -> Result<Self, Never> {
    Ok(Self { ranges, remaining })
  }
}

impl<I: Iterator<Item = RangeInclusive<u32>>> Iterator for Complement<I> {
  type Item = RangeInclusive<u32>;

  fn next(&mut self) -> Option<Self::Item> {
    let remaining = self.remaining.take()?;
    let mut first = *remaining.start();
    let last = *remaining.end();

    for excluded in self.ranges.by_ref() {
      match excluded.end().cmp(&first) {
        Ordering::Less => continue,
        Ordering::Equal | Ordering::Greater => {}
      }

      match excluded.start().cmp(&last) {
        Ordering::Greater => return Some(first..=last),
        Ordering::Less | Ordering::Equal => {}
      }

      let gap = match excluded.start().cmp(&first) {
        Ordering::Greater => Some(first..=excluded.start().saturating_sub(1)),
        Ordering::Less | Ordering::Equal => None,
      };
      let after = excluded.end().checked_add(1).filter(|value| *value <= last);

      match gap {
        Some(range) => {
          self.remaining = after.map(|value| value..=last);

          return Some(range);
        }
        None => {
          let value = after?;

          first = value;
        }
      }
    }

    Some(first..=last)
  }
}

struct DomainRanges<'a, I> {
  values: I,
  set: &'a U32Set,
  selection: Selection,
}

impl<I: Iterator<Item = u32>> Iterator for DomainRanges<'_, I> {
  type Item = RangeInclusive<u32>;

  fn next(&mut self) -> Option<Self::Item> {
    let mut range: Option<RangeInclusive<u32>> = None;

    for value in self.values.by_ref() {
      let Ok(stored) = self.set.contains(value);
      let Ok(selected) = match self.selection {
        Selection::Members => Ok(stored),
        Selection::Exceptions => stored.opposite(),
      };

      match (selected, range.as_mut()) {
        (Presence::Present, Some(current)) => *current = *current.start()..=value,
        (Presence::Present, None) => range = Some(value..=value),
        (Presence::Absent, Some(_)) => return range,
        (Presence::Absent, None) => {}
      }
    }

    range
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::IntSet;

  #[derive(Clone, Copy, Debug, PartialEq)]
  struct NoMembers;

  impl Domain for NoMembers {
    fn to_u32(&self) -> Result<u32, Never> {
      Ok(0)
    }

    fn contains(_value: u32) -> Result<Presence, Never> {
      Ok(Presence::Absent)
    }

    fn from_u32(_member: InDomain) -> Result<Self, Never> {
      Ok(Self)
    }

    fn continuity() -> Result<Continuity, Never> {
      Ok(Continuity::Continuous)
    }

    fn ordered_values() -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
      Ok(core::iter::empty())
    }

    fn ordered_values_range(
      _range: RangeInclusive<Self>,
    ) -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
      Self::ordered_values()
    }

    fn count() -> Result<u64, Never> {
      Ok(0)
    }
  }

  #[test]
  fn inversion_of_an_empty_domain_stays_empty() -> Result<(), Never> {
    let Ok(empty) = IntSet::<NoMembers>::empty();
    let Ok(all) = IntSet::<NoMembers>::all();
    let Ok(first) = all.first();
    let Ok(last) = all.last();
    let Ok(length) = all.len();
    let Ok(mut included) = all.iter_ranges();
    let Ok(mut excluded) = all.iter_excluded_ranges();

    assert_eq!(first, None);
    assert_eq!(last, None);
    assert_eq!(length, 0);
    assert_eq!(included.next(), None);
    assert_eq!(excluded.next(), None);
    assert_eq!(empty, all);

    Ok(())
  }
}
