//! Ordered bitmap pages keep point insertion and lookup independent of density.
//!
//! A page is keyed by the first value it holds rather than by its number, so a
//! value's page is the value with its low nine bits cleared and a value inside
//! a page is the key with them set: a mask either way, and nothing to overflow.
//! Pages contain no empty entries, and querying a set changes no cache. Its
//! order, equality and hash depend only on members. A page's bitmap is also
//! the representation retained for compatibility with the former serde shape.

use alloc::collections::{BTreeMap, btree_map::Entry};
use core::cmp::Ordering;
use core::ops::RangeInclusive;

use console_core_never::Never;

use crate::{Emptiness, Overlap, Presence};

mod page;
pub(crate) use page::Mark;
use page::{OFFSET_MASK, Page};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct PageStart(u32);

impl PageStart {
  fn of(value: u32) -> Result<Self, Never> {
    Ok(Self(value & !OFFSET_MASK))
  }

  fn last(self) -> Result<u32, Never> {
    Ok(self.0 | OFFSET_MASK)
  }

  fn with(self, offset: u32) -> Result<u32, Never> {
    Ok(self.0 | offset)
  }

  fn values(self, page: &Page) -> Result<impl DoubleEndedIterator<Item = u32> + '_, Never> {
    let Ok(offsets) = page.iter();

    Ok(offsets.map(move |offset| {
      let Ok(value) = self.with(offset);

      value
    }))
  }
}

enum Cut {
  Intersection,
  Difference,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct U32Set {
  pages: BTreeMap<PageStart, Page>,
  length: u64,
}

impl U32Set {
  pub(crate) const EMPTY: Self = Self {
    pages: BTreeMap::new(),
    length: 0,
  };

  pub const fn empty() -> Result<Self, Never> {
    Ok(Self::EMPTY)
  }

  pub fn insert(&mut self, value: u32) -> Result<Presence, Never> {
    let Ok(start) = PageStart::of(value);
    let Ok(before) = self.pages.entry(start).or_default().mark(value, Mark::Set);

    match before {
      Presence::Absent => self.length = self.length.saturating_add(1),
      Presence::Present => {}
    }

    Ok(before)
  }

  pub fn remove(&mut self, value: u32) -> Result<Presence, Never> {
    let Ok(start) = PageStart::of(value);

    Ok(match self.pages.entry(start) {
      Entry::Occupied(mut entry) => {
        let Ok(before) = entry.get_mut().mark(value, Mark::Clear);
        let Ok(emptiness) = entry.get().is_empty();

        match before {
          Presence::Present => self.length = self.length.saturating_sub(1),
          Presence::Absent => {}
        }

        match emptiness {
          Emptiness::Empty => {
            let _emptied = entry.remove();
          }
          Emptiness::NonEmpty => {}
        }

        before
      }
      Entry::Vacant(_) => Presence::Absent,
    })
  }

  pub fn insert_range(&mut self, range: RangeInclusive<u32>) -> Result<(), Never> {
    let first = *range.start();
    let last = *range.end();

    match first.cmp(&last) {
      Ordering::Less | Ordering::Equal => {}
      Ordering::Greater => return Ok(()),
    }

    let first_page = first / page::WIDTH;
    let last_page = last / page::WIDTH;

    for number in first_page..=last_page {
      let start = PageStart(number.saturating_mul(page::WIDTH.get()));
      let Ok(page_last) = start.last();
      let page = self.pages.entry(start).or_default();
      let Ok(before) = page.len();
      let Ok(()) = page.mark_range(first.max(start.0)..=last.min(page_last), Mark::Set);
      let Ok(after) = page.len();

      self.length = self
        .length
        .saturating_add(u64::from(after.saturating_sub(before)));
    }

    Ok(())
  }

  pub fn remove_range(&mut self, range: RangeInclusive<u32>) -> Result<(), Never> {
    let first = *range.start();
    let last = *range.end();

    match first.cmp(&last) {
      Ordering::Less | Ordering::Equal => {}
      Ordering::Greater => return Ok(()),
    }

    self.pages.retain(|start, page| {
      let Ok(page_last) = start.last();

      match Overlap::from(start.0 <= last && page_last >= first) {
        Overlap::Intersecting => {
          let Ok(()) = page.mark_range(first.max(start.0)..=last.min(page_last), Mark::Clear);

          matches!(page.is_empty(), Ok(Emptiness::NonEmpty))
        }
        Overlap::Disjoint => true,
      }
    });

    self.recount()
  }

  pub fn contains(&self, value: u32) -> Result<Presence, Never> {
    let Ok(start) = PageStart::of(value);

    match self.pages.get(&start) {
      Some(page) => page.contains(value),
      None => Ok(Presence::Absent),
    }
  }

  pub fn clear(&mut self) -> Result<(), Never> {
    self.pages.clear();
    self.length = 0;

    Ok(())
  }

