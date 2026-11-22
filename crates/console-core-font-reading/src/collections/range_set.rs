//! Stores a disjoint collection of ranges over numeric types.
//!
//! Overlapping and adjacent ranges are automatically merged together.

use alloc::collections::BTreeMap;
use core::{
  cmp::{max, min},
  fmt::{Debug, Formatter},
  iter::Peekable,
  ops::RangeInclusive,
};

use types::Fixed;

#[derive(Default, Clone, PartialEq, Eq)]
pub struct RangeSet<T> {
  ranges: BTreeMap<T, T>,
}

pub trait OrdAdjacency {
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(explicit007_no_bool_return, reason = "the published `OrdAdjacency::are_adjacent` trait method retains the Boolean result used by existing predicate callers and implementations")
  )]
  fn are_adjacent(self, rhs: Self) -> bool;
}

impl<T> RangeSet<T>
where
  T: Ord + Copy + OrdAdjacency,
{
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `RangeSet::is_empty` method returns `bool` directly in the existing parser interface")
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(explicit007_no_bool_return, reason = "the published `RangeSet::is_empty` method retains the Boolean result used by existing predicate callers and implementations")
  )]
  pub fn is_empty(&self) -> bool {
    self.ranges.is_empty()
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `RangeSet::insert` method is called through its existing unit-returning signature")
  )]
  pub fn insert(&mut self, range: RangeInclusive<T>) {
    match ::core::primitive::bool::then_some(range.end() < range.start(), ()) {
      ::core::option::Option::Some(()) => {
        return;
      }
      ::core::option::Option::None => {}
    }

    let mut start = *range.start();
    let mut end = *range.end();

    let Ok(prev_range) = self.prev_range(start);

    match prev_range {
      Some((prev_start, prev_end)) => {
        match range_is_subset(start, end, prev_start, prev_end) {
          Ok(Containment::Subset) => {
            return;
          }
          Ok(Containment::Outside) => {}
        }

        match ranges_overlap_or_adjacent(start, end, prev_start, prev_end) {
          Ok(MergeDecision::Merge) => {
            start = min(start, prev_start);
            end = max(end, prev_end);
            self.ranges.remove(&prev_start);
          }
          Ok(MergeDecision::KeepSeparate) => {}
        }
      }
      ::core::option::Option::None => {}
    };

    let Ok(max_removals) = console_core_number_conversion::fitted::<_, u64>(self.ranges.len());

    for _ in 0..max_removals {
      let (next_start, next_end) = match self.next_range(start) {
        Ok(Some(next)) => next,
        Ok(None) => break,
      };

      match range_is_subset(start, end, next_start, next_end) {
        Ok(Containment::Subset) => {
          return;
        }
        Ok(Containment::Outside) => {}
      }

      match ranges_overlap_or_adjacent(start, end, next_start, next_end) {
        Ok(MergeDecision::Merge) => {
          start = min(start, next_start);
          end = max(end, next_end);
          self.ranges.remove(&next_start);
        }
        Ok(MergeDecision::KeepSeparate) => break,
      }
    }

    self.ranges.insert(start, end);
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `RangeSet::iter` method returns its iterator or trait value directly for existing callers to compose")
  )]
  pub fn iter(&'_ self) -> impl Iterator<Item = RangeInclusive<T>> + '_ {
    self.ranges.iter().map(|(a, b)| *a..=*b)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `RangeSet::intersection` method returns its iterator or trait value directly for existing callers to compose")
  )]
  pub fn intersection<'a>(
    &'a self,
    other: &'a Self,
  ) -> impl Iterator<Item = RangeInclusive<T>> + 'a {
    let Ok(left_count) = console_core_number_conversion::wrapped::<_, u64>(self.ranges.len());
    let Ok(right_count) = console_core_number_conversion::wrapped::<_, u64>(other.ranges.len());
    IntersectionIter {
      it_a: self.iter().peekable(),
      it_b: other.iter().peekable(),
      steps_remaining: left_count.saturating_add(right_count),
    }
  }

  #[must_use]
  fn next_range(&self, start: T) -> core::result::Result<Option<(T, T)>, console_core_never::Never> {
      let (next_start, next_end) = match self.ranges.range(start..).next() { Some(explicit_value) => explicit_value, None => return Ok(None) };

      Ok(Some((*next_start, *next_end)))
  }

  #[must_use]
  fn prev_range(&self, start: T) -> core::result::Result<Option<(T, T)>, console_core_never::Never> {
      let (next_start, next_end) = match self.ranges.range(..start).next_back() { Some(explicit_value) => explicit_value, None => return Ok(None) };

      Ok(Some((*next_start, *next_end)))
  }
}

impl<T> Extend<RangeInclusive<T>> for RangeSet<T>
where
  T: Copy + Ord + OrdAdjacency,
{
  fn extend<I: IntoIterator<Item = RangeInclusive<T>>>(&mut self, iter: I) {
    for range in iter {
      self.insert(range);
    }
  }
}

