//! RIFF chunks carry WAVE audio properties, INFO strings and optional ID3v2 tags.

use std::io::{Read, Seek};
use std::time::Duration;

use crate::Contents;
use crate::bytes::{Cursor, Source};
use crate::duration::{Interval, rounded};
use crate::fault::{FileEncodingError, Malformed, ParseError};
use crate::tag::{Tag, TagType};

struct Format {
  encoding: u16,
  channels: u16,
  rate: u32,
  bytes_per_second: u32,
  bits: u16,
}

fn format(bytes: &[u8]) -> Result<Format, Malformed> {
  let Ok(mut reader) = Cursor::new(bytes);
  let mut encoding = reader.u16_le()?;
  let channels = reader.u16_le()?;
  let rate = reader.u32_le()?;
  let bytes_per_second = reader.u32_le()?;
  let _alignment = reader.u16_le()?;
  let bits = reader.u16_le()?;

  match encoding {
    0xfffe => {
      reader.skip(8)?;

      let value_1 = reader.u16_le()?;

      encoding = value_1;

      reader.skip(14)?;
    }
    _encoding => {}
  }

  match (channels, bits.wrapping_rem(8)) {
    (0, _) | (_, 1..) => Err(Malformed::Properties),
    (_, 0) => Ok(Format {
      encoding,
      channels,
      rate,
      bytes_per_second,
      bits,
    }),
  }
}

fn info(bytes: &[u8]) -> Result<Tag, Malformed> {
  let Ok(mut reader) = Cursor::new(bytes);
  let Ok(limit) = reader.left();
  let Ok(mut tag) = Tag::new(TagType::RiffInfo);

  for _field in 0..limit.wrapping_div(8).saturating_add(1) {
    let Ok(left) = reader.left();

    match left {
      0..=7 => break,
      _has_header => {}
    }

    let key = reader.array::<4>()?;
    let size = reader.u32_le()?;
    let value = reader.take(u64::from(size))?;
    let Ok(key_2) = crate::text::latin1(&key);
    let Ok(known) = crate::keys::named(crate::keys::RIFF_INFO, &key_2);

    match known {
      Some(key_3) => {
        let Ok(text) = crate::text::latin1(value);
        let Ok(()) = tag.push_text(key_3, text);
      }
      None => {}
    }

    reader.skip(u64::from(size & 1))?;
  }

  Ok(tag)
}

pub(crate) fn read<R: Read + Seek>(reader: &mut R) -> Result<Contents, ParseError> {
  let mut source = Source::new(reader)?;
  let header = source.array::<12>()?;
  let [r, i, f, another_f, _a, _b, _c, _d, w, a, v, e] = header;

  match ([r, i, f, another_f], [w, a, v, e]) {
    ([b'R', b'I', b'F', b'F'], [b'W', b'A', b'V', b'E']) => {}
    (_, _) => return Err(ParseError::Parse(Malformed::Signature)),
  }

  let Ok(length) = source.length();
  let mut fmt = None;
  let mut data_length = 0;
  let mut samples = 0;
  let mut list = None;
  let mut id3 = None;

  for _chunk in 0..length.wrapping_div(8).saturating_add(1) {
    let left = source.left()?;

    match left {
      0..=7 => break,
      _has_header => {}
    }

    let kind = source.array::<4>()?;
    let value_2 = source.array::<4>()?;

    let size = u64::from(u32::from_le_bytes(value_2));
    let start = source.at()?;

    match &kind {
      b"fmt " => {
        let bytes = source.exact(size)?;

        let value_3 = format(&bytes)?;

        fmt = Some(value_3);
      }
      b"data" => data_length = size,
      b"fact" => {
        let value_4 = source.array::<4>()?;

        samples = u64::from(u32::from_le_bytes(value_4))
      }
      b"LIST" => {
        let bytes = source.exact(size)?;

        match bytes.strip_prefix(b"INFO") {
          Some(fields) => {
            let value_5 = info(fields)?;

            list = Some(value_5)
          }
          None => {}
        }
      }
      b"id3 " | b"ID3 " => {
        let bytes = source.exact(size)?;
        let mut cursor = std::io::Cursor::new(bytes);
        let mut tag_source = Source::new(&mut cursor)?;

        let value_6 = crate::id3v2::leading(&mut tag_source)?;

        id3 = value_6;
      }
      _unread => {}
    }

    source.seek_to(start.saturating_add(size).saturating_add(size & 1))?;
  }

  let fmt_2 = fmt.ok_or(Malformed::Properties)?;

  match (data_length, fmt_2.encoding, samples) {
    (0, _, _) | (_, 0 | 2 | 4.., 0) => return Err(ParseError::Parse(Malformed::Properties)),
    (_, _, _) => {}
  }

  match (fmt_2.bits, samples, fmt_2.encoding) {
    (1.., 0, _) | (1.., _, 1 | 3) => {
      let frame_bytes =
        u64::from(fmt_2.channels).saturating_mul(u64::from(fmt_2.bits.wrapping_div(8)));

      let value_7 = data_length
        .checked_div(frame_bytes)
        .ok_or(Malformed::Properties)?;

      samples = value_7;
    }
    _known => {}
  }

  let duration = match (fmt_2.rate, samples, fmt_2.bytes_per_second) {
    (1.., 1.., _) => rounded(Interval {
      units: samples,
      per_second: u64::from(fmt_2.rate),
    }),
    (_, _, 1..) => rounded(Interval {
      units: data_length,
      per_second: u64::from(fmt_2.bytes_per_second),
    }),
    (_, _, 0) => Ok(Duration::ZERO),
  }?;

  let mut tags = Vec::new();

  tags.extend(list);

  match id3 {
    Some(id3_2) => {
      let Ok(tag) = id3_2.tag();

      tags.push(tag);
    }
    None => {}
  }

  Ok(Contents { duration, tags })
}

