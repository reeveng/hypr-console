//! The binary form the predefined cmaps are carried in, in the way pdf.js
//! carries them and not in its format: a file is a header, then segments, and
//! a segment is a type, a length and a payload. A range or a character
//! segment keeps its codes as the distance from the end of the last one, in
//! columns, Huffman coded with tables shared by the whole bundle; the bundle
//! is one zlib stream of every cmap this tree can name. See
//! <https://github.com/LaurenzV/cmap-resources/tree/hayro> for the upstream bundle.
//!
//! A cmap that names another with `usecmap` is read as its own file and the
//! name is handed back, so no file reads another from inside itself.

mod embedded;
mod huffman;
mod reader;

pub use embedded::load_embedded;

use crate::layer::{Layer, Span};
use crate::{CharacterCollection, CidFamily, Layered, Metadata, ParseError, WritingMode};
use embedded::{Bundle, bundle};
use huffman::Huffman;
use reader::{Bits, Bytes, Remaining};

pub(crate) const MAGIC: &[u8] = b"bcmap";
const VERSION: u8 = 0x01;
const FILE_HEADER: u32 = 10;
const SEGMENT_HEADER: u32 = 5;

const SEGMENT_RANGE_1B: u8 = 0x01;
const SEGMENT_SINGLE_1B: u8 = 0x02;
const SEGMENT_RANGE_2B: u8 = 0x03;
const SEGMENT_SINGLE_2B: u8 = 0x04;
const SEGMENT_RANGE_3B: u8 = 0x05;
const SEGMENT_SINGLE_3B: u8 = 0x06;
const SEGMENT_RANGE_4B: u8 = 0x07;
const SEGMENT_SINGLE_4B: u8 = 0x08;
const SEGMENT_USECMAP: u8 = 0x09;
const SEGMENT_NOTDEF: u8 = 0x0A;
const SEGMENT_WMODE: u8 = 0x0B;
const SEGMENT_CODESPACE: u8 = 0x0C;
const SEGMENT_NAME: u8 = 0x0D;
const SEGMENT_CID_SYSTEM_INFO: u8 = 0x0E;
const SEGMENT_BF_RANGE_VARIABLE: u8 = 0x0F;
const SEGMENT_BF_SINGLE_VARIABLE: u8 = 0x10;
const SEGMENT_BF_SINGLE_1U: u8 = 0x11;
const SEGMENT_BF_SINGLE_2U: u8 = 0x12;
const SEGMENT_BF_SINGLE_3U: u8 = 0x13;
const SEGMENT_BF_SINGLE_4U: u8 = 0x14;
const SEGMENT_BF_RANGE_1U: u8 = 0x15;
const SEGMENT_BF_RANGE_2U: u8 = 0x16;

#[derive(Debug, Clone, Copy)]
enum Shape {
  Singles,
  Ranges,
}

#[derive(Debug, Clone, Copy)]
enum Units {
  Prefixed,
  Fixed(u32),
}

#[derive(Debug, Clone, Copy)]
struct Column {
  delta: u32,
  count: Option<u32>,
}

#[derive(Debug, Default)]
struct Cursor {
  next_start: u32,
}

#[derive(Debug, Clone, Copy)]
struct Previous {
  cid: u32,
  length: u32,
}

#[derive(Debug, Default)]
struct Reading {
  layer: Layer,
  character_collection: Option<CharacterCollection>,
  writing_mode: Option<WritingMode>,
  base: Option<Vec<u8>>,
}

trait Grown {
  fn grown(self, by: u32) -> Result<u32, ParseError>;
}

impl Grown for u32 {
  fn grown(self, by: u32) -> Result<u32, ParseError> {
    self.checked_add(by).ok_or(ParseError::BadSegment)
  }
}

impl Cursor {
  fn advance(&mut self, column: &Column) -> Result<(u32, u32), ParseError> {
    let start = self.next_start.grown(column.delta)?;
    let end = match column.count {
      Some(count) => {
        let past = start.grown(count)?;

        past.grown(1)?
      }
      None => start,
    };

    self.next_start = end.saturating_add(1);

    Ok((start, end))
  }
}

