

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for MetadataTable<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(data_maps_byte_range) = self.data_maps_byte_range();

        Ok(0..data_maps_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for MetadataTable<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"meta"); value });
}

impl<'a> FontRead<'a> for MetadataTable<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct MetadataTable<'a> {
    data: FontData<'a>,
}

impl<'a> MetadataTable<'a> {
    pub const MIN_SIZE: u64 =
        (u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<u32, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn flags(&self) -> Result<u32, Never> {
        let Ok(range) = self.flags_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn data_maps_count(&self) -> Result<u32, Never> {
        let Ok(range) = self.data_maps_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn data_maps(&self) -> Result<&'a [DataMapRecord], Never> {
        let Ok(range) = self.data_maps_byte_range();

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

    pub fn reserved_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.flags_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn data_maps_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.reserved_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn data_maps_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(data_maps_count) = self.data_maps_count();
        let Ok(previous) = self.data_maps_count_byte_range();
        let start = previous.end;
        let Ok(value_2) = transforms::to_u64(data_maps_count);

        let end = start.saturating_add(value_2.saturating_mul(DataMapRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(MetadataTable::MIN_SIZE); held }));

impl Default for MetadataTable<'_> {
    fn default() -> Self {
        let Ok(value_3) = FontData::default_table_data();

        Self {
            data: value_3,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for MetadataTable<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Meta")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "flags", value: FieldType::from(self.flags()) }),
            2u64 => Some(Field { name: "data_maps_count", value: FieldType::from(self.data_maps_count()) }),
            3u64 => {
                let Ok(value_4) = self.offset_data();

                Some(Field { name: "data_maps", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(DataMapRecord),
                    { let Ok(explicit_value) = self.data_maps(); explicit_value },
                    value_4,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for MetadataTable<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct DataMapRecord {
    pub tag: BigEndian<Tag>,
    pub data_offset: BigEndian<Offset32>,
    pub data_length: BigEndian<u32>,
}

const _: () = {
  assert!(core::mem::size_of::<DataMapRecord>() == 12);
  assert!(core::mem::align_of::<DataMapRecord>() == 1);
  assert!(core::mem::offset_of!(DataMapRecord, tag) == 0);
  assert!(core::mem::offset_of!(DataMapRecord, data_offset) == 4);
  assert!(core::mem::offset_of!(DataMapRecord, data_length) == 8);
};

impl DataMapRecord {
    pub fn tag(&self) -> Result<Tag, Never> {
        self.tag.get()
    }

    pub fn data_offset(&self) -> Result<Offset32, Never> {
        self.data_offset.get()
    }

    pub fn data<'a>(&self, data: FontData<'a>) -> Result<Metadata<'a>, ReadError> {
        let Ok(tag) = self.tag();
        let Ok(data_length) = self.data_length();
        let args = (tag, data_length);
        let Ok(data_offset) = self.data_offset();

        data_offset.resolve_with_args(data, &args)
    }

    pub fn data_length(&self) -> Result<u32, Never> {
        self.data_length.get()
    }
}

impl FixedSize for DataMapRecord {
    const RAW_BYTE_LEN: u64 = Tag::RAW_BYTE_LEN + Offset32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for DataMapRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "DataMapRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "tag", value: FieldType::from(self.tag()) }),
                1u64 => Some(Field { name: "data_offset", value: traversal::FieldType::Unknown }),
                2u64 => Some(Field { name: "data_length", value: FieldType::from(self.data_length()) }),
                _ => None,
            }),
            data,
        })
    }
}
