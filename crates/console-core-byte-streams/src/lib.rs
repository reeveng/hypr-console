//! Reading bytes and bits out of a source without knowing the format carried in it.
//!
//! A `Stream` reads from anything that implements [`ReadBytes`], and a `Reader` reads a byte
//! slice it borrows. [`MediaSourceStream`] is the one exception, reading anything that
//! implements [`MediaSource`] through a ring of its own. Bits are read in one of two orders,
//! most significant first through [`ReadBitsLtr`] or least significant first through
//! [`ReadBitsRtl`], over a byte slice or over a byte stream.
//!
//! Every count and position is held at the width a stream measures itself in, which is `u64`,
//! and meets the `usize` the standard library measures a slice in only where a slice is cut.
//! The two `io::Read` impls are the exceptions, because that trait answers in `usize` itself.
//! Callers set any I/O deadline their media source needs before constructing a stream.
//! A single boxed read may allocate up to 256 MiB. The seek-back ring may grow
//! to 32 MiB, including its space for the next block.
//!
//! Adapted from Symphonia, Copyright (c) 2019-2022 The Project Symphonia Developers. This Source
//! Code Form is subject to the terms of the Mozilla Public License, v. 2.0, whose text is the
//! `LICENSE` beside this crate; a copy is also at <https://mozilla.org/MPL/2.0/>.

#![forbid(unsafe_code)]

use std::{cmp::Ordering, io};

use console_core_iteration::Endless;
use console_core_never::Never;
use console_core_number_conversion::{Ends, fitted, index};

mod bit;
mod buf_reader;
mod media_source_stream;
mod monitor_stream;
mod scoped_stream;
pub mod vlc;

pub use bit::{
  BitReaderLtr, BitReaderRtl, BitStreamLtr, FiniteBitStream, FlagBit, ReadBitsLtr, ReadBitsRtl,
};
pub use buf_reader::BufReader;
pub use console_core_checksums::Checksum as Monitor;
pub use media_source_stream::{MediaSourceStream, MediaSourceStreamOptions};
pub use monitor_stream::MonitorStream;
pub use scoped_stream::ScopedStream;

const MAX_BOXED_READ_LEN: u64 = 256 * 1024 * 1024;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Seeking {
  Anywhere,
  ForwardOnly,
}

impl From<bool> for Seeking {
  fn from(seekable: bool) -> Self {
    match bool::then_some(seekable, ()) {
      Some(()) => Self::Anywhere,
      None => Self::ForwardOnly,
    }
  }
}

pub trait MediaSource: io::Read + io::Seek + Send + Sync {
  fn seeking(&self) -> Seeking;

  fn byte_len(&self) -> Option<u64>;
}

impl MediaSource for std::fs::File {
  fn seeking(&self) -> Seeking {
    match self.metadata() {
      Ok(metadata) => Seeking::from(metadata.is_file()),
      Err(_unknown) => Seeking::ForwardOnly,
    }
  }

  fn byte_len(&self) -> Option<u64> {
    match self.metadata() {
      Ok(metadata) => Some(metadata.len()),
      Err(_unknown) => None,
    }
  }
}

impl<T: AsRef<[u8]> + Send + Sync> MediaSource for io::Cursor<T> {
  fn seeking(&self) -> Seeking {
    Seeking::Anywhere
  }

  fn byte_len(&self) -> Option<u64> {
    let Ok(length) = fitted::<_, u64>(self.get_ref().as_ref().len());

    Some(length)
  }
}

pub struct ReadOnlySource<R: io::Read> {
  inner: R,
}

impl<R: io::Read + Send> ReadOnlySource<R> {
  pub fn new(inner: R) -> Result<Self, Never> {
    Ok(ReadOnlySource { inner })
  }
}

impl<R: io::Read + Send + Sync> MediaSource for ReadOnlySource<R> {
  fn seeking(&self) -> Seeking {
    Seeking::ForwardOnly
  }

  fn byte_len(&self) -> Option<u64> {
    None
  }
}

impl<R: io::Read> io::Read for ReadOnlySource<R> {
  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "this adapter forwards to the reader supplied by the caller, which owns any I/O deadline"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    allow(
      explicit051_no_machine_width,
      reason = "std::io::Read requires a usize byte count, and this hands on the inner reader's own"
    )
  )]
  fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    self.inner.read(buf)
  }
}

impl<R: io::Read> io::Seek for ReadOnlySource<R> {
  fn seek(&mut self, _wanted: io::SeekFrom) -> io::Result<u64> {
    Err(io::Error::other("source does not support seeking"))
  }
}

