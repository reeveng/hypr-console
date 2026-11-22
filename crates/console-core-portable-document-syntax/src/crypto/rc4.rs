//! Ported from <https://github.com/mozilla/pdf.js/blob/master/src/core/crypto.js>.

use crate::crypto::DecryptionError::{self, InvalidEncryption};
use alloc::vec::Vec;
use console_core_number_conversion::index;

#[derive(Clone)]
pub(crate) struct Rc4 {
  a: u8,
  b: u8,
  s: [u8; 256],
}

impl Rc4 {
  pub(crate) fn new(key: &[u8]) -> Result<Self, DecryptionError> {
    let mut cipher = Self {
      a: 0,
      b: 0,
      s: [0_u8; 256],
    };

    for (value, slot) in (0..=u8::MAX).zip(&mut cipher.s) {
      *slot = value;
    }

    let mut other = 0_u8;

    match key {
      [] => {
        return Err(InvalidEncryption);
      }
      [_, ..] => {}
    }

    let mut key_bytes = key.iter();

    for slot in 0..=u8::MAX {
      let key_byte = match key_bytes.next() {
        Some(byte) => byte,
        None => {
          key_bytes = key.iter();
          key_bytes.next().ok_or(InvalidEncryption)?
        }
      };
      let current = cipher.byte_at(slot)?;
      other = other.wrapping_add(current).wrapping_add(*key_byte);
      cipher.swap([slot, other])?;
    }

    Ok(cipher)
  }

  fn byte_at(&self, slot: u8) -> Result<u8, DecryptionError> {
    let Ok(position) = index(slot);
    self.s.get(position).copied().ok_or(InvalidEncryption)
  }

  fn swap(&mut self, [first, second]: [u8; 2]) -> Result<(), DecryptionError> {
    let Ok(from) = index(first);
    let Ok(to) = index(second);
    let left = self.byte_at(first)?;
    let right = self.byte_at(second)?;
    let first_slot = self.s.get_mut(from).ok_or(InvalidEncryption)?;
    *first_slot = right;
    let second_slot = self.s.get_mut(to).ok_or(InvalidEncryption)?;
    *second_slot = left;
    Ok(())
  }

  pub(crate) fn decrypt(&mut self, data: &[u8]) -> Result<Vec<u8>, DecryptionError> {
    let mut output = Vec::new();

    for byte in data {
      self.a = self.a.wrapping_add(1);
      let first = self.byte_at(self.a)?;
      self.b = self.b.wrapping_add(first);
      let second = self.byte_at(self.b)?;
      self.swap([self.a, self.b])?;
      let stream_byte = self.byte_at(first.wrapping_add(second))?;
      output.push(*byte ^ stream_byte);
    }

    Ok(output)
  }

  pub(crate) fn encrypt(&mut self, data: &[u8]) -> Result<Vec<u8>, DecryptionError> {
    self.decrypt(data)
  }
}

#[cfg(test)]
mod rc4_tests {
  use super::*;

  fn rc4_decrypt(key: &[u8], input: &[u8]) -> Result<Vec<u8>, DecryptionError> {
    let mut cipher = Rc4::new(key)?;

    cipher.decrypt(input)
  }

  #[test]
  fn correctness() {
    assert_eq!(rc4_decrypt(b"a", &[0x68]), Ok(b"x".to_vec()));
    assert_eq!(
      rc4_decrypt(b"key", &[0x7F, 0x09, 0x47, 0x99]),
      Ok(b"test".to_vec())
    );
    assert_eq!(
      rc4_decrypt(b"hello", &[0x78, 0x3E, 0xCD, 0x96, 0xCF]),
      Ok(b"world".to_vec())
    );
    assert_eq!(
      rc4_decrypt(b"\x01\x02", &[0x0C, 0x74, 0xB9]),
      Ok(b"Hi!".to_vec())
    );
    assert_eq!(
      rc4_decrypt(b"secret", &[0x80, 0x45, 0xB5]),
      Ok(b"msg".to_vec())
    );

    assert_eq!(
      rc4_decrypt(
        b"encryption",
        &[
          0x8A, 0x36, 0x3F, 0x85, 0xDB, 0x9A, 0x62, 0x7C, 0x6C, 0x56, 0x81, 0x89
        ]
      ),
      Ok(b"Hello World!".to_vec())
    );
    assert_eq!(
      rc4_decrypt(
        b"my_secret_key",
        &[
          0x1D, 0xE2, 0xCE, 0x64, 0x4C, 0x88, 0x1C, 0x42, 0x7D, 0x94, 0x7B, 0x1C, 0x49, 0xCD,
          0x62, 0x3F, 0xCA, 0x90, 0x99
        ]
      ),
      Ok(b"The quick brown fox".to_vec())
    );
    assert_eq!(
      rc4_decrypt(
        b"abcdefghijklmnop",
        &[
          0xF8, 0xA8, 0x2A, 0xAE, 0x92, 0x2F, 0x8C, 0x3B, 0x13, 0xC7, 0x4B, 0x99, 0xDB, 0x41,
          0x2C, 0x9F, 0x92, 0x59, 0xC2, 0xE5, 0x62, 0x2C
        ]
      ),
      Ok(b"This is a test message".to_vec())
    );
    assert_eq!(
      rc4_decrypt(
        b"password123",
        &[
          0xDB, 0xC3, 0xA3, 0x21, 0x7E, 0x55, 0x0B, 0x65, 0xA2, 0x3F, 0x0E, 0xB7, 0x58, 0x7C,
          0x28, 0xEE, 0xD6, 0x92, 0x05, 0xBC, 0x6C
        ]
      ),
      Ok(b"Decryption successful".to_vec())
    );
    assert_eq!(
      rc4_decrypt(
        b"very_long_key",
        &[
          0xC6, 0xD2, 0x00, 0x3B, 0x8C, 0x18, 0x49, 0x2C, 0xD0, 0x5C, 0x8A, 0x34, 0xEC, 0xFE,
          0xED, 0x62, 0xC0, 0x3C, 0x80, 0x81, 0x7E, 0x4E, 0xE1, 0x7D, 0xBD
        ]
      ),
      Ok(b"PDF uses RC4 for security".to_vec())
    );
  }
}
