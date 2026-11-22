//! The ten bytes an ID3v2 tag opens with, and the extended header some of
//! them are followed by. The extended header is read for its shape and for
//! the faults it can carry; nothing in it -- a checksum, a restriction -- is
//! something a reader of tags acts on, so none of it is kept.

use std::cmp::Ordering;

use console_core_audio_decoding_interfaces::errors::{Error, decode_error, unsupported_error};
use console_core_byte_streams::ReadBytes;

use super::unsync::read_syncsafe;

#[derive(Clone, Copy)]
pub(super) enum Version {
  V2,
  V3,
  V4,
}

pub(super) struct Header {
  pub(super) version: Version,
  pub(super) size: u32,
  pub(super) synchronisation: Synchronisation,
  pub(super) extension: Extension,
}

#[derive(Clone, Copy)]
pub(super) enum Synchronisation {
  Plain,
  Unsynchronised,
}

#[derive(Clone, Copy)]
pub(super) enum Extension {
  Present,
  Absent,
}

const UNSYNCHRONISATION: u8 = 0x80;
const EXTENDED_HEADER: u8 = 0x40;
const V2_COMPRESSION: u8 = 0x40;

pub(super) fn read_header<B: ReadBytes>(reader: &mut B) -> Result<Header, Error> {
  let marker = reader.read_triple_bytes()?;

  match marker {
    [b'I', b'D', b'3'] => {}
    _other => return unsupported_error("id3v2: not an ID3v2 tag"),
  }

  let major = reader.read_u8()?;
  let minor = reader.read_u8()?;
  let flags = reader.read_u8()?;
  let size = read_syncsafe(reader, 28)?;

  match (major, minor) {
    (0xff, _) | (_, 0xff) => return decode_error("id3v2: invalid version number(s)"),
    _ => {}
  }

  let version = match major {
    2 => Version::V2,
    3 => Version::V3,
    4 => Version::V4,
    _ => return unsupported_error("id3v2: unsupported ID3v2 version"),
  };

  match (version, flags & V2_COMPRESSION) {
    (Version::V2, 0) | (Version::V3 | Version::V4, _) => {}
    (Version::V2, _) => return unsupported_error("id3v2: ID3v2.2 compression is not supported"),
  }

  let extension = match (version, flags & EXTENDED_HEADER) {
    (Version::V2, _) | (Version::V3 | Version::V4, 0) => Extension::Absent,
    (Version::V3 | Version::V4, _) => Extension::Present,
  };

  let synchronisation = match flags & UNSYNCHRONISATION {
    0 => Synchronisation::Plain,
    _set => Synchronisation::Unsynchronised,
  };

  Ok(Header {
    version,
    size,
    synchronisation,
    extension,
  })
}

pub(super) fn skip_extended_header<B: ReadBytes>(
  reader: &mut B,
  version: Version,
) -> Result<(), Error> {
  match version {
    Version::V2 => Ok(()),
    Version::V3 => skip_v3(reader),
    Version::V4 => skip_v4(reader),
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
  read_syncsafe(reader, 28)?;

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
      read_syncsafe(reader, 32)?;
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