pub trait ReadBytes {
  fn read_byte(&mut self) -> io::Result<u8>;

  fn read_double_bytes(&mut self) -> io::Result<[u8; 2]>;

  fn read_triple_bytes(&mut self) -> io::Result<[u8; 3]>;

  fn read_quad_bytes(&mut self) -> io::Result<[u8; 4]>;

  fn read_buf(&mut self, buf: &mut [u8]) -> io::Result<u64>;

  fn read_buf_exact(&mut self, buf: &mut [u8]) -> io::Result<()>;

  #[inline(always)]
  fn read_u8(&mut self) -> io::Result<u8> {
    self.read_byte()
  }

  #[inline(always)]
  fn read_i8(&mut self) -> io::Result<i8> {
    self.read_byte().map(u8::cast_signed)
  }

  #[inline(always)]
  fn read_u16(&mut self) -> io::Result<u16> {
    self.read_double_bytes().map(u16::from_le_bytes)
  }

  #[inline(always)]
  fn read_i16(&mut self) -> io::Result<i16> {
    self.read_double_bytes().map(i16::from_le_bytes)
  }

  #[inline(always)]
  fn read_be_u16(&mut self) -> io::Result<u16> {
    self.read_double_bytes().map(u16::from_be_bytes)
  }

  #[inline(always)]
  fn read_be_i16(&mut self) -> io::Result<i16> {
    self.read_double_bytes().map(i16::from_be_bytes)
  }

  #[inline(always)]
  fn read_u24(&mut self) -> io::Result<u32> {
    self
      .read_triple_bytes()
      .map(|[low, middle, high]| u32::from_le_bytes([low, middle, high, 0]))
  }

  #[inline(always)]
  fn read_i24(&mut self) -> io::Result<i32> {
    self
      .read_triple_bytes()
      .map(|[low, middle, high]| i32::from_le_bytes([0, low, middle, high]).wrapping_shr(8))
  }

  #[inline(always)]
  fn read_be_u24(&mut self) -> io::Result<u32> {
    self
      .read_triple_bytes()
      .map(|[high, middle, low]| u32::from_be_bytes([0, high, middle, low]))
  }

  #[inline(always)]
  fn read_be_i24(&mut self) -> io::Result<i32> {
    self
      .read_triple_bytes()
      .map(|[high, middle, low]| i32::from_be_bytes([high, middle, low, 0]).wrapping_shr(8))
  }

  #[inline(always)]
  fn read_u32(&mut self) -> io::Result<u32> {
    self.read_quad_bytes().map(u32::from_le_bytes)
  }

  #[inline(always)]
  fn read_i32(&mut self) -> io::Result<i32> {
    self.read_quad_bytes().map(i32::from_le_bytes)
  }

  #[inline(always)]
  fn read_be_u32(&mut self) -> io::Result<u32> {
    self.read_quad_bytes().map(u32::from_be_bytes)
  }

  #[inline(always)]
  fn read_be_i32(&mut self) -> io::Result<i32> {
    self.read_quad_bytes().map(i32::from_be_bytes)
  }

  #[inline(always)]
  fn read_be_u64(&mut self) -> io::Result<u64> {
    let mut bytes = [0; 8];

    self.read_buf_exact(&mut bytes)?;

    Ok(u64::from_be_bytes(bytes))
  }

  #[inline(always)]
  fn read_f32(&mut self) -> io::Result<f32> {
    self.read_quad_bytes().map(f32::from_le_bytes)
  }

  #[inline(always)]
  fn read_be_f32(&mut self) -> io::Result<f32> {
    self.read_quad_bytes().map(f32::from_be_bytes)
  }

  #[inline(always)]
  fn read_f64(&mut self) -> io::Result<f64> {
    let mut bytes = [0; 8];

    self.read_buf_exact(&mut bytes)?;

    Ok(f64::from_le_bytes(bytes))
  }

  #[inline(always)]
  fn read_be_f64(&mut self) -> io::Result<f64> {
    let mut bytes = [0; 8];

    self.read_buf_exact(&mut bytes)?;

    Ok(f64::from_be_bytes(bytes))
  }

