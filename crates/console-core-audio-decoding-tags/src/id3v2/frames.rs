//! The header in front of each frame, and what each version lets it say.
//!
//! ID3v2.3 and ID3v2.4 differ in how a size is written and in which bit of the
//! flags means what, and in nothing else a reader acts on, so the two are one
//! walk over a `Layout` that names the bits. A bit a version does not have is a
//! mask of nothing, and a flag test against it is never set.
//!
//! ID3v2.3 has an encryption bit at 0x40 which this reads past as if the frame
//! were plain, which is what it always did; the test for it that stood at 0x04
//! could never be reached, because 0x04 is one of the bits that must be clear.

use console_core_audio_decoding_interfaces::errors::{Error, decode_error, unsupported_error};
use console_core_audio_decoding_interfaces::meta::{StandardTagKey, Tag, Visual};
use console_core_byte_streams::{BufReader, ReadBytes};
use console_core_logging::warn;
use console_core_never::Never;

use super::header::Version;
use super::kinds::{self, Kind};
use super::parsers;
use super::unsync::{decode_unsynchronisation, read_syncsafe};

pub(super) enum Frame {
  Padding,
  Unsupported(String),
  Invalid(String),
  Tag(Tag),
  Visual(Visual),
  Tags(Vec<Tag>),
}

enum Identifier {
  Valid,
  Padding,
}

impl From<bool> for Identifier {
  fn from(valid: bool) -> Self {
    match bool::then_some(valid, ()) {
      Some(()) => Self::Valid,
      None => Self::Padding,
    }
  }
}

enum SizeEncoding {
  BigEndian,
  Synchsafe,
}

struct Layout {
  size: SizeEncoding,
  unused: u16,
  compression: u16,
  encryption: u16,
  grouping: u16,
  data_length: u16,
  unsynchronisation: u16,
}

const V3: Layout = Layout {
  size: SizeEncoding::BigEndian,
  unused: 0x1f1f,
  compression: 0x80,
  encryption: 0,
  grouping: 0x20,
  data_length: 0,
  unsynchronisation: 0,
};

const V4: Layout = Layout {
  size: SizeEncoding::Synchsafe,
  unused: 0x8fb0,
  compression: 0x8,
  encryption: 0x4,
  grouping: 0x40,
  data_length: 0x1,
  unsynchronisation: 0x2,
};

struct Wanted {
  name: String,
  kind: Kind,
  standard: Option<StandardTagKey>,
}

fn identifier(id: &[u8]) -> Result<Identifier, Never> {
  Ok(Identifier::from(
    id.iter()
      .all(|byte| matches!(byte, b'0'..=b'9' | b'A'..=b'Z')),
  ))
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

fn read_data<'d>(reader: &mut BufReader<'d>, size: u64) -> std::io::Result<&'d [u8]> {
  reader.read_buf_bytes_ref(size)
}

pub(super) fn read_frame(reader: &mut BufReader<'_>, version: Version) -> Result<Frame, Error> {
  match version {
    Version::V2 => read_legacy(reader),
    Version::V3 => read_modern(reader, &V3),
    Version::V4 => read_modern(reader, &V4),
  }
}

fn read_legacy(reader: &mut BufReader<'_>) -> Result<Frame, Error> {
  let id = reader.read_triple_bytes()?;
  let Ok(identified) = identifier(&id);

  match identified {
    Identifier::Padding => {
      let Ok(frame) = padding(&id);

      Ok(frame)
    }
    Identifier::Valid => read_legacy_body(reader, id),
  }
}

fn read_legacy_body(reader: &mut BufReader<'_>, id: [u8; 3]) -> Result<Frame, Error> {
  let wide = reader.read_be_u24()?;
  let size = u64::from(wide);
  let Ok(found) = kinds::find_legacy(id);
  let Ok(name) = name(&id);

  match (found, size) {
    (None, _) => {
      reader.ignore_bytes(size)?;

      Ok(Frame::Unsupported(name))
    }
    (Some(_), 0) => Ok(Frame::Invalid(name)),
    (Some((kind, standard)), _) => {
      let data = read_data(reader, size)?;
      let Ok(mut body) = BufReader::new(data);

      parsers::parse(kind, &mut body, standard, &name)
    }
  }
}

