//! The tags and the cover an iTunes-style `meta` atom keeps under `ilst`, and the two atoms
//! (`udta` and `meta`) a player has to open to get there.
//!
//! One match names every four-byte tag and what kind of value it holds, so the loop over
//! the list is the same for all of them and a tag nobody maps is skipped by the iterator.

use console_core_audio_decoding_interfaces::errors::{Error, Result, decode_error};
use console_core_audio_decoding_interfaces::meta::{
  MetadataBuilder, MetadataRevision, StandardTagKey, StandardVisualKey, Tag, Value, Visual,
};
use console_core_audio_decoding_tags::{id3v1, itunes};
use console_core_byte_streams::ReadBytes;
use console_core_never::Never;
use encoding_rs::{SHIFT_JIS, UTF_8, UTF_16BE};
use std::borrow::Cow;

use crate::atoms::{Atom, AtomHeader, AtomIterator, Kind, OVERFLOW};

const DATA: Kind = Kind(*b"data");

const NAME: Kind = Kind(*b"name");

const MEAN: Kind = Kind(*b"mean");

const NOT_A_DATA_ATOM: Error = Error::DecodeError("isomp4: invalid data atom version");

#[derive(Copy, Clone)]
enum DataType {
  NoType,
  Utf8,
  Utf16,
  ShiftJis,
  Jpeg,
  Png,
  Bmp,
  SignedIntVariable,
  UnsignedIntVariable,
  Float32,
  Float64,
  SignedInt8,
  SignedInt16,
  SignedInt32,
  SignedInt64,
  UnsignedInt8,
  UnsignedInt16,
  UnsignedInt32,
  UnsignedInt64,
  Unsupported,
}

impl From<u32> for DataType {
  fn from(value: u32) -> Self {
    match value {
      0 => DataType::NoType,
      1 | 4 => DataType::Utf8,
      2 | 5 => DataType::Utf16,
      3 => DataType::ShiftJis,
      13 => DataType::Jpeg,
      14 => DataType::Png,
      21 => DataType::SignedIntVariable,
      22 => DataType::UnsignedIntVariable,
      23 => DataType::Float32,
      24 => DataType::Float64,
      27 => DataType::Bmp,
      65 => DataType::SignedInt8,
      66 => DataType::SignedInt16,
      67 => DataType::SignedInt32,
      74 => DataType::SignedInt64,
      75 => DataType::UnsignedInt8,
      76 => DataType::UnsignedInt16,
      77 => DataType::UnsignedInt32,
      78 => DataType::UnsignedInt64,
      _ => DataType::Unsupported,
    }
  }
}

fn metadata_utf8(data: &[u8]) -> std::result::Result<Cow<'_, str>, Never> {
  let (text, _replaced_invalid_bytes) = UTF_8.decode_without_bom_handling(data);

  Ok(text)
}

fn parse_tag_value(
  data_type: DataType,
  data: &[u8],
) -> std::result::Result<Option<Value>, Never> {
  Ok(match (data_type, data) {
    (DataType::NoType, _) => {
      let text = match data.split(|byte| *byte == 0).next() {
        Some(text) => text,
        None => data,
      };

      let Ok(decoded) = metadata_utf8(text);
      Some(Value::from(decoded))
    }
    (DataType::Utf8, _) => {
      let Ok(decoded) = metadata_utf8(data);
      Some(Value::from(decoded))
    }
    (DataType::Utf16, _) => Some(Value::from(UTF_16BE.decode(data).0)),
    (DataType::ShiftJis, _) => Some(Value::from(SHIFT_JIS.decode(data).0)),
    (DataType::UnsignedInt8 | DataType::UnsignedIntVariable, [a]) => Some(Value::from(*a)),
    (DataType::UnsignedInt16 | DataType::UnsignedIntVariable, [a, b]) => {
      Some(Value::from(u16::from_be_bytes([*a, *b])))
    }
    (DataType::UnsignedInt32 | DataType::UnsignedIntVariable, [a, b, c, d]) => {
      Some(Value::from(u32::from_be_bytes([*a, *b, *c, *d])))
    }
    (DataType::UnsignedInt64, [a, b, c, d, e, f, g, h]) => {
      Some(Value::from(u64::from_be_bytes([
        *a, *b, *c, *d, *e, *f, *g, *h,
      ])))
    }
    (DataType::SignedInt8 | DataType::SignedIntVariable, [a]) => {
      Some(Value::from(i8::from_be_bytes([*a])))
    }
    (DataType::SignedInt16 | DataType::SignedIntVariable, [a, b]) => {
      Some(Value::from(i16::from_be_bytes([*a, *b])))
    }
    (DataType::SignedInt32 | DataType::SignedIntVariable, [a, b, c, d]) => {
      Some(Value::from(i32::from_be_bytes([*a, *b, *c, *d])))
    }
    (DataType::SignedInt64, [a, b, c, d, e, f, g, h]) => Some(Value::from(i64::from_be_bytes([
      *a, *b, *c, *d, *e, *f, *g, *h,
    ]))),
    (DataType::Float32, [a, b, c, d]) => Some(Value::Float(f64::from(f32::from_be_bytes([
      *a, *b, *c, *d,
    ])))),
    (DataType::Float64, [a, b, c, d, e, f, g, h]) => Some(Value::Float(f64::from_be_bytes([
      *a, *b, *c, *d, *e, *f, *g, *h,
    ]))),
    _ => None,
  })
}

