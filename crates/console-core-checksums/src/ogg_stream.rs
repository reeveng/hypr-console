//! Incremental, non-reflected CRC32 for Ogg pages, using polynomial 0x04c11db7.
//!
//! Adapted from Symphonia, copyright 2019-2022 The Project Symphonia
//! Developers, under the Mozilla Public License 2.0 kept beside this crate.

use console_core_never::Never;

use crate::Checksum;
use crate::crc32::looked_up;

pub struct Crc32 {
  state: u32,
}

impl Crc32 {
  pub fn new(state: u32) -> Result<Self, Never> {
    Ok(Crc32 { state })
  }

  pub fn crc(&self) -> Result<u32, Never> {
    Ok(self.state)
  }
}

impl Checksum for Crc32 {
  #[inline(always)]
  fn process_byte(&mut self, byte: u8) -> Result<(), Never> {
    let [high, ..] = self.state.to_be_bytes();
    let Ok(next) = looked_up(&crate::tables::OGG, high ^ byte);

    self.state = self.state.wrapping_shl(8) ^ next;

    Ok(())
  }

  fn process_buf_bytes(&mut self, buf: &[u8]) -> Result<(), Never> {
    let [t0, t1, t2, t3, t4, t5, t6, t7] = &crate::tables::OGG_SLICED;
    let (chunks, rest) = buf.as_chunks::<8>();

    for [a, b, c, d, e, f, g, h] in chunks {
      let word = u32::from_be_bytes([*a, *b, *c, *d]);
      let [w, x, y, z] = (self.state ^ word).to_be_bytes();
      let Ok(w_crc) = looked_up(t7, w);
      let Ok(x_crc) = looked_up(t6, x);
      let Ok(y_crc) = looked_up(t5, y);
      let Ok(z_crc) = looked_up(t4, z);
      let Ok(e_crc) = looked_up(t3, *e);
      let Ok(f_crc) = looked_up(t2, *f);
      let Ok(g_crc) = looked_up(t1, *g);
      let Ok(h_crc) = looked_up(t0, *h);

      self.state = w_crc ^ x_crc ^ y_crc ^ z_crc ^ e_crc ^ f_crc ^ g_crc ^ h_crc;
    }

    for byte in rest {
      let Ok(()) = self.process_byte(*byte);
    }

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_mpeg2_vectors_keep_their_initial_register() {
    for (bytes, expected) in [
      (b"".as_slice(), 0xffff_ffff),
      ([0u8].as_slice(), 0x4e08_bfb4),
      (b"123456789".as_slice(), 0x0376_e6e7),
      (
        b"abcdefghijklmnopqrstuvwxyz123456789".as_slice(),
        0x178d_c1e0,
      ),
    ] {
      let Ok(mut crc) = Crc32::new(0xffff_ffff);
      let Ok(()) = crc.process_buf_bytes(bytes);

      assert_eq!(crc.crc(), Ok(expected));
    }
  }

  #[test]
  fn the_same_register_is_reached_a_byte_at_a_time() {
    let Ok(mut crc) = Crc32::new(0xffff_ffff);

    for byte in b"123456789" {
      let Ok(()) = crc.process_byte(*byte);
    }

    assert_eq!(crc.crc(), Ok(0x0376_e6e7));
  }
}