impl<T> FromIterator<RangeInclusive<T>> for RangeSet<T>
where
  T: Default + Copy + Ord + OrdAdjacency,
{
  fn from_iter<I: IntoIterator<Item = RangeInclusive<T>>>(iter: I) -> Self {
    let mut result: Self = Default::default();
    result.extend(iter);
    result
  }
}

impl<T> Debug for RangeSet<T>
where
  T: Debug,
{
  fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
    write!(f, "RangeSet {{")?;

    for (start, end) in self.ranges.iter() {
      write!(f, "[{:?}, {:?}], ", start, end)?;
    }

    write!(f, "}}")
  }
}

struct IntersectionIter<A, B, T>
where
  A: Iterator<Item = RangeInclusive<T>>,
  B: Iterator<Item = RangeInclusive<T>>,
{
  it_a: Peekable<A>,
  it_b: Peekable<B>,
  steps_remaining: u64,
}

impl<A, B, T> Iterator for IntersectionIter<A, B, T>
where
  A: Iterator<Item = RangeInclusive<T>>,
  B: Iterator<Item = RangeInclusive<T>>,
  T: Ord + Copy,
{
  type Item = RangeInclusive<T>;

  fn next(&mut self) -> Option<Self::Item> {
    for _ in 0..self.steps_remaining {
      let (left, right) = match (self.it_a.peek(), self.it_b.peek()) {
        (Some(left), Some(right)) => (left.clone(), right.clone()),
        (None, _) | (_, None) => return None,
      };

      let Ok(()) = self.step_iterators(&left, &right);
      self.steps_remaining = self.steps_remaining.saturating_sub(1);

      match range_intersection(&left, &right) {
        Ok(Some(intersection)) => return Some(intersection),
        Ok(None) => {}
      }
    }

    None
  }
}

