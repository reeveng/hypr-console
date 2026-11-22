//! The CRC every Ogg page carries over its own header and content.
//!
//! It is a CRC32 by polynomial, `0x04C11DB7`, and nothing else about it is
//! the one a zip entry or a PNG chunk carries: bits go in from the high end,
//! the register begins at nought and is not inverted at either end. So the
//! two give different answers for the same bytes and neither can stand in for
//! the other, which is why this is a module of its own rather than a spelling
//! of `crc32`. `123456789` checks to `0x89A1897F`.
//!
//! The metadata reader and playback demuxer share the same incremental
//! arithmetic. Immutable sliced tables derive from the one polynomial table;
//! whole-buffer updates take eight bytes at a time.

use crate::Checksum;
use console_core_never::Never;

pub use crate::ogg_stream::Crc32;

pub fn of(bytes: &[u8]) -> Result<u32, Never> {
  let Ok(mut state) = Crc32::new(0);
  let Ok(()) = state.process_buf_bytes(bytes);

  state.crc()
}

#[cfg(test)]
mod tests {
  use super::*;
  use alloc::vec::Vec;

  const POLYNOMIAL: u32 = 0x04C1_1DB7;

  const HIGH_BIT: u32 = 0x8000_0000;

  #[test]
  fn every_table_entry_matches_its_polynomial() {
    for (byte, entry) in (0u32..=255).zip(crate::tables::OGG.iter()) {
      let expected = (0..8).fold(byte.wrapping_shl(24), |register, _bit| {
        match register & HIGH_BIT {
          0 => register.wrapping_shl(1),
          _high => register.wrapping_shl(1) ^ POLYNOMIAL,
        }
      });

      assert_eq!(*entry, expected, "byte {byte}");
    }
  }

  #[test]
  fn the_published_vector_is_the_one_this_answers() {
    assert_eq!(of(b"123456789"), Ok(0x89A1_897F));
  }

  #[test]
  fn nothing_at_all_checks_to_nothing() {
    assert_eq!(of(b""), Ok(0));
  }

  #[test]
  fn it_is_not_the_crc_a_zip_entry_carries() {
    assert_ne!(of(b"123456789"), crate::crc32::of(b"123456789"));
  }

  #[test]
  fn a_table_is_the_same_as_a_bit_at_a_time() {
    let bytes: Vec<u8> = (0u32..1000)
      .map(|at| {
        let [_, _, _, high] = at.wrapping_mul(2_654_435_761).to_le_bytes();

        high
      })
      .collect();

    let slow = bytes.iter().fold(0u32, |crc, byte| {
      (0..8).fold(
        crc ^ u32::from(*byte).wrapping_shl(24),
        |register, _bit| match register & HIGH_BIT {
          0 => register.wrapping_shl(1),
          _high => register.wrapping_shl(1) ^ POLYNOMIAL,
        },
      )
    });

    assert_eq!(of(&bytes), Ok(slow));
  }
}
