//! Movie headers provide the sample clock; the iTunes item list provides tags and covers.

use std::cmp::Ordering;
use std::io::{Read, Seek};
use std::time::Duration;

use console_core_iso_media_boxes::{BoxHeader, Children};
use console_core_never::Never;
use console_core_number_conversion::fitted;

use crate::Contents;
use crate::bytes::{Cursor, Source};
use crate::fault::{Malformed, ParseError};
use crate::picture::{MimeType, Picture, PictureType};
use crate::tag::{Tag, TagType};

const TRACK: [u8; 4] = *b"trak";
const SOUND: [u8; 4] = *b"soun";
const DATA: [u8; 4] = *b"data";

pub(crate) fn read<R: Read + Seek>(reader: &mut R) -> Result<Contents, ParseError> {
  let mut source = Source::new(reader)?;
  let Ok(length) = source.length();
  let mut movie = None;

  for _box in 0..length.wrapping_div(8).saturating_add(1) {
    let start = source.at()?;

    let value_1 = source.left()?;

    match value_1 {
      0..=7 => break,
      _has_header => {}
    }

    let mut header = source.exact(8)?;
    let Ok(required) = match BoxHeader::required_len(&header) {
      Ok(width) => fitted::<_, u64>(width),
      Err(_invalid_header) => return Err(ParseError::Parse(Malformed::Atom)),
    };
    let rest = source.exact(required.saturating_sub(8))?;

    header.extend(rest);

    let header_2 = BoxHeader::parse(&header).map_err(|_invalid| Malformed::Atom)?;
    let end = match header_2.size {
      Some(size) => start.checked_add(size).ok_or(Malformed::Atom),
      None => Ok(length),
    };
    let end_2 = end?;

    match end_2.cmp(&length) {
      Ordering::Greater => return Err(ParseError::Parse(Malformed::Atom)),
      Ordering::Less | Ordering::Equal => {}
    }

    match header_2.kind.0 {
      [b'm', b'o', b'o', b'v'] => {
        let value_3 = source.exact(
          end_2
            .saturating_sub(start)
            .saturating_sub(header_2.header_len),
        )?;

        movie = Some(value_3)
      }
      _unread => {}
    }

    source.seek_to(end_2)?;
  }

  let movie_2 = movie.ok_or(Malformed::Atom)?;
  let duration = audio_duration(&movie_2)?;
  let value_4 = child(&movie_2, *b"meta")?;

  let metadata = match value_4 {
    Some(metadata) => Ok(Some(metadata)),
    None => {
      let value_5 = child(&movie_2, *b"udta")?;

      match value_5 {
        Some(user_data) => child(user_data, *b"meta"),
        None => Ok(None),
      }
    }
  };
  let metadata_2 = metadata?;
  let mut tags = Vec::new();

  match metadata_2 {
    Some(metadata_3) => {
      let Ok(mut fields) = Cursor::new(metadata_3);

      fields.skip(4)?;

      let Ok(content) = fields.rest();

      let value_7 = child(content, *b"ilst")?;

      match value_7 {
        Some(items) => {
          let value_8 = item_list(items)?;

          tags.push(value_8)
        }
        None => {}
      }
    }
    None => {}
  }

  Ok(Contents { duration, tags })
}

fn child(bytes: &[u8], kind: [u8; 4]) -> Result<Option<&[u8]>, Malformed> {
  let Ok(views) = Children::new(bytes);

  for view in views {
    let view_2 = view.map_err(|_invalid| Malformed::Atom)?;

    match view_2.kind.0.cmp(&kind) {
      Ordering::Equal => return Ok(Some(view_2.payload)),
      Ordering::Less | Ordering::Greater => {}
    }
  }

  Ok(None)
}

