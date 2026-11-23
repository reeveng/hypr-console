//! Every set answers what a `BTreeSet` holding the same members answers, over seeded walks of inserts, removals, ranges, inversions and the set operations.
//!
//! The sparse-encoding digest was recorded with unmodified `read-fonts` 0.41.0 before the rewrite.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::{Bound, RangeInclusive};

use console_core_integer_sets::sparse_bit_set::{DecodingBounds, to_sparse_bit_set_with_bf};
use console_core_integer_sets::{
  Continuity, Domain, Emptiness, InDomain, IntSet, Membership, Overlap, Presence, U32Set,
};
use console_core_never::Never;
use console_core_number_conversion::fitted;

struct Seeded(u64);

impl Seeded {
  fn next(&mut self) -> Result<u64, Never> {
    self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);

    let mut mixed = self.0;

    mixed = (mixed ^ mixed.wrapping_shr(30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ mixed.wrapping_shr(27)).wrapping_mul(0x94D0_49BB_1331_11EB);

    Ok(mixed ^ mixed.wrapping_shr(31))
  }

  fn below(&mut self, bound: u64) -> Result<u64, Never> {
    let Ok(drawn) = self.next();

    Ok(match drawn.checked_rem(bound) {
      Some(below) => below,
      None => 0,
    })
  }

  fn u16_below(&mut self, bound: u64) -> Result<u16, Never> {
    let Ok(drawn) = self.below(bound);

    fitted(drawn)
  }

  fn u32_below(&mut self, bound: u64) -> Result<u32, Never> {
    let Ok(drawn) = self.below(bound);

    fitted(drawn)
  }

  fn u16_span(&mut self, widest: u64) -> Result<RangeInclusive<u16>, Never> {
    let Ok(start) = self.u16();
    let Ok(width) = self.u16_below(widest);

    Ok(start..=start.saturating_add(width))
  }

  fn u32_span(&mut self, widest: u64) -> Result<RangeInclusive<u32>, Never> {
    let Ok(start) = self.u32();
    let Ok(width) = self.u32_below(widest);

    Ok(start..=start.saturating_add(width))
  }

  fn u16(&mut self) -> Result<u16, Never> {
    let Ok(kind) = self.below(4);

    match kind {
      0 => self.u16_below(1200),
      1 => {
        let Ok(offset) = self.u16_below(1200);

        Ok(u16::MAX.saturating_sub(offset))
      }
      _anywhere => self.u16_below(65536),
    }
  }

  fn u32(&mut self) -> Result<u32, Never> {
    let Ok(kind) = self.below(4);
    let near = match kind {
      0 => 0u64,
      1 => 1_000_000,
      2 => 4_000_000_000,
      _top => u64::from(u32::MAX).saturating_sub(2600),
    };
    let Ok(offset) = self.below(2600);

    fitted(near.saturating_add(offset))
  }
}

fn hashed<T: Hash>(value: &T) -> Result<u64, Never> {
  let mut hasher = DefaultHasher::new();

  value.hash(&mut hasher);

  Ok(hasher.finish())
}

fn count_of<T>(model: &BTreeSet<T>) -> Result<u64, Never> {
  fitted(model.len())
}

fn emptiness_of<T>(model: &BTreeSet<T>) -> Result<Emptiness, Never> {
  Ok(Emptiness::from(model.is_empty()))
}

fn presence_in<T: Ord>(model: &BTreeSet<T>, value: &T) -> Result<Presence, Never> {
  Ok(Presence::from(model.contains(value)))
}

fn inserted_into<T: Ord>(model: &mut BTreeSet<T>, value: T) -> Result<Presence, Never> {
  Ok(Presence::from(!model.insert(value)))
}

fn removed_from<T: Ord>(model: &mut BTreeSet<T>, value: &T) -> Result<Presence, Never> {
  Ok(Presence::from(model.remove(value)))
}

fn overlap_of<T: Ord>(model: &BTreeSet<T>, other: &BTreeSet<T>) -> Result<Overlap, Never> {
  Ok(match model.intersection(other).next() {
    Some(_shared) => Overlap::Intersecting,
    None => Overlap::Disjoint,
  })
}

fn overlap_with<T>(expected: &[T]) -> Result<Overlap, Never> {
  Ok(Overlap::from(!expected.is_empty()))
}

fn ranges_of<I: Iterator<Item = u32>>(values: I) -> Result<Vec<RangeInclusive<u32>>, Never> {
  let mut out: Vec<RangeInclusive<u32>> = Vec::new();

  for value in values {
    match out
      .last_mut()
      .filter(|last| last.end().checked_add(1) == Some(value))
    {
      Some(adjacent) => *adjacent = *adjacent.start()..=value,
      None => out.push(value..=value),
    }
  }

  Ok(out)
}

