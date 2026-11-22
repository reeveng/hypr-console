//! Arrays borrow their bytes and validate a record only when it is requested.
//!
//! Fixed-size arrays use their declared record width. Variable-size arrays
//! consume a positive record extent at each step; a zero extent ends the walk,
//! and a truncated extent is returned to the caller as a read error.

use std::marker::PhantomData;

use bytemuck::AnyBitPattern;
use console_core_never::Never;
use font_types::FixedSize;

use crate::read::{ComputeSize, FontReadWithArgs, ReadArgs, VarSize};
use crate::{Emptiness, FontData, FontRead, ReadError};

#[derive(Clone)]
#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(explicit065_checked_constructors, reason = "the published ComputedArray Default contains no records and cannot request a record width or arguments")
)]
pub struct ComputedArray<'a, T: ReadArgs> {
  item_len: u64,
  len: u64,
  data: FontData<'a>,
  args: T::Args,
}

impl<'a, T: ComputeSize> ComputedArray<'a, T> {
  pub fn new(data: FontData<'a>, args: T::Args) -> Result<Self, ReadError> {
    let item_len = T::compute_size(&args)?;
    let Ok(bytes) = data.len();
    let len = match bytes.checked_div(item_len) {
      Some(length) => length,
      None => 0,
    };

    Ok(Self {
      item_len,
      len,
      data,
      args,
    })
  }

  pub fn len(&self) -> Result<u64, Never> {
    Ok(self.len)
  }

  pub fn is_empty(&self) -> Result<Emptiness, Never> {
    Ok(match self.len {
      0 => Emptiness::Empty,
      _ => Emptiness::NonEmpty,
    })
  }
}

impl<T: ReadArgs> ReadArgs for ComputedArray<'_, T> {
  type Args = T::Args;
}

impl<'a, T> FontReadWithArgs<'a> for ComputedArray<'a, T>
where
  T: ComputeSize + FontReadWithArgs<'a>,
{
  fn read_with_args(data: FontData<'a>, args: &Self::Args) -> Result<Self, ReadError> {
    Self::new(data, *args)
  }
}

impl<T> Default for ComputedArray<'_, T>
where
  T: ReadArgs,
  T::Args: Default,
{
  fn default() -> Self {
    Self {
      item_len: 0,
      len: 0,
      data: FontData::EMPTY,
      args: T::Args::default(),
    }
  }
}

impl<'a, T> ComputedArray<'a, T>
where
  T: FontReadWithArgs<'a>,
  T::Args: 'static,
{
  pub fn iter(
    &self,
  ) -> Result<impl Iterator<Item = Result<T, ReadError>> + 'a + use<'a, T>, Never> {
    let data = self.data;
    let args = self.args;
    let width = self.item_len;

    Ok((0..self.len).map(move |index| {
      let start = index.checked_mul(width).ok_or(ReadError::OutOfBounds)?;
      let Ok(data_2) = data.split_off(start);
      let data_3 = data_2.ok_or(ReadError::OutOfBounds)?;

      T::read_with_args(data_3, &args)
    }))
  }

  pub fn get(&self, index: u64) -> Result<T, ReadError> {
    match ::core::primitive::bool::then_some(index < self.len, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(ReadError::OutOfBounds),
    }

    let start = index
      .checked_mul(self.item_len)
      .ok_or(ReadError::OutOfBounds)?;
    let Ok(data) = self.data.split_off(start);
    let data_2 = data.ok_or(ReadError::OutOfBounds)?;

    T::read_with_args(data_2, &self.args)
  }
}

impl<T: ReadArgs> std::fmt::Debug for ComputedArray<'_, T> {
  fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    output
      .debug_struct("DynSizedArray")
      .field("bytes", &self.data)
      .finish()
  }
}

pub struct VarLenArray<'a, T> {
  data: FontData<'a>,
  phantom: PhantomData<*const T>,
}

