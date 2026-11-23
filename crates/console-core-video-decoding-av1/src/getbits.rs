//! Bit fields in AV1 OBU headers. An overrun is latched so parsers can read
//! consecutive fields and check the reader before accepting the header.

#![forbid(unsafe_code)]

use core::cmp::Ordering;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index, wrapped};

use crate::include::common::intops::inv_recenter;
use crate::src::error::{Rav1dError, Rav1dResult};

const BYTE_BITS: u32 = u8::BITS;
const BUFFER_BITS: u32 = u64::BITS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderBit {
  Zero,
  One,
}

impl From<HeaderBit> for u8 {
  fn from(bit: HeaderBit) -> Self {
    match bit {
      HeaderBit::Zero => 0,
      HeaderBit::One => 1,
    }
  }
}

impl From<HeaderBit> for u32 {
  fn from(bit: HeaderBit) -> Self {
    Self::from(u8::from(bit))
  }
}

impl From<HeaderBit> for i32 {
  fn from(bit: HeaderBit) -> Self {
    Self::from(u8::from(bit))
  }
}

pub struct GetBits<'a> {
  state: u64,
  buffered: u32,
  failure: Option<Rav1dError>,
  length: u64,
  unread: &'a [u8],
}

impl<'a> GetBits<'a> {
  pub fn new(data: &'a [u8]) -> Result<Self, Never> {
    let Ok(length) = fitted::<_, u64>(data.len());

    Ok(Self {
      state: 0,
      buffered: 0,
      failure: None,
      length,
      unread: data,
    })
  }

  pub fn check(&self) -> Rav1dResult {
    match self.failure {
      Some(failure) => Err(failure),
      None => Ok(()),
    }
  }

  #[inline]
  pub fn get_bit(&mut self) -> Result<HeaderBit, Never> {
    let Ok(bit) = self.read_bits(1);

    Ok(match bit {
      0 => HeaderBit::Zero,
      _ => HeaderBit::One,
    })
  }

  #[inline]
  fn read_bits(&mut self, width: u32) -> Result<u32, Never> {
    let missing = width.saturating_sub(self.buffered);

    for _ in 0..missing.div_ceil(BYTE_BITS) {
      match self.unread.split_first() {
        Some((&byte, rest)) => {
          self.buffered = self.buffered.saturating_add(BYTE_BITS);
          self.state |= u64::from(byte).wrapping_shl(BUFFER_BITS.saturating_sub(self.buffered));
          self.unread = rest;
        }
        None => {
          self.failure = Some(Rav1dError::EINVAL);
          break;
        }
      }
    }

    let field = self.state.wrapping_shr(BUFFER_BITS.saturating_sub(width));
    self.state = self.state.wrapping_shl(width);
    self.buffered = self.buffered.saturating_sub(width);
    fitted(field)
  }

  pub fn get_bits(&mut self, width: i32) -> Result<u32, Never> {
    match width {
      1..=32 => {
        let Ok(width_bits) = fitted::<_, u32>(width);
        self.read_bits(width_bits)
      }
      _ => {
        self.failure = Some(Rav1dError::EINVAL);
        Ok(0)
      }
    }
  }

  pub fn get_sbits(&mut self, width: i32) -> Result<i32, Never> {
    let Ok(field) = self.get_bits(width);
    let Ok(width_bits) = fitted::<_, u32>(width);
    let shift = i32::BITS.saturating_sub(width_bits);
    let Ok(signed) = wrapped::<_, i32>(field);

    Ok(signed.wrapping_shl(shift).wrapping_shr(shift))
  }

  pub fn get_uleb128(&mut self) -> Rav1dResult<u32> {
    let mut value = 0u64;

    for shift in (0..56).step_by(7) {
      let Ok(byte) = self.read_bits(BYTE_BITS);
      self.check()?;
      value |= u64::from(byte & 0x7f).wrapping_shl(shift);

      match byte & 0x80 {
        0 => return u32::try_from(value).map_err(|_too_wide| Rav1dError::EINVAL),
        _ => {}
      }
    }

    Err(Rav1dError::EINVAL)
  }

  pub fn get_uniform(&mut self, choices: u32) -> Result<u32, Never> {
    match choices {
      0 => {
        self.failure = Some(Rav1dError::EINVAL);
        return Ok(0);
      }
      1 => return Ok(0),
      _ => {}
    }

    let width = u32::BITS.saturating_sub(choices.leading_zeros());
    let wide_cutoff = 1u64.wrapping_shl(width).saturating_sub(u64::from(choices));
    let Ok(cutoff) = fitted::<_, u32>(wide_cutoff);
    let Ok(value) = self.read_bits(width.saturating_sub(1));

    Ok(match value.cmp(&cutoff) {
      Ordering::Less => value,
      Ordering::Equal | Ordering::Greater => {
        let Ok(low_bit) = self.get_bit();
        value
          .wrapping_shl(1)
          .wrapping_sub(cutoff)
          .wrapping_add(u32::from(low_bit))
      }
    })
  }

