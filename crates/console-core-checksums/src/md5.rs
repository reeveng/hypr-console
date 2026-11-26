//! MD5 for the thumbnail standard and FLAC's decoded-sample verification.
//!
//! Whole-message and incremental hashing use the same compression kernel.
//! `digest` is the raw sixteen bytes of any message, UTF-8 or not, which is
//! what PDF encryption asks for; `of` is the hex of a string, which is what
//! the thumbnail standard files a picture under.

use alloc::string::String;

use crate::Checksum;
use console_core_bytes::hex::{self, Case};
use console_core_never::Never;

pub use crate::streaming_md5::Md5;

pub fn digest(bytes: &[u8]) -> Result<[u8; 16], Never> {
  let mut state = Md5::default();
  let Ok(()) = state.process_buf_bytes(bytes);
  state.md5()
}

pub fn of(said: &str) -> Result<String, Never> {
  let Ok(hash) = digest(said.as_bytes());
  hex::encode(&hash, Case::Lower)
}

#[cfg(test)]
mod tests {
  use super::*;
  use alloc::string::ToString;

  #[test]
  fn the_vectors_rfc_1321_ends_with() {
    assert_eq!(of(""), Ok("d41d8cd98f00b204e9800998ecf8427e".to_string()));
    assert_eq!(of("a"), Ok("0cc175b9c0f1b6a831c399e269772661".to_string()));
    assert_eq!(
      of("abc"),
      Ok("900150983cd24fb0d6963f7d28e17f72".to_string())
    );
    assert_eq!(
      of("message digest"),
      Ok("f96b697d7cb7938d525a2f31aaf161d0".to_string())
    );
    assert_eq!(
      of("abcdefghijklmnopqrstuvwxyz"),
      Ok("c3fcd3d76192e4007dfb496cca67e13b".to_string())
    );
    assert_eq!(
      of("12345678901234567890123456789012345678901234567890123456789012345678901234567890"),
      Ok("57edf4a22be3c955ac49da2e2107b67a".to_string())
    );
  }

  #[test]
  fn a_message_that_ends_where_the_length_goes_takes_a_block_of_its_own() {
    assert_eq!(
      of("12345678901234567890123456789012345678901234567890123"),
      Ok("cc741494163f0816c4c0ab6502553ac9".to_string())
    );
    assert_eq!(
      of("123456789012345678901234567890123456789012345678901234"),
      Ok("f40a0ec3fbf6cf062c9faf3752bd6e6c".to_string())
    );
  }
}
