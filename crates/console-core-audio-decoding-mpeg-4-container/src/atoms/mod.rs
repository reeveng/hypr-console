//! The atoms of an ISO base media file, and the iterator that walks the ones in a container.
//!
//! An atom is named by four bytes and kept as them: the list of names a player reads is
//! short, each module matches the ones it keeps against constants, and anything else is
//! stepped over by the iterator's next call, which is what makes an atom nobody reads free.

use std::cmp::Ordering;

use console_core_iso_media_boxes::{BoxHeader, FullBoxHeader as FullBox};

use console_core_audio_decoding_interfaces::errors::{Error, Result, decode_error};
use console_core_byte_streams::ReadBytes;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

pub(crate) mod alac;
pub(crate) mod esds;
pub(crate) mod flac;
pub(crate) mod fragment;
pub(crate) mod metadata;
pub(crate) mod movie;
pub(crate) mod opus;
pub(crate) mod sample_entry;
pub(crate) mod sample_table;
pub(crate) mod track;

pub(crate) const OVERFLOW: Error = Error::DecodeError("isomp4: a size or count overflows");

const OVERREAD: Error = Error::DecodeError("isomp4: overread atom");

const NO_ATOM: Error = Error::DecodeError("isomp4: no atom is open to read");

const ATOM_SIZE_INVALID: Error = Error::DecodeError("isomp4: atom size is invalid");

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Kind(pub [u8; 4]);

impl Kind {
  pub const FILE_TYPE: Kind = Kind(*b"ftyp");
  pub const MOVIE: Kind = Kind(*b"moov");
  pub const MOVIE_HEADER: Kind = Kind(*b"mvhd");
  pub const TRACK: Kind = Kind(*b"trak");
  pub const TRACK_HEADER: Kind = Kind(*b"tkhd");
  pub const MEDIA: Kind = Kind(*b"mdia");
  pub const MEDIA_HEADER: Kind = Kind(*b"mdhd");
  pub const HANDLER: Kind = Kind(*b"hdlr");
  pub const MEDIA_INFO: Kind = Kind(*b"minf");
  pub const SAMPLE_TABLE: Kind = Kind(*b"stbl");
  pub const SAMPLE_DESCRIPTION: Kind = Kind(*b"stsd");
  pub const TIME_TO_SAMPLE: Kind = Kind(*b"stts");
  pub const SAMPLE_TO_CHUNK: Kind = Kind(*b"stsc");
  pub const SAMPLE_SIZE: Kind = Kind(*b"stsz");
  pub const CHUNK_OFFSET: Kind = Kind(*b"stco");
  pub const CHUNK_OFFSET_64: Kind = Kind(*b"co64");
  pub const MOVIE_EXTENDS: Kind = Kind(*b"mvex");
  pub const TRACK_EXTENDS: Kind = Kind(*b"trex");
  pub const MOVIE_FRAGMENT: Kind = Kind(*b"moof");
  pub const MOVIE_FRAGMENT_HEADER: Kind = Kind(*b"mfhd");
  pub const TRACK_FRAGMENT: Kind = Kind(*b"traf");
  pub const TRACK_FRAGMENT_HEADER: Kind = Kind(*b"tfhd");
  pub const TRACK_FRAGMENT_RUN: Kind = Kind(*b"trun");
  pub const MEDIA_DATA: Kind = Kind(*b"mdat");
  pub const SEGMENT_INDEX: Kind = Kind(*b"sidx");
  pub const USER_DATA: Kind = Kind(*b"udta");
  pub const META: Kind = Kind(*b"meta");
  pub const META_LIST: Kind = Kind(*b"ilst");
}

#[derive(Copy, Clone, Debug)]
pub struct AtomHeader {
  pub kind: Kind,
  pub header_len: u64,
  pub atom_len: u64,
  pub data_len: u64,
}

