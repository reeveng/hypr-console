

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Eblc<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(bitmap_sizes_byte_range) = self.bitmap_sizes_byte_range();

        Ok(0..bitmap_sizes_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Eblc<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"EBLC"); value });
}

impl<'a> FontRead<'a> for Eblc<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Eblc<'a> {
    data: FontData<'a>,
}

impl<'a> Eblc<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn major_version(&self) -> Result<u16, Never> {
        let Ok(range) = self.major_version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn minor_version(&self) -> Result<u16, Never> {
        let Ok(range) = self.minor_version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_sizes(&self) -> Result<u32, Never> {
        let Ok(range) = self.num_sizes_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn bitmap_sizes(&self) -> Result<&'a [BitmapSize], Never> {
        let Ok(range) = self.bitmap_sizes_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn major_version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn minor_version_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.major_version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_sizes_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.minor_version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn bitmap_sizes_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_sizes) = self.num_sizes();
        let Ok(previous) = self.num_sizes_byte_range();
        let start = previous.end;
        let Ok(value_2) = transforms::to_u64(num_sizes);

        let end =
            start.saturating_add(value_2.saturating_mul(BitmapSize::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Eblc::MIN_SIZE); held }));

impl Default for Eblc<'_> {
    fn default() -> Self {
        let Ok(value_3) = FontData::default_table_data();

        Self {
            data: value_3,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Eblc<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Eblc")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "major_version", value: FieldType::from(self.major_version()) }),
            1u64 => Some(Field { name: "minor_version", value: FieldType::from(self.minor_version()) }),
            2u64 => Some(Field { name: "num_sizes", value: FieldType::from(self.num_sizes()) }),
            3u64 => {
                let Ok(value_4) = self.offset_data();

                Some(Field { name: "bitmap_sizes", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(BitmapSize),
                    { let Ok(explicit_value) = self.bitmap_sizes(); explicit_value },
                    value_4,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Eblc<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
