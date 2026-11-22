//! A byte slice read as a stream, each read cutting what it took off the front of what is left.
//!
//! The reader holds the slice it began with and the slice still ahead of it, so a read is a
//! split of the second and the position is the difference of their lengths. Upstream kept an
//! index into the slice instead, which every read had to check against the end by hand.

use std::{cmp::Ordering, io};

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use super::{FiniteStream, ReadBytes, ahead, copied};

fn underrun<T>() -> io::Result<T> {
  Err(io::Error::new(
    io::ErrorKind::UnexpectedEof,
    "buffer underrun",
  ))
}

pub struct BufReader<'a> {
  whole: &'a [u8],
  rest: &'a [u8],
}

impl<'a> BufReader<'a> {
  pub fn new(buf: &'a [u8]) -> Result<Self, Never> {
    Ok(BufReader {
      whole: buf,
      rest: buf,
    })
  }

  pub fn scan_bytes_aligned_ref(
    &mut self,
    pattern: &[u8],
    align: u8,
    scan_len: u64,
  ) -> io::Result<&'a [u8]> {
    let Ok((region, _beyond)) = ahead(self.rest, scan_len);
    let Ok(step) = index(align);

    let scanned = match region.len().cmp(&pattern.len()) {
      Ordering::Less => self.rest.split_at_checked(region.len()),
      Ordering::Equal | Ordering::Greater => (pattern.len()..=region.len().saturating_add(step.max(1)))
        .step_by(step.max(1))
        .find(|end| {
          *end >= region.len()
            || region.get(end.saturating_sub(pattern.len())..*end) == Some(pattern)
        })
        .and_then(|end| self.rest.split_at_checked(end.min(self.rest.len()))),
    };

    match scanned {
      Some((taken, rest)) => {
        self.rest = rest;

        Ok(taken)
      }
      None => underrun(),
    }
  }

  pub fn read_buf_bytes_ref(&mut self, len: u64) -> io::Result<&'a [u8]> {
    let Ok(at) = index(len);

    match self.rest.split_at_checked(at) {
      Some((taken, rest)) => {
        self.rest = rest;

        Ok(taken)
      }
      None => underrun(),
    }
  }

  pub fn read_buf_bytes_available_ref(&mut self) -> Result<&'a [u8], Never> {
    Ok(std::mem::take(&mut self.rest))
  }

  fn taken<const WIDTH: usize>(&mut self) -> io::Result<[u8; WIDTH]> {
    match self.rest.split_first_chunk::<WIDTH>() {
      Some((bytes, rest)) => {
        self.rest = rest;

        Ok(*bytes)
      }
      None => underrun(),
    }
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
    let Ok((taken, rest)) = ahead(self.rest, room);
    let Ok(count) = copied(taken, buf);

    self.rest = rest;

    Ok(count)
  }

  fn read_buf_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
    match self.rest.split_at_checked(buf.len()) {
      Some((taken, rest)) => {
        let Ok(_count) = copied(taken, buf);

        self.rest = rest;

        Ok(())
      }
      None => underrun(),
    }
  }

  fn ignore_bytes(&mut self, count: u64) -> io::Result<()> {
    let Ok(at) = index(count);

    match self.rest.get(at..) {
      Some(rest) => {
        self.rest = rest;

        Ok(())
      }
      None => underrun(),
    }
  }

  #[inline(always)]
  fn pos(&self) -> u64 {
    let Ok(position) = fitted::<_, u64>(self.whole.len().saturating_sub(self.rest.len()));

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
    let Ok(length) = fitted::<_, u64>(self.rest.len());

    length
  }
}