fn read_modern(reader: &mut BufReader<'_>, layout: &Layout) -> Result<Frame, Error> {
  let id = reader.read_quad_bytes()?;
  let Ok(identified) = identifier(&id);

  match identified {
    Identifier::Padding => {
      let Ok(frame) = padding(&id);

      Ok(frame)
    }
    Identifier::Valid => read_modern_body(reader, id, layout),
  }
}

fn read_size(reader: &mut BufReader<'_>, encoding: &SizeEncoding) -> Result<u32, Error> {
  match encoding {
    SizeEncoding::BigEndian => reader.read_be_u32().map_err(Error::from),
    SizeEncoding::Synchsafe => read_syncsafe(reader, 28),
  }
}

fn read_modern_body(
  reader: &mut BufReader<'_>,
  id: [u8; 4],
  layout: &Layout,
) -> Result<Frame, Error> {
  let wide = read_size(reader, &layout.size)?;
  let size = u64::from(wide);
  let flags = reader.read_be_u16()?;

  match flags & layout.unused {
    0 => {}
    _ => return decode_error("id3v2: unused flag bits are not cleared"),
  }

  let Ok(found) = kinds::find(id);
  let Ok(name) = name(&id);

  match found {
    None => {
      reader.ignore_bytes(size)?;

      Ok(Frame::Unsupported(name))
    }
    Some((kind, standard)) => {
      let wanted = Wanted {
        name,
        kind,
        standard,
      };

      read_payload(reader, layout, flags, size, &wanted)
    }
  }
}

fn refuse_unsupported(
  reader: &mut BufReader<'_>,
  layout: &Layout,
  flags: u16,
  size: u64,
) -> Result<(), Error> {
  match (flags & layout.compression, flags & layout.encryption) {
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

fn skip_prefixes(
  reader: &mut BufReader<'_>,
  layout: &Layout,
  flags: u16,
  size: u64,
) -> Result<u64, Error> {
  let mut size_2 = size;

  match (size_2, flags & layout.grouping) {
    (0, _) | (_, 0) => {}
    _ => {
      reader.read_byte()?;
      size_2 = size_2.saturating_sub(1);
    }
  }

  match (size_2, flags & layout.data_length) {
    (0..=3, _) | (_, 0) => {}
    _ => {
      read_syncsafe(reader, 28)?;
      size_2 = size_2.saturating_sub(4);
    }
  }

  Ok(size_2)
}

fn read_payload(
  reader: &mut BufReader<'_>,
  layout: &Layout,
  flags: u16,
  size: u64,
  wanted: &Wanted,
) -> Result<Frame, Error> {
  refuse_unsupported(reader, layout, flags, size)?;

  let size_2 = skip_prefixes(reader, layout, flags, size)?;

  match size_2 {
    0 => Ok(Frame::Invalid(wanted.name.clone())),
    _ => read_body(reader, layout, flags, size_2, wanted),
  }
}

fn read_body(
  reader: &mut BufReader<'_>,
  layout: &Layout,
  flags: u16,
  size: u64,
  wanted: &Wanted,
) -> Result<Frame, Error> {
  let data = read_data(reader, size)?;

  match flags & layout.unsynchronisation {
    0 => {
      let Ok(mut body) = BufReader::new(data);

      parsers::parse(wanted.kind, &mut body, wanted.standard, &wanted.name)
    }
    _ => {
      let Ok(decoded) = decode_unsynchronisation(data);
      let Ok(mut body) = BufReader::new(&decoded);

      parsers::parse(wanted.kind, &mut body, wanted.standard, &wanted.name)
    }
  }
}
