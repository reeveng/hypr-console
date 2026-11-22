//! Offset arrays borrow their entries and resolve each target on demand.

use crate::{
  Emptiness, FontData, FontReadWithArgs, Offset, ReadArgs, ReadError, ResolveNullableOffset, ResolveOffset,
};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use font_types::{BigEndian, Nullable, Offset16, Scalar};

#[derive(Clone)]
pub struct ArrayOfOffsets<'a, T: ReadArgs, O: Scalar = Offset16> {
  offsets: &'a [BigEndian<O>],
  data: FontData<'a>,
  args: T::Args,
}

#[derive(Clone)]
pub struct ArrayOfNullableOffsets<'a, T: ReadArgs, O: Scalar = Offset16> {
  offsets: &'a [BigEndian<Nullable<O>>],
  data: FontData<'a>,
  args: T::Args,
}

impl<'a, T: ReadArgs, O: Scalar> ArrayOfOffsets<'a, T, O> {
  pub(crate) fn new(
    offsets: &'a [BigEndian<O>],
    data: FontData<'a>,
    args: T::Args,
  ) -> Result<Self, Never> {
    Ok(Self {
      offsets,
      data,
      args,
    })
  }
}

impl<'a, T, O> ArrayOfOffsets<'a, T, O>
where
  O: Scalar + Offset,
  T: ReadArgs + FontReadWithArgs<'a>,
  T::Args: Copy + 'static,
{
  pub fn len(&self) -> Result<u64, Never> {
    fitted(self.offsets.len())
  }

  pub fn is_empty(&self) -> Result<Emptiness, Never> {
    Ok(match self.offsets {
      [] => Emptiness::Empty,
      [_, ..] => Emptiness::NonEmpty,
    })
  }

  pub fn get(&self, position: u64) -> Result<T, ReadError> {
    let Ok(at) = index(position);
    let offset = self
      .offsets
      .get(at)
      .ok_or(ReadError::InvalidArrayIndex(position))?;
    let Ok(raw) = offset.get();

    raw.resolve_with_args(self.data, &self.args)
  }

  pub fn iter(
    &self,
  ) -> Result<impl Iterator<Item = Result<T, ReadError>> + 'a + use<'a, T, O>, Never> {
    let args = self.args;
    let data = self.data;

    Ok(self.offsets.iter().map(move |offset| {
      let Ok(raw) = offset.get();

      raw.resolve_with_args(data, &args)
    }))
  }

  pub(crate) fn iter_as_nullable(
    &self,
  ) -> Result<impl Iterator<Item = Result<Option<T>, ReadError>> + 'a + use<'a, T, O>, Never> {
    let Ok(targets) = self.iter();

    Ok(targets.map(|target| match target {
      Ok(value) => Ok(Some(value)),
      Err(ReadError::NullOffset) => Ok(None),
      Err(fault) => Err(fault),
    }))
  }
}

impl<'a, T: ReadArgs, O: Scalar> ArrayOfNullableOffsets<'a, T, O> {
  pub(crate) fn new(
    offsets: &'a [BigEndian<Nullable<O>>],
    data: FontData<'a>,
    args: T::Args,
  ) -> Result<Self, Never> {
    Ok(Self {
      offsets,
      data,
      args,
    })
  }
}

