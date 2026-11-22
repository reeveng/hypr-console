//! Decode Lempel-Ziv-Welch streams using a dictionary of ranges in the output.
//!
//! Each new dictionary entry is the previous code's bytes followed by the first
//! byte of the current code. Both have just been written, so an entry can point
//! into the output. Codes widen up to twelve bits as the dictionary fills,
//! and the clear code resets it to the original single-byte entries.
//! The literal width excludes the clear and end codes. An output limit can
//! stop decoding partway through a dictionary entry. Callers decide whether
//! exhausted input before an explicit end code is acceptable.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::vec::Vec;
use core::cmp::Ordering;

use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, indices};

const MAX_CODE_WIDTH: u32 = 12;
const DICTIONARY_SIZE: u32 = 1 << MAX_CODE_WIDTH;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitOrder {
  LeastSignificantFirst,
  MostSignificantFirst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidthGrowth {
  AtCapacity,
  OneCodeEarly,
}

#[derive(Debug, Clone, Copy)]
pub struct Parameters {
  pub bit_order: BitOrder,
  pub literal_width: u32,
  pub width_growth: WidthGrowth,
  pub output_limit: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LzwError {
  InvalidLiteralWidth,
  InvalidCode,
}

impl core::fmt::Display for LzwError {
  fn fmt(&self, to: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::InvalidLiteralWidth => write!(to, "LZW literal width must be between 1 and 8"),
      Self::InvalidCode => write!(to, "LZW code is missing from the dictionary"),
    }
  }
}

impl core::error::Error for LzwError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
  Code,
  Input,
  OutputLimit,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Decoded {
  pub bytes: Vec<u8>,
  pub end: End,
}

pub fn decode(packed: &[u8], parameters: Parameters) -> Result<Decoded, LzwError> {
  match parameters.literal_width {
    1..=8 => {}
    _ => return Err(LzwError::InvalidLiteralWidth),
  }

  let clear = 1u32.wrapping_shl(parameters.literal_width);
  let mut decoder = Decoder {
    codes: Codes {
      bytes: packed.iter(),
      order: parameters.bit_order,
      held: 0,
      count: 0,
    },
    dictionary: (0..DICTIONARY_SIZE)
      .map(|_| Written { start: 0, len: 0 })
      .collect(),
    output: Vec::new(),
    limit: parameters.output_limit,
    clear,
    early: match parameters.width_growth {
      WidthGrowth::AtCapacity => 0,
      WidthGrowth::OneCodeEarly => 1,
    },
    next: 0,
    width: 0,
    previous: None,
  };
  let Ok(()) = decoder.reset();

  let result = iterate(&mut decoder, |decoder_2| {
    Ok(match decoder_2.step() {
      Ok(None) => Step::Again(decoder_2),
      Ok(Some(end)) => Step::Halt(Ok(end)),
      Err(fault) => Step::Halt(Err(fault)),
    })
  });

  let end = match result {
    Ok(end) => end?,
    Err(Endless) => return Err(LzwError::InvalidCode),
  };

  Ok(Decoded {
    bytes: decoder.output,
    end,
  })
}

struct Codes<'a> {
  bytes: core::slice::Iter<'a, u8>,
  order: BitOrder,
  held: u32,
  count: u32,
}

impl Codes<'_> {
  #[inline]
  fn read(&mut self, width: u32) -> Result<Option<u32>, Never> {
    for _ in 0..width.saturating_sub(self.count).div_ceil(8) {
      let byte = match self.bytes.next() {
        Some(&byte) => u32::from(byte),
        None => return Ok(None),
      };

      self.held = match self.order {
        BitOrder::LeastSignificantFirst => self.held | byte.wrapping_shl(self.count),
        BitOrder::MostSignificantFirst => self.held.wrapping_shl(8) | byte,
      };
      self.count = self.count.saturating_add(8);
    }

    self.count = self.count.saturating_sub(width);
    let mask = 1u32.wrapping_shl(width).wrapping_sub(1);
    let code = match self.order {
      BitOrder::LeastSignificantFirst => {
        let code = self.held & mask;

        self.held = self.held.wrapping_shr(width);
        code
      }
      BitOrder::MostSignificantFirst => self.held.wrapping_shr(self.count) & mask,
    };

    Ok(Some(code))
  }
}

#[derive(Clone, Copy)]
struct Written {
  start: u64,
  len: u64,
}

enum WidthUpdate {
  Widen,
  Retain,
}

impl From<bool> for WidthUpdate {
  fn from(at_growth_point: bool) -> Self {
    match bool::then_some(at_growth_point, ()) {
      Some(()) => Self::Widen,
      None => Self::Retain,
    }
  }
}

struct Decoder<'a> {
  codes: Codes<'a>,
  dictionary: Vec<Written>,
  output: Vec<u8>,
  limit: Option<u64>,
  clear: u32,
  early: u32,
  next: u32,
  width: u32,
  previous: Option<Written>,
}

