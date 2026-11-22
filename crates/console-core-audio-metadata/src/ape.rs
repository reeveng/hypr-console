//! The APE item list, read from its footer before the optional ID3v1 trailer.

use std::io::{Read, Seek};

use crate::bytes::{Cursor, Source};
use crate::fault::{Malformed, ParseError};
use crate::keys;
use crate::tag::{Tag, TagType};

pub(crate) fn trailer<R: Read + Seek>(
  source: &mut Source<'_, R>,
  end: u64,
) -> Result<Option<(Tag, u64)>, ParseError> {
  let footer = match end.checked_sub(32) {
    Some(footer) => footer,
    None => return Ok(None),
  };

  source.seek_to(footer)?;

  let header = source.array::<32>()?;

  match header.strip_prefix(b"APETAGEX") {
    None => return Ok(None),
    Some(_fields) => {}
  }

  let Ok(mut fields) = Cursor::new(&header);

  fields.skip(8)?;

  let _version = fields.u32_le()?;
  let size = fields.u32_le()?;
  let count = fields.u32_le()?;
  let flags = fields.u32_le()?;
  let start = end.checked_sub(u64::from(size)).ok_or(Malformed::TagSize)?;
  let length = size.checked_sub(32).ok_or(Malformed::TagSize)?;

  source.seek_to(start)?;

  let bytes = source.exact(u64::from(length))?;
  let tag = parsed(&bytes, count)?;
  let start_2 = match flags & 0x8000_0000 {
    0 => Ok(start),
    _header => start.checked_sub(32).ok_or(Malformed::TagSize),
  };
  let start_3 = start_2?;

  Ok(Some((tag, start_3)))
}

struct Item {
  name: String,
  text: Option<String>,
}

fn parsed(content: &[u8], count: u32) -> Result<Tag, Malformed> {
  let Ok(mut reader) = Cursor::new(content);
  let mut items: Vec<Item> = Vec::new();

  for _item in 0..count {
    let Ok(left) = reader.left();

    match left {
      0..=10 => break,
      _has_item => {}
    }

    let size = reader.u32_le()?;
    let flags = reader.u32_le()?;
    let mut key = Vec::new();

    'key: for _byte in 0..left {
      let value_2 = reader.byte()?;

      match value_2 {
        0 => break 'key,
        byte => key.push(byte),
      }
    }

    let name = crate::text::utf8(&key)?;
    let value = reader.take(u64::from(size))?;
    let text = match flags.wrapping_shr(1) & 3 {
      0 => {
        let value_3 = crate::text::utf8(value)?;

        Some(value_3)
      }
      1 | 2 => None,
      _invalid => return Err(Malformed::Item),
    };

    items.retain(|item| !item.name.eq_ignore_ascii_case(&name));
    items.push(Item { name, text });
  }

  let Ok(mut tag) = Tag::new(TagType::Ape);

  for item in items {
    let Ok(key) = keys::named(keys::APE, &item.name);

    match (key, item.text) {
      (Some(key_2), Some(text)) => {
        for value in text.split('\0') {
          let Ok(()) = tag.push_text(key_2, value.to_string());
        }
      }
      (None, _) | (Some(_), None) => {}
    }
  }

  Ok(tag)
}
