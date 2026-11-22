//! The two FLAC metadata blocks that are tags. A comment block is a Vorbis
//! comment without its framing bit, and a picture block is the one layout
//! Vorbis comments carry a picture in too, which is why `vorbis` asks this
//! module for it rather than reading pictures a second way.

use std::num::NonZeroU32;

use crate::{id3v2, utf8_with_replacement, vorbis};
use console_core_audio_decoding_interfaces::errors::{Error, decode_error};
use console_core_audio_decoding_interfaces::meta::{
  ColorMode, MetadataBuilder, Size, StandardTagKey, Tag, Value, Visual,
};
use console_core_byte_streams::ReadBytes;
use console_core_never::Never;

fn printable_ascii_to_string(bytes: &[u8]) -> Result<Option<String>, Never> {
  let mut result = String::with_capacity(bytes.len());

  for byte in bytes {
    match byte {
      0x00 => break,
      0x20..=0x7e => result.push(char::from(*byte)),
      _ => return Ok(None),
    }
  }

  Ok(Some(result))
}

fn read_sized<B: ReadBytes>(reader: &mut B) -> Result<Box<[u8]>, Error> {
  let length = reader.read_be_u32()?;

  reader
    .read_boxed_slice_exact(u64::from(length))
    .map_err(Error::from)
}

pub fn read_comment_block<B: ReadBytes>(
  reader: &mut B,
  metadata: &mut MetadataBuilder,
) -> Result<(), Error> {
  vorbis::read_comment_no_framing(reader, metadata)
}

pub fn read_picture_block<B: ReadBytes>(
  reader: &mut B,
  metadata: &mut MetadataBuilder,
) -> Result<(), Error> {
  let type_enc = reader.read_be_u32()?;
  let media_type_buf = read_sized(reader)?;
  let Ok(printable) = printable_ascii_to_string(&media_type_buf);

  let media_type = match printable {
    Some(media_type) => media_type,
    None => return decode_error("meta (flac): picture mime-type contains invalid characters"),
  };

  let desc_buf = read_sized(reader)?;
  let Ok(desc) = utf8_with_replacement(&desc_buf);
  let Ok(description) = Tag::new(
    Some(StandardTagKey::Description),
    "DESCRIPTION",
    Value::from(desc),
  );
  let tags = vec![description];
  let width = reader.read_be_u32()?;
  let height = reader.read_be_u32()?;

  let dimensions = match (width, height) {
    (0, _) | (_, 0) => None,
    _ => Some(Size { width, height }),
  };

  let bits = reader.read_be_u32()?;
  let indexed_colours = reader.read_be_u32()?;

  let color_mode = match NonZeroU32::new(indexed_colours) {
    None => ColorMode::Discrete,
    Some(colours) => ColorMode::Indexed(colours),
  };

  let data_len = reader.read_be_u32()?;
  let data = reader.read_boxed_slice_exact(u64::from(data_len))?;
  let Ok(usage) = id3v2::util::apic_picture_type_to_visual_key(type_enc);

  let Ok(_configured_add_visual) = metadata.add_visual(Visual {
    media_type,
    dimensions,
    bits_per_pixel: NonZeroU32::new(bits),
    color_mode: Some(color_mode),
    usage,
    tags,
    data,
  });

  Ok(())
}
