//! A page is 512 consecutive values held as eight words of bits.
//!
//! A value's place in its page is its low nine bits: the word is the three above
//! the low six, and the bit is those six. Both come out of a mask or out of a
//! division by a width that cannot be zero, so nothing here that finds a place
//! can overflow, and a bit is shifted into its word only by an amount already
//! reduced below the word's width. A word the page does not have is one it
//! neither holds nor takes: asking it reads absent, and marking it changes
//! nothing and answers the presence the mark would have left, so nobody counts
//! a change that did not happen.

use core::cmp::Ordering;
use core::num::{NonZeroU32, NonZeroU64};
use core::ops::RangeInclusive;

use console_core_never::Never;
use console_core_number_conversion::index;

use crate::{Emptiness, Overlap, Presence};

pub(super) const WIDTH: NonZeroU32 = match NonZeroU32::new(512) {
  Some(width) => width,
  None => NonZeroU32::MIN,
};

pub(super) const OFFSET_MASK: u32 = WIDTH.get() - 1;

const WORD_WIDTH: NonZeroU32 = match NonZeroU32::new(64) {
  Some(width) => width,
  None => NonZeroU32::MIN,
};

const WORD_LAST: u32 = WORD_WIDTH.get() - 1;

const WORD_STARTS: [u32; 8] = [0, 64, 128, 192, 256, 320, 384, 448];

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(super) struct Page {
  words: [u64; 8],
}

#[derive(Clone, Copy)]
pub(crate) enum Mark {
  Set,
  Clear,
}

impl Mark {
  pub(crate) fn opposite(self) -> Result<Self, Never> {
    Ok(match self {
      Mark::Set => Mark::Clear,
      Mark::Clear => Mark::Set,
    })
  }

  fn leaves(self) -> Result<Presence, Never> {
    Ok(match self {
      Mark::Set => Presence::Present,
      Mark::Clear => Presence::Absent,
    })
  }

  fn apply(self, word: &mut u64, Mask(bits): Mask) -> Result<(), Never> {
    match self {
      Mark::Set => *word |= bits,
      Mark::Clear => *word &= !bits,
    }

    Ok(())
  }
}

#[derive(Clone, Copy)]
struct Bit(u32);

#[derive(Clone, Copy)]
struct Mask(u64);

struct Run {
  first: Bit,
  last: Bit,
}

struct Address {
  word: u32,
  bit: Bit,
}

impl Bit {
  fn within(offset: u32) -> Result<Self, Never> {
    Ok(Self(offset % WORD_WIDTH))
  }

  fn mask(self) -> Result<Mask, Never> {
    Ok(Mask(match 1u64.checked_shl(self.0) {
      Some(mask) => mask,
      None => 0,
    }))
  }
}

impl Run {
  fn mask(&self) -> Result<Mask, Never> {
    let Ok(Mask(low)) = self.first.mask();
    let Ok(Mask(high)) = self.last.mask();

    Ok(Mask(high.saturating_sub(low) | high))
  }
}

impl Address {
  fn of(value: u32) -> Result<Self, Never> {
    let offset = value & OFFSET_MASK;
    let Ok(bit) = Bit::within(offset);

    Ok(Self {
      word: offset / WORD_WIDTH,
      bit,
    })
  }
}

fn presence(bits: u64) -> Result<Presence, Never> {
  Ok(match bits {
    0 => Presence::Absent,
    _held => Presence::Present,
  })
}

impl Page {
  #[cfg(feature = "serde")]
  pub(super) fn from_words(words: [u64; 8]) -> Result<Self, Never> {
    Ok(Self { words })
  }

  #[cfg(feature = "serde")]
  pub(super) fn words(&self) -> Result<[u64; 8], Never> {
    Ok(self.words)
  }

  pub(super) fn len(&self) -> Result<u32, Never> {
    Ok(self.words.iter().fold(0u32, |length, word| {
      length.saturating_add(word.count_ones())
    }))
  }

  pub(super) fn is_empty(&self) -> Result<Emptiness, Never> {
    Ok(Emptiness::from(self.words.iter().all(|word| *word == 0)))
  }

  pub(super) fn contains(&self, value: u32) -> Result<Presence, Never> {
    let Ok(address) = Address::of(value);
    let Ok(at) = index(address.word);
    let Ok(Mask(mask)) = address.bit.mask();

    match self.words.get(at) {
      Some(word) => presence(*word & mask),
      None => Ok(Presence::Absent),
    }
  }

