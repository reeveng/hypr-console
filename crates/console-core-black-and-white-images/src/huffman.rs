//! Huffman tables: the standard ones, the ones a segment carries, and the ones made to fit a text region.
//!
//! A table is a list of lines, each a range of values, the length of the prefix that says it and how many
//! bits follow to say where in the range. Codes of one length are handed out in the order the lines are
//! listed (B.3), and decoding walks a tree of them one bit at a time.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::error::DecodeError;
use crate::reading::Reader;
use crate::standard::Standard;

const LONGEST_PREFIX: u32 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
  Range,
  Lower,
  OutOfBand,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Line {
  pub(crate) low: i32,
  pub(crate) prefix: u8,
  pub(crate) range: u8,
  pub(crate) kind: Kind,
}

#[derive(Debug, Clone, Copy)]
enum Node {
  Branch { zero: Option<u32>, one: Option<u32> },
  Leaf(Line),
}

#[derive(Debug, Clone)]
pub(crate) struct HuffmanTable {
  nodes: Vec<Node>,
}

impl HuffmanTable {
  pub(crate) fn standard(which: Standard) -> Result<HuffmanTable, DecodeError> {
    let Ok(lines) = which.lines();

    HuffmanTable::build(lines)
  }

  pub(crate) fn uniform(symbols: u32, length: u8) -> Result<HuffmanTable, DecodeError> {
    let mut lines = Vec::new();

    for value in 0..symbols {
      let low = i32::try_from(value).map_err(|_| DecodeError::Overflow)?;

      lines.push(Line {
        low,
        prefix: length,
        range: 0,
        kind: Kind::Range,
      });
    }

    HuffmanTable::build(&lines)
  }

  pub(crate) fn build(lines: &[Line]) -> Result<HuffmanTable, DecodeError> {
    let codes = assign(lines)?;
    let mut nodes = vec![Node::Branch {
      zero: None,
      one: None,
    }];

    for (line, code) in lines.iter().zip(codes) {
      match line.prefix {
        0 => {}
        _ => insert(&mut nodes, line, code)?,
      }
    }

    Ok(HuffmanTable { nodes })
  }

  pub(crate) fn decode(&self, reader: &mut Reader<'_>) -> Result<Option<i32>, DecodeError> {
    let mut at = 0u32;

    for _step in 0..=LONGEST_PREFIX {
      let Ok(slot) = index(at);

      match self.nodes.get(slot) {
        Some(Node::Branch { zero, one }) => {
          let bit = reader.bit()?;
          let next = match bit {
            0 => zero,
            _ => one,
          };
          let following = (*next).ok_or(DecodeError::InvalidHuffmanCode)?;

          at = following;
        }
        Some(Node::Leaf(line)) => return value(line, reader),
        None => return Err(DecodeError::InvalidHuffmanCode),
      }
    }

    Err(DecodeError::InvalidHuffmanCode)
  }

  pub(crate) fn decode_in_band(&self, reader: &mut Reader<'_>) -> Result<i32, DecodeError> {
    let decoded = self.decode(reader)?;

    decoded.ok_or(DecodeError::UnexpectedOutOfBand)
  }