  pub fn get_vlc(&mut self) -> Rav1dResult<u32> {
    for leading_zeros in 0..32 {
      let Ok(stop) = self.get_bit();
      self.check()?;

      match stop {
        HeaderBit::One => {
          match leading_zeros {
            0 => return Ok(0),
            _ => {}
          }

          let Ok(suffix) = self.read_bits(leading_zeros);
          self.check()?;

          return Ok(
            1u32
              .wrapping_shl(leading_zeros)
              .wrapping_sub(1)
              .wrapping_add(suffix),
          );
        }
        HeaderBit::Zero => {}
      }
    }

    Err(Rav1dError::EINVAL)
  }

  pub fn get_bits_subexp(&mut self, reference: i32, width: u32) -> Result<i32, Never> {
    let bias = 1i32.wrapping_shl(width);
    let Ok(unsigned_reference) = wrapped::<_, u32>(reference.wrapping_add(bias));
    let maximum = 2u32.wrapping_shl(width);
    let mut offset = 0u32;
    let mut value = 0;

    for group_width in core::iter::once(3).chain(3..32) {
      let group = 1u32.wrapping_shl(group_width);
      let wide_end = u64::from(offset).saturating_add(u64::from(group).saturating_mul(3));

      match u64::from(maximum).cmp(&wide_end) {
        Ordering::Less => {
          let choices = maximum.saturating_sub(offset).saturating_add(1);
          let Ok(tail) = self.get_uniform(choices);
          value = offset.wrapping_add(tail);
          break;
        }
        Ordering::Equal | Ordering::Greater => match self.get_bit() {
          Ok(HeaderBit::One) => offset = offset.wrapping_add(group),
          Ok(HeaderBit::Zero) => {
            let Ok(tail) = self.read_bits(group_width);
            value = offset.wrapping_add(tail);
            break;
          }
        },
      }
    }

    let decoded = match unsigned_reference.cmp(&maximum.wrapping_shr(1)) {
      Ordering::Less | Ordering::Equal => inv_recenter(unsigned_reference, value),
      Ordering::Greater => maximum.wrapping_sub(inv_recenter(
        maximum.wrapping_sub(unsigned_reference),
        value,
      )),
    };
    let Ok(signed) = wrapped::<_, i32>(decoded);

    Ok(signed.wrapping_sub(bias))
  }

  #[inline]
  pub fn bytealign(&mut self) -> Result<(), Never> {
    self.buffered = 0;
    self.state = 0;

    Ok(())
  }

  pub fn check_alignment(&self) -> Rav1dResult {
    match self.buffered {
      0 => Ok(()),
      _ => Err(Rav1dError::EINVAL),
    }
  }

  pub fn pos(&self) -> Result<u64, Never> {
    let Ok(bytes) = self.byte_pos();

    Ok(
      bytes
        .saturating_mul(u64::from(BYTE_BITS))
        .saturating_sub(u64::from(self.buffered)),
    )
  }

  pub fn byte_pos(&self) -> Result<u64, Never> {
    let Ok(remaining) = self.remaining_len();

    Ok(self.length.saturating_sub(remaining))
  }

  pub const fn pending_bits(&self) -> Result<u64, Never> {
    Ok(self.state)
  }

  pub const fn remaining_bytes(&self) -> Result<&'a [u8], Never> {
    Ok(self.unread)
  }

  pub fn set_remaining_len(&mut self, length: u32) -> Rav1dResult {
    let Ok(bytes) = index(length);
    let Ok(position) = self.byte_pos();

    match self.unread.get(..bytes) {
      Some(unread) => {
        self.unread = unread;
        self.length = position.saturating_add(u64::from(length));
        Ok(())
      }
      None => Err(Rav1dError::EINVAL),
    }
  }

  pub const fn len(&self) -> Result<u64, Never> {
    Ok(self.length)
  }

  pub fn remaining_len(&self) -> Result<u64, Never> {
    fitted(self.unread.len())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reads_signed_and_unsigned_fields_across_a_byte_boundary() {
    let Ok(mut flags) = GetBits::new(&[0b0100_0000]);
    assert_eq!(flags.get_bit(), Ok(HeaderBit::Zero));
    assert_eq!(flags.get_bit(), Ok(HeaderBit::One));

    let Ok(mut bits) = GetBits::new(&[0b1011_1101, 0b1010_0101, 0xff]);

    assert_eq!(bits.get_bits(3), Ok(0b101));
    assert_eq!(bits.get_sbits(6), Ok(-5));
    assert_eq!(bits.pos(), Ok(9));
    assert_eq!(bits.get_bits(7), Ok(0b010_0101));
    assert_eq!(bits.check_alignment(), Ok(()));

    assert_eq!(bits.set_remaining_len(1), Ok(()));
    assert_eq!(bits.get_sbits(8), Ok(-1));
    assert_eq!(bits.check(), Ok(()));
  }

  #[test]
  fn length_delimits_reads_and_the_first_overrun_stays_latched() {
    let Ok(mut bits) = GetBits::new(&[0x83, 0x01, 0xb4, 0xc8]);

    assert_eq!(bits.get_uleb128(), Ok(131));
    assert_eq!(bits.set_remaining_len(1), Ok(()));
    assert_eq!(bits.get_bits(12), Ok(0xb40));
    assert_eq!(bits.check(), Err(Rav1dError::EINVAL));
    assert_eq!(bits.bytealign(), Ok(()));
    assert_eq!(bits.get_bits(3), Ok(0));
    assert_eq!(bits.check(), Err(Rav1dError::EINVAL));
  }
}
