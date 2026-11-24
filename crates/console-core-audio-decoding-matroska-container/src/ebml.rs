//! The stream, walked one element header at a time.
//!
//! Upstream read every element, master or not, through an iterator that kept its
//! own idea of where the next sibling began, and each master element read its
//! children out of the stream through the same iterator. Only the top of a file
//! has to be read that way, because a Cluster can be as long as the file. A
//! master element small enough to hold -- the tracks, the cues, the tags, a
//! block group -- is read whole and its children are taken out of the bytes by
//! `console-core-extensible-binary-meta-language`, so the walker here only
//! knows where a header is, how much follows it, and how to move past it.

use std::cmp::Ordering;
use std::io::{Seek, SeekFrom};

use console_core_audio_decoding_interfaces::errors::SeekErrorKind;
use console_core_byte_streams::{MediaSource, MediaSourceStream, ReadBytes, Seeking};
use console_core_extensible_binary_meta_language::{
  Size, read_id as decode_id, read_size as decode_size,
};
use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::index;

use crate::fault::{DecodeError, Invalid};
use crate::ids;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ElementHeader {
  pub(crate) id: u32,
  pub(crate) data_pos: u64,
  pub(crate) data_len: u64,
  pub(crate) end: Option<u64>,
}

pub(crate) struct Walker {
  reader: MediaSourceStream,
  next_pos: u64,
  end: Option<u64>,
}

impl Walker {
  pub(crate) fn new(reader: MediaSourceStream, end: Option<u64>) -> Result<Self, Never> {
    let next_pos = reader.pos();

    Ok(Self {
      reader,
      next_pos,
      end,
    })
  }

  pub(crate) fn into_inner(self) -> Result<MediaSourceStream, Never> {
    Ok(self.reader)
  }

  pub(crate) fn rewind(self, to: u64) -> Result<Self, DecodeError> {
    let end = self.end;
    let mut reader = self.reader;

    reader.seek(SeekFrom::Start(to))?;

    let Ok(walker) = Self::new(reader, end);

    Ok(walker)
  }

  pub(crate) fn position(&self) -> Result<u64, Never> {
    Ok(self.reader.pos())
  }

  pub(crate) fn read_header(&mut self) -> Result<Option<ElementHeader>, DecodeError> {
    self.catch_up()?;

    match self.end.map(|end| self.reader.pos().cmp(&end)) {
      Some(Ordering::Less) | None => self.read_header_here().map(Some),
      Some(Ordering::Equal | Ordering::Greater) => Ok(None),
    }
  }

  pub(crate) fn skip(&mut self, header: &ElementHeader) -> Result<(), DecodeError> {
    self.reader.ignore_bytes(header.data_len)?;
    self.next_pos = header.data_pos.saturating_add(header.data_len);

    Ok(())
  }

  pub(crate) fn read_payload(
    &mut self,
    header: &ElementHeader,
  ) -> Result<Box<[u8]>, DecodeError> {
    let limit = self
      .end
      .map(|end| header.data_pos.saturating_add(header.data_len).cmp(&end));

    match limit {
      Some(Ordering::Less | Ordering::Equal) | None => {}
      Some(Ordering::Greater) => return Err(DecodeError::Invalid(Invalid::Length)),
    }

    let data = self.reader.read_boxed_slice_exact(header.data_len)?;

    self.next_pos = self.reader.pos();

    Ok(data)
  }

  pub(crate) fn seek(&mut self, to: u64) -> Result<(), DecodeError> {
    let at = self.reader.pos();

    match (self.reader.seeking(), to.cmp(&at)) {
      (Seeking::Anywhere, _) => {
        self.reader.seek(SeekFrom::Start(to))?;
      }
      (Seeking::ForwardOnly, Ordering::Less) => {
        return Err(DecodeError::Seek(SeekErrorKind::ForwardOnly));
      }
      (Seeking::ForwardOnly, Ordering::Equal | Ordering::Greater) => {
        self.reader.ignore_bytes(to.saturating_sub(at))?;
      }
    }

    self.next_pos = to;

    Ok(())
  }

  fn catch_up(&mut self) -> Result<(), DecodeError> {
    let at = self.reader.pos();

    match at.cmp(&self.next_pos) {
      Ordering::Less => self
        .reader
        .ignore_bytes(self.next_pos.saturating_sub(at))
        .map_err(DecodeError::from),
      Ordering::Equal => Ok(()),
      Ordering::Greater => Err(DecodeError::Invalid(Invalid::Position)),
    }
  }

  fn read_header_here(&mut self) -> Result<ElementHeader, DecodeError> {
    let id = self.read_id()?;
    let size = self.read_size()?;
    let data_pos = self.reader.pos();
    let data_len = match size {
      Size::Known(len) => len,
      Size::Unknown => 0,
    };
    let end = match data_len {
      0 => None,
      len => Some(data_pos.saturating_add(len)),
    };

    self.next_pos = data_pos;

    Ok(ElementHeader {
      id,
      data_pos,
      data_len,
      end,
    })
  }

  fn read_variable_integer_bytes(&mut self, first: u8) -> Result<[u8; 8], DecodeError> {
    let Ok(rest) = index(first.leading_zeros());
    let mut bytes = [first, 0, 0, 0, 0, 0, 0, 0];

    for slot in bytes.iter_mut().skip(1).take(rest) {
      let byte = self.reader.read_byte()?;

      *slot = byte;
    }

    Ok(bytes)
  }

  fn read_size(&mut self) -> Result<Size, DecodeError> {
    let first = self.reader.read_byte()?;
    let bytes = self.read_variable_integer_bytes(first)?;
    let Ok(width) = index(first.leading_zeros().saturating_add(1));

    match bytes.get(..width).map(decode_size) {
      Some(Ok(Some((size, _width)))) => Ok(size),
      Some(Ok(None)) | None => Err(DecodeError::Invalid(Invalid::VariableInteger)),
    }
  }

  fn read_id(&mut self) -> Result<u32, DecodeError> {
    let first = self.reader.read_byte()?;

    match first.leading_zeros() {
      0..=3 => {
        let bytes = self.read_variable_integer_bytes(first)?;
        let Ok(width) = index(first.leading_zeros().saturating_add(1));

        match bytes.get(..width).map(decode_id) {
          Some(Ok(Some((id, _width)))) => Ok(id),
          Some(Ok(None)) | None => Err(DecodeError::Invalid(Invalid::ElementId)),
        }
      }
      4.. => self.find_top_level_id(),
    }
  }

  fn find_top_level_id(&mut self) -> Result<u32, DecodeError> {
    iterate((&mut self.reader, 0u32), |(reader, scanned)| {
      Ok(match scanned {
        ids::CLUSTER | ids::CUES | ids::INFO | ids::SEEK_HEAD | ids::TAGS | ids::TRACKS => {
          Step::Halt(Ok(scanned))
        }
        _ => match reader.read_u8() {
          Ok(byte) => Step::Again((reader, scanned.wrapping_shl(8) | u32::from(byte))),
          Err(fault) => Step::Halt(Err(DecodeError::from(fault))),
        },
      })
    })
    .map_err(DecodeError::from)
    .and_then(std::convert::identity)
  }
}