struct MetaTagDataAtom {
  data: Box<[u8]>,
  data_type: DataType,
}

impl Atom for MetaTagDataAtom {
  fn read<B: ReadBytes>(reader: &mut B, header: AtomHeader) -> Result<Self> {
    let full_box = AtomHeader::read_full_box(reader)?;

    match full_box.version {
      0 => {}
      _ => return Err(NOT_A_DATA_ATOM),
    }

    reader.ignore_bytes(4)?;
    let parsed_data_len = header.payload_after_full_box()?;
    let data_len = parsed_data_len.checked_sub(4).ok_or(OVERFLOW)?;
    let data = reader.read_boxed_slice_exact(data_len)?;
    Ok(MetaTagDataAtom {
      data,
      data_type: DataType::from(full_box.flags),
    })
  }
}

struct MetaTagNamespaceAtom {
  value: String,
}

impl Atom for MetaTagNamespaceAtom {
  fn read<B: ReadBytes>(reader: &mut B, header: AtomHeader) -> Result<Self> {
    AtomHeader::read_full_box(reader)?;
    let parsed_value = header.payload_after_full_box()?;
    let buf = reader.read_boxed_slice_exact(parsed_value)?;
    let Ok(decoded) = metadata_utf8(&buf);

    Ok(MetaTagNamespaceAtom {
      value: decoded.into_owned(),
    })
  }
}

struct MetaTagAtom {
  values: Vec<MetaTagDataAtom>,
  mean: Option<MetaTagNamespaceAtom>,
  name: Option<MetaTagNamespaceAtom>,
}

impl MetaTagAtom {
  fn full_name(&self) -> std::result::Result<String, Never> {
    match (&self.mean, &self.name) {
      (None, None) => Ok(String::new()),
      (mean_atom, name_atom) => {
        let mean = match mean_atom {
          Some(found_mean) => found_mean.value.as_str(),
          None => "",
        };
        let name = match name_atom {
          Some(found_name) => found_name.value.as_str(),
          None => "",
        };

        Ok(format!("{mean}:{name}"))
      }
    }
  }
}

impl Atom for MetaTagAtom {
  fn read<B: ReadBytes>(reader: &mut B, header: AtomHeader) -> Result<Self> {
    let Ok(mut iter) = AtomIterator::new(reader, header);
    let mut mean = None;
    let mut name = None;
    let mut values = Vec::new();
    let Ok(limit) = iter.atoms_at_most();

    for _ in 0..limit {
      let next = iter.next()?;

      match next {
        Some(AtomHeader { kind: DATA, .. }) => {
          let parsed_value = iter.read_atom::<MetaTagDataAtom>()?;
          values.push(parsed_value);
        }
        Some(AtomHeader { kind: NAME, .. }) => {
          let parsed_name = iter.read_atom::<MetaTagNamespaceAtom>()?;
          name = Some(parsed_name);
        }
        Some(AtomHeader { kind: MEAN, .. }) => {
          let parsed_mean = iter.read_atom::<MetaTagNamespaceAtom>()?;
          mean = Some(parsed_mean);
        }
        Some(_) => {}
        None => break,
      }
    }

    Ok(MetaTagAtom { values, mean, name })
  }
}

enum Handling {
  Text(StandardTagKey),
  UnsignedNumber(StandardTagKey),
  SignedNumber(StandardTagKey),
  Flag(StandardTagKey),
  OfTotal {
    number: StandardTagKey,
    total: StandardTagKey,
  },
  Cover,
  Genre,
  MediaType,
  FreeForm,
  Skip,
}