impl<'a, T, O> ArrayOfNullableOffsets<'a, T, O>
where
  O: Scalar + Offset,
  T: ReadArgs + FontReadWithArgs<'a>,
  T::Args: Copy + 'static,
{
  pub fn len(&self) -> Result<u64, Never> {
    fitted(self.offsets.len())
  }

  pub fn is_empty(&self) -> Result<Emptiness, Never> {
    Ok(match self.offsets {
      [] => Emptiness::Empty,
      [_, ..] => Emptiness::NonEmpty,
    })
  }

  pub fn get(&self, position: u64) -> Result<Option<T>, ReadError> {
    let Ok(at) = index(position);
    let offset = self
      .offsets
      .get(at)
      .ok_or(ReadError::InvalidArrayIndex(position))?;
    let Ok(raw) = offset.get();

    raw.resolve_with_args(self.data, &self.args)
  }

  pub fn iter(
    &self,
  ) -> Result<impl Iterator<Item = Result<Option<T>, ReadError>> + 'a + use<'a, T, O>, Never> {
    let args = self.args;
    let data = self.data;

    Ok(self.offsets.iter().map(move |offset| {
      let Ok(raw) = offset.get();

      raw.resolve_with_args(data, &args)
    }))
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a, T, O> crate::traversal::SomeArray<'a> for ArrayOfOffsets<'a, T, O>
where
  O: Scalar + Offset + Into<crate::traversal::OffsetType>,
  T: ReadArgs + FontReadWithArgs<'a> + crate::traversal::SomeTable<'a> + 'a,
  T::Args: Copy + 'static,
{
  fn type_name(&self) -> Result<&str, Never> {
    let full_name = std::any::type_name::<T>();
    Ok(match full_name.rsplit_once("::") {
      Some((_module, short_name)) => short_name,
      None => full_name,
    })
  }

  fn len(&self) -> Result<u64, Never> {
    let Ok(explicit_index) = console_core_number_conversion::fitted::<_, u64>(self.offsets.len());

    Ok(explicit_index)
  }

  fn get(&self, idx: u64) -> Result<Option<crate::traversal::FieldType<'a>>, Never> {
    let Ok(at) = console_core_number_conversion::index(idx);

    Ok(match self.offsets.get(at) {
      Some(off) => {
        let Ok(raw) = off.get();
        let result = raw.resolve_with_args::<T>(self.data, &self.args);
        let Ok(field) = crate::traversal::FieldType::offset(raw, result.map(Some));

        Some(field)
      }
      None => None,
    })
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a, T, O> crate::traversal::SomeArray<'a> for ArrayOfNullableOffsets<'a, T, O>
where
  O: Scalar + Offset + Into<crate::traversal::OffsetType> + Clone,
  T: ReadArgs + FontReadWithArgs<'a> + crate::traversal::SomeTable<'a> + 'a,
  T::Args: Copy + 'static,
{
  fn type_name(&self) -> Result<&str, Never> {
    let full_name = std::any::type_name::<T>();
    Ok(match full_name.rsplit_once("::") {
      Some((_module, short_name)) => short_name,
      None => full_name,
    })
  }

  fn len(&self) -> Result<u64, Never> {
    let Ok(explicit_index) = console_core_number_conversion::fitted::<_, u64>(self.offsets.len());

    Ok(explicit_index)
  }

  fn get(&self, idx: u64) -> Result<Option<crate::traversal::FieldType<'a>>, Never> {
    let Ok(at) = console_core_number_conversion::index(idx);

    Ok(match self.offsets.get(at) {
      Some(off) => {
        let Ok(raw) = off.get();
        let result = raw.resolve_with_args::<T>(self.data, &self.args);
        let Ok(field) = crate::traversal::FieldType::offset(raw, result);

        Some(field)
      }
      None => None,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::FontRead;

  #[derive(Debug, PartialEq)]
  struct Record(u16);

  impl<'a> FontRead<'a> for Record {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
      data.read_at::<u16>(0).map(Self)
    }
  }

  #[test]
  fn nullable_offsets_preserve_missing_targets_and_read_faults() -> Result<(), Never> {
    let Ok(data) = FontData::new(&[0, 0x12, 0x34]);
    let Ok(null) = BigEndian::<Nullable<Offset16>>::new([0, 0]);
    let Ok(valid) = BigEndian::<Nullable<Offset16>>::new([0, 1]);
    let Ok(truncated) = BigEndian::<Nullable<Offset16>>::new([0, 2]);
    let Ok(outside) = BigEndian::<Nullable<Offset16>>::new([0, 5]);
    let offsets = [null, valid, truncated, outside];
    let Ok(array) = ArrayOfNullableOffsets::<Record>::new(&offsets, data, ());
    let Ok(targets) = array.iter();
    let Ok(length) = array.len();
    let Ok(empty) = array.is_empty();

    assert_eq!(length, 4);
    assert_eq!(empty, Emptiness::NonEmpty);
    assert_eq!(array.get(0), Ok(None));
    assert_eq!(array.get(1), Ok(Some(Record(0x1234))));
    assert_eq!(array.get(2), Err(ReadError::OutOfBounds));
    assert_eq!(array.get(3), Err(ReadError::OutOfBounds));
    assert_eq!(
      array.get(u64::MAX),
      Err(ReadError::InvalidArrayIndex(u64::MAX))
    );
    assert!(targets.eq([
      Ok(None),
      Ok(Some(Record(0x1234))),
      Err(ReadError::OutOfBounds),
      Err(ReadError::OutOfBounds)
    ]));

    Ok(())
  }

  #[test]
  fn required_offsets_report_null_and_bad_indices() -> Result<(), Never> {
    let Ok(data) = FontData::new(&[0, 0x12, 0x34]);
    let Ok(null) = BigEndian::<Offset16>::new([0, 0]);
    let Ok(valid) = BigEndian::<Offset16>::new([0, 1]);
    let offsets = [null, valid];
    let Ok(array) = ArrayOfOffsets::<Record>::new(&offsets, data, ());
    let Ok(targets) = array.iter();
    let Ok(nullable) = array.iter_as_nullable();

    assert_eq!(array.get(0), Err(ReadError::NullOffset));
    assert_eq!(array.get(1), Ok(Record(0x1234)));
    assert_eq!(array.get(2), Err(ReadError::InvalidArrayIndex(2)));
    assert!(targets.eq([Err(ReadError::NullOffset), Ok(Record(0x1234))]));
    assert!(nullable.eq([Ok(None), Ok(Some(Record(0x1234)))]));

    Ok(())
  }
}
