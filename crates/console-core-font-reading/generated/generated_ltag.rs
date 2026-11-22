

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Ltag<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(tag_ranges_byte_range) = self.tag_ranges_byte_range();

        Ok(0..tag_ranges_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Ltag<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"ltag"); value });
}

impl<'a> FontRead<'a> for Ltag<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Ltag<'a> {
    data: FontData<'a>,
}

impl<'a> Ltag<'a> {
    pub const MIN_SIZE: u64 = (u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<u32, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn flags(&self) -> Result<u32, Never> {
        let Ok(range) = self.flags_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_tags(&self) -> Result<u32, Never> {
        let Ok(range) = self.num_tags_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn tag_ranges(&self) -> Result<&'a [FTStringRange], Never> {
        let Ok(range) = self.tag_ranges_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn flags_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_tags_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.flags_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn tag_ranges_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_tags) = self.num_tags();
        let Ok(previous) = self.num_tags_byte_range();
        let start = previous.end;
        let Ok(value_2) = transforms::to_u64(num_tags);

        let end =
            start.saturating_add(value_2.saturating_mul(FTStringRange::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Ltag::MIN_SIZE); held }));

impl Default for Ltag<'_> {
    fn default() -> Self {
        let Ok(value_3) = FontData::default_table_data();

        Self {
            data: value_3,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Ltag<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Ltag")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "flags", value: FieldType::from(self.flags()) }),
            2u64 => Some(Field { name: "num_tags", value: FieldType::from(self.num_tags()) }),
            3u64 => {
                let Ok(value_4) = self.offset_data();

                Some(Field { name: "tag_ranges", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(FTStringRange),
                    { let Ok(explicit_value) = self.tag_ranges(); explicit_value },
                    value_4,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Ltag<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct FTStringRange {
    pub offset: BigEndian<u16>,
    pub length: BigEndian<u16>,
}

const _: () = {
  assert!(core::mem::size_of::<FTStringRange>() == 4);
  assert!(core::mem::align_of::<FTStringRange>() == 1);
  assert!(core::mem::offset_of!(FTStringRange, offset) == 0);
  assert!(core::mem::offset_of!(FTStringRange, length) == 2);
};

impl FTStringRange {
    pub fn offset(&self) -> Result<u16, Never> {
        self.offset.get()
    }

    pub fn length(&self) -> Result<u16, Never> {
        self.length.get()
    }
}

impl FixedSize for FTStringRange {
    const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for FTStringRange {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "FTStringRange",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "offset", value: FieldType::from(self.offset()) }),
                1u64 => Some(Field { name: "length", value: FieldType::from(self.length()) }),
                _ => None,
            }),
            data,
        })
    }
}
