//! A source read through a ring of its own, so a format can look back at what it has read.
//!
//! The ring is a power of two long and every position into it is a `u64` masked by its length,
//! so wrapping round is a bitwise and rather than a comparison. Reads from the source start at a
//! kilobyte and double up to thirty-two, which keeps a seek from throwing away much that was
//! read ahead of it. What has been read stays in the ring until a later read overwrites it, and
//! a buffered seek moves within that rather than asking the source; a real seek empties it.
//!
//! A ring asked for at a length that is not a power of two above the largest block is made the
//! next one that is, where upstream stopped the program.
//! A prefix read stops at its byte count or at end of input and leaves other buffered bytes
//! unread. Each refill uses the source's own I/O deadlines.

use std::cmp::Ordering;
use std::io::{self, IoSliceMut, Read, Seek};

use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use super::{MediaSource, ReadBytes, SeekBuffered, Seeking, copied, counted, finished};

const MIN_BLOCK_LEN: u64 = 1024;
const MAX_BLOCK_LEN: u64 = 32_768;
const DEFAULT_BUFFER_LEN: u64 = 65_536;
const MAX_RING_LEN: u64 = 32 * 1024 * 1024;
const PREFIX_RETRIES: u64 = 32;

enum RingContents {
  Empty,
  Buffered,
}

impl From<bool> for RingContents {
  fn from(empty: bool) -> Self {
    match bool::then_some(empty, ()) {
      Some(()) => Self::Empty,
      None => Self::Buffered,
    }
  }
}

fn end_of_stream<T>() -> io::Result<T> {
  Err(io::Error::new(
    io::ErrorKind::UnexpectedEof,
    "end of stream",
  ))
}

pub struct MediaSourceStreamOptions {
  pub buffer_len: u64,
}

impl Default for MediaSourceStreamOptions {
  fn default() -> Self {
    MediaSourceStreamOptions {
      buffer_len: DEFAULT_BUFFER_LEN,
    }
  }
}

pub struct MediaSourceStream {
  inner: Box<dyn MediaSource>,
  ring: Box<[u8]>,
  ring_mask: u64,
  read_pos: u64,
  write_pos: u64,
  read_block_len: u64,
  abs_pos: u64,
  rel_pos: u64,
}

impl MediaSourceStream {
  pub fn new(
    source: Box<dyn MediaSource>,
    options: MediaSourceStreamOptions,
  ) -> Result<Self, Never> {
    let requested = options.buffer_len.clamp(DEFAULT_BUFFER_LEN, MAX_RING_LEN);
    let length = match requested.checked_next_power_of_two() {
      Some(length) => length,
      None => DEFAULT_BUFFER_LEN,
    };

    Ok(MediaSourceStream {
      inner: source,
      ring: (0..length).map(|_| 0).collect(),
      ring_mask: length.wrapping_sub(1),
      read_pos: 0,
      write_pos: 0,
      read_block_len: MIN_BLOCK_LEN,
      abs_pos: 0,
      rel_pos: 0,
    })
  }

  pub fn read_prefix(&mut self, limit: u64) -> io::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut remaining = limit;
    let mut retries = PREFIX_RETRIES;

    for _ in 0..limit.saturating_add(PREFIX_RETRIES) {
      match remaining {
        0 => break,
        _ => {}
      }

      match self.fetch() {
        Ok(()) => {}
        Err(fault) => match (fault.kind(), retries.checked_sub(1)) {
          (io::ErrorKind::Interrupted, Some(left)) => {
            retries = left;

            continue;
          }
          (_kind, _remaining) => return Err(fault),
        },
      }

      let Ok(ready) = self.contiguous();
      let Ok(available) = fitted::<_, u64>(ready.len());
      let taken = available.min(remaining);
      let Ok(end) = index(taken);
      let prefix = ready.get(..end).ok_or(io::ErrorKind::InvalidData)?;

      match prefix {
        [] => break,
        [_first, ..] => out.extend_from_slice(prefix),
      }

      let Ok(()) = self.consume(taken);

      remaining = remaining.saturating_sub(taken);
    }

