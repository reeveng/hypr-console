//! Bits read against the chance that each is a nought.
//!
//! VP8 writes everything after a frame's first bytes with a boolean entropy
//! coder, and VP9 kept the coder when it changed everything around it. Each
//! bit comes with the chance, out of 256, that it is a nought, and the coder
//! spends less than a bit on one that went the likely way. Reading is
//! splitting a range at that chance and asking which side of the split the
//! value read so far falls on; the range is kept between 128 and 255 by
//! shifting it, and the value with it, a bit at a time.
//!
//! Two decoders held this. The WebP reader had libwebp's, which is RFC 6386's
//! with the range held less one and seven bytes taken at a time while there
//! are eight left. The VP9 decoder had its own window over the same
//! arithmetic, with the split chosen by a mask where this one branches. They
//! answer the same bit for the same bytes, which the test beside this holds
//! against the specification's own slow reading, so there is one.
//!
//! Past the end of its bytes it reads noughts, as both specifications say,
//! and remembers having done so. What that means is the caller's: a WebP
//! picture that needed them is cut short and says so, and a VP9 frame goes
//! on, because its specification does. The reader libwebp has stops shifting
//! after the first byte it did not have, which is harmless where the picture
//! is refused and wrong where it is not, so this one keeps reading noughts.
//!
//! What a bit means stays with whoever's syntax it is. A marker bit before
//! the first one, a tree walked by its answers, a magnitude and then its
//! sign: none of those are here.

use console_core_never::Never;
use console_core_number_conversion::index;

const TAKEN: u32 = 56;

const WINDOW: u32 = 8;

const SEVEN_BYTES: u32 = 7;

const NORMAL: u32 = 24;

pub const EVEN: u8 = 0x80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PastTheEnd;

impl std::fmt::Display for PastTheEnd {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str("the bits ran out before the reading did")
  }
}

impl std::error::Error for PastTheEnd {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reachability {
  Within,
  Past,
}

pub struct Booleans<'a> {
  bytes: &'a [u8],
  at: u32,
  value: u64,
  count: u32,
  range: u32,
  reach: Reachability,
}

impl<'a> Booleans<'a> {
  pub fn new(bytes: &'a [u8]) -> Result<Booleans<'a>, Never> {
    let mut booleans = Booleans {
      bytes,
      at: 0,
      value: 0,
      count: 0,
      range: 254,
      reach: Reachability::Within,
    };
    let Ok(()) = booleans.loaded();

    Ok(booleans)
  }

  #[inline(always)]
  fn loaded(&mut self) -> Result<(), Never> {
    let Ok(at) = index(self.at);

    match self.bytes.get(at..).and_then(<[u8]>::first_chunk::<8>) {
      Some(eight) => {
        self.value =
          self.value.wrapping_shl(TAKEN) | u64::from_be_bytes(*eight).wrapping_shr(WINDOW);
        self.count = self.count.wrapping_add(TAKEN);
        self.at = self.at.wrapping_add(SEVEN_BYTES);

        Ok(())
      }
      None => self.last_loaded(),
    }
  }

  #[inline(never)]
  fn last_loaded(&mut self) -> Result<(), Never> {
    let Ok(at) = index(self.at);

    self.count = self.count.wrapping_add(WINDOW);

    match self.bytes.get(at) {
      Some(byte) => {
        self.value = self.value.wrapping_shl(WINDOW) | u64::from(*byte);
        self.at = self.at.wrapping_add(1);
      }
      None => {
        self.value = self.value.wrapping_shl(WINDOW);
        self.reach = Reachability::Past;
      }
    }

    Ok(())
  }

  #[inline(always)]
  pub fn bit(&mut self, chance: u8) -> Result<u32, Never> {
    let mut range = self.range;

    match self.count {
      0..WINDOW => {
        let Ok(()) = self.loaded();
      }
      WINDOW.. => {}
    }

    let position = self.count.wrapping_sub(WINDOW);
    let split = range.wrapping_mul(u32::from(chance)).wrapping_shr(WINDOW);

    let bit = match self.value.wrapping_shr(position).cmp(&u64::from(split)) {
      core::cmp::Ordering::Greater => {
        range = range.wrapping_sub(split);
        self.value = self
          .value
          .wrapping_sub(u64::from(split.wrapping_add(1)).wrapping_shl(position));

        1
      }
      core::cmp::Ordering::Less | core::cmp::Ordering::Equal => {
        range = split.wrapping_add(1);

        0
      }
    };

    let shift = range.leading_zeros().wrapping_sub(NORMAL);

    self.range = range.wrapping_shl(shift).wrapping_sub(1);
    self.count = self.count.wrapping_sub(shift);

    Ok(bit)
  }

  pub fn even(&mut self) -> Result<u32, Never> {
    self.bit(EVEN)
  }

  pub fn number(&mut self, bits: u32) -> Result<u32, Never> {
    let mut number = 0u32;

    for _bit in 0..bits {
      let Ok(bit) = self.even();

      number = number.wrapping_shl(1) | bit;
    }

    Ok(number)
  }

  pub fn within(&self) -> Result<(), PastTheEnd> {
    match self.reach {
      Reachability::Within => Ok(()),
      Reachability::Past => Err(PastTheEnd),
    }
  }
}