fn u16_ranges(model: &BTreeSet<u16>) -> Result<Vec<RangeInclusive<u16>>, Never> {
  let Ok(ranges) = ranges_of(model.iter().map(|value| u32::from(*value)));

  Ok(
    ranges
      .into_iter()
      .map(|range| {
        let Ok(first) = fitted(*range.start());
        let Ok(last) = fitted(*range.end());

        first..=last
      })
      .collect(),
  )
}

fn complement16(model: &BTreeSet<u16>) -> Result<BTreeSet<u16>, Never> {
  Ok(
    (0..=u16::MAX)
      .filter(|value| !model.contains(value))
      .collect(),
  )
}

#[derive(Clone, Copy)]
enum Combination {
  Union,
  Intersection,
  Difference,
}

fn membership_after(
  combination: Combination,
  mine: Membership,
  theirs: Membership,
) -> Result<Membership, Never> {
  Ok(match (combination, mine, theirs) {
    (Combination::Union, Membership::Inclusive, Membership::Exclusive)
    | (Combination::Union, Membership::Exclusive, Membership::Inclusive)
    | (Combination::Union, Membership::Exclusive, Membership::Exclusive)
    | (Combination::Intersection, Membership::Exclusive, Membership::Exclusive)
    | (Combination::Difference, Membership::Exclusive, Membership::Inclusive) => {
      Membership::Exclusive
    }
    (Combination::Union, Membership::Inclusive, Membership::Inclusive)
    | (Combination::Intersection, Membership::Inclusive, Membership::Inclusive)
    | (Combination::Intersection, Membership::Inclusive, Membership::Exclusive)
    | (Combination::Intersection, Membership::Exclusive, Membership::Inclusive)
    | (Combination::Difference, Membership::Inclusive, Membership::Inclusive)
    | (Combination::Difference, Membership::Inclusive, Membership::Exclusive)
    | (Combination::Difference, Membership::Exclusive, Membership::Exclusive) => {
      Membership::Inclusive
    }
  })
}

enum Change<T> {
  Insertion(RangeInclusive<T>),
  Removal(RangeInclusive<T>),
}

impl Change<u16> {
  fn applied_to(self, set: &mut IntSet<u16>) -> Result<(), Never> {
    match self {
      Change::Insertion(range) => set.insert_range(range),
      Change::Removal(range) => set.remove_range(range),
    }
  }
}

impl Change<u32> {
  fn applied_to(self, set: &mut U32Set) -> Result<(), Never> {
    match self {
      Change::Insertion(range) => set.insert_range(range),
      Change::Removal(range) => set.remove_range(range),
    }
  }
}

fn change_in<T: Ord>(
  seeded: &mut Seeded,
  range: RangeInclusive<T>,
  model: &mut BTreeSet<T>,
) -> Result<Change<T>, Never>
where
  RangeInclusive<T>: Iterator<Item = T> + Clone,
{
  let Ok(kind) = seeded.below(2);

  Ok(match kind {
    0 => {
      model.extend(range.clone());

      Change::Insertion(range)
    }
    _removal => {
      model.retain(|value| !range.contains(value));

      Change::Removal(range)
    }
  })
}

fn inverted(membership: Membership) -> Result<Membership, Never> {
  Ok(match membership {
    Membership::Inclusive => Membership::Exclusive,
    Membership::Exclusive => Membership::Inclusive,
  })
}

fn random_u16_set(seeded: &mut Seeded) -> Result<(IntSet<u16>, BTreeSet<u16>), Never> {
  let Ok(mut set) = IntSet::<u16>::empty();
  let mut model = BTreeSet::new();
  let Ok(ranges) = seeded.below(6);

  for _range in 0..ranges {
    let Ok(start) = seeded.u16();
    let Ok(width) = seeded.u16_below(700);
    let end = start.saturating_add(width);
    let Ok(()) = set.insert_range(start..=end);

    model.extend(start..=end);
  }

  let Ok(values) = seeded.below(40);

  for _value in 0..values {
    let Ok(value) = seeded.u16();
    let Ok(_before) = set.insert(value);

    model.insert(value);
  }

  let Ok(flip) = seeded.below(2);

  match flip {
    0 => {
      let Ok(()) = set.invert();
      let Ok(complement) = complement16(&model);

      model = complement;
    }
    _kept => {}
  }

  let Ok(removals) = seeded.below(20);

  for _removal in 0..removals {
    let Ok(value) = seeded.u16();
    let Ok(_before) = set.remove(value);

    model.remove(&value);
  }

  Ok((set, model))
}

