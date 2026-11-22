//! Bits read highest first, and the Exp-Golomb codes written in them.
//!
//! A position is a count of bits from the front. [`BitReader::peek`] reads
//! the eight bytes from the one the position is in, which always hold the
//! thirty-two bits a field can be after the seven a position can be into its
//! byte, so a field is one load and two shifts rather than a walk a bit at a
//! time.

use core::cmp::Ordering;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::BitError;

const BYTE: u64 = 8;

const WORD: u32 = 64;

const WIDEST: u32 = 32;

const LONGEST_CODE: u32 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagBit {
  Zero,
  One,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remaining {
  Payload,
  TrailingBits,
}

#[derive(Debug, Clone)]
pub struct BitReader<'a> {
  data: &'a [u8],
  at: u64,
  end: u64,
}

impl<'a> BitReader<'a> {
  pub fn new(data: &'a [u8]) -> Result<BitReader<'a>, Never> {
    let Ok(bytes) = fitted::<_, u64>(data.len());

    Ok(BitReader {
      data,
      at: 0,
      end: bytes.saturating_mul(BYTE),
    })
  }

  pub fn data(&self) -> Result<&'a [u8], Never> {
    Ok(self.data)
  }

  pub fn position(&self) -> Result<u64, Never> {
    Ok(self.at)
  }

  pub fn bits_left(&self) -> Result<u64, Never> {
    Ok(self.end.saturating_sub(self.at))
  }

  pub fn byte_align(&mut self) -> Result<(), Never> {
    self.at = self.at.div_ceil(BYTE).saturating_mul(BYTE);

    Ok(())
  }

  pub fn byte_pos(&self) -> Result<u64, Never> {
    Ok(self.at.div_euclid(BYTE))
  }

  pub fn seek_bits(&mut self, at: u64) -> Result<(), Never> {
    self.at = at;

    Ok(())
  }

  #[inline]
  pub fn peek(&self, width: u32) -> Result<u32, Never> {
    let wanted = width.min(WIDEST);
    let Ok(first) = index(self.at.div_euclid(BYTE));
    let mut window = [0u8; 8];

    let ahead = match self.data.get(first..) {
      Some(ahead) => ahead,
      None => &[],
    };

    for (slot, byte) in window.iter_mut().zip(ahead) {
      *slot = *byte;
    }

    let Ok(into) = fitted::<u64, u32>(self.at.rem_euclid(BYTE));
    let lined_up = u64::from_be_bytes(window).wrapping_shl(into);

    let field = match lined_up.checked_shr(WORD.saturating_sub(wanted)) {
      Some(field) => field,
      None => 0,
    };

    let Ok(narrowed) = fitted::<u64, u32>(field);

    Ok(narrowed)
  }

  #[inline]
  pub fn read_bits(&mut self, width: u32) -> Result<u32, BitError> {
    match width.cmp(&WIDEST) {
      Ordering::Greater => Err(BitError::TooWide),
      Ordering::Less | Ordering::Equal => {
        self.room_for(u64::from(width))?;

        let Ok(field) = self.peek(width);

        self.at = self.at.saturating_add(u64::from(width));

        Ok(field)
      }
    }
  }

  pub fn read_bits_u64(&mut self, width: u32) -> Result<u64, BitError> {
    self.room_for(u64::from(width))?;

    let high = self.read_bits(width.saturating_sub(WIDEST))?;
    let low = self.read_bits(width.min(WIDEST))?;

    Ok(u64::from(high).wrapping_shl(width.min(WIDEST)) | u64::from(low))
  }

  #[inline]
  pub fn read_flag(&mut self) -> Result<FlagBit, BitError> {
    let bit = self.read_bits(1)?;

    Ok(match bit {
      0 => FlagBit::Zero,
      _ => FlagBit::One,
    })
  }

  pub fn skip(&mut self, count: u64) -> Result<(), BitError> {
    self.room_for(count)?;

    self.at = self.at.saturating_add(count);

    Ok(())
  }

  pub fn read_ue(&mut self) -> Result<u32, BitError> {
    let zeros = self.leading_zeros()?;
    let after = self.read_bits_u64(zeros)?;
    let coded = 1u64.wrapping_shl(zeros).wrapping_sub(1).wrapping_add(after);

    match u32::try_from(coded) {
      Ok(number) => Ok(number),
      Err(_wider) => Err(BitError::TooLong),
    }
  }

  pub fn read_se(&mut self) -> Result<i32, BitError> {
    let code = self.read_ue()?;
    let half = code.wrapping_shr(1);

    let signed = match code & 1 {
      0 => i32::try_from(half).map(i32::wrapping_neg),
      _ => i32::try_from(half.saturating_add(1)),
    };

    match signed {
      Ok(number) => Ok(number),
      Err(_wider) => Err(BitError::TooLong),
    }
  }

  pub fn more_rbsp_data(&self) -> Result<Remaining, Never> {
    Ok(
      match self
        .data
        .iter()
        .enumerate()
        .rev()
        .find(|(_at, byte)| **byte != 0)
      {
        Some((at, byte)) => {
          let Ok(before) = fitted::<_, u64>(at);
          let stop = before
            .saturating_mul(BYTE)
            .saturating_add(7)
            .saturating_sub(u64::from(byte.trailing_zeros()));

          match self.at.cmp(&stop) {
            Ordering::Less => Remaining::Payload,
            Ordering::Equal | Ordering::Greater => Remaining::TrailingBits,
          }
        }
        None => Remaining::TrailingBits,
      },
    )
  }

  fn room_for(&self, width: u64) -> Result<(), BitError> {
    let Ok(left) = self.bits_left();

    match width.cmp(&left) {
      Ordering::Greater => Err(BitError::PastTheEnd),
      Ordering::Less | Ordering::Equal => Ok(()),
    }
  }

  fn leading_zeros(&mut self) -> Result<u32, BitError> {
    for zeros in 0..=LONGEST_CODE {
      let bit = self.read_flag()?;

      match bit {
        FlagBit::One => return Ok(zeros),
        FlagBit::Zero => {}
      }
    }

    Err(BitError::TooLong)
  }
}
