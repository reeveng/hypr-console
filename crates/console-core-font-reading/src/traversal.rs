//! A table read without knowing its type: its name and its fields in the order the
//! specification lists them, each a scalar, an offset to another table, a record or an array.
//!
//! This is what printing a table for debugging walks. Every generated table and record answers
//! [`SomeTable`] or [`SomeRecord`], and an array of them answers [`SomeArray`], so a walk can step
//! from a table through its offsets into the tables they reach without naming any of them. A
//! field that a table's version does not have ends the walk of that table, as it did upstream.
//!
//! A getter answers `Result<T, Never>`, so a field is built from that answer directly: a field
//! value is anything that turns into a [`FieldType`], and an infallible answer turns into what
//! its value turns into. The module is behind `experimental_traverse`, as it was upstream, and
//! its shape is not promised.

use std::{fmt::Debug, ops::Deref};

use console_core_integer_sets::Emptiness;
use console_core_never::Never;
use types::{
  BigEndian, F2Dot14, FWord, Fixed, GlyphId16, GlyphId24, Int24, LongDateTime, MajorMinor,
  NameId, Nullable, Offset16, Offset24, Offset32, Scalar, Tag, UfWord, Uint24, Version16Dot16,
};

use crate::{
  FontData, FontRead, FontReadWithArgs, ReadError, VarSize,
  array::{ComputedArray, VarLenArray},
  read::{ComputeSize, ReadArgs},
};

pub enum FieldType<'a> {
  I8(i8),
  U8(u8),
  I16(i16),
  U16(u16),
  I32(i32),
  U32(u32),
  I24(Int24),
  U24(Uint24),
  Tag(Tag),
  FWord(FWord),
  UfWord(UfWord),
  MajorMinor(MajorMinor),
  Version16Dot16(Version16Dot16),
  F2Dot14(F2Dot14),
  Fixed(Fixed),
  LongDateTime(LongDateTime),
  GlyphId16(GlyphId16),
  GlyphId24(GlyphId24),
  NameId(NameId),
  BareOffset(OffsetType),
  ResolvedOffset(ResolvedOffset<'a>),
  StringOffset(StringOffset<'a>),
  ArrayOffset(ArrayOffset<'a>),
  Record(RecordResolver<'a>),
  Array(Box<dyn SomeArray<'a> + 'a>),
  ReadError(ReadError),
  Unknown,
}

#[derive(Clone, Copy)]
pub enum OffsetType {
  Offset16(u16),
  Offset24(Uint24),
  Offset32(u32),
}

impl OffsetType {
  pub fn to_u32(self) -> Result<u32, Never> {
    Ok(match self {
      Self::Offset16(val) => {
        <::core::primitive::u32 as ::core::convert::From<::core::primitive::u16>>::from(val)
      }
      Self::Offset24(val) => {
        <::core::primitive::u32 as ::core::convert::From<::font_types::Uint24>>::from(val)
      }
      Self::Offset32(val) => val,
    })
  }
}

pub struct ResolvedOffset<'a> {
  pub offset: OffsetType,
  pub target: Result<Box<dyn SomeTable<'a> + 'a>, ReadError>,
}

pub struct StringOffset<'a> {
  pub offset: OffsetType,
  pub target: Result<Box<dyn SomeString<'a> + 'a>, ReadError>,
}

pub struct ArrayOffset<'a> {
  pub offset: OffsetType,
  pub target: Result<Box<dyn SomeArray<'a> + 'a>, ReadError>,
}

impl<'a> FieldType<'a> {
  pub fn array_of_records<T>(
    type_name: &'static str,
    records: &'a [T],
    data: FontData<'a>,
  ) -> Result<FieldType<'a>, Never>
  where
    T: Clone + SomeRecord<'a> + 'a,
  {
    Ok(FieldType::from(ArrayOfRecords {
      type_name,
      data,
      records,
    }))
  }

  pub fn computed_array<T>(
    type_name: &'static str,
    array: ComputedArray<'a, T>,
    data: FontData<'a>,
  ) -> Result<FieldType<'a>, Never>
  where
    T: FontReadWithArgs<'a> + ComputeSize + SomeRecord<'a> + 'a,
    T::Arguments: Copy + 'static,
  {
    Ok(FieldType::from(ComputedArrayOfRecords {
      type_name,
      data,
      array,
    }))
  }