fn check_u16_whole(set: &IntSet<u16>, model: &BTreeSet<u16>) -> Result<(), Never> {
  let Ok(values) = set.iter();
  let Ok(backwards) = set.iter();
  let Ok(ranges) = set.iter_ranges();
  let Ok(excluded) = set.iter_excluded_ranges();
  let Ok(complement) = complement16(model);
  let Ok(members) = set.inclusive_iter();
  let Ok(membership) = set.membership();
  let shape = match &members {
    Some(_members) => Membership::Inclusive,
    None => Membership::Exclusive,
  };

  assert_eq!(set.len(), count_of(model));
  assert_eq!(set.is_empty(), emptiness_of(model));
  assert_eq!(set.first(), Ok(model.first().copied()));
  assert_eq!(set.last(), Ok(model.last().copied()));
  assert!(values.eq(model.iter().copied()));
  assert!(backwards.rev().eq(model.iter().rev().copied()));
  assert_eq!(Ok(ranges.collect::<Vec<_>>()), u16_ranges(model));
  assert_eq!(Ok(excluded.collect::<Vec<_>>()), u16_ranges(&complement));
  assert_eq!(shape, membership);

  match members {
    Some(explicit_members) => assert!(explicit_members.eq(model.iter().copied())),
    None => {}
  }

  let rebuilt: IntSet<u16> = model.iter().copied().collect();

  assert_eq!(*set, rebuilt);
  assert_eq!(hashed(set), hashed(&rebuilt));
  assert_eq!(set.cmp(&rebuilt), Ordering::Equal);

  Ok(())
}

fn check_u16_ranges(
  set: &IntSet<u16>,
  model: &BTreeSet<u16>,
  seeded: &mut Seeded,
) -> Result<(), Never> {
  let Ok(start) = seeded.u16();
  let Ok(width) = seeded.u16_below(3000);
  let end = start.saturating_add(width);
  let expected: Vec<u16> = model.range(start..=end).copied().collect();
  let Ok(within) = set.range(start..=end);
  let Ok(before_end) = set.range(start..end);
  let Ok(from) = set.range(start..);
  let Ok(through) = set.range(..=end);
  let Ok(after) = set.iter_after(start);

  assert_eq!(within.collect::<Vec<_>>(), expected);
  assert_eq!(set.intersects_range(start..=end), overlap_with(&expected));
  assert_eq!(
    before_end.collect::<Vec<_>>(),
    model.range(start..end).copied().collect::<Vec<_>>()
  );
  assert!(from.take(50).eq(model.range(start..).take(50).copied()));
  assert!(through.take(50).eq(model.range(..=end).take(50).copied()));
  assert!(
    after.take(50).eq(
      model
        .range((Bound::Excluded(start), Bound::Unbounded))
        .take(50)
        .copied()
    )
  );

  Ok(())
}

fn check_u16(set: &IntSet<u16>, model: &BTreeSet<u16>, seeded: &mut Seeded) -> Result<(), Never> {
  let Ok(()) = check_u16_whole(set, model);

  for _sample in 0..64 {
    let Ok(value) = seeded.u16();

    assert_eq!(set.contains(value), presence_in(model, &value), "{value}");
  }

  for _range in 0..12 {
    let Ok(()) = check_u16_ranges(set, model, seeded);
  }

  Ok(())
}

struct U16Walk {
  seeded: Seeded,
  set: IntSet<u16>,
  model: BTreeSet<u16>,
  membership: Membership,
}

impl U16Walk {
  fn ranges(&mut self) -> Result<(), Never> {
    let Ok(span) = self.seeded.u16_span(1500);
    let Ok(change) = change_in(&mut self.seeded, span, &mut self.model);

    change.applied_to(&mut self.set)
  }

  fn several(&mut self) -> Result<(), Never> {
    let Ok(count) = self.seeded.below(30);
    let mut values = Vec::new();

    for _value in 0..count {
      let Ok(value) = self.seeded.u16();

      values.push(value);
    }

    let Ok(kind) = self.seeded.below(3);

    match kind {
      0 => {
        self.set.extend(values.iter().copied());
        self.model.extend(values.iter().copied());
      }
      1 => {
        let Ok(()) = self.set.extend_unsorted(values.iter().copied());

        self.model.extend(values.iter().copied());
      }
      _removal => {
        let removed: BTreeSet<u16> = values.iter().copied().collect();
        let Ok(()) = self.set.remove_all(values.iter().copied());

        self.model.retain(|value| !removed.contains(value));
      }
    }

    Ok(())
  }