fn frame(data: &[u8]) -> Result<&[u8], ParseError> {
  let Ok(mut head) = Bytes::new(data);
  let magic = head.take(5)?;
  let version = head.u8()?;

  match (magic, version) {
    (MAGIC, VERSION) => {}
    (_other_magic, _other_version) => return Err(ParseError::BadSegment),
  }

  let length = head.u32()?;
  let body = length
    .checked_sub(FILE_HEADER)
    .ok_or(ParseError::Truncated)?;

  head.take(body)
}

fn decoded(
  reader: &mut Bytes<'_>,
  entries: u16,
  table: &Huffman,
) -> Result<Vec<u32>, ParseError> {
  let length = reader.u32()?;
  let data = reader.take(length)?;
  let Ok(mut bits) = Bits::new(data);
  let mut values = Vec::new();

  for _entry in 0..entries {
    let value = table.decode(&mut bits)?;

    values.push(value);
  }

  Ok(values)
}

fn columns(
  reader: &mut Bytes<'_>,
  entries: u16,
  shape: Shape,
  tables: &Bundle,
) -> Result<Vec<Column>, ParseError> {
  let deltas = decoded(reader, entries, &tables.delta)?;

  match shape {
    Shape::Singles => Ok(
      deltas
        .into_iter()
        .map(|delta| Column { delta, count: None })
        .collect(),
    ),
    Shape::Ranges => {
      let counts = decoded(reader, entries, &tables.count)?;

      Ok(
        deltas
          .into_iter()
          .zip(counts)
          .map(|(delta, count)| Column {
            delta,
            count: Some(count),
          })
          .collect(),
      )
    }
  }
}

fn consecutive(previous: Option<Previous>) -> Result<u32, ParseError> {
  match previous {
    Some(Previous { cid, length }) => {
      let past = cid.grown(length)?;

      past.grown(1)
    }
    None => Ok(0),
  }
}

fn codespace(payload: &[u8]) -> Result<(), ParseError> {
  let Ok(mut reader) = Bytes::new(payload);
  let ranges = reader.u8()?;

  for _range in 0..ranges {
    let width = reader.u8()?;

    reader.n_bytes(width)?;
    reader.n_bytes(width)?;
  }

  Ok(())
}

impl Reading {
  fn system_info(&mut self, payload: &[u8]) -> Result<(), ParseError> {
    let Ok(mut reader) = Bytes::new(payload);
    let registry = reader.terminated()?;
    let ordering = reader.terminated()?;
    let supplement = reader.u16()?;
    let Ok(family) = CidFamily::from_registry_ordering(registry, ordering);

    self.character_collection = Some(CharacterCollection {
      family,
      supplement: i32::from(supplement),
    });

    Ok(())
  }

  fn writing_mode(&mut self, payload: &[u8]) -> Result<(), ParseError> {
    self.writing_mode = match payload.first() {
      Some(0) => Some(WritingMode::Horizontal),
      Some(1) => Some(WritingMode::Vertical),
      Some(_other) => None,
      None => return Err(ParseError::Truncated),
    };

    Ok(())
  }

  fn notdef(&mut self, payload: &[u8]) -> Result<(), ParseError> {
    let Ok(mut reader) = Bytes::new(payload);
    let width = reader.u8()?;
    let entries = reader.u16()?;

    for _entry in 0..entries {
      let start = reader.n_bytes(width)?;
      let end = reader.n_bytes(width)?;
      let cid = reader.u16()?;
      let Ok(()) = self.layer.add_notdef(
        width,
        Span {
          start,
          end,
          value: u32::from(cid),
        },
      );
    }

    Ok(())
  }

  fn cids(
    &mut self,
    payload: &[u8],
    width: u8,
    shape: Shape,
    tables: &Bundle,
  ) -> Result<(), ParseError> {
    let Ok(mut reader) = Bytes::new(payload);
    let entries = reader.u16()?;

    match entries {
      0 => return Ok(()),
      _some => {}
    }

    let columns = columns(&mut reader, entries, shape, tables)?;
    let mut cursor = Cursor::default();
    let mut previous = None;

    for column in &columns {
      let raw = reader.u16()?;
      let (start, end) = cursor.advance(column)?;
      let cid = match raw {
        0 => consecutive(previous)?,
        given => u32::from(given),
      };
      let Ok(()) = self.layer.add_cid(
        width,
        Span {
          start,
          end,
          value: cid,
        },
      );

      previous = Some(Previous {
        cid,
        length: end.saturating_sub(start),
      });
    }

    Ok(())
  }