  pub(super) fn mark(&mut self, value: u32, mark: Mark) -> Result<Presence, Never> {
    let Ok(address) = Address::of(value);
    let Ok(at) = index(address.word);
    let Ok(bit) = address.bit.mask();

    match self.words.get_mut(at) {
      Some(word) => {
        let Ok(before) = presence(*word & bit.0);
        let Ok(()) = mark.apply(word, bit);

        Ok(before)
      }
      None => mark.leaves(),
    }
  }

  pub(super) fn mark_range(
    &mut self,
    range: RangeInclusive<u32>,
    mark: Mark,
  ) -> Result<(), Never> {
    let low = *range.start() & OFFSET_MASK;
    let high = *range.end() & OFFSET_MASK;

    for (start, word) in WORD_STARTS.iter().zip(self.words.iter_mut()) {
      let from = low.max(*start);
      let to = high.min(*start | WORD_LAST);

      match from.cmp(&to) {
        Ordering::Less | Ordering::Equal => {
          let Ok(first) = Bit::within(from);
          let Ok(last) = Bit::within(to);
          let Ok(mask) = Run { first, last }.mask();
          let Ok(()) = mark.apply(word, mask);
        }
        Ordering::Greater => {}
      }
    }

    Ok(())
  }

  pub(super) fn union(&mut self, other: &Self) -> Result<(), Never> {
    for (word, added) in self.words.iter_mut().zip(&other.words) {
      *word |= *added;
    }

    Ok(())
  }

  pub(super) fn intersect(&mut self, other: &Self) -> Result<(), Never> {
    for (word, retained) in self.words.iter_mut().zip(&other.words) {
      *word &= *retained;
    }

    Ok(())
  }

  pub(super) fn subtract(&mut self, other: &Self) -> Result<(), Never> {
    for (word, removed) in self.words.iter_mut().zip(&other.words) {
      *word &= !*removed;
    }

    Ok(())
  }

  pub(super) fn intersects(&self, other: &Self) -> Result<Overlap, Never> {
    Ok(Overlap::from(
      self
        .words
        .iter()
        .zip(&other.words)
        .any(|(mine, theirs)| *mine & *theirs != 0),
    ))
  }

  pub(super) fn iter(&self) -> Result<impl DoubleEndedIterator<Item = u32> + '_, Never> {
    Ok(
      WORD_STARTS
        .iter()
        .zip(&self.words)
        .flat_map(|(start, word)| WordValues {
          remaining: *word,
          start: *start,
        }),
    )
  }

  pub(super) fn ranges(&self) -> Result<impl Iterator<Item = RangeInclusive<u32>> + '_, Never> {
    Ok(
      WORD_STARTS
        .iter()
        .zip(&self.words)
        .flat_map(|(start, word)| WordRanges {
          remaining: *word,
          start: *start,
        }),
    )
  }
}

struct WordValues {
  remaining: u64,
  start: u32,
}

impl WordValues {
  fn take(&mut self, bit: Bit) -> Result<u32, Never> {
    let Ok(Mask(mask)) = bit.mask();

    self.remaining &= !mask;

    Ok(self.start | bit.0)
  }
}

impl Iterator for WordValues {
  type Item = u32;

  fn next(&mut self) -> Option<u32> {
    let remaining = NonZeroU64::new(self.remaining)?;
    let Ok(lowest) = Bit::within(remaining.trailing_zeros());
    let Ok(value) = self.take(lowest);

    Some(value)
  }
}

impl DoubleEndedIterator for WordValues {
  fn next_back(&mut self) -> Option<u32> {
    let remaining = NonZeroU64::new(self.remaining)?;
    let Ok(highest) = Bit::within(remaining.ilog2());
    let Ok(value) = self.take(highest);

    Some(value)
  }
}

struct WordRanges {
  remaining: u64,
  start: u32,
}

impl Iterator for WordRanges {
  type Item = RangeInclusive<u32>;

  fn next(&mut self) -> Option<Self::Item> {
    let remaining = NonZeroU64::new(self.remaining)?;
    let Ok(first) = Bit::within(remaining.trailing_zeros());
    let Ok(Mask(lowest)) = first.mask();
    let filled = self.remaining | lowest.saturating_sub(1);
    let Ok(last) = Bit::within(filled.trailing_ones().saturating_sub(1));
    let run = Run { first, last };
    let Ok(Mask(mask)) = run.mask();

    self.remaining &= !mask;

    Some((self.start | run.first.0)..=(self.start | run.last.0))
  }
}
