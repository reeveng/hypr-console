//! Borrowed font bytes and the cursor that validates each record's extent.
//!
//! File positions remain 64-bit quantities until a slice is accessed. Scalar
//! widths come from the wire type, and a zero-copy record is checked by
//! `bytemuck` before it is borrowed, including its alignment and array length.

use std::ops::{Bound, Range, RangeBounds};

use bytemuck::AnyBitPattern;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use types::{BigEndian, FixedSize, Scalar};

use crate::{Emptiness, FontRead};
use crate::array::{ComputedArray, VarLenArray};
use crate::read::{ComputeSize, FontReadWithArgs, ReadError};

#[derive(Debug, Default, Clone, Copy)]
pub struct FontData<'a> {
  bytes: &'a [u8],
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Cursor<'a> {
  pos: u64,
  data: FontData<'a>,
}

static EMPTY_TABLE_BYTES: [u8; 264] = {
  let mut bytes = [0; 264];
  let [_, format, ..] = &mut bytes;

  *format = 1;

  bytes
};

impl FontData<'static> {
  pub(crate) fn default_table_data() -> Result<Self, Never> {
    let [_, _, rest @ ..] = &EMPTY_TABLE_BYTES;

    Self::new(rest)
  }

  pub(crate) fn default_format_1_u16_table_data() -> Result<Self, Never> {
    Self::new(&EMPTY_TABLE_BYTES)
  }

  pub(crate) fn default_format_1_u8_table_data() -> Result<Self, Never> {
    let [_, rest @ ..] = &EMPTY_TABLE_BYTES;

    Self::new(rest)
  }

  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(explicit007_no_bool_return, reason = "generated fixed default-table constructors call this predicate from compile-time Boolean assertions")
  )]
  pub(crate) const fn default_data_long_enough(n_bytes: u64) -> Result<bool, Never> {
    Ok(n_bytes <= 262)
  }
}

impl<'a> FontData<'a> {
  pub const EMPTY: FontData<'static> = FontData { bytes: &[] };

  pub const fn new(bytes: &'a [u8]) -> Result<Self, Never> {
    Ok(Self { bytes })
  }

  pub(crate) fn base_address(&self) -> Result<u64, Never> {
    fitted(self.bytes.as_ptr().addr())
  }

  pub fn len(&self) -> Result<u64, Never> {
    fitted(self.bytes.len())
  }

  pub fn is_empty(&self) -> Result<Emptiness, Never> {
    Ok(match self.bytes {
      [] => Emptiness::Empty,
      [_, ..] => Emptiness::NonEmpty,
    })
  }

  pub fn split_off(&self, position: u64) -> Result<Option<Self>, Never> {
    self.slice(position..)
  }

  pub fn take_up_to(&mut self, position: u64) -> Result<Option<Self>, Never> {
    let Ok(position_2) = index(position);

    Ok(match self.bytes.split_at_checked(position_2) {
      Some((head, tail)) => {
        self.bytes = tail;

        Some(Self { bytes: head })
      }
      None => None,
    })
  }

  pub fn slice(&self, range: impl RangeBounds<u64>) -> Result<Option<Self>, Never> {
    let Ok(length) = self.len();
    let start = match range.start_bound() {
      Bound::Included(position) => Some(*position),
      Bound::Excluded(position) => position.checked_add(1),
      Bound::Unbounded => Some(0),
    };
    let end = match range.end_bound() {
      Bound::Included(position) => position.checked_add(1),
      Bound::Excluded(position) => Some(*position),
      Bound::Unbounded => Some(length),
    };

    Ok(match (start, end) {
      (Some(start_2), Some(end_2)) => {
        let Ok(start_3) = index(start_2);
        let Ok(end_3) = index(end_2);

        self.bytes.get(start_3..end_3).map(|bytes| Self { bytes })
      }
      (None, _) | (_, None) => None,
    })
  }

