

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Hvar<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(rsb_mapping_offset_byte_range) = self.rsb_mapping_offset_byte_range();

        Ok(0..rsb_mapping_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Hvar<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"HVAR"); value });
}

impl<'a> FontRead<'a> for Hvar<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Hvar<'a> {
    data: FontData<'a>,
}

impl<'a> Hvar<'a> {
    pub const MIN_SIZE: u64 = (MajorMinor::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<MajorMinor, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn item_variation_store_offset(&self) -> Result<Offset32, Never> {
        let Ok(range) = self.item_variation_store_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn item_variation_store(&self) -> Result<ItemVariationStore<'a>, ReadError> {
        let data = self.data;
        let Ok(item_variation_store_offset) = self.item_variation_store_offset();

        item_variation_store_offset.resolve(data)
    }

    pub fn advance_width_mapping_offset(&self) -> Result<Nullable<Offset32>, Never> {
        let Ok(range) = self.advance_width_mapping_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn advance_width_mapping(&self) -> Result<Option<DeltaSetIndexMap<'a>>, ReadError> {
        let data = self.data;
        let Ok(advance_width_mapping_offset) = self.advance_width_mapping_offset();

        advance_width_mapping_offset.resolve(data)
    }

    pub fn lsb_mapping_offset(&self) -> Result<Nullable<Offset32>, Never> {
        let Ok(range) = self.lsb_mapping_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn lsb_mapping(&self) -> Result<Option<DeltaSetIndexMap<'a>>, ReadError> {
        let data = self.data;
        let Ok(lsb_mapping_offset) = self.lsb_mapping_offset();

        lsb_mapping_offset.resolve(data)
    }

    pub fn rsb_mapping_offset(&self) -> Result<Nullable<Offset32>, Never> {
        let Ok(range) = self.rsb_mapping_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn rsb_mapping(&self) -> Result<Option<DeltaSetIndexMap<'a>>, ReadError> {
        let data = self.data;
        let Ok(rsb_mapping_offset) = self.rsb_mapping_offset();

        rsb_mapping_offset.resolve(data)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(MajorMinor::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn item_variation_store_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn advance_width_mapping_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.item_variation_store_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn lsb_mapping_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.advance_width_mapping_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn rsb_mapping_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.lsb_mapping_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Hvar::MIN_SIZE); held }));

impl Default for Hvar<'_> {
    fn default() -> Self {
        let Ok(value_2) = FontData::default_table_data();

        Self {
            data: value_2,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Hvar<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Hvar")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "item_variation_store_offset", value: FieldType::from(FieldType::offset(
                    self.item_variation_store_offset(),
                    (self.item_variation_store()).map(Some),
                )) }),
            2u64 => Some(Field { name: "advance_width_mapping_offset", value: FieldType::from(FieldType::offset(
                    self.advance_width_mapping_offset(),
                    self.advance_width_mapping(),
                )) }),
            3u64 => Some(Field { name: "lsb_mapping_offset", value: FieldType::from(FieldType::offset(self.lsb_mapping_offset(), self.lsb_mapping())) }),
            4u64 => Some(Field { name: "rsb_mapping_offset", value: FieldType::from(FieldType::offset(self.rsb_mapping_offset(), self.rsb_mapping())) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Hvar<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
