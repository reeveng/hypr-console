//! The two ways the crate reads: a slice taken from the front, and a file read
//! at a place.
//!
//! A tag, a comment block and an atom are small and are read whole into
//! memory, then taken apart with `Cursor`, which says a short read as
//! `Malformed::Short` and leaves the caller to decide whether short means the
//! end of what it was reading or a broken file. The audio around them can be
//! large, so `Source` reads a file a piece at a time and seeks between the
//! pieces, the way each format's own reader walks it, and a read past the end
//! is the same `UnexpectedEof` a `read_exact` gives.

use std::cmp::Ordering;
use std::io::{ErrorKind, Read, Seek, SeekFrom};

use console_core_bytes::Cursor as ByteCursor;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::fault::Malformed;

pub(crate) fn length(bytes: &[u8]) -> Result<u64, Never> {
  fitted::<_, u64>(bytes.len())
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the public read call takes a caller-provided seekable reader, so any I/O deadline belongs to that reader and this call bounds the byte count"
  )
)]
pub(crate) fn read_into(reader: &mut impl Read, bytes: &mut [u8]) -> Result<(), std::io::Error> {
  reader.read_exact(bytes)
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Cursor<'a> {
  bytes: ByteCursor<'a>,
}

impl<'a> Cursor<'a> {
  pub(crate) fn new(bytes: &'a [u8]) -> Result<Cursor<'a>, Never> {
    let Ok(cursor) = ByteCursor::new(bytes);

    Ok(Cursor { bytes: cursor })
  }

  pub(crate) fn rest(&self) -> Result<&'a [u8], Never> {
    self.bytes.rest()
  }

  pub(crate) fn left(&self) -> Result<u64, Never> {
    let Ok(rest) = self.bytes.rest();

    length(rest)
  }

  pub(crate) fn take(&mut self, count: u64) -> Result<&'a [u8], Malformed> {
    self.bytes.take(count).map_err(|_| Malformed::Short)
  }

  pub(crate) fn upto(&mut self, count: u64) -> Result<&'a [u8], Never> {
    let Ok(left) = self.left();

    match self.take(count.min(left)) {
      Ok(taken) => Ok(taken),
      Err(_short) => Ok(&[]),
    }
  }

  pub(crate) fn array<const WIDTH: usize>(&mut self) -> Result<[u8; WIDTH], Malformed> {
    self.bytes.array().map_err(|_| Malformed::Short)
  }

  pub(crate) fn byte(&mut self) -> Result<u8, Malformed> {
    self.bytes.byte().map_err(|_| Malformed::Short)
  }

  pub(crate) fn u16_le(&mut self) -> Result<u16, Malformed> {
    self.bytes.u16_le().map_err(|_| Malformed::Short)
  }

  pub(crate) fn u32_be(&mut self) -> Result<u32, Malformed> {
    self.bytes.u32_be().map_err(|_| Malformed::Short)
  }

  pub(crate) fn u32_le(&mut self) -> Result<u32, Malformed> {
    self.bytes.u32_le().map_err(|_| Malformed::Short)
  }

  pub(crate) fn skip(&mut self, count: u64) -> Result<(), Malformed> {
    self.take(count)?;

    Ok(())
  }

  pub(crate) fn finish(&mut self) -> Result<(), Never> {
    let Ok(_skipped) = self.bytes.take_rest();

    Ok(())
  }
}

pub(crate) struct Source<'r, R> {
  reader: &'r mut R,
  length: u64,
}

impl<'r, R: Read + Seek> Source<'r, R> {
  pub(crate) fn new(reader: &'r mut R) -> Result<Source<'r, R>, std::io::Error> {
    let length = reader.seek(SeekFrom::End(0))?;

    reader.seek(SeekFrom::Start(0))?;

    Ok(Source { reader, length })
  }

  pub(crate) fn length(&self) -> Result<u64, Never> {
    Ok(self.length)
  }

  pub(crate) fn at(&mut self) -> Result<u64, std::io::Error> {
    self.reader.stream_position()
  }

  pub(crate) fn seek_to(&mut self, place: u64) -> Result<(), std::io::Error> {
    self.reader.seek(SeekFrom::Start(place))?;

    Ok(())
  }

  pub(crate) fn seek_forward(&mut self, count: u64) -> Result<(), std::io::Error> {
    let place = self.at()?;

    self.seek_to(place.saturating_add(count))
  }

  pub(crate) fn left(&mut self) -> Result<u64, std::io::Error> {
    let place = self.at()?;

    Ok(self.length.saturating_sub(place))
  }

  pub(crate) fn exact(&mut self, count: u64) -> Result<Vec<u8>, std::io::Error> {
    let left = self.left()?;

    match count.cmp(&left) {
      Ordering::Greater => {
        let end = self.length;

        self.seek_to(end)?;

        return Err(std::io::Error::from(ErrorKind::UnexpectedEof));
      }
      Ordering::Less | Ordering::Equal => {}
    }

    let mut buffer = [0u8; 16_384];
    let Ok(block) = length(&buffer);
    let mut bytes = Vec::new();
    let mut remaining = count;

    for _part in 0..count.div_ceil(block.max(1)) {
      let part = remaining.min(block);
      let Ok(size) = index(part);
      let piece = buffer.get_mut(..size).ok_or(ErrorKind::InvalidInput)?;

      read_into(self.reader, piece)?;
      bytes
        .try_reserve(piece.len())
        .map_err(|_fault| ErrorKind::OutOfMemory)?;
      bytes.extend_from_slice(piece);
      remaining = remaining.saturating_sub(part);
    }

    Ok(bytes)
  }

  pub(crate) fn upto(&mut self, count: u64) -> Result<Vec<u8>, std::io::Error> {
    let left = self.left()?;

    self.exact(count.min(left))
  }

  pub(crate) fn array<const WIDTH: usize>(&mut self) -> Result<[u8; WIDTH], std::io::Error> {
    let mut bytes = [0u8; WIDTH];

    read_into(self.reader, &mut bytes)?;

    Ok(bytes)
  }

  pub(crate) fn byte(&mut self) -> Result<u8, std::io::Error> {
    let [read] = self.array::<1>()?;

    Ok(read)
  }
}