  pub fn var_array<T>(
    type_name: &'static str,
    array: VarLenArray<'a, T>,
    data: FontData<'a>,
  ) -> Result<FieldType<'a>, Never>
  where
    T: FontRead<'a> + VarSize + SomeRecord<'a> + 'a,
  {
    Ok(FieldType::from(VarLenArrayOfRecords {
      type_name,
      data,
      array,
    }))
  }

  pub fn offset_to_array_of_scalars<T: SomeArray<'a> + 'a>(
    offset: impl Into<OffsetType>,
    result: impl Into<Result<Option<T>, ReadError>>,
  ) -> Result<Self, Never> {
    let offset_2 = <_ as ::core::convert::Into<crate::traversal::OffsetType>>::into(offset);

    Ok(
      match ::core::convert::Into::<Result<Option<T>, ReadError>>::into(result) {
        Ok(Some(target)) => FieldType::ArrayOffset(ArrayOffset {
          offset: offset_2,
          target: Ok(Box::new(target)),
        }),
        Ok(None) => FieldType::BareOffset(offset_2),
        Err(error) => FieldType::ArrayOffset(ArrayOffset {
          offset: offset_2,
          target: Err(error),
        }),
      },
    )
  }

  pub fn offset_to_array_of_records<T: Clone + SomeRecord<'a> + 'a>(
    offset: impl Into<OffsetType>,
    result: impl Into<Result<Option<&'a [T]>, ReadError>>,
    type_name: &'static str,
    data: FontData<'a>,
  ) -> Result<Self, Never> {
    let offset_2 = <_ as ::core::convert::Into<crate::traversal::OffsetType>>::into(offset);

    Ok(
      match ::core::convert::Into::<Result<Option<&'a [T]>, ReadError>>::into(result) {
        Ok(Some(records)) => {
          let array = ArrayOfRecords {
            type_name,
            data,
            records,
          };

          FieldType::ArrayOffset(ArrayOffset {
            offset: offset_2,
            target: Ok(Box::new(array)),
          })
        }
        Ok(None) => FieldType::BareOffset(offset_2),
        Err(error) => FieldType::ArrayOffset(ArrayOffset {
          offset: offset_2,
          target: Err(error),
        }),
      },
    )
  }

  pub fn offset<T: SomeTable<'a> + 'a>(
    offset: impl Into<OffsetType>,
    result: impl Into<Result<Option<T>, ReadError>>,
  ) -> Result<Self, Never> {
    let offset_2 = <_ as ::core::convert::Into<crate::traversal::OffsetType>>::into(offset);

    Ok(
      match ::core::convert::Into::<Result<Option<T>, ReadError>>::into(result) {
        Ok(Some(target)) => FieldType::ResolvedOffset(ResolvedOffset {
          offset: offset_2,
          target: Ok(Box::new(target)),
        }),
        Ok(None) => FieldType::BareOffset(offset_2),
        Err(error) => FieldType::ResolvedOffset(ResolvedOffset {
          offset: offset_2,
          target: Err(error),
        }),
      },
    )
  }

  pub fn unknown_offset(offset: impl Into<OffsetType>) -> Result<Self, Never> {
    Ok(Self::BareOffset(<_ as ::core::convert::Into<
      crate::traversal::OffsetType,
    >>::into(offset)))
  }
}

pub struct Field<'a> {
  pub name: &'static str,
  pub value: FieldType<'a>,
}

pub trait SomeTable<'a> {
  fn type_name(&self) -> Result<&str, Never>;

  fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never>;
}

impl<'a> dyn SomeTable<'a> + 'a {
  pub fn iter(&self) -> Result<impl Iterator<Item = Field<'a>> + '_, Never> {
    Ok(FieldIter {
      table: self,
      idx: 0,
    })
  }
}

struct FieldIter<'a, 'b> {
  table: &'b dyn SomeTable<'a>,
  idx: u64,
}

impl<'a> Iterator for FieldIter<'a, '_> {
  type Item = Field<'a>;

  fn next(&mut self) -> Option<Self::Item> {
    let this = self.idx;

    self.idx = self.idx.saturating_add(1);

    let Ok(field) = self.table.get_field(this);

    field
  }
}

impl<'a> SomeTable<'a> for Box<dyn SomeTable<'a> + 'a> {
  fn type_name(&self) -> Result<&str, Never> {
    self.deref().type_name()
  }

  fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
    self.deref().get_field(idx)
  }
}

pub trait SomeRecord<'a> {
  fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never>;
}

