//! How a state is written down and read back, a field at a time.
//!
//! effect's `Schema` is a value that knows how to `encode` and `decode` a type;
//! here it is a trait on the type, because a Rust state is a type before it is
//! anything else, and a state made of other states writes itself by asking
//! each part to write itself. A number is written most significant byte first,
//! a length is a `u64` before what it counts, and a byte that stands for a case is a constant named
//! in the type that owns the case.
//!
//! Nothing here can be cut short or flipped without being met: `restore`
//! checks the sum over the whole snapshot before any `decode` is asked, so a
//! `ParseError` from a field is a snapshot written by a different layout, not
//! a bad disk. Length-prefixed binary fields can be borrowed during decoding.

use alloc::{string::String, vec::Vec};
use core::fmt;
#[cfg(all(feature = "std", unix))]
use std::os::unix::ffi::{OsStrExt, OsStringExt};
#[cfg(all(feature = "std", unix))]
use std::path::PathBuf;

use console_core_bytes::{Bytes, Cursor};
use console_core_never::Never;
use console_core_number_conversion::fitted;

use crate::snapshot::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
  Truncated,
  NotASnapshot,
  Corrupt,
  Trailing,
  UnknownTag(u8),
  NotText,
  Version(Version),
}

impl fmt::Display for ParseError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      ParseError::Truncated => to.write_str("the snapshot ends before the state does"),
      ParseError::NotASnapshot => to.write_str("this is not a snapshot of a machine"),
      ParseError::Corrupt => to.write_str("the snapshot's sum does not match what it holds"),
      ParseError::Trailing => to.write_str("the snapshot goes on after the state has ended"),
      ParseError::UnknownTag(tag) => write!(
        to,
        "the snapshot names a case, {tag}, that this layout does not have"
      ),
      ParseError::NotText => to.write_str("the snapshot holds text that is not UTF-8"),
      ParseError::Version(Version(found)) => write!(
        to,
        "the snapshot was written by layout {found}, which nothing here reads"
      ),
    }
  }
}

impl core::error::Error for ParseError {}

pub trait Serializable: Sized {
  fn encode(&self, encoder: &mut Encoder) -> Result<(), Never>;

  fn decode(decoder: &mut Decoder<'_>) -> Result<Self, ParseError>;
}

#[derive(Debug)]
pub struct Encoder {
  bytes: Vec<u8>,
}

impl Encoder {
  pub(crate) fn new() -> Result<Self, Never> {
    Ok(Encoder { bytes: Vec::new() })
  }

  pub fn byte(&mut self, byte: u8) -> Result<(), Never> {
    self.bytes.push(byte);

    Ok(())
  }

  pub fn bytes(&mut self, bytes: &[u8]) -> Result<(), Never> {
    self.bytes.extend_from_slice(bytes);

    Ok(())
  }

  pub fn counted(&mut self, bytes: &[u8]) -> Result<(), Never> {
    let Ok(count): Result<u64, Never> = fitted(bytes.len());
    let Ok(()) = count.encode(self);

    self.bytes(bytes)
  }

  pub(crate) fn finish(self) -> Result<Vec<u8>, Never> {
    Ok(self.bytes)
  }
}

#[derive(Debug)]
pub struct Decoder<'a> {
  cursor: Cursor<'a>,
}

impl<'a> Decoder<'a> {
  pub(crate) fn new(bytes: &'a [u8]) -> Result<Self, Never> {
    let Ok(cursor) = Cursor::new(bytes);

    Ok(Decoder { cursor })
  }

  pub fn byte(&mut self) -> Result<u8, ParseError> {
    self.cursor.byte().map_err(|_| ParseError::Truncated)
  }

  pub fn bytes(&mut self) -> Result<Bytes, ParseError> {
    let bytes = self.counted_slice()?;
    let Ok(owned) = Bytes::copy_from_slice(bytes);

    Ok(owned)
  }

  pub fn counted_slice(&mut self) -> Result<&'a [u8], ParseError> {
    let count = u64::decode(self)?;