    Ok(out)
  }

  fn ring_len(&self) -> Result<u64, Never> {
    Ok(self.ring_mask.wrapping_add(1))
  }

  fn fetch(&mut self) -> io::Result<()> {
    match RingContents::from(self.read_pos == self.write_pos) {
      RingContents::Empty => self.refill(),
      RingContents::Buffered => Ok(()),
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "the MediaSource supplied by the caller owns any I/O deadline, and refill reads at most one bounded block"
    )
  )]
  fn refill(&mut self) -> io::Result<()> {
    let Ok(at) = index(self.write_pos);
    let Ok(block) = index(self.read_block_len);

    let read = match self.ring.split_at_mut_checked(at) {
      Some((wrapped, ahead)) => {
        let Ok(room) = fitted::<_, u64>(ahead.len());
        let Ok(rest) = index(self.read_block_len.saturating_sub(room));

        match ahead.len().cmp(&block) {
          Ordering::Greater | Ordering::Equal => match ahead.get_mut(..block) {
            Some(whole) => counted(self.inner.read(whole)),
            None => end_of_stream(),
          },
          Ordering::Less => match wrapped.get_mut(..rest) {
            Some(start) => counted(
              self
                .inner
                .read_vectored(&mut [IoSliceMut::new(ahead), IoSliceMut::new(start)]),
            ),
            None => end_of_stream(),
          },
        }
      }
      None => end_of_stream(),
    };

    let count = read?;

    self.write_pos = self.write_pos.wrapping_add(count) & self.ring_mask;
    self.abs_pos = self.abs_pos.saturating_add(count);
    self.rel_pos = self.rel_pos.saturating_add(count);
    self.read_block_len = self.read_block_len.wrapping_shl(1).min(MAX_BLOCK_LEN);

    Ok(())
  }

  fn fetch_or_eof(&mut self) -> io::Result<()> {
    self.fetch()?;

    match RingContents::from(self.read_pos == self.write_pos) {
      RingContents::Empty => end_of_stream(),
      RingContents::Buffered => Ok(()),
    }
  }

  fn consume(&mut self, len: u64) -> Result<(), Never> {
    self.read_pos = self.read_pos.wrapping_add(len) & self.ring_mask;

    Ok(())
  }

  fn contiguous(&self) -> Result<&[u8], Never> {
    let Ok(from) = index(self.read_pos);
    let Ok(to) = index(self.write_pos);

    let readable = match self.write_pos.cmp(&self.read_pos) {
      Ordering::Greater | Ordering::Equal => self.ring.get(from..to),
      Ordering::Less => self.ring.get(from..),
    };

    Ok(match readable {
      Some(bytes) => bytes,
      None => Default::default(),
    })
  }

  fn reset(&mut self, pos: u64) -> Result<u64, Never> {
    self.read_pos = 0;
    self.write_pos = 0;
    self.read_block_len = MIN_BLOCK_LEN;
    self.abs_pos = pos;
    self.rel_pos = 0;

    Ok(pos)
  }

  fn regrown(&mut self, requested: u64) -> Result<(), Never> {
    let length = requested.min(MAX_RING_LEN);
    let Ok(from) = index(self.read_pos);
    let Ok(unread) = index(self.unread_buffer_len());
    let mut ring: Box<[u8]> = (0..length).map(|_| 0).collect();

    let wrapped = self
      .ring
      .get(from..)
      .into_iter()
      .flatten()
      .chain(self.ring.iter());

    for (slot, byte) in ring.iter_mut().zip(wrapped.take(unread)) {
      *slot = *byte;
    }

    self.write_pos = self.unread_buffer_len();
    self.ring = ring;
    self.ring_mask = length.wrapping_sub(1);
    self.read_pos = 0;

    Ok(())
  }

  fn taken<const WIDTH: usize>(&mut self) -> io::Result<[u8; WIDTH]> {
    let Ok(readable) = self.contiguous();

    match readable.first_chunk::<WIDTH>().copied() {
      Some(bytes) => {
        let Ok(width) = fitted::<_, u64>(WIDTH);
        let Ok(()) = self.consume(width);

        Ok(bytes)
      }
      None => {
        let mut bytes = [0; WIDTH];

        for slot in bytes.iter_mut() {
          let byte = self.read_byte()?;

          *slot = byte;
        }

        Ok(bytes)
      }
    }
  }

  fn skipped_by_seeking(&mut self, count: u64) -> io::Result<u64> {
    let Ok(ring_len) = self.ring_len();
    let Ok(kept) = fitted::<u64, i64>(ring_len);
    let far = ring_len.saturating_mul(2);

    let stepped = iterate((&mut *self, count), |(stream, left)| {
      Ok(match left.cmp(&far) {
        Ordering::Greater | Ordering::Equal => match stream.seeking() {
          Seeking::Anywhere => {
            let Ok(most) = fitted::<u64, i64>(left);
            let delta = most.saturating_sub(kept);

            match stream.seek(io::SeekFrom::Current(delta)) {
              Ok(_position) => Step::Again((stream, left.saturating_sub(delta.unsigned_abs()))),
              Err(fault) => Step::Halt(Err(fault)),
            }
          }
          Seeking::ForwardOnly => Step::Halt(Ok(left)),
        },
        Ordering::Less => Step::Halt(Ok(left)),
      })
    });

    finished(stepped)
  }

  fn discarded(&mut self, count: u64) -> io::Result<()> {
    let stepped = iterate((&mut *self, count), |(stream, left)| {
      Ok(match left {
        0 => Step::Halt(Ok(())),
        1.. => match stream.fetch_or_eof() {
          Ok(()) => {
            let discard = stream.unread_buffer_len().min(left);
            let Ok(()) = stream.consume(discard);

            Step::Again((stream, left.saturating_sub(discard)))
          }
          Err(fault) => Step::Halt(Err(fault)),
        },
      })
    });

    finished(stepped)
  }
}

