

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Mvar<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(value_records_byte_range) = self.value_records_byte_range();

        Ok(0..value_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Mvar<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"MVAR"); value });
}

impl<'a> FontRead<'a> for Mvar<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Mvar<'a> {
    data: FontData<'a>,
}

impl<'a> Mvar<'a> {
    pub const MIN_SIZE: u64 = (MajorMinor::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<MajorMinor, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn value_record_size(&self) -> Result<u16, Never> {
        let Ok(range) = self.value_record_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn value_record_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.value_record_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn item_variation_store_offset(&self) -> Result<Nullable<Offset16>, Never> {
        let Ok(range) = self.item_variation_store_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn item_variation_store(&self) -> Result<Option<ItemVariationStore<'a>>, ReadError> {
        let data = self.data;
        let Ok(item_variation_store_offset) = self.item_variation_store_offset();

        item_variation_store_offset.resolve(data)
    }

    pub fn value_records(&self) -> Result<&'a [ValueRecord], Never> {
        let Ok(range) = self.value_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(MajorMinor::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn _reserved_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn value_record_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self._reserved_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn value_record_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.value_record_size_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn item_variation_store_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.value_record_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn value_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(value_record_count) = self.value_record_count();
        let Ok(previous) = self.item_variation_store_offset_byte_range();
        let start = previous.end;
        let Ok(value_2) = transforms::to_u64(value_record_count);

        let end = start.saturating_add(value_2.saturating_mul(ValueRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Mvar::MIN_SIZE); held }));

impl Default for Mvar<'_> {
    fn default() -> Self {
        let Ok(value_3) = FontData::default_table_data();

        Self {
            data: value_3,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Mvar<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Mvar")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "value_record_size", value: FieldType::from(self.value_record_size()) }),
            2u64 => Some(Field { name: "value_record_count", value: FieldType::from(self.value_record_count()) }),
            3u64 => Some(Field { name: "item_variation_store_offset", value: FieldType::from(FieldType::offset(
                    self.item_variation_store_offset(),
                    self.item_variation_store(),
                )) }),
            4u64 => {
                let Ok(value_4) = self.offset_data();

                Some(Field { name: "value_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(ValueRecord),
                    { let Ok(explicit_value) = self.value_records(); explicit_value },
                    value_4,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Mvar<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct ValueRecord {
    pub value_tag: BigEndian<Tag>,
    pub delta_set_outer_index: BigEndian<u16>,
    pub delta_set_inner_index: BigEndian<u16>,
}

const _: () = {
  assert!(core::mem::size_of::<ValueRecord>() == 8);
  assert!(core::mem::align_of::<ValueRecord>() == 1);
  assert!(core::mem::offset_of!(ValueRecord, value_tag) == 0);
  assert!(core::mem::offset_of!(ValueRecord, delta_set_outer_index) == 4);
  assert!(core::mem::offset_of!(ValueRecord, delta_set_inner_index) == 6);
};

impl ValueRecord {
    pub fn value_tag(&self) -> Result<Tag, Never> {
        self.value_tag.get()
    }

    pub fn delta_set_outer_index(&self) -> Result<u16, Never> {
        self.delta_set_outer_index.get()
    }

    pub fn delta_set_inner_index(&self) -> Result<u16, Never> {
        self.delta_set_inner_index.get()
    }
}

impl FixedSize for ValueRecord {
    const RAW_BYTE_LEN: u64 = Tag::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for ValueRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "ValueRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "value_tag", value: FieldType::from(self.value_tag()) }),
                1u64 => Some(Field { name: "delta_set_outer_index", value: FieldType::from(self.delta_set_outer_index()) }),
                2u64 => Some(Field { name: "delta_set_inner_index", value: FieldType::from(self.delta_set_inner_index()) }),
                _ => None,
            }),
            data,
        })
    }
}
