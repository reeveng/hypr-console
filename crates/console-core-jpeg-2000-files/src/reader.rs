//! Combined byte and bit reader utilities.

use crate::error::{MarkerError, Result};
use crate::states::{ReaderEnd, StuffBit};
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};

#[derive(Debug, Clone)]
pub(crate) struct BitReader<'a> {
  remaining: &'a [u8],
  byte_offset: u64,
  bit_offset: u32,
}

impl<'a> BitReader<'a> {
  #[inline]
  pub(crate) fn new(data: &'a [u8]) -> core::result::Result<Self, Never> {
    Ok(Self {
      remaining: data,
      byte_offset: 0,
      bit_offset: 0,
    })
  }

  #[inline]
  pub(crate) fn align(&mut self) -> core::result::Result<(), Never> {
    match (self.bit_offset, self.remaining) {
      (1..=7, [_, rest @ ..]) => {
        self.remaining = rest;
        self.byte_offset = self.byte_offset.wrapping_add(1);
        self.bit_offset = 0;
      }
      _ => {}
    }

    Ok(())
  }

  #[inline]
  pub(crate) fn at_end(&self) -> core::result::Result<ReaderEnd, Never> {
    Ok(match self.remaining {
      [] => ReaderEnd::Reached,
      _ => ReaderEnd::Pending,
    })
  }

  #[inline]
  pub(crate) fn jump_to_end(&mut self) -> core::result::Result<(), Never> {
    let Ok(length) = fixed_index(self.remaining.len());
    self.byte_offset = self.byte_offset.wrapping_add(length);
    self.bit_offset = 0;
    self.remaining = &[];
    Ok(())
  }

  #[inline]
  pub(crate) fn tail(&self) -> core::result::Result<Option<&'a [u8]>, Never> {
    Ok(Some(self.remaining))
  }

  #[inline]
  pub(crate) fn remaining_bytes(&self) -> core::result::Result<u64, Never> {
    fixed_index(self.remaining.len())
  }

  #[inline]
  pub(crate) fn offset(&self) -> core::result::Result<u64, Never> {
    Ok(self.byte_offset)
  }

  #[inline]
  pub(crate) fn read_bytes(&mut self, len: u64) -> core::result::Result<Option<&'a [u8]>, Never> {
    let Ok(available) = fixed_index(self.remaining.len());

    match (self.bit_offset, len.cmp(&available)) {
      (0, core::cmp::Ordering::Less | core::cmp::Ordering::Equal) => {}
      _ => return Ok(None),
    }

    let Ok(offset) = index(len);
    let (bytes, remaining) = {
      let optional_bytes = self.remaining.split_at_checked(offset);

      match optional_bytes {
        Some((matched_bytes, matched_remaining)) => (matched_bytes, matched_remaining),
        ::core::option::Option::None => return Ok(None),
      }
    };

    self.remaining = remaining;
    self.byte_offset = self.byte_offset.wrapping_add(len);
    Ok(Some(bytes))
  }

  #[inline]
  pub(crate) fn read_byte(&mut self) -> core::result::Result<Option<u8>, Never> {
    let Ok(bytes) = self.read_bytes(1);
    Ok(bytes.and_then(|value| value.first().copied()))
  }

  #[inline]
  pub(crate) fn skip_bytes(&mut self, len: u64) -> core::result::Result<Option<()>, Never> {
    let Ok(bytes) = self.read_bytes(len);
    Ok(bytes.map(|_| ()))
  }

  #[inline]
  pub(crate) fn read_u16(&mut self) -> core::result::Result<Option<u16>, Never> {
    let Ok(bytes) = self.read_bytes(2);
    Ok(bytes.and_then(|value| value.first_chunk().copied().map(u16::from_be_bytes)))
  }

  #[inline]
  pub(crate) fn read_u32(&mut self) -> core::result::Result<Option<u32>, Never> {
    let Ok(bytes) = self.read_bytes(4);
    Ok(bytes.and_then(|value| value.first_chunk().copied().map(u32::from_be_bytes)))
  }

  #[inline]
  pub(crate) fn read_u64(&mut self) -> core::result::Result<Option<u64>, Never> {
    let Ok(bytes) = self.read_bytes(8);
    Ok(bytes.and_then(|value| value.first_chunk().copied().map(u64::from_be_bytes)))
  }

  #[inline(always)]
  pub(crate) fn read_bit(&mut self) -> core::result::Result<Option<u32>, Never> {
    let (byte, rest) = match self.remaining {
      [byte, rest @ ..] => (byte, rest),
      [] => return Ok(None),
    };

    let bit = u32::from(*byte).wrapping_shr(7u32.wrapping_sub(self.bit_offset)) & 1;

    match self.bit_offset {
      7 => {
        self.remaining = rest;
        self.byte_offset = self.byte_offset.wrapping_add(1);
        self.bit_offset = 0;
      }
      _ => self.bit_offset = self.bit_offset.wrapping_add(1),
    }

    Ok(Some(bit))
  }

  #[inline]
  pub(crate) fn read_bits_with_stuffing(
    &mut self,
    bit_size: u8,
  ) -> core::result::Result<Option<u32>, Never> {
    let mut bits = 0u32;

    for _ in 0..bit_size {
      let Ok(stuffing) = self.needs_to_read_stuff_bit();

      let bit = {
        let outcome_bit = self.read_bit();

        match outcome_bit {
          Ok(Some(matched_bit)) => matched_bit,
          ::core::result::Result::Ok(::core::option::Option::None)
          | ::core::result::Result::Err(_) => return Ok(None),
        }
      };

      bits = bits.wrapping_shl(1) | bit;

      match stuffing {
        StuffBit::Required => {
          {
            let outcome_value = self.read_stuff_bit();

            match outcome_value {
              Ok(Some(())) => (),
              ::core::result::Result::Ok(::core::option::Option::None)
              | ::core::result::Result::Err(_) => return Ok(None),
            }
          };
        }
        StuffBit::Absent => {}
      }
    }

    Ok(Some(bits))
  }

  pub(crate) fn needs_to_read_stuff_bit(&self) -> core::result::Result<StuffBit, Never> {
    Ok(match (self.bit_offset, self.remaining) {
      (7, [0xff, ..]) => StuffBit::Required,
      _ => StuffBit::Absent,
    })
  }

  #[inline]
  pub(crate) fn read_stuff_bit(&mut self) -> core::result::Result<Option<()>, Never> {
    let Ok(bit) = self.read_bit();
    Ok(match bit {
      Some(0) => Some(()),
      ::core::option::Option::None | ::core::option::Option::Some(_) => None,
    })
  }

  #[inline]
  pub(crate) fn read_marker(&mut self) -> Result<u8> {
    match (self.bit_offset, self.remaining) {
      (0, [0xff, ..]) => {
        let Ok(prefix) = self.read_byte();
        prefix.ok_or(MarkerError::Invalid)?;
        let Ok(marker) = self.read_byte();
        let checked_value = marker.ok_or(MarkerError::Invalid)?;
        Ok(checked_value)
      }
      _ => Err(<crate::error::DecodeError as ::core::convert::From<
        crate::error::MarkerError,
      >>::from(MarkerError::Invalid)),
    }
  }

  #[inline]
  pub(crate) fn peek_marker(&self) -> core::result::Result<Option<u8>, Never> {
    Ok(match (self.bit_offset, self.remaining) {
      (0, [0xff, marker, ..]) => Some(*marker),
      _ => None,
    })
  }
}