fn audio_duration(movie: &[u8]) -> Result<Duration, Malformed> {
  let Ok(tracks) = Children::new(movie);

  for track in tracks {
    let track_2 = track.map_err(|_invalid| Malformed::Atom)?;

    match track_2.kind.0 {
      TRACK => {}
      _not_track => continue,
    }

    let value_9 = child(track_2.payload, *b"mdia")?;

    let media = match value_9 {
      Some(media) => media,
      None => continue,
    };
    let value_10 = child(media, *b"hdlr")?;

    let handler = match value_10 {
      Some(handler) => handler,
      None => continue,
    };
    let Ok(mut fields) = Cursor::new(handler);

    fields.skip(8)?;

    let value_11 = fields.array::<4>()?;

    match value_11 {
      SOUND => {}
      _not_audio => continue,
    }

    let value_12 = child(media, *b"mdhd")?;

    let header = value_12.ok_or(Malformed::Properties)?;
    let Ok(mut fields_2) = Cursor::new(header);
    let [version, _a, _b, _c] = fields_2.array::<4>()?;
    let (rate, ticks) = match version {
      1 => {
        fields_2.skip(16)?;

        let rate = fields_2.u32_be()?;
        let ticks = fields_2.array::<8>()?;

        (u64::from(rate), u64::from_be_bytes(ticks))
      }
      _version => {
        fields_2.skip(8)?;

        let rate = fields_2.u32_be()?;
        let ticks = fields_2.u32_be()?;

        (u64::from(rate), u64::from(ticks))
      }
    };

    return match rate {
      0 => Ok(Duration::ZERO),
      _rate => crate::duration::rounded(crate::duration::Interval {
        units: ticks,
        per_second: rate,
      }),
    };
  }

  Err(Malformed::NoAudio)
}

fn freeform_name(content: &[u8]) -> Result<String, Malformed> {
  let value_13 = child(content, *b"mean")?;

  let mean = value_13.ok_or(Malformed::Item)?;
  let value_14 = child(content, *b"name")?;

  let name = value_14.ok_or(Malformed::Item)?;
  let Ok(mut domain) = Cursor::new(mean);
  let Ok(mut name_2) = Cursor::new(name);

  domain.skip(4)?;
  name_2.skip(4)?;

  let Ok(domain_2) = domain.rest();
  let Ok(name_3) = name_2.rest();
  let domain_3 = crate::text::utf8(domain_2)?;
  let name_4 = crate::text::utf8(name_3)?;

  Ok(format!(
    "----:{domain}:{name}",
    name = name_4,
    domain = domain_3
  ))
}

fn cover_type(kind: u32, content: &[u8]) -> Result<MimeType, Never> {
  match kind {
    13 => Ok(MimeType::Jpeg),
    14 => Ok(MimeType::Png),
    _other => MimeType::sniffed(content),
  }
}

fn add_genre(tag: &mut Tag, number: u8) -> Result<(), Never> {
  let Ok(name) = crate::id3v1::genre_name(number.saturating_sub(1));

  match (number, name) {
    (1.., Some(genre)) => tag.push_text(crate::ItemKey::Genre, genre.to_string()),
    (0, _) | (_, None) => Ok(()),
  }
}

fn item_list(bytes: &[u8]) -> Result<Tag, Malformed> {
  let Ok(mut tag) = Tag::new(TagType::Mp4Ilst);
  let Ok(items) = Children::new(bytes);

  'items: for item in items {
    let item_2 = item.map_err(|_invalid| Malformed::Atom)?;
    let Ok(name) = crate::text::latin1(&item_2.kind.0);
    let name_2 = match name.as_str() {
      "----" => freeform_name(item_2.payload),
      _ordinary => Ok(name),
    };
    let name_3 = name_2?;
    let Ok(key) = crate::keys::named(crate::keys::ILST, &name_3);
    let Ok(values) = Children::new(item_2.payload);

    'values: for value in values {
      let value_2 = value.map_err(|_invalid| Malformed::Atom)?;

      match value_2.kind.0 {
        DATA => {}
        _not_data => continue 'values,
      }

      let Ok(mut fields) = Cursor::new(value_2.payload);
      let value_16 = fields.u32_be()?;

      let kind = value_16 & 0x00ff_ffff;

      fields.skip(4)?;

      let Ok(content) = fields.rest();

      match (name_3.as_str(), key, kind) {
        ("covr", _, _) => {
          let Ok(mime_type) = cover_type(kind, content);

          tag.pictures.push(Picture {
            kind: PictureType::Other,
            mime_type: Some(mime_type),
            data: content.to_vec(),
          });
        }
        ("gnre", _, _) => {
          let number = match content {
            [_high, number, ..] => *number,
            [] | [_] => continue 'items,
          };
          let Ok(()) = add_genre(&mut tag, number);
        }
        (_, Some(key_2), 1 | 2) => {
          let text = match kind {
            1 => crate::text::utf8(content),
            2 => crate::text::utf16(content, crate::text::ByteOrder::Big),
            _other => continue 'values,
          };
          let text_2 = text?;
          let Ok(()) = tag.push_text(key_2, text_2);
        }
        (_, Some(_), _) | (_, None, _) => {}
      }
    }
  }

  Ok(tag)
}
