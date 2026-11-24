//! The byte layout shared by the ID3v2 tag readers. Callers decide which flags,
//! frame identifiers, and extended headers they accept.
//!
//! A nonmatching tag signature leaves the rest of the input unread. Frame flags
//! are normalised to their ID3v2.4 positions. In unsynchronised data every byte
//! advances the state, so the second zero in `ff 00 00` is kept.

use std::io;

use console_core_byte_streams::ReadBytes;
use console_core_never::Never;

const SIGNATURE: [u8; 3] = *b"ID3";
pub const UNSYNCHRONISED: u8 = 0x80;
pub const COMPRESSION_OR_EXTENDED_HEADER: u8 = 0x40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Version {
  Two,
  Three,
  Four,
}

impl Version {
  pub fn frame_header_bytes(self) -> Result<u64, Never> {
    Ok(match self {
      Self::Two => 6,
      Self::Three | Self::Four => 10,
    })
  }
}

pub struct TagFields {
  pub major_version: u8,
  pub revision: u8,
  pub flags: u8,
  pub size: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagVersionError {
  Unsupported,
  LegacyCompression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsynchronisation {
  Applied,
  Absent,
}

pub fn tag_unsynchronisation(flags: u8) -> Result<Unsynchronisation, Never> {
  Ok(match flags & UNSYNCHRONISED {
    0 => Unsynchronisation::Absent,
    _set => Unsynchronisation::Applied,
  })
}

impl TagFields {
  pub fn version(&self) -> Result<Version, TagVersionError> {
    match (
      self.major_version,
      self.flags & COMPRESSION_OR_EXTENDED_HEADER,
    ) {
      (2, 0) => Ok(Version::Two),
      (2, _) => Err(TagVersionError::LegacyCompression),
      (3, _) => Ok(Version::Three),
      (4, _) => Ok(Version::Four),
      _unknown => Err(TagVersionError::Unsupported),
    }
  }
}

pub fn read_tag_fields(reader: &mut impl ReadBytes) -> io::Result<Option<TagFields>> {
  let signature = reader.read_triple_bytes()?;

  match signature {
    SIGNATURE => {}
    _other => return Ok(None),
  }

  let major_version = reader.read_u8()?;
  let revision = reader.read_u8()?;
  let flags = reader.read_u8()?;
  let size = read_synchsafe(reader, 28)?;

  Ok(Some(TagFields {
    major_version,
    revision,
    flags,
    size,
  }))
}

pub enum FrameIdentifier {
  Legacy([u8; 3]),
  Modern([u8; 4]),
}

impl FrameIdentifier {
  pub fn bytes(&self) -> Result<&[u8], Never> {
    Ok(match self {
      Self::Legacy(bytes) => bytes.as_slice(),
      Self::Modern(bytes) => bytes.as_slice(),
    })
  }
}

pub struct FrameFields {
  pub size: u32,
  pub flags: u16,
}

pub fn read_frame_identifier(
  reader: &mut impl ReadBytes,
  version: Version,
) -> io::Result<FrameIdentifier> {
  match version {
    Version::Two => reader.read_triple_bytes().map(FrameIdentifier::Legacy),
    Version::Three | Version::Four => reader.read_quad_bytes().map(FrameIdentifier::Modern),
  }
}

pub fn read_frame_fields(
  reader: &mut impl ReadBytes,
  version: Version,
) -> io::Result<FrameFields> {
  let size = match version {
    Version::Two => reader.read_be_u24()?,
    Version::Three => reader.read_be_u32()?,
    Version::Four => read_synchsafe(reader, 28)?,
  };
  let flags = match version {
    Version::Two => 0,
    Version::Three | Version::Four => reader.read_be_u16()?,
  };

  Ok(FrameFields { size, flags })
}

pub enum FrameNameValidity {
  Valid,
  Invalid,
}

pub fn check_frame_name(bytes: &[u8]) -> Result<FrameNameValidity, Never> {
  let characters = bytes
    .iter()
    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit());

  Ok(match (bytes.len(), characters) {
    (3 | 4, true) => FrameNameValidity::Valid,
    _invalid => FrameNameValidity::Invalid,
  })
}

pub mod frame_flags {
  use console_core_never::Never;

  use super::Version;

  pub const GROUPED: u16 = 0x0040;
  pub const COMPRESSED: u16 = 0x0008;
  pub const ENCRYPTED: u16 = 0x0004;
  pub const UNSYNCHRONISED: u16 = 0x0002;
  pub const DATA_LENGTH_INDICATOR: u16 = 0x0001;

  const VERSION_FOUR_RESERVED: u16 = 0x8fb0;

  pub fn reserved(version: Version, raw: u16) -> Result<u16, Never> {
    Ok(match version {
      Version::Two => raw,
      Version::Three => raw & 0x1f1f,
      Version::Four => raw & VERSION_FOUR_RESERVED,
    })
  }

