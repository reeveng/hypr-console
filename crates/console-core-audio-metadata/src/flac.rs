//! FLAC stream information, comments and picture blocks before the audio frames.

use std::io::{Read, Seek};
use std::time::Duration;

use crate::Contents;
use crate::bytes::{Cursor, Source};
use crate::fault::{Malformed, ParseError};
use crate::tag::{Tag, TagType};

const FLAC_SIGNATURE: [u8; 4] = *b"fLaC";

pub(crate) fn read<R: Read + Seek>(reader: &mut R) -> Result<Contents, ParseError> {
  let mut source = Source::new(reader)?;
  let leading = crate::id3v2::leading(&mut source)?;
  let signature = source.array::<4>()?;

  match signature {
    FLAC_SIGNATURE => {}
    _not_flac => return Err(ParseError::Parse(Malformed::Signature)),
  }

  let mut duration = None;
  let mut comments = None;
  let mut pictures = Vec::new();
  let Ok(length) = source.length();

  'blocks: for _block in 0..length.wrapping_div(4).saturating_add(1) {
    let [kind, a, b, c] = source.array::<4>()?;
    let Ok(size) = console_core_bytes::endian::u24_be([a, b, c]);
    let content = source.exact(u64::from(size))?;

    match (kind & 0x7f, duration) {
      (0, None) => {
        let value_1 = stream_info(&content)?;

        duration = Some(value_1)
      }
      (4, Some(_)) => {
        let value_2 = crate::vorbis_comments::read(&content)?;

        comments = Some(value_2)
      }
      (6, Some(_)) => {
        let value_3 = crate::picture::from_flac_block(&content)?;

        pictures.push(value_3)
      }
      (_, None) => return Err(ParseError::Parse(Malformed::Properties)),
      (_, Some(_)) => {}
    }

    match kind & 0x80 {
      0 => {}
      _last => break 'blocks,
    }
  }

  let mut tags = Vec::new();

  match leading {
    Some(id3) => {
      let Ok(tag) = id3.tag();

      tags.push(tag);
    }
    None => {}
  }

  let comments_4 = match (comments, pictures.as_slice()) {
    (Some(mut comments_2), _) => {
      comments_2.pictures.extend(pictures);

      Some(comments_2)
    }
    (None, [_, ..]) => {
      let Ok(mut comments_3) = Tag::new(TagType::VorbisComments);

      comments_3.pictures = pictures;

      Some(comments_3)
    }
    (None, []) => None,
  };

  tags.extend(comments_4);

  let value_4 = duration.ok_or(Malformed::Properties)?;

  Ok(Contents {
    duration: value_4,
    tags,
  })
}

fn stream_info(bytes: &[u8]) -> Result<Duration, Malformed> {
  let Ok(mut reader) = Cursor::new(bytes);

  reader.skip(10)?;

  let packed = reader.array::<8>()?;
  let word = u64::from_be_bytes(packed);
  let rate = word.wrapping_shr(44);
  let samples = word & 0x0f_ffff_ffff;
  let millis = samples
    .saturating_mul(1000)
    .checked_div(rate)
    .ok_or(Malformed::Properties)?;

  Ok(Duration::from_millis(millis))
}