  fn read_boxed_slice_exact(&mut self, len: u64) -> io::Result<Box<[u8]>> {
    let bounded = len.min(MAX_BOXED_READ_LEN);

    match bounded.cmp(&len) {
      Ordering::Equal => {}
      Ordering::Less | Ordering::Greater => {
        return Err(io::Error::new(
          io::ErrorKind::InvalidInput,
          "boxed read exceeds its byte limit",
        ));
      }
    }

    let Ok(size) = index(bounded);
    let mut bytes = Vec::new();

    #[cfg_attr(
      dylint_lib = "explicit088_no_arriving_length",
      expect(
        explicit088_no_arriving_length,
        reason = "size comes from the MAX_BOXED_READ_LEN check above through the exact index conversion"
      )
    )]
    let reserved = bytes.try_reserve_exact(size);

    reserved.map_err(|fault| io::Error::new(io::ErrorKind::OutOfMemory, fault))?;

    bytes.extend((0..bounded).map(|_| 0));
    self.read_buf_exact(&mut bytes)?;

    Ok(bytes.into_boxed_slice())
  }

  fn ignore_bytes(&mut self, count: u64) -> io::Result<()>;

  fn pos(&self) -> u64;
}

impl<R: ReadBytes> ReadBytes for &mut R {
  #[inline(always)]
  fn read_byte(&mut self) -> io::Result<u8> {
    (*self).read_byte()
  }

  #[inline(always)]
  fn read_double_bytes(&mut self) -> io::Result<[u8; 2]> {
    (*self).read_double_bytes()
  }

  #[inline(always)]
  fn read_triple_bytes(&mut self) -> io::Result<[u8; 3]> {
    (*self).read_triple_bytes()
  }

  #[inline(always)]
  fn read_quad_bytes(&mut self) -> io::Result<[u8; 4]> {
    (*self).read_quad_bytes()
  }

  #[inline(always)]
  fn read_buf(&mut self, buf: &mut [u8]) -> io::Result<u64> {
    (*self).read_buf(buf)
  }

  #[inline(always)]
  fn read_buf_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
    (*self).read_buf_exact(buf)
  }

  #[inline(always)]
  fn ignore_bytes(&mut self, count: u64) -> io::Result<()> {
    (*self).ignore_bytes(count)
  }

  #[inline(always)]
  fn pos(&self) -> u64 {
    (**self).pos()
  }
}

pub trait SeekBuffered {
  fn ensure_seekback_buffer(&mut self, len: u64);

  fn unread_buffer_len(&self) -> u64;

  fn read_buffer_len(&self) -> u64;

  fn seek_buffered(&mut self, pos: u64) -> u64;

  fn seek_buffered_rel(&mut self, delta: i64) -> u64;

  fn seek_buffered_rev(&mut self, delta: u64) -> Result<(), Never> {
    let Ok(back) = fitted::<u64, i64>(delta);
    let _position = self.seek_buffered_rel(back.saturating_neg());

    Ok(())
  }
}

impl<S: SeekBuffered> SeekBuffered for &mut S {
  fn ensure_seekback_buffer(&mut self, len: u64) {
    (*self).ensure_seekback_buffer(len)
  }

  fn unread_buffer_len(&self) -> u64 {
    (**self).unread_buffer_len()
  }

  fn read_buffer_len(&self) -> u64 {
    (**self).read_buffer_len()
  }

  fn seek_buffered(&mut self, pos: u64) -> u64 {
    (*self).seek_buffered(pos)
  }

  fn seek_buffered_rel(&mut self, delta: i64) -> u64 {
    (*self).seek_buffered_rel(delta)
  }
}

pub trait FiniteStream {
  fn byte_len(&self) -> u64;

  fn bytes_available(&self) -> u64;
}

impl<F: FiniteStream> FiniteStream for &mut F {
  fn byte_len(&self) -> u64 {
    (**self).byte_len()
  }

  fn bytes_available(&self) -> u64 {
    (**self).bytes_available()
  }
}

fn finished<T>(stepped: Result<io::Result<T>, Endless>) -> io::Result<T> {
  match stepped {
    Ok(ended) => ended,
    Err(endless) => Err(io::Error::other(endless)),
  }
}

fn ahead(bytes: &[u8], count: u64) -> Result<(&[u8], &[u8]), Never> {
  let Ok(at) = index(count);

  Ok(match bytes.split_at_checked(at) {
    Some(split) => split,
    None => (bytes, Default::default()),
  })
}

fn copied(from: &[u8], into: &mut [u8]) -> Result<u64, Never> {
  let Ok(count) = fitted::<_, u64>(from.len().min(into.len()));

  for (slot, byte) in into.iter_mut().zip(from) {
    *slot = *byte;
  }

  Ok(count)
}

fn counted<Count>(read: io::Result<Count>) -> io::Result<u64>
where
  Count: Ends + PartialOrd + Copy,
  u64: TryFrom<Count>,
{
  read.map(|count| {
    let Ok(wide) = fitted::<Count, u64>(count);

    wide
  })
}
