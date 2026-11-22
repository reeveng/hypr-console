//! A Huffman table, and a symbol read out of the bits with it.
//!
//! A table arrives as how many codes there are of each length and the symbols
//! in order, and the codes themselves are implied: canonical, counting up
//! from zero and doubling at each length. Nearly every symbol in a photograph
//! is coded in ten bits or fewer, so those are found by looking the next ten
//! bits up in a table of a thousand and twenty-four, and only a longer code is
//! read the way the standard describes, a length at a time.
//!
//! An AC symbol is followed by the bits of its value, and most of the time the
//! code and those bits are ten together, so a second table says for each ten
//! bits the whole step they spell: how far along the block it moves, what the
//! coefficient it lands on is, and how many bits it took. That is one look
//! where there would be two reads and a sign to work out, for nearly every
//! coefficient there is. Sixteen zeros and the end of a block are steps like
//! any other, one sixteen along with nothing to put down and the other past the
//! end, so reading a block never asks which kind of symbol it was given.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Ordering;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::JpegError;
use crate::bits::{Bits, Extra, signed};

const QUICK: u32 = 10;

const LONGEST: u32 = 16;

const ZERO_RUN: u32 = 15;

const SIXTEEN: u8 = 16;

const PAST_THE_END: u8 = 64;

pub(crate) const DC_LUMINANCE_COUNTS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];

pub(crate) const DC_CHROMINANCE_COUNTS: [u8; 16] =
  [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];

pub(crate) const DC_SYMBOLS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

pub(crate) const AC_LUMINANCE_COUNTS: [u8; 16] =
  [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7D];

pub(crate) const AC_LUMINANCE_SYMBOLS: [u8; 162] = [
  0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
  0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xA1, 0x08, 0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0,
  0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0A, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28,
  0x29, 0x2A, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
  0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
  0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
  0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7,
  0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5,
  0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1, 0xE2,
  0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8,
  0xF9, 0xFA,
];

pub(crate) const AC_CHROMINANCE_COUNTS: [u8; 16] =
  [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];

