//! The length-prefixed UTF-8 fields shared by FLAC, Vorbis and Opus.

use crate::bytes::Cursor;
use crate::fault::Malformed;
use crate::keys;
use crate::picture;
use crate::tag::{Tag, TagType};

#[derive(Clone, Copy)]
enum Attachment {
  PictureBlock,
  CoverArt,
}

const ATTACHMENTS: [(&str, Attachment); 2] = [
  ("METADATA_BLOCK_PICTURE", Attachment::PictureBlock),
  ("COVERART", Attachment::CoverArt),
];

fn attachment(tag: &mut Tag, (key, value): (&str, &str)) -> Result<(), Malformed> {
  let kind = ATTACHMENTS
    .iter()
    .find(|(spelling, _)| key.eq_ignore_ascii_case(spelling))
    .map(|(_, attachment)| *attachment);

  let picture = match kind {
    Some(Attachment::PictureBlock) => {
      let decoded = data_encoding::BASE64
        .decode(value.as_bytes())
        .map_err(|_invalid| Malformed::Picture)?;

      picture::from_flac_block(&decoded)?
    }
    Some(Attachment::CoverArt) => {
      let data = data_encoding::BASE64
        .decode(value.as_bytes())
        .map_err(|_invalid| Malformed::Picture)?;
      let Ok(mime_type) = picture::MimeType::sniffed(&data);

      picture::Picture {
        kind: picture::PictureType::Other,
        mime_type: Some(mime_type),
        data,
      }
    }
    None => return Ok(()),
  };

  tag.pictures.push(picture);

  Ok(())
}

pub(crate) fn read(content: &[u8]) -> Result<Tag, Malformed> {
  let Ok(mut reader) = Cursor::new(content);
  let vendor_length = reader.u32_le()?;

  reader.skip(u64::from(vendor_length))?;

  let count = reader.u32_le()?;
  let Ok(mut tag) = Tag::new(TagType::VorbisComments);

  for _comment in 0..count {
    let size = reader.u32_le()?;
    let bytes = reader.take(u64::from(size))?;
    let text = crate::text::utf8(bytes)?;

    match text.split_once('=') {
      Some((key, value)) => {
        let Ok(known) = keys::named(keys::VORBIS_COMMENTS, key);

        match known {
          Some(key_2) => {
            let Ok(()) = tag.push_text(key_2, value.to_string());
          }
          None => attachment(&mut tag, (key, value))?,
        }
      }
      None => {}
    }
  }

  Ok(tag)
}