fn handling(kind: Kind) -> std::result::Result<Handling, Never> {
  Ok(match &kind.0 {
    b"aART" => Handling::Text(StandardTagKey::AlbumArtist),
    b"\xa9alb" => Handling::Text(StandardTagKey::Album),
    b"\xa9ART" => Handling::Text(StandardTagKey::Artist),
    b"catg" => Handling::Text(StandardTagKey::PodcastCategory),
    b"\xa9cmt" => Handling::Text(StandardTagKey::Comment),
    b"cpil" => Handling::Text(StandardTagKey::Compilation),
    b"\xa9wrt" => Handling::Text(StandardTagKey::Composer),
    b"cprt" => Handling::Text(StandardTagKey::Copyright),
    b"\xa9gen" => Handling::Text(StandardTagKey::Genre),
    b"\xa9day" => Handling::Text(StandardTagKey::Date),
    b"desc" | b"ldes" => Handling::Text(StandardTagKey::Description),
    b"\xa9enc" => Handling::Text(StandardTagKey::EncodedBy),
    b"\xa9too" => Handling::Text(StandardTagKey::Encoder),
    b"egid" => Handling::Text(StandardTagKey::IdentPodcast),
    b"\xa9grp" => Handling::Text(StandardTagKey::ContentGroup),
    b"keyw" => Handling::Text(StandardTagKey::PodcastKeywords),
    b"\xa9lyr" => Handling::Text(StandardTagKey::Lyrics),
    b"ownr" => Handling::Text(StandardTagKey::Owner),
    b"purd" => Handling::Text(StandardTagKey::PurchaseDate),
    b"rate" => Handling::Text(StandardTagKey::Rating),
    b"soaa" => Handling::Text(StandardTagKey::SortAlbumArtist),
    b"soal" => Handling::Text(StandardTagKey::SortAlbum),
    b"soar" => Handling::Text(StandardTagKey::SortArtist),
    b"soco" => Handling::Text(StandardTagKey::SortComposer),
    b"sonm" => Handling::Text(StandardTagKey::SortTrackTitle),
    b"\xa9nam" => Handling::Text(StandardTagKey::TrackTitle),
    b"tven" => Handling::Text(StandardTagKey::TvEpisodeTitle),
    b"tvnn" => Handling::Text(StandardTagKey::TvNetwork),
    b"tvsh" => Handling::Text(StandardTagKey::TvShowTitle),
    b"purl" => Handling::Text(StandardTagKey::UrlPodcast),
    b"tves" => Handling::UnsignedNumber(StandardTagKey::TvEpisode),
    b"tvsn" => Handling::UnsignedNumber(StandardTagKey::TvSeason),
    b"tmpo" => Handling::SignedNumber(StandardTagKey::Bpm),
    b"pcst" => Handling::Flag(StandardTagKey::Podcast),
    b"disk" => Handling::OfTotal {
      number: StandardTagKey::DiscNumber,
      total: StandardTagKey::DiscTotal,
    },
    b"trkn" => Handling::OfTotal {
      number: StandardTagKey::TrackNumber,
      total: StandardTagKey::TrackTotal,
    },
    b"covr" => Handling::Cover,
    b"gnre" => Handling::Genre,
    b"stik" => Handling::MediaType,
    b"----" => Handling::FreeForm,
    _ => Handling::Skip,
  })
}

fn media_type_name(code: u8) -> std::result::Result<&'static str, Never> {
  Ok(match code {
    0 => "Movie",
    1 => "Normal",
    2 => "Audio Book",
    5 => "Whacked Bookmark",
    6 => "Music Video",
    9 => "Short Film",
    10 => "TV Show",
    11 => "Booklet",
    _ => "Unknown",
  })
}

fn image_media_type(data_type: DataType) -> std::result::Result<&'static str, Never> {
  Ok(match data_type {
    DataType::Bmp => "image/bmp",
    DataType::Jpeg => "image/jpeg",
    DataType::Png => "image/png",
    DataType::NoType
    | DataType::Utf8
    | DataType::Utf16
    | DataType::ShiftJis
    | DataType::SignedIntVariable
    | DataType::UnsignedIntVariable
    | DataType::Float32
    | DataType::Float64
    | DataType::SignedInt8
    | DataType::SignedInt16
    | DataType::SignedInt32
    | DataType::SignedInt64
    | DataType::UnsignedInt8
    | DataType::UnsignedInt16
    | DataType::UnsignedInt32
    | DataType::UnsignedInt64
    | DataType::Unsupported => "",
  })
}