  fn combined(&mut self) -> Result<(), Never> {
    let Ok((other, other_model)) = random_u16_set(&mut self.seeded);
    let Ok(theirs) = other.membership();
    let Ok(kind) = self.seeded.below(3);

    assert_eq!(
      self.set.intersects_set(&other),
      overlap_of(&self.model, &other_model)
    );
    assert_eq!(self.set.cmp(&other), self.model.cmp(&other_model));
    assert_eq!(self.set == other, self.model == other_model);

    let combination = match kind {
      0 => {
        let Ok(()) = self.set.union(&other);

        self.model = self.model.union(&other_model).copied().collect();

        Combination::Union
      }
      1 => {
        let Ok(()) = self.set.intersect(&other);

        self.model = self.model.intersection(&other_model).copied().collect();

        Combination::Intersection
      }
      _difference => {
        let Ok(()) = self.set.subtract(&other);

        self.model = self.model.difference(&other_model).copied().collect();

        Combination::Difference
      }
    };
    let Ok(membership) = membership_after(combination, self.membership, theirs);

    self.membership = membership;

    Ok(())
  }

  fn step(&mut self) -> Result<(), Never> {
    let Ok(kind) = self.seeded.below(10);

    match kind {
      0 => {
        let Ok(value) = self.seeded.u16();

        assert_eq!(
          self.set.insert(value),
          inserted_into(&mut self.model, value)
        );
      }
      1 => {
        let Ok(value) = self.seeded.u16();

        assert_eq!(
          self.set.remove(value),
          removed_from(&mut self.model, &value)
        );
      }
      2 => {
        let Ok(()) = self.ranges();
      }
      3 => {
        let Ok(()) = self.set.invert();
        let Ok(membership) = inverted(self.membership);
        let Ok(complement) = complement16(&self.model);

        self.membership = membership;
        self.model = complement;
      }
      4 => {
        let Ok(()) = self.several();
      }
      5..=7 => {
        let Ok(()) = self.combined();
      }
      8 => {
        let Ok(()) = self.set.clear();

        self.membership = Membership::Inclusive;
        self.model.clear();
      }
      _copy => {
        let copy = self.set.clone();

        assert_eq!(copy, self.set);
      }
    }

    assert_eq!(self.set.membership(), Ok(self.membership));

    check_u16(&self.set, &self.model, &mut self.seeded)
  }
}

#[test]
fn a_u16_set_answers_what_a_btree_answers() {
  for seed in 0..24 {
    let mut seeded = Seeded(seed);
    let Ok((set, model)) = random_u16_set(&mut seeded);
    let Ok(membership) = set.membership();
    let Ok(()) = check_u16(&set, &model, &mut seeded);
    let mut walk = U16Walk {
      seeded,
      set,
      model,
      membership,
    };

    for _step in 0..30 {
      let Ok(()) = walk.step();
    }
  }
}

fn check_u32(set: &U32Set, model: &BTreeSet<u32>, seeded: &mut Seeded) -> Result<(), Never> {
  let Ok(values) = set.iter();
  let Ok(backwards) = set.iter();
  let Ok(ranges) = set.iter_ranges();

  assert_eq!(set.len(), count_of(model));
  assert_eq!(set.is_empty(), emptiness_of(model));
  assert!(values.eq(model.iter().copied()));
  assert!(backwards.rev().eq(model.iter().rev().copied()));
  assert_eq!(
    Ok(ranges.collect::<Vec<_>>()),
    ranges_of(model.iter().copied())
  );

  for _sample in 0..64 {
    let Ok(value) = seeded.u32();

    assert_eq!(set.contains(value), presence_in(model, &value), "{value}");
  }

  for _start in 0..8 {
    let Ok(from) = seeded.u32();
    let Ok(tail) = set.iter_from(from);

    assert!(tail.eq(model.range(from..).copied()));
  }

  let rebuilt: U32Set = model.iter().copied().collect();

  assert_eq!(*set, rebuilt);
  assert_eq!(hashed(set), hashed(&rebuilt));
  assert_eq!(set.cmp(&rebuilt), Ordering::Equal);

  Ok(())
}

fn random_u32_set(seeded: &mut Seeded) -> Result<(U32Set, BTreeSet<u32>), Never> {
  let Ok(mut set) = U32Set::empty();
  let mut model = BTreeSet::new();
  let Ok(ranges) = seeded.below(5);

  for _range in 0..ranges {
    let Ok(start) = seeded.u32();
    let Ok(width) = seeded.u32_below(1400);
    let end = start.saturating_add(width);
    let Ok(()) = set.insert_range(start..=end);

    model.extend(start..=end);
  }

  let Ok(count) = seeded.below(60);
  let mut values = Vec::new();

  for _value in 0..count {
    let Ok(value) = seeded.u32();

    values.push(value);
  }

  let Ok(kind) = seeded.below(3);

  match kind {
    0 => {
      let mut sorted = values.clone();

      sorted.sort_unstable();
      set.extend(sorted);
    }
    1 => set.extend(values.iter().copied()),
    _unsorted => {
      let Ok(()) = set.extend_unsorted(values.iter().copied());
    }
  }

  model.extend(values.iter().copied());

  let Ok(removals) = seeded.below(20);

  for _removal in 0..removals {
    let Ok(value) = seeded.u32();

    assert_eq!(set.remove(value), removed_from(&mut model, &value));
  }

  Ok((set, model))
}