impl MediaSource for MediaSourceStream {
  #[inline]
  fn seeking(&self) -> Seeking {
    self.inner.seeking()
  }

  #[inline]
  fn byte_len(&self) -> Option<u64> {
    self.inner.byte_len()
  }
}

impl io::Read for MediaSourceStream {
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    allow(
      explicit051_no_machine_width,
      reason = "std::io::Read requires a usize byte count; the stream's positions are held as u64"
    )
  )]
  fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    let stepped = iterate((&mut *self, buf, 0_u64), |(stream, rest, filled)| {
      Ok(match rest {
        [] => Step::Halt(Ok(filled)),
        [_, ..] => match stream.fetch() {
          Ok(()) => {
            let Ok(readable) = stream.contiguous();
            let Ok(count) = copied(readable, rest);
            let Ok(at) = index(count);
            let Ok(()) = stream.consume(count);

            match count {
              0 => Step::Halt(Ok(filled)),
              _ => match rest.split_at_mut_checked(at) {
                Some((_copied, after)) => {
                  Step::Again((stream, after, filled.saturating_add(count)))
                }
                None => Step::Halt(Ok(filled)),
              },
            }
          }
          Err(fault) => Step::Halt(Err(fault)),
        },
      })
    });

    let filled = finished(stepped)?;
    let Ok(count) = index(filled);

    Ok(count)
  }
}

impl io::Seek for MediaSourceStream {
  fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
    match pos {
      io::SeekFrom::Current(0) => Ok(self.pos()),
      io::SeekFrom::Current(delta) => {
        let Ok(unread) = fitted::<u64, i64>(self.unread_buffer_len());
        let moved = self
          .inner
          .seek(io::SeekFrom::Current(delta.saturating_sub(unread)))?;
        let Ok(at) = self.reset(moved);

        Ok(at)
      }
      io::SeekFrom::Start(_) | io::SeekFrom::End(_) => {
        let moved = self.inner.seek(pos)?;
        let Ok(at) = self.reset(moved);

        Ok(at)
      }
    }
  }
}

