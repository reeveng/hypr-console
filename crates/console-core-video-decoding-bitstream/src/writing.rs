//! The reader the other way round: fields and Exp-Golomb codes written
//! highest bit first.
//!
//! Nothing in this desktop encodes a film. The writer is here because the
//! reader's tests are written with it and so are the decoders', and a stream
//! a test spells out a bit at a time in hex is a stream nobody can check.
//! Whole bytes leave the word as soon as they are made, so what is held is
//! never more than the seven bits of a byte not yet finished and one field.

use core::cmp::Ordering;

use console_core_never::Never;
use console_core_number_conversion::fitted;

const WIDEST: u32 = 32;

const BYTE: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
  pub value: u32,
  pub width: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitWriter {
  bytes: Vec<u8>,
  word: u64,
  count: u32,
}

impl BitWriter {
  pub fn new() -> Result<BitWriter, Never> {
    Ok(BitWriter {
      bytes: Vec::new(),
      word: 0,
      count: 0,
    })
  }

  pub fn write_bits(&mut self, field: Field) -> Result<(), Never> {
    let width = field.width.min(WIDEST);
    let Ok(kept) = mask(u64::from(field.value), width);

    self.word = self.word.wrapping_shl(width) | kept;
    self.count = self.count.saturating_add(width);

    for _byte in 0..self.count.div_euclid(BYTE) {
      self.count = self.count.saturating_sub(BYTE);

      let Ok(byte) = fitted::<u64, u8>(self.word.wrapping_shr(self.count) & 0xFF);

      self.bytes.push(byte);
    }

    let Ok(unfinished) = mask(self.word, self.count);

    self.word = unfinished;

    Ok(())
  }

  pub fn write_ue(&mut self, number: u32) -> Result<(), Never> {
    self.write_code(u64::from(number))
  }

  pub fn write_se(&mut self, number: i32) -> Result<(), Never> {
    let doubled = u64::from(number.unsigned_abs()).saturating_mul(2);

    self.write_code(match number.cmp(&0) {
      Ordering::Greater => doubled.saturating_sub(1),
      Ordering::Less | Ordering::Equal => doubled,
    })
  }

  pub fn rbsp_trailing(&mut self) -> Result<(), Never> {
    let Ok(()) = self.write_bits(Field { value: 1, width: 1 });

    self.align()
  }

  pub fn finish(mut self) -> Result<Vec<u8>, Never> {
    let Ok(()) = self.align();

    Ok(self.bytes)
  }

  fn align(&mut self) -> Result<(), Never> {
    self.write_bits(Field {
      value: 0,
      width: self.count.wrapping_neg() & 7,
    })
  }

  fn write_code(&mut self, code: u64) -> Result<(), Never> {
    let coded = code.saturating_add(1);
    let length = 64u32.saturating_sub(coded.leading_zeros());
    let Ok(()) = self.write_bits(Field {
      value: 0,
      width: length.saturating_sub(1),
    });
    let low = length.min(WIDEST);
    let Ok(high) = fitted::<u64, u32>(coded.wrapping_shr(low));
    let Ok(()) = self.write_bits(Field {
      value: high,
      width: length.saturating_sub(low),
    });
    let Ok(rest) = mask(coded, low);
    let Ok(narrowed) = fitted::<u64, u32>(rest);

    self.write_bits(Field {
      value: narrowed,
      width: low,
    })
  }
}

fn mask(value: u64, width: u32) -> Result<u64, Never> {
  Ok(match 1u64.checked_shl(width) {
    Some(past) => value & past.wrapping_sub(1),
    None => value,
  })
}
