//! Ordered unsigned sets store either their members or their exceptions.
//!
//! Fixed-width bitmap pages provide sparse storage. Domains define the ordered
//! values represented by each integer; inversion and set algebra preserve that
//! domain, including gaps and empty domains. Serde retains the former page shape.
//!
//! Every question a set answers comes back as the answer's name rather than as
//! a `bool`: a value is present or absent, a set is empty or not, two sets
//! intersect or are disjoint, a domain is continuous or has gaps. Inserting and
//! removing answer with the presence the value had before the call, which is the
//! one fact the call cannot otherwise give back.
//!
//! A set that holds its exceptions takes every change as the opposite mark on
//! them, so inserting and removing, of a value or of a range, are one change
//! told which mark it is. Union, intersection and difference are one table: for
//! each pair of stored forms it names the operation on the stored values that
//! gives the answer, and whether the answer then holds the other form.

#![forbid(unsafe_code)]
#![cfg_attr(not(any(feature = "std", test)), no_std)]

extern crate alloc;
#[cfg(not(any(feature = "std", test)))]
extern crate core as std;

#[cfg(feature = "serde")]
use alloc::format;
use console_core_never::Never;
use core::cmp::Ordering;
use core::fmt::{Debug, Display};
use core::hash::Hash;
use core::marker::PhantomData;
use core::ops::{Bound, RangeBounds, RangeInclusive};

mod bitset;
mod domain;
mod input_bit_stream;
mod iteration;
mod output_bit_stream;
pub mod sparse_bit_set;

use bitset::Mark;
pub use bitset::U32Set;
use iteration::{Bounds, Selection, Visibility};

#[derive(Clone)]
pub struct IntSet<T>(Stored, PhantomData<T>);

pub trait Domain: Sized + Copy {
  fn to_u32(&self) -> Result<u32, Never>;

  fn contains(value: u32) -> Result<Presence, Never>;

  fn from_u32(member: InDomain) -> Result<Self, Never>;

  fn continuity() -> Result<Continuity, Never>;

  fn ordered_values() -> Result<impl DoubleEndedIterator<Item = u32>, Never>;

  fn ordered_values_range(
    range: RangeInclusive<Self>,
  ) -> Result<impl DoubleEndedIterator<Item = u32>, Never>;

  fn count() -> Result<u64, Never>;
}

pub struct InDomain(u32);

impl InDomain {
  pub fn value(&self) -> Result<u32, Never> {
    Ok(self.0)
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Presence {
  Present,
  Absent,
}

impl From<bool> for Presence {
  fn from(present: bool) -> Self {
    match bool::then_some(present, ()) {
      Some(()) => Self::Present,
      None => Self::Absent,
    }
  }
}

impl Presence {
  fn opposite(self) -> Result<Self, Never> {
    Ok(match self {
      Self::Present => Self::Absent,
      Self::Absent => Self::Present,
    })
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Emptiness {
  Empty,
  NonEmpty,
}

impl From<bool> for Emptiness {
  fn from(empty: bool) -> Self {
    match bool::then_some(empty, ()) {
      Some(()) => Self::Empty,
      None => Self::NonEmpty,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Overlap {
  Intersecting,
  Disjoint,
}

impl From<bool> for Overlap {
  fn from(intersecting: bool) -> Self {
    match bool::then_some(intersecting, ()) {
      Some(()) => Self::Intersecting,
      None => Self::Disjoint,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Membership {
  Inclusive,
  Exclusive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Continuity {
  Continuous,
  Discontinuous,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(
  feature = "serde",
  derive(serde::Serialize, serde::Deserialize),
  serde(rename = "Membership")
)]
enum Stored {
  Inclusive(U32Set),
  Exclusive(U32Set),
}

#[derive(Clone, Copy)]
enum Combination {
  Union,
  Intersection,
  Difference,
}

impl<T> Default for IntSet<T> {
  fn default() -> Self {
    let Ok(set) = Self::empty();

    set
  }
}

impl<T> IntSet<T> {
  pub const fn new() -> Result<Self, Never> {
    Self::empty()
  }

  pub const fn empty() -> Result<Self, Never> {
    Ok(Self(Stored::Inclusive(U32Set::EMPTY), PhantomData))
  }

  pub const fn all() -> Result<Self, Never> {
    Ok(Self(Stored::Exclusive(U32Set::EMPTY), PhantomData))
  }

  pub fn membership(&self) -> Result<Membership, Never> {
    Ok(match self.0 {
      Stored::Inclusive(_) => Membership::Inclusive,
      Stored::Exclusive(_) => Membership::Exclusive,
    })
  }

  pub fn invert(&mut self) -> Result<(), Never> {
    let stored = core::mem::replace(&mut self.0, Stored::Inclusive(U32Set::EMPTY));

    self.0 = match stored {
      Stored::Inclusive(bits) => Stored::Exclusive(bits),
      Stored::Exclusive(bits) => Stored::Inclusive(bits),
    };

    Ok(())
  }

  pub fn clear(&mut self) -> Result<(), Never> {
    self.0 = Stored::Inclusive(U32Set::EMPTY);

    Ok(())
  }

  fn stored(&self) -> Result<&U32Set, Never> {
    Ok(match &self.0 {
      Stored::Inclusive(bits) | Stored::Exclusive(bits) => bits,
    })
  }

  fn selection(&self, visibility: Visibility) -> Result<Selection, Never> {
    Ok(match (&self.0, visibility) {
      (Stored::Inclusive(_), Visibility::Included)
      | (Stored::Exclusive(_), Visibility::Excluded) => Selection::Members,
      (Stored::Exclusive(_), Visibility::Included)
      | (Stored::Inclusive(_), Visibility::Excluded) => Selection::Exceptions,
    })
  }
}

impl<T: Domain> IntSet<T> {
  pub fn iter(&self) -> Result<impl DoubleEndedIterator<Item = T> + '_, Never> {
    self.range(..)
  }

  pub fn inclusive_iter(&self) -> Result<Option<impl DoubleEndedIterator<Item = T> + '_>, Never> {
    Ok(match &self.0 {
      Stored::Inclusive(members) => {
        let Ok(values) = members.iter();

        Some(values.map(|raw| {
          let Ok(member) = T::from_u32(InDomain(raw));

          member
        }))
      }
      Stored::Exclusive(_) => None,
    })
  }

  pub fn iter_after(&self, value: T) -> Result<impl Iterator<Item = T> + '_, Never> {
    self.range((Bound::Excluded(value), Bound::Unbounded))
  }

  pub fn range<R: RangeBounds<T>>(
    &self,
    range: R,
  ) -> Result<impl DoubleEndedIterator<Item = T> + '_, Never> {
    let Ok(bounds) = Bounds::new(range);
    let Ok(bits) = self.stored();
    let Ok(selection) = self.selection(Visibility::Included);
    let Ok(values) = iteration::values(bits, selection, bounds);

    Ok(values.map(|raw| {
      let Ok(member) = T::from_u32(InDomain(raw));

      member
    }))
  }

  pub fn iter_ranges(&self) -> Result<impl Iterator<Item = RangeInclusive<T>> + '_, Never> {
    self.ranges(Visibility::Included)
  }

  pub fn iter_excluded_ranges(
    &self,
  ) -> Result<impl Iterator<Item = RangeInclusive<T>> + '_, Never> {
    self.ranges(Visibility::Excluded)
  }

  fn ranges(
    &self,
    visibility: Visibility,
  ) -> Result<impl Iterator<Item = RangeInclusive<T>> + '_, Never> {
    let Ok(bits) = self.stored();
    let Ok(selection) = self.selection(visibility);
    let Ok(ranges) = iteration::ranges::<T>(bits, selection);

    Ok(ranges.map(|range| {
      let Ok(first) = T::from_u32(InDomain(*range.start()));
      let Ok(last) = T::from_u32(InDomain(*range.end()));

      first..=last
    }))
  }

  pub fn insert(&mut self, value: T) -> Result<Presence, Never> {
    self.mark(value, Mark::Set)
  }

  pub fn remove(&mut self, value: T) -> Result<Presence, Never> {
    self.mark(value, Mark::Clear)
  }

  fn mark(&mut self, value: T, mark: Mark) -> Result<Presence, Never> {
    let Ok(raw) = value.to_u32();

    match &mut self.0 {
      Stored::Inclusive(members) => members.mark(raw, mark),
      Stored::Exclusive(exceptions) => {
        let Ok(opposite) = mark.opposite();
        let Ok(excepted) = exceptions.mark(raw, opposite);

        excepted.opposite()
      }
    }
  }

  pub fn insert_range(&mut self, range: RangeInclusive<T>) -> Result<(), Never> {
    self.mark_range(range, Mark::Set)
  }

  pub fn remove_range(&mut self, range: RangeInclusive<T>) -> Result<(), Never> {
    self.mark_range(range, Mark::Clear)
  }

  fn mark_range(&mut self, range: RangeInclusive<T>, mark: Mark) -> Result<(), Never> {
    let Ok(continuity) = T::continuity();
    let Ok(opposite) = mark.opposite();
    let (bits, held) = match &mut self.0 {
      Stored::Inclusive(members) => (members, mark),
      Stored::Exclusive(exceptions) => (exceptions, opposite),
    };

    match continuity {
      Continuity::Continuous => {
        let Ok(raw) = raw_range(&range);

        bits.mark_range(raw, held)
      }
      Continuity::Discontinuous => {
        let Ok(values) = T::ordered_values_range(range);

        bits.mark_all(values, held)
      }
    }
  }

  pub fn extend_unsorted<U: IntoIterator<Item = T>>(&mut self, values: U) -> Result<(), Never> {
    self.extend(values);

    Ok(())
  }

  pub fn remove_all<U: IntoIterator<Item = T>>(&mut self, values: U) -> Result<(), Never> {
    for value in values {
      let Ok(_before) = self.remove(value);
    }

    Ok(())
  }

  pub fn union(&mut self, other: &Self) -> Result<(), Never> {
    self.combine(other, Combination::Union)
  }

  pub fn intersect(&mut self, other: &Self) -> Result<(), Never> {
    self.combine(other, Combination::Intersection)
  }

  pub fn subtract(&mut self, other: &Self) -> Result<(), Never> {
    self.combine(other, Combination::Difference)
  }

  fn combine(&mut self, other: &Self, combination: Combination) -> Result<(), Never> {
    match (combination, &mut self.0, &other.0) {
      (Combination::Union, Stored::Inclusive(left), Stored::Inclusive(right))
      | (Combination::Intersection, Stored::Exclusive(left), Stored::Exclusive(right))
      | (Combination::Difference, Stored::Exclusive(left), Stored::Inclusive(right)) => {
        left.union(right)
      }
      (Combination::Union, Stored::Exclusive(left), Stored::Exclusive(right))
      | (Combination::Intersection, Stored::Inclusive(left), Stored::Inclusive(right))
      | (Combination::Difference, Stored::Inclusive(left), Stored::Exclusive(right)) => {
        left.intersect(right)
      }
      (Combination::Union, Stored::Exclusive(left), Stored::Inclusive(right))
      | (Combination::Intersection, Stored::Inclusive(left), Stored::Exclusive(right))
      | (Combination::Difference, Stored::Inclusive(left), Stored::Inclusive(right)) => {
        left.subtract(right)
      }
      (Combination::Union, Stored::Inclusive(left), Stored::Exclusive(right))
      | (Combination::Intersection, Stored::Exclusive(left), Stored::Inclusive(right))
      | (Combination::Difference, Stored::Exclusive(left), Stored::Exclusive(right)) => {
        let Ok(()) = left.reversed_subtract(right);

        self.invert()
      }
    }
  }

  pub fn intersects_range(&self, range: RangeInclusive<T>) -> Result<Overlap, Never> {
    let Ok(mut values) = self.range(range);

    Ok(match values.next() {
      Some(_member) => Overlap::Intersecting,
      None => Overlap::Disjoint,
    })
  }

  pub fn intersects_set(&self, other: &Self) -> Result<Overlap, Never> {
    match (&self.0, &other.0) {
      (Stored::Inclusive(left), Stored::Inclusive(right)) => left.intersects_set(right),
      (Stored::Inclusive(members), Stored::Exclusive(exceptions))
      | (Stored::Exclusive(exceptions), Stored::Inclusive(members)) => {
        let Ok(mut values) = members.iter();

        Ok(Overlap::from(values.any(|value| {
          matches!(exceptions.contains(value), Ok(Presence::Absent))
        })))
      }
      (Stored::Exclusive(left), Stored::Exclusive(right)) => {
        let mut exceptions = left.clone();
        let Ok(()) = exceptions.union(right);
        let Ok(excepted) = exceptions.len();
        let Ok(total) = T::count();

        Ok(Overlap::from(excepted < total))
      }
    }
  }

  pub fn contains(&self, value: T) -> Result<Presence, Never> {
    let Ok(raw) = value.to_u32();

    match &self.0 {
      Stored::Inclusive(members) => members.contains(raw),
      Stored::Exclusive(exceptions) => {
        let Ok(excepted) = exceptions.contains(raw);

        excepted.opposite()
      }
    }
  }

  pub fn first(&self) -> Result<Option<T>, Never> {
    let Ok(mut values) = self.iter();

    Ok(values.next())
  }

  pub fn last(&self) -> Result<Option<T>, Never> {
    let Ok(mut values) = self.iter();

    Ok(values.next_back())
  }

  #[allow(
    clippy::len_without_is_empty,
    reason = "is_empty answers an Emptiness, which is not the bool clippy looks for"
  )]
  pub fn len(&self) -> Result<u64, Never> {
    let Ok(bits) = self.stored();
    let Ok(stored) = bits.len();

    Ok(match &self.0 {
      Stored::Inclusive(_) => stored,
      Stored::Exclusive(_) => {
        let Ok(total) = T::count();

        total.saturating_sub(stored)
      }
    })
  }

  pub fn is_empty(&self) -> Result<Emptiness, Never> {
    let Ok(length) = self.len();

    Ok(match length {
      0 => Emptiness::Empty,
      _members => Emptiness::NonEmpty,
    })
  }

  fn raw_ranges(&self) -> Result<impl Iterator<Item = RangeInclusive<u32>> + '_, Never> {
    let Ok(bits) = self.stored();
    let Ok(selection) = self.selection(Visibility::Included);

    iteration::ranges::<T>(bits, selection)
  }
}

fn raw_range<T: Domain>(range: &RangeInclusive<T>) -> Result<RangeInclusive<u32>, Never> {
  let Ok(first) = range.start().to_u32();
  let Ok(last) = range.end().to_u32();

  Ok(first..=last)
}

impl IntSet<u32> {
  pub(crate) fn from_bitset(bits: U32Set) -> Result<Self, Never> {
    Ok(Self(Stored::Inclusive(bits), PhantomData))
  }
}

impl<T: Domain> FromIterator<T> for IntSet<T> {
  fn from_iter<I: IntoIterator<Item = T>>(values: I) -> Self {
    let mut set = Self::default();

    set.extend(values);

    set
  }
}

impl<T: Domain> Extend<T> for IntSet<T> {
  fn extend<I: IntoIterator<Item = T>>(&mut self, values: I) {
    for value in values {
      let Ok(_before) = self.insert(value);
    }
  }
}

impl<T: Domain> PartialEq for IntSet<T> {
  fn eq(&self, other: &Self) -> bool {
    match (&self.0, &other.0) {
      (Stored::Inclusive(left), Stored::Inclusive(right))
      | (Stored::Exclusive(left), Stored::Exclusive(right)) => left == right,
      (Stored::Inclusive(_), Stored::Exclusive(_))
      | (Stored::Exclusive(_), Stored::Inclusive(_)) => {
        let Ok(left_count) = self.len();
        let Ok(right_count) = other.len();

        match left_count.cmp(&right_count) {
          Ordering::Less | Ordering::Greater => false,
          Ordering::Equal => {
            let Ok(left) = self.raw_ranges();
            let Ok(right) = other.raw_ranges();

            left.eq(right)
          }
        }
      }
    }
  }
}

impl<T: Domain> Eq for IntSet<T> {}

impl<T: Domain> Hash for IntSet<T> {
  fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
    let Ok(ranges) = self.raw_ranges();

