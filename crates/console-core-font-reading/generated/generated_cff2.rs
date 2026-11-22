

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Cff2Header<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(trailing_data_byte_range) = self.trailing_data_byte_range();

        Ok(0..trailing_data_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Cff2Header<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Cff2Header<'a> {
    data: FontData<'a>,
}

impl<'a> Cff2Header<'a> {
    pub const MIN_SIZE: u64 =
        (u8::RAW_BYTE_LEN + u8::RAW_BYTE_LEN + u8::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn major_version(&self) -> Result<u8, Never> {
        let Ok(range) = self.major_version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn minor_version(&self) -> Result<u8, Never> {
        let Ok(range) = self.minor_version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn header_size(&self) -> Result<u8, Never> {
        let Ok(range) = self.header_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn top_dict_length(&self) -> Result<u16, Never> {
        let Ok(range) = self.top_dict_length_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn _padding(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self._padding_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn top_dict_data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.top_dict_data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn trailing_data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.trailing_data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn major_version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn minor_version_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.major_version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn header_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.minor_version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn top_dict_length_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.header_size_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn _padding_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(header_size) = self.header_size();
        let Ok(previous) = self.top_dict_length_byte_range();
        let start = previous.end;
        let Ok(value_2) = transforms::subtract(header_size, 5_u64);

        let end =
            start.saturating_add(value_2.saturating_mul(u8::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn top_dict_data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(top_dict_length) = self.top_dict_length();
        let Ok(previous) = self._padding_byte_range();
        let start = previous.end;
        let Ok(value_3) = transforms::to_u64(top_dict_length);

        let end = start.saturating_add(value_3.saturating_mul(u8::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn trailing_data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.top_dict_data_byte_range();
        let start = previous.end;
        let Ok(value_8) = self.data.len();

        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_8.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Cff2Header::MIN_SIZE); held }));

impl Default for Cff2Header<'_> {
    fn default() -> Self {
        let Ok(value_4) = FontData::default_table_data();

        Self {
            data: value_4,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Cff2Header<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Cff2Header")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "major_version", value: FieldType::from(self.major_version()) }),
            1u64 => Some(Field { name: "minor_version", value: FieldType::from(self.minor_version()) }),
            2u64 => Some(Field { name: "header_size", value: FieldType::from(self.header_size()) }),
            3u64 => Some(Field { name: "top_dict_length", value: FieldType::from(self.top_dict_length()) }),
            4u64 => Some(Field { name: "_padding", value: FieldType::from(self._padding()) }),
            5u64 => Some(Field { name: "top_dict_data", value: FieldType::from(self.top_dict_data()) }),
            6u64 => Some(Field { name: "trailing_data", value: FieldType::from(self.trailing_data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Cff2Header<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for Index<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(data_byte_range) = self.data_byte_range();

        Ok(0..data_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Index<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_5) = data.len();

        match ::core::primitive::bool::then_some(value_5 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Index<'a> {
    data: FontData<'a>,
}

impl<'a> Index<'a> {
    pub const MIN_SIZE: u64 = (u32::RAW_BYTE_LEN + u8::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn count(&self) -> Result<u32, Never> {
        let Ok(range) = self.count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn off_size(&self) -> Result<u8, Never> {
        let Ok(range) = self.off_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn offsets(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn off_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(count) = self.count();
        let Ok(off_size) = self.off_size();
        let Ok(previous) = self.off_size_byte_range();
        let start = previous.end;
        let Ok(value_6) = transforms::add_multiply(count, 1_u64, off_size);

        let end = start.saturating_add(value_6.saturating_mul(u8::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.offsets_byte_range();
        let start = previous.end;
        let Ok(value_9) = self.data.len();

        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_9.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Index::MIN_SIZE); held }));

impl Default for Index<'_> {
    fn default() -> Self {
        let Ok(value_7) = FontData::default_table_data();

        Self {
            data: value_7,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Index<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Index")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "count", value: FieldType::from(self.count()) }),
            1u64 => Some(Field { name: "off_size", value: FieldType::from(self.off_size()) }),
            2u64 => Some(Field { name: "offsets", value: FieldType::from(self.offsets()) }),
            3u64 => Some(Field { name: "data", value: FieldType::from(self.data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Index<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
