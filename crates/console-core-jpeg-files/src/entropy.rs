//! Quantized blocks written as the bits of a baseline scan.
//!
//! The codes are the four tables printed in Annex K of the standard, which
//! libjpeg writes when it is not asked to optimize: one for the first
//! frequency of a block and one for the rest, for brightness and for color.
//! Counting each picture's symbols and building its own codes is what
//! libjpeg's `-optimize` does, and it makes a file a few percent smaller for a
//! second pass over every block; a thumbnail is not worth it.
//!
//! The first frequency is written as how far it is from the one before it in
//! the same component, the rest as runs of zeros and the value that ends each
//! run, in the zigzag order that puts the frequencies most likely to be zero
//! last, so a block ends early with one symbol saying the rest are zero.
//!
//! Bits go in from the high end of each byte, and a byte that comes out as
//! 0xFF is followed by a zero, so that nothing in a scan reads as a marker.
//! The last byte is filled with ones.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::kept::NATURAL;

const END_OF_BLOCK: u8 = 0x00;

const SIXTEEN_ZEROS: u8 = 0xF0;

const LONGEST_RUN: u32 = 16;

const MARKER: u8 = 0xFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Code {
  bits: u16,
  length: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Table {
  codes: [Code; 256],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Tables<'a> {
  pub(crate) dc: &'a Table,
  pub(crate) ac: &'a Table,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Writer {
  bytes: Vec<u8>,
  word: u64,
  count: u32,
}

pub(crate) fn table(counts: &[u8; 16], symbols: &[u8]) -> Result<Table, Never> {
  let mut codes = [Code { bits: 0, length: 0 }; 256];
  let mut symbols_2 = symbols.iter();
  let mut bits = 0u16;

  for (length, count) in (1u8..).zip(counts) {
    for (_, symbol) in (0..*count).zip(symbols_2.by_ref()) {
      let Ok(at) = index(*symbol);

      match codes.get_mut(at) {
        Some(code) => *code = Code { bits, length },
        None => {}
      }

      bits = bits.wrapping_add(1);
    }

    bits = bits.wrapping_shl(1);
  }

  Ok(Table { codes })
}

impl Writer {
  pub(crate) fn new() -> Result<Writer, Never> {
    Ok(Writer {
      bytes: Vec::new(),
      word: 0,
      count: 0,
    })
  }

  #[inline(always)]
  fn put(&mut self, bits: u32, length: u8) -> Result<(), Never> {
    let length_2 = u32::from(length);
    let kept = u64::from(bits) & 1u64.wrapping_shl(length_2).wrapping_sub(1);

    self.word = self.word.wrapping_shl(length_2) | kept;
    self.count = self.count.saturating_add(length_2);

    for _byte in 0..self.count.wrapping_shr(3) {
      self.count = self.count.saturating_sub(8);

      let Ok(byte) = fitted::<u64, u8>(self.word.wrapping_shr(self.count) & 0xFF);

      self.bytes.push(byte);

      match byte {
        MARKER => self.bytes.push(0),
        _ => {}
      }
    }

    Ok(())
  }

  #[inline(always)]
  fn coded(&mut self, table: &Table, symbol: u8) -> Result<(), Never> {
    let Ok(at) = index(symbol);

    match table.codes.get(at) {
      Some(code) => self.put(u32::from(code.bits), code.length),
      None => Ok(()),
    }
  }

  pub(crate) fn finished(mut self) -> Result<Vec<u8>, Never> {
    let Ok(width) = fitted::<u32, u8>(self.count.wrapping_neg() & 7);
    let Ok(()) = self.put(u32::MAX, width);

    Ok(self.bytes)
  }
}

#[inline(always)]
fn magnitude(value: i32) -> Result<(u8, u32), Never> {
  let Ok(size) = fitted::<u32, u8>(32u32.saturating_sub(value.unsigned_abs().leading_zeros()));

  let written = match value {
    i32::MIN..=-1 => value.wrapping_sub(1),
    0.. => value,
  };

  Ok((size, u32::from_le_bytes(written.to_le_bytes())))
}

pub(crate) fn block(
  writer: &mut Writer,
  coefficients: &[i16; 64],
  before: i16,
  tables: Tables<'_>,
) -> Result<i16, Never> {
  let dc = match coefficients.first() {
    Some(dc) => *dc,
    None => before,
  };

  let Ok((size, bits)) = magnitude(i32::from(dc).wrapping_sub(i32::from(before)));
  let Ok(()) = writer.coded(tables.dc, size);
  let Ok(()) = writer.put(bits, size);
  let mut zeros = 0u32;

  for natural in NATURAL.iter().skip(1) {
    let Ok(at) = index(*natural);

    let value = match coefficients.get(at) {
      Some(value) => i32::from(*value),
      None => 0,
    };

    match value {
      0 => zeros = zeros.saturating_add(1),
      _ => {
        let Ok(()) = ac(writer, tables.ac, (zeros, value));

        zeros = 0;
      }
    }
  }

  match zeros {
    0 => {}
    1.. => {
      let Ok(()) = writer.coded(tables.ac, END_OF_BLOCK);
    }
  }

  Ok(dc)
}

#[inline(always)]
fn ac(writer: &mut Writer, table: &Table, run: (u32, i32)) -> Result<(), Never> {
  let (zeros, value) = run;

  for _sixteen in 0..zeros.div_euclid(LONGEST_RUN) {
    let Ok(()) = writer.coded(table, SIXTEEN_ZEROS);
  }

  let Ok((size, bits)) = magnitude(value);
  let Ok(left) = fitted::<u32, u8>(zeros.rem_euclid(LONGEST_RUN));
  let Ok(()) = writer.coded(table, left.wrapping_shl(4) | size);

  writer.put(bits, size)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::huffman::{
    AC_CHROMINANCE_COUNTS, AC_CHROMINANCE_SYMBOLS, AC_LUMINANCE_COUNTS, AC_LUMINANCE_SYMBOLS,
  };
  use std::collections::BTreeSet;

  #[test]
  fn every_run_and_size_a_baseline_scan_can_say_has_one_code_in_each_ac_table() {
    let every: BTreeSet<u8> = [END_OF_BLOCK, SIXTEEN_ZEROS]
      .into_iter()
      .chain((0u8..16).flat_map(|run| (1u8..=10).map(move |size| run.wrapping_shl(4) | size)))
      .collect();

    for (counts, symbols) in [
      (AC_LUMINANCE_COUNTS, AC_LUMINANCE_SYMBOLS),
      (AC_CHROMINANCE_COUNTS, AC_CHROMINANCE_SYMBOLS),
    ] {
      let listed: BTreeSet<u8> = symbols.iter().copied().collect();
      let counted = counts.iter().map(|count| u32::from(*count)).sum::<u32>();

      assert_eq!(listed, every);
      assert_eq!(fitted::<_, u32>(symbols.len()), Ok(counted));
    }
  }

  #[test]
  fn a_negative_value_is_written_as_its_ones_complement_in_as_many_bits_as_its_size() {
    assert_eq!(magnitude(0), Ok((0, 0)));
    assert_eq!(magnitude(5), Ok((3, 0b101)));
    assert_eq!(
      magnitude(-5).map(|(size, bits)| (size, bits & 0b111)),
      Ok((3, 0b010))
    );
    assert_eq!(
      magnitude(-1).map(|(size, bits)| (size, bits & 0b1)),
      Ok((1, 0))
    );
  }

  #[test]
  fn a_byte_that_reads_as_a_marker_is_followed_by_a_zero_and_the_last_is_filled_with_ones() {
    let Ok(mut writer) = Writer::new();
    let Ok(()) = writer.put(0xFF, 8);
    let Ok(()) = writer.put(0b01, 2);

    assert_eq!(writer.finished(), Ok(vec![0xFF, 0x00, 0b0111_1111]));
  }
}