fn add_cover(tag: MetaTagAtom, builder: &mut MetadataBuilder) -> std::result::Result<(), Never> {
  for value in tag.values {
    let Ok(media_type) = image_media_type(value.data_type);

    let Ok(_configured_add_visual) = builder.add_visual(Visual {
      media_type: media_type.to_string(),
      dimensions: None,
      bits_per_pixel: None,
      color_mode: None,
      usage: Some(StandardVisualKey::FrontCover),
      tags: Vec::new(),
      data: value.data,
    });
  }

  Ok(())
}

fn genre_index(tag: &MetaTagAtom) -> Result<Option<u8>> {
  let value = match tag.values.first() {
    Some(found) => found,
    None => return Ok(None),
  };
  let number = match &*value.data {
    [a, b, ..] => u16::from_be_bytes([*a, *b]),
    _ => return decode_error("isomp4: genre atom is shorter than two bytes"),
  };

  Ok(number.checked_sub(1).map(|zero_based| {
    let [_, low] = zero_based.to_be_bytes();
    low
  }))
}

fn add_genre(tag: &MetaTagAtom, builder: &mut MetadataBuilder) -> Result<()> {
  let position = genre_index(tag)?;
  let name = match position {
    Some(number) => {
      let Ok(found) = id3v1::util::genre_name(number);
      found
    }
    None => None,
  };

  match name {
    Some(genre) => {
      let Ok(value_1) = Tag::new(Some(StandardTagKey::Genre), "", Value::from(*genre));

      let Ok(_configured_add_tag) = builder.add_tag(value_1);
    }
    None => {}
  }

  Ok(())
}

fn add_of_total(
  tag: &MetaTagAtom,
  builder: &mut MetadataBuilder,
  number: StandardTagKey,
  total: StandardTagKey,
) -> std::result::Result<(), Never> {
  match tag.values.first().map(|value| &*value.data) {
    Some([_, _, _, m, _, n, _, _]) => {
      let Ok(value_2) = Tag::new(Some(number), "", Value::from(*m));

      let Ok(_configured_add_tag) = builder.add_tag(value_2);

      let Ok(value_3) = Tag::new(Some(total), "", Value::from(*n));

      let Ok(_configured_add_tag) = builder.add_tag(value_3);
    }
    Some(_) | None => {}
  }

  Ok(())
}

fn add_tag<B: ReadBytes>(
  iter: &mut AtomIterator<B>,
  builder: &mut MetadataBuilder,
  handling: Handling,
) -> Result<()> {
  match handling {
    Handling::Skip => return Ok(()),
    Handling::Text(key) => {
      let tag = iter.read_atom::<MetaTagAtom>()?;

      for value in &tag.values {
        let Ok(parsed) = parse_tag_value(value.data_type, &value.data);

        match parsed {
          Some(value_2) => {
            let Ok(value_4) = Tag::new(Some(key), "", value_2);

            let Ok(_configured_add_tag) = builder.add_tag(value_4);
          }
          None => {}
        }
      }
    }
    Handling::UnsignedNumber(key) => {
      let tag = iter.read_atom::<MetaTagAtom>()?;

      let Ok(()) = add_variable(&tag, builder, key, DataType::UnsignedIntVariable);
    }
    Handling::SignedNumber(key) => {
      let tag = iter.read_atom::<MetaTagAtom>()?;

      let Ok(()) = add_variable(&tag, builder, key, DataType::SignedIntVariable);
    }
    Handling::Flag(key) => {
      let tag = iter.read_atom::<MetaTagAtom>()?;

      match tag.values.first().map(|value| &*value.data) {
        Some([1, ..]) => {
          let Ok(value_5) = Tag::new(Some(key), "", Value::Flag);

          let Ok(_configured_add_tag) = builder.add_tag(value_5);
        }
        Some(_) | None => {}
      }
    }
    Handling::OfTotal { number, total } => {
      let tag = iter.read_atom::<MetaTagAtom>()?;
      let Ok(()) = add_of_total(&tag, builder, number, total);
    }
    Handling::Cover => {
      let tag = iter.read_atom::<MetaTagAtom>()?;
      let Ok(()) = add_cover(tag, builder);
    }
    Handling::Genre => {
      let tag = iter.read_atom::<MetaTagAtom>()?;

      add_genre(&tag, builder)?;
    }
    Handling::MediaType => {
      let tag = iter.read_atom::<MetaTagAtom>()?;

      match tag.values.first().and_then(|value| value.data.first()) {
        Some(code) => {
          let Ok(name) = media_type_name(*code);

          let Ok(value_6) = Tag::new(Some(StandardTagKey::MediaFormat), "", Value::from(name));

          let Ok(_configured_add_tag) = builder.add_tag(value_6);
        }
        None => {}
      }
    }
    Handling::FreeForm => {
      let tag = iter.read_atom::<MetaTagAtom>()?;
      let Ok(full_name) = tag.full_name();
      let Ok(std_key) = itunes::std_key_from_tag(&full_name);

      for value in &tag.values {
        let Ok(parsed) = parse_tag_value(value.data_type, &value.data);

        match parsed {
          Some(value_3) => {
            let Ok(value_7) = Tag::new(std_key, &full_name, value_3);

            let Ok(_configured_add_tag) = builder.add_tag(value_7);
          }
          None => {}
        }
      }
    }
  }

  Ok(())
}