  pub(crate) fn read_custom(reader: &mut Reader<'_>) -> Result<HuffmanTable, DecodeError> {
    let flags = reader.byte()?;
    let prefix_bits = u32::from(flags.wrapping_shr(1) & 7).saturating_add(1);
    let range_bits = u32::from(flags.wrapping_shr(4) & 7).saturating_add(1);
    let minimum = reader.i32()?;
    let maximum = reader.i32()?;
    let mut lines = Vec::new();
    let mut low = minimum;
    let Ok(budget) = reader.remaining();

    for _line in 0..budget.saturating_mul(8) {
      match ::core::primitive::bool::then_some(low < maximum, ()) {
        ::core::option::Option::None => break,
        ::core::option::Option::Some(()) => {
          let prefix = prefix_in(reader, prefix_bits)?;
          let range = prefix_in(reader, range_bits)?;
          let size = 1i64
            .checked_shl(u32::from(range))
            .ok_or(DecodeError::InvalidHuffmanCode)?;
          let next = i64::from(low)
            .checked_add(size)
            .ok_or(DecodeError::InvalidHuffmanCode)?;

          lines.push(Line {
            low,
            prefix,
            range,
            kind: Kind::Range,
          });
          let advanced = i32::try_from(next).map_err(|_| DecodeError::InvalidHuffmanCode)?;

          low = advanced;
        }
      }
    }

    let below = minimum
      .checked_sub(1)
      .ok_or(DecodeError::InvalidHuffmanCode)?;
    let lower = prefix_in(reader, prefix_bits)?;
    let upper = prefix_in(reader, prefix_bits)?;

    lines.push(Line {
      low: below,
      prefix: lower,
      range: 32,
      kind: Kind::Lower,
    });
    lines.push(Line {
      low,
      prefix: upper,
      range: 32,
      kind: Kind::Range,
    });

    match flags & 1 {
      0 => {}
      _ => {
        let prefix = prefix_in(reader, prefix_bits)?;

        lines.push(Line {
          low: 0,
          prefix,
          range: 0,
          kind: Kind::OutOfBand,
        });
      }
    }

    HuffmanTable::build(&lines)
  }
}

pub(crate) fn prefix_of(bits: u32) -> Result<u8, Never> {
  fitted::<_, u8>(bits)
}

fn prefix_in(reader: &mut Reader<'_>, count: u32) -> Result<u8, DecodeError> {
  let taken = reader.bits(count)?;
  let Ok(prefix) = prefix_of(taken);

  Ok(prefix)
}

fn assign(lines: &[Line]) -> Result<Vec<u32>, DecodeError> {
  let longest = match lines.iter().map(|line| u32::from(line.prefix)).max() {
    Some(longest) => longest,
    None => 0,
  };

  match ::core::primitive::bool::then_some(longest > LONGEST_PREFIX, ()) {
    ::core::option::Option::Some(()) => return Err(DecodeError::PrefixTooLong),
    ::core::option::Option::None => {}
  }

  let Ok(width) = index(longest.saturating_add(1));
  #[cfg_attr(
    dylint_lib = "explicit088_no_arriving_length",
    expect(
      explicit088_no_arriving_length,
      reason = "the measured prefix length is checked against LONGEST_PREFIX before allocating one count per length including zero"
    )
  )]
  let mut counts = vec![0u32; width];

  for line in lines {
    let Ok(length) = index(line.prefix);

    match counts.get_mut(length) {
      Some(count) => *count = count.saturating_add(1),
      None => return Err(DecodeError::InvalidHuffmanCode),
    }
  }

  match counts.first_mut() {
    Some(count) => *count = 0,
    None => return Err(DecodeError::InvalidHuffmanCode),
  }

  let mut codes = vec![0u32; lines.len()];
  let mut first = 0u32;

  for length in 1..=longest {
    let Ok(below) = index(length.saturating_sub(1));
    let held = counts
      .get(below)
      .copied()
      .ok_or(DecodeError::InvalidHuffmanCode)?;

    let doubled = first
      .saturating_add(held)
      .checked_mul(2)
      .ok_or(DecodeError::InvalidHuffmanCode)?;

    first = doubled;

    let mut next = first;

    for (line, code) in lines.iter().zip(codes.iter_mut()) {
      match ::core::primitive::bool::then_some(u32::from(line.prefix) == length, ()) {
        ::core::option::Option::Some(()) => {
          let following = next.checked_add(1).ok_or(DecodeError::InvalidHuffmanCode)?;

          *code = next;
          next = following;
        }
        ::core::option::Option::None => {}
      }
    }
  }

  Ok(codes)
}

