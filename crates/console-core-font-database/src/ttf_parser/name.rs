//! A font's `name` table: the records of its names, each with the platform, encoding and language it is in.
//!
//! The names are walked in the order the table lists them, and the walk ends at
//! the first record whose bytes are not inside the table's storage, which is
//! where the upstream reader stopped too. Unicode is the Unicode platform, or
//! Windows in its symbol or BMP encoding. A Windows record says its language by
//! the Windows identifier; a Macintosh Roman record in language zero is
//! American English, and every other language is unknown here.

use super::Language;
use super::parser::{FromData, LazyArray16, Offset, Offset16, Stream};
use console_core_never::Never;
use core::cmp::Ordering;

pub mod name_id {
  pub const FAMILY: u16 = 1;
  pub const POST_SCRIPT_NAME: u16 = 6;
  pub const TYPOGRAPHIC_FAMILY: u16 = 16;
}

const WINDOWS_SYMBOL_ENCODING_ID: u16 = 0;
const WINDOWS_UNICODE_BMP_ENCODING_ID: u16 = 1;
const MACINTOSH_ROMAN_ENCODING_ID: u16 = 0;
const MACINTOSH_ENGLISH_LANGUAGE_ID: u16 = 0;
const LANG_TAG_RECORD_SIZE: u16 = 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlatformId {
  Unicode,
  Macintosh,
  Iso,
  Windows,
  Custom,
}

impl FromData for PlatformId {
  const SIZE: u32 = 2;

  #[inline]
  fn parse(data: &[u8]) -> Option<Self> {
    let id = u16::parse(data)?;

    match id {
      0 => Some(PlatformId::Unicode),
      1 => Some(PlatformId::Macintosh),
      2 => Some(PlatformId::Iso),
      3 => Some(PlatformId::Windows),
      4 => Some(PlatformId::Custom),
      _ => None,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnicodeEncoding {
  Unicode,
  Other,
}

#[derive(Clone, Copy)]
struct NameRecord {
  platform_id: PlatformId,
  encoding_id: u16,
  language_id: u16,
  name_id: u16,
  length: u16,
  offset: Offset16,
}

impl FromData for NameRecord {
  const SIZE: u32 = 12;

  #[inline]
  fn parse(data: &[u8]) -> Option<Self> {
    let Ok(mut s) = Stream::new(data);
    let Ok(platform_id) = s.read::<PlatformId>();
    let Ok(encoding_id) = s.read::<u16>();
    let Ok(language_id) = s.read::<u16>();
    let Ok(name_id) = s.read::<u16>();
    let Ok(length) = s.read::<u16>();
    let Ok(offset) = s.read::<Offset16>();

    match (
      platform_id,
      encoding_id,
      language_id,
      name_id,
      length,
      offset,
    ) {
      (
        Some(platform_id_2),
        Some(encoding_id_2),
        Some(language_id_2),
        Some(name_id_2),
        Some(length_2),
        Some(offset_2),
      ) => Some(NameRecord {
        platform_id: platform_id_2,
        encoding_id: encoding_id_2,
        language_id: language_id_2,
        name_id: name_id_2,
        length: length_2,
        offset: offset_2,
      }),
      _ => None,
    }
  }
}

#[derive(Clone, Copy)]
pub struct Name<'a> {
  pub platform_id: PlatformId,

  pub encoding_id: u16,

  pub language_id: u16,

  pub name_id: u16,

  pub name: &'a [u8],
}

impl Name<'_> {
  #[inline]
  pub fn unicode_encoding(&self) -> Result<UnicodeEncoding, Never> {
    Ok(match (self.platform_id, self.encoding_id) {
      (PlatformId::Unicode, _)
      | (PlatformId::Windows, WINDOWS_SYMBOL_ENCODING_ID | WINDOWS_UNICODE_BMP_ENCODING_ID) => {
        UnicodeEncoding::Unicode
      }
      (PlatformId::Windows | PlatformId::Macintosh | PlatformId::Iso | PlatformId::Custom, _) => {
        UnicodeEncoding::Other
      }
    })
  }

  pub fn language(&self) -> Result<Language, Never> {
    match (self.platform_id, self.encoding_id, self.language_id) {
      (PlatformId::Windows, _, language_id) => Language::windows_language(language_id),
      (PlatformId::Macintosh, MACINTOSH_ROMAN_ENCODING_ID, MACINTOSH_ENGLISH_LANGUAGE_ID) => {
        Ok(Language::English_UnitedStates)
      }
      (
        PlatformId::Unicode | PlatformId::Macintosh | PlatformId::Iso | PlatformId::Custom,
        _,
        _,
      ) => Ok(Language::Unknown),
    }
  }
}

#[derive(Clone, Copy, Default)]
pub struct Names<'a> {
  records: LazyArray16<'a, NameRecord>,
  storage: &'a [u8],
}