impl AtomHeader {
  pub fn read<B: ReadBytes>(reader: &mut B) -> Result<AtomHeader> {
    let first = reader.read_quad_bytes()?;
    let second = reader.read_quad_bytes()?;
    let mut bytes = BoxHeader::BUFFER;

    for (destination, source) in bytes.iter_mut().zip(first.into_iter().chain(second)) {
      *destination = source;
    }

    let Ok(base) = index(BoxHeader::BASE_HEADER_LEN);
    let prefix = bytes.get(..base).ok_or(ATOM_SIZE_INVALID)?;
    let width = BoxHeader::required_len(prefix)
      .map(|native| {
        let Ok(count) = fitted::<_, u64>(native);
        count
      })
      .map_err(|_invalid| ATOM_SIZE_INVALID)?;
    let Ok(end) = index(width);
    let remainder = bytes.get_mut(base..end).ok_or(ATOM_SIZE_INVALID)?;

    reader.read_buf_exact(remainder)?;

    let encoded = bytes.get(..end).ok_or(ATOM_SIZE_INVALID)?;
    let header = BoxHeader::parse(encoded).map_err(|_invalid| ATOM_SIZE_INVALID)?;
    let atom_len = match header.size {
      Some(total) => total,
      None => 0,
    };
    let Ok(payload_len) = header.payload_len();
    let data_len = match payload_len {
      Some(payload) => payload,
      None => 0,
    };

    Ok(AtomHeader {
      kind: Kind(header.kind.0),
      header_len: header.header_len,
      atom_len,
      data_len,
    })
  }

  pub fn read_full_box<B: ReadBytes>(reader: &mut B) -> Result<FullBox> {
    let version = reader.read_byte()?;
    let [a, b, c] = reader.read_triple_bytes()?;
    let Ok(full_box) = FullBox::from_bytes([version, a, b, c]);

    Ok(full_box)
  }

  pub fn payload_after_full_box(&self) -> Result<u64> {
    let Ok(version_bytes) = fitted::<_, u64>(FullBox::ENCODED_LEN);

    self
      .data_len
      .checked_sub(version_bytes)
      .ok_or(ATOM_SIZE_INVALID)
  }
}

pub trait Atom: Sized {
  fn read<B: ReadBytes>(reader: &mut B, header: AtomHeader) -> Result<Self>;
}

pub struct AtomIterator<B: ReadBytes> {
  reader: B,
  len: Option<u64>,
  cur_atom: Option<AtomHeader>,
  base_pos: u64,
  next_atom_pos: u64,
}

impl<B: ReadBytes> AtomIterator<B> {
  pub fn new_root(reader: B, len: Option<u64>) -> std::result::Result<Self, Never> {
    let base_pos = reader.pos();

    Ok(AtomIterator {
      reader,
      len,
      cur_atom: None,
      base_pos,
      next_atom_pos: base_pos,
    })
  }

  pub fn new(reader: B, container: AtomHeader) -> std::result::Result<Self, Never> {
    let base_pos = reader.pos();

    Ok(AtomIterator {
      reader,
      len: Some(container.data_len),
      cur_atom: None,
      base_pos,
      next_atom_pos: base_pos,
    })
  }

  pub fn into_inner(self) -> std::result::Result<B, Never> {
    Ok(self.reader)
  }

  pub fn inner_mut(&mut self) -> std::result::Result<&mut B, Never> {
    Ok(&mut self.reader)
  }

  pub fn atoms_at_most(&self) -> std::result::Result<u64, Never> {
    let Ok(minimum_header) = fitted::<_, u64>(BoxHeader::BASE_HEADER_LEN);

    Ok(match self.len {
      Some(len) => match len.checked_div(minimum_header) {
        Some(count) => count.saturating_add(1),
        None => u64::MAX,
      },
      None => u64::MAX,
    })
  }

  pub fn next(&mut self) -> Result<Option<AtomHeader>> {
    let cur_pos = self.reader.pos();

    match cur_pos.cmp(&self.next_atom_pos) {
      Ordering::Less => {
        let gap = self.next_atom_pos.checked_sub(cur_pos).ok_or(OVERREAD)?;

        self.reader.ignore_bytes(gap)?;
      }
      Ordering::Greater => return decode_error("isomp4: overread atom"),
      Ordering::Equal => {}
    }

    let consumed = self
      .next_atom_pos
      .checked_sub(self.base_pos)
      .ok_or(OVERREAD)?;

    match self.len {
      Some(len) => match consumed.cmp(&len) {
        Ordering::Less => {}
        Ordering::Equal | Ordering::Greater => return Ok(None),
      },
      None => {}
    }

    let atom = AtomHeader::read(&mut self.reader)?;

    self.next_atom_pos = match atom.atom_len {
      0 => match self.len {
        Some(len) => self.base_pos.checked_add(len).ok_or(OVERFLOW)?,
        None => u64::MAX,
      },
      len => self.next_atom_pos.checked_add(len).ok_or(OVERFLOW)?,
    };

    self.cur_atom = Some(atom);

    Ok(self.cur_atom)
  }

