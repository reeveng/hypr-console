//! Incremental checksums over bytes handed in by a caller.
//!
//! Adapted from Symphonia, copyright 2019-2022 The Project Symphonia
//! Developers, under the Mozilla Public License 2.0 kept beside this crate.

use console_core_never::Never;

pub trait Checksum {
  fn process_byte(&mut self, byte: u8) -> Result<(), Never>;

  #[inline(always)]
  fn process_double_bytes(&mut self, buf: [u8; 2]) -> Result<(), Never> {
    self.process_buf_bytes(&buf)
  }

  #[inline(always)]
  fn process_triple_bytes(&mut self, buf: [u8; 3]) -> Result<(), Never> {
    self.process_buf_bytes(&buf)
  }

  #[inline(always)]
  fn process_quad_bytes(&mut self, buf: [u8; 4]) -> Result<(), Never> {
    self.process_buf_bytes(&buf)
  }

  fn process_buf_bytes(&mut self, buf: &[u8]) -> Result<(), Never> {
    for byte in buf {
      let Ok(()) = self.process_byte(*byte);
    }

    Ok(())
  }
}