  #[allow(
    clippy::len_without_is_empty,
    reason = "is_empty answers an Emptiness, which is not the bool clippy looks for"
  )]
  pub fn len(&self) -> Result<u64, Never> {
    Ok(self.length)
  }

  pub fn is_empty(&self) -> Result<Emptiness, Never> {
    Ok(Emptiness::from(self.pages.is_empty()))
  }

  pub fn extend_unsorted(&mut self, values: impl IntoIterator<Item = u32>) -> Result<(), Never> {
    self.extend(values);

    Ok(())
  }

  pub fn remove_all(&mut self, values: impl IntoIterator<Item = u32>) -> Result<(), Never> {
    for value in values {
      let Ok(_before) = self.remove(value);
    }

    Ok(())
  }

  pub(crate) fn mark(&mut self, value: u32, mark: Mark) -> Result<Presence, Never> {
    match mark {
      Mark::Set => self.insert(value),
      Mark::Clear => self.remove(value),
    }
  }

  pub(crate) fn mark_range(
    &mut self,
    range: RangeInclusive<u32>,
    mark: Mark,
  ) -> Result<(), Never> {
    match mark {
      Mark::Set => self.insert_range(range),
      Mark::Clear => self.remove_range(range),
    }
  }

  pub(crate) fn mark_all(
    &mut self,
    values: impl IntoIterator<Item = u32>,
    mark: Mark,
  ) -> Result<(), Never> {
    match mark {
      Mark::Set => self.extend_unsorted(values),
      Mark::Clear => self.remove_all(values),
    }
  }

  pub fn union(&mut self, other: &Self) -> Result<(), Never> {
    for (start, page) in &other.pages {
      let Ok(()) = self.pages.entry(*start).or_default().union(page);
    }

    self.recount()
  }

  pub fn intersect(&mut self, other: &Self) -> Result<(), Never> {
    self.cut(other, Cut::Intersection)
  }

  pub fn subtract(&mut self, other: &Self) -> Result<(), Never> {
    self.cut(other, Cut::Difference)
  }

  fn cut(&mut self, other: &Self, cut: Cut) -> Result<(), Never> {
    self
      .pages
      .retain(|start, page| match (&cut, other.pages.get(start)) {
        (Cut::Intersection, Some(retained)) => {
          let Ok(()) = page.intersect(retained);

          matches!(page.is_empty(), Ok(Emptiness::NonEmpty))
        }
        (Cut::Difference, Some(removed)) => {
          let Ok(()) = page.subtract(removed);

          matches!(page.is_empty(), Ok(Emptiness::NonEmpty))
        }
        (Cut::Intersection, None) => false,
        (Cut::Difference, None) => true,
      });

    self.recount()
  }

  pub fn reversed_subtract(&mut self, other: &Self) -> Result<(), Never> {
    let mut difference = other.clone();
    let Ok(()) = difference.subtract(self);

    *self = difference;

    Ok(())
  }

  pub fn intersects_set(&self, other: &Self) -> Result<Overlap, Never> {
    Ok(Overlap::from(self.pages.iter().any(|(start, page)| {
      other
        .pages
        .get(start)
        .is_some_and(|theirs| matches!(page.intersects(theirs), Ok(Overlap::Intersecting)))
    })))
  }

  pub fn iter(&self) -> Result<impl DoubleEndedIterator<Item = u32> + '_, Never> {
    Ok(self.pages.iter().flat_map(|(start, page)| {
      let Ok(values) = start.values(page);

      values
    }))
  }

  pub(super) fn range(
    &self,
    range: Option<RangeInclusive<u32>>,
  ) -> Result<impl DoubleEndedIterator<Item = u32> + '_, Never> {
    Ok(range.into_iter().flat_map(move |range_2| {
      let Ok(opening) = PageStart::of(*range_2.start());
      let Ok(closing) = PageStart::of(*range_2.end());

      let held = self
        .pages
        .range(opening..=closing)
        .flat_map(|(start, page)| {
          let Ok(values) = start.values(page);

          values
        });

      held.filter(move |value| range_2.contains(value))
    }))
  }

  pub fn iter_from(&self, value: u32) -> Result<impl Iterator<Item = u32> + '_, Never> {
    let Ok(opening) = PageStart::of(value);

    Ok(
      self
        .pages
        .range(opening..)
        .flat_map(|(start, page)| {
          let Ok(values) = start.values(page);

          values
        })
        .filter(move |candidate| *candidate >= value),
    )
  }

  pub fn iter_ranges(&self) -> Result<impl Iterator<Item = RangeInclusive<u32>> + '_, Never> {
    let ranges = self.pages.iter().flat_map(|(start, page)| {
      let Ok(ranges) = page.ranges();
      let start_2 = *start;

      ranges.map(move |range| {
        let Ok(first) = start_2.with(*range.start());
        let Ok(last) = start_2.with(*range.end());

        first..=last
      })
    });

    Ok(JoinedRanges {
      ranges,
      pending: None,
    })
  }

  fn recount(&mut self) -> Result<(), Never> {
    self.length = self.pages.values().fold(0u64, |length, page| {
      let Ok(count) = page.len();

      length.saturating_add(u64::from(count))
    });

    Ok(())
  }
}

