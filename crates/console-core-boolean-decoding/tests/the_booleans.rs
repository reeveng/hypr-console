//! The reader held against the specification's own reading, a bit at a time.
//!
//! `Slowly` is section 9.2 of the VP9 specification written out as it is
//! printed: a value eight bits wide, a range, and one bit shifted in for every
//! doubling. It is the reading both decoders were written to agree with, so
//! agreeing with it over bytes of noise, at every chance and well past the
//! end of them, is what says the one reader does the work of the two.

use console_core_boolean_decoding::{Booleans, EVEN, PastTheEnd};
use console_core_never::Never;
use console_core_number_conversion::index;

const LOWEST_RANGE_BITS: u32 = 24;

struct Noise(u32);

impl Noise {
  fn next(&mut self) -> Result<u32, Never> {
    self.0 ^= self.0.wrapping_shl(13);
    self.0 ^= self.0.wrapping_shr(17);
    self.0 ^= self.0.wrapping_shl(5);

    Ok(self.0)
  }

  fn byte(&mut self) -> Result<u8, Never> {
    let Ok(noise) = self.next();
    let [byte, _, _, _] = noise.to_le_bytes();

    Ok(byte)
  }

  fn chance(&mut self) -> Result<u8, Never> {
    let Ok(byte) = self.byte();

    Ok(byte.max(1))
  }

  fn bytes(&mut self, length: u32) -> Result<Vec<u8>, Never> {
    let mut bytes = Vec::new();

    for _byte in 0..length {
      let Ok(byte) = self.byte();

      bytes.push(byte);
    }

    Ok(bytes)
  }
}

struct Slowly<'a> {
  bytes: &'a [u8],
  at: u32,
  value: u32,
  range: u32,
}

impl<'a> Slowly<'a> {
  fn begun(bytes: &'a [u8]) -> Result<Slowly<'a>, Never> {
    let mut slowly = Slowly {
      bytes,
      at: 0,
      value: 0,
      range: 255,
    };

    for _bit in 0..8 {
      let Ok(bit) = slowly.next();

      slowly.value = slowly.value.wrapping_shl(1) | bit;
    }

    Ok(slowly)
  }

  fn next(&mut self) -> Result<u32, Never> {
    let Ok(byte) = index(self.at.wrapping_shr(3));
    let held = match self.bytes.get(byte) {
      Some(held) => *held,
      None => 0,
    };
    let bit = u32::from(held).wrapping_shr(7u32.wrapping_sub(self.at & 7)) & 1;

    self.at = self.at.wrapping_add(1);

    Ok(bit)
  }

  fn bit(&mut self, chance: u8) -> Result<u32, Never> {
    let split = self
      .range
      .wrapping_sub(1)
      .wrapping_mul(u32::from(chance))
      .wrapping_shr(8)
      .wrapping_add(1);

    let bit = match self.value.cmp(&split) {
      core::cmp::Ordering::Less => {
        self.range = split;

        0
      }
      core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {
        self.range = self.range.wrapping_sub(split);
        self.value = self.value.wrapping_sub(split);

        1
      }
    };

    for _doubling in 0..self.range.leading_zeros().wrapping_sub(LOWEST_RANGE_BITS) {
      let Ok(shifted_in) = self.next();

      self.range = self.range.wrapping_shl(1);
      self.value = self.value.wrapping_shl(1) | shifted_in;
    }

    Ok(bit)
  }
}

#[test]
fn every_bit_is_the_one_the_specification_reads() {
  let mut noise = Noise(0x1234_5678);

  for length in [0u32, 1, 2, 3, 7, 8, 9, 15, 16, 17, 100, 1000] {
    let Ok(bytes) = noise.bytes(length);
    let Ok(mut booleans) = Booleans::new(&bytes);
    let Ok(mut slowly) = Slowly::begun(&bytes);

    for read in 0..length.saturating_mul(12).saturating_add(64) {
      let Ok(chance) = noise.chance();

      assert_eq!(
        booleans.bit(chance),
        slowly.bit(chance),
        "{length} bytes, bit {read}, chance {chance}"
      );
    }
  }
}

#[test]
fn past_the_end_every_bit_is_a_nought_however_far() {
  let Ok(mut booleans) = Booleans::new(&[]);

  for _bit in 0..4096 {
    assert_eq!(booleans.even(), Ok(0));
  }

  assert_eq!(booleans.within(), Err(PastTheEnd));
}

#[test]
fn reading_what_is_there_is_within() {
  let Ok(mut booleans) = Booleans::new(&[0x00, 0x00, 0x00]);

  assert_eq!(booleans.number(8), Ok(0));
  assert_eq!(booleans.within(), Ok(()));
}

#[test]
fn a_bit_certain_to_be_a_nought_is_one_only_when_the_value_is_at_the_top() {
  let Ok(mut booleans) = Booleans::new(&[0xFF, 0xFF]);

  assert_eq!(booleans.bit(1), Ok(1));
}

#[test]
fn even_bits_spell_the_number_highest_first() {
  let Ok(mut plain) = Booleans::new(&[0xA5, 0x00, 0x00, 0x00]);
  let Ok(mut again) = Booleans::new(&[0xA5, 0x00, 0x00, 0x00]);
  let mut spelled = 0u32;

  for _bit in 0..6 {
    let Ok(bit) = again.bit(EVEN);

    spelled = spelled.wrapping_shl(1) | bit;
  }

  assert_eq!(plain.number(6), Ok(spelled));
}