pub struct RecordResolver<'a> {
  pub(crate) name: &'static str,
  pub(crate) get_field: Box<dyn Fn(u64, FontData<'a>) -> Option<Field<'a>> + 'a>,
  pub(crate) data: FontData<'a>,
}

pub trait SomeArray<'a> {
  fn type_name(&self) -> Result<&str, Never>;

  fn len(&self) -> Result<u64, Never>;

  fn is_empty(&self) -> Result<Emptiness, Never> {
    let Ok(length) = self.len();

    Ok(match length {
      0 => Emptiness::Empty,
      _items => Emptiness::NonEmpty,
    })
  }

  fn get(&self, idx: u64) -> Result<Option<FieldType<'a>>, Never>;
}

impl<'a> dyn SomeArray<'a> + 'a {
  pub fn iter(&self) -> Result<impl Iterator<Item = FieldType<'a>> + '_, Never> {
    Ok(ArrayIter {
      array: self,
      idx: 0,
    })
  }
}

struct ArrayIter<'a, 'b> {
  array: &'b dyn SomeArray<'a>,
  idx: u64,
}

impl<'a> Iterator for ArrayIter<'a, '_> {
  type Item = FieldType<'a>;

  fn next(&mut self) -> Option<Self::Item> {
    let this = self.idx;

    self.idx = self.idx.saturating_add(1);

    let Ok(item) = self.array.get(this);

    item
  }
}

impl<'a, T: Scalar + Into<FieldType<'a>>> SomeArray<'a> for &'a [BigEndian<T>]
where
  BigEndian<T>: Copy,
{
  fn len(&self) -> Result<u64, Never> {
    console_core_number_conversion::fitted((*self).len())
  }

  fn get(&self, idx: u64) -> Result<Option<FieldType<'a>>, Never> {
    let Ok(position) = console_core_number_conversion::index(idx);

    Ok((*self).get(position).map(|val| {
      let Ok(value) = val.get();

      <_ as ::core::convert::Into<crate::traversal::FieldType<'_>>>::into(value)
    }))
  }

  fn type_name(&self) -> Result<&str, Never> {
    let full_name = std::any::type_name::<T>();

    Ok(match full_name.split("::").last() {
      Some(last) => last,
      None => full_name,
    })
  }
}

impl<'a> SomeArray<'a> for &'a [u8] {
  fn type_name(&self) -> Result<&str, Never> {
    Ok("u8")
  }

  fn len(&self) -> Result<u64, Never> {
    console_core_number_conversion::fitted((*self).len())
  }

  fn get(&self, idx: u64) -> Result<Option<FieldType<'a>>, Never> {
    let Ok(position) = console_core_number_conversion::index(idx);

    Ok((*self).get(position).copied().map(Into::into))
  }
}

impl<'a> SomeArray<'a> for Box<dyn SomeArray<'a> + 'a> {
  fn type_name(&self) -> Result<&str, Never> {
    self.deref().type_name()
  }

  fn len(&self) -> Result<u64, Never> {
    self.deref().len()
  }

  fn get(&self, idx: u64) -> Result<Option<FieldType<'a>>, Never> {
    self.deref().get(idx)
  }
}

pub trait SomeString<'a> {
  fn iter_chars(&self) -> Result<Box<dyn Iterator<Item = char> + 'a>, Never>;
}

impl<'a> SomeString<'a> for Box<dyn SomeString<'a> + 'a> {
  fn iter_chars(&self) -> Result<Box<dyn Iterator<Item = char> + 'a>, Never> {
    self.deref().iter_chars()
  }
}

struct ArrayOfRecords<'a, T> {
  pub(crate) type_name: &'static str,
  pub(crate) data: FontData<'a>,
  pub(crate) records: &'a [T],
}

struct ComputedArrayOfRecords<'a, T: ReadArgs> {
  pub(crate) type_name: &'static str,
  pub(crate) data: FontData<'a>,
  pub(crate) array: ComputedArray<'a, T>,
}

struct VarLenArrayOfRecords<'a, T> {
  pub(crate) type_name: &'static str,
  pub(crate) data: FontData<'a>,
  pub(crate) array: VarLenArray<'a, T>,
}

