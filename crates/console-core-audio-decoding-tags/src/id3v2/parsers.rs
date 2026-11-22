//! What each kind of frame holds once its header has been read and its body
//! is in hand: text split at its nulls, a description and a value, a count, a
//! picture. A text value is made into one tag apiece, because tags here are
//! allowed to repeat a key and a frame that lists three artists is three tags.

use console_core_audio_decoding_interfaces::errors::{Error, decode_error, unsupported_error};
use console_core_audio_decoding_interfaces::meta::{StandardTagKey, Tag, Value, Visual};
use console_core_byte_streams::{BufReader, FiniteStream, ReadBytes};
use console_core_never::Never;

use super::frames::Frame;
use super::kinds::{Kind, user_text_key};
use super::text::{Encoding, read_encoding, scan_text};
use super::util::apic_picture_type_to_visual_key;

pub(super) fn parse(
  kind: Kind,
  reader: &mut BufReader<'_>,
  standard: Option<StandardTagKey>,
  id: &str,
) -> Result<Frame, Error> {
  match kind {
    Kind::CdIdentifier => read_cd_identifier(reader, standard, id),
    Kind::Comment => read_comment(reader, standard, id),
    Kind::PlayCount => read_play_count(reader, standard, id),
    Kind::Picture => read_picture(reader),
    Kind::Popularimeter => read_popularimeter(reader, standard, id),
    Kind::Private => read_private(reader, standard),
    Kind::Text => read_text(reader, standard, id),
    Kind::Url => read_url(reader, standard, id),
    Kind::UserText => read_user_text(reader),
    Kind::UserUrl => read_user_url(reader, standard),
  }
}

fn language(code: [u8; 3]) -> Result<Option<String>, Never> {
  Ok(match code {
    [b'a'..=b'z', b'a'..=b'z', b'a'..=b'z'] => {
      Some(code.iter().map(|byte| char::from(*byte)).collect())
    }
    _invalid => None,
  })
}

fn read_values(
  reader: &mut BufReader<'_>,
  encoding: Encoding,
  standard: Option<StandardTagKey>,
  key: &str,
) -> Result<Vec<Tag>, Error> {
  let mut tags = Vec::new();

  for _ in 0..reader.bytes_available() {
    match reader.bytes_available() {
      0 => break,
      _ => {
        let text = scan_text(reader, encoding)?;

        let Ok(value_1) = Tag::new(standard, key, Value::from(text));

        tags.push(value_1);
      }
    }
  }

  Ok(tags)
}

fn read_text(
  reader: &mut BufReader<'_>,
  standard: Option<StandardTagKey>,
  id: &str,
) -> Result<Frame, Error> {
  let encoding = read_encoding(reader, "id3v2: invalid text encoding")?;
  let tags = read_values(reader, encoding, standard, id)?;

  Ok(Frame::Tags(tags))
}

fn read_user_text(reader: &mut BufReader<'_>) -> Result<Frame, Error> {
  let encoding = read_encoding(reader, "id3v2: invalid TXXX text encoding")?;
  let description = scan_text(reader, encoding)?;
  let Ok(standard) = user_text_key(&description);
  let tags = read_values(reader, encoding, standard, &format!("TXXX:{description}"))?;

  Ok(Frame::Tags(tags))
}

fn read_url(
  reader: &mut BufReader<'_>,
  standard: Option<StandardTagKey>,
  id: &str,
) -> Result<Frame, Error> {
  let url = scan_text(reader, Encoding::Iso8859_1)?;

  let Ok(value_2) = Tag::new(standard, id, Value::from(url));

  Ok(Frame::Tag(value_2))
}

fn read_user_url(
  reader: &mut BufReader<'_>,
  standard: Option<StandardTagKey>,
) -> Result<Frame, Error> {
  let encoding = read_encoding(reader, "id3v2: invalid WXXX URL description encoding")?;
  let description = scan_text(reader, encoding)?;
  let url = scan_text(reader, Encoding::Iso8859_1)?;

  let Ok(value_3) = Tag::new(standard, &format!("WXXX:{description}"), Value::from(url));

  Ok(Frame::Tag(value_3))
}

fn read_private(
  reader: &mut BufReader<'_>,
  standard: Option<StandardTagKey>,
) -> Result<Frame, Error> {
  let owner = scan_text(reader, Encoding::Iso8859_1)?;
  let Ok(data) = reader.read_buf_bytes_available_ref();

  let Ok(value_4) = Tag::new(standard, &format!("PRIV:{owner}"), Value::from(data));

  Ok(Frame::Tag(value_4))
}

fn read_comment(
  reader: &mut BufReader<'_>,
  standard: Option<StandardTagKey>,
  id: &str,
) -> Result<Frame, Error> {
  let encoding = read_encoding(reader, "id3v2: invalid text encoding")?;
  let code = reader.read_triple_bytes()?;
  let Ok(spoken) = language(code);

  let key = match spoken {
    Some(language) => format!("{id}!{language}"),
    None => id.to_string(),
  };

  scan_text(reader, encoding)?;

  let text = scan_text(reader, encoding)?;

  let Ok(value_5) = Tag::new(standard, &key, Value::from(text));

  Ok(Frame::Tag(value_5))
}

fn read_play_count(
  reader: &mut BufReader<'_>,
  standard: Option<StandardTagKey>,
  id: &str,
) -> Result<Frame, Error> {
  let length = reader.byte_len();

  match length {
    0..=3 => decode_error("id3v2: play counters must be a minimum of 32bits"),
    4..=8 => {
      let bytes = reader.read_buf_bytes_ref(length)?;
      let count = bytes
        .iter()
        .fold(0u64, |count, byte| count.wrapping_shl(8) | u64::from(*byte));

      let Ok(value_6) = Tag::new(standard, id, Value::from(count));

      Ok(Frame::Tag(value_6))
    }
    _ => unsupported_error("id3v2: play counters greater than 64bits are not supported"),
  }
}

fn read_popularimeter(
  reader: &mut BufReader<'_>,
  standard: Option<StandardTagKey>,
  id: &str,
) -> Result<Frame, Error> {
  let email = scan_text(reader, Encoding::Iso8859_1)?;
  let rating = reader.read_u8()?;

  let Ok(value_7) = Tag::new(standard, &format!("{id}:{email}"), Value::from(rating));

  Ok(Frame::Tag(value_7))
}

fn read_cd_identifier(
  reader: &mut BufReader<'_>,
  standard: Option<StandardTagKey>,
  id: &str,
) -> Result<Frame, Error> {
  let Ok(table_of_contents) = reader.read_buf_bytes_available_ref();

  let Ok(value_8) = Tag::new(standard, id, Value::from(table_of_contents));

  Ok(Frame::Tag(value_8))
}

fn read_picture(reader: &mut BufReader<'_>) -> Result<Frame, Error> {
  let encoding = read_encoding(reader, "id3v2: invalid text encoding")?;
  let media_type = scan_text(reader, Encoding::Iso8859_1)?;
  let picture_type = reader.read_u8()?;
  let Ok(usage) = apic_picture_type_to_visual_key(u32::from(picture_type));
  let text = scan_text(reader, encoding)?;
  let Ok(description) = Tag::new(Some(StandardTagKey::Description), "", Value::from(text));
  let tags = vec![description];
  let Ok(available) = reader.read_buf_bytes_available_ref();
  let data = Box::<[u8]>::from(available);

  Ok(Frame::Visual(Visual {
    media_type,
    dimensions: None,
    bits_per_pixel: None,
    color_mode: None,
    usage,
    tags,
    data,
  }))
}