fn insert(nodes: &mut Vec<Node>, line: &Line, code: u32) -> Result<(), DecodeError> {
  let mut at = 0u32;

  for place in (0..u32::from(line.prefix)).rev() {
    let bit = code.wrapping_shr(place) & 1;
    let Ok(slot) = index(at);
    let child = match nodes.get(slot) {
      Some(Node::Branch { zero, one }) => match bit {
        0 => *zero,
        _ => *one,
      },
      Some(Node::Leaf(_)) | None => return Err(DecodeError::ConflictingCodes),
    };

    at = match child {
      Some(child_2) => child_2,
      None => {
        let Ok(made) = fitted::<_, u32>(nodes.len());

        nodes.push(Node::Branch {
          zero: None,
          one: None,
        });

        match nodes.get_mut(slot) {
          Some(Node::Branch { zero, one }) => match bit {
            0 => *zero = Some(made),
            _ => *one = Some(made),
          },
          Some(Node::Leaf(_)) | None => return Err(DecodeError::ConflictingCodes),
        }

        made
      }
    };
  }

  let Ok(slot) = index(at);

  match nodes.get_mut(slot) {
    Some(node) => *node = Node::Leaf(*line),
    None => return Err(DecodeError::ConflictingCodes),
  }

  Ok(())
}

fn value(line: &Line, reader: &mut Reader<'_>) -> Result<Option<i32>, DecodeError> {
  match line.kind {
    Kind::OutOfBand => Ok(None),
    Kind::Range | Kind::Lower => {
      let taken = reader.bits(u32::from(line.range))?;
      let offset = i64::from(taken);
      let low = i64::from(line.low);
      let placed = match line.kind {
        Kind::Lower => low.saturating_sub(offset),
        Kind::Range | Kind::OutOfBand => low.saturating_add(offset),
      };
      let found = i32::try_from(placed).map_err(|_| DecodeError::InvalidHuffmanCode)?;

      Ok(Some(found))
    }
  }
}

pub(crate) fn symbol_table(
  reader: &mut Reader<'_>,
  symbols: u32,
) -> Result<HuffmanTable, DecodeError> {
  let mut run_lines = Vec::new();

  for code in 0..35i32 {
    let prefix = prefix_in(reader, 4)?;

    run_lines.push(Line {
      low: code,
      prefix,
      range: 0,
      kind: Kind::Range,
    });
  }

  let runs = HuffmanTable::build(&run_lines)?;
  let mut lengths: Vec<u8> = Vec::new();
  let Ok(wanted) = index(symbols);

  for _step in 0..symbols {
    match ::core::primitive::bool::then_some(lengths.len() >= wanted, ()) {
      ::core::option::Option::Some(()) => break,
      ::core::option::Option::None => {}
    }

    let code = runs.decode_in_band(reader)?;

    repeat(&mut lengths, code, reader)?;
  }

  match ::core::primitive::bool::then_some(lengths.len() == wanted, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(DecodeError::InvalidHuffmanCode),
  }

  let Ok(()) = reader.align();

  let mut lines = Vec::new();

  for (symbol, length) in (0i32..).zip(&lengths) {
    lines.push(Line {
      low: symbol,
      prefix: *length,
      range: 0,
      kind: Kind::Range,
    });
  }

  HuffmanTable::build(&lines)
}

fn repeat(lengths: &mut Vec<u8>, code: i32, reader: &mut Reader<'_>) -> Result<(), DecodeError> {
  let (value, count) = match code {
    0..=31 => {
      let length = u8::try_from(code).map_err(|_| DecodeError::InvalidHuffmanCode)?;

      (length, 1u32)
    }
    32 => {
      let previous = lengths
        .last()
        .copied()
        .ok_or(DecodeError::InvalidHuffmanCode)?;
      let extra = reader.bits(2)?;

      (previous, extra.saturating_add(3))
    }
    33 => {
      let extra = reader.bits(3)?;

      (0, extra.saturating_add(3))
    }
    34 => {
      let extra = reader.bits(7)?;

      (0, extra.saturating_add(11))
    }
    _ => return Err(DecodeError::InvalidHuffmanCode),
  };

  for _copy in 0..count {
    lengths.push(value);
  }

  Ok(())
}
