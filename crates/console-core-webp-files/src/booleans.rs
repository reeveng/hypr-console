//! What VP8 spells with a partition's bits beyond the bits themselves.
//!
//! Everything in a lossy bitstream after its first ten bytes is written with
//! a boolean entropy coder, and reading one is `console-core-boolean-decoding`,
//! which the VP9 decoder asks as well. What is left here is VP8's own: a
//! magnitude followed by its sign, and a flag that says whether either was
//! sent at all.

use console_core_boolean_decoding::Booleans;
use console_core_never::Never;

pub(crate) fn signed(booleans: &mut Booleans<'_>, bits: u32) -> Result<i32, Never> {
  let Ok(magnitude) = booleans.number(bits);
  let Ok(negative) = booleans.even();
  let positive = i32::from_le_bytes(magnitude.to_le_bytes());

  Ok(match negative {
    0 => positive,
    _ => positive.wrapping_neg(),
  })
}

pub(crate) fn flagged(booleans: &mut Booleans<'_>, bits: u32) -> Result<i32, Never> {
  let Ok(flag) = booleans.even();

  match flag {
    0 => Ok(0),
    _ => signed(booleans, bits),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_flag_that_is_a_nought_sends_nothing() {
    let Ok(mut booleans) = Booleans::new(&[0x00, 0x00, 0x00]);

    assert_eq!(flagged(&mut booleans, 7), Ok(0));
  }

  #[test]
  fn a_magnitude_with_its_sign_bit_set_is_negative() {
    let Ok(mut booleans) = Booleans::new(&[0b0000_1110, 0x00, 0x00]);
    let Ok(mut again) = Booleans::new(&[0b0000_1110, 0x00, 0x00]);
    let Ok(magnitude) = again.number(7);
    let Ok(sign) = again.even();
    let Ok(value) = signed(&mut booleans, 7);

    assert_eq!(
      (value.unsigned_abs(), u32::from(value < 0)),
      (magnitude, sign)
    );
  }
}
