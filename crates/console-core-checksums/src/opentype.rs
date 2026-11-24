//! OpenType table checksum: sum of big-endian words with zero padding.
//!
//! The specification sums a table as unsigned 32-bit words modulo two to the
//! thirty-second, so `wrapping_add` is its policy. A table whose length is not
//! a multiple of four is read as though zeros filled its last word.

use console_core_never::Never;

pub fn words(table: &[u8]) -> Result<impl Iterator<Item = u32> + '_, Never> {
  let (quads, rest) = table.as_chunks::<4>();
  let last = match rest {
    [] => None,
    &[a] => Some(u32::from_be_bytes([a, 0, 0, 0])),
    &[a, b] => Some(u32::from_be_bytes([a, b, 0, 0])),
    &[a, b, c, ..] => Some(u32::from_be_bytes([a, b, c, 0])),
  };

  Ok(quads.iter().copied().map(u32::from_be_bytes).chain(last))
}

pub fn of(table: &[u8]) -> Result<u32, Never> {
  let Ok(words) = words(table);

  Ok(words.fold(0, u32::wrapping_add))
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