struct U32Walk {
  seeded: Seeded,
  set: U32Set,
  model: BTreeSet<u32>,
}

impl U32Walk {
  fn ranges(&mut self) -> Result<(), Never> {
    let Ok(span) = self.seeded.u32_span(3000);
    let Ok(change) = change_in(&mut self.seeded, span, &mut self.model);

    change.applied_to(&mut self.set)
  }

  fn removals(&mut self) -> Result<(), Never> {
    let Ok(count) = self.seeded.below(30);
    let mut values = Vec::new();

    for _value in 0..count {
      let Ok(value) = self.seeded.u32();

      values.push(value);
    }

    let removed: BTreeSet<u32> = values.iter().copied().collect();
    let Ok(()) = self.set.remove_all(values.iter().copied());

    self.model.retain(|value| !removed.contains(value));

    Ok(())
  }

  fn combined(&mut self) -> Result<(), Never> {
    let Ok((other, other_model)) = random_u32_set(&mut self.seeded);
    let Ok(kind) = self.seeded.below(4);

    assert_eq!(
      self.set.intersects_set(&other),
      overlap_of(&self.model, &other_model)
    );
    assert_eq!(self.set.cmp(&other), self.model.cmp(&other_model));
    assert_eq!(self.set == other, self.model == other_model);

    match kind {
      0 => {
        let Ok(()) = self.set.union(&other);

        self.model = self.model.union(&other_model).copied().collect();
      }
      1 => {
        let Ok(()) = self.set.intersect(&other);

        self.model = self.model.intersection(&other_model).copied().collect();
      }
      2 => {
        let Ok(()) = self.set.subtract(&other);

        self.model = self.model.difference(&other_model).copied().collect();
      }
      _reversed => {
        let Ok(()) = self.set.reversed_subtract(&other);

        self.model = other_model.difference(&self.model).copied().collect();
      }
    }

    Ok(())
  }

  fn step(&mut self) -> Result<(), Never> {
    let Ok(kind) = self.seeded.below(9);

    match kind {
      0 => {
        let Ok(value) = self.seeded.u32();

        assert_eq!(
          self.set.insert(value),
          inserted_into(&mut self.model, value)
        );
      }
      1 => {
        let Ok(value) = self.seeded.u32();

        assert_eq!(
          self.set.remove(value),
          removed_from(&mut self.model, &value)
        );
      }
      2 => {
        let Ok(()) = self.ranges();
      }
      3 => {
        let Ok(()) = self.removals();
      }
      4..=6 => {
        let Ok(()) = self.combined();
      }
      7 => {
        let Ok(()) = self.set.clear();

        self.model.clear();
      }
      _copy => {
        let copy = self.set.clone();

        assert_eq!(copy, self.set);
      }
    }

    check_u32(&self.set, &self.model, &mut self.seeded)
  }
}

#[test]
fn a_u32_set_answers_what_a_btree_answers() {
  for seed in 100..140 {
    let mut seeded = Seeded(seed);
    let Ok((set, model)) = random_u32_set(&mut seeded);
    let Ok(()) = check_u32(&set, &model, &mut seeded);
    let mut walk = U32Walk { seeded, set, model };

    for _step in 0..40 {
      let Ok(()) = walk.step();
    }
  }
}

fn excluded_ranges(exceptions: &BTreeSet<u32>) -> Result<Vec<RangeInclusive<u32>>, Never> {
  let mut out = Vec::new();
  let mut next: Option<u32> = Some(0);

  for value in exceptions {
    match next {
      Some(start) => match start.cmp(value) {
        Ordering::Less => out.push(start..=value.saturating_sub(1)),
        Ordering::Equal | Ordering::Greater => {}
      },
      None => {}
    }

    next = value.checked_add(1);
  }

  match next {
    Some(start) => out.push(start..=u32::MAX),
    None => {}
  }

  Ok(out)
}

fn opposite(presence: Presence) -> Result<Presence, Never> {
  Ok(match presence {
    Presence::Present => Presence::Absent,
    Presence::Absent => Presence::Present,
  })
}

struct Exceptions {
  seeded: Seeded,
  set: IntSet<u32>,
  excepted: BTreeSet<u32>,
}

