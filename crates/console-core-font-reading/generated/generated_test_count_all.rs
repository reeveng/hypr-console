

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for CountAll16<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(remainder_byte_range) = self.remainder_byte_range();

        Ok(0..remainder_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for CountAll16<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct CountAll16<'a> {
    data: FontData<'a>,
}

impl<'a> CountAll16<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn some_field(&self) -> Result<u16, Never> {
        let Ok(range) = self.some_field_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn remainder(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.remainder_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn some_field_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn remainder_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.some_field_byte_range();
        let start = previous.end;
        let Ok(length) = self.data.len();
        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: length.saturating_sub(start), item_width: u16::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(CountAll16::MIN_SIZE); held }));

impl Default for CountAll16<'_> {
    fn default() -> Self {
        let Ok(value_2) = FontData::default_table_data();

        Self {
            data: value_2,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for CountAll16<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("CountAll16")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "some_field", value: FieldType::from(self.some_field()) }),
            1u64 => Some(Field { name: "remainder", value: FieldType::from(self.remainder()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for CountAll16<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for CountAll32<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(remainder_byte_range) = self.remainder_byte_range();

        Ok(0..remainder_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for CountAll32<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_3) = data.len();

        match ::core::primitive::bool::then_some(value_3 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct CountAll32<'a> {
    data: FontData<'a>,
}

impl<'a> CountAll32<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn some_field(&self) -> Result<u16, Never> {
        let Ok(range) = self.some_field_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn remainder(&self) -> Result<&'a [BigEndian<u32>], Never> {
        let Ok(range) = self.remainder_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn some_field_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn remainder_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.some_field_byte_range();
        let start = previous.end;
        let Ok(length) = self.data.len();
        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: length.saturating_sub(start), item_width: u32::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(CountAll32::MIN_SIZE); held }));

impl Default for CountAll32<'_> {
    fn default() -> Self {
        let Ok(value_4) = FontData::default_table_data();

        Self {
            data: value_4,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for CountAll32<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("CountAll32")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "some_field", value: FieldType::from(self.some_field()) }),
            1u64 => Some(Field { name: "remainder", value: FieldType::from(self.remainder()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for CountAll32<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