fn chunk(kind: [u8; 4], content: &[u8]) -> Result<Vec<u8>, FileEncodingError> {
  let size = u32::try_from(content.len()).map_err(|_too_large| FileEncodingError::TooLarge)?;
  let mut chunk = Vec::new();

  chunk.extend_from_slice(&kind);
  chunk.extend_from_slice(&size.to_le_bytes());
  chunk.extend_from_slice(content);

  match size & 1 {
    0 => {}
    _odd => chunk.push(0),
  }

  Ok(chunk)
}

pub fn tagged_wave(original: &[u8], tag: &Tag) -> Result<Vec<u8>, FileEncodingError> {
  let Ok(mut reader) = Cursor::new(original);
  let header = reader
    .array::<12>()
    .map_err(|_short| FileEncodingError::NotWave)?;
  let [r, i, f, another_f, _a, _b, _c, _d, w, a, v, e] = header;

  match ([r, i, f, another_f], [w, a, v, e]) {
    ([b'R', b'I', b'F', b'F'], [b'W', b'A', b'V', b'E']) => {}
    (_, _) => return Err(FileEncodingError::NotWave),
  }

  let mut result = b"RIFF\0\0\0\0WAVE".to_vec();
  let Ok(length) = reader.left();

  for _chunk in 0..length.wrapping_div(8).saturating_add(1) {
    let Ok(left) = reader.left();

    match left {
      0..=7 => break,
      _has_header => {}
    }

    let kind = reader
      .array::<4>()
      .map_err(|_short| FileEncodingError::SizeMismatch)?;
    let size = reader
      .u32_le()
      .map_err(|_short| FileEncodingError::SizeMismatch)?;
    let content = reader
      .take(u64::from(size))
      .map_err(|_short| FileEncodingError::SizeMismatch)?;

    reader
      .skip(u64::from(size & 1))
      .map_err(|_short| FileEncodingError::SizeMismatch)?;

    match &kind {
      b"id3 " | b"ID3 " => {}
      _kept => {
        let encoded = chunk(kind, content)?;

        result.extend(encoded);
      }
    }
  }

  let encoded = crate::id3v2::written(tag)?;

  match encoded.as_slice() {
    [] => {}
    bytes => {
      let encoded_2 = chunk(*b"ID3 ", bytes)?;

      result.extend(encoded_2);
    }
  }

  let value_8 = u32::try_from(result.len()).map_err(|_large| FileEncodingError::TooLarge)?;

  let size = value_8.checked_sub(8).ok_or(FileEncodingError::TooLarge)?;

  match result.get_mut(4..).and_then(<[u8]>::first_chunk_mut::<4>) {
    Some(field) => *field = size.to_le_bytes(),
    None => return Err(FileEncodingError::NotWave),
  }

  Ok(result)
}