impl<'a, T> SomeArray<'a> for ComputedArrayOfRecords<'a, T>
where
  T: FontReadWithArgs<'a> + ComputeSize + SomeRecord<'a> + 'a,
  T::Arguments: Copy + 'static,
  Self: 'a,
{
  fn len(&self) -> Result<u64, Never> {
    self.array.len()
  }

  fn get(&self, idx: u64) -> Result<Option<FieldType<'a>>, Never> {
    Ok(match self.array.get(idx) {
      Ok(record) => {
        let Ok(resolver) = record.traverse(self.data);

        Some(<crate::traversal::FieldType<'_> as ::core::convert::From<
          crate::traversal::RecordResolver<'_>,
        >>::from(resolver))
      }
      Err(_beyond_the_array) => None,
    })
  }

  fn type_name(&self) -> Result<&str, Never> {
    Ok(self.type_name)
  }
}

impl<'a, T: SomeRecord<'a> + Clone> SomeArray<'a> for ArrayOfRecords<'a, T> {
  fn type_name(&self) -> Result<&str, Never> {
    Ok(self.type_name)
  }

  fn len(&self) -> Result<u64, Never> {
    console_core_number_conversion::fitted(self.records.len())
  }

  fn get(&self, idx: u64) -> Result<Option<FieldType<'a>>, Never> {
    let Ok(position) = console_core_number_conversion::index(idx);

    Ok(self.records.get(position).map(|record| {
      let Ok(resolver) = record.clone().traverse(self.data);

      <crate::traversal::FieldType<'_> as ::core::convert::From<
        crate::traversal::RecordResolver<'_>,
      >>::from(resolver)
    }))
  }
}

impl<'a, T> SomeArray<'a> for VarLenArrayOfRecords<'a, T>
where
  T: FontRead<'a> + VarSize + SomeRecord<'a> + 'a,
  Self: 'a,
{
  fn len(&self) -> Result<u64, Never> {
    let Ok(records) = self.array.iter();

    console_core_number_conversion::fitted(records.count())
  }

  fn get(&self, idx: u64) -> Result<Option<FieldType<'a>>, Never> {
    Ok(match self.array.get(idx) {
      Ok(Some(record)) => {
        let Ok(resolver) = record.traverse(self.data);

        Some(<crate::traversal::FieldType<'_> as ::core::convert::From<
          crate::traversal::RecordResolver<'_>,
        >>::from(resolver))
      }
      Ok(None) => None,
      Err(error) => Some(FieldType::ReadError(error)),
    })
  }

  fn type_name(&self) -> Result<&str, Never> {
    Ok(self.type_name)
  }
}

impl<'a> Field<'a> {
  pub fn from_optional<T: Into<FieldType<'a>>>(
    name: &'static str,
    value: Result<Option<T>, ReadError>,
  ) -> Result<Option<Self>, Never> {
    Ok(match value {
      Ok(Some(value_2)) => Some(Field {
        name,
        value: <_ as ::core::convert::Into<crate::traversal::FieldType<'_>>>::into(value_2),
      }),
      Ok(None) => None,
      Err(error) => Some(Field {
        name,
        value: FieldType::ReadError(error),
      }),
    })
  }

  pub fn new(name: &'static str, value: impl Into<FieldType<'a>>) -> Result<Self, Never> {
    Ok(Field {
      name,
      value: <_ as ::core::convert::Into<crate::traversal::FieldType<'_>>>::into(value),
    })
  }
}

