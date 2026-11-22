

use crate::codegen_prelude::*;

impl Format<u16> for Table1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for Table1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(flex_byte_range) = self.flex_byte_range();

        Ok(0..flex_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Table1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Table1<'a> {
    data: FontData<'a>,
}

impl<'a> Table1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u32::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn heft(&self) -> Result<u32, Never> {
        let Ok(range) = self.heft_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn flex(&self) -> Result<u16, Never> {
        let Ok(range) = self.flex_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn heft_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn flex_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.heft_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Table1::MIN_SIZE); held }));

impl Default for Table1<'_> {
    fn default() -> Self {
        let Ok(value_2) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_2,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Table1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Table1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "heft", value: FieldType::from(self.heft()) }),
            2u64 => Some(Field { name: "flex", value: FieldType::from(self.flex()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Table1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for Table2<'_> {
    const FORMAT: u16 = 2;
}

impl<'a> MinByteRange<'a> for Table2<'a> {
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

impl<'a> FontRead<'a> for Table2<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_3) = data.len();

        match ::core::primitive::bool::then_some(value_3 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Table2<'a> {
    data: FontData<'a>,
}

impl<'a> Table2<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn value_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.value_count_byte_range();

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

    pub fn value_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn values_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(value_count) = self.value_count();
        let Ok(previous) = self.value_count_byte_range();
        let start = previous.end;
        let Ok(value_4) = transforms::to_u64(value_count);

        let end = start.saturating_add(value_4.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Table2<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Table2")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "value_count", value: FieldType::from(self.value_count()) }),
            2u64 => Some(Field { name: "values", value: FieldType::from(self.values()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Table2<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for Table3<'_> {
    const FORMAT: u16 = 3;
}

impl<'a> MinByteRange<'a> for Table3<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(something_byte_range) = self.something_byte_range();

        Ok(0..something_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Table3<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_5) = data.len();

        match ::core::primitive::bool::then_some(value_5 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Table3<'a> {
    data: FontData<'a>,
}

impl<'a> Table3<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn something(&self) -> Result<u16, Never> {
        let Ok(range) = self.something_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn something_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Table3<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Table3")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "something", value: FieldType::from(self.something()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Table3<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone)]
pub enum MyTable<'a> {
    Format1(Table1<'a>),
    MyFormat22(Table2<'a>),
    Format3(Table3<'a>),
}

impl Default for MyTable<'_> {
    fn default() -> Self {
        Self::Format1(Default::default())
    }
}

impl<'a> MyTable<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Format1(item) => item.offset_data(),
            Self::MyFormat22(item) => item.offset_data(),
            Self::Format3(item) => item.offset_data(),
        }
    }

    pub fn format(&self) -> Result<u16, Never> {
        match self {
            Self::Format1(item) => item.format(),
            Self::MyFormat22(item) => item.format(),
            Self::Format3(item) => item.format(),
        }
    }
}

impl<'a> FontRead<'a> for MyTable<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u16 = data.read_at(0u64)?;

        match format {
            Table1::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format1(read))
            }
            Table2::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::MyFormat22(read))
            }
            Table3::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format3(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u16>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for MyTable<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        match self {
            Self::Format1(item) => item.min_byte_range(),
            Self::MyFormat22(item) => item.min_byte_range(),
            Self::Format3(item) => item.min_byte_range(),
        }
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        match self {
            Self::Format1(item) => item.min_table_bytes(),
            Self::MyFormat22(item) => item.min_table_bytes(),
            Self::Format3(item) => item.min_table_bytes(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> MyTable<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Format1(table) => table,
            Self::MyFormat22(table) => table,
            Self::Format3(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for MyTable<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for MyTable<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}
