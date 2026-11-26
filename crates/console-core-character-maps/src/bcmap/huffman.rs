use console_core_number_conversion::index;

use super::reader::{Bits, Reader};
use crate::ParseError;

#[derive(Debug, Default)]
pub(crate) struct Huffman {
  counts: Vec<u16>,
  symbols: Vec<u32>,
}

#[derive(Debug, Clone, Copy)]
enum Symbol {
  Word,
  Byte,
}

impl Huffman {
  pub(crate) fn decode(&self, bits: &mut Bits<'_>) -> Result<u32, ParseError> {
    let mut code = 0_u32;
    let mut first = 0_u32;
    let mut before = 0_u32;

    for count in &self.counts {
      let bit = bits.bit()?;
      let wanted = u32::from(*count);

      code = code.saturating_mul(2).saturating_add(bit);

      match code.checked_sub(first).filter(|offset| *offset < wanted) {
        Some(offset) => {
          let Ok(at) = index(before.saturating_add(offset));

          return self.symbols.get(at).copied().ok_or(ParseError::BadCode);
        }
        None => {}
      }

      before = before.saturating_add(wanted);
      first = first.saturating_add(wanted).saturating_mul(2);
    }

    Err(ParseError::BadCode)
  }
}

pub(crate) fn tables(data: &[u8]) -> Result<(Huffman, Huffman), ParseError> {
  let Ok(mut reader) = Reader::new(data);
  let delta = table(&mut reader, Symbol::Word)?;
  let count = table(&mut reader, Symbol::Byte)?;

  Ok((delta, count))
}

fn symbol(reader: &mut Reader<'_>, kind: Symbol) -> Result<u32, ParseError> {
  match kind {
    Symbol::Word => reader.u32(),
    Symbol::Byte => reader.u8().map(u32::from),
  }
}

fn table(reader: &mut Reader<'_>, kind: Symbol) -> Result<Huffman, ParseError> {
  let total = reader.u16()?;
  let longest = reader.u8()?;
  let mut counts = Vec::new();
  let mut symbols = Vec::new();

  match total {
    0 => return Ok(Huffman::default()),
    _some => {}
  }

  for _length in 0..longest {
    let count = reader.u16()?;

    counts.push(count);
  }

  for count in &counts {
    for _symbol in 0..*count {
      let found = symbol(reader, kind)?;

      symbols.push(found);
    }
  }

  Ok(Huffman { counts, symbols })
}
