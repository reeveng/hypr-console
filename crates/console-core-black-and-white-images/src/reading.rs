use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::error::DecodeError;

const BYTE: u64 = 8;

#[derive(Debug, Clone)]
pub(crate) struct Reader<'a> {
  data: &'a [u8],
  at: u64,
}

impl<'a> Reader<'a> {
  pub(crate) fn new(data: &'a [u8]) -> Result<Reader<'a>, Never> {
    Ok(Reader { data, at: 0 })
  }

  pub(crate) fn remaining(&self) -> Result<u64, Never> {
    let Ok(held) = fitted::<_, u64>(self.data.len());

    let Ok(at) = self.byte_position();

    Ok(held.saturating_sub(at))
  }

  pub(crate) fn byte_position(&self) -> Result<u64, Never> {
    Ok(self.at.div_euclid(BYTE))
  }

  pub(crate) fn align(&mut self) -> Result<(), Never> {
    self.at = self.at.div_ceil(BYTE).saturating_mul(BYTE);

    Ok(())
  }

  pub(crate) fn peek(&self, count: u32) -> Result<&'a [u8], DecodeError> {
    let Ok(from) = self.byte_position();
    let Ok(to) = fitted::<_, u64>(count);
    let Ok(start) = index(from);
    let Ok(end) = index(from.saturating_add(to));

    self.data.get(start..end).ok_or(DecodeError::EndOfData)
  }

  pub(crate) fn bytes(&mut self, count: u32) -> Result<&'a [u8], DecodeError> {
    let taken = self.peek(count)?;
    let Ok(width) = fitted::<_, u64>(count);

    self.at = self.at.saturating_add(width.saturating_mul(BYTE));

    Ok(taken)
  }

  pub(crate) fn skip(&mut self, count: u32) -> Result<(), DecodeError> {
    let _skipped = self.bytes(count)?;

    Ok(())
  }

  pub(crate) fn tail(&self) -> Result<&'a [u8], DecodeError> {
    let Ok(at) = self.byte_position();
    let Ok(start) = index(at);

    self.data.get(start..).ok_or(DecodeError::EndOfData)
  }

  pub(crate) fn byte(&mut self) -> Result<u8, DecodeError> {
    let taken = self.bytes(1)?;

    taken.first().copied().ok_or(DecodeError::EndOfData)
  }

  pub(crate) fn signed_byte(&mut self) -> Result<i8, DecodeError> {
    let raw = self.byte()?;

    Ok(i8::from_be_bytes([raw]))
  }

  pub(crate) fn u16(&mut self) -> Result<u16, DecodeError> {
    let high = self.byte()?;
    let low = self.byte()?;

    Ok(u16::from_be_bytes([high, low]))
  }

  pub(crate) fn u32(&mut self) -> Result<u32, DecodeError> {
    let taken = self.bytes(4)?;

    match <[u8; 4]>::try_from(taken) {
      Ok(array) => Ok(u32::from_be_bytes(array)),
      Err(_short) => Err(DecodeError::EndOfData),
    }
  }

  pub(crate) fn i32(&mut self) -> Result<i32, DecodeError> {
    let taken = self.u32()?;

    Ok(i32::from_be_bytes(taken.to_be_bytes()))
  }

  pub(crate) fn nonzero_byte(&mut self) -> Result<u8, DecodeError> {
    let taken = self.byte()?;

    match taken {
      0 => Err(DecodeError::EndOfData),
      kept => Ok(kept),
    }
  }

  pub(crate) fn bit(&mut self) -> Result<u32, DecodeError> {
    let Ok(at) = index(self.at.div_euclid(BYTE));
    let held = self.data.get(at).copied().ok_or(DecodeError::EndOfData)?;
    let Ok(within) = fitted::<_, u32>(self.at.rem_euclid(BYTE));

    self.at = self.at.saturating_add(1);

    Ok(u32::from(held).wrapping_shr(7u32.saturating_sub(within)) & 1)
  }

  pub(crate) fn bits(&mut self, count: u32) -> Result<u32, DecodeError> {
    let mut value = 0u32;

    for _place in 0..count {
      let bit = self.bit()?;

      value = value.wrapping_shl(1) | bit;
    }

    Ok(value)
  }
}
