

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Cvar<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(tuple_variation_headers_byte_range) = self.tuple_variation_headers_byte_range();

        Ok(0..tuple_variation_headers_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Cvar<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"cvar"); value });
}

impl<'a> FontRead<'a> for Cvar<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Cvar<'a> {
    data: FontData<'a>,
}

impl<'a> Cvar<'a> {
    pub const MIN_SIZE: u64 =
        (MajorMinor::RAW_BYTE_LEN + TupleVariationCount::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<MajorMinor, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn tuple_variation_count(&self) -> Result<TupleVariationCount, Never> {
        let Ok(range) = self.tuple_variation_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn data_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.data_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn data(&self) -> Result<FontData<'a>, ReadError> {
        let data = self.data;
        let Ok(data_offset) = self.data_offset();

        data_offset.resolve(data)
    }

    pub fn tuple_variation_headers(&self) -> Result<VarLenArray<'a, TupleVariationHeader<'a>>, Never> {
        let Ok(range) = self.tuple_variation_headers_byte_range();

        self.data.read_var_array_or_empty(range.start)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(MajorMinor::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn tuple_variation_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(TupleVariationCount::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn data_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.tuple_variation_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn tuple_variation_headers_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.data_offset_byte_range();
        let start = previous.end;
        let Ok(value_3) = self.data.len();

        let end = start.saturating_add(value_3.saturating_sub(start));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Cvar::MIN_SIZE); held }));

impl Default for Cvar<'_> {
    fn default() -> Self {
        let Ok(value_2) = FontData::default_table_data();

        Self {
            data: value_2,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Cvar<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Cvar")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "tuple_variation_count", value: traversal::FieldType::Unknown }),
            2u64 => Some(Field { name: "data_offset", value: traversal::FieldType::Unknown }),
            3u64 => Some(Field { name: "tuple_variation_headers", value: traversal::FieldType::Unknown }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Cvar<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