  fn scalar_bytes<T: FixedSize>(&self, offset: u64) -> Result<&'a [u8], ReadError> {
    let end = offset
      .checked_add(T::RAW_BYTE_LEN)
      .ok_or(ReadError::OutOfBounds)?;
    let Ok(start) = index(offset);
    let Ok(end_2) = index(end);

    self.bytes.get(start..end_2).ok_or(ReadError::OutOfBounds)
  }

  pub fn read_at<T: Scalar>(&self, offset: u64) -> Result<T, ReadError> {
    let bytes = self.scalar_bytes::<T>(offset)?;
    let Ok(value) = T::read(bytes);

    value.ok_or(ReadError::OutOfBounds)
  }

  pub fn read_at_or_zero<T: Scalar>(&self, offset: u64) -> Result<T, Never> {
    match self.read_at(offset) {
      Ok(value) => Ok(value),
      Err(_beyond_the_data) => T::from_raw(bytemuck::Zeroable::zeroed()),
    }
  }

  pub fn read_at_or_default<T: Scalar + Default>(&self, offset: u64) -> Result<T, Never> {
    Ok(match self.read_at(offset) {
      Ok(value) => value,
      Err(_beyond_the_data) => T::default(),
    })
  }

  pub fn read_at_if_present<T: Scalar>(&self, range: Range<u64>) -> Result<Option<T>, Never> {
    Ok(match ::core::primitive::bool::then_some(range.is_empty(), ()) {
      ::core::option::Option::Some(()) => None,
      ::core::option::Option::None => match self.read_at(range.start) {
        Ok(value) => Some(value),
        Err(_beyond_the_data) => None,
      },
    })
  }

  pub fn read_array_or_empty<T: AnyBitPattern + FixedSize>(
    &self,
    range: Range<u64>,
  ) -> Result<&'a [T], Never> {
    Ok(match self.read_array(range) {
      Ok(values) => values,
      Err(_beyond_the_data) => &[],
    })
  }

  pub fn read_array_if_present<T: AnyBitPattern + FixedSize>(
    &self,
    range: Range<u64>,
  ) -> Result<Option<&'a [T]>, Never> {
    Ok(match ::core::primitive::bool::then_some(range.is_empty(), ()) {
      ::core::option::Option::Some(()) => None,
      ::core::option::Option::None => match self.read_array(range) {
        Ok(values) => Some(values),
        Err(_beyond_the_data) => None,
      },
    })
  }

  pub fn read_with_args_or_default<T>(
    &self,
    range: Range<u64>,
    args: &T::Args,
  ) -> Result<T, Never>
  where
    T: FontReadWithArgs<'a> + Default,
  {
    Ok(match self.read_with_args(range, args) {
      Ok(value) => value,
      Err(_beyond_the_data) => T::default(),
    })
  }

  pub fn read_var_array_or_empty<T>(&self, position: u64) -> Result<VarLenArray<'a, T>, Never> {
    let Ok(rest) = self.split_off(position);

    Ok(match rest {
      Some(rest_2) => match VarLenArray::read(rest_2) {
        Ok(values) => values,
        Err(_unreadable) => VarLenArray::default(),
      },
      None => VarLenArray::default(),
    })
  }

  pub fn read_be_at<T: Scalar>(&self, offset: u64) -> Result<BigEndian<T>, ReadError> {
    let bytes = self.scalar_bytes::<T>(offset)?;
    let Ok(value) = BigEndian::from_slice(bytes);

    value.ok_or(ReadError::OutOfBounds)
  }

  pub fn read_with_args<T>(&self, range: Range<u64>, args: &T::Args) -> Result<T, ReadError>
  where
    T: FontReadWithArgs<'a>,
  {
    let Ok(data) = self.slice(range);
    let data_2 = data.ok_or(ReadError::OutOfBounds)?;

    T::read_with_args(data_2, args)
  }

  fn check_in_bounds(&self, offset: u64) -> Result<(), ReadError> {
    let Ok(length) = self.len();

    match ::core::primitive::bool::then_some(offset <= length, ()) {
      ::core::option::Option::Some(()) => Ok(()),
      ::core::option::Option::None => Err(ReadError::OutOfBounds),
    }
  }

  pub fn read_ref_at<T: AnyBitPattern + FixedSize>(
    &self,
    offset: u64,
  ) -> Result<&'a T, ReadError> {
    let bytes = self.scalar_bytes::<T>(offset)?;

    bytemuck::try_from_bytes(bytes).map_err(|_invalid_record_layout| ReadError::InvalidArrayLen)
  }

  pub fn read_array<T: AnyBitPattern + FixedSize>(
    &self,
    range: Range<u64>,
  ) -> Result<&'a [T], ReadError> {
    match std::mem::size_of::<T>() {
      0 => return Err(ReadError::InvalidArrayLen),
      _nonzero => {}
    }

    let Ok(start) = index(range.start);
    let Ok(end) = index(range.end);
    let bytes = self.bytes.get(start..end).ok_or(ReadError::OutOfBounds)?;

    bytemuck::try_cast_slice(bytes).map_err(|_invalid_array_layout| ReadError::InvalidArrayLen)
  }

  pub(crate) fn cursor(&self) -> Result<Cursor<'a>, Never> {
    Ok(Cursor {
      pos: 0,
      data: *self,
    })
  }

  pub fn as_bytes(&self) -> Result<&'a [u8], Never> {
    Ok(self.bytes)
  }
}

