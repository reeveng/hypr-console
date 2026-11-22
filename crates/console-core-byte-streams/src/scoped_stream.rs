//! A stream that may read no further than a length given when it was made.
//!
//! A buffered seek is held inside the scope as well, but moves the inner stream only; what the
//! scope counts as read is what was read through it, which is how upstream left it.

use std::{cmp::Ordering, io};

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use super::{FiniteStream, ReadBytes, SeekBuffered};

fn out_of_bounds<T>() -> io::Result<T> {
  Err(io::Error::new(
    io::ErrorKind::UnexpectedEof,
    "out of bounds",
  ))
}

pub struct ScopedStream<B: ReadBytes> {
  inner: B,
  start: u64,
  len: u64,
  read: u64,
}

impl<B: ReadBytes> ScopedStream<B> {
  pub fn new(inner: B, len: u64) -> Result<Self, Never> {
    Ok(ScopedStream {
      start: inner.pos(),
      inner,
      len,
      read: 0,
    })
  }

  pub fn ignore(&mut self) -> io::Result<()> {
    self.inner.ignore_bytes(self.len.saturating_sub(self.read))
  }

  pub fn into_inner(self) -> Result<B, Never> {
    Ok(self.inner)
  }

  fn claimed(&mut self, count: u64) -> io::Result<()> {
    match self.len.saturating_sub(self.read).cmp(&count) {
      Ordering::Less => out_of_bounds(),
      Ordering::Equal | Ordering::Greater => {
        self.read = self.read.saturating_add(count);

        Ok(())
      }
    }
  }
}

impl<B: ReadBytes> FiniteStream for ScopedStream<B> {
  fn byte_len(&self) -> u64 {
    self.len
  }

  fn bytes_available(&self) -> u64 {
    self.len.saturating_sub(self.read)
  }
}

impl<B: ReadBytes> ReadBytes for ScopedStream<B> {
  #[inline(always)]
  fn read_byte(&mut self) -> io::Result<u8> {
    self.claimed(1)?;

    self.inner.read_byte()
  }

  #[inline(always)]
  fn read_double_bytes(&mut self) -> io::Result<[u8; 2]> {
    self.claimed(2)?;

    self.inner.read_double_bytes()
  }

  #[inline(always)]
  fn read_triple_bytes(&mut self) -> io::Result<[u8; 3]> {
    self.claimed(3)?;

    self.inner.read_triple_bytes()
  }

  #[inline(always)]
  fn read_quad_bytes(&mut self) -> io::Result<[u8; 4]> {
    self.claimed(4)?;

    self.inner.read_quad_bytes()
  }

  fn read_buf(&mut self, buf: &mut [u8]) -> io::Result<u64> {
    let Ok(room) = fitted::<_, u64>(buf.len());
    let Ok(most) = index(room.min(self.len.saturating_sub(self.read)));

    match buf.get_mut(..most) {
      Some(part) => {
        let count = self.inner.read_buf(part)?;

        self.read = self.read.saturating_add(count);

        Ok(count)
      }
      None => out_of_bounds(),
    }
  }

  fn read_buf_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
    let Ok(wanted) = fitted::<_, u64>(buf.len());

    self.claimed(wanted)?;

    self.inner.read_buf_exact(buf)
  }

  #[inline(always)]
  fn ignore_bytes(&mut self, count: u64) -> io::Result<()> {
    self.claimed(count)?;

    self.inner.ignore_bytes(count)
  }

  #[inline(always)]
  fn pos(&self) -> u64 {
    self.inner.pos()
  }
}

impl<B: ReadBytes + SeekBuffered> SeekBuffered for ScopedStream<B> {
  #[inline(always)]
  fn ensure_seekback_buffer(&mut self, len: u64) {
    self.inner.ensure_seekback_buffer(len)
  }

  #[inline(always)]
  fn unread_buffer_len(&self) -> u64 {
    self
      .inner
      .unread_buffer_len()
      .min(self.len.saturating_sub(self.read))
  }

  #[inline(always)]
  fn read_buffer_len(&self) -> u64 {
    self.inner.read_buffer_len().min(self.read)
  }

  #[inline(always)]
  fn seek_buffered(&mut self, pos: u64) -> u64 {
    self
      .inner
      .seek_buffered(pos.max(self.start).min(self.start.saturating_add(self.len)))
  }

  #[inline(always)]
  fn seek_buffered_rel(&mut self, delta: i64) -> u64 {
    let Ok(back) = fitted::<u64, i64>(self.read);
    let Ok(forward) = fitted::<u64, i64>(self.len.saturating_sub(self.read));

    self
      .inner
      .seek_buffered_rel(delta.max(back.saturating_neg()).min(forward))
  }
}
