//! The ten bytes an ID3v2 tag opens with, and the extended header some of
//! them are followed by. The extended header is read for its shape and for
//! the faults it can carry; nothing in it -- a checksum, a restriction -- is
//! something a reader of tags acts on, so none of it is kept.

use std::cmp::Ordering;

use console_core_audio_decoding_interfaces::errors::{Error, decode_error, unsupported_error};
use console_core_byte_streams::ReadBytes;

use console_core_audio_decoding_interfaces::id3v2::{
  COMPRESSION_OR_EXTENDED_HEADER, TagVersionError, Unsynchronisation, Version, read_synchsafe,
  read_tag_fields, tag_unsynchronisation,
};

pub(super) struct Header {
  pub(super) version: Version,
  pub(super) size: u32,
  pub(super) unsynchronisation: Unsynchronisation,
  pub(super) extension: Extension,
}

#[derive(Clone, Copy)]
pub(super) enum Extension {
  Present,
  Absent,
}

pub(super) fn read_header<B: ReadBytes>(reader: &mut B) -> Result<Header, Error> {
  let fields = match {
    let checked_value = read_tag_fields(reader)?;
    checked_value
  } {
    Some(fields) => fields,
    None => return unsupported_error("id3v2: not an ID3v2 tag"),
  };

  match (fields.major_version, fields.revision) {
    (0xff, _) | (_, 0xff) => return decode_error("id3v2: invalid version number(s)"),
    _ => {}
  }

  let version = match fields.version() {
    Ok(version) => version,
    Err(TagVersionError::Unsupported) => {
      return unsupported_error("id3v2: unsupported ID3v2 version");
    }
    Err(TagVersionError::LegacyCompression) => {
      return unsupported_error("id3v2: ID3v2.2 compression is not supported");
    }
  };
  let flags = fields.flags;

  let extension = match (version, flags & COMPRESSION_OR_EXTENDED_HEADER) {
    (Version::Two, _) | (Version::Three | Version::Four, 0) => Extension::Absent,
    (Version::Three | Version::Four, _) => Extension::Present,
  };

  let Ok(unsynchronisation) = tag_unsynchronisation(flags);

  Ok(Header {
    version,
    size: fields.size,
    unsynchronisation,
    extension,
  })
}

pub(super) fn skip_extended_header<B: ReadBytes>(
  reader: &mut B,
  version: Version,
) -> Result<(), Error> {
  match version {
    Version::Two => Ok(()),
    Version::Three => skip_v3(reader),
    Version::Four => skip_v4(reader),
  }
}

fn skip_v3<B: ReadBytes>(reader: &mut B) -> Result<(), Error> {
  let size = reader.read_be_u32()?;
  let flags = reader.read_be_u16()?;

  reader.read_be_u32()?;

  match (size, flags & 0x8000) {
    (6, _) | (10, 0) => Ok(()),
    (10, _) => {
      reader.read_be_u32()?;

      Ok(())
    }
    _ => decode_error("id3v2: invalid extended header size"),
  }
}

fn skip_flag_data<B: ReadBytes>(
  reader: &mut B,
  length: u8,
  invalid: &'static str,
) -> Result<(), Error> {
  let found = reader.read_u8()?;

  match found.cmp(&length) {
    Ordering::Equal => Ok(()),
    Ordering::Less | Ordering::Greater => decode_error(invalid),
  }
}

fn skip_v4<B: ReadBytes>(reader: &mut B) -> Result<(), Error> {
  read_synchsafe(reader, 28)?;

  let flag_bytes = reader.read_u8()?;

  match flag_bytes {
    1 => {}
    _ => return decode_error("id3v2: extended flags should have a length of 1"),
  }

  let flags = reader.read_u8()?;

  match flags & 0x40 {
    0 => {}
    _ => skip_flag_data(reader, 1, "id3v2: is update extended flag has invalid size")?,
  }

  match flags & 0x20 {
    0 => {}
    _ => {
      skip_flag_data(reader, 5, "id3v2: CRC32 extended flag has invalid size")?;
      read_synchsafe(reader, 32)?;
    }
  }

  match flags & 0x10 {
    0 => Ok(()),
    _ => {
      skip_flag_data(
        reader,
        1,
        "id3v2: restrictions extended flag has invalid size",
      )?;
      reader.read_u8()?;

      Ok(())
    }
  }
}
