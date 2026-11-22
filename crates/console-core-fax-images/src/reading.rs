//! The bits of a fax picture, read highest first, and the end-of-line codes
//! between its lines.
//!
//! An end of line is eleven zeros and a one, with as many zeros in front of
//! it as a sender liked to fill a line out to a byte. The zeros counted in
//! front are held to twenty-four, which is what the fork held them to: past
//! that a picture is a run of zeros, not a line ending.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::DecodeError;

const BYTE: u64 = 8;

const FILL_AT_MOST: u32 = 24;

const END_OF_LINE_ZEROS: u32 = 11;

const END_OF_LINE_LENGTH: u64 = 12;

#[derive(Debug, Clone)]
pub(crate) struct BitReader<'a> {
  data: &'a [u8],
  at: u64,
}

impl<'a> BitReader<'a> {
  pub(crate) fn new(data: &'a [u8]) -> Result<BitReader<'a>, Never> {
    Ok(BitReader { data, at: 0 })
  }

  #[inline(always)]
  pub(crate) fn read_bit(&mut self) -> Result<u32, DecodeError> {
    let Ok(at) = index(self.at.div_euclid(BYTE));

    let byte = match self.data.get(at) {
      Some(byte) => *byte,
      None => return Err(DecodeError::UnexpectedEof),
    };

    let Ok(within) = fitted::<u64, u32>(self.at.rem_euclid(BYTE));

    self.at = self.at.saturating_add(1);

    Ok(u32::from(byte).wrapping_shr(7u32.saturating_sub(within)) & 1)
  }

  pub(crate) fn read_bits(&mut self, count: u32) -> Result<u32, DecodeError> {
    let mut bits = 0u32;

    for place in (0..count).rev() {
      let bit = self.read_bit()?;

      bits |= bit.wrapping_shl(place);
    }

    Ok(bits)
  }

  pub(crate) fn peek_bits(&self, count: u32) -> Result<u32, DecodeError> {
    self.clone().read_bits(count)
  }

  pub(crate) fn align(&mut self) -> Result<(), Never> {
    self.at = self.at.div_ceil(BYTE).saturating_mul(BYTE);

    Ok(())
  }

  pub(crate) fn bits_left(&self) -> Result<u64, Never> {
    let Ok(bytes) = fitted::<_, u64>(self.data.len());

    Ok(bytes.saturating_mul(BYTE).saturating_sub(self.at))
  }

  pub(crate) fn byte_pos(&self) -> Result<u64, Never> {
    Ok(self.at.div_euclid(BYTE))
  }

  pub(crate) fn read_eols(&mut self) -> Result<u32, Never> {
    let Ok(left) = self.bits_left();
    let mut count = 0u32;

    for _end_of_line in 0..=left.div_euclid(END_OF_LINE_LENGTH) {
      let Ok(fill) = self.zeros_ahead();
      let ending = self.peek_bits(fill.saturating_add(1));

      match ::core::primitive::bool::then_some(fill >= END_OF_LINE_ZEROS && ending == Ok(1), ()) {
        ::core::option::Option::Some(()) => match self.read_bits(fill.saturating_add(1)) {
          Ok(_end_of_line) => count = count.saturating_add(1),
          Err(_unreachable_after_a_peek) => return Ok(count),
        },
        ::core::option::Option::None => return Ok(count),
      }
    }

    Ok(count)
  }

  fn zeros_ahead(&self) -> Result<u32, Never> {
    for fill in 0..FILL_AT_MOST {
      match self.peek_bits(fill.saturating_add(1)) {
        Ok(0) => {}
        Ok(_one) => return Ok(fill),
        Err(_ended) => return Ok(fill),
      }
    }

    Ok(FILL_AT_MOST)
  }
}