pub(crate) const AC_CHROMINANCE_SYMBOLS: [u8; 162] = [
  0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
  0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xA1, 0xB1, 0xC1, 0x09, 0x23, 0x33, 0x52, 0xF0,
  0x15, 0x62, 0x72, 0xD1, 0x0A, 0x16, 0x24, 0x34, 0xE1, 0x25, 0xF1, 0x17, 0x18, 0x19, 0x1A, 0x26,
  0x27, 0x28, 0x29, 0x2A, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
  0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
  0x69, 0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
  0x88, 0x89, 0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5,
  0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3,
  0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA,
  0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8,
  0xF9, 0xFA,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Step {
  pub(crate) advance: u32,
  pub(crate) value: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fast {
  Slow,
  Known { length: u8, advance: u8, value: i16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quick {
  Longer,
  Code { length: u8, symbol: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Run {
  first: u32,
  last: u32,
  at: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Huffman {
  quick: Box<[Quick; 1024]>,
  fast: Box<[Fast; 1024]>,
  runs: [Option<Run>; 17],
  symbols: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Coded<'a> {
  length: u32,
  symbols: &'a [u8],
}

pub(crate) fn huffman(counts: &[u8; 16], symbols: &[u8]) -> Result<Huffman, JpegError> {
  let mut quick = Box::new([Quick::Longer; 1024]);
  let mut runs = [None; 17];
  let mut code = 0u32;
  let mut at = 0u32;

  for (length, counted) in (1u32..).zip(counts) {
    let counted_2 = u32::from(*counted);
    let last = code.saturating_add(counted_2).saturating_sub(1);
    let run = Run {
      first: code,
      last,
      at,
    };

    match (counted_2, last.cmp(&1u32.wrapping_shl(length))) {
      (0, _) => {}
      (_, Ordering::Equal | Ordering::Greater) => return Err(JpegError::Corrupt),
      (_, Ordering::Less) => {
        let Ok(slot) = index(length);

        match runs.get_mut(slot) {
          Some(slot_2) => *slot_2 = Some(run),
          None => return Err(JpegError::Corrupt),
        }

        match length {
          0..=QUICK => quickly(quick.as_mut_slice(), run, Coded { length, symbols })?,
          _ => {}
        }
      }
    }

    at = at.saturating_add(counted_2);
    code = code.saturating_add(counted_2).wrapping_shl(1);
  }

  let Ok(kept) = index(at);

  let Ok(fast) = fast(&quick);

  match symbols.get(..kept) {
    Some(kept_2) => Ok(Huffman {
      quick,
      fast,
      runs,
      symbols: kept_2.to_vec(),
    }),
    None => Err(JpegError::Truncated),
  }
}

fn fast(quick: &[Quick; 1024]) -> Result<Box<[Fast; 1024]>, Never> {
  let mut fast = Box::new([Fast::Slow; 1024]);

  for ((at, entry), step) in (0u32..).zip(quick).zip(fast.iter_mut()) {
    *step = match entry {
      Quick::Code { length, symbol } => {
        let Ok(step_2) = stepped(at, (u32::from(*length), *symbol));

        step_2
      }
      Quick::Longer => Fast::Slow,
    };
  }

  Ok(fast)
}

fn stepped(at: u32, code: (u32, u8)) -> Result<Fast, Never> {
  let (length, symbol) = code;
  let size = u32::from(symbol & 0x0F);
  let whole = length.saturating_add(size);
  let Ok(short) = fitted::<u32, u8>(length);

  Ok(match (symbol.wrapping_shr(4), size, whole) {
    (0x0F, 0, _) => Fast::Known {
      length: short,
      advance: SIXTEEN,
      value: 0,
    },
    (_, 0, _) => Fast::Known {
      length: short,
      advance: PAST_THE_END,
      value: 0,
    },
    (run, 1.., 0..=QUICK) => {
      let bits =
        at.wrapping_shr(QUICK.saturating_sub(whole)) & 1u32.wrapping_shl(size).wrapping_sub(1);
      let Ok(value) = signed(Extra { bits, count: size });
      let Ok(value_2) = fitted::<i32, i16>(value);
      let Ok(length_2) = fitted::<u32, u8>(whole);

      Fast::Known {
        length: length_2,
        advance: run.saturating_add(1),
        value: value_2,
      }
    }
    (_, _, _) => Fast::Slow,
  })
}

fn quickly(quick: &mut [Quick], run: Run, coded: Coded<'_>) -> Result<(), JpegError> {
  let spread = QUICK.saturating_sub(coded.length);
  let Ok(length) = fitted::<u32, u8>(coded.length);
  let Ok(from) = index(run.at);
  let Ok(past) = index(
    run
      .at
      .saturating_add(run.last.saturating_sub(run.first))
      .saturating_add(1),
  );

  let symbols = match coded.symbols.get(from..past) {
    Some(symbols) => symbols,
    None => return Err(JpegError::Truncated),
  };

  for (code, symbol) in (run.first..).zip(symbols) {
    let Ok(start) = index(code.wrapping_shl(spread));
    let Ok(end) = index(code.saturating_add(1).wrapping_shl(spread));

    match quick.get_mut(start..end) {
      Some(slots) => slots.fill(Quick::Code {
        length,
        symbol: *symbol,
      }),
      None => return Err(JpegError::Corrupt),
    }
  }

  Ok(())
}

impl Huffman {
  #[inline(always)]
  pub(crate) fn ac(&self, bits: &mut Bits<'_>) -> Result<Step, JpegError> {
    let Ok(peeked) = bits.peek();
    let Ok(at) = index(peeked.wrapping_shr(LONGEST.saturating_sub(QUICK)));

    match self.fast.get(at) {
      Some(Fast::Known {
        length,
        advance,
        value,
      }) => {
        let Ok(()) = bits.skip(u32::from(*length));

        Ok(Step {
          advance: u32::from(*advance),
          value: i32::from(*value),
        })
      }
      Some(Fast::Slow) | None => {
        let (after, step) = self.slowly(*bits)?;

        *bits = after;

        Ok(step)
      }
    }
  }

  #[inline(never)]
  fn slowly<'b>(&self, bits: Bits<'b>) -> Result<(Bits<'b>, Step), JpegError> {
    let mut bits_2 = bits;
    let symbol = self.decode(&mut bits_2)?;
    let run = u32::from(symbol.wrapping_shr(4));
    let size = u32::from(symbol & 0x0F);

    let step = match (run, size) {
      (ZERO_RUN, 0) => Step {
        advance: u32::from(SIXTEEN),
        value: 0,
      },
      (_, 0) => Step {
        advance: u32::from(PAST_THE_END),
        value: 0,
      },
      (_, _) => {
        let Ok(value) = bits_2.extended(size);

        Step {
          advance: run.saturating_add(1),
          value,
        }
      }
    };

    Ok((bits_2, step))
  }

  #[inline(always)]
  pub(crate) fn decode(&self, bits: &mut Bits<'_>) -> Result<u8, JpegError> {
    let Ok(peeked) = bits.peek();
    let Ok(at) = index(peeked.wrapping_shr(LONGEST.saturating_sub(QUICK)));

    match self.quick.get(at) {
      Some(Quick::Code { length, symbol }) => {
        let Ok(()) = bits.skip(u32::from(*length));

        Ok(*symbol)
      }
      Some(Quick::Longer) | None => {
        let (length, symbol) = self.longer(peeked)?;
        let Ok(()) = bits.skip(length);

        Ok(symbol)
      }
    }
  }

  #[inline(never)]
  fn longer(&self, peeked: u32) -> Result<(u32, u8), JpegError> {
    for (length, run) in (0u32..).zip(&self.runs).skip(10) {
      let code = peeked.wrapping_shr(LONGEST.saturating_sub(length));

      let found = match run {
        Some(run_2) => match (code.cmp(&run_2.last), code.checked_sub(run_2.first)) {
          (Ordering::Less | Ordering::Equal, Some(past)) => Some(run_2.at.saturating_add(past)),
          (Ordering::Less | Ordering::Equal, None) => return Err(JpegError::Corrupt),
          (Ordering::Greater, _) => None,
        },
        None => None,
      };

      match found {
        Some(at) => {
          let Ok(at_2) = index(at);

          return match self.symbols.get(at_2) {
            Some(symbol) => Ok((length, *symbol)),
            None => Err(JpegError::Corrupt),
          };
        }
        None => {}
      }
    }

    Err(JpegError::Corrupt)
  }
}