impl<'a> Names<'a> {
  pub fn get(&self, index: u16) -> Result<Option<Name<'a>>, Never> {
    let Ok(found) = self.records.get(index);
    let record = match found {
      Some(record) => record,
      None => return Ok(None),
    };
    let Ok(name_start) = record.offset.position();
    let name_end = name_start.saturating_add(u64::from(record.length));
    let Ok(name_span) = console_core_number_conversion::indices(name_start..name_end);

    Ok(self.storage.get(name_span).map(|name| Name {
      platform_id: record.platform_id,
      encoding_id: record.encoding_id,
      language_id: record.language_id,
      name_id: record.name_id,
      name,
    }))
  }

  pub fn len(&self) -> Result<u16, Never> {
    self.records.len()
  }
}

impl core::fmt::Debug for Names<'_> {
  fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
    write!(f, "Names {{ ... }}")
  }
}

impl<'a> IntoIterator for Names<'a> {
  type Item = Name<'a>;
  type IntoIter = NamesIter<'a>;

  #[inline]
  fn into_iter(self) -> Self::IntoIter {
    NamesIter {
      names: self,
      index: 0,
    }
  }
}

#[derive(Clone, Copy)]
pub struct NamesIter<'a> {
  names: Names<'a>,
  index: u16,
}

impl<'a> Iterator for NamesIter<'a> {
  type Item = Name<'a>;

  fn next(&mut self) -> Option<Self::Item> {
    let Ok(length) = self.names.len();

    match self.index.cmp(&length) {
      Ordering::Less => {
        let Ok(name) = self.names.get(self.index);

        self.index = self.index.saturating_add(1);

        name
      }
      Ordering::Equal | Ordering::Greater => None,
    }
  }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Table<'a> {
  pub names: Names<'a>,
}

impl<'a> Table<'a> {
  pub fn parse(data: &'a [u8]) -> Result<Option<Self>, Never> {
    let Ok(mut s) = Stream::new(data);
    let Ok(version) = s.read::<u16>();
    let Ok(count) = s.read::<u16>();
    let Ok(storage) = s.read::<Offset16>();
    let (version_3, count_3, storage_3) = match (version, count, storage) {
      (Some(version_2), Some(count_2), Some(storage_2)) => (version_2, count_2, storage_2),
      _ => return Ok(None),
    };
    let Ok(storage_offset) = storage_3.position();

    match version_3 {
      0 => {}
      1 => {
        let Ok(read_tags) = s.read::<u16>();
        let tag_bytes = read_tags.and_then(|tags| tags.checked_mul(LANG_TAG_RECORD_SIZE));

        match tag_bytes {
          Some(tag_bytes_2) => s.advance(u64::from(tag_bytes_2))?,
          None => return Ok(None),
        }
      }
      _ => return Ok(None),
    }

    let Ok(read_records) = s.read_array16::<NameRecord>(count_3);
    let records = match read_records {
      Some(records) => records,
      None => return Ok(None),
    };
    let Ok(here) = s.offset();
    let Ok(()) = s.advance(storage_offset.saturating_sub(here));
    let Ok(tail) = s.tail();

    Ok(tail.map(|storage_4| Table {
      names: Names {
        records,
        storage: storage_4,
      },
    }))
  }
}
