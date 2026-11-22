//! The little of a TrueType or OpenType file the database reads: where its tables are.
//!
//! This is the part of ttf-parser that finds a face and its table records, and
//! with `name` and `OS/2` it is all the database asks a font file. A collection
//! is a header naming where each face starts, and each face starts with a magic
//! of its own; a face in a collection that is itself a collection is refused,
//! and a single font asked for any face but its first has no such face.

mod language;
pub mod name;
pub mod os2;
mod parser;

pub use language::Language;
pub use name::{PlatformId, name_id};
pub use os2::{Style, Width};
pub use parser::LazyArray16;

use console_core_never::Never;
use parser::{FromData, Offset, Offset32, Stream};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Magic {
  TrueType,
  OpenType,
  FontCollection,
}

impl FromData for Magic {
  const SIZE: u32 = 4;

  #[inline]
  fn parse(data: &[u8]) -> Option<Self> {
    let word = u32::parse(data)?;

    match word {
      0x00010000 | 0x74727565 => Some(Magic::TrueType),
      0x4F54544F => Some(Magic::OpenType),
      0x74746366 => Some(Magic::FontCollection),
      _ => None,
    }
  }
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Tag(pub u32);

impl Tag {
  #[inline]
  pub const fn from_bytes(bytes: &[u8; 4]) -> Result<Self, Never> {
    Ok(Tag(u32::from_be_bytes(*bytes)))
  }
}

impl FromData for Tag {
  const SIZE: u32 = 4;

  #[inline]
  fn parse(data: &[u8]) -> Option<Self> {
    u32::parse(data).map(Tag)
  }
}

#[derive(Clone, Copy)]
struct TableRecord {
  tag: Tag,
  offset: u32,
  length: u32,
}

impl FromData for TableRecord {
  const SIZE: u32 = 16;

  #[inline]
  fn parse(data: &[u8]) -> Option<Self> {
    let Ok(mut s) = Stream::new(data);
    let Ok(tag) = s.read::<Tag>();
    let Ok(()) = s.skip::<u32>();
    let Ok(offset) = s.read::<u32>();
    let Ok(length) = s.read::<u32>();

    tag
      .zip(offset)
      .zip(length)
      .map(|((tag_2, offset_2), length_2)| TableRecord {
        tag: tag_2,
        offset: offset_2,
        length: length_2,
      })
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FaceParsingError {
  MalformedFont,

  UnknownMagic,

  FaceIndexOutOfBounds,
}

#[derive(Clone, Copy)]
pub struct RawFace<'a> {
  pub data: &'a [u8],

  table_records: LazyArray16<'a, TableRecord>,
}

impl<'a> RawFace<'a> {
  pub fn parse(data: &'a [u8], index: u32) -> Result<Self, FaceParsingError> {
    let Ok(mut s) = Stream::new(data);
    let Ok(read_magic) = s.read::<Magic>();
    let font_magic = read_magic.ok_or(FaceParsingError::UnknownMagic)?;

    match font_magic {
      Magic::FontCollection => face_in_collection(&mut s, index)?,
      Magic::TrueType | Magic::OpenType => match index {
        0 => {}
        _ => return Err(FaceParsingError::FaceIndexOutOfBounds),
      },
    }

    let Ok(read_count) = s.read::<u16>();
    let table_count = read_count.ok_or(FaceParsingError::MalformedFont)?;
    let Ok(()) = s.advance(6);
    let Ok(read_records) = s.read_array16::<TableRecord>(table_count);
    let table_records = read_records.ok_or(FaceParsingError::MalformedFont)?;

    Ok(RawFace {
      data,
      table_records,
    })
  }

  pub fn table(&self, tag: Tag) -> Result<Option<&'a [u8]>, Never> {
    let Ok(found) = self
      .table_records
      .binary_search_by(|record| record.tag.cmp(&tag));
    let record = match found {
      Some((_, record)) => record,
      None => return Ok(None),
    };
    let offset = u64::from(record.offset);
    let end = match offset.checked_add(u64::from(record.length)) {
      Some(end) => end,
      None => return Ok(None),
    };
    let Ok(span) = console_core_number_conversion::indices(offset..end);

    Ok(self.data.get(span))
  }
}

impl core::fmt::Debug for RawFace<'_> {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "RawFace {{ ... }}")
  }
}

fn face_in_collection(s: &mut Stream, index: u32) -> Result<(), FaceParsingError> {
  let Ok(()) = s.skip::<u32>();
  let Ok(read_count) = s.read::<u32>();
  let face_count = read_count.ok_or(FaceParsingError::MalformedFont)?;
  let Ok(read_offsets) = s.read_array32::<Offset32>(face_count);
  let offsets = read_offsets.ok_or(FaceParsingError::MalformedFont)?;
  let Ok(read_offset) = offsets.get(index);
  let face_offset = read_offset.ok_or(FaceParsingError::FaceIndexOutOfBounds)?;
  let Ok(face_start) = face_offset.position();
  let Ok(here) = s.offset();
  let ahead = face_start
    .checked_sub(here)
    .ok_or(FaceParsingError::MalformedFont)?;
  let Ok(advanced) = s.advance_checked(ahead);
  advanced.ok_or(FaceParsingError::MalformedFont)?;
  let Ok(read_magic) = s.read::<Magic>();
  let face_magic = read_magic.ok_or(FaceParsingError::UnknownMagic)?;

  match face_magic {
    Magic::FontCollection => Err(FaceParsingError::UnknownMagic),
    Magic::TrueType | Magic::OpenType => Ok(()),
  }
}

#[inline]
pub fn fonts_in_collection(data: &[u8]) -> Result<Option<u32>, Never> {
  let Ok(mut s) = Stream::new(data);
  let Ok(magic) = s.read::<Magic>();

  match magic {
    Some(Magic::FontCollection) => {
      let Ok(()) = s.skip::<u32>();

      s.read::<u32>()
    }
    Some(Magic::TrueType | Magic::OpenType) | None => Ok(None),
  }
}
