

use crate::codegen_prelude::*;

impl Discriminant for MyLookup<'_, ()> {
    fn read_discriminant(data: FontData<'_>) -> Result<u16, ReadError> {
        data.read_at(0)
    }
}

impl<'a, T> MinByteRange<'a> for MyLookup<'a, T> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(subtable_offsets_byte_range) = self.subtable_offsets_byte_range();

        Ok(0..subtable_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a, T> FontRead<'a> for MyLookup<'a, T> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self {
            data,
            offset_type: std::marker::PhantomData,
        })
    }
}

impl<'a, T> MyLookup<'a, T> {
    #[cfg(test)]
    pub(crate) fn of_unit_type(&self) -> Result<MyLookup<'a, ()>, Never> {
        Ok(MyLookup {
            data: self.data,
            offset_type: std::marker::PhantomData,
        })
    }

}

#[derive(Clone)]
pub struct MyLookup<'a, T = ()> {
    data: FontData<'a>,
    offset_type: std::marker::PhantomData<*const T>,
}

impl<'a, T> MyLookup<'a, T> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn lookup_type(&self) -> Result<u16, Never> {
        let Ok(range) = self.lookup_type_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn sub_table_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.sub_table_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn subtable_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.subtable_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn subtables(&self) -> Result<ArrayOfOffsets<'a, T, Offset16>, Never>
    where
        T: FontRead<'a>,
    {
        let data = self.data;
        let Ok(offsets) = self.subtable_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn lookup_type_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn sub_table_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.lookup_type_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn subtable_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(sub_table_count) = self.sub_table_count();
        let Ok(previous) = self.sub_table_count_byte_range();
        let start = previous.end;
        let Ok(value_2) = transforms::to_u64(sub_table_count);

        let end =
            start.saturating_add(value_2.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(MyLookup::<()>::MIN_SIZE); held }));

