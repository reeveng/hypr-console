//! The header in front of each frame, and what each version lets it say.
//!
//! ID3v2.3 has an encryption bit at 0x40 which this reads past as if the frame
//! were plain, which is what it always did; the test for it that stood at 0x04
//! could never be reached, because 0x04 is one of the bits that must be clear.

use console_core_audio_decoding_interfaces::errors::{Error, decode_error, unsupported_error};
use console_core_audio_decoding_interfaces::id3v2::frame_flags::{
  COMPRESSED, DATA_LENGTH_INDICATOR, ENCRYPTED, GROUPED, UNSYNCHRONISED, normalised, reserved,
};
use console_core_audio_decoding_interfaces::id3v2::{
  FrameFields, FrameIdentifier, FrameNameValidity, Version, check_frame_name, read_frame_fields,
  read_frame_identifier, read_synchsafe, undo_unsynchronisation,
};
use console_core_audio_decoding_interfaces::meta::{StandardTagKey, Tag, Visual};
use console_core_byte_streams::{BufReader, ReadBytes};
use console_core_logging::warn;
use console_core_never::Never;

use super::kinds::{self, Kind};
use super::parsers;

pub(super) enum Frame {
  Padding,
  Unsupported(String),
  Invalid(String),
  Tag(Tag),
  Visual(Visual),
  Tags(Vec<Tag>),
}

fn name(id: &[u8]) -> Result<String, Never> {
  Ok(id.iter().map(|byte| char::from(*byte)).collect())
}

fn padding(id: &[u8]) -> Result<Frame, Never> {
  match id.iter().find(|byte| **byte != 0) {
    None => {}
    Some(_nonzero) => warn!("padding bytes not zero"),
  }

  Ok(Frame::Padding)
}

pub(super) fn read_frame(reader: &mut BufReader<'_>, version: Version) -> Result<Frame, Error> {
  let frame_identifier = read_frame_identifier(reader, version)?;
  let Ok(bytes) = frame_identifier.bytes();
  let Ok(identified) = check_frame_name(bytes);

  match identified {
    FrameNameValidity::Invalid => {
      let Ok(frame) = padding(bytes);

      return Ok(frame);
    }
    FrameNameValidity::Valid => {}
  }

  let fields = read_frame_fields(reader, version)?;

  match frame_identifier {
    FrameIdentifier::Legacy(identifier) => read_legacy_body(reader, identifier, fields.size),
    FrameIdentifier::Modern(identifier) => read_modern_body(reader, identifier, version, fields),
  }
}

fn read_legacy_body(reader: &mut BufReader<'_>, id: [u8; 3], size: u32) -> Result<Frame, Error> {
  let size_2 = u64::from(size);
  let Ok(found) = kinds::find_legacy(id);
  let Ok(name) = name(&id);

  match (found, size_2) {
    (None, _) => {
      reader.ignore_bytes(size_2)?;

      Ok(Frame::Unsupported(name))
    }
    (Some(_), 0) => Ok(Frame::Invalid(name)),
    (Some((kind, standard)), _) => {
      let data = reader.read_buf_bytes_ref(size_2)?;
      let Ok(mut body) = BufReader::new(data);

      parsers::parse(kind, &mut body, standard, &name)
    }
  }
}

fn read_modern_body(
  reader: &mut BufReader<'_>,
  id: [u8; 4],
  version: Version,
  FrameFields { size, flags }: FrameFields,
) -> Result<Frame, Error> {
  let size_2 = u64::from(size);

  let Ok(reserved_bits) = reserved(version, flags);

  match reserved_bits {
    0 => {}
    _ => return decode_error("id3v2: unused flag bits are not cleared"),
  }

  let Ok(flags_2) = normalised(version, flags);
  let flags_3 = match version {
    Version::Three => flags_2 & !ENCRYPTED,
    Version::Two | Version::Four => flags_2,
  };
  let Ok(found) = kinds::find(id);
  let Ok(name) = name(&id);

  match found {
    None => {
      reader.ignore_bytes(size_2)?;

      Ok(Frame::Unsupported(name))
    }
    Some(kind) => read_payload(reader, flags_3, size_2, kind, name),
  }
}

fn refuse_unsupported(reader: &mut BufReader<'_>, flags: u16, size: u64) -> Result<(), Error> {
  match (flags & COMPRESSED, flags & ENCRYPTED) {
    (0, 0) => Ok(()),
    (0, _) => {
      reader.ignore_bytes(size)?;

      unsupported_error("id3v2: encrypted frames are not supported")
    }
    _ => {
      reader.ignore_bytes(size)?;

      unsupported_error("id3v2: compressed frames are not supported")
    }
  }
}

fn skip_prefixes(reader: &mut BufReader<'_>, flags: u16, size: u64) -> Result<u64, Error> {
  let mut size_2 = size;

  match (size_2, flags & GROUPED) {
    (0, _) | (_, 0) => {}
    _ => {
      reader.read_byte()?;
      size_2 = size_2.saturating_sub(1);
    }
  }

  match (size_2, flags & DATA_LENGTH_INDICATOR) {
    (0..=3, _) | (_, 0) => {}
    _ => {
      read_synchsafe(reader, 28)?;
      size_2 = size_2.saturating_sub(4);
    }
  }

  Ok(size_2)
}

fn read_payload(
  reader: &mut BufReader<'_>,
  flags: u16,
  size: u64,
  (kind, standard): (Kind, Option<StandardTagKey>),
  name: String,
) -> Result<Frame, Error> {
  refuse_unsupported(reader, flags, size)?;

  let size_2 = skip_prefixes(reader, flags, size)?;

  match size_2 {
    0 => return Ok(Frame::Invalid(name)),
    _payload => {}
  }

  let data = reader.read_buf_bytes_ref(size_2)?;

  let decoded = match flags & UNSYNCHRONISED {
    0 => None,
    _set => {
      let Ok(decoded) = undo_unsynchronisation(data);

      Some(decoded)
    }
  };
  let bytes = match decoded.as_deref() {
    Some(unsynchronised) => unsynchronised,
    None => data,
  };
  let Ok(mut body) = BufReader::new(bytes);

  parsers::parse(kind, &mut body, standard, &name)
}

#[cfg(test)]
mod tests {
  use super::{BufReader, Frame, Version, read_frame};

  #[test]
  fn version_three_keeps_the_decoders_legacy_encryption_flag_behaviour() {
    let Ok(mut reader) = BufReader::new(b"TIT2\0\0\0\x06\0\x40\0Title");

    assert!(matches!(
      read_frame(&mut reader, Version::Three),
      Ok(Frame::Tags(tags)) if tags.len() == 1
    ));
  }
}