impl<'a> Cursor<'a> {
  pub(crate) fn advance<T: Scalar>(&mut self) -> Result<(), Never> {
    self.advance_by(T::RAW_BYTE_LEN)
  }

  pub(crate) fn advance_by(&mut self, n_bytes: u64) -> Result<(), Never> {
    self.pos = self.pos.saturating_add(n_bytes);

    Ok(())
  }

  pub(crate) fn read_u32_var(&mut self) -> Result<u32, ReadError> {
    let first = self.read::<u8>()?;
    let (mut value, following) = match first {
      0..=0x7f => (u32::from(first), 0u32),
      0x80..=0xbf => (u32::from(first & 0x3f), 1),
      0xc0..=0xdf => (u32::from(first & 0x1f), 2),
      0xe0..=0xef => (u32::from(first & 0x0f), 3),
      0xf0..=0xff => (0, 4),
    };

    for _byte in 0..following {
      let byte = self.read::<u8>()?;

      value = value.wrapping_shl(8) | u32::from(byte);
    }

    Ok(value)
  }

  pub(crate) fn read<T: Scalar>(&mut self) -> Result<T, ReadError> {
    let read = self.data.read_at(self.pos);
    let Ok(()) = self.advance::<T>();

    read
  }

  pub(crate) fn read_be<T: Scalar>(&mut self) -> Result<BigEndian<T>, ReadError> {
    let read = self.data.read_be_at(self.pos);
    let Ok(()) = self.advance::<T>();

    read
  }

  pub(crate) fn read_with_args<T>(&mut self, args: &T::Args) -> Result<T, ReadError>
  where
    T: FontReadWithArgs<'a> + ComputeSize,
  {
    let length = T::compute_size(args)?;
    let end = self.pos.checked_add(length).ok_or(ReadError::OutOfBounds)?;
    let read = self.data.read_with_args(self.pos..end, args);
    let Ok(()) = self.advance_by(length);

    read
  }

  pub(crate) fn read_computed_array<T>(
    &mut self,
    count: u64,
    args: &T::Args,
  ) -> Result<ComputedArray<'a, T>, ReadError>
  where
    T: FontReadWithArgs<'a> + ComputeSize,
  {
    let each = T::compute_size(args)?;
    let length = count.checked_mul(each).ok_or(ReadError::OutOfBounds)?;
    let end = self.pos.checked_add(length).ok_or(ReadError::OutOfBounds)?;
    let read = self.data.read_with_args(self.pos..end, args);
    let Ok(()) = self.advance_by(length);

    read
  }

