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

use console_core_audio_decoding_interfaces::id3v2::{
  COMPRESSION_OR_EXTENDED_HEADER, UNSYNCHRONISED, UnsynchronisationDecoder, read_tag_fields,
  synchsafe_word, tag_unsynchronisation,
};
use console_core_audio_decoding_interfaces::io::BufReader;
use console_core_never::Never;

use crate::bytes::Source;
use crate::fault::{Malformed, ParseError};

pub(crate) use console_core_audio_decoding_interfaces::id3v2::{Unsynchronisation, Version};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Header {
  pub(crate) version: Version,
  pub(crate) flags: u8,
  pub(crate) size: u32,
  pub(crate) extended_size: u32,
}

const FOOTER: u8 = 0x10;

impl Header {
  pub(crate) fn unsynchronised(&self) -> Result<Unsynchronisation, Never> {
    tag_unsynchronisation(self.flags)
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

pub(crate) fn read<R: Read + Seek>(source: &mut Source<'_, R>) -> Result<Header, ParseError> {
  let bytes = source.array::<10>()?;
  let Ok(mut reader) = BufReader::new(&bytes);
  let tag_fields = read_tag_fields(&mut reader)?;
  let fields = tag_fields.ok_or(Malformed::Signature)?;
  let version = fields.version().map_err(|_invalid| Malformed::TagVersion)?;
  let flags = fields.flags;
  let size = fields.size;

  let extended_size = match (
    version,
    flags & COMPRESSION_OR_EXTENDED_HEADER,
    flags & UNSYNCHRONISED,
  ) {
    (Version::Two, _extended, _unsynchronised) => 0,
    (Version::Three | Version::Four, 0, _unsynchronised) => 0,
    (Version::Four, _extended, _unsynchronised) => {
      let size_2 = source.array::<4>()?;
      let Ok(size_3) = synchsafe_word(size_2);

      extended(source, size_3, Unsynchronisation::Absent)?
    }
    (Version::Three, _extended, 0) => {
      let size_4 = source.array::<4>()?;

      extended(
        source,
        u32::from_be_bytes(size_4),
        Unsynchronisation::Absent,
      )?
    }
    (Version::Three, _extended, _unsynchronised) => {
      let size_5 = unsynchronised_bytes(source, 4)?;

      let size_6 = match size_5.as_slice() {
        [a, b, c, d] => u32::from_be_bytes([*a, *b, *c, *d]),
        _short => return Err(ParseError::Parse(Malformed::Short)),
      };

      extended(source, size_6, Unsynchronisation::Applied)?
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
  unsynchronised: Unsynchronisation,
) -> Result<u32, ParseError> {
  match size {
    0..=5 => return Err(ParseError::Parse(Malformed::TagSize)),
    _has_header => {}
  }

  let flags = match unsynchronised {
    Unsynchronisation::Absent => source.array::<2>(),
    Unsynchronisation::Applied => {
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
    Unsynchronisation::Absent => {
      source.exact(checksum)?;
      source.exact(restrictions)?;
    }
    Unsynchronisation::Applied => {
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
  let mut decoder = UnsynchronisationDecoder::default();

  'bytes: for _byte in 0..=u32::MAX {
    let Ok(held) = crate::bytes::length(&read);

    match held.cmp(&count) {
      Ordering::Equal | Ordering::Greater => break 'bytes,
      Ordering::Less => {}
    }

    let Ok(kept) = decoder.push({
      let checked_value = source.byte()?;
      checked_value
    });

    read.extend(kept);
  }

  Ok(read)
}
