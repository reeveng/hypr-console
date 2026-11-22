//! The padding FIPS 180-4 gives every SHA-2 message before the first block.
//!
//! A one bit, as many zeros as bring the message to a length a block short
//! of its length field, and the length in bits, big-endian. SHA-256 counts
//! the length in eight bytes and SHA-512 in sixteen, and that width and the
//! block's are the only two numbers in which the two paddings differ, so they
//! are the two this is told. A length that will not fit the widest count
//! is not a slice this machine could hold, so `fitted`'s saturation is never
//! the answer anybody reads.

use alloc::vec::Vec;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

const WIDEST: u64 = 16;

pub(crate) fn padded<const BLOCK: usize, const LENGTH: usize>(
  data: &[u8],
) -> Result<Vec<u8>, Never> {
  let Ok(block) = fitted::<_, u64>(BLOCK);
  let Ok(length) = fitted::<_, u64>(LENGTH);
  let mut held = data.to_vec();

  held.push(0x80);

  let Ok(written) = fitted::<_, u64>(held.len());
  let tail = match written.checked_rem(block) {
    Some(tail) => tail,
    None => 0,
  };

  let room = block.saturating_sub(length);
  let zeros = match room
    .saturating_add(block)
    .saturating_sub(tail)
    .checked_rem(block)
  {
    Some(zeros) => zeros,
    None => 0,
  };

  let Ok(zeroed) = index(zeros);

  held.extend(core::iter::repeat_n(0u8, zeroed));

  let Ok(message) = fitted::<_, u128>(data.len());
  let bits = message.wrapping_mul(8).to_be_bytes();
  let Ok(skipped) = index(WIDEST.saturating_sub(length));

  held.extend(bits.iter().skip(skipped));

  Ok(held)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_message_is_padded_to_whole_blocks_with_its_length_in_bits_last() {
    let Ok(short) = padded::<64, 8>(b"abc");
    assert_eq!(short.len(), 64);
    assert_eq!(short.get(3), Some(&0x80));
    assert_eq!(
      short.get(56..),
      Some([0u8, 0, 0, 0, 0, 0, 0, 24].as_slice())
    );

    let Ok(wide) = padded::<128, 16>(b"abc");
    assert_eq!(wide.len(), 128);
    assert_eq!(wide.get(127), Some(&24));
  }

  #[test]
  fn a_message_that_ends_where_the_length_goes_takes_a_block_of_its_own() {
    let Ok(fills) = padded::<64, 8>(&[0; 55]);
    assert_eq!(fills.len(), 64);

    let Ok(spills) = padded::<64, 8>(&[0; 56]);
    assert_eq!(spills.len(), 128);
  }
}