struct DebugPrintTable<'a, 'b>(pub &'b (dyn SomeTable<'a> + 'a));

struct DebugPrintArray<'a, 'b>(pub &'b (dyn SomeArray<'a> + 'a));

impl<'a> Debug for FieldType<'a> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::I8(arg0) => arg0.fmt(f),
      Self::U8(arg0) => arg0.fmt(f),
      Self::I16(arg0) => arg0.fmt(f),
      Self::U16(arg0) => arg0.fmt(f),
      Self::I32(arg0) => arg0.fmt(f),
      Self::U32(arg0) => arg0.fmt(f),
      Self::I24(arg0) => arg0.fmt(f),
      Self::U24(arg0) => arg0.fmt(f),
      Self::Tag(arg0) => arg0.fmt(f),
      Self::FWord(arg0) => {
        let Ok(units) = arg0.to_i16();

        units.fmt(f)
      }
      Self::UfWord(arg0) => {
        let Ok(units) = arg0.to_u16();

        units.fmt(f)
      }
      Self::MajorMinor(arg0) => write!(f, "{}.{}", arg0.major, arg0.minor),
      Self::Version16Dot16(arg0) => arg0.fmt(f),
      Self::F2Dot14(arg0) => arg0.fmt(f),
      Self::Fixed(arg0) => arg0.fmt(f),
      Self::LongDateTime(arg0) => {
        let Ok(seconds) = arg0.as_secs();

        seconds.fmt(f)
      }
      Self::GlyphId16(arg0) => {
        write!(f, "g")?;

        let Ok(glyph) = arg0.to_u16();

        glyph.fmt(f)
      }
      Self::GlyphId24(arg0) => {
        write!(f, "g")?;

        let Ok(glyph) = arg0.to_u32();

        glyph.fmt(f)
      }
      Self::NameId(arg0) => arg0.fmt(f),
      Self::StringOffset(string) => match &string.target {
        Ok(arg0) => arg0.as_ref().fmt(f),
        Err(_unreadable_string) => string.target.fmt(f),
      },
      Self::ArrayOffset(array) => match &array.target {
        Ok(arg0) => arg0.as_ref().fmt(f),
        Err(_unreadable_array) => array.target.fmt(f),
      },
      Self::BareOffset(arg0) => {
        let Ok(offset) = arg0.to_u32();

        write!(f, "0x{offset:04X}")
      }
      Self::ResolvedOffset(ResolvedOffset {
        target: Ok(arg0), ..
      }) => arg0.fmt(f),
      Self::ResolvedOffset(arg0) => arg0.target.fmt(f),
      Self::Record(arg0) => {
        let table: &(dyn SomeTable<'a> + 'a) = arg0;

        table.fmt(f)
      }
      Self::Array(arg0) => arg0.fmt(f),
      Self::ReadError(error) => error.fmt(f),
      Self::Unknown => write!(f, "no repr available"),
    }
  }
}

impl std::fmt::Debug for DebugPrintTable<'_, '_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    let Ok(name) = self.0.type_name();
    let mut debug_struct = f.debug_struct(name);
    let Ok(fields) = self.0.iter();

    for field in fields {
      let _updated_fields = debug_struct.field(field.name, &field.value);
    }

    debug_struct.finish()
  }
}

impl<'a> Debug for dyn SomeTable<'a> + 'a {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    DebugPrintTable(self).fmt(f)
  }
}

impl<'a> Debug for dyn SomeString<'a> + 'a {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "\"")?;

    let Ok(chars) = self.iter_chars();

    for c in chars {
      write!(f, "{c}")?;
    }

    write!(f, "\"")
  }
}

impl std::fmt::Debug for DebugPrintArray<'_, '_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    let mut debug_list = f.debug_list();
    let Ok(items) = self.0.iter();

    for item in items {
      let _updated_items = debug_list.entry(&item);
    }

    debug_list.finish()
  }
}

impl<'a> Debug for dyn SomeArray<'a> + 'a {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    DebugPrintArray(self).fmt(f)
  }
}

impl std::fmt::Display for OffsetType {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    let Ok(offset) = self.to_u32();

    write!(f, "{offset:+}")
  }
}

impl<'a> SomeTable<'a> for RecordResolver<'a> {
  fn type_name(&self) -> Result<&str, Never> {
    Ok(self.name)
  }

  fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
    Ok((self.get_field)(idx, self.data))
  }
}

impl<'a, T: Into<FieldType<'a>>> From<Result<T, Never>> for FieldType<'a> {
  fn from(src: Result<T, Never>) -> FieldType<'a> {
    let Ok(value) = src;

    <_ as ::core::convert::Into<crate::traversal::FieldType<'_>>>::into(value)
  }
}

impl<'a> From<u8> for FieldType<'a> {
  fn from(src: u8) -> FieldType<'a> {
    FieldType::U8(src)
  }
}

impl<'a> From<i8> for FieldType<'a> {
  fn from(src: i8) -> FieldType<'a> {
    FieldType::I8(src)
  }
}

impl<'a> From<u16> for FieldType<'a> {
  fn from(src: u16) -> FieldType<'a> {
    FieldType::U16(src)
  }
}

impl<'a> From<i16> for FieldType<'a> {
  fn from(src: i16) -> FieldType<'a> {
    FieldType::I16(src)
  }
}

impl<'a> From<u32> for FieldType<'a> {
  fn from(src: u32) -> FieldType<'a> {
    FieldType::U32(src)
  }
}

impl<'a> From<i32> for FieldType<'a> {
  fn from(src: i32) -> FieldType<'a> {
    FieldType::I32(src)
  }
}

impl<'a> From<Uint24> for FieldType<'a> {
  fn from(src: Uint24) -> FieldType<'a> {
    FieldType::U24(src)
  }
}

impl<'a> From<Int24> for FieldType<'a> {
  fn from(src: Int24) -> FieldType<'a> {
    FieldType::I24(src)
  }
}