impl Default for U32Set {
  fn default() -> Self {
    Self::EMPTY
  }
}

impl FromIterator<u32> for U32Set {
  fn from_iter<T: IntoIterator<Item = u32>>(values: T) -> Self {
    let mut set = Self::default();

    set.extend(values);

    set
  }
}

impl Extend<u32> for U32Set {
  fn extend<T: IntoIterator<Item = u32>>(&mut self, values: T) {
    for value in values {
      let Ok(_before) = self.insert(value);
    }
  }
}

impl Ord for U32Set {
  fn cmp(&self, other: &Self) -> Ordering {
    match (self.pages == other.pages).then_some(Ordering::Equal) {
      Some(same) => same,
      None => {
        let Ok(left) = self.iter();
        let Ok(right) = other.iter();

        left.cmp(right)
      }
    }
  }
}

impl PartialOrd for U32Set {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

struct JoinedRanges<I> {
  ranges: I,
  pending: Option<RangeInclusive<u32>>,
}

impl<I: Iterator<Item = RangeInclusive<u32>>> Iterator for JoinedRanges<I> {
  type Item = RangeInclusive<u32>;

  fn next(&mut self) -> Option<Self::Item> {
    let mut current = match self.pending.take() {
      Some(range) => range,
      None => self.ranges.next()?,
    };

    for next in self.ranges.by_ref() {
      match current.end().checked_add(1).filter(|after| after == next.start()) {
        Some(_adjacent) => current = *current.start()..=*next.end(),
        None => {
          self.pending = Some(next);

          return Some(current);
        }
      }
    }

    Some(current)
  }
}

#[cfg(feature = "serde")]
mod serialization;

#[cfg(test)]
mod test {
  use super::*;
  use std::collections::HashSet;

  #[test]
  fn len() -> Result<(), crate::TestFailure> {
    let Ok(bitset) = U32Set::empty();
    assert_eq!(bitset.len(), Ok(0));
    assert_eq!(bitset.is_empty(), Ok(Emptiness::Empty));

    Ok(())
  }

  #[test]
  fn from_iter() -> Result<(), crate::TestFailure> {
    let Ok(mut expected) = U32Set::empty();
    expected.extend([2, 8, 13]);
    assert_eq!(U32Set::from_iter([2, 8, 13]), expected);
    assert_eq!(U32Set::from_iter([8, 2, 13]), expected);

    Ok(())
  }

  #[test]
  fn iter() -> Result<(), crate::TestFailure> {
    let Ok(mut bitset) = U32Set::empty();
    let Ok(_before) = bitset.insert(3);
    let Ok(_before) = bitset.insert(8);
    let Ok(_before) = bitset.insert(534);
    let Ok(_before) = bitset.insert(700);
    let Ok(_before) = bitset.insert(10000);
    let Ok(_before) = bitset.insert(10001);
    let Ok(_before) = bitset.insert(10002);
    let Ok(values) = bitset.iter();
    let v: Vec<u32> = values.collect();
    assert_eq!(v, vec![3, 8, 534, 700, 10000, 10001, 10002]);

    Ok(())
  }

  fn check_iter_ranges(
    ranges: Vec<RangeInclusive<u32>>,
  ) -> std::result::Result<(), console_core_never::Never> {
    let Ok(mut set) = U32Set::empty();

    for range in ranges.iter() {
      let Ok(()) = set.insert_range(*range.start()..=*range.end());
    }

    let Ok(set_ranges) = set.iter_ranges();
    let items: Vec<_> = set_ranges.collect();
    assert_eq!(items, ranges);

    Ok(())
  }

