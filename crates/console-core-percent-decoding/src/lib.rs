//! The bytes an address spells with `%XX`, read back.
//!
//! Three places wrote this. A `file://` address read its path through one, a
//! book's links were read through a copy of it a crate away, and a `data:`
//! address in a fork walked its own. The two in this tree asked
//! `u8::from_str_radix` what two characters spell, and that function reads a
//! sign: `%+5` came back as the byte five, which no address has ever meant.
//! The fork asked each character whether it was a digit, which is the question.
//!
//! So an escape is a percent sign and two hexadecimal digits and nothing else
//! is one. A percent sign that escapes nothing is a percent sign, because an
//! address somebody typed is not rejected for a name with `100%` in it. What
//! the bytes are afterwards, a path, text or a picture, is the caller's: this
//! hands back bytes because a name that is not UTF-8 has to survive the trip.
//!
//! `escaped_byte` is the half a caller with a walk of its own asks. A `data:`
//! address drops tabs and line ends and stops at a `#`, which is that format's
//! rule and stays with it; what two digits spell is the same everywhere.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;

use console_core_never::Never;

pub const ESCAPE: u8 = b'%';

pub fn decoded(spelled: &[u8]) -> Result<Vec<u8>, Never> {
  let mut written = Vec::with_capacity(spelled.len());
  let mut rest = spelled;

  for _at_most in spelled {
    let Ok(found) = next(rest);

    match found {
      Some((byte, beyond)) => {
        written.push(byte);
        rest = beyond;
      }
      None => {}
    }
  }

  Ok(written)
}

pub fn escaped_byte(after: &[u8]) -> Result<Option<u8>, Never> {
  Ok(match after.first_chunk::<2>() {
    Some(digits) => match console_core_bytes::hex::decode_pair(*digits) {
      Ok(byte) => Some(byte),
      Err(console_core_bytes::hex::InvalidDigit(_)) => None,
    },
    None => None,
  })
}

fn next(rest: &[u8]) -> Result<Option<(u8, &[u8])>, Never> {
  let (first, after) = match rest.split_first() {
    Some(split) => split,
    None => return Ok(None),
  };

  let Ok(escape) = escaped_byte(after);

  Ok(Some(
    match (*first, escape, after.split_first_chunk::<2>()) {
      (ESCAPE, Some(byte), Some((_digits, beyond))) => (byte, beyond),
      (plain, _, _) => (plain, after),
    },
  ))
}
