//! What a stream's numbers pass through before anything is done with them.
//!
//! A Vorbis header is a run of fields of the width the format gives them, and
//! the decoder holds each at the width it has. Reading one is the stream's
//! bits, said as a `u8` or a `u16`, and a position the stream names in a list
//! is a position that may not be there: it comes back as the fault a malformed
//! stream is rather than as a panic, which is what indexing it would have
//! been. Upstream read every field into a `u32` and cut it down with `as`; the
//! widths are asked here, so a field that is wider than the place it is going
//! is a number that saturates instead of one that wraps into a different
//! valid value.
//!
//! A stream that ends in the middle of a packet is not a stream that is wrong.
//! Vorbis lets an encoder drop the end of a packet whose remaining bits would
//! all have been zero, so a floor or a residue that runs out of bits has
//! decoded what it was given, and `tolerate_end` is where that is told apart
//! from a fault.

use std::io::ErrorKind;

use console_core_audio_decoding_interfaces::errors::{Error, decode_error};
use console_core_byte_streams::ReadBitsRtl;
use console_core_number_conversion::{fitted, index};

pub(crate) trait Fields: ReadBitsRtl {
  fn bits_u8(&mut self, width: u32) -> Result<u8, Error>;

  fn bits_u16(&mut self, width: u32) -> Result<u16, Error>;
}

impl<B: ReadBitsRtl> Fields for B {
  fn bits_u8(&mut self, width: u32) -> Result<u8, Error> {
    let raw = self.read_bits_leq32(width)?;
    let Ok(field) = fitted::<u32, u8>(raw);

    Ok(field)
  }

  fn bits_u16(&mut self, width: u32) -> Result<u16, Error> {
    let raw = self.read_bits_leq32(width)?;
    let Ok(field) = fitted::<u32, u16>(raw);

    Ok(field)
  }
}

pub(crate) fn entry<T>(list: &[T], position: u32) -> Result<&T, Error> {
  let Ok(at) = index(position);

  match list.get(at) {
    Some(found) => Ok(found),
    None => decode_error("vorbis: the stream names something that is not there"),
  }
}

pub(crate) fn copied<T: Copy>(list: &[T], position: u32) -> Result<T, Error> {
  let found = entry(list, position)?;

  Ok(*found)
}

pub(crate) fn entry_mut<T>(list: &mut [T], position: u32) -> Result<&mut T, Error> {
  let Ok(at) = index(position);

  match list.get_mut(at) {
    Some(found) => Ok(found),
    None => decode_error("vorbis: the stream names something that is not there"),
  }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Stretch {
  pub(crate) start: u32,
  pub(crate) length: u32,
}

pub(crate) fn span(list: &[f32], stretch: Stretch) -> Result<&[f32], Error> {
  let Ok(from) = index(stretch.start);
  let Ok(to) = index(stretch.start.saturating_add(stretch.length));

  match list.get(from..to) {
    Some(found) => Ok(found),
    None => decode_error("vorbis: the stream names a stretch that is not there"),
  }
}

pub(crate) fn span_mut(list: &mut [f32], stretch: Stretch) -> Result<&mut [f32], Error> {
  let Ok(from) = index(stretch.start);
  let Ok(to) = index(stretch.start.saturating_add(stretch.length));

  match list.get_mut(from..to) {
    Some(found) => Ok(found),
    None => decode_error("vorbis: the stream names a stretch that is not there"),
  }
}

pub(crate) fn tolerate_end(result: Result<(), Error>) -> Result<(), Error> {
  match result {
    Ok(()) => Ok(()),
    Err(Error::IoError(fault)) => match bool::then_some(fault.kind() == ErrorKind::Other, ()) {
      Some(()) => Ok(()),
      None => Err(Error::IoError(fault)),
    },
    Err(Error::DecodeError(sentence)) => Err(Error::DecodeError(sentence)),
    Err(Error::SeekError(kind)) => Err(Error::SeekError(kind)),
    Err(Error::Unsupported(sentence)) => Err(Error::Unsupported(sentence)),
    Err(Error::LimitError(sentence)) => Err(Error::LimitError(sentence)),
    Err(Error::ResetRequired) => Err(Error::ResetRequired),
  }
}