impl<'a> From<Tag> for FieldType<'a> {
  fn from(src: Tag) -> FieldType<'a> {
    FieldType::Tag(src)
  }
}

impl<'a> From<FWord> for FieldType<'a> {
  fn from(src: FWord) -> FieldType<'a> {
    FieldType::FWord(src)
  }
}

impl<'a> From<UfWord> for FieldType<'a> {
  fn from(src: UfWord) -> FieldType<'a> {
    FieldType::UfWord(src)
  }
}

impl<'a> From<Fixed> for FieldType<'a> {
  fn from(src: Fixed) -> FieldType<'a> {
    FieldType::Fixed(src)
  }
}

impl<'a> From<F2Dot14> for FieldType<'a> {
  fn from(src: F2Dot14) -> FieldType<'a> {
    FieldType::F2Dot14(src)
  }
}

impl<'a> From<LongDateTime> for FieldType<'a> {
  fn from(src: LongDateTime) -> FieldType<'a> {
    FieldType::LongDateTime(src)
  }
}

impl<'a> From<MajorMinor> for FieldType<'a> {
  fn from(src: MajorMinor) -> FieldType<'a> {
    FieldType::MajorMinor(src)
  }
}

impl<'a> From<Version16Dot16> for FieldType<'a> {
  fn from(src: Version16Dot16) -> FieldType<'a> {
    FieldType::Version16Dot16(src)
  }
}

impl<'a> From<GlyphId16> for FieldType<'a> {
  fn from(src: GlyphId16) -> FieldType<'a> {
    FieldType::GlyphId16(src)
  }
}

impl<'a> From<GlyphId24> for FieldType<'a> {
  fn from(src: GlyphId24) -> FieldType<'a> {
    FieldType::GlyphId24(src)
  }
}

impl<'a> From<NameId> for FieldType<'a> {
  fn from(src: NameId) -> FieldType<'a> {
    FieldType::NameId(src)
  }
}

impl<'a> From<RecordResolver<'a>> for FieldType<'a> {
  fn from(src: RecordResolver<'a>) -> Self {
    FieldType::Record(src)
  }
}

impl<'a, T: SomeArray<'a> + 'a> From<T> for FieldType<'a> {
  fn from(src: T) -> Self {
    FieldType::Array(Box::new(src))
  }
}

impl<T: Into<OffsetType>> From<Result<T, Never>> for OffsetType {
  fn from(src: Result<T, Never>) -> OffsetType {
    let Ok(offset) = src;

    <_ as ::core::convert::Into<crate::traversal::OffsetType>>::into(offset)
  }
}

impl From<Offset16> for OffsetType {
  fn from(src: Offset16) -> OffsetType {
    let Ok(offset) = src.to_u32();
    let Ok(offset_2) = console_core_number_conversion::wrapped::<_, u16>(offset);

    OffsetType::Offset16(offset_2)
  }
}

impl From<Offset24> for OffsetType {
  fn from(src: Offset24) -> OffsetType {
    let Ok(offset) = src.to_u32();
    let Ok(offset_2) = Uint24::new(offset);

    OffsetType::Offset24(offset_2)
  }
}

impl From<Offset32> for OffsetType {
  fn from(src: Offset32) -> OffsetType {
    let Ok(offset) = src.to_u32();

    OffsetType::Offset32(offset)
  }
}

impl<'a> From<Offset16> for FieldType<'a> {
  fn from(src: Offset16) -> FieldType<'a> {
    FieldType::BareOffset(<crate::traversal::OffsetType as ::core::convert::From<
      ::font_types::Offset16,
    >>::from(src))
  }
}

impl<'a> From<Offset24> for FieldType<'a> {
  fn from(src: Offset24) -> FieldType<'a> {
    FieldType::BareOffset(<crate::traversal::OffsetType as ::core::convert::From<
      ::font_types::Offset24,
    >>::from(src))
  }
}

impl<'a> From<Offset32> for FieldType<'a> {
  fn from(src: Offset32) -> FieldType<'a> {
    FieldType::BareOffset(<crate::traversal::OffsetType as ::core::convert::From<
      ::font_types::Offset32,
    >>::from(src))
  }
}

impl<T: Into<OffsetType> + Clone> From<Nullable<T>> for OffsetType {
  fn from(src: Nullable<T>) -> Self {
    let Ok(offset) = src.offset();

    ::core::convert::Into::<OffsetType>::into(offset.clone())
  }
}
