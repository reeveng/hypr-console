//! Font offsets distinguish a null target from an unreadable target.

use crate::{FontData, FontRead, FontReadWithArgs, ReadError};
use console_core_never::Never;
use types::{Nullable, Offset16, Offset24, Offset32};

pub trait Offset: Copy {
  fn to_u64(self) -> Result<u64, Never>;

  fn non_null(self) -> Result<Option<u64>, Never> {
    let Ok(value) = self.to_u64();

    Ok(match value {
      0 => None,
      _present => Some(value),
    })
  }
}

impl Offset for Offset16 {
  fn to_u64(self) -> Result<u64, Never> {
    let Ok(value) = self.to_u32();

    Ok(u64::from(value))
  }
}

impl Offset for Offset24 {
  fn to_u64(self) -> Result<u64, Never> {
    let Ok(value) = self.to_u32();

    Ok(u64::from(value))
  }
}

impl Offset for Offset32 {
  fn to_u64(self) -> Result<u64, Never> {
    let Ok(value) = self.to_u32();

    Ok(u64::from(value))
  }
}

pub trait ResolveOffset {
  fn resolve<'a, T: FontRead<'a>>(&self, data: FontData<'a>) -> Result<T, ReadError>;

  fn resolve_with_args<'a, T: FontReadWithArgs<'a>>(
    &self,
    data: FontData<'a>,
    args: &T::Arguments,
  ) -> Result<T, ReadError>;
}

pub trait ResolveNullableOffset {
  fn resolve<'a, T: FontRead<'a>>(&self, data: FontData<'a>) -> Result<Option<T>, ReadError>;

  fn resolve_with_args<'a, T: FontReadWithArgs<'a>>(
    &self,
    data: FontData<'a>,
    args: &T::Arguments,
  ) -> Result<Option<T>, ReadError>;
}

fn target<'a>(offset: impl Offset, data: FontData<'a>) -> Result<FontData<'a>, ReadError> {
  let Ok(position) = offset.non_null();
  let position_2 = position.ok_or(ReadError::NullOffset)?;
  let Ok(tail) = data.split_off(position_2);

  tail.ok_or(ReadError::OutOfBounds)
}

impl<O: Offset> ResolveOffset for O {
  fn resolve<'a, T: FontRead<'a>>(&self, data: FontData<'a>) -> Result<T, ReadError> {
    let data_2 = target(*self, data)?;

    T::read(data_2)
  }

  fn resolve_with_args<'a, T: FontReadWithArgs<'a>>(
    &self,
    data: FontData<'a>,
    args: &T::Arguments,
  ) -> Result<T, ReadError> {
    let data_2 = target(*self, data)?;

    T::read_with_args(data_2, args)
  }
}

impl<O: Offset> ResolveNullableOffset for Nullable<O> {
  fn resolve<'a, T: FontRead<'a>>(&self, data: FontData<'a>) -> Result<Option<T>, ReadError> {
    let Ok(offset) = self.offset();

    match offset.resolve(data) {
      Ok(value) => Ok(Some(value)),
      Err(ReadError::NullOffset) => Ok(None),
      Err(fault) => Err(fault),
    }
  }

  fn resolve_with_args<'a, T: FontReadWithArgs<'a>>(
    &self,
    data: FontData<'a>,
    args: &T::Arguments,
  ) -> Result<Option<T>, ReadError> {
    let Ok(offset) = self.offset();

    match offset.resolve_with_args(data, args) {
      Ok(value) => Ok(Some(value)),
      Err(ReadError::NullOffset) => Ok(None),
      Err(fault) => Err(fault),
    }
  }
}
