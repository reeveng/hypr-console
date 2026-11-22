//! The ten bytes an ID3v2 tag opens with, and the extended header after them.
//!
//! A version past 2.4 is refused, as is the compression bit 2.2 defined and
//! nobody wrote. The extended header is read only for what lofty read of it,
//! which is how far to step over it: a CRC and a restrictions byte are
//! skipped by the flags that announce them, and the size the header gives
//! itself is what the tag's own size is measured against. In a 2.3 tag that
//! is unsynchronised the extended header is unsynchronised with it, so it is
//! read through the same rule that undoes the rest of the tag.

use std::cmp::Ordering;
use std::io::{Read, Seek};

use console_core_never::Never;

use crate::bytes::Source;
use crate::fault::{Malformed, ParseError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Version {
  Two,
  Three,
  Four,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Header {
  pub(crate) version: Version,
  pub(crate) flags: u8,
  pub(crate) size: u32,
  pub(crate) extended_size: u32,
}

const UNSYNCHRONISED: u8 = 0x80;

const EXTENDED: u8 = 0x40;

const FOOTER: u8 = 0x10;

pub(crate) fn synchsafe(bytes: [u8; 4]) -> Result<u32, Never> {
  let [a, b, c, d] = bytes;

  Ok(
    u32::from(a & 0x7F).wrapping_shl(21)
      | u32::from(b & 0x7F).wrapping_shl(14)
      | u32::from(c & 0x7F).wrapping_shl(7)
      | u32::from(d & 0x7F),
  )
}

impl Header {
  pub(crate) fn unsynchronised(&self) -> Result<Unsynchronised, Never> {
    Ok(match self.flags & UNSYNCHRONISED {
      0 => Unsynchronised::No,
      _set => Unsynchronised::Yes,
    })
  }

  pub(crate) fn footer(&self) -> Result<u64, Never> {
    Ok(match (self.version, self.flags & FOOTER) {
      (Version::Three | Version::Four, FOOTER) => 10,
      (Version::Two, _) | (Version::Three | Version::Four, _) => 0,
    })
  }

  pub(crate) fn content_size(&self) -> Result<u64, Never> {
    Ok(u64::from(self.size.saturating_sub(self.extended_size)))
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unsynchronised {
  Yes,
  No,
}

pub(crate) fn opening(bytes: [u8; 10]) -> Result<(Version, u8, u32), Malformed> {
  let [i, d, three, major, _minor, flags, s0, s1, s2, s3] = bytes;

  match [i, d, three] {
    [b'I', b'D', b'3'] => {}
    _other => return Err(Malformed::Signature),
  }

  let version = match major {
    2 => Version::Two,
    3 => Version::Three,
    4 => Version::Four,
    _other => return Err(Malformed::TagVersion),
  };

  match (version, flags & EXTENDED) {
    (Version::Two, EXTENDED) => return Err(Malformed::TagVersion),
    (Version::Two, _) | (Version::Three | Version::Four, _) => {}
  }

  let Ok(size) = synchsafe([s0, s1, s2, s3]);

  Ok((version, flags, size))
}

pub(crate) fn read<R: Read + Seek>(source: &mut Source<'_, R>) -> Result<Header, ParseError> {
  let bytes = source.array::<10>()?;
  let (version, flags, size) = opening(bytes)?;

  let extended_size = match (version, flags & EXTENDED, flags & UNSYNCHRONISED) {
    (Version::Two, _extended, _unsynchronised) => 0,
    (Version::Three | Version::Four, 0, _unsynchronised) => 0,
    (Version::Four, _extended, _unsynchronised) => {
      let size_2 = source.array::<4>()?;
      let Ok(size_3) = synchsafe(size_2);

      extended(source, size_3, Unsynchronised::No)?
    }
    (Version::Three, _extended, 0) => {
      let size_4 = source.array::<4>()?;

      extended(source, u32::from_be_bytes(size_4), Unsynchronised::No)?
    }
    (Version::Three, _extended, _unsynchronised) => {
      let size_5 = unsynchronised_bytes(source, 4)?;

      let size_6 = match size_5.as_slice() {
        [a, b, c, d] => u32::from_be_bytes([*a, *b, *c, *d]),
        _short => return Err(ParseError::Parse(Malformed::Short)),
      };

      extended(source, size_6, Unsynchronised::Yes)?
    }
  };

  match (extended_size, extended_size.cmp(&size)) {
    (1.., Ordering::Equal | Ordering::Greater) => {
      return Err(ParseError::Parse(Malformed::TagSize));
    }
    (0, _) | (_, Ordering::Less) => {}
  }

  Ok(Header {
    version,
    flags,
    size,
    extended_size,
  })
}

fn extended<R: Read + Seek>(
  source: &mut Source<'_, R>,
  size: u32,
  unsynchronised: Unsynchronised,
) -> Result<u32, ParseError> {
  match size {
    0..=5 => return Err(ParseError::Parse(Malformed::TagSize)),
    _has_header => {}
  }

  let flags = match unsynchronised {
    Unsynchronised::No => source.array::<2>(),
    Unsynchronised::Yes => {
      let read = unsynchronised_bytes(source, 2)?;

      match read.as_slice() {
        [count, flags] => Ok([*count, *flags]),
        _short => return Err(ParseError::Parse(Malformed::Short)),
      }
    }
  };

  let flags_2 = flags?;
  let [_count, flags_3] = flags_2;
  let checksum = match flags_3 & 0x20 {
    0 => 0,
    _set => 6,
  };
  let restrictions = match flags_3 & 0x10 {
    0 => 0,
    _set => 2,
  };

  match unsynchronised {
    Unsynchronised::No => {
      source.exact(checksum)?;
      source.exact(restrictions)?;
    }
    Unsynchronised::Yes => {
      unsynchronised_bytes(source, checksum)?;
      unsynchronised_bytes(source, restrictions)?;
    }
  }

  Ok(size)
}

fn unsynchronised_bytes<R: Read + Seek>(
  source: &mut Source<'_, R>,
  count: u64,
) -> Result<Vec<u8>, ParseError> {
  let mut read = Vec::new();
  let mut previous = 0u8;

  'bytes: for _byte in 0..=u32::MAX {
    let Ok(held) = crate::bytes::length(&read);

    match held.cmp(&count) {
      Ordering::Equal | Ordering::Greater => break 'bytes,
      Ordering::Less => {}
    }

    let byte = source.byte()?;

    match (previous, byte) {
      (0xFF, 0x00) => previous = 0x00,
      (_previous, byte_2) => {
        read.push(byte_2);
        previous = byte_2;
      }
    }
  }

  Ok(read)
}

pub(crate) fn resynchronised(bytes: &[u8]) -> Result<Vec<u8>, Never> {
  let mut undone = Vec::with_capacity(bytes.len());
  let mut previous = 0u8;

  for byte in bytes {
    match (previous, *byte) {
      (0xFF, 0x00) => previous = 0x00,
      (_previous, byte_2) => {
        undone.push(byte_2);
        previous = byte_2;
      }
    }
  }

  Ok(undone)
}