    for range in ranges {
      range.hash(state);
    }
  }
}

impl<T: Domain + Ord> Ord for IntSet<T> {
  fn cmp(&self, other: &Self) -> Ordering {
    let Ok(mut left) = self.raw_ranges();
    let Ok(mut right) = other.raw_ranges();

    for first in left.by_ref() {
      let second = match right.next() {
        Some(range) => range,
        None => return Ordering::Greater,
      };
      let start = first.start().cmp(second.start());

      match start {
        Ordering::Less | Ordering::Greater => return start,
        Ordering::Equal => {}
      }

      match first.end().cmp(second.end()) {
        Ordering::Equal => {}
        Ordering::Less => {
          return match left.next() {
            Some(_later) => Ordering::Greater,
            None => Ordering::Less,
          };
        }
        Ordering::Greater => {
          return match right.next() {
            Some(_later) => Ordering::Less,
            None => Ordering::Greater,
          };
        }
      }
    }

    match right.next() {
      Some(_later) => Ordering::Less,
      None => Ordering::Equal,
    }
  }
}

impl<T: Domain + Ord> PartialOrd for IntSet<T> {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl<T: Domain, const N: usize> From<[T; N]> for IntSet<T> {
  fn from(values: [T; N]) -> Self {
    values.into_iter().collect()
  }
}

impl<T: Domain + Debug> Debug for IntSet<T> {
  fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    let Ok(values) = self.iter();

    output.debug_set().entries(values).finish()
  }
}

impl<T: Domain + Display> Display for IntSet<T> {
  fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    let Ok(ranges) = self.iter_ranges();
    let mut separator = "";

    write!(output, "{{ ")?;

    for range in ranges {
      write!(output, "{separator}{}..={}", range.start(), range.end())?;
      separator = ", ";
    }

    write!(output, "}}")
  }
}

#[cfg(feature = "serde")]
impl<T: Domain> serde::Serialize for IntSet<T> {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    self.0.serialize(serializer)
  }
}

#[cfg(feature = "serde")]
impl<'de, T: Domain> serde::Deserialize<'de> for IntSet<T> {
  fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let stored = Stored::deserialize(deserializer)?;
    let bits = match &stored {
      Stored::Inclusive(bits) | Stored::Exclusive(bits) => bits,
    };
    let outside = {
      let Ok(mut values) = bits.iter();

      values.find(|raw| matches!(T::contains(*raw), Ok(Presence::Absent)))
    };

    match outside {
      Some(value) => {
        return Err(serde::de::Error::custom(format!(
          "value '{value}' out of range for domain {}",
          core::any::type_name::<T>()
        )));
      }
      None => {}
    }

    Ok(Self(stored, PhantomData))
  }
}

#[cfg(test)]
mod test {
  use core::cmp::Ordering;
  #[cfg(feature = "font-types")]
  use font_types::{GlyphId, GlyphId16, Tag};
  use std::{collections::HashSet, hash::Hash};

  use super::*;

  fn among(ranges: &[RangeInclusive<u32>], value: u32) -> Result<Presence, Never> {
    Ok(Presence::from(ranges.iter().any(|range| range.contains(&value))))
  }

  fn raw_bounds<T: Domain>(range: &RangeInclusive<T>) -> Result<RangeInclusive<u32>, Never> {
    let Ok(start) = range.start().to_u32();
    let Ok(end) = range.end().to_u32();

    Ok(start..=end)
  }

  #[derive(PartialEq, Eq, Debug, PartialOrd, Ord, Clone, Copy)]
  struct EvenInts(u16);

  impl Domain for EvenInts {
    fn to_u32(&self) -> Result<u32, Never> {
      Ok(u32::from(self.0))
    }

    fn contains(value: u32) -> Result<Presence, Never> {
      Ok(Presence::from(
        value <= u32::from(u16::MAX) && value.is_multiple_of(2),
      ))
    }

    fn from_u32(member: InDomain) -> Result<EvenInts, Never> {
      let Ok(value) = member.value();
      let Ok(narrow) = console_core_number_conversion::fitted::<_, u16>(value);

      Ok(EvenInts(narrow))
    }

    fn continuity() -> Result<Continuity, Never> {
      Ok(Continuity::Discontinuous)
    }

    fn ordered_values() -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
      Ok(
        (u16::MIN..=u16::MAX)
          .filter(|value| value.is_multiple_of(2))
          .map(u32::from),
      )
    }

    fn ordered_values_range(
      range: RangeInclusive<EvenInts>,
    ) -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
      let Ok(values) = Self::ordered_values();
      let Ok(bounds) = raw_bounds(&range);