impl ReadBytes for MediaSourceStream {
  #[inline(always)]
  fn read_byte(&mut self) -> io::Result<u8> {
    self.fetch_or_eof()?;

    let Ok(at) = index(self.read_pos);

    match self.ring.get(at).copied() {
      Some(byte) => {
        let Ok(()) = self.consume(1);

        Ok(byte)
      }
      None => end_of_stream(),
    }
  }

  fn read_double_bytes(&mut self) -> io::Result<[u8; 2]> {
    self.taken::<2>()
  }

  fn read_triple_bytes(&mut self) -> io::Result<[u8; 3]> {
    self.taken::<3>()
  }

  fn read_quad_bytes(&mut self) -> io::Result<[u8; 4]> {
    self.taken::<4>()
  }

  fn read_buf(&mut self, buf: &mut [u8]) -> io::Result<u64> {
    let count = counted(io::Read::read(self, buf))?;

    match (count, buf) {
      (0, [_, ..]) => end_of_stream(),
      (0, []) | (1.., _) => Ok(count),
    }
  }

  fn read_buf_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
    let stepped = iterate((&mut *self, buf), |(stream, rest)| {
      Ok(match rest {
        [] => Step::Halt(Ok(())),
        [_, ..] => match counted(io::Read::read(stream, rest)) {
          Ok(0) => Step::Halt(end_of_stream()),
          Ok(count) => {
            let Ok(at) = index(count);

            match rest.split_at_mut_checked(at) {
              Some((_filled, after)) => Step::Again((stream, after)),
              None => Step::Halt(end_of_stream()),
            }
          }
          Err(fault) => match bool::then_some(fault.kind() == io::ErrorKind::Interrupted, ()) {
            Some(()) => Step::Again((stream, rest)),
            None => Step::Halt(Err(fault)),
          },
        },
      })
    });

    finished(stepped)
  }

  fn ignore_bytes(&mut self, count: u64) -> io::Result<()> {
    let left = self.skipped_by_seeking(count)?;

    self.discarded(left)
  }

  fn pos(&self) -> u64 {
    self.abs_pos.saturating_sub(self.unread_buffer_len())
  }
}

impl SeekBuffered for MediaSourceStream {
  fn ensure_seekback_buffer(&mut self, len: u64) {
    let Ok(ring_len) = self.ring_len();
    let retained = len.min(MAX_RING_LEN.saturating_sub(MAX_BLOCK_LEN));

    match MAX_BLOCK_LEN
      .saturating_add(retained)
      .checked_next_power_of_two()
    {
      Some(length) => match ring_len.cmp(&length) {
        Ordering::Less => {
          let Ok(()) = self.regrown(length);
        }
        Ordering::Equal | Ordering::Greater => (),
      },
      None => (),
    }
  }

  fn unread_buffer_len(&self) -> u64 {
    self.write_pos.wrapping_sub(self.read_pos) & self.ring_mask
  }

  fn read_buffer_len(&self) -> u64 {
    let Ok(ring_len) = self.ring_len();

    ring_len
      .min(self.rel_pos)
      .saturating_sub(self.unread_buffer_len())
  }

  fn seek_buffered(&mut self, pos: u64) -> u64 {
    let old = self.pos();
    let Ok(ahead) = fitted::<u64, i64>(pos.saturating_sub(old));
    let Ok(behind) = fitted::<u64, i64>(old.saturating_sub(pos));

    self.seek_buffered_rel(ahead.saturating_sub(behind))
  }

  fn seek_buffered_rel(&mut self, delta: i64) -> u64 {
    let back = delta.min(0).unsigned_abs().min(self.read_buffer_len());
    let forward = delta.max(0).unsigned_abs().min(self.unread_buffer_len());

    self.read_pos = self.read_pos.wrapping_add(forward).wrapping_sub(back) & self.ring_mask;

    self.pos()
  }
}