  pub(crate) fn read_array<T: AnyBitPattern + FixedSize>(
    &mut self,
    count: u64,
  ) -> Result<&'a [T], ReadError> {
    let length = count
      .checked_mul(T::RAW_BYTE_LEN)
      .ok_or(ReadError::OutOfBounds)?;
    let end = self.pos.checked_add(length).ok_or(ReadError::OutOfBounds)?;
    let read = self.data.read_array(self.pos..end);
    let Ok(()) = self.advance_by(length);

    read
  }

  pub(crate) fn position(&self) -> Result<u64, ReadError> {
    self.data.check_in_bounds(self.pos)?;

    Ok(self.pos)
  }

  pub(crate) fn remaining_bytes(&self) -> Result<u64, Never> {
    let Ok(length) = self.data.len();

    Ok(length.saturating_sub(self.pos))
  }

  pub(crate) fn remaining(self) -> Result<Option<FontData<'a>>, Never> {
    self.data.split_off(self.pos)
  }

  pub fn is_empty(&self) -> Result<Emptiness, Never> {
    let Ok(remaining) = self.remaining_bytes();

    Ok(match remaining {
      0 => Emptiness::Empty,
      _ => Emptiness::NonEmpty,
    })
  }
}

impl<'a> FontRead<'a> for FontData<'a> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    Ok(data)
  }
}

impl AsRef<[u8]> for FontData<'_> {
  fn as_ref(&self) -> &[u8] {
    self.bytes
  }
}

impl<'a> From<&'a [u8]> for FontData<'a> {
  fn from(bytes: &'a [u8]) -> Self {
    Self { bytes }
  }
}

#[cfg(feature = "std")]
impl<'a> From<FontData<'a>> for std::borrow::Cow<'a, [u8]> {
  fn from(source: FontData<'a>) -> Self {
    std::borrow::Cow::Borrowed(source.bytes)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn default_table_formats_keep_their_leading_one() {
    let Ok(word) = FontData::default_format_1_u16_table_data();
    let Ok(byte) = FontData::default_format_1_u8_table_data();

    assert_eq!(word.read_at(0), Ok(1u16));
    assert_eq!(byte.read_at(0), Ok(1u8));
  }

  #[test]
  fn missing_and_overflowing_scalar_extents_are_read_errors() {
    let Ok(data) = FontData::new(&[0, 1, 2]);

    assert_eq!(data.read_at::<u16>(2), Err(ReadError::OutOfBounds));
    assert_eq!(data.read_at::<u16>(u64::MAX), Err(ReadError::OutOfBounds));
    assert_eq!(
      data.read_array::<BigEndian<u16>>(0..3),
      Err(ReadError::InvalidArrayLen)
    );
  }

  #[test]
  fn an_unaligned_record_is_a_read_error() {
    #[repr(align(2))]
    struct Aligned([u8; 3]);

    let bytes = Aligned([0, 1, 2]);
    let Ok(data) = FontData::new(&bytes.0);

    assert_eq!(data.read_ref_at::<u16>(1), Err(ReadError::InvalidArrayLen));
  }

  #[test]
  fn variable_integers_use_their_announced_width() -> Result<(), ReadError> {
    let cases: &[(&[u8], u32)] = &[
      (&[0x7f], 0x7f),
      (&[0x81, 0x23], 0x123),
      (&[0xc1, 0x23, 0x45], 0x12345),
      (&[0xe1, 0x23, 0x45, 0x67], 0x1234567),
      (&[0xf0, 0x89, 0xab, 0xcd, 0xef], 0x89abcdef),
    ];

    for (bytes, expected) in cases {
      let Ok(data) = FontData::new(bytes);
      let Ok(mut cursor) = data.cursor();
      let value = cursor.read_u32_var()?;
      let position = cursor.position()?;
      let Ok(length) = fitted::<_, u64>(bytes.len());

      assert_eq!(value, *expected);
      assert_eq!(position, length);
    }

    let Ok(truncated) = FontData::new(&[0xc1, 0x23]);
    let Ok(mut cursor) = truncated.cursor();

    assert_eq!(cursor.read_u32_var(), Err(ReadError::OutOfBounds));

    Ok(())
  }
}