      Ok(values.filter(move |value| bounds.contains(value)))
    }

    fn count() -> Result<u64, Never> {
      Ok(32768)
    }
  }

  #[derive(PartialEq, Eq, Debug, PartialOrd, Ord, Hash, Clone, Copy)]
  struct TwoParts(u16);

  impl Domain for TwoParts {
    fn to_u32(&self) -> Result<u32, Never> {
      Ok(u32::from(self.0))
    }

    fn contains(value: u32) -> Result<Presence, Never> {
      among(&[2..=5, 8..=16], value)
    }

    fn from_u32(member: InDomain) -> Result<TwoParts, Never> {
      let Ok(value) = member.value();
      let Ok(narrow) = console_core_number_conversion::fitted::<_, u16>(value);

      Ok(TwoParts(narrow))
    }

    fn continuity() -> Result<Continuity, Never> {
      Ok(Continuity::Discontinuous)
    }

    fn ordered_values() -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
      Ok([2..=5, 8..=16].into_iter().flatten())
    }

    fn ordered_values_range(
      range: RangeInclusive<TwoParts>,
    ) -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
      let Ok(values) = Self::ordered_values();
      let Ok(bounds) = raw_bounds(&range);

      Ok(values.filter(move |value| bounds.contains(value)))
    }

    fn count() -> Result<u64, Never> {
      Ok(13)
    }
  }

  #[derive(PartialEq, Eq, Debug, PartialOrd, Ord, Clone, Copy)]
  struct TwoPartsBounds(u32);

  impl Domain for TwoPartsBounds {
    fn to_u32(&self) -> Result<u32, Never> {
      Ok(self.0)
    }

    fn contains(value: u32) -> Result<Presence, Never> {
      among(&[0..=1, u32::MAX.saturating_sub(1)..=u32::MAX], value)
    }

    fn from_u32(member: InDomain) -> Result<TwoPartsBounds, Never> {
      let Ok(value) = member.value();

      Ok(TwoPartsBounds(value))
    }

    fn continuity() -> Result<Continuity, Never> {
      Ok(Continuity::Discontinuous)
    }

    fn ordered_values() -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
      Ok(
        [0..=1, u32::MAX.saturating_sub(1)..=u32::MAX]
          .into_iter()
          .flatten(),
      )
    }

    fn ordered_values_range(
      range: RangeInclusive<TwoPartsBounds>,
    ) -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
      let Ok(values) = Self::ordered_values();
      let Ok(bounds) = raw_bounds(&range);

      Ok(values.filter(move |value| bounds.contains(value)))
    }

    fn count() -> Result<u64, Never> {
      Ok(4)
    }
  }

  #[test]
  fn from_sparse_set() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;
    let bytes = [0b00001101, 0b00000011, 0b00110001];
    let set = IntSet::<u32>::from_sparse_bit_set(&bytes).required()?;
    let Ok(empty) = IntSet::<u32>::empty();
    let mut expected: IntSet<u32> = empty;
    let Ok(()) = expected.insert_range(0..=17);
    assert_eq!(set, expected);

    Ok(())
  }

  #[test]
  fn insert() -> Result<(), crate::TestFailure> {
    let Ok(mut empty) = IntSet::<u32>::empty();
    let Ok(mut all) = IntSet::<u32>::all();
    assert_eq!(empty.contains(10), Ok(Presence::Absent));
    assert_eq!(empty.insert(10), Ok(Presence::Absent));
    assert_eq!(empty.contains(10), Ok(Presence::Present));
    assert_eq!(empty.insert(10), Ok(Presence::Present));
    assert_eq!(all.contains(10), Ok(Presence::Present));
    assert_eq!(all.insert(10), Ok(Presence::Present));
    assert_eq!(all.contains(10), Ok(Presence::Present));
    assert_eq!(all.insert(10), Ok(Presence::Present));

    Ok(())
  }

  #[test]
  fn remove() -> Result<(), crate::TestFailure> {
    let Ok(mut empty) = IntSet::<u32>::empty();
    let Ok(_before) = empty.insert(10);
    let Ok(mut all) = IntSet::<u32>::all();
    assert_eq!(empty.contains(10), Ok(Presence::Present));
    assert_eq!(empty.remove(10), Ok(Presence::Present));
    assert_eq!(empty.contains(10), Ok(Presence::Absent));
    assert_eq!(empty.remove(10), Ok(Presence::Absent));
    assert_eq!(all.contains(10), Ok(Presence::Present));
    assert_eq!(all.remove(10), Ok(Presence::Present));
    assert_eq!(all.contains(10), Ok(Presence::Absent));
    assert_eq!(all.remove(10), Ok(Presence::Absent));

    Ok(())
  }

  #[test]
  fn is_empty() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<u32>::empty();
    assert_eq!(set.is_empty(), Ok(Emptiness::Empty));
    let Ok(_before) = set.insert(13);
    let Ok(_before) = set.insert(800);
    assert_eq!(set.is_empty(), Ok(Emptiness::NonEmpty));
    let Ok(()) = set.invert();
    assert_eq!(set.is_empty(), Ok(Emptiness::NonEmpty));
    let Ok(mut empty) = IntSet::<u32>::empty();
    assert_eq!(empty.is_empty(), Ok(Emptiness::Empty));
    let Ok(()) = empty.invert();
    assert_eq!(empty.is_empty(), Ok(Emptiness::NonEmpty));

    Ok(())
  }

  #[test]
  fn first() -> Result<(), crate::TestFailure> {
    let Ok(set) = IntSet::<u16>::empty();
    let Ok(first) = set.first();
    assert_eq!(first, None);
    let Ok(set_2) = IntSet::<u16>::all();
    assert_eq!(set_2.first(), Ok(Some(0)));
    let Ok(mut set_3) = IntSet::<u16>::empty();
    set_3.extend([0]);
    assert_eq!(set_3.first(), Ok(Some(0)));
    let Ok(mut set_4) = IntSet::<u16>::empty();
    set_4.extend([u16::MAX]);
    assert_eq!(set_4.first(), Ok(Some(u16::MAX)));
    let Ok(mut set_5) = IntSet::<u16>::empty();
    set_5.extend([100, 1000, 10000]);
    assert_eq!(set_5.first(), Ok(Some(100)));
    let Ok(()) = set_5.invert();
    assert_eq!(set_5.first(), Ok(Some(0)));
    let Ok(()) = set_5.remove_range(0..=100);
    assert_eq!(set_5.first(), Ok(Some(101)));

    Ok(())
  }

  #[test]
  fn last() -> Result<(), crate::TestFailure> {
    let Ok(set) = IntSet::<u16>::empty();
    let Ok(last) = set.last();
    assert_eq!(last, None);
    let Ok(set_2) = IntSet::<u16>::all();
    assert_eq!(set_2.last(), Ok(Some(u16::MAX)));
    let Ok(mut set_3) = IntSet::<u16>::empty();
    set_3.extend([0]);
    assert_eq!(set_3.last(), Ok(Some(0)));
    let Ok(mut set_4) = IntSet::<u16>::empty();
    set_4.extend([u16::MAX]);
    assert_eq!(set_4.last(), Ok(Some(u16::MAX)));
    let Ok(mut set_5) = IntSet::<u16>::empty();
    set_5.extend([5, 7, 8]);
    assert_eq!(set_5.last(), Ok(Some(8)));
    let Ok(mut set_6) = IntSet::<u16>::empty();
    set_6.extend([100, 1000, 10000]);
    assert_eq!(set_6.last(), Ok(Some(10000)));
    let Ok(()) = set_6.invert();
    assert_eq!(set_6.last(), Ok(Some(u16::MAX)));
    let Ok(()) = set_6.remove_range(u16::MAX.saturating_sub(10)..=u16::MAX);
    assert_eq!(set_6.last(), Ok(Some(u16::MAX.saturating_sub(11))));

    Ok(())
  }

  #[test]
  fn clear() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<u32>::empty();
    let Ok(_before) = set.insert(13);
    let Ok(_before) = set.insert(800);
    let Ok(mut set_inverted) = IntSet::<u32>::empty();
    let Ok(_before) = set_inverted.insert(13);
    let Ok(_before) = set_inverted.insert(800);
    let Ok(()) = set_inverted.invert();
    let Ok(()) = set.clear();
    assert_eq!(set.is_empty(), Ok(Emptiness::Empty));
    let Ok(()) = set_inverted.clear();
    assert_eq!(set_inverted.is_empty(), Ok(Emptiness::Empty));

    Ok(())
  }

  #[allow(deprecated, reason = "the test compares hashes from a fixed SipHasher")]
  fn hash<T>(set: &IntSet<T>) -> std::result::Result<u64, console_core_never::Never>
  where
    T: Domain,
  {
    use std::hash::Hasher;
    let mut h = std::hash::SipHasher::new();
    set.hash(&mut h);
    Ok(h.finish())
  }

  #[test]
  fn equal_and_hash() -> Result<(), crate::TestFailure> {
    let Ok(mut inc1) = IntSet::<u32>::empty();
    let Ok(_before) = inc1.insert(14);
    let Ok(_before) = inc1.insert(670);
    let Ok(mut inc2) = IntSet::<u32>::empty();
    let Ok(_before) = inc2.insert(670);
    let Ok(_before) = inc2.insert(14);
    let mut inc3 = inc1.clone();
    let Ok(_before) = inc3.insert(5);
    let mut exc = inc1.clone();
    let Ok(()) = exc.invert();
    assert_eq!(inc1, inc2);
    assert_ne!(inc1, inc3);
    assert_ne!(inc1, exc);
    let set = HashSet::from([inc1.clone(), inc3.clone(), exc.clone()]);
    assert!(set.contains(&inc1));
    assert!(set.contains(&inc3));
    assert!(set.contains(&exc));
    assert_ne!(hash(&inc1), hash(&exc));
    assert_eq!(hash(&inc1), hash(&inc2));

    Ok(())
  }

  #[test]
  fn equal_and_hash_mixed_membership_types() -> Result<(), crate::TestFailure> {
    let Ok(mut inverted_all) = IntSet::<TwoParts>::all();
    let Ok(mut all) = IntSet::<TwoParts>::empty();
    let Ok(values) = TwoParts::ordered_values();

    for v in values {
      let Ok(fitted) = console_core_number_conversion::fitted::<_, u16>(v);
      let Ok(_before) = all.insert(TwoParts(fitted));
    }

    assert_eq!(inverted_all, all);
    assert_eq!(hash(&all), hash(&inverted_all));
    let Ok(_before) = inverted_all.remove(TwoParts(5));
    assert_ne!(inverted_all, all);
    let Ok(_before) = all.remove(TwoParts(5));
    assert_eq!(inverted_all, all);
    assert_eq!(hash(&all), hash(&inverted_all));

    Ok(())
  }

  #[test]
  fn iter() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;
    let Ok(mut set) = IntSet::<u32>::empty();
    let Ok(_before) = set.insert(3);
    let Ok(_before) = set.insert(8);
    let Ok(_before) = set.insert(534);
    let Ok(_before) = set.insert(700);
    let Ok(_before) = set.insert(10000);
    let Ok(_before) = set.insert(10001);
    let Ok(_before) = set.insert(10002);
    let Ok(values) = set.iter();
    let v: Vec<u32> = values.collect();
    assert_eq!(v, vec![3, 8, 534, 700, 10000, 10001, 10002]);
    let v_2: Vec<u32> = {
      let Ok(members) = set.inclusive_iter();
      let queried_value = members.required()?;
      queried_value.collect()
    };
    assert_eq!(v_2, vec![3, 8, 534, 700, 10000, 10001, 10002]);

    Ok(())
  }

  #[test]
  fn iter_backwards() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<u32>::empty();
    let Ok(()) = set.insert_range(1..=6);

    {
      let Ok(mut it) = set.iter();
      assert_eq!(Some(1), it.next());
      assert_eq!(Some(6), it.next_back());
      assert_eq!(Some(5), it.next_back());
      assert_eq!(Some(2), it.next());
      assert_eq!(Some(3), it.next());
      assert_eq!(Some(4), it.next());
      assert_eq!(None, it.next());
      assert_eq!(None, it.next_back());
    }

    let Ok(mut set_2) = IntSet::<u8>::empty();
    let Ok(()) = set_2.invert();
    let Ok(()) = set_2.remove_range(10..=255);
    let Ok(_before) = set_2.remove(4);
    let Ok(_before) = set_2.remove(8);

    {
      let Ok(mut it) = set_2.iter();
      assert_eq!(Some(0), it.next());
      assert_eq!(Some(1), it.next());
      assert_eq!(Some(2), it.next());
      assert_eq!(Some(3), it.next());
      assert_eq!(Some(9), it.next_back());
      assert_eq!(Some(7), it.next_back());
      assert_eq!(Some(6), it.next_back());
      assert_eq!(Some(5), it.next_back());
      assert_eq!(None, it.next_back());
      assert_eq!(None, it.next());
    }

    let Ok(mut set_3) = IntSet::<u8>::empty();
    let Ok(()) = set_3.invert();
    let Ok(()) = set_3.remove_range(10..=255);
    let Ok(_before) = set_3.remove(4);
    let Ok(_before) = set_3.remove(8);

    {
      let Ok(mut it) = set_3.iter();
      assert_eq!(Some(0), it.next());
      assert_eq!(Some(1), it.next());
      assert_eq!(Some(2), it.next());
      assert_eq!(Some(3), it.next());
      assert_eq!(Some(5), it.next());
      assert_eq!(Some(9), it.next_back());
      assert_eq!(Some(7), it.next_back());
      assert_eq!(Some(6), it.next_back());
      assert_eq!(None, it.next_back());
      assert_eq!(None, it.next());
    }

    Ok(())
  }

  #[test]
  fn exclusive_iter() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<u32>::all();
    let Ok(_before) = set.remove(3);
    let Ok(_before) = set.remove(7);
    let Ok(_before) = set.remove(8);
    let Ok(mut iter) = set.iter();
    assert_eq!(iter.next(), Some(0));
    assert_eq!(iter.next(), Some(1));
    assert_eq!(iter.next(), Some(2));
    assert_eq!(iter.next(), Some(4));
    assert_eq!(iter.next(), Some(5));
    assert_eq!(iter.next(), Some(6));
    assert_eq!(iter.next(), Some(9));
    assert_eq!(iter.next(), Some(10));
    let Ok(members) = set.inclusive_iter();
    assert!(members.is_none());
    let Ok(mut set_2) = IntSet::<u32>::all();
    let Ok(()) = set_2.remove_range(0..=200);
    let Ok(mut iter_2) = set_2.iter();
    assert_eq!(iter_2.next(), Some(201));
    let Ok(mut set_3) = IntSet::<u8>::all();
    let Ok(()) = set_3.remove_range(200..=255);
    let Ok(mut iter_3) = set_3.iter();
    assert_eq!(iter_3.next_back(), Some(199));

    Ok(())
  }

  #[test]
  fn iter_ranges_inclusive() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<u32>::empty();
    let Ok(ranges) = set.iter_ranges();
    let items: Vec<_> = ranges.collect();
    assert_eq!(items, vec![]);
    let Ok(()) = set.insert_range(200..=700);
    let Ok(_before) = set.insert(5);
    let Ok(ranges_2) = set.iter_ranges();
    let items_2: Vec<_> = ranges_2.collect();
    assert_eq!(items_2, vec![5..=5, 200..=700]);
    let Ok(mut set_2) = IntSet::<u32>::empty();
    let Ok(()) = set_2.insert_range(0..=0);
    let Ok(()) = set_2.insert_range(u32::MAX..=u32::MAX);
    let Ok(ranges_3) = set_2.iter_ranges();
    let items_3: Vec<_> = ranges_3.collect();
    assert_eq!(items_3, vec![0..=0, u32::MAX..=u32::MAX]);
    let Ok(mut set_3) = IntSet::<u32>::empty();
    let Ok(()) = set_3.insert_range(0..=5);
    let Ok(()) = set_3.insert_range(u32::MAX.saturating_sub(5)..=u32::MAX);
    let Ok(ranges_4) = set_3.iter_ranges();
    let items_4: Vec<_> = ranges_4.collect();
    assert_eq!(items_4, vec![0..=5, u32::MAX.saturating_sub(5)..=u32::MAX]);
    let mut inverted = set_3.clone();
    let Ok(()) = inverted.invert();
    let Ok(ranges_5) = set_3.iter_ranges();
    let Ok(excluded) = inverted.iter_excluded_ranges();
    assert_eq!(ranges_5.collect::<Vec<_>>(), excluded.collect::<Vec<_>>());

    Ok(())
  }

  #[test]
  fn iter_ranges_inclusive_discontinuous() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<EvenInts>::empty();
    let Ok(ranges) = set.iter_ranges();
    let items: Vec<_> = ranges.collect();
    assert_eq!(items, vec![]);
    let Ok(()) = set.insert_range(EvenInts(4)..=EvenInts(12));
    let Ok(_before) = set.insert(EvenInts(16));
    let Ok(ranges_2) = set.iter_ranges();
    let items_2: Vec<_> = ranges_2.collect();
    assert_eq!(
      items_2,
      vec![EvenInts(4)..=EvenInts(12), EvenInts(16)..=EvenInts(16)]
    );

    let mut inverted = set.clone();
    let Ok(()) = inverted.invert();
    let Ok(ranges_3) = set.iter_ranges();
    let Ok(excluded) = inverted.iter_excluded_ranges();
    assert_eq!(ranges_3.collect::<Vec<_>>(), excluded.collect::<Vec<_>>());

    Ok(())
  }

  #[test]
  fn iter_ranges_exclusive() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<u32>::all();
    let Ok(()) = set.remove_range(200..=700);
    let Ok(_before) = set.remove(5);
    let Ok(ranges) = set.iter_ranges();
    let items: Vec<_> = ranges.collect();
    assert_eq!(items, vec![0..=4, 6..=199, 701..=u32::MAX]);
    let Ok(mut set_2) = IntSet::<u32>::all();
    let Ok(()) = set_2.remove_range(0..=700);
    let Ok(ranges_7) = set_2.iter_ranges();
    let items_2: Vec<_> = ranges_7.collect();
    assert_eq!(items_2, vec![701..=u32::MAX]);
    let mut inverted = set_2.clone();
    let Ok(()) = inverted.invert();
    let Ok(ranges_8) = set_2.iter_ranges();
    let Ok(excluded) = inverted.iter_excluded_ranges();
    assert_eq!(ranges_8.collect::<Vec<_>>(), excluded.collect::<Vec<_>>());

    let Ok(mut set_3) = IntSet::<u32>::all();
    let Ok(()) = set_3.remove_range(u32::MAX.saturating_sub(10)..=u32::MAX);
    let Ok(ranges_2) = set_3.iter_ranges();
    let items_3: Vec<_> = ranges_2.collect();
    assert_eq!(items_3, vec![0..=u32::MAX.saturating_sub(11)]);
    let Ok(mut set_4) = IntSet::<u16>::all();
    let Ok(()) = set_4.remove_range(0..=u16::MAX);
    let Ok(ranges_3) = set_4.iter_ranges();
    let items_4: Vec<_> = ranges_3.collect();
    assert_eq!(items_4, vec![]);
    let Ok(mut set_5) = IntSet::<u16>::all();
    let Ok(()) = set_5.remove_range(0..=u16::MAX.saturating_sub(1));
    let Ok(ranges_4) = set_5.iter_ranges();
    let items_5: Vec<_> = ranges_4.collect();
    assert_eq!(items_5, vec![u16::MAX..=u16::MAX]);
    let Ok(mut set_6) = IntSet::<u16>::all();
    let Ok(()) = set_6.remove_range(1..=u16::MAX);
    let Ok(ranges_5) = set_6.iter_ranges();
    let items_6: Vec<_> = ranges_5.collect();
    assert_eq!(items_6, vec![0..=0]);
    let Ok(set_7) = IntSet::<u32>::all();
    let Ok(ranges_6) = set_7.iter_ranges();
    let items_7: Vec<_> = ranges_6.collect();
    assert_eq!(items_7, vec![0..=u32::MAX]);

    Ok(())
  }

  #[test]
  fn iter_ranges_exclusive_discontinuous() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<EvenInts>::all();
    let Ok(()) = set.remove_range(EvenInts(0)..=EvenInts(8));
    let Ok(()) = set.remove_range(EvenInts(16)..=EvenInts(u16::MAX.saturating_sub(1)));
    let Ok(ranges) = set.iter_ranges();
    let items: Vec<_> = ranges.collect();
    assert_eq!(items, vec![EvenInts(10)..=EvenInts(14),]);
    let Ok(mut set_2) = IntSet::<TwoParts>::all();
    let Ok(()) = set_2.remove_range(TwoParts(11)..=TwoParts(13));
    let Ok(ranges_6) = set_2.iter_ranges();
    let items_2: Vec<_> = ranges_6.collect();
    assert_eq!(
      items_2,
      vec![TwoParts(2)..=TwoParts(10), TwoParts(14)..=TwoParts(16),]
    );

    let mut inverted = set_2.clone();
    let Ok(()) = inverted.invert();
    let Ok(ranges_7) = set_2.iter_ranges();
    let Ok(excluded) = inverted.iter_excluded_ranges();
    assert_eq!(ranges_7.collect::<Vec<_>>(), excluded.collect::<Vec<_>>());

    let Ok(mut set_3) = IntSet::<TwoParts>::all();
    let Ok(()) = set_3.remove_range(TwoParts(2)..=TwoParts(16));
    let Ok(ranges_2) = set_3.iter_ranges();
    let items_3: Vec<_> = ranges_2.collect();
    assert_eq!(items_3, vec![]);
    let Ok(mut set_4) = IntSet::<TwoParts>::all();
    let Ok(()) = set_4.remove_range(TwoParts(2)..=TwoParts(5));
    let Ok(ranges_3) = set_4.iter_ranges();
    let items_4: Vec<_> = ranges_3.collect();
    assert_eq!(items_4, vec![TwoParts(8)..=TwoParts(16),]);
    let Ok(mut set_5) = IntSet::<TwoParts>::all();
    let Ok(()) = set_5.remove_range(TwoParts(6)..=TwoParts(16));
    let Ok(ranges_4) = set_5.iter_ranges();
    let items_5: Vec<_> = ranges_4.collect();
    assert_eq!(items_5, vec![TwoParts(2)..=TwoParts(5),]);
    let Ok(set_6) = IntSet::<TwoPartsBounds>::all();
    let Ok(ranges_5) = set_6.iter_ranges();
    let items_6: Vec<_> = ranges_5.collect();
    assert_eq!(items_6, vec![TwoPartsBounds(0)..=TwoPartsBounds(u32::MAX),]);

    Ok(())
  }

  #[test]
  fn iter_range() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<u32>::empty();
    let Ok(after) = set.iter_after(0);
    assert_eq!(after.count(), 0);
    set.extend([5, 7, 10, 1250, 1300, 3001]);
    let Ok(after_2) = set.iter_after(0);
    assert_eq!(
      after_2.collect::<Vec<u32>>(),
      vec![5, 7, 10, 1250, 1300, 3001]
    );
    let Ok(values) = set.range(0..);
    assert_eq!(
      values.collect::<Vec<u32>>(),
      vec![5, 7, 10, 1250, 1300, 3001]
    );

    let Ok(after_3) = set.iter_after(5);
    assert_eq!(after_3.collect::<Vec<u32>>(), vec![7, 10, 1250, 1300, 3001]);
    let Ok(values_2) = set.range(5..);
    assert_eq!(
      values_2.collect::<Vec<u32>>(),
      vec![5, 7, 10, 1250, 1300, 3001]
    );

    let Ok(after_4) = set.iter_after(700);
    assert_eq!(after_4.collect::<Vec<u32>>(), vec![1250, 1300, 3001]);
    let Ok(values_3) = set.range(700..);
    assert_eq!(values_3.collect::<Vec<u32>>(), vec![1250, 1300, 3001]);

    Ok(())
  }

  #[test]
  fn iter_after_from_exclusive() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<u32>::empty();
    set.extend([5, 7, 10, 1250, 1300, 3001]);
    let Ok(()) = set.invert();
    let Ok(after) = set.iter_after(3);
    assert_eq!(after.take(5).collect::<Vec<u32>>(), vec![4, 6, 8, 9, 11]);
    let Ok(values) = set.range(3..);
    assert_eq!(values.take(5).collect::<Vec<u32>>(), vec![3, 4, 6, 8, 9]);

    let Ok(after_2) = set.iter_after(0);
    assert_eq!(after_2.take(5).collect::<Vec<u32>>(), vec![1, 2, 3, 4, 6]);
    let Ok(values_2) = set.range(0..);
    assert_eq!(values_2.take(5).collect::<Vec<u32>>(), vec![0, 1, 2, 3, 4]);

    let Ok(after_3) = set.iter_after(u32::MAX.saturating_sub(1));
    assert_eq!(after_3.take(1).collect::<Vec<u32>>(), vec![u32::MAX]);
    let Ok(values_3) = set.range(u32::MAX.saturating_sub(1)..);
    assert_eq!(
      values_3.take(2).collect::<Vec<u32>>(),
      vec![u32::MAX.saturating_sub(1), u32::MAX]
    );

    let Ok(after_4) = set.iter_after(u32::MAX);
    assert_eq!(after_4.take(1).count(), 0);
    let Ok(_before) = set.remove(u32::MAX);
    let Ok(values_4) = set.range(u32::MAX..);
    assert_eq!(values_4.take(1).count(), 0);
    let Ok(after_5) = set.iter_after(u32::MAX.saturating_sub(1));
    assert_eq!(after_5.take(1).count(), 0);

    Ok(())
  }

  #[test]
  fn iter_after_discontinuous() -> Result<(), crate::TestFailure> {
    let mut set = IntSet::from([EvenInts(6), EvenInts(10)]);
    let Ok(()) = set.invert();
    let last = u16::MAX.saturating_sub(1);
    let cases: &[(u16, &[u16])] = &[
      (2, &[4, 8, 12, 14, 16]),
      (4, &[8, 12, 14, 16, 18]),
      (last, &[]),
      (last.saturating_sub(4), &[last.saturating_sub(2), last]),
    ];

    for (start, expected) in cases {
      let Ok(values) = set.iter_after(EvenInts(*start));
      assert!(values.take(5).eq(expected.iter().copied().map(EvenInts)));
    }

    let cases_2: &[(u16, &[u16])] = &[(2, &[2, 4, 8, 12, 14]), (last, &[last])];

    for (start, expected) in cases_2 {
      let Ok(values) = set.range(EvenInts(*start)..);
      assert!(values.take(5).eq(expected.iter().copied().map(EvenInts)));
    }

    let Ok(_removed) = set.remove(EvenInts(last));
    let Ok(after) = set.iter_after(EvenInts(last.saturating_sub(4)));
    let Ok(included) = set.range(EvenInts(last.saturating_sub(4))..);
    assert!(after.eq([EvenInts(last.saturating_sub(2))]));
    assert!(included.eq([
      EvenInts(last.saturating_sub(4)),
      EvenInts(last.saturating_sub(2))
    ]));

    Ok(())
  }

  #[test]
  #[allow(
    clippy::reversed_empty_ranges,
    reason = "empty ranges exercise range membership"
  )]
  fn range() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<u32>::empty();
    let Ok(through_five) = set.range(0..=5);
    assert_eq!(through_five.count(), 0);
    set.extend([5, 7, 10, 1250, 1300, 3001]);
    let Ok(values) = set.range(0..=5);
    assert_eq!(values.collect::<Vec<u32>>(), vec![5]);
    let Ok(from_five) = set.range(5..=11);
    assert_eq!(from_five.collect::<Vec<u32>>(), vec![5, 7, 10]);
    let Ok(below_ten) = set.range(5..10);
    assert_eq!(below_ten.collect::<Vec<u32>>(), vec![5, 7]);
    let Ok(before_ten) = set.range(..10);
    assert_eq!(before_ten.collect::<Vec<u32>>(), vec![5, 7]);
    let Ok(through_ten) = set.range(..=10);
    assert_eq!(through_ten.collect::<Vec<u32>>(), vec![5, 7, 10]);
    let Ok(from_six) = set.range(6..=11);
    assert_eq!(from_six.collect::<Vec<u32>>(), vec![7, 10]);
    let Ok(mut reversed) = set.range(7..6);
    assert!(reversed.next().is_none());
    let Ok(mut empty) = set.range(7..7);
    assert!(empty.next().is_none());
    let Ok(only_seven) = set.range(7..=7);
    assert_eq!(only_seven.collect::<Vec<u32>>(), vec![7]);
    let Ok(mut reversed_to_zero) = set.range(5..=0);
    assert!(reversed_to_zero.next().is_none());

    Ok(())
  }

  #[test]
  fn from_iterator() -> Result<(), crate::TestFailure> {
    let s: IntSet<u32> = [3, 8, 12, 589].into_iter().collect();
    let Ok(mut expected) = IntSet::<u32>::empty();
    let Ok(_before) = expected.insert(3);
    let Ok(_before) = expected.insert(8);
    let Ok(_before) = expected.insert(12);
    let Ok(_before) = expected.insert(589);
    assert_eq!(s, expected);

    Ok(())
  }

  #[test]
  fn from_int_set_iterator() -> Result<(), crate::TestFailure> {
    let s1: IntSet<u32> = [3, 8, 12, 589].into_iter().collect();
    let Ok(values) = s1.iter();
    let s2: IntSet<u32> = values.collect();
    assert_eq!(s1, s2);

    Ok(())
  }

  #[test]
  fn extend() -> Result<(), crate::TestFailure> {
    let Ok(mut s) = IntSet::<u32>::empty();
    s.extend([3, 12]);
    s.extend([8, 10, 589]);
    let Ok(mut expected) = IntSet::<u32>::empty();
    let Ok(_before) = expected.insert(3);
    let Ok(_before) = expected.insert(8);
    let Ok(_before) = expected.insert(10);
    let Ok(_before) = expected.insert(12);
    let Ok(_before) = expected.insert(589);
    assert_eq!(s, expected);

    Ok(())
  }

  #[test]
  fn extend_on_inverted() -> Result<(), crate::TestFailure> {
    let Ok(mut s) = IntSet::<u32>::all();

    for i in 10..=20 {
      let Ok(_before) = s.remove(i);
    }

    s.extend([12, 17, 18]);
    assert_eq!(s.contains(11), Ok(Presence::Absent));
    assert_eq!(s.contains(12), Ok(Presence::Present));
    assert_eq!(s.contains(13), Ok(Presence::Absent));
    assert_eq!(s.contains(16), Ok(Presence::Absent));
    assert_eq!(s.contains(17), Ok(Presence::Present));
    assert_eq!(s.contains(18), Ok(Presence::Present));
    assert_eq!(s.contains(19), Ok(Presence::Absent));
    assert_eq!(s.contains(100), Ok(Presence::Present));

    Ok(())
  }

  #[test]
  fn remove_all() -> Result<(), crate::TestFailure> {
    let Ok(mut empty) = IntSet::<u32>::empty();
    let Ok(mut all) = IntSet::<u32>::all();
    empty.extend([1, 2, 3, 4]);
    let Ok(()) = empty.remove_all([2, 3]);
    let Ok(()) = all.remove_all([2, 3]);
    assert_eq!(empty.contains(1), Ok(Presence::Present));
    assert_eq!(empty.contains(2), Ok(Presence::Absent));
    assert_eq!(empty.contains(3), Ok(Presence::Absent));
    assert_eq!(empty.contains(4), Ok(Presence::Present));
    assert_eq!(all.contains(1), Ok(Presence::Present));
    assert_eq!(all.contains(2), Ok(Presence::Absent));
    assert_eq!(all.contains(3), Ok(Presence::Absent));
    assert_eq!(all.contains(4), Ok(Presence::Present));

    Ok(())
  }

  #[test]
  fn remove_range() -> Result<(), crate::TestFailure> {
    let Ok(mut empty) = IntSet::<u32>::empty();
    let Ok(mut all) = IntSet::<u32>::all();
    empty.extend([1, 2, 3, 4]);
    let Ok(()) = empty.remove_range(2..=3);
    let Ok(()) = all.remove_range(2..=3);
    assert_eq!(empty.contains(1), Ok(Presence::Present));
    assert_eq!(empty.contains(2), Ok(Presence::Absent));
    assert_eq!(empty.contains(3), Ok(Presence::Absent));
    assert_eq!(empty.contains(4), Ok(Presence::Present));
    assert_eq!(all.contains(1), Ok(Presence::Present));
    assert_eq!(all.contains(2), Ok(Presence::Absent));
    assert_eq!(all.contains(3), Ok(Presence::Absent));
    assert_eq!(all.contains(4), Ok(Presence::Present));

    Ok(())
  }

  #[test]
  fn insert_remove_range_boundary() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<u32>::empty();
    let Ok(()) = set.remove_range(u32::MAX.saturating_sub(10)..=u32::MAX);
    assert_eq!(set.contains(u32::MAX), Ok(Presence::Absent));
    let Ok(()) = set.insert_range(u32::MAX.saturating_sub(10)..=u32::MAX);
    assert_eq!(set.contains(u32::MAX), Ok(Presence::Present));
    let Ok(()) = set.remove_range(u32::MAX.saturating_sub(10)..=u32::MAX);
    assert_eq!(set.contains(u32::MAX), Ok(Presence::Absent));
    let Ok(()) = set.remove_range(0..=10);
    assert_eq!(set.contains(0), Ok(Presence::Absent));
    let Ok(()) = set.insert_range(0..=10);
    assert_eq!(set.contains(0), Ok(Presence::Present));
    let Ok(()) = set.remove_range(0..=10);
    assert_eq!(set.contains(0), Ok(Presence::Absent));

    Ok(())
  }

  #[test]
  fn insert_remove_range_exclusive_boundary() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<u32>::all();
    let Ok(()) = set.remove_range(u32::MAX.saturating_sub(10)..=u32::MAX);
    assert_eq!(set.contains(u32::MAX), Ok(Presence::Absent));
    let Ok(()) = set.insert_range(u32::MAX.saturating_sub(10)..=u32::MAX);
    assert_eq!(set.contains(u32::MAX), Ok(Presence::Present));
    let Ok(()) = set.remove_range(u32::MAX.saturating_sub(10)..=u32::MAX);
    assert_eq!(set.contains(u32::MAX), Ok(Presence::Absent));
    let Ok(()) = set.remove_range(0..=10);
    assert_eq!(set.contains(0), Ok(Presence::Absent));
    let Ok(()) = set.insert_range(0..=10);
    assert_eq!(set.contains(0), Ok(Presence::Present));
    let Ok(()) = set.remove_range(0..=10);
    assert_eq!(set.contains(0), Ok(Presence::Absent));

    Ok(())
  }

  #[derive(Clone, Copy, Debug)]
  enum SetCase {
    Empty,
    OnlyX,
    AllExceptX,
    All,
  }

  impl SetCase {
    const CASES: [Self; 4] = [Self::Empty, Self::OnlyX, Self::AllExceptX, Self::All];

    fn to_set(self, x: u32) -> Result<IntSet<u32>, console_core_never::Never> {
      let Ok(mut set) = IntSet::empty();

      match self {
        Self::Empty => {}
        Self::OnlyX => {
          let Ok(_inserted) = set.insert(x);
        }
        Self::AllExceptX => {
          let Ok(()) = set.invert();
          let Ok(_removed) = set.remove(x);
        }
        Self::All => {
          let Ok(()) = set.invert();
        }
      }

      Ok(set)
    }

    fn membership(self) -> Result<(Presence, Presence), console_core_never::Never> {
      Ok(match self {
        Self::Empty => (Presence::Absent, Presence::Absent),
        Self::OnlyX => (Presence::Present, Presence::Absent),
        Self::AllExceptX => (Presence::Absent, Presence::Present),
        Self::All => (Presence::Present, Presence::Present),
      })
    }
  }

  #[derive(Clone, Copy, Debug)]
  enum Operation {
    Union,
    Intersect,
    Subtract,
  }

  impl Operation {
    fn expected(self, left: Presence, right: Presence) -> Result<Presence, Never> {
      Ok(match (self, left, right) {
        (Self::Union, Presence::Present, Presence::Present | Presence::Absent)
        | (Self::Union, Presence::Absent, Presence::Present)
        | (Self::Intersect, Presence::Present, Presence::Present)
        | (Self::Subtract, Presence::Present, Presence::Absent) => Presence::Present,
        (Self::Union, Presence::Absent, Presence::Absent)
        | (Self::Intersect, Presence::Present, Presence::Absent)
        | (
          Self::Intersect | Self::Subtract,
          Presence::Absent,
          Presence::Present | Presence::Absent,
        )
        | (Self::Subtract, Presence::Present, Presence::Present) => Presence::Absent,
      })
    }
  }

  #[test]
  fn set_operations() {
    for first in SetCase::CASES {
      let Ok(left) = first.to_set(13);
      let Ok((left_x, left_other)) = first.membership();

      for second in SetCase::CASES {
        let Ok(right) = second.to_set(13);
        let Ok((right_x, right_other)) = second.membership();

        for operation in [Operation::Union, Operation::Intersect, Operation::Subtract] {
          let mut actual = left.clone();

          match operation {
            Operation::Union => {
              let Ok(()) = actual.union(&right);
            }
            Operation::Intersect => {
              let Ok(()) = actual.intersect(&right);
            }
            Operation::Subtract => {
              let Ok(()) = actual.subtract(&right);
            }
          }

          let Ok(expected_x) = operation.expected(left_x, right_x);
          let Ok(expected_other) = operation.expected(left_other, right_other);
          let expected = match (expected_x, expected_other) {
            (Presence::Absent, Presence::Absent) => SetCase::Empty,
            (Presence::Present, Presence::Absent) => SetCase::OnlyX,
            (Presence::Absent, Presence::Present) => SetCase::AllExceptX,
            (Presence::Present, Presence::Present) => SetCase::All,
          };
          let Ok(expected_set) = expected.to_set(13);
          assert!(actual == expected_set, "{first:?} {operation:?} {second:?}");
        }
      }
    }
  }

  #[test]
  fn inverted() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<u32>::empty();
    let Ok(_before) = set.insert(13);
    let Ok(_before) = set.insert(800);
    assert_eq!(set.contains(13), Ok(Presence::Present));
    assert_eq!(set.contains(800), Ok(Presence::Present));
    assert_eq!(set.len(), Ok(2));
    assert_eq!(set.membership(), Ok(Membership::Inclusive));
    let Ok(()) = set.invert();
    let Ok(length) = set.len();
    let Ok(fitted) = console_core_number_conversion::fitted::<_, u64>(u32::MAX);
    assert_eq!(length, fitted.saturating_sub(1));
    assert_eq!(set.contains(13), Ok(Presence::Absent));
    assert_eq!(set.contains(80), Ok(Presence::Present));
    assert_eq!(set.contains(800), Ok(Presence::Absent));
    assert_eq!(set.membership(), Ok(Membership::Exclusive));
    let Ok(_before) = set.remove(80);
    assert_eq!(set.contains(80), Ok(Presence::Absent));
    let Ok(_before) = set.insert(13);
    assert_eq!(set.contains(13), Ok(Presence::Present));
    let Ok(()) = set.invert();
    assert_eq!(set.contains(80), Ok(Presence::Present));
    assert_eq!(set.contains(800), Ok(Presence::Present));

    Ok(())
  }

  #[test]
  fn limited_domain_type() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<EvenInts>::empty();
    let Ok(_before) = set.insert(EvenInts(2));
    let Ok(_before) = set.insert(EvenInts(8));
    let Ok(_before) = set.insert(EvenInts(12));
    let Ok(()) = set.insert_range(EvenInts(20)..=EvenInts(34));
    let Ok(()) = set.remove_range(EvenInts(30)..=EvenInts(34));
    assert_eq!(set.contains(EvenInts(2)), Ok(Presence::Present));
    assert_eq!(set.contains(EvenInts(4)), Ok(Presence::Absent));
    assert_eq!(set.contains(EvenInts(18)), Ok(Presence::Absent));
    assert_eq!(set.contains(EvenInts(19)), Ok(Presence::Absent));
    assert_eq!(set.contains(EvenInts(20)), Ok(Presence::Present));
    assert_eq!(set.contains(EvenInts(21)), Ok(Presence::Absent));
    assert_eq!(set.contains(EvenInts(28)), Ok(Presence::Present));
    assert_eq!(set.contains(EvenInts(29)), Ok(Presence::Absent));
    assert_eq!(set.contains(EvenInts(30)), Ok(Presence::Absent));
    let Ok(values) = set.iter();
    let copy: IntSet<EvenInts> = values.collect();
    assert_eq!(set, copy);
    let Ok(()) = set.invert();
    assert_eq!(set.contains(EvenInts(2)), Ok(Presence::Absent));
    assert_eq!(set.contains(EvenInts(4)), Ok(Presence::Present));
    let Ok(values_2) = set.iter();
    let max = crate::test_support::Required::required(values_2.max())?;
    assert_eq!(max.0, u16::MAX.saturating_sub(1));

    {
      let Ok(mut it) = set.iter();
      assert_eq!(it.next(), Some(EvenInts(0)));
      assert_eq!(it.next(), Some(EvenInts(4)));
      assert_eq!(it.next(), Some(EvenInts(6)));
      assert_eq!(it.next(), Some(EvenInts(10)));
      assert_eq!(it.next(), Some(EvenInts(14)));
    }

    let Ok(()) = set.insert_range(EvenInts(6)..=EvenInts(10));

    {
      let Ok(mut it) = set.iter();
      assert_eq!(it.next(), Some(EvenInts(0)));
      assert_eq!(it.next(), Some(EvenInts(4)));
      assert_eq!(it.next(), Some(EvenInts(6)));
      assert_eq!(it.next(), Some(EvenInts(8)));
      assert_eq!(it.next(), Some(EvenInts(10)));
      assert_eq!(it.next(), Some(EvenInts(14)));
    }

    let Ok(()) = set.remove_range(EvenInts(6)..=EvenInts(10));

    {
      let Ok(mut it) = set.iter();
      assert_eq!(it.next(), Some(EvenInts(0)));
      assert_eq!(it.next(), Some(EvenInts(4)));
      assert_eq!(it.next(), Some(EvenInts(14)));
    }

    Ok(())
  }

  #[test]
  fn with_u16() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<u16>::empty();
    let Ok(_before) = set.insert(5);
    let Ok(_before) = set.insert(8);
    let Ok(_before) = set.insert(12);
    let Ok(()) = set.insert_range(200..=210);
    assert_eq!(set.contains(5), Ok(Presence::Present));
    assert_eq!(set.contains(6), Ok(Presence::Absent));
    assert_eq!(set.contains(199), Ok(Presence::Absent));
    assert_eq!(set.contains(200), Ok(Presence::Present));
    assert_eq!(set.contains(210), Ok(Presence::Present));
    assert_eq!(set.contains(211), Ok(Presence::Absent));
    let Ok(values) = set.iter();
    let copy: IntSet<u16> = values.collect();
    assert_eq!(set, copy);
    let Ok(()) = set.invert();
    assert_eq!(set.contains(5), Ok(Presence::Absent));
    assert_eq!(set.contains(6), Ok(Presence::Present));
    let Ok(values_2) = set.iter();
    let max = crate::test_support::Required::required(values_2.max())?;
    assert_eq!(max, u16::MAX);
    let Ok(mut it) = set.iter();
    assert_eq!(it.next(), Some(0));
    assert_eq!(it.next(), Some(1));
    assert_eq!(it.next(), Some(2));
    assert_eq!(it.next(), Some(3));
    assert_eq!(it.next(), Some(4));
    assert_eq!(it.next(), Some(6));

    Ok(())
  }

  #[cfg(feature = "font-types")]
  fn glyph16(id: u16) -> Result<GlyphId16, Never> {
    GlyphId16::new(id)
  }

  #[cfg(feature = "font-types")]
  fn glyph(id: u32) -> Result<GlyphId, Never> {
    GlyphId::new(id)
  }

  #[cfg(feature = "font-types")]
  fn tag(bytes: &[u8; 4]) -> Result<Tag, Never> {
    Tag::new(bytes)
  }

  #[test]
  #[cfg(feature = "font-types")]
  fn with_glyph_id_16() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<font_types::GlyphId16>::empty();

    for id in [5, 8, 12] {
      let Ok(glyph) = glyph16(id);
      let Ok(_before) = set.insert(glyph);
    }

    let Ok(first) = glyph16(200);
    let Ok(last) = glyph16(210);
    let Ok(()) = set.insert_range(first..=last);

    for (id, expected) in [
      (5, Presence::Present),
      (6, Presence::Absent),
      (199, Presence::Absent),
      (200, Presence::Present),
      (210, Presence::Present),
      (211, Presence::Absent),
    ] {
      let Ok(glyph) = glyph16(id);

      assert_eq!(set.contains(glyph), Ok(expected), "{id}");
    }

    let Ok(values) = set.iter();
    let copy: IntSet<GlyphId16> = values.collect();

    assert_eq!(set, copy);

    let Ok(()) = set.invert();

    for (id, expected) in [(5, Presence::Absent), (6, Presence::Present)] {
      let Ok(glyph) = glyph16(id);

      assert_eq!(set.contains(glyph), Ok(expected), "{id}");
    }

    let Ok(values_2) = set.iter();
    let max = crate::test_support::Required::required(values_2.max())?;

    assert_eq!(Ok(max), glyph16(u16::MAX));

    let Ok(values_3) = set.iter();
    let expected: Vec<GlyphId16> = [0, 1, 2, 3, 4, 6]
      .into_iter()
      .map(|id| {
        let Ok(glyph) = glyph16(id);

        glyph
      })
      .collect();

    assert_eq!(values_3.take(6).collect::<Vec<_>>(), expected);

    Ok(())
  }

  #[test]
  #[cfg(feature = "font-types")]
  fn with_glyph_id() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<font_types::GlyphId>::empty();

    for id in [5, 8, 12] {
      let Ok(glyph) = glyph(id);
      let Ok(_before) = set.insert(glyph);
    }

    let Ok(first) = glyph(200);
    let Ok(last) = glyph(210);
    let Ok(()) = set.insert_range(first..=last);

    for (id, expected) in [
      (5, Presence::Present),
      (6, Presence::Absent),
      (199, Presence::Absent),
      (200, Presence::Present),
      (210, Presence::Present),
      (211, Presence::Absent),
    ] {
      let Ok(glyph) = glyph(id);

      assert_eq!(set.contains(glyph), Ok(expected), "{id}");
    }

    let Ok(values) = set.iter();
    let copy: IntSet<GlyphId> = values.collect();

    assert_eq!(set, copy);

    let Ok(()) = set.invert();

    for (id, expected) in [(5, Presence::Absent), (6, Presence::Present)] {
      let Ok(glyph) = glyph(id);

      assert_eq!(set.contains(glyph), Ok(expected), "{id}");
    }

    let Ok(values_2) = set.iter();
    let expected: Vec<GlyphId> = [0, 1, 2, 3, 4, 6]
      .into_iter()
      .map(|id| {
        let Ok(glyph) = glyph(id);

        glyph
      })
      .collect();

    assert_eq!(values_2.take(6).collect::<Vec<_>>(), expected);

    Ok(())
  }

  #[test]
  #[cfg(feature = "font-types")]
  fn with_tag() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<Tag>::empty();

    for bytes in [b"GSUB", b"CFF ", b"OS/2"] {
      let Ok(tag) = tag(bytes);
      let Ok(_before) = set.insert(tag);
    }

    for (bytes, expected) in [
      (b"GSUB", Presence::Present),
      (b"GSU ", Presence::Absent),
      (b"CFF ", Presence::Present),
      (b"OS/2", Presence::Present),
    ] {
      let Ok(tag) = tag(bytes);

      assert_eq!(set.contains(tag), Ok(expected));
    }

    let Ok(values) = set.iter();
    let copy: IntSet<Tag> = values.collect();

    assert_eq!(set, copy);

    let Ok(()) = set.invert();

    for (bytes, expected) in [
      (b"GSUB", Presence::Absent),
      (b"GSU ", Presence::Present),
      (b"CFF ", Presence::Absent),
      (b"OS/2", Presence::Absent),
    ] {
      let Ok(tag) = tag(bytes);

      assert_eq!(set.contains(tag), Ok(expected));
    }

    Ok(())
  }

  #[test]
  fn intersects_range() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<u32>::empty();
    assert_eq!(set.intersects_range(0..=0), Ok(Overlap::Disjoint));
    assert_eq!(set.intersects_range(0..=100), Ok(Overlap::Disjoint));
    assert_eq!(set.intersects_range(0..=u32::MAX), Ok(Overlap::Disjoint));
    assert_eq!(
      set.intersects_range(u32::MAX..=u32::MAX),
      Ok(Overlap::Disjoint)
    );
    let Ok(_before) = set.insert(1234);
    assert_eq!(set.intersects_range(0..=1233), Ok(Overlap::Disjoint));
    assert_eq!(set.intersects_range(1235..=1240), Ok(Overlap::Disjoint));
    assert_eq!(set.intersects_range(1234..=1234), Ok(Overlap::Intersecting));
    assert_eq!(set.intersects_range(1230..=1240), Ok(Overlap::Intersecting));
    assert_eq!(set.intersects_range(0..=1234), Ok(Overlap::Intersecting));
    assert_eq!(
      set.intersects_range(1234..=u32::MAX),
      Ok(Overlap::Intersecting)
    );
    let Ok(_before) = set.insert(0);
    assert_eq!(set.intersects_range(0..=0), Ok(Overlap::Intersecting));
    assert_eq!(set.intersects_range(1..=1), Ok(Overlap::Disjoint));

    Ok(())
  }

  #[test]
  fn intersects_set() -> Result<(), crate::TestFailure> {
    fn check_intersection(
      [left, right]: [&IntSet<u32>; 2],
      expected: Overlap,
    ) -> Result<(), console_core_never::Never> {
      assert_eq!(left.intersects_set(right), Ok(expected));
      assert_eq!(right.intersects_set(left), Ok(expected));

      Ok(())
    }

    let Ok(nothing) = IntSet::<u32>::empty();
    let Ok(also_nothing) = IntSet::<u32>::empty();

    assert_eq!(nothing.intersects_set(&also_nothing), Ok(Overlap::Disjoint));

    let Ok(empty) = IntSet::<u32>::empty();
    let a = IntSet::from([1u32, 5, 6, 7, 8, 12]);
    let b = IntSet::from([2u32, 13]);
    let c = IntSet::from([8u32, 14]);
    let Ok(mut d) = IntSet::all();
    let Ok(()) = d.remove_range(0u32..=13);
    let Ok(mut e) = IntSet::all();
    let Ok(()) = e.remove_range(0u32..=100);
    let Ok(()) = check_intersection([&a, &b], Overlap::Disjoint);
    let Ok(()) = check_intersection([&a, &c], Overlap::Intersecting);
    let Ok(()) = check_intersection([&a, &d], Overlap::Disjoint);
    let Ok(()) = check_intersection([&b, &c], Overlap::Disjoint);
    let Ok(()) = check_intersection([&b, &d], Overlap::Disjoint);
    let Ok(()) = check_intersection([&b, &e], Overlap::Disjoint);
    let Ok(()) = check_intersection([&c, &d], Overlap::Intersecting);
    let Ok(()) = check_intersection([&c, &e], Overlap::Disjoint);
    let Ok(()) = check_intersection([&d, &e], Overlap::Intersecting);
    let Ok(()) = check_intersection([&a, &empty], Overlap::Disjoint);
    let Ok(()) = check_intersection([&b, &empty], Overlap::Disjoint);
    let Ok(()) = check_intersection([&c, &empty], Overlap::Disjoint);
    let Ok(()) = check_intersection([&d, &empty], Overlap::Disjoint);
    let Ok(()) = check_intersection([&e, &empty], Overlap::Disjoint);

    Ok(())
  }

  #[test]
  fn intersects_range_discontinuous() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<EvenInts>::empty();
    assert_eq!(
      set.intersects_range(EvenInts(0)..=EvenInts(0)),
      Ok(Overlap::Disjoint)
    );
    assert_eq!(
      set.intersects_range(EvenInts(0)..=EvenInts(100)),
      Ok(Overlap::Disjoint)
    );
    assert_eq!(
      set.intersects_range(EvenInts(0)..=EvenInts(u16::MAX.saturating_sub(1))),
      Ok(Overlap::Disjoint)
    );
    assert_eq!(
      set.intersects_range(
        EvenInts(u16::MAX.saturating_sub(1))..=EvenInts(u16::MAX.saturating_sub(1))
      ),
      Ok(Overlap::Disjoint)
    );
    let Ok(_before) = set.insert(EvenInts(1234));
    assert_eq!(
      set.intersects_range(EvenInts(0)..=EvenInts(1232)),
      Ok(Overlap::Disjoint)
    );
    assert_eq!(
      set.intersects_range(EvenInts(1236)..=EvenInts(1240)),
      Ok(Overlap::Disjoint)
    );
    assert_eq!(
      set.intersects_range(EvenInts(1234)..=EvenInts(1234)),
      Ok(Overlap::Intersecting)
    );
    assert_eq!(
      set.intersects_range(EvenInts(1230)..=EvenInts(1240)),
      Ok(Overlap::Intersecting)
    );
    assert_eq!(
      set.intersects_range(EvenInts(0)..=EvenInts(1234)),
      Ok(Overlap::Intersecting)
    );
    assert_eq!(
      set.intersects_range(EvenInts(1234)..=EvenInts(u16::MAX.saturating_sub(1))),
      Ok(Overlap::Intersecting)
    );
    let Ok(_before) = set.insert(EvenInts(0));
    assert_eq!(
      set.intersects_range(EvenInts(0)..=EvenInts(0)),
      Ok(Overlap::Intersecting)
    );
    assert_eq!(
      set.intersects_range(EvenInts(2)..=EvenInts(2)),
      Ok(Overlap::Disjoint)
    );

    Ok(())
  }

  #[test]
  fn intersects_range_exclusive() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = IntSet::<u32>::all();
    assert_eq!(set.intersects_range(0..=0), Ok(Overlap::Intersecting));
    assert_eq!(set.intersects_range(0..=100), Ok(Overlap::Intersecting));
    assert_eq!(
      set.intersects_range(0..=u32::MAX),
      Ok(Overlap::Intersecting)
    );
    assert_eq!(
      set.intersects_range(u32::MAX..=u32::MAX),
      Ok(Overlap::Intersecting)
    );
    let Ok(_before) = set.remove(1234);
    assert_eq!(set.intersects_range(0..=1233), Ok(Overlap::Intersecting));
    assert_eq!(set.intersects_range(1235..=1240), Ok(Overlap::Intersecting));
    assert_eq!(set.intersects_range(1234..=1234), Ok(Overlap::Disjoint));
    assert_eq!(set.intersects_range(1230..=1240), Ok(Overlap::Intersecting));
    assert_eq!(set.intersects_range(0..=1234), Ok(Overlap::Intersecting));
    assert_eq!(
      set.intersects_range(1234..=u32::MAX),
      Ok(Overlap::Intersecting)
    );
    let Ok(_before) = set.remove(0);
    assert_eq!(set.intersects_range(0..=0), Ok(Overlap::Disjoint));
    assert_eq!(set.intersects_range(1..=1), Ok(Overlap::Intersecting));
    let Ok(()) = set.remove_range(5000..=5200);
    assert_eq!(set.intersects_range(5000..=5200), Ok(Overlap::Disjoint));
    assert_eq!(set.intersects_range(5100..=5150), Ok(Overlap::Disjoint));
    assert_eq!(set.intersects_range(4999..=5200), Ok(Overlap::Intersecting));
    assert_eq!(set.intersects_range(5000..=5201), Ok(Overlap::Intersecting));

    Ok(())
  }

  #[test]
  fn intersects_range_exclusive_discontinuous() -> Result<(), crate::TestFailure> {
    let check = |set: &IntSet<EvenInts>, cases: &[(u16, u16, Overlap)]| {
      for &(first, last, expected) in cases {
        assert_eq!(
          set.intersects_range(EvenInts(first)..=EvenInts(last)),
          Ok(expected),
          "range {first} through {last}"
        );
      }
    };
    let last_even = u16::MAX.saturating_sub(1);
    let Ok(mut set) = IntSet::<EvenInts>::all();

    check(
      &set,
      &[
        (0, 0, Overlap::Intersecting),
        (0, 100, Overlap::Intersecting),
        (0, last_even, Overlap::Intersecting),
        (last_even, last_even, Overlap::Intersecting),
      ],
    );
    let Ok(_before) = set.remove(EvenInts(1234));
    check(
      &set,
      &[
        (0, 1232, Overlap::Intersecting),
        (1236, 1240, Overlap::Intersecting),
        (1234, 1234, Overlap::Disjoint),
        (1230, 1240, Overlap::Intersecting),
        (0, 1234, Overlap::Intersecting),
        (1234, last_even, Overlap::Intersecting),
      ],
    );
    let Ok(_before) = set.remove(EvenInts(0));
    check(
      &set,
      &[(0, 0, Overlap::Disjoint), (2, 2, Overlap::Intersecting)],
    );
    let Ok(()) = set.remove_range(EvenInts(5000)..=EvenInts(5200));
    check(
      &set,
      &[
        (5000, 5200, Overlap::Disjoint),
        (5100, 5150, Overlap::Disjoint),
        (4998, 5200, Overlap::Intersecting),
        (5000, 5202, Overlap::Intersecting),
      ],
    );

    Ok(())
  }

  #[test]
  fn length() -> Result<(), crate::TestFailure> {
    let Ok(mut s) = IntSet::<u32>::empty();
    assert_eq!(s.len(), Ok(0));
    let Ok(_before) = s.insert(5);
    let Ok(_before) = s.insert(5);
    let Ok(_before) = s.insert(100);
    assert_eq!(s.len(), Ok(2));
    let Ok(()) = s.invert();
    let Ok(length) = s.len();
    let Ok(fitted) = console_core_number_conversion::fitted::<_, u64>(u32::MAX.saturating_sub(1));
    assert_eq!(length, fitted);
    let Ok(all) = IntSet::<u32>::all();
    let Ok(length_2) = all.len();
    let Ok(fitted_2) = console_core_number_conversion::fitted::<_, u64>(u32::MAX);
    assert_eq!(length_2, fitted_2.saturating_add(1));
    let Ok(mut s_2) = IntSet::<TwoParts>::all();
    assert_eq!(s_2.len(), Ok(13));
    let Ok(member) = TwoParts::from_u32(InDomain(5));
    let Ok(_before) = s_2.remove(member);
    assert_eq!(s_2.len(), Ok(12));
    let Ok(values) = TwoParts::ordered_values();

    for v in values {
      let Ok(member_2) = TwoParts::from_u32(InDomain(v));
      let Ok(_before) = s_2.remove(member_2);
    }

    assert_eq!(s_2.len(), Ok(0));

    Ok(())
  }

  #[test]
  fn ordering() -> Result<(), crate::TestFailure> {
    let assert_order = |left: &[u16], right: &[u16], expected| {
      assert_eq!(
        IntSet::from_iter(left.iter().copied()).cmp(&IntSet::from_iter(right.iter().copied())),
        expected,
        "{left:?}, {right:?}"
      );
    };

    assert_order(&[], &[], Ordering::Equal);
    assert_order(&[], &[0], Ordering::Less);
    assert_order(&[0u16], &[0], Ordering::Equal);
    assert_order(&[0u16, 1, 2], &[1, 2, 3], Ordering::Less);
    assert_order(&[0u16, 1, 4], &[1, 2, 3], Ordering::Less);
    assert_order(&[1u16, 2, 3], &[0, 2, 4], Ordering::Greater);
    assert_order(&[5u16, 4, 0], &[1, 2, 3], Ordering::Less);
    assert_order(&[1u16, 2, 3], &[1, 2, 3, 4], Ordering::Less);
    assert_order(&[2u16, 3, 4], &[1, 2, 3, 4, 5], Ordering::Greater);
    let Ok(all) = IntSet::<u16>::all();
    let mut all_but_0 = all.clone();
    let Ok(_before) = all_but_0.remove(0);
    let mut all_but_5 = all.clone();
    let Ok(_before) = all_but_5.remove(5);
    assert_eq!(all.cmp(&all), Ordering::Equal);
    assert_eq!(all.cmp(&all_but_0), Ordering::Less);
    assert_eq!(all_but_0.cmp(&all), Ordering::Greater);
    let Ok(mut a) = IntSet::<u16>::all();
    let Ok(()) = a.remove_range(0..=5);
    let Ok(()) = a.remove_range(221..=1693);
    let Ok(mut b) = IntSet::<u16>::all();
    let Ok(()) = b.remove_range(0..=1693);
    assert_eq!(a.cmp(&b), Ordering::Less);
    let Ok(mut inc_all_but_0) = IntSet::<u16>::empty();
    let Ok(()) = inc_all_but_0.insert_range(1..=u16::MAX);
    let Ok(mut inc_all_but_5) = IntSet::<u16>::empty();
    let Ok(()) = inc_all_but_5.insert_range(0..=4);
    let Ok(()) = inc_all_but_5.insert_range(6..=u16::MAX);
    assert_eq!(all.cmp(&all), Ordering::Equal);
    assert_eq!(all.cmp(&inc_all_but_0), Ordering::Less);
    assert_eq!(inc_all_but_0.cmp(&all), Ordering::Greater);
    assert_eq!(inc_all_but_5.cmp(&all_but_0), Ordering::Less);
    let Ok(mut middle_missing) = IntSet::<u16>::all();
    let Ok(()) = middle_missing.remove_range(8..=1160);
    let Ok(mut through_259) = IntSet::<u16>::empty();
    let Ok(()) = through_259.insert_range(0..=259);
    assert_eq!(middle_missing.cmp(&through_259), Ordering::Greater);
    let Ok(mut only_first_eight) = IntSet::<u16>::all();
    let Ok(()) = only_first_eight.remove_range(8..=u16::MAX);
    assert_eq!(only_first_eight.cmp(&through_259), Ordering::Less);

    Ok(())
  }

  #[cfg(feature = "serde")]
  fn roundtrip_json<T: Domain>(set: &IntSet<T>) -> Result<IntSet<T>, serde_json::Error> {
    let json = serde_json::to_vec(&set)?;
    serde_json::from_slice(&json)
  }

  #[test]
  #[cfg(feature = "serde")]
  fn simple_serde() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;
    let Ok(mut set) = IntSet::empty();
    let Ok(_before) = set.insert(0u32);
    let Ok(_before) = set.insert(u32::MAX);

    {
      let queried_value = roundtrip_json(&set).required()?;
      assert_eq!(queried_value, set);
    }

    Ok(())
  }

  #[test]
  #[cfg(feature = "serde")]
  fn serde_non_contiguous() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;
    let set = IntSet::<EvenInts>::from([
      EvenInts(2),
      EvenInts(166),
      EvenInts(u16::MAX.saturating_sub(1)),
    ]);

    {
      let queried_value = roundtrip_json(&set).required()?;
      assert_eq!(queried_value, set);
    }

    Ok(())
  }

  #[test]
  #[cfg(feature = "serde")]
  fn serde_non_contiguous_out_of_domain() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;
    let set = IntSet::from([1u16, 2, 3, 4, 5, 6, 7]);
    let bytes = serde_json::to_vec(&set).required()?;
    let failure = match serde_json::from_slice::<IntSet<EvenInts>>(&bytes) {
      Err(failure) => failure,
      Ok(_accepted) => return Err(crate::TestFailure::UnexpectedSuccess),
    };
    assert!(failure.to_string().contains("out of range for domain"));

    Ok(())
  }

  #[test]
  #[cfg(feature = "serde")]
  fn non_contiguous_inverted() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;
    let Ok(all) = IntSet::<u16>::all();
    let bytes = serde_json::to_vec(&all).required()?;
    let readback: IntSet<EvenInts> = serde_json::from_slice(&bytes).required()?;
    let Ok(readback_values) = readback.iter();
    let mut iter = readback_values.map(|v| v.0);
    let mut values = (&mut iter).take(5).collect::<Vec<_>>();
    values.extend(iter.rev().take(5));
    assert_eq!(values, [0, 2, 4, 6, 8, 65534, 65532, 65530, 65528, 65526]);
    Ok(())
  }

  #[test]
  #[cfg(feature = "serde")]
  fn serde_inverted() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;
    let Ok(mut set) = IntSet::all();
    let Ok(()) = set.remove_range(0u16..=420);
    let bytes = serde_json::to_string(&set).required()?;
    assert!(bytes.len() < 5000, "sanity check serialization");

    {
      let queried_value = roundtrip_json(&set).required()?;
      assert_eq!(queried_value, set);
    }

    Ok(())
  }

  #[test]
  #[cfg(feature = "serde")]
  fn serde_inverted_out_of_domain() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;
    let Ok(mut set) = IntSet::all();
    let Ok(()) = set.remove_range(0u16..=250);
    let bytes = serde_json::to_string(&set).required()?;
    let readback: IntSet<u8> = serde_json::from_str(&bytes).required()?;
    assert_eq!(readback.len(), Ok(5));
    let Ok(values) = readback.iter();
    assert_eq!(values.collect::<Vec<_>>(), [251, 252, 253, 254, 255]);

    Ok(())
  }

  #[test]
  #[cfg(feature = "serde")]
  #[cfg(feature = "font-types")]
  fn serde_out_of_domain() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;
    let set = IntSet::from([u32::MAX]);
    let json = serde_json::to_vec(&set).required()?;
    let failure = match serde_json::from_slice::<IntSet<GlyphId16>>(&json) {
      Err(failure) => failure,
      Ok(_accepted) => return Err(crate::TestFailure::UnexpectedSuccess),
    };
    assert!(failure.to_string().contains("out of range for domain"));

    Ok(())
  }
}

#[cfg(test)]
mod test_support;
#[cfg(test)]
use test_support::TestFailure;