impl Exceptions {
  fn step(&mut self) -> Result<(), Never> {
    let Ok(kind) = self.seeded.below(4);

    match kind {
      0 => {
        let Ok(value) = self.seeded.u32();
        let Ok(excepted) = inserted_into(&mut self.excepted, value);

        assert_eq!(self.set.remove(value), opposite(excepted));
      }
      1 => {
        let Ok(value) = self.seeded.u32();
        let Ok(excepted) = removed_from(&mut self.excepted, &value);

        assert_eq!(self.set.insert(value), opposite(excepted));
      }
      2 => {
        let Ok(start) = self.seeded.u32();
        let Ok(width) = self.seeded.u32_below(1500);
        let end = start.saturating_add(width);
        let Ok(()) = self.set.remove_range(start..=end);

        self.excepted.extend(start..=end);
      }
      _inclusion => {
        let Ok(start) = self.seeded.u32();
        let Ok(width) = self.seeded.u32_below(1500);
        let end = start.saturating_add(width);
        let Ok(()) = self.set.insert_range(start..=end);

        self.excepted.retain(|value| !(start..=end).contains(value));
      }
    }

    self.check()
  }

  fn check_whole(&self) -> Result<(), Never> {
    let members = || (0..=u32::MAX).filter(|value| !self.excepted.contains(value));
    let members_back = || {
      (0..=u32::MAX)
        .rev()
        .filter(|value| !self.excepted.contains(value))
    };
    let Ok(excepted) = count_of(&self.excepted);
    let Ok(values) = self.set.iter();
    let Ok(backwards) = self.set.iter();
    let Ok(ranges) = self.set.iter_ranges();
    let Ok(excluded) = self.set.iter_excluded_ranges();

    assert_eq!(self.set.membership(), Ok(Membership::Exclusive));
    assert_eq!(
      self.set.len(),
      Ok(4_294_967_296u64.saturating_sub(excepted))
    );
    assert_eq!(self.set.first(), Ok(members().next()));
    assert_eq!(self.set.last(), Ok(members_back().next()));
    assert!(values.take(40).eq(members().take(40)));
    assert!(backwards.rev().take(40).eq(members_back().take(40)));
    assert_eq!(
      Ok(ranges.collect::<Vec<_>>()),
      excluded_ranges(&self.excepted)
    );
    assert_eq!(
      Ok(excluded.collect::<Vec<_>>()),
      ranges_of(self.excepted.iter().copied())
    );

    Ok(())
  }

  fn check_ranges(&mut self) -> Result<(), Never> {
    let Ok(start) = self.seeded.u32();
    let Ok(width) = self.seeded.u32_below(3000);
    let end = start.saturating_add(width);
    let expected: Vec<u32> = (start..=end)
      .filter(|value| !self.excepted.contains(value))
      .collect();
    let Ok(within) = self.set.range(start..=end);
    let Ok(after) = self.set.iter_after(start);
    let later = (start..=u32::MAX)
      .skip(1)
      .filter(|value| !self.excepted.contains(value));

    assert_eq!(within.collect::<Vec<_>>(), expected);
    assert_eq!(
      self.set.intersects_range(start..=end),
      overlap_with(&expected)
    );
    assert!(after.take(20).eq(later.take(20)));

    Ok(())
  }

  fn check(&mut self) -> Result<(), Never> {
    let Ok(()) = self.check_whole();

    for _sample in 0..32 {
      let Ok(value) = self.seeded.u32();
      let Ok(excepted) = presence_in(&self.excepted, &value);

      assert_eq!(self.set.contains(value), opposite(excepted), "{value}");
    }

    for _range in 0..6 {
      let Ok(()) = self.check_ranges();
    }

    let mut flipped = self.set.clone();
    let Ok(()) = flipped.invert();
    let Ok(mut inclusive) = IntSet::<u32>::empty();

    inclusive.extend(self.excepted.iter().copied());

    assert_eq!(flipped, inclusive);
    assert_eq!(hashed(&flipped), hashed(&inclusive));

    Ok(())
  }
}

