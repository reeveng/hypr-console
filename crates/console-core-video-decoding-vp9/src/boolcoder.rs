//! Boolean (arithmetic) decoder of section 9.2.
//!
//! The arithmetic is `console-core-boolean-decoding`, which the WebP reader shares: VP9 kept
//! VP8's coder. What is here is VP9's own, the marker bit read at initialisation and the tree
//! walk. Reading past the end of the data yields zero bits, as the specification does once
//! `BoolMaxBits` is exhausted.
//!
//! A bit is answered as the number it is, a nought or a one, because most of what reads one
//! counts it into a table of two or adds it into a value, and a `bool` would have to be turned
//! back into the number at every one of those.

use console_core_boolean_decoding::Booleans;
use console_core_never::Never;

use crate::places::read;

pub struct BoolDecoder<'a> {
  bits: Booleans<'a>,
}

impl<'a> BoolDecoder<'a> {
  pub fn new(data: &'a [u8]) -> Result<Self, Never> {
    let Ok(mut bits) = Booleans::new(data);
    let Ok(_marker) = bits.even();

    Ok(BoolDecoder { bits })
  }

  #[inline(always)]
  pub fn read_bool(&mut self, probability: u8) -> Result<u32, Never> {
    self.bits.bit(probability)
  }

  pub fn read_literal(&mut self, bits: u32) -> Result<u32, Never> {
    self.bits.number(bits)
  }

  #[inline]
  pub fn read_tree(&mut self, tree: &[i8], probabilities: &[u8]) -> Result<u8, Never> {
    let mut node = 0_u8;

    for _step in tree {
      let Ok(probability) = read(probabilities, u64::from(node.wrapping_shr(1)));
      let Ok(bit) = self.read_bool(probability);
      let Ok(next) = read(tree, u64::from(node).wrapping_add(u64::from(bit)));

      match ::core::primitive::bool::then_some(next.is_positive(), ()) {
        ::core::option::Option::Some(()) => node = next.unsigned_abs(),
        ::core::option::Option::None => return Ok(next.unsigned_abs()),
      }
    }

    Ok(0)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::places::length;
  use crate::testing::Xorshift;

  struct SpecDecoder<'a> {
    data: &'a [u8],
    position: u64,
    value: u32,
    range: u32,
    remaining: u64,
  }

  impl<'a> SpecDecoder<'a> {
    fn new(data: &'a [u8]) -> Result<Self, Never> {
      let Ok(bytes) = length(data);
      let mut decoder = SpecDecoder {
        data,
        position: 0,
        value: 0,
        range: 255,
        remaining: bytes.wrapping_mul(8).saturating_sub(8),
      };

      for _bit in 0..8 {
        let Ok(bit) = decoder.bit();

        decoder.value = decoder.value.wrapping_shl(1) | bit;
      }

      let Ok(_marker) = decoder.read(128);

      Ok(decoder)
    }

    fn bit(&mut self) -> Result<u32, Never> {
      let Ok(byte) = read(self.data, self.position.wrapping_shr(3));
      let Ok(shift) =
        console_core_number_conversion::fitted::<u64, u32>(7_u64.wrapping_sub(self.position & 7));

      self.position = self.position.wrapping_add(1);

      Ok(u32::from(byte.wrapping_shr(shift) & 1))
    }

    fn read(&mut self, probability: u8) -> Result<u32, Never> {
      let split = self
        .range
        .wrapping_sub(1)
        .wrapping_mul(u32::from(probability))
        .wrapping_shr(8)
        .wrapping_add(1);

      let bit = match ::core::primitive::bool::then_some(self.value < split, ()) {
        ::core::option::Option::Some(()) => {
          self.range = split;

          0
        }
        ::core::option::Option::None => {
          self.range = self.range.wrapping_sub(split);
          self.value = self.value.wrapping_sub(split);

          1
        }
      };

      for _doubling in 0..self.range.leading_zeros().saturating_sub(24) {
        let next = match self.remaining {
          0 => 0,
          _left => {
            self.remaining = self.remaining.wrapping_sub(1);

            let Ok(next) = self.bit();

            next
          }
        };

        self.range = self.range.wrapping_shl(1);
        self.value = self.value.wrapping_shl(1).wrapping_add(next);
      }

      Ok(bit)
    }
  }

  #[test]
  fn matches_spec_model() {
    let mut random = Xorshift(0x1234_5678);

    for bytes in [1_u64, 2, 3, 7, 8, 9, 15, 16, 17, 100, 1000] {
      let mut data = Vec::new();

      for _byte in 0..bytes {
        let Ok(byte) = random.byte();

        data.push(byte);
      }

      let Ok(mut fast) = BoolDecoder::new(&data);
      let Ok(mut spec) = SpecDecoder::new(&data);

      for read_so_far in 0..bytes.wrapping_mul(12) {
        let Ok(drawn) = random.next();
        let Ok(probability) = console_core_number_conversion::fitted::<u32, u8>(
          drawn.wrapping_rem(255).wrapping_add(1),
        );

        assert_eq!(
          fast.read_bool(probability),
          spec.read(probability),
          "length {bytes} bit {read_so_far}"
        );
      }
    }
  }
}