impl<'a, T: FontRead<'a> + VarSize> VarLenArray<'a, T> {
  pub fn get(&self, index: u64) -> Result<Option<T>, ReadError> {
    let Ok(empty) = self.data.is_empty();

    match empty {
      Emptiness::Empty => return Ok(None),
      Emptiness::NonEmpty => {}
    }

    let mut position = 0u64;

    for _record in 0..index {
      let length = T::read_len_at(self.data, position)?;

      match length {
        0 => return Ok(None),
        _nonzero => {
          let end = position.checked_add(length).ok_or(ReadError::OutOfBounds)?;

          position = end;
        }
      }
    }

    let Ok(data) = self.data.split_off(position);

    match data {
      Some(data_2) => T::read(data_2).map(Some),
      None => Ok(None),
    }
  }

  pub fn iter(
    &self,
  ) -> Result<impl Iterator<Item = Result<T, ReadError>> + 'a + use<'a, T>, Never>
  where
    T: 'a,
  {
    Ok(VariableItems {
      rest: self.data,
      phantom: PhantomData::<fn() -> T>,
    })
  }
}

struct VariableItems<'a, T> {
  rest: FontData<'a>,
  phantom: PhantomData<fn() -> T>,
}

impl<'a, T: FontRead<'a> + VarSize> Iterator for VariableItems<'a, T> {
  type Item = Result<T, ReadError>;

  fn next(&mut self) -> Option<Self::Item> {
    let Ok(empty) = self.rest.is_empty();

    match empty {
      Emptiness::Empty => return None,
      Emptiness::NonEmpty => {}
    }

    let current = std::mem::replace(&mut self.rest, FontData::EMPTY);
    let length = match T::read_len_at(current, 0) {
      Ok(0) => return None,
      Ok(length) => length,
      Err(fault) => return Some(Err(fault)),
    };
    let Ok(record) = current.slice(..length);
    let Ok(rest) = current.split_off(length);

    match (record, rest) {
      (Some(record_2), Some(rest_2)) => {
        self.rest = rest_2;

        Some(T::read(record_2))
      }
      (None, _) | (_, None) => Some(Err(ReadError::OutOfBounds)),
    }
  }
}

impl<'a, T> FontRead<'a> for VarLenArray<'a, T> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    Ok(Self {
      data,
      phantom: PhantomData,
    })
  }
}

impl<T> Default for VarLenArray<'_, T> {
  fn default() -> Self {
    Self {
      data: FontData::EMPTY,
      phantom: PhantomData,
    }
  }
}

impl<T: AnyBitPattern> ReadArgs for &[T] {
  type Args = u16;
}

impl<'a, T: AnyBitPattern + FixedSize> FontReadWithArgs<'a> for &'a [T] {
  fn read_with_args(data: FontData<'a>, count: &u16) -> Result<Self, ReadError> {
    let length = u64::from(*count)
      .checked_mul(T::RAW_BYTE_LEN)
      .ok_or(ReadError::OutOfBounds)?;

    data.read_array(0..length)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  struct Item;

  impl FontRead<'_> for Item {
    fn read(_data: FontData<'_>) -> Result<Self, ReadError> {
      Ok(Self)
    }
  }

  impl VarSize for Item {
    type Size = u32;

    fn read_len_at(data: FontData<'_>, position: u64) -> Result<u64, ReadError> {
      data.read_at::<u32>(position).map(u64::from)
    }
  }

  #[test]
  fn a_zero_length_item_ends_variable_array_iteration() -> Result<(), ReadError> {
    let Ok(data) = FontData::new(&[0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0]);
    let array = VarLenArray::<Item>::read(data)?;
    let Ok(items) = array.iter();

    assert_eq!(items.take(10).count(), 2);

    Ok(())
  }

  #[test]
  fn a_truncated_item_reports_its_fault_once() -> Result<(), ReadError> {
    let Ok(data) = FontData::new(&[0, 0, 0, 8, 1]);
    let array = VarLenArray::<Item>::read(data)?;
    let Ok(mut items) = array.iter();

    assert!(matches!(items.next(), Some(Err(ReadError::OutOfBounds))));
    assert!(items.next().is_none());

    Ok(())
  }
}