impl Decoder<'_> {
  fn reset(&mut self) -> Result<(), Never> {
    self.next = self.clear.saturating_add(2);
    self.width = self.clear.trailing_zeros().saturating_add(1);
    self.previous = None;

    Ok(())
  }

  #[inline]
  fn step(&mut self) -> Result<Option<End>, LzwError> {
    let Ok(start) = fitted::<_, u64>(self.output.len());

    match self.limit {
      Some(limit) => match start.cmp(&limit) {
        Ordering::Equal | Ordering::Greater => {
          let Ok(limit_index) = index(limit);
          self.output.truncate(limit_index);

          return Ok(Some(End::OutputLimit));
        }
        Ordering::Less => {}
      },
      None => {}
    }

    let Ok(read_code) = self.codes.read(self.width);
    let code = match read_code {
      Some(code) => code,
      None => return Ok(Some(End::Input)),
    };

    match (
      code.checked_sub(self.clear),
      code.cmp(&self.next),
      self.previous,
    ) {
      (Some(0), _, _) => {
        let Ok(()) = self.reset();

        return Ok(None);
      }
      (Some(1), _, _) => return Ok(Some(End::Code)),
      (None, _, _) => {
        let byte = u8::try_from(code).map_err(|_| LzwError::InvalidCode)?;
        self.output.push(byte);
      }
      (Some(2..), Ordering::Less, _) => {
        let Ok(code_index) = index(code);
        let entry = self
          .dictionary
          .get(code_index)
          .ok_or(LzwError::InvalidCode)?;
        let Ok(range) = indices(entry.start..entry.start.saturating_add(entry.len));

        self.output.extend_from_within(range);
      }
      (Some(2..), Ordering::Equal, Some(previous)) => {
        let Ok(previous_index) = index(previous.start);
        let first = self
          .output
          .get(previous_index)
          .copied()
          .ok_or(LzwError::InvalidCode)?;
        let Ok(range) = indices(previous.start..previous.start.saturating_add(previous.len));

        self.output.extend_from_within(range);
        self.output.push(first);
      }
      (Some(2..), Ordering::Equal, None) | (Some(2..), Ordering::Greater, _) => {
        return Err(LzwError::InvalidCode);
      }
    }

    let Ok(()) = self.register();
    let Ok(end) = fitted::<_, u64>(self.output.len());
    self.previous = Some(Written {
      start,
      len: end.saturating_sub(start),
    });

    Ok(None)
  }

  #[inline]
  fn register(&mut self) -> Result<(), Never> {
    let Ok(next_index) = index(self.next);
    let (previous, slot) = match (self.previous, self.dictionary.get_mut(next_index)) {
      (Some(previous), Some(slot)) => (previous, slot),
      (None, _) | (_, None) => return Ok(()),
    };

    *slot = Written {
      start: previous.start,
      len: previous.len.saturating_add(1),
    };
    self.next = self.next.saturating_add(1);

    match WidthUpdate::from(
      self.next.saturating_add(self.early) == 1u32.wrapping_shl(self.width)
        && self.width < MAX_CODE_WIDTH,
    ) {
      WidthUpdate::Widen => self.width = self.width.saturating_add(1),
      WidthUpdate::Retain => {}
    }

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[must_use]
  fn msb_codes(codes: impl IntoIterator<Item = (u32, u32)>) -> Result<Vec<u8>, Never> {
    let mut bytes = Vec::new();
    let mut pending = 0u8;
    let mut count = 0u32;

    for (code, width) in codes {
      for bit in (0..width).rev() {
        pending = pending.wrapping_shl(1) | u8::from(code & 1u32.wrapping_shl(bit) != 0);
        count = count.saturating_add(1);

        match count {
          8 => {
            bytes.push(pending);
            count = 0;
          }
          _ => {}
        }
      }
    }

    match count {
      0 => {}
      _ => bytes.push(pending.wrapping_shl(8u32.saturating_sub(count))),
    }

    Ok(bytes)
  }

  #[test]
  fn the_width_growth_selects_how_many_bits_the_next_code_uses() -> Result<(), LzwError> {
    for (width_growth, end_width) in [
      (WidthGrowth::AtCapacity, 9),
      (WidthGrowth::OneCodeEarly, 10),
    ] {
      let Ok(packed) = msb_codes(
        [(256, 9), (65, 9)]
          .into_iter()
          .chain(core::iter::repeat_n((0, 9), 253))
          .chain([(257, end_width)]),
      );
      let decoded = decode(
        &packed,
        Parameters {
          bit_order: BitOrder::MostSignificantFirst,
          literal_width: 8,
          width_growth,
          output_limit: None,
        },
      )?;

      assert_eq!(decoded.end, End::Code);
      assert_eq!(decoded.bytes.len(), 254);
      assert_eq!(decoded.bytes.first(), Some(&65));
      assert!(decoded.bytes.iter().skip(1).all(|&byte| byte == 0));
    }

    Ok(())
  }

  #[test]
  fn an_output_limit_can_stop_partway_through_a_dictionary_entry() -> Result<(), LzwError> {
    let decoded = decode(
      &[0b1000_1100, 0b0000_1011],
      Parameters {
        bit_order: BitOrder::LeastSignificantFirst,
        literal_width: 2,
        width_growth: WidthGrowth::AtCapacity,
        output_limit: Some(2),
      },
    )?;

    assert_eq!(decoded.end, End::OutputLimit);
    assert_eq!(decoded.bytes, [1, 1]);

    Ok(())
  }
}