fn add_variable(
  tag: &MetaTagAtom,
  builder: &mut MetadataBuilder,
  key: StandardTagKey,
  data_type: DataType,
) -> std::result::Result<(), Never> {
  match tag.values.first() {
    Some(raw_value) => {
      let Ok(parsed) = parse_tag_value(data_type, &raw_value.data);

      match parsed {
        Some(value) => {
          let Ok(value_8) = Tag::new(Some(key), "", value);

          let Ok(_configured_add_tag) = builder.add_tag(value_8);
        }
        None => {}
      }
    }
    None => {}
  }

  Ok(())
}

struct IlstAtom {
  metadata: MetadataRevision,
}

impl Atom for IlstAtom {
  fn read<B: ReadBytes>(reader: &mut B, header: AtomHeader) -> Result<Self> {
    let Ok(mut iter) = AtomIterator::new(reader, header);
    let Ok(mut builder) = MetadataBuilder::new();
    let Ok(limit) = iter.atoms_at_most();

    for _ in 0..limit {
      let next = iter.next()?;

      match next {
        Some(tag) => {
          let Ok(handling) = handling(tag.kind);

          add_tag(&mut iter, &mut builder, handling)?;
        }
        None => break,
      }
    }

    let Ok(value_9) = builder.metadata();

    Ok(IlstAtom { metadata: value_9 })
  }
}

pub struct MetaAtom {
  pub metadata: Option<MetadataRevision>,
}

impl Atom for MetaAtom {
  fn read<B: ReadBytes>(reader: &mut B, header: AtomHeader) -> Result<Self> {
    AtomHeader::read_full_box(reader)?;
    let data_len = header.payload_after_full_box()?;
    let Ok(mut iter) = AtomIterator::new(reader, AtomHeader { data_len, ..header });
    let mut metadata = None;
    let Ok(limit) = iter.atoms_at_most();

    for _ in 0..limit {
      let next = iter.next()?;

      match next {
        Some(AtomHeader {
          kind: Kind::META_LIST,
          ..
        }) => {
          let parsed_metadata = iter.read_atom::<IlstAtom>()?;
          metadata = Some(parsed_metadata.metadata);
        }
        Some(_) => {}
        None => break,
      }
    }

    Ok(MetaAtom { metadata })
  }
}

pub struct UdtaAtom {
  pub metadata: Option<MetadataRevision>,
}

impl Atom for UdtaAtom {
  fn read<B: ReadBytes>(reader: &mut B, header: AtomHeader) -> Result<Self> {
    let Ok(mut iter) = AtomIterator::new(reader, header);
    let mut metadata = None;
    let Ok(limit) = iter.atoms_at_most();

    for _ in 0..limit {
      let next = iter.next()?;

      match next {
        Some(AtomHeader {
          kind: Kind::META, ..
        }) => {
          let parsed_metadata = iter.read_atom::<MetaAtom>()?;
          metadata = parsed_metadata.metadata;
        }
        Some(_) => {}
        None => break,
      }
    }

    Ok(UdtaAtom { metadata })
  }
}

#[cfg(test)]
mod text_tests {
  use super::metadata_utf8;

  #[test]
  fn metadata_keeps_a_byte_order_mark_and_replaces_invalid_sequences() {
    let Ok(recovered) = metadata_utf8(&[b'A', 0xff, b'B', 0xe2, 0x82]);
    assert_eq!(recovered, "A\u{fffd}B\u{fffd}");
    let Ok(marked) = metadata_utf8(&[0xef, 0xbb, 0xbf, b'A']);
    assert_eq!(marked, "\u{feff}A");
  }
}