  pub fn next_no_consume(&mut self) -> Result<Option<AtomHeader>> {
    match self.cur_atom {
      Some(atom) => Ok(Some(atom)),
      None => self.next(),
    }
  }

  pub fn read_atom<A: Atom>(&mut self) -> Result<A> {
    match self.cur_atom.take() {
      Some(header) => A::read(&mut self.reader, header),
      None => Err(NO_ATOM),
    }
  }

  pub fn consume_atom(&mut self) -> std::result::Result<(), Never> {
    self.cur_atom = None;

    Ok(())
  }
}

#[cfg(test)]
mod framing_tests {
  use super::*;
  use console_core_byte_streams::BufReader;

  #[test]
  fn stream_headers_leave_the_reader_at_the_payload_for_every_header_shape() -> Result<()> {
    for (kind, width, size_field, total_size) in [
      (*b"free", 8u32, 11u32, 11u64),
      (*b"free", 16, 1, 19),
      (*b"uuid", 24, 27, 27),
      (*b"uuid", 32, 1, 35),
    ] {
      let mut bytes = size_field.to_be_bytes().to_vec();
      bytes.extend(kind);

      match size_field {
        1 => bytes.extend(total_size.to_be_bytes()),
        _ => {}
      }

      match &kind {
        b"uuid" => bytes.extend([7; 16]),
        _ => {}
      }

      bytes.extend([41, 42, 43]);
      let Ok(mut reader) = BufReader::new(&bytes);
      let header = AtomHeader::read(&mut reader)?;
      let payload = reader.read_triple_bytes()?;

      assert_eq!(header.kind, Kind(kind));
      assert_eq!(header.atom_len, total_size);
      assert_eq!(header.data_len, 3);
      assert_eq!(header.header_len, u64::from(width));
      assert_eq!(payload, [41, 42, 43]);
      assert_eq!(reader.pos(), total_size);
    }

    Ok(())
  }

  #[test]
  fn streaming_full_box_fields_leave_the_remaining_payload_unread() -> Result<()> {
    let Ok(mut reader) = BufReader::new(&[0x81, 0xfe, 0xdc, 0xba, 77]);
    let header = AtomHeader::read_full_box(&mut reader)?;
    let next_byte = reader.read_byte()?;

    assert_eq!(header.version, 0x81);
    assert_eq!(header.flags, 0x00fe_dcba);
    assert_eq!(next_byte, 77);

    Ok(())
  }

  #[test]
  fn a_truncated_stream_keeps_the_existing_field_read_positions() {
    let Ok(mut header_stream) = BufReader::new(&[0, 0, 0, 8]);
    let header = AtomHeader::read(&mut header_stream);

    assert!(matches!(header, Err(Error::IoError(_truncated_header))));
    assert_eq!(header_stream.pos(), 4);
    let Ok(mut full_box_stream) = BufReader::new(&[1, 2]);
    let full_box = AtomHeader::read_full_box(&mut full_box_stream);

    assert!(matches!(full_box, Err(Error::IoError(_truncated_flags))));
    assert_eq!(full_box_stream.pos(), 1);
  }

  #[test]
  fn incomplete_and_undersized_headers_are_rejected() {
    let Ok(mut free) = BufReader::new(&[0, 0, 0, 4, b'f', b'r', b'e', b'e']);
    let undersized = AtomHeader::read(&mut free);

    assert!(matches!(
      undersized,
      Err(Error::DecodeError("isomp4: atom size is invalid"))
    ));

    let truncated_headers: [&[u8]; 2] = [&[0, 0, 0, 1, b'u', b'u', b'i', b'd', 0], &[0, 0, 0]];

    for bytes in truncated_headers {
      let Ok(mut truncated) = BufReader::new(bytes);
      let incomplete = AtomHeader::read(&mut truncated);

      assert!(matches!(incomplete, Err(Error::IoError(_truncated_header))));
    }
  }
}