    match self.cursor.take(count) {
      Ok(bytes) => Ok(bytes),
      Err(_truncated) => {
        let Ok(_unread) = self.cursor.take_rest();

        Err(ParseError::Truncated)
      }
    }
  }

  pub(crate) fn finish(self) -> Result<(), ParseError> {
    let Ok(rest) = self.cursor.rest();

    match rest {
      [] => Ok(()),
      [_, ..] => Err(ParseError::Trailing),
    }
  }
}

impl Serializable for u8 {
  fn encode(&self, encoder: &mut Encoder) -> Result<(), Never> {
    encoder.byte(*self)
  }

  fn decode(decoder: &mut Decoder<'_>) -> Result<Self, ParseError> {
    decoder.byte()
  }
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one implementation template supplies five integer types with matching big-endian encoding and checked fixed-width decoding, keeping the wire format and truncation handling identical"
  )
)]
macro_rules! numbers {
    ($(($number:ty, $read:ident)),*) => {
        $(
            impl Serializable for $number {
                fn encode(&self, encoder: &mut Encoder) -> Result<(), Never> {
                    encoder.bytes(&self.to_be_bytes())
                }

                fn decode(decoder: &mut Decoder<'_>) -> Result<Self, ParseError> {
                    decoder.cursor.$read().map_err(|_| ParseError::Truncated)
                }
            }
        )*
    };
}

numbers!(
  (u16, u16_be),
  (u32, u32_be),
  (u64, u64_be),
  (i32, i32_be),
  (i64, i64_be)
);

impl Serializable for Bytes {
  fn encode(&self, encoder: &mut Encoder) -> Result<(), Never> {
    encoder.counted(self)
  }

  fn decode(decoder: &mut Decoder<'_>) -> Result<Self, ParseError> {
    decoder.bytes()
  }
}

impl Serializable for String {
  fn encode(&self, encoder: &mut Encoder) -> Result<(), Never> {
    encoder.counted(self.as_bytes())
  }

  fn decode(decoder: &mut Decoder<'_>) -> Result<Self, ParseError> {
    let bytes = decoder.bytes()?;

    bytes.into_utf8().map_err(|_not_text| ParseError::NotText)
  }
}

#[cfg(all(feature = "std", unix))]
impl Serializable for PathBuf {
  fn encode(&self, encoder: &mut Encoder) -> Result<(), Never> {
    encoder.counted(self.as_os_str().as_bytes())
  }

  fn decode(decoder: &mut Decoder<'_>) -> Result<Self, ParseError> {
    let bytes = decoder.bytes()?;

    let Ok(raw) = bytes.into_vec();

    Ok(PathBuf::from(std::ffi::OsString::from_vec(raw)))
  }
}

const NONE: u8 = 0;
const SOME: u8 = 1;

impl<T: Serializable> Serializable for Option<T> {
  fn encode(&self, encoder: &mut Encoder) -> Result<(), Never> {
    match self {
      None => encoder.byte(NONE),
      Some(value) => {
        let Ok(()) = encoder.byte(SOME);

        value.encode(encoder)
      }
    }
  }

  fn decode(decoder: &mut Decoder<'_>) -> Result<Self, ParseError> {
    let tag = decoder.byte()?;

    match tag {
      NONE => Ok(None),
      SOME => T::decode(decoder).map(Some),
      unknown => Err(ParseError::UnknownTag(unknown)),
    }
  }
}

impl<T: Serializable> Serializable for Vec<T> {
  fn encode(&self, encoder: &mut Encoder) -> Result<(), Never> {
    let Ok(count): Result<u64, Never> = fitted(self.len());
    let Ok(()) = count.encode(encoder);

    for item in self {
      let Ok(()) = item.encode(encoder);
    }

    Ok(())
  }

  fn decode(decoder: &mut Decoder<'_>) -> Result<Self, ParseError> {
    let count = u64::decode(decoder)?;
    let mut items = Vec::new();

    for _ in 0..count {
      let item = T::decode(decoder)?;

      items.push(item);
    }

    Ok(items)
  }
}

#[cfg(kani)]
#[path = "../proofs/schema.rs"]
mod proofs;
