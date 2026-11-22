

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Ebdt<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(minor_version_byte_range) = self.minor_version_byte_range();

        Ok(0..minor_version_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Ebdt<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"EBDT"); value });
}

impl<'a> FontRead<'a> for Ebdt<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Ebdt<'a> {
    data: FontData<'a>,
}

impl<'a> Ebdt<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn major_version(&self) -> Result<u16, Never> {
        let Ok(range) = self.major_version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn minor_version(&self) -> Result<u16, Never> {
        let Ok(range) = self.minor_version_byte_range();

        self.data.read_at_or_zero(range.start)
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
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Ebdt::MIN_SIZE); held }));

impl Default for Ebdt<'_> {
    fn default() -> Self {
        let Ok(value_2) = FontData::default_table_data();

        Self {
            data: value_2,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Ebdt<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Ebdt")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "major_version", value: FieldType::from(self.major_version()) }),
            1u64 => Some(Field { name: "minor_version", value: FieldType::from(self.minor_version()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Ebdt<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