  #[test]
  #[allow(
    clippy::single_range_in_vec_init,
    reason = "test inputs contain ranges rather than the values inside them"
  )]
  fn iter_ranges() -> Result<(), crate::TestFailure> {
    let Ok(()) = check_iter_ranges(vec![0..=0]);
    let Ok(()) = check_iter_ranges(vec![4578..=4578]);
    let Ok(()) = check_iter_ranges(vec![0..=10, 4578..=4583]);
    let Ok(()) = check_iter_ranges(vec![0..=700]);
    let Ok(()) = check_iter_ranges(vec![353..=737]);
    let Ok(()) = check_iter_ranges(vec![u32::MAX..=u32::MAX]);
    let Ok(()) = check_iter_ranges(vec![u32::MAX.saturating_sub(10)..=u32::MAX]);
    let Ok(()) = check_iter_ranges(vec![0..=5, u32::MAX.saturating_sub(5)..=u32::MAX]);
    let Ok(()) = check_iter_ranges(vec![0..=511, 513..=517]);
    let Ok(()) = check_iter_ranges(vec![512..=1023, 1025..=1027]);
    let Ok(()) = check_iter_ranges(vec![1792..=2650]);

    Ok(())
  }

  #[test]
  fn iter_ranges_zero_pages() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = U32Set::empty();
    let Ok(_before) = set.insert(1000);
    let Ok(()) = set.insert_range(300..=511);
    let Ok(_before) = set.remove(1000);
    let Ok(ranges) = set.iter_ranges();
    let items: Vec<_> = ranges.collect();
    assert_eq!(items, vec![300..=511]);

    Ok(())
  }

  #[test]
  fn iter_backwards() -> Result<(), crate::TestFailure> {
    let Ok(mut bitset) = U32Set::empty();
    let Ok(()) = bitset.insert_range(1..=6);

    {
      let Ok(mut it) = bitset.iter();
      assert_eq!(Some(1), it.next());
      assert_eq!(Some(6), it.next_back());
      assert_eq!(Some(5), it.next_back());
      assert_eq!(Some(2), it.next());
      assert_eq!(Some(3), it.next());
      assert_eq!(Some(4), it.next());
      assert_eq!(None, it.next());
      assert_eq!(None, it.next_back());
    }

    let Ok(()) = bitset.insert_range(700..=701);

    {
      let Ok(mut it) = bitset.iter();
      assert_eq!(Some(1), it.next());
      assert_eq!(Some(701), it.next_back());
      assert_eq!(Some(700), it.next_back());
      assert_eq!(Some(6), it.next_back());
      assert_eq!(Some(5), it.next_back());
      assert_eq!(Some(2), it.next());
      assert_eq!(Some(3), it.next());
      assert_eq!(Some(4), it.next());
      assert_eq!(None, it.next());
      assert_eq!(None, it.next_back());
    }

    let Ok(values) = bitset.iter();
    let v: Vec<u32> = values.rev().collect();
    assert_eq!(vec![701, 700, 6, 5, 4, 3, 2, 1], v);

    Ok(())
  }

  #[test]
  fn iter_from() -> Result<(), crate::TestFailure> {
    let mut set = U32Set::from_iter([5, 7, 10, 1250, 1300, 3001]);
    let cases: &[(u32, &[u32])] = &[
      (0, &[5, 7, 10, 1250, 1300, 3001]),
      (4, &[5, 7, 10, 1250, 1300, 3001]),
      (5, &[5, 7, 10, 1250, 1300, 3001]),
      (6, &[7, 10, 1250, 1300, 3001]),
      (10, &[10, 1250, 1300, 3001]),
      (700, &[1250, 1300, 3001]),
      (1250, &[1250, 1300, 3001]),
      (1251, &[1300, 3001]),
      (3000, &[3001]),
      (3001, &[3001]),
      (3002, &[]),
      (5000, &[]),
      (u32::MAX, &[]),
    ];

    for (start, expected) in cases {
      let Ok(values) = set.iter_from(*start);
      assert!(values.eq(expected.iter().copied()), "from {start}");
    }

    let Ok(_inserted) = set.insert(u32::MAX);

    for start in [u32::MAX, u32::MAX.saturating_sub(1)] {
      let Ok(values) = set.iter_from(start);
      assert!(values.eq([u32::MAX]));
    }

    Ok(())
  }

  #[test]
  fn iter_from_crosses_page_boundaries() -> Result<(), Never> {
    let set = U32Set::from_iter([510, 511, 512]);
    let cases: &[(u32, &[u32])] = &[
      (509, &[510, 511, 512]),
      (510, &[510, 511, 512]),
      (511, &[511, 512]),
      (512, &[512]),
      (513, &[]),
    ];

    for (start, expected) in cases {
      let Ok(values) = set.iter_from(*start);
      assert!(values.eq(expected.iter().copied()), "from {start}");
    }

    Ok(())
  }

  #[test]
  fn extend() -> Result<(), crate::TestFailure> {
    let values = [3, 8, 534, 700, 10000, 10001, 10002];
    let values_unsorted = [10000, 3, 534, 700, 8, 10001, 10002];
    let Ok(mut s1) = U32Set::empty();
    let Ok(mut s2) = U32Set::empty();
    let Ok(mut s3) = U32Set::empty();
    let Ok(mut s4) = U32Set::empty();
    assert_eq!(s1.len(), Ok(0));
    s1.extend(values.iter().copied());
    let Ok(()) = s2.extend_unsorted(values.iter().copied());
    s3.extend(values_unsorted.iter().copied());
    let Ok(()) = s4.extend_unsorted(values_unsorted.iter().copied());
    let Ok(s1_values) = s1.iter();
    assert_eq!(s1_values.collect::<Vec<u32>>(), values);
    let Ok(s2_values) = s2.iter();
    assert_eq!(s2_values.collect::<Vec<u32>>(), values);
    let Ok(s3_values) = s3.iter();
    assert_eq!(s3_values.collect::<Vec<u32>>(), values);
    let Ok(s4_values) = s4.iter();
    assert_eq!(s4_values.collect::<Vec<u32>>(), values);
    assert_eq!(s1.len(), Ok(7));
    assert_eq!(s2.len(), Ok(7));
    assert_eq!(s3.len(), Ok(7));
    assert_eq!(s4.len(), Ok(7));

    Ok(())
  }

  #[test]
  fn insert_unordered() -> Result<(), crate::TestFailure> {
    let Ok(mut bitset) = U32Set::empty();
    assert_eq!(bitset.contains(0), Ok(Presence::Absent));
    assert_eq!(bitset.contains(768), Ok(Presence::Absent));
    assert_eq!(bitset.contains(1678), Ok(Presence::Absent));
    assert_eq!(bitset.insert(0), Ok(Presence::Absent));
    assert_eq!(bitset.insert(1678), Ok(Presence::Absent));
    assert_eq!(bitset.insert(768), Ok(Presence::Absent));
    assert_eq!(bitset.contains(0), Ok(Presence::Present));
    assert_eq!(bitset.contains(768), Ok(Presence::Present));
    assert_eq!(bitset.contains(1678), Ok(Presence::Present));
    assert_eq!(bitset.contains(1), Ok(Presence::Absent));
    assert_eq!(bitset.contains(769), Ok(Presence::Absent));
    assert_eq!(bitset.contains(1679), Ok(Presence::Absent));
    assert_eq!(bitset.len(), Ok(3));

    Ok(())
  }

  #[test]
  fn remove() -> Result<(), crate::TestFailure> {
    let Ok(mut bitset) = U32Set::empty();
    assert_eq!(bitset.insert(0), Ok(Presence::Absent));
    assert_eq!(bitset.insert(511), Ok(Presence::Absent));
    assert_eq!(bitset.insert(512), Ok(Presence::Absent));
    assert_eq!(bitset.insert(1678), Ok(Presence::Absent));
    assert_eq!(bitset.insert(768), Ok(Presence::Absent));
    assert_eq!(bitset.len(), Ok(5));
    assert_eq!(bitset.remove(12), Ok(Presence::Absent));
    assert_eq!(bitset.remove(511), Ok(Presence::Present));
    assert_eq!(bitset.remove(512), Ok(Presence::Present));
    assert_eq!(bitset.remove(512), Ok(Presence::Absent));
    assert_eq!(bitset.len(), Ok(3));
    assert_eq!(bitset.contains(0), Ok(Presence::Present));
    assert_eq!(bitset.contains(511), Ok(Presence::Absent));
    assert_eq!(bitset.contains(512), Ok(Presence::Absent));

    Ok(())
  }

  #[test]
  fn remove_all() -> Result<(), crate::TestFailure> {
    let Ok(mut bitset) = U32Set::empty();
    bitset.extend([5, 7, 11, 18, 620, 2000]);
    assert_eq!(bitset.len(), Ok(6));
    let Ok(()) = bitset.remove_all([7, 11, 13, 18, 620]);
    assert_eq!(bitset.len(), Ok(2));
    let Ok(values) = bitset.iter();
    assert_eq!(values.collect::<Vec<u32>>(), vec![5, 2000]);

    Ok(())
  }

  #[test]
  fn remove_range() -> Result<(), crate::TestFailure> {
    let Ok(mut bitset) = U32Set::empty();
    bitset.extend([5, 7, 11, 18, 511, 620, 1023, 1024, 1200]);
    assert_eq!(bitset.len(), Ok(9));
    let Ok(()) = bitset.remove_range(7..=620);
    assert_eq!(bitset.len(), Ok(4));
    let Ok(values) = bitset.iter();
    assert_eq!(values.collect::<Vec<u32>>(), vec![5, 1023, 1024, 1200]);

    let Ok(mut bitset_2) = U32Set::empty();
    bitset_2.extend([5, 7, 11, 18, 511, 620, 1023, 1024, 1200]);
    let Ok(()) = bitset_2.remove_range(7..=1024);
    assert_eq!(bitset_2.len(), Ok(2));
    let Ok(values_2) = bitset_2.iter();
    assert_eq!(values_2.collect::<Vec<u32>>(), vec![5, 1200]);
    let Ok(mut bitset_3) = U32Set::empty();
    bitset_3.extend([5, 7, 11, 18, 511, 620, 1023, 1024, 1200]);
    let Ok(()) = bitset_3.remove_range(2000..=2100);
    assert_eq!(bitset_3.len(), Ok(9));
    let Ok(values_3) = bitset_3.iter();
    assert_eq!(
      values_3.collect::<Vec<u32>>(),
      vec![5, 7, 11, 18, 511, 620, 1023, 1024, 1200]
    );

    let Ok(mut bitset_4) = U32Set::empty();
    bitset_4.extend([1001, 1002, 1003, 1004]);
    let Ok(()) = bitset_4.remove_range(1002..=1003);
    assert_eq!(bitset_4.contains(1001), Ok(Presence::Present));
    assert_eq!(bitset_4.contains(1002), Ok(Presence::Absent));
    assert_eq!(bitset_4.contains(1003), Ok(Presence::Absent));
    assert_eq!(bitset_4.contains(1004), Ok(Presence::Present));
    let Ok(()) = bitset_4.remove_range(100..=200);
    assert_eq!(bitset_4.contains(1001), Ok(Presence::Present));
    assert_eq!(bitset_4.contains(1002), Ok(Presence::Absent));
    assert_eq!(bitset_4.contains(1003), Ok(Presence::Absent));
    assert_eq!(bitset_4.contains(1004), Ok(Presence::Present));
    let Ok(()) = bitset_4.remove_range(100..=1001);
    assert_eq!(bitset_4.contains(1001), Ok(Presence::Absent));
    assert_eq!(bitset_4.contains(1002), Ok(Presence::Absent));
    assert_eq!(bitset_4.contains(1003), Ok(Presence::Absent));
    assert_eq!(bitset_4.contains(1004), Ok(Presence::Present));

    Ok(())
  }

  #[test]
  fn remove_range_boundary() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = U32Set::empty();
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
  fn remove_to_empty_page() -> Result<(), crate::TestFailure> {
    let Ok(mut bitset) = U32Set::empty();
    let Ok(_before) = bitset.insert(793);
    let Ok(_before) = bitset.insert(43);
    let Ok(_before) = bitset.remove(793);
    assert_eq!(bitset.contains(43), Ok(Presence::Present));
    assert_eq!(bitset.contains(793), Ok(Presence::Absent));
    assert_eq!(bitset.len(), Ok(1));

    Ok(())
  }

  #[test]
  fn insert_max_value() -> Result<(), crate::TestFailure> {
    let Ok(mut bitset) = U32Set::empty();
    assert_eq!(bitset.contains(u32::MAX), Ok(Presence::Absent));
    assert_eq!(bitset.insert(u32::MAX), Ok(Presence::Absent));
    assert_eq!(bitset.contains(u32::MAX), Ok(Presence::Present));
    assert_eq!(
      bitset.contains(u32::MAX.saturating_sub(1)),
      Ok(Presence::Absent)
    );
    assert_eq!(bitset.len(), Ok(1));

    Ok(())
  }

  #[test]
  fn contains_index_cache() -> Result<(), crate::TestFailure> {
    let mut bitset = U32Set::from_iter([10, 11, 12, 2023]);
    assert_eq!(bitset.contains(9), Ok(Presence::Absent));
    assert_eq!(bitset.contains(10), Ok(Presence::Present));
    assert_eq!(bitset.contains(11), Ok(Presence::Present));
    assert_eq!(bitset.contains(12), Ok(Presence::Present));
    assert_eq!(bitset.contains(1200), Ok(Presence::Absent));
    assert_eq!(bitset.contains(2022), Ok(Presence::Absent));
    assert_eq!(bitset.contains(2023), Ok(Presence::Present));
    assert_eq!(bitset.contains(2024), Ok(Presence::Absent));
    assert_eq!(bitset.contains(2023), Ok(Presence::Present));
    assert_eq!(bitset.contains(11), Ok(Presence::Present));
    assert_eq!(bitset.contains(5000), Ok(Presence::Absent));
    assert_eq!(bitset.contains(11), Ok(Presence::Present));
    assert_eq!(bitset.contains(2023), Ok(Presence::Present));
    assert_eq!(bitset.contains(12), Ok(Presence::Present));
    assert_eq!(bitset.contains(2024), Ok(Presence::Absent));
    assert_eq!(bitset.contains(13), Ok(Presence::Absent));
    let Ok(()) = bitset.clear();
    let Ok(_before) = bitset.insert(2024);
    let Ok(_before) = bitset.insert(13);
    assert_eq!(bitset.contains(13), Ok(Presence::Present));
    assert_eq!(bitset.contains(12), Ok(Presence::Absent));
    assert_eq!(bitset.contains(2024), Ok(Presence::Present));
    assert_eq!(bitset.contains(2023), Ok(Presence::Absent));

    Ok(())
  }

  fn check_process<A, B, C, Op>(
    a: A,
    b: B,
    expected: C,
    op: Op,
  ) -> std::result::Result<(), console_core_never::Never>
  where
    A: IntoIterator<Item = u32>,
    B: IntoIterator<Item = u32>,
    C: IntoIterator<Item = u32>,
    Op: Fn(&mut U32Set, &U32Set) -> Result<(), Never>,
  {
    let mut result = U32Set::from_iter(a);
    let b_set = U32Set::from_iter(b);
    let expected_set = U32Set::from_iter(expected);
    let Ok(_length) = result.len();
    let Ok(()) = op(&mut result, &b_set);
    assert_eq!(result, expected_set);
    let Ok(length) = result.len();
    let Ok(expected_set_length) = expected_set.len();
    assert_eq!(length, expected_set_length);

    Ok(())
  }

  #[test]
  fn union() -> Result<(), crate::TestFailure> {
    let Ok(_answer) = check_process([], [5], [5], |a, b| a.union(b));
    let Ok(_answer) = check_process([128], [5], [128, 5], |a, b| a.union(b));
    let Ok(_answer) = check_process([128], [], [128], |a, b| a.union(b));
    let Ok(_answer) = check_process([1280], [5], [5, 1280], |a, b| a.union(b));
    let Ok(_answer) = check_process([5], [1280], [5, 1280], |a, b| a.union(b));

    Ok(())
  }

  #[test]
  fn intersect() -> Result<(), crate::TestFailure> {
    let Ok(_answer) = check_process([], [5], [], |a, b| a.intersect(b));
    let Ok(_answer) = check_process([5], [], [], |a, b| a.intersect(b));
    let Ok(_answer) = check_process([1, 5, 9], [5, 7], [5], |a, b| a.intersect(b));
    let Ok(_answer) = check_process([1, 1000, 2000], [1000], [1000], |a, b| a.intersect(b));
    let Ok(_answer) = check_process([1000], [1, 1000, 2000], [1000], |a, b| a.intersect(b));
    let Ok(_answer) = check_process([1, 1000, 2000], [1000, 5000], [1000], |a, b| a.intersect(b));

    Ok(())
  }

  #[test]
  fn subtract() -> Result<(), crate::TestFailure> {
    let Ok(_answer) = check_process([], [5], [], |a, b| a.subtract(b));
    let Ok(_answer) = check_process([5], [], [5], |a, b| a.subtract(b));
    let Ok(_answer) = check_process([5, 1000], [1000], [5], |a, b| a.subtract(b));
    let Ok(_answer) = check_process([5, 1000], [5], [1000], |a, b| a.subtract(b));

    Ok(())
  }

  #[test]
  fn reversed_subtract() -> Result<(), crate::TestFailure> {
    let Ok(_answer) = check_process([], [5], [5], |a, b| a.reversed_subtract(b));
    let Ok(_answer) = check_process([5], [], [], |a, b| a.reversed_subtract(b));
    let Ok(_answer) = check_process([1000], [5, 1000], [5], |a, b| a.reversed_subtract(b));
    let Ok(_answer) = check_process([5], [5, 1000], [1000], |a, b| a.reversed_subtract(b));

    Ok(())
  }

  fn set_for_range(
    range: RangeInclusive<u32>,
  ) -> std::result::Result<U32Set, console_core_never::Never> {
    let Ok(mut set) = U32Set::empty();

    for i in range {
      let Ok(_before) = set.insert(i);
    }

    Ok(set)
  }

  #[test]
  fn insert_range() -> Result<(), crate::TestFailure> {
    for range in [
      (0, 0),
      (0, 364),
      (0, 511),
      (512, 1023),
      (0, 1023),
      (364, 700),
      (364, 6000),
    ] {
      let Ok(mut set) = U32Set::empty();
      let Ok(()) = set.insert_range(range.0..=range.1);
      let Ok(expected) = set_for_range(range.0..=range.1);
      assert_eq!(set, expected, "{range:?}");
      let Ok(length) = set.len();
      let Ok(fitted) = console_core_number_conversion::fitted::<_, u64>(
        range.1.saturating_sub(range.0).saturating_add(1),
      );
      assert_eq!(length, fitted, "{range:?}");
    }

    Ok(())
  }

  #[test]
  fn insert_range_on_existing() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = U32Set::empty();
    let Ok(_before) = set.insert(700);
    let Ok(_before) = set.insert(2000);
    let Ok(()) = set.insert_range(32..=4000);
    let Ok(expected) = set_for_range(32..=4000);
    assert_eq!(set, expected);
    assert_eq!(set.len(), Ok(4000u64.saturating_sub(32).saturating_add(1)));

    Ok(())
  }

  #[test]
  fn insert_range_max() -> Result<(), crate::TestFailure> {
    let Ok(mut set) = U32Set::empty();
    let Ok(()) = set.insert_range(u32::MAX..=u32::MAX);
    assert_eq!(set.contains(u32::MAX), Ok(Presence::Present));
    assert_eq!(set.len(), Ok(1));

    Ok(())
  }

  #[test]
  fn clear() -> Result<(), crate::TestFailure> {
    let Ok(mut bitset) = U32Set::empty();
    let Ok(_before) = bitset.insert(13);
    let Ok(_before) = bitset.insert(670);
    assert_eq!(bitset.contains(13), Ok(Presence::Present));
    assert_eq!(bitset.contains(670), Ok(Presence::Present));
    let Ok(()) = bitset.clear();
    assert_eq!(bitset.contains(13), Ok(Presence::Absent));
    assert_eq!(bitset.contains(670), Ok(Presence::Absent));
    assert_eq!(bitset.len(), Ok(0));

    Ok(())
  }

  #[test]
  fn hash_and_eq() -> Result<(), crate::TestFailure> {
    let Ok(mut bitset1) = U32Set::empty();
    let Ok(mut bitset2) = U32Set::empty();
    let Ok(mut bitset3) = U32Set::empty();
    let Ok(mut bitset4) = U32Set::empty();
    let Ok(_before) = bitset1.insert(43);
    let Ok(_before) = bitset1.insert(793);
    let Ok(_before) = bitset2.insert(793);
    let Ok(_before) = bitset2.insert(43);
    let Ok(_length) = bitset2.len();
    let Ok(_before) = bitset3.insert(43);
    let Ok(_before) = bitset3.insert(793);
    let Ok(_before) = bitset3.insert(794);
    let Ok(_before) = bitset4.insert(0);
    let Ok(empty) = U32Set::empty();
    let Ok(u32_set_empty) = U32Set::empty();
    assert_eq!(empty, u32_set_empty);
    assert_eq!(bitset1, bitset2);
    assert_ne!(bitset1, bitset3);
    assert_ne!(bitset2, bitset3);
    assert_eq!(bitset4, bitset4);
    let set = HashSet::from([bitset1]);
    assert!(set.contains(&bitset2));
    assert!(!set.contains(&bitset3));

    Ok(())
  }

  #[test]
  fn removed_values_do_not_affect_hash_or_membership() -> Result<(), crate::TestFailure> {
    let Ok(mut bitset1) = U32Set::empty();
    let Ok(mut bitset2) = U32Set::empty();
    let Ok(mut bitset3) = U32Set::empty();
    let Ok(_before) = bitset1.insert(43);
    let Ok(_before) = bitset2.insert(793);
    let Ok(_before) = bitset2.insert(43);
    let Ok(_before) = bitset2.remove(793);
    let Ok(_before) = bitset3.insert(43);
    let Ok(_before) = bitset3.insert(793);
    assert_eq!(bitset1, bitset2);
    assert_ne!(bitset1, bitset3);
    let set = HashSet::from([bitset1]);
    assert!(set.contains(&bitset2));

    Ok(())
  }

  #[test]
  fn queries_do_not_affect_hash_or_membership() -> Result<(), crate::TestFailure> {
    let bitset1 = U32Set::from_iter([5, 1027]);
    let bitset2 = U32Set::from_iter([5, 1027]);
    assert_eq!(bitset1.contains(1027), Ok(Presence::Present));
    assert_eq!(bitset2.contains(5), Ok(Presence::Present));
    assert_eq!(bitset1, bitset2);
    assert!(matches!(bitset1.cmp(&bitset2), Ordering::Equal));
    let set = HashSet::from([bitset1]);
    assert!(set.contains(&bitset2));

    Ok(())
  }

  #[test]
  fn ordering() -> Result<(), crate::TestFailure> {
    let assert_order = |left: &[u32], right: &[u32], expected| {
      assert_eq!(
        U32Set::from_iter(left.iter().copied()).cmp(&U32Set::from_iter(right.iter().copied())),
        expected,
        "{left:?}, {right:?}"
      );
    };

    assert_order(&[], &[], Ordering::Equal);
    assert_order(&[], &[0], Ordering::Less);
    assert_order(&[0], &[0], Ordering::Equal);
    assert_order(&[0, 1, 2], &[1, 2, 3], Ordering::Less);
    assert_order(&[0, 1, 4], &[1, 2, 3], Ordering::Less);
    assert_order(&[1, 2, 3], &[0, 2, 4], Ordering::Greater);
    assert_order(&[5, 4, 0], &[1, 2, 3], Ordering::Less);
    assert_order(&[1, 2, 3], &[1, 2, 3, 4], Ordering::Less);
    assert_order(&[2, 3, 4], &[1, 2, 3, 4, 5], Ordering::Greater);
    assert_order(
      &[1000, 2000, 3000],
      &[1000, 2000, 3000, 4000],
      Ordering::Less,
    );
    assert_order(&[1000, 1001], &[1000, 2000], Ordering::Less);
    assert_order(
      &[2000, 3000, 4000],
      &[1000, 2000, 3000, 4000, 5000],
      Ordering::Greater,
    );

    Ok(())
  }

  #[test]
  fn intersects() -> Result<(), crate::TestFailure> {
    fn check_intersection(
      [left, right]: [&U32Set; 2],
      expected: Overlap,
    ) -> Result<(), console_core_never::Never> {
      assert_eq!(left.intersects_set(right), Ok(expected));
      assert_eq!(right.intersects_set(left), Ok(expected));

      Ok(())
    }

    let a = U32Set::from_iter([2, 4, 5, 2057, 7000]);
    let b = U32Set::from_iter([3]);
    let c = U32Set::from_iter([2058]);
    let d = U32Set::from_iter([2057, 3000]);
    let e = U32Set::from_iter([3, 7000]);
    let Ok(()) = check_intersection([&a, &b], Overlap::Disjoint);
    let Ok(()) = check_intersection([&a, &c], Overlap::Disjoint);
    let Ok(()) = check_intersection([&e, &d], Overlap::Disjoint);
    let Ok(()) = check_intersection([&a, &d], Overlap::Intersecting);
    let Ok(()) = check_intersection([&a, &e], Overlap::Intersecting);
    let Ok(()) = check_intersection([&b, &e], Overlap::Intersecting);
    let Ok(mut a_2) = U32Set::empty();
    let Ok(_before) = a_2.insert(4000);
    let Ok(_before) = a_2.insert(0);
    let b_2 = U32Set::from_iter([4000]);
    let Ok(()) = check_intersection([&a_2, &b_2], Overlap::Intersecting);

    Ok(())
  }
}
