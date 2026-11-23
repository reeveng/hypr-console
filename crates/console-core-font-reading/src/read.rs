//! The requirements a font record supplies to its byte reader.

use types::{FixedSize, Scalar, Tag};

use crate::font_data::FontData;

pub trait FontRead<'a>: Sized {
  fn read(data: FontData<'a>) -> Result<Self, ReadError>;
}

pub trait ReadArgs {
  type Arguments: Copy;
}

pub trait FontReadWithArgs<'a>: Sized + ReadArgs {
  fn read_with_args(data: FontData<'a>, args: &Self::Arguments) -> Result<Self, ReadError>;
}

impl<'a, T: FontRead<'a>> ReadArgs for T {
  type Arguments = ();
}

impl<'a, T: FontRead<'a>> FontReadWithArgs<'a> for T {
  fn read_with_args(data: FontData<'a>, _args: &Self::Arguments) -> Result<Self, ReadError> {
    Self::read(data)
  }
}

pub trait Format<T> {
  const FORMAT: T;
}

pub trait Discriminant {
  fn read_discriminant(data: FontData<'_>) -> Result<u16, ReadError>;
}

pub trait ComputeSize: ReadArgs {
  fn compute_size(args: &Self::Arguments) -> Result<u64, ReadError>;
}

pub trait VarSize {
  type Size: Scalar + Into<u32>;

  fn read_len_at(data: FontData<'_>, position: u64) -> Result<u64, ReadError> {
    let size = data.read_at::<Self::Size>(position)?;
    let payload = u64::from(Into::<u32>::into(size));

    payload
      .checked_add(Self::Size::RAW_BYTE_LEN)
      .ok_or(ReadError::OutOfBounds)
  }

  fn total_len_for_count(data: FontData<'_>, count: u64) -> Result<u64, ReadError> {
    let mut position = 0u64;

    for _record in 0..count {
      let length = Self::read_len_at(data, position)?;

      match length {
        0 => return Ok(position),
        _nonzero => {
          let end = position.checked_add(length).ok_or(ReadError::OutOfBounds)?;

          position = end;
        }
      }
    }

    Ok(position)
  }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReadError {
  OutOfBounds,
  InvalidFormat(i64),
  InvalidSfnt(u32),
  InvalidTtc(Tag),
  InvalidCollectionIndex(u32),
  InvalidArrayLen,
  InvalidArrayIndex(u64),
  ValidationError,
  NullOffset,
  TableIsMissing(Tag),
  MetricIsMissing(Tag),
  MalformedData(&'static str),
}

impl std::fmt::Display for ReadError {
  fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::OutOfBounds => output.write_str("An offset was out of bounds"),
      Self::InvalidFormat(format) => write!(output, "Invalid format '{format}'"),
      Self::InvalidSfnt(version) => write!(output, "Invalid sfnt version 0x{version:08X}"),
      Self::InvalidTtc(tag) => write!(output, "Invalid ttc tag {tag}"),
      Self::InvalidCollectionIndex(index) => {
        write!(output, "Invalid index {index} for font collection")
      }
      Self::InvalidArrayIndex(position) => {
        write!(output, "array index {position} is out of bounds")
      }
      Self::InvalidArrayLen => {
        output.write_str("Specified array length not a multiple of item size")
      }
      Self::ValidationError => output.write_str("A validation error occurred"),
      Self::NullOffset => output.write_str("An offset was unexpectedly null"),
      Self::TableIsMissing(tag) => write!(output, "the {tag} table is missing"),
      Self::MetricIsMissing(tag) => write!(output, "the {tag} metric is missing"),
      Self::MalformedData(message) => write!(output, "Malformed data: '{message}'"),
    }
  }
}

impl core::error::Error for ReadError {}

#[cfg(test)]
mod tests {
  use super::*;

  struct LengthField;

  impl VarSize for LengthField {
    type Size = u16;

    fn read_len_at(data: FontData<'_>, position: u64) -> Result<u64, ReadError> {
      data.read_at::<u16>(position).map(u64::from)
    }
  }

  #[test]
  fn a_zero_length_record_ends_the_extent_walk() {
    let Ok(data) = FontData::new(&[0, 2, 0, 4, 0, 1, 0, 0, 0, 20]);

    assert_eq!(LengthField::total_len_for_count(data, u64::MAX), Ok(6));
  }
}