  fn strings(
    &mut self,
    payload: &[u8],
    shape: Shape,
    units: Units,
    tables: &Bundle,
  ) -> Result<(), ParseError> {
    let Ok(mut reader) = Bytes::new(payload);
    let entries = reader.u16()?;
    let columns = columns(&mut reader, entries, shape, tables)?;
    let mut cursor = Cursor::default();

    for column in &columns {
      let length = match units {
        Units::Fixed(length) => length,
        Units::Prefixed => {
          let prefix = reader.u8()?;

          u32::from(prefix)
        }
      };
      let mut destination = Vec::new();

      for _unit in 0..length {
        let unit = reader.u16()?;

        destination.push(unit);
      }

      let (start, end) = cursor.advance(column)?;
      let Ok(()) = self.layer.add_bf(Span {
        start,
        end,
        value: destination,
      });
    }

    Ok(())
  }

  fn segment(&mut self, kind: u8, payload: &[u8], tables: &Bundle) -> Result<(), ParseError> {
    match kind {
      SEGMENT_NAME => Ok(()),
      SEGMENT_CID_SYSTEM_INFO => self.system_info(payload),
      SEGMENT_USECMAP => {
        self.base = Some(payload.to_vec());

        Ok(())
      }
      SEGMENT_WMODE => self.writing_mode(payload),
      SEGMENT_CODESPACE => codespace(payload),
      SEGMENT_NOTDEF => self.notdef(payload),
      SEGMENT_RANGE_1B => self.cids(payload, 1, Shape::Ranges, tables),
      SEGMENT_RANGE_2B => self.cids(payload, 2, Shape::Ranges, tables),
      SEGMENT_RANGE_3B => self.cids(payload, 3, Shape::Ranges, tables),
      SEGMENT_RANGE_4B => self.cids(payload, 4, Shape::Ranges, tables),
      SEGMENT_SINGLE_1B => self.cids(payload, 1, Shape::Singles, tables),
      SEGMENT_SINGLE_2B => self.cids(payload, 2, Shape::Singles, tables),
      SEGMENT_SINGLE_3B => self.cids(payload, 3, Shape::Singles, tables),
      SEGMENT_SINGLE_4B => self.cids(payload, 4, Shape::Singles, tables),
      SEGMENT_BF_RANGE_VARIABLE => self.strings(payload, Shape::Ranges, Units::Prefixed, tables),
      SEGMENT_BF_SINGLE_VARIABLE => {
        self.strings(payload, Shape::Singles, Units::Prefixed, tables)
      }
      SEGMENT_BF_RANGE_1U => self.strings(payload, Shape::Ranges, Units::Fixed(1), tables),
      SEGMENT_BF_RANGE_2U => self.strings(payload, Shape::Ranges, Units::Fixed(2), tables),
      SEGMENT_BF_SINGLE_1U => self.strings(payload, Shape::Singles, Units::Fixed(1), tables),
      SEGMENT_BF_SINGLE_2U => self.strings(payload, Shape::Singles, Units::Fixed(2), tables),
      SEGMENT_BF_SINGLE_3U => self.strings(payload, Shape::Singles, Units::Fixed(3), tables),
      SEGMENT_BF_SINGLE_4U => self.strings(payload, Shape::Singles, Units::Fixed(4), tables),
      _unknown => Err(ParseError::BadSegment),
    }
  }
}

pub(crate) fn read(data: &[u8]) -> Result<Layered, ParseError> {
  let tables = bundle()?;
  let body = frame(data)?;
  let Ok(mut segments) = Bytes::new(body);
  let mut reading = Reading::default();

  for _segment in body {
    let Ok(remaining) = segments.remaining();

    match remaining {
      Remaining::Absent => break,
      Remaining::Present => {}
    }

    let kind = segments.u8()?;
    let length = segments.u32()?;
    let payload_length = length
      .checked_sub(SEGMENT_HEADER)
      .ok_or(ParseError::Truncated)?;
    let payload = segments.take(payload_length)?;

    reading.segment(kind, payload, tables)?;
  }

  let Ok(()) = reading.layer.sort();

  Ok(Layered {
    metadata: Metadata {
      character_collection: reading.character_collection,
      writing_mode: reading.writing_mode,
    },
    layer: reading.layer,
    base: reading.base,
  })
}
