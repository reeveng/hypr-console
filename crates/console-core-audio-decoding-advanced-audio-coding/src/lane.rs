//! Asking a list for a position the stream gave, and what the answer is when
//! the stream was wrong.
//!
//! Every index in this decoder comes out of the bitstream or out of arithmetic
//! on something that did, and a malformed stream is exactly the one that makes
//! it point past the end. Indexing there is a panic in a decoder that is handed
//! whatever a file contains, so a position is asked of the list instead and the
//! answer for one that is not there is the decode fault every other malformed
//! field already is. The width is the one the stream wrote -- `u32` -- and the
//! single place it meets the standard library's is `console-core-number-conversion`.

use std::io;
use std::ops::Range;

use console_core_audio_decoding_interfaces::errors::{Result, decode_error};
use console_core_byte_streams::ReadBitsLtr;
use console_core_frequency_transforms::TransformError;
use console_core_never::Never;
use console_core_number_conversion::index;

pub(crate) type Sure<T> = std::result::Result<T, Never>;

pub(crate) fn invalid<T>() -> Result<T> {
  decode_error("aac: invalid data")
}

pub(crate) fn transformed<T>(outcome: std::result::Result<T, TransformError>) -> Result<T> {
  match outcome {
    Ok(done) => Ok(done),
    Err(TransformError::Size) => decode_error("aac: no transform of that size"),
    Err(TransformError::Spectrum) => decode_error("aac: spectrum does not fit the transform"),
    Err(TransformError::Output) => decode_error("aac: output does not fit the transform"),
  }
}

pub(crate) enum Constraint {
  Satisfied,
  Violated,
}

impl From<bool> for Constraint {
  fn from(satisfied: bool) -> Self {
    match bool::then_some(satisfied, ()) {
      Some(()) => Self::Satisfied,
      None => Self::Violated,
    }
  }
}

impl Constraint {
  pub(crate) fn require(self) -> Result<()> {
    match self {
      Self::Satisfied => Ok(()),
      Self::Violated => invalid(),
    }
  }
}

pub(crate) trait Lane<T> {
  fn at(&self, position: u32) -> Result<T>
  where
    T: Copy;

  fn put(&mut self, position: u32, value: T) -> Result<()>;

  fn borrowed(&self, position: u32) -> Result<&T>;

  fn borrowed_mut(&mut self, position: u32) -> Result<&mut T>;

  fn between(&self, bounds: Range<u32>) -> Result<&[T]>;

  fn between_mut(&mut self, bounds: Range<u32>) -> Result<&mut [T]>;
}

impl<T> Lane<T> for [T] {
  fn at(&self, position: u32) -> Result<T>
  where
    T: Copy,
  {
    let Ok(at) = index(position);

    match self.get(at) {
      Some(held) => Ok(*held),
      None => invalid(),
    }
  }

  fn put(&mut self, position: u32, value: T) -> Result<()> {
    let Ok(at) = index(position);

    match self.get_mut(at) {
      Some(held) => {
        *held = value;

        Ok(())
      }
      None => invalid(),
    }
  }

  fn borrowed(&self, position: u32) -> Result<&T> {
    let Ok(at) = index(position);

    match self.get(at) {
      Some(held) => Ok(held),
      None => invalid(),
    }
  }

  fn borrowed_mut(&mut self, position: u32) -> Result<&mut T> {
    let Ok(at) = index(position);

    match self.get_mut(at) {
      Some(held) => Ok(held),
      None => invalid(),
    }
  }

  fn between(&self, bounds: Range<u32>) -> Result<&[T]> {
    let Ok(start) = index(bounds.start);
    let Ok(end) = index(bounds.end);

    match self.get(start..end) {
      Some(held) => Ok(held),
      None => invalid(),
    }
  }

  fn between_mut(&mut self, bounds: Range<u32>) -> Result<&mut [T]> {
    let Ok(start) = index(bounds.start);
    let Ok(end) = index(bounds.end);

    match self.get_mut(start..end) {
      Some(held) => Ok(held),
      None => invalid(),
    }
  }
}

pub(crate) trait Grid<T> {
  fn cell(&self, position: (u32, u32)) -> Result<T>
  where
    T: Copy;

  fn put_cell(&mut self, position: (u32, u32), value: T) -> Result<()>;
}

impl<T, const N: usize> Grid<T> for [[T; N]] {
  fn cell(&self, position: (u32, u32)) -> Result<T>
  where
    T: Copy,
  {
    let (row, column) = position;
    let Ok(at) = index(row);

    match self.get(at) {
      Some(line) => line.at(column),
      None => invalid(),
    }
  }

  fn put_cell(&mut self, position: (u32, u32), value: T) -> Result<()> {
    let (row, column) = position;
    let Ok(at) = index(row);

    match self.get_mut(at) {
      Some(line) => line.put(column, value),
      None => invalid(),
    }
  }
}

pub(crate) struct Fields<'a, B> {
  source: &'a mut B,
  width: u32,
}

impl<'a, B: ReadBitsLtr> Fields<'a, B> {
  pub(crate) fn of(source: &'a mut B, width: u32) -> Sure<Self> {
    Ok(Self { source, width })
  }
}

impl<B: ReadBitsLtr> Iterator for Fields<'_, B> {
  type Item = io::Result<u32>;

  fn next(&mut self) -> Option<Self::Item> {
    Some(self.source.read_bits_leq32(self.width))
  }
}
