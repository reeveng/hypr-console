//! Text as ID3v2 writes it: four encodings, an end that is either a
//! terminator or the end of the frame, and UTF-16 that says its byte order
//! with a mark or does not.
//!
//! Each rule is lofty's, because a frame that reads differently reads as a
//! different song: Latin-1 and UTF-8 lose their trailing NULs, a UTF-16 text
//! with no terminator has to open with a byte order mark, every pair that
//! spells a mark is dropped wherever it falls, an odd byte at the end of
//! UTF-16 is not a character, and a terminated text that is empty is empty
//! whatever its encoding says it should have begun with.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::bytes::{Cursor, length};
use crate::fault::Malformed;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Encoding {
  Latin1,
  Utf16,
  Utf16BigEndian,
  Utf8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ending {
  Terminated,
  Rest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ByteOrder {
  Big,
  Little,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Decoded {
  pub(crate) text: String,
  pub(crate) mark: [u8; 2],
}

impl Encoding {
  pub(crate) fn from_byte(byte: u8) -> Result<Encoding, Malformed> {
    match byte {
      0 => Ok(Encoding::Latin1),
      1 => Ok(Encoding::Utf16),
      2 => Ok(Encoding::Utf16BigEndian),
      3 => Ok(Encoding::Utf8),
      _other => Err(Malformed::Encoding),
    }
  }
}

pub(crate) fn latin1(bytes: &[u8]) -> Result<String, Never> {
  let text: String = bytes.iter().map(|byte| char::from(*byte)).collect();

  Ok(text.trim_end_matches('\0').to_string())
}

pub(crate) fn utf8(bytes: &[u8]) -> Result<String, Malformed> {
  match std::str::from_utf8(bytes) {
    Ok(text) => Ok(text.trim_end_matches('\0').to_string()),
    Err(_not_utf8) => Err(Malformed::Text),
  }
}

pub(crate) fn utf16(bytes: &[u8], order: ByteOrder) -> Result<String, Malformed> {
  let (pairs, _odd) = bytes.as_chunks::<2>();

  let units: Vec<u16> = pairs
    .iter()
    .filter(|pair| !matches!(pair, [0xFF, 0xFE] | [0xFE, 0xFF]))
    .map(|pair| match order {
      ByteOrder::Big => u16::from_be_bytes(*pair),
      ByteOrder::Little => u16::from_le_bytes(*pair),
    })
    .collect();

  match String::from_utf16(&units) {
    Ok(text) => Ok(text.trim_end_matches('\0').to_string()),
    Err(_not_utf16) => Err(Malformed::Text),
  }
}

pub(crate) fn terminated<'a>(
  reader: &mut Cursor<'a>,
  encoding: Encoding,
) -> Result<&'a [u8], Never> {
  let Ok(rest) = reader.rest();

  let text_length = match encoding {
    Encoding::Latin1 | Encoding::Utf8 => {
      let before = rest.split(|byte| *byte == 0).next();

      match before {
        Some(text) => length(text),
        None => Ok(0),
      }
    }
    Encoding::Utf16 | Encoding::Utf16BigEndian => {
      let (pairs, _odd_byte) = rest.as_chunks::<2>();
      let Ok(pairs_2) =
        fitted::<_, u64>(pairs.iter().take_while(|pair| **pair != [0, 0]).count());

      Ok(pairs_2.saturating_mul(2))
    }
  };

  let Ok(text_length_2) = text_length;

  let terminator = match encoding {
    Encoding::Latin1 | Encoding::Utf8 => 1,
    Encoding::Utf16 | Encoding::Utf16BigEndian => 2,
  };

  let Ok(left) = reader.left();
  let consumed = text_length_2.saturating_add(terminator).min(left);
  let Ok(taken) = reader.upto(consumed);

  let Ok(end) = index(text_length_2);

  Ok(match taken.get(..end) {
    Some(text) => text,
    None => taken,
  })
}

pub(crate) fn decoded(
  reader: &mut Cursor<'_>,
  encoding: Encoding,
  ending: Ending,
) -> Result<Decoded, Malformed> {
  let raw = match ending {
    Ending::Terminated => {
      let Ok(raw) = terminated(reader, encoding);

      raw
    }
    Ending::Rest => {
      let Ok(raw) = reader.rest();
      let Ok(()) = reader.finish();

      raw
    }
  };

  match raw {
    [] => {
      return Ok(Decoded {
        text: String::new(),
        mark: [0, 0],
      });
    }
    [_, ..] => {}
  }

  match encoding {
    Encoding::Latin1 => {
      let Ok(text) = latin1(raw);

      Ok(Decoded { text, mark: [0, 0] })
    }
    Encoding::Utf8 => {
      let value_1 = utf8(raw)?;

      Ok(Decoded {
        text: value_1,
        mark: [0, 0],
      })
    }
    Encoding::Utf16BigEndian => {
      let value_2 = utf16(raw, ByteOrder::Big)?;

      Ok(Decoded {
        text: value_2,
        mark: [0, 0],
      })
    }
    Encoding::Utf16 => marked(raw),
  }
}

fn marked(raw: &[u8]) -> Result<Decoded, Malformed> {
  match raw.as_chunks::<2>().1 {
    [] => {}
    [_] | [_, _, ..] => return Err(Malformed::Text),
  }

  match raw {
    [0xFE, 0xFF, ..] => {
      let value_3 = utf16(raw, ByteOrder::Big)?;

      Ok(Decoded {
        text: value_3,
        mark: [0xFE, 0xFF],
      })
    }
    [0xFF, 0xFE, ..] => {
      let value_4 = utf16(raw, ByteOrder::Little)?;

      Ok(Decoded {
        text: value_4,
        mark: [0xFF, 0xFE],
      })
    }
    _unmarked => Err(Malformed::Text),
  }
}

pub(crate) fn order_of(mark: [u8; 2]) -> Result<Option<ByteOrder>, Never> {
  Ok(match mark {
    [0xFE, 0xFF] => Some(ByteOrder::Big),
    [0xFF, 0xFE] => Some(ByteOrder::Little),
    _unmarked => None,
  })
}

pub(crate) fn terminated_maybe_marked(
  reader: &mut Cursor<'_>,
  order: ByteOrder,
) -> Result<String, Malformed> {
  let Ok(raw) = terminated(reader, Encoding::Utf16);

  match raw {
    [0xFF, 0xFE, rest @ ..] => utf16(rest, ByteOrder::Little),
    [0xFE, 0xFF, rest @ ..] => utf16(rest, ByteOrder::Big),
    _unmarked => utf16(raw, order),
  }
}