impl<A, B, T> IntersectionIter<A, B, T>
where
  A: Iterator<Item = RangeInclusive<T>>,
  B: Iterator<Item = RangeInclusive<T>>,
  T: Ord,
{
  fn step_iterators(&mut self, a: &RangeInclusive<T>, b: &RangeInclusive<T>) -> core::result::Result<(), console_core_never::Never> { {
    match ::core::primitive::bool::then_some(a.end() <= b.end(), ()) {
      ::core::option::Option::Some(()) => {
        self.it_a.next();
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(a.end() >= b.end(), ()) {
      ::core::option::Option::Some(()) => {
        self.it_b.next();
      }
      ::core::option::Option::None => {}
    }
  }; Ok(()) }
}

impl OrdAdjacency for u32 {
  fn are_adjacent(self, rhs: u32) -> bool {
    matches!(self.checked_add(1).map(|r| r == rhs), Some(true))
      || matches!(rhs.checked_add(1).map(|r| r == self), Some(true))
  }
}

impl OrdAdjacency for u16 {
  fn are_adjacent(self, rhs: u16) -> bool {
    matches!(self.checked_add(1).map(|r| r == rhs), Some(true))
      || matches!(rhs.checked_add(1).map(|r| r == self), Some(true))
  }
}

impl OrdAdjacency for Fixed {
  fn are_adjacent(self, rhs: Fixed) -> bool {
    let Ok(forward) = self.checked_add(Fixed::EPSILON);
    let Ok(backward) = rhs.checked_add(Fixed::EPSILON);

    forward == Some(rhs) || backward == Some(self)
  }
}

fn range_intersection<T: Ord + Copy>(
  a: &RangeInclusive<T>,
  b: &RangeInclusive<T>,
) -> ::core::result::Result<Option<RangeInclusive<T>>, ::console_core_never::Never> {
  ::core::result::Result::Ok(match ::core::primitive::bool::then_some(a.start() <= b.end() && b.start() <= a.end(), ()) {
    ::core::option::Option::Some(()) => Some(*max(a.start(), b.start())..=*min(a.end(), b.end())),
    ::core::option::Option::None => None,
  })
}

enum MergeDecision {
  Merge,
  KeepSeparate,
}

enum Containment {
  Subset,
  Outside,
}

fn ranges_overlap_or_adjacent<T>(a_start: T, a_end: T, b_start: T, b_end: T) -> Result<MergeDecision, console_core_never::Never>
where
  T: Ord + OrdAdjacency,
{
  Ok(match crate::Presence::from((a_start <= b_end && b_start <= a_end)
    || a_end.are_adjacent(b_start)
    || b_end.are_adjacent(a_start)) {
    crate::Presence::Present => MergeDecision::Merge,
    crate::Presence::Absent => MergeDecision::KeepSeparate,
  })
}

fn range_is_subset<T: Ord>(a_start: T, a_end: T, b_start: T, b_end: T) -> Result<Containment, console_core_never::Never> {
  Ok(match crate::Presence::from(a_start >= b_start && a_end <= b_end) {
    crate::Presence::Present => Containment::Subset,
    crate::Presence::Absent => Containment::Outside,
  })
}

#[cfg(test)]
mod test {

  use super::*;

  #[test]
  fn insert_invalid() {
    let mut map: RangeSet<u32> = Default::default();
    map.insert(core::ops::RangeInclusive::new(12, 11));
    assert_eq!(map.iter().collect::<Vec<_>>(), vec![],);
  }

  #[test]
  fn insert_non_overlapping() {
    let mut map: RangeSet<u32> = Default::default();

    map.insert(11..=11);
    map.insert(2..=3);
    map.insert(6..=9);

    assert_eq!(map.iter().collect::<Vec<_>>(), vec![2..=3, 6..=9, 11..=11],);
  }

  #[test]
  fn insert_subset_before() {
    let mut map: RangeSet<u32> = Default::default();

    map.insert(2..=8);
    map.insert(3..=7);

    assert_eq!(map.iter().collect::<Vec<_>>(), vec![2..=8],);
  }

  #[test]
  fn insert_subset_after() {
    let mut map: RangeSet<u32> = Default::default();

    map.insert(2..=8);
    map.insert(2..=7);
    map.insert(2..=8);

    assert_eq!(map.iter().collect::<Vec<_>>(), vec![2..=8],);
  }

  #[test]
  fn insert_overlapping_before() {
    let mut map: RangeSet<u32> = Default::default();

    map.insert(2..=8);
    map.insert(7..=11);

    assert_eq!(map.iter().collect::<Vec<_>>(), vec![2..=11],);
  }

  #[test]
  fn insert_overlapping_after() {
    let mut map: RangeSet<u32> = Default::default();
    map.insert(10..=14);
    map.insert(7..=11);
    assert_eq!(map.iter().collect::<Vec<_>>(), vec![7..=14],);

    let mut map_2: RangeSet<u32> = Default::default();
    map_2.insert(10..=14);
    map_2.insert(10..=17);
    assert_eq!(map_2.iter().collect::<Vec<_>>(), vec![10..=17],);
  }

  #[test]
  fn insert_overlapping_multiple_after() {
    let mut map: RangeSet<u32> = Default::default();
    map.insert(10..=14);
    map.insert(16..=17);
    map.insert(7..=16);
    assert_eq!(map.iter().collect::<Vec<_>>(), vec![7..=17],);

    let mut map_2: RangeSet<u32> = Default::default();
    map_2.insert(10..=14);
    map_2.insert(16..=17);
    map_2.insert(10..=16);
    assert_eq!(map_2.iter().collect::<Vec<_>>(), vec![10..=17],);

    let mut map_3: RangeSet<u32> = Default::default();
    map_3.insert(10..=14);
    map_3.insert(16..=17);
    map_3.insert(10..=17);
    assert_eq!(map_3.iter().collect::<Vec<_>>(), vec![10..=17],);
  }

  #[test]
  fn insert_overlapping_before_and_after() {
    let mut map: RangeSet<u32> = Default::default();

    map.insert(6..=8);
    map.insert(10..=14);
    map.insert(16..=20);

    map.insert(7..=19);

    assert_eq!(map.iter().collect::<Vec<_>>(), vec![6..=20],);
  }

  #[test]
  fn insert_joins_adjacent() {
    let mut map: RangeSet<u32> = Default::default();
    map.insert(6..=8);
    map.insert(9..=10);
    assert_eq!(map.iter().collect::<Vec<_>>(), vec![6..=10],);

    let mut map_2: RangeSet<u32> = Default::default();
    map_2.insert(9..=10);
    map_2.insert(6..=8);
    assert_eq!(map_2.iter().collect::<Vec<_>>(), vec![6..=10],);

    let mut map_3: RangeSet<u32> = Default::default();
    map_3.insert(6..=8);
    map_3.insert(10..=10);
    map_3.insert(9..=9);
    assert_eq!(map_3.iter().collect::<Vec<_>>(), vec![6..=10],);
  }

  #[test]
  fn from_iter_and_extend() {
    let mut map: RangeSet<u32> = [2..=5, 13..=64, 7..=9].into_iter().collect();
    assert_eq!(map.iter().collect::<Vec<_>>(), vec![2..=5, 7..=9, 13..=64],);

    map.extend([6..=17, 100..=101]);

    assert_eq!(map.iter().collect::<Vec<_>>(), vec![2..=64, 100..=101],);
  }

  #[test]
  fn intersection() {
    let a: RangeSet<u32> = [2..=5, 7..=9, 13..=64].into_iter().collect();
    let b: RangeSet<u32> = [1..=3, 5..=8, 13..=64, 67..=69].into_iter().collect();

    let expected = vec![2..=3, 5..=5, 7..=8, 13..=64];

    assert_eq!(a.intersection(&b).collect::<Vec<_>>(), expected);
    assert_eq!(b.intersection(&a).collect::<Vec<_>>(), expected);
  }
}
