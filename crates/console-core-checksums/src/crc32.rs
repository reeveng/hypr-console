//! CRC32, in the one spelling both formats that name it use.
//!
//! A zip entry carries one and so does every chunk of a PNG, which is two
//! callers for the same reflected polynomial. It is the ordinary one --
//! `0xEDB88320`, bits in from the low end, the register begun and ended
//! inverted -- and `123456789` checks to `0xCBF43926`, which is the vector
//! every implementation of it is tested against.
//!
//! It is read eight bytes at a step, through eight tables of what each byte
//! does to the register from where it stands among the eight, rather than a
//! bit at a time. A PNG of a photograph is tens of megabytes of chunks, every
//! one of them checked, and a bit at a time thirty megabytes took a tenth of a
//! second; this takes a quarter of that. The tables are the polynomial's and
//! the same for every caller, so they are held as immutable constants with
//! nothing to initialize when the first checksum is asked for.
//!
//! `update` is the raw register, neither begun nor ended inverted, for a
//! caller that carries it across calls: begin it at `0xffff_ffff` and invert
//! what it ends on to have the checksum `of` answers.

use console_core_never::Never;
use console_core_number_conversion::index;

type Table = [u32; 256];

#[inline(always)]
pub(super) fn looked_up(table: &Table, byte: u8) -> Result<u32, Never> {
  let Ok(at) = index(byte);

  Ok(match table.get(at) {
    Some(entry) => *entry,
    None => 0,
  })
}

pub fn of(bytes: &[u8]) -> Result<u32, Never> {
  let Ok(register) = update(0xffff_ffff, bytes);
  Ok(!register)
}

pub fn update(register: u32, bytes: &[u8]) -> Result<u32, Never> {
  let [t0, t1, t2, t3, t4, t5, t6, t7] = &crate::tables::CRC32;
  let (eights, rest) = bytes.as_chunks::<8>();

  let chunks_crc = eights
    .iter()
    .fold(register, |prior, [a, b, c, d, e, f, g, h]| {
      let [w, x, y, z] = (prior ^ u32::from_le_bytes([*a, *b, *c, *d])).to_le_bytes();
      let Ok(w_crc) = looked_up(t7, w);
      let Ok(x_crc) = looked_up(t6, x);
      let Ok(y_crc) = looked_up(t5, y);
      let Ok(z_crc) = looked_up(t4, z);
      let Ok(e_crc) = looked_up(t3, *e);
      let Ok(f_crc) = looked_up(t2, *f);
      let Ok(g_crc) = looked_up(t1, *g);
      let Ok(h_crc) = looked_up(t0, *h);

      w_crc ^ x_crc ^ y_crc ^ z_crc ^ e_crc ^ f_crc ^ g_crc ^ h_crc
    });

  let crc = rest.iter().fold(chunks_crc, |register_2, byte| {
    let [low, ..] = register_2.to_le_bytes();
    let Ok(moved) = looked_up(t0, low ^ *byte);

    register_2.wrapping_shr(8) ^ moved
  });

  Ok(crc)
}

#[cfg(test)]
mod tests {
  use super::*;
  use alloc::vec::Vec;

  const POLYNOMIAL: u32 = 0xEDB8_8320;

  #[test]
  fn every_table_entry_matches_its_polynomial() {
    for (extra_bytes, table) in (0u32..8).zip(crate::tables::CRC32.iter()) {
      for (byte, entry) in (0u32..=255).zip(table) {
        let bits = extra_bytes.saturating_add(1).saturating_mul(8);
        let expected = (0..bits).fold(byte, |register, _bit| match register & 1 {
          1 => register.wrapping_shr(1) ^ POLYNOMIAL,
          _low => register.wrapping_shr(1),
        });

        assert_eq!(*entry, expected, "table {extra_bytes}, byte {byte}");
      }
    }
  }

  #[test]
  fn the_published_vector_is_the_one_this_answers() {
    assert_eq!(of(b"123456789"), Ok(0xCBF4_3926));
  }

  #[test]
  fn nothing_at_all_checks_to_nothing() {
    assert_eq!(of(b""), Ok(0));
  }

  #[test]
  fn one_byte_changed_is_a_different_answer() {
    let Ok(one) = of(b"console");
    let Ok(two) = of(b"consolf");

    assert_ne!(one, two);
  }

  #[test]
  fn eight_at_a_time_is_the_same_as_a_bit_at_a_time() {
    let bytes: Vec<u8> = (0u32..1000)
      .map(|at| {
        let [_, _, _, high] = at.wrapping_mul(2_654_435_761).to_le_bytes();

        high
      })
      .collect();

    for cut in [0, 1, 7, 8, 9, 63, 64, 999, 1000] {
      let checked = bytes.get(..cut).map(of);
      let slow = bytes.iter().take(cut).fold(0xFFFF_FFFFu32, |crc, byte| {
        (0..8).fold(crc ^ u32::from(*byte), |register, _bit| {
          match register & 1 {
            1 => register.wrapping_shr(1) ^ POLYNOMIAL,
            _low => register.wrapping_shr(1),
          }
        })
      });

      assert_eq!(checked, Some(Ok(!slow)), "{cut}");
    }
  }
}
