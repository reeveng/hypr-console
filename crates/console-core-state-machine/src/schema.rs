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
//! a bad disk.

use std::fmt;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

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
            ParseError::UnknownTag(tag) => write!(to, "the snapshot names a case, {tag}, that this layout does not have"),
            ParseError::NotText => to.write_str("the snapshot holds text that is not UTF-8"),
            ParseError::Version(Version(found)) => write!(to, "the snapshot was written by layout {found}, which nothing here reads"),
        }
    }
}

impl std::error::Error for ParseError {}

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
    rest: &'a [u8],
}

impl<'a> Decoder<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Result<Self, Never> {
        Ok(Decoder { rest: bytes })
    }

    pub fn byte(&mut self) -> Result<u8, ParseError> {
        match self.rest.split_first() {
            Some((byte, rest)) => {
                self.rest = rest;

                Ok(*byte)
            }
            None => Err(ParseError::Truncated),
        }
    }

    pub fn bytes(&mut self) -> Result<Vec<u8>, ParseError> {
        let count = u64::decode(self)?;
        let mut bytes = Vec::new();

        for _ in 0..count {
            let byte = self.byte()?;

            bytes.push(byte);
        }

        Ok(bytes)
    }

    pub(crate) fn finish(self) -> Result<(), ParseError> {
        match self.rest.is_empty() {
            true => Ok(()),
            false => Err(ParseError::Trailing),
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

macro_rules! numbers {
    ($($number:ty),*) => {
        $(
            impl Serializable for $number {
                fn encode(&self, encoder: &mut Encoder) -> Result<(), Never> {
                    encoder.bytes(&self.to_be_bytes())
                }

                fn decode(decoder: &mut Decoder<'_>) -> Result<Self, ParseError> {
                    match decoder.rest.split_first_chunk() {
                        Some((array, rest)) => {
                            decoder.rest = rest;

                            Ok(<$number>::from_be_bytes(*array))
                        }
                        None => Err(ParseError::Truncated),
                    }
                }
            }
        )*
    };
}

numbers!(u16, u32, u64, i32, i64);

impl Serializable for String {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), Never> {
        encoder.counted(self.as_bytes())
    }

    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, ParseError> {
        let bytes = decoder.bytes()?;

        String::from_utf8(bytes).map_err(|_not_text| ParseError::NotText)
    }
}

impl Serializable for PathBuf {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), Never> {
        encoder.counted(self.as_os_str().as_bytes())
    }

    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, ParseError> {
        let bytes = decoder.bytes()?;

        Ok(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
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