impl<T> Default for MyLookup<'_, T> {
    fn default() -> Self {
        let Ok(value_3) = FontData::default_table_data();

        Self {
            data: value_3,
            offset_type: std::marker::PhantomData,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a, T: FontRead<'a> + SomeTable<'a> + 'a> SomeTable<'a> for MyLookup<'a, T> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("MyLookup")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "lookup_type", value: FieldType::from(self.lookup_type()) }),
            1u64 => Some(Field { name: "sub_table_count", value: FieldType::from(self.sub_table_count()) }),
            2u64 => Some(Field { name: "subtable_offsets", value: FieldType::from(self.subtables()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a, T: FontRead<'a> + SomeTable<'a> + 'a> std::fmt::Debug for MyLookup<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone)]
pub enum MySubtable<'a> {
    Format1(MySubtableFormat1<'a>),
    Format2(MySubtableFormat2<'a>),
}

impl Default for MySubtable<'_> {
    fn default() -> Self {
        Self::Format1(Default::default())
    }
}

impl<'a> MySubtable<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Format1(item) => item.offset_data(),
            Self::Format2(item) => item.offset_data(),
        }
    }

    pub fn format(&self) -> Result<u16, Never> {
        match self {
            Self::Format1(item) => item.format(),
            Self::Format2(item) => item.format(),
        }
    }
}

impl<'a> FontRead<'a> for MySubtable<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u16 = data.read_at(0u64)?;

        match format {
            MySubtableFormat1::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format1(read))
            }
            MySubtableFormat2::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format2(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u16>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for MySubtable<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        match self {
            Self::Format1(item) => item.min_byte_range(),
            Self::Format2(item) => item.min_byte_range(),
        }
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        match self {
            Self::Format1(item) => item.min_table_bytes(),
            Self::Format2(item) => item.min_table_bytes(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> MySubtable<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Format1(table) => table,
            Self::Format2(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for MySubtable<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for MySubtable<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}

impl Format<u16> for MySubtableFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for MySubtableFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(value_byte_range) = self.value_byte_range();

        Ok(0..value_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for MySubtableFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_6) = data.len();

        match ::core::primitive::bool::then_some(value_6 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct MySubtableFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> MySubtableFormat1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn value(&self) -> Result<u16, Never> {
        let Ok(range) = self.value_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn value_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    MySubtableFormat1::MIN_SIZE
); held }));

impl Default for MySubtableFormat1<'_> {
    fn default() -> Self {
        let Ok(value_7) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_7,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for MySubtableFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("MySubtableFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "value", value: FieldType::from(self.value()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for MySubtableFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for MySubtableFormat2<'_> {
    const FORMAT: u16 = 2;
}

impl<'a> MinByteRange<'a> for MySubtableFormat2<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(values_byte_range) = self.values_byte_range();

        Ok(0..values_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for MySubtableFormat2<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_8) = data.len();

        match ::core::primitive::bool::then_some(value_8 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct MySubtableFormat2<'a> {
    data: FontData<'a>,
}

impl<'a> MySubtableFormat2<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn count(&self) -> Result<u16, Never> {
        let Ok(range) = self.count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn values(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.values_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn values_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(count) = self.count();
        let Ok(previous) = self.count_byte_range();
        let start = previous.end;
        let Ok(value_9) = transforms::to_u64(count);

        let end = start.saturating_add(value_9.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for MySubtableFormat2<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("MySubtableFormat2")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "count", value: FieldType::from(self.count()) }),
            2u64 => Some(Field { name: "values", value: FieldType::from(self.values()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for MySubtableFormat2<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

pub enum MyLookupGroup<'a> {
    TypeOne(MyLookup<'a, MySubtable<'a>>),
    TypeTwo(MyLookup<'a, MySubtableFormat1<'a>>),
}

impl Default for MyLookupGroup<'_> {
    fn default() -> Self {
        Self::TypeOne(Default::default())
    }
}

impl<'a> FontRead<'a> for MyLookupGroup<'a> {
    fn read(bytes: FontData<'a>) -> Result<Self, ReadError> {
        let discriminant = MyLookup::read_discriminant(bytes)?;

        match discriminant {
            1 => {
                let read = FontRead::read(bytes)?;

                Ok(MyLookupGroup::TypeOne(read))
            }
            2 => {
                let read = FontRead::read(bytes)?;

                Ok(MyLookupGroup::TypeTwo(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u16>>::from(other))),
        }
    }
}

impl<'a> MyLookupGroup<'a> {
    #[cfg(test)]
    pub(crate) fn of_unit_type(&self) -> Result<MyLookup<'a, ()>, Never> {
        match self {
            MyLookupGroup::TypeOne(inner) => inner.of_unit_type(),
            MyLookupGroup::TypeTwo(inner) => inner.of_unit_type(),
        }
    }

}

#[cfg(feature = "experimental_traverse")]
impl<'a> MyLookupGroup<'a> {
    fn dyn_inner(&self) -> Result<&(dyn SomeTable<'a> + 'a), Never> {
        Ok(match self {
            MyLookupGroup::TypeOne(table) => table,
            MyLookupGroup::TypeTwo(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for MyLookupGroup<'a> {
    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }

    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for MyLookupGroup<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

impl<'a> MinByteRange<'a> for ContainsLookupGroup<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(lookup_offset_byte_range) = self.lookup_offset_byte_range();

        Ok(0..lookup_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ContainsLookupGroup<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_10) = data.len();

        match ::core::primitive::bool::then_some(value_10 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ContainsLookupGroup<'a> {
    data: FontData<'a>,
}

impl<'a> ContainsLookupGroup<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<u16, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn lookup_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.lookup_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn lookup(&self) -> Result<MyLookupGroup<'a>, ReadError> {
        let data = self.data;
        let Ok(lookup_offset) = self.lookup_offset();

        lookup_offset.resolve(data)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn lookup_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    ContainsLookupGroup::MIN_SIZE
); held }));

impl Default for ContainsLookupGroup<'_> {
    fn default() -> Self {
        let Ok(value_11) = FontData::default_table_data();

        Self {
            data: value_11,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ContainsLookupGroup<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ContainsLookupGroup")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "lookup_offset", value: FieldType::from(FieldType::offset(self.lookup_offset(), (self.lookup()).map(Some))) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ContainsLookupGroup<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