#[test]
fn an_inverted_u32_set_answers_from_its_exceptions() {
  for seed in 200..230 {
    let Ok(set) = IntSet::<u32>::all();
    let mut walk = Exceptions {
      seeded: Seeded(seed),
      set,
      excepted: BTreeSet::new(),
    };

    for _step in 0..30 {
      let Ok(()) = walk.step();
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Even(u16);

impl Domain for Even {
  fn to_u32(&self) -> Result<u32, Never> {
    Ok(u32::from(self.0))
  }

  fn contains(value: u32) -> Result<Presence, Never> {
    Ok(Presence::from(
      value.is_multiple_of(2) && value <= u32::from(u16::MAX),
    ))
  }

  fn from_u32(member: InDomain) -> Result<Even, Never> {
    let Ok(value) = member.value();
    let Ok(narrow) = fitted(value);

    Ok(Even(narrow))
  }

  fn continuity() -> Result<Continuity, Never> {
    Ok(Continuity::Discontinuous)
  }

  fn ordered_values() -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
    Ok((0..=u16::MAX).step_by(2).map(u32::from))
  }

  fn ordered_values_range(
    range: RangeInclusive<Even>,
  ) -> Result<impl DoubleEndedIterator<Item = u32>, Never> {
    let Ok(start) = range.start().to_u32();
    let Ok(end) = range.end().to_u32();
    let Ok(values) = Self::ordered_values();

    Ok(values.filter(move |value| (start..=end).contains(value)))
  }

  fn count() -> Result<u64, Never> {
    Ok(32768)
  }
}

fn even_ranges(model: &BTreeSet<u16>) -> Result<Vec<RangeInclusive<Even>>, Never> {
  let mut out: Vec<RangeInclusive<Even>> = Vec::new();

  for value in model {
    match out
      .last_mut()
      .filter(|last| last.end().0.checked_add(2) == Some(*value))
    {
      Some(adjacent) => *adjacent = *adjacent.start()..=Even(*value),
      None => out.push(Even(*value)..=Even(*value)),
    }
  }

  Ok(out)
}

fn even(seeded: &mut Seeded) -> Result<u16, Never> {
  let Ok(value) = seeded.u16();

  Ok(value & !1)
}

struct EvenWalk {
  seeded: Seeded,
  set: IntSet<Even>,
  model: BTreeSet<u16>,
  every: BTreeSet<u16>,
}

impl EvenWalk {
  fn span(&mut self) -> Result<RangeInclusive<u16>, Never> {
    let Ok(start) = even(&mut self.seeded);
    let Ok(width) = self.seeded.u16_below(900);

    Ok(start..=(start.saturating_add(width) & !1))
  }

  fn step(&mut self) -> Result<(), Never> {
    let Ok(kind) = self.seeded.below(6);

    match kind {
      0 => {
        let Ok(value) = even(&mut self.seeded);

        assert_eq!(
          self.set.insert(Even(value)),
          inserted_into(&mut self.model, value)
        );
      }
      1 => {
        let Ok(value) = even(&mut self.seeded);

        assert_eq!(
          self.set.remove(Even(value)),
          removed_from(&mut self.model, &value)
        );
      }
      2 => {
        let Ok(span) = self.span();
        let Ok(()) = self
          .set
          .insert_range(Even(*span.start())..=Even(*span.end()));

        self.model.extend(span.step_by(2));
      }
      3 => {
        let Ok(span) = self.span();
        let Ok(()) = self
          .set
          .remove_range(Even(*span.start())..=Even(*span.end()));

        self.model.retain(|value| !span.contains(value));
      }
      _inversion => {
        let Ok(()) = self.set.invert();

        self.model = self.every.difference(&self.model).copied().collect();
      }
    }

    self.check()
  }

  fn check_ranges(&mut self) -> Result<(), Never> {
    let Ok(start) = even(&mut self.seeded);
    let Ok(width) = self.seeded.u16_below(3000);
    let end = start.saturating_add(width) & !1;
    let expected: Vec<Even> = self
      .model
      .range(start..=end)
      .map(|value| Even(*value))
      .collect();
    let Ok(within) = self.set.range(Even(start)..=Even(end));
    let Ok(after) = self.set.iter_after(Even(start));
    let later = self
      .model
      .range((Bound::Excluded(start), Bound::Unbounded))
      .map(|value| Even(*value));

    assert_eq!(within.collect::<Vec<_>>(), expected);
    assert_eq!(
      self.set.intersects_range(Even(start)..=Even(end)),
      overlap_with(&expected)
    );
    assert!(after.take(30).eq(later.take(30)));

    Ok(())
  }

  fn check(&mut self) -> Result<(), Never> {
    let Ok(values) = self.set.iter();
    let Ok(backwards) = self.set.iter();
    let Ok(ranges) = self.set.iter_ranges();
    let Ok(excluded) = self.set.iter_excluded_ranges();
    let gaps: BTreeSet<u16> = self.every.difference(&self.model).copied().collect();

    assert_eq!(self.set.len(), count_of(&self.model));
    assert_eq!(
      self.set.first(),
      Ok(self.model.first().map(|value| Even(*value)))
    );
    assert_eq!(
      self.set.last(),
      Ok(self.model.last().map(|value| Even(*value)))
    );
    assert!(values.eq(self.model.iter().map(|value| Even(*value))));
    assert!(
      backwards
        .rev()
        .eq(self.model.iter().rev().map(|value| Even(*value)))
    );
    assert_eq!(Ok(ranges.collect::<Vec<_>>()), even_ranges(&self.model));
    assert_eq!(Ok(excluded.collect::<Vec<_>>()), even_ranges(&gaps));

    for _range in 0..6 {
      let Ok(()) = self.check_ranges();
    }

    let rebuilt: IntSet<Even> = self.model.iter().map(|value| Even(*value)).collect();

    assert_eq!(self.set, rebuilt);
    assert_eq!(hashed(&self.set), hashed(&rebuilt));

    Ok(())
  }
}

#[test]
fn a_set_over_a_domain_with_gaps_answers_what_a_btree_answers() {
  let every: BTreeSet<u16> = (0..=u16::MAX).step_by(2).collect();

  for seed in 300..320 {
    let Ok(set) = IntSet::<Even>::empty();
    let mut walk = EvenWalk {
      seeded: Seeded(seed),
      set,
      model: BTreeSet::new(),
      every: every.clone(),
    };

    for _step in 0..30 {
      let Ok(()) = walk.step();
    }
  }
}

fn digest(bytes: &[u8], into: &mut u64) -> Result<(), Never> {
  for byte in bytes {
    *into = (*into ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01B3);
  }

  Ok(())
}

fn random_encoded_set(seeded: &mut Seeded) -> Result<(IntSet<u32>, BTreeSet<u32>), Never> {
  let Ok(mut set) = IntSet::<u32>::empty();
  let mut model = BTreeSet::new();
  let Ok(ranges) = seeded.below(5);

  for _range in 0..ranges {
    let Ok(start) = seeded.u32();
    let Ok(width) = seeded.u32_below(900);
    let end = start.saturating_add(width);
    let Ok(()) = set.insert_range(start..=end);

    model.extend(start..=end);
  }

  let Ok(values) = seeded.below(40);

  for _value in 0..values {
    let Ok(kind) = seeded.below(2);
    let Ok(value) = match kind {
      0 => seeded.u32(),
      _small => seeded.u32_below(5000),
    };
    let Ok(_before) = set.insert(value);

    model.insert(value);
  }

  Ok((set, model))
}

fn random_bounds(seeded: &mut Seeded) -> Result<DecodingBounds, Never> {
  let Ok(bias) = seeded.u32_below(3000);
  let Ok(kind) = seeded.below(2);
  let Ok(maximum) = match kind {
    0 => Ok(u32::MAX),
    _bounded => seeded.u32_below(6000),
  };

  Ok(DecodingBounds { bias, maximum })
}

#[test]
fn a_set_written_as_a_sparse_bit_set_reads_back_and_writes_the_bytes_it_always_wrote()
-> Result<(), Box<dyn std::error::Error>> {
  let mut written = 0xCBF2_9CE4_8422_2325u64;

  for seed in 400..460 {
    let mut seeded = Seeded(seed);
    let Ok((set, model)) = random_encoded_set(&mut seeded);
    let chosen = set.to_sparse_bit_set()?;
    let binary = to_sparse_bit_set_with_bf::<2>(&set)?;
    let quaternary = to_sparse_bit_set_with_bf::<4>(&set)?;
    let octal = to_sparse_bit_set_with_bf::<8>(&set)?;
    let wide = to_sparse_bit_set_with_bf::<32>(&set)?;
    let encodings = [binary, quaternary, octal, wide];
    let shortest = encodings.iter().fold(u64::MAX, |least, bytes| {
      let Ok(length) = fitted::<_, u64>(bytes.len());

      least.min(length)
    });
    let Ok(chosen_length) = fitted::<_, u64>(chosen.len());

    assert_eq!(chosen_length, shortest);

    for bytes in [&chosen].into_iter().chain(&encodings) {
      let Ok(()) = digest(bytes, &mut written);
      let decoded = IntSet::<u32>::from_sparse_bit_set(bytes)?;

      assert_eq!(decoded, set);
    }

    let Ok(bounds) = random_bounds(&mut seeded);
    let expected: IntSet<u32> = model
      .iter()
      .filter_map(|value| value.checked_add(bounds.bias))
      .filter(|value| *value <= bounds.maximum)
      .collect();

    for bytes in [&chosen].into_iter().chain(&encodings) {
      let mut trailing = bytes.clone();

      trailing.extend([0xAB, 0xCD]);

      let (decoded, rest) = IntSet::<u32>::from_sparse_bit_set_bounded(&trailing, bounds)?;

      assert_eq!(decoded, expected);
      assert_eq!(rest, [0xAB, 0xCD]);
    }
  }

  assert_eq!(written, 6_203_232_447_417_118_335);

  Ok(())
}
