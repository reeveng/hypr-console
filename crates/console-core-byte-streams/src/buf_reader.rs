//! A byte slice read as a stream, each read cutting what it took off the front of what is left.
//!
//! The reader holds the slice it began with and the slice still ahead of it, so a read is a
//! split of the second and the position is the difference of their lengths. Upstream kept an
//! index into the slice instead, which every read had to check against the end by hand.

use std::{cmp::Ordering, io};

use console_core_bytes::{Cursor, Truncated};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use super::{FiniteStream, ReadBytes, ahead, copied};

fn stream_read<T>(outcome: Result<T, Truncated>) -> io::Result<T> {
  match outcome {
    Ok(value) => Ok(value),
    Err(Truncated) => Err(io::Error::new(io::ErrorKind::UnexpectedEof, "buffer underrun")),
  }
}

pub struct BufReader<'a> {
  whole: &'a [u8],
  cursor: Cursor<'a>,
}

impl<'a> BufReader<'a> {
  pub fn new(buf: &'a [u8]) -> Result<Self, Never> {
    let Ok(cursor) = Cursor::new(buf);

    Ok(BufReader { whole: buf, cursor })
  }

  pub fn scan_bytes_aligned_ref(
    &mut self,
    pattern: &[u8],
    align: u8,
    scan_len: u64,
  ) -> io::Result<&'a [u8]> {
    let Ok(rest) = self.cursor.rest();
    let Ok((region, _beyond)) = ahead(rest, scan_len);
    let Ok(step) = index(align);

    let scanned = match region.len().cmp(&pattern.len()) {
      Ordering::Less => rest.split_at_checked(region.len()),
      Ordering::Equal | Ordering::Greater => (pattern.len()
        ..=region.len().saturating_add(step.max(1)))
        .step_by(step.max(1))
        .find(|end| {
          *end >= region.len()
            || region.get(end.saturating_sub(pattern.len())..*end) == Some(pattern)
        })
        .and_then(|end| rest.split_at_checked(end.min(rest.len()))),
    };

    let (taken, remaining) = stream_read(scanned.ok_or(Truncated))?;
    let Ok(cursor) = Cursor::new(remaining);
    self.cursor = cursor;

    Ok(taken)
  }

  pub fn read_buf_bytes_ref(&mut self, len: u64) -> io::Result<&'a [u8]> {
    stream_read(self.cursor.take(len))
  }

  pub fn read_buf_bytes_available_ref(&mut self) -> Result<&'a [u8], Never> {
    self.cursor.take_rest()
  }

  fn taken<const WIDTH: usize>(&mut self) -> io::Result<[u8; WIDTH]> {
    stream_read(self.cursor.array())
  }
}

impl ReadBytes for BufReader<'_> {
  #[inline(always)]
  fn read_byte(&mut self) -> io::Result<u8> {
    self.taken::<1>().map(|[byte]| byte)
  }

  #[inline(always)]
  fn read_double_bytes(&mut self) -> io::Result<[u8; 2]> {
    self.taken::<2>()
  }

  #[inline(always)]
  fn read_triple_bytes(&mut self) -> io::Result<[u8; 3]> {
    self.taken::<3>()
  }

  #[inline(always)]
  fn read_quad_bytes(&mut self) -> io::Result<[u8; 4]> {
    self.taken::<4>()
  }

  fn read_buf(&mut self, buf: &mut [u8]) -> io::Result<u64> {
    let Ok(room) = fitted::<_, u64>(buf.len());
    let Ok(rest) = self.cursor.rest();
    let Ok((taken, remaining)) = ahead(rest, room);
    let Ok(count) = copied(taken, buf);
    let Ok(cursor) = Cursor::new(remaining);
    self.cursor = cursor;

    Ok(count)
  }

  fn read_buf_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
    let Ok(count) = fitted::<_, u64>(buf.len());
    let taken = self.read_buf_bytes_ref(count)?;
    let Ok(_copied) = copied(taken, buf);

    Ok(())
  }

  fn ignore_bytes(&mut self, count: u64) -> io::Result<()> {
    let _ignored = self.read_buf_bytes_ref(count)?;

    Ok(())
  }

  #[inline(always)]
  fn pos(&self) -> u64 {
    let Ok(rest) = self.cursor.rest();
    let Ok(position) = fitted::<_, u64>(self.whole.len().saturating_sub(rest.len()));

    position
  }
}

impl FiniteStream for BufReader<'_> {
  #[inline(always)]
  fn byte_len(&self) -> u64 {
    let Ok(length) = fitted::<_, u64>(self.whole.len());

    length
  }

  #[inline(always)]
  fn bytes_available(&self) -> u64 {
    let Ok(rest) = self.cursor.rest();
    let Ok(length) = fitted::<_, u64>(rest.len());

    length
  }
}
