//! OpenType table checksum: sum of big-endian words with zero padding.
//!
//! The specification sums a table as unsigned 32-bit words modulo two to the
//! thirty-second, so `wrapping_add` is its policy. A table whose length is not
//! a multiple of four is read as though zeros filled its last word.

use console_core_never::Never;

pub fn of(table: &[u8]) -> Result<u32, Never> {
  let (quads, rest) = table.as_chunks::<4>();
  let sum = quads.iter().fold(0u32, |sum, quad| {
    sum.wrapping_add(u32::from_be_bytes(*quad))
  });
  let mut last = [0u8; 4];

  for (slot, byte) in last.iter_mut().zip(rest) {
    *slot = *byte;
  }

  Ok(sum.wrapping_add(u32::from_be_bytes(last)))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn sums_big_endian_words_and_zero_pads_each_possible_tail() {
    assert_eq!(of(&[]), Ok(0));
    assert_eq!(of(&[1]), Ok(0x0100_0000));
    assert_eq!(of(&[1, 2]), Ok(0x0102_0000));
    assert_eq!(of(&[1, 2, 3]), Ok(0x0102_0300));
    assert_eq!(of(&[1, 2, 3, 4, 5]), Ok(0x0602_0304));
    assert_eq!(of(&[255, 255, 255, 255, 0, 0, 0, 1]), Ok(0));
  }
}