  pub fn normalised(version: Version, raw: u16) -> Result<u16, Never> {
    Ok(match version {
      Version::Two => 0,
      Version::Three => [
        (0x8000, 0x4000),
        (0x4000, 0x2000),
        (0x2000, 0x1000),
        (0x0020, GROUPED),
        (0x0080, COMPRESSED),
        (0x0040, ENCRYPTED),
      ]
      .into_iter()
      .fold(0, |flags, (before, after)| match raw & before {
        0 => flags,
        _set => flags | after,
      }),
      Version::Four => raw & !VERSION_FOUR_RESERVED,
    })
  }
}

pub fn read_synchsafe(reader: &mut impl ReadBytes, bit_width: u32) -> io::Result<u32> {
  let mut value = 0u32;
  let mut remaining = bit_width;

  for _ in 0..bit_width.div_ceil(7) {
    let taken = remaining.min(7);
    let byte = reader.read_u8()?;
    let mask = 1u32.wrapping_shl(taken).wrapping_sub(1);

    remaining = remaining.saturating_sub(taken);
    value |= (u32::from(byte) & mask).wrapping_shl(remaining);
  }

  Ok(value)
}

pub fn synchsafe_word(bytes: [u8; 4]) -> Result<u32, Never> {
  Ok(bytes.into_iter().fold(0u32, |value, byte| {
    value.wrapping_shl(7) | u32::from(byte & 0x7f)
  }))
}

#[derive(Default)]
pub struct UnsynchronisationDecoder {
  previous: u8,
}

impl UnsynchronisationDecoder {
  pub fn push(&mut self, byte: u8) -> Result<Option<u8>, Never> {
    let previous = std::mem::replace(&mut self.previous, byte);

    Ok(match (previous, byte) {
      (0xff, 0) => None,
      _kept => Some(byte),
    })
  }
}

pub fn undo_unsynchronisation(raw: &[u8]) -> Result<Vec<u8>, Never> {
  let mut decoded = Vec::with_capacity(raw.len());
  let mut decoder = UnsynchronisationDecoder::default();

  for &byte in raw {
    let Ok(kept) = decoder.push(byte);
    decoded.extend(kept);
  }

  Ok(decoded)
}

#[cfg(test)]
mod tests {
  use std::io::Error;

  use console_core_byte_streams::{BufReader, ReadBytes};

  use super::{
    Version, frame_flags, read_frame_fields, read_synchsafe, read_tag_fields, synchsafe_word,
    undo_unsynchronisation,
  };

  #[test]
  fn a_missing_tag_signature_leaves_the_following_bytes_unread() -> Result<(), Error> {
    let Ok(mut reader) = BufReader::new(b"bad\x04\x00\x00\x00\x00\x00\x00");
    let fields = read_tag_fields(&mut reader)?;

    assert!(fields.is_none());
    assert_eq!(reader.pos(), 3);
    Ok(())
  }

  #[test]
  fn frame_sizes_and_flags_follow_their_version() -> Result<(), Error> {
    let Ok(mut third) = BufReader::new(b"\0\0\x01\x02\0\xe0");
    let Ok(mut fourth) = BufReader::new(b"\0\0\x01\x02\0\x4f");
    let third_2 = read_frame_fields(&mut third, Version::Three)?;
    let fourth_2 = read_frame_fields(&mut fourth, Version::Four)?;

    assert_eq!((third_2.size, fourth_2.size), (258, 130));
    assert_eq!(
      frame_flags::normalised(Version::Three, third_2.flags),
      Ok(0x004c)
    );
    assert_eq!(
      frame_flags::normalised(Version::Four, fourth_2.flags),
      Ok(0x004f)
    );
    assert_eq!(
      frame_flags::reserved(Version::Three, third_2.flags | 0x0002),
      Ok(2)
    );
    Ok(())
  }

  #[test]
  fn a_synchsafe_integer_keeps_seven_bits_of_each_byte() -> Result<(), Error> {
    let cases: &[(&[u8], u32, u32)] = &[
      (&[3, 4, 80, 1, 15], 32, 101_875_743),
      (&[16, 16, 16, 16, 16], 32, 541_098_240),
      (&[3, 4, 80, 1], 28, 6_367_233),
      (&[3, 4, 80, 1], 0, 0),
    ];

    for (bytes, width, expected) in cases {
      let Ok(mut reader) = BufReader::new(bytes);
      let read = read_synchsafe(&mut reader, *width)?;

      assert_eq!(read, *expected);
    }

    assert_eq!(synchsafe_word([3, 4, 80, 1]), Ok(6_367_233));
    Ok(())
  }

  #[test]
  fn a_zero_after_an_ff_is_dropped_and_nothing_else_is() {
    let Ok(decoded) =
      undo_unsynchronisation(&[0xff, 0x00, 0x00, 0xff, 0xff, 0x00, 0x01, 0xff, 0x00]);

    assert_eq!(decoded, [0xff, 0x00, 0xff, 0xff, 0x01, 0xff]);
  }
}
