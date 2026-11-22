

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Morx<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(chains_byte_range) = self.chains_byte_range();

        Ok(0..chains_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Morx<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"morx"); value });
}

impl<'a> FontRead<'a> for Morx<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Morx<'a> {
    data: FontData<'a>,
}

impl<'a> Morx<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<u16, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn n_chains(&self) -> Result<u32, Never> {
        let Ok(range) = self.n_chains_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn chains(&self) -> Result<VarLenArray<'a, Chain<'a>>, Never> {
        let Ok(range) = self.chains_byte_range();

        self.data.read_var_array_or_empty(range.start)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn unused_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn n_chains_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.unused_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn chains_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(n_chains) = self.n_chains();
        let Ok(previous) = self.n_chains_byte_range();
        let start = previous.end;
        let end = start.saturating_add({ let Ok(explicit_value) = transforms::total_len_for_count::<Chain, _>(self.data, start, n_chains); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Morx::MIN_SIZE); held }));

impl Default for Morx<'_> {
    fn default() -> Self {
        let Ok(value_3) = FontData::default_table_data();

        Self {
            data: value_3,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Morx<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Morx")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "n_chains", value: FieldType::from(self.n_chains()) }),
            2u64 => {
                let Ok(value_4) = self.offset_data();

                let Ok(chains) = self.chains();

                Some(Field { name: "chains", value: FieldType::from(traversal::FieldType::var_array("Chain", chains, value_4)) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Morx<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for Chain<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(subtables_byte_range) = self.subtables_byte_range();

        Ok(0..subtables_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Chain<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_5) = data.len();

        match ::core::primitive::bool::then_some(value_5 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Chain<'a> {
    data: FontData<'a>,
}

impl<'a> Chain<'a> {
    pub const MIN_SIZE: u64 =
        (u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn default_flags(&self) -> Result<u32, Never> {
        let Ok(range) = self.default_flags_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn chain_length(&self) -> Result<u32, Never> {
        let Ok(range) = self.chain_length_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn n_feature_entries(&self) -> Result<u32, Never> {
        let Ok(range) = self.n_feature_entries_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn n_subtables(&self) -> Result<u32, Never> {
        let Ok(range) = self.n_subtables_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn features(&self) -> Result<&'a [Feature], Never> {
        let Ok(range) = self.features_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn subtables(&self) -> Result<VarLenArray<'a, Subtable<'a>>, Never> {
        let Ok(range) = self.subtables_byte_range();

        self.data.read_var_array_or_empty(range.start)
    }

    pub fn default_flags_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn chain_length_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.default_flags_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn n_feature_entries_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.chain_length_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn n_subtables_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.n_feature_entries_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn features_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(n_feature_entries) = self.n_feature_entries();
        let Ok(previous) = self.n_subtables_byte_range();
        let start = previous.end;
        let Ok(value_6) = transforms::to_u64(n_feature_entries);

        let end =
            start.saturating_add(value_6.saturating_mul(Feature::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn subtables_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(n_subtables) = self.n_subtables();
        let Ok(previous) = self.features_byte_range();
        let start = previous.end;
        let end = start.saturating_add({ let Ok(explicit_value) = transforms::total_len_for_count::<Subtable, _>(self.data, start, n_subtables); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Chain::MIN_SIZE); held }));

impl Default for Chain<'_> {
    fn default() -> Self {
        let Ok(value_8) = FontData::default_table_data();

        Self {
            data: value_8,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Chain<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Chain")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "default_flags", value: FieldType::from(self.default_flags()) }),
            1u64 => Some(Field { name: "chain_length", value: FieldType::from(self.chain_length()) }),
            2u64 => Some(Field { name: "n_feature_entries", value: FieldType::from(self.n_feature_entries()) }),
            3u64 => Some(Field { name: "n_subtables", value: FieldType::from(self.n_subtables()) }),
            4u64 => {
                let Ok(value_9) = self.offset_data();

                Some(Field { name: "features", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(Feature),
                    { let Ok(explicit_value) = self.features(); explicit_value },
                    value_9,
                )) })
            },
            5u64 => {
                let Ok(value_10) = self.offset_data();

                let Ok(subtables) = self.subtables();

                Some(Field { name: "subtables", value: FieldType::from(traversal::FieldType::var_array("Subtable", subtables, value_10)) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Chain<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct Feature {
    pub feature_type: BigEndian<u16>,
    pub feature_settings: BigEndian<u16>,
    pub enable_flags: BigEndian<u32>,
    pub disable_flags: BigEndian<u32>,
}

const _: () = {
  assert!(core::mem::size_of::<Feature>() == 12);
  assert!(core::mem::align_of::<Feature>() == 1);
  assert!(core::mem::offset_of!(Feature, feature_type) == 0);
  assert!(core::mem::offset_of!(Feature, feature_settings) == 2);
  assert!(core::mem::offset_of!(Feature, enable_flags) == 4);
  assert!(core::mem::offset_of!(Feature, disable_flags) == 8);
};

impl Feature {
    pub fn feature_type(&self) -> Result<u16, Never> {
        self.feature_type.get()
    }

    pub fn feature_settings(&self) -> Result<u16, Never> {
        self.feature_settings.get()
    }

    pub fn enable_flags(&self) -> Result<u32, Never> {
        self.enable_flags.get()
    }

    pub fn disable_flags(&self) -> Result<u32, Never> {
        self.disable_flags.get()
    }
}

impl FixedSize for Feature {
    const RAW_BYTE_LEN: u64 =
        u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for Feature {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "Feature",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "feature_type", value: FieldType::from(self.feature_type()) }),
                1u64 => Some(Field { name: "feature_settings", value: FieldType::from(self.feature_settings()) }),
                2u64 => Some(Field { name: "enable_flags", value: FieldType::from(self.enable_flags()) }),
                3u64 => Some(Field { name: "disable_flags", value: FieldType::from(self.disable_flags()) }),
                _ => None,
            }),
            data,
        })
    }
}

impl<'a> MinByteRange<'a> for Subtable<'a> {
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

impl<'a> FontRead<'a> for Subtable<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_15) = data.len();

        match ::core::primitive::bool::then_some(value_15 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Subtable<'a> {
    data: FontData<'a>,
}

impl<'a> Subtable<'a> {
    pub const MIN_SIZE: u64 = (u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn length(&self) -> Result<u32, Never> {
        let Ok(range) = self.length_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage(&self) -> Result<u32, Never> {
        let Ok(range) = self.coverage_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn sub_feature_flags(&self) -> Result<u32, Never> {
        let Ok(range) = self.sub_feature_flags_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn length_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coverage_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.length_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn sub_feature_flags_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.sub_feature_flags_byte_range();
        let start = previous.end;
        let Ok(value_21) = self.data.len();

        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_21.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Subtable::MIN_SIZE); held }));

impl Default for Subtable<'_> {
    fn default() -> Self {
        let Ok(value_16) = FontData::default_table_data();

        Self {
            data: value_16,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Subtable<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Subtable")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "length", value: FieldType::from(self.length()) }),
            1u64 => Some(Field { name: "coverage", value: FieldType::from(self.coverage()) }),
            2u64 => Some(Field { name: "sub_feature_flags", value: FieldType::from(self.sub_feature_flags()) }),
            3u64 => Some(Field { name: "data", value: FieldType::from(self.data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Subtable<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct ContextualEntryData {
    pub mark_index: BigEndian<u16>,
    pub current_index: BigEndian<u16>,
}

const _: () = {
  assert!(core::mem::size_of::<ContextualEntryData>() == 4);
  assert!(core::mem::align_of::<ContextualEntryData>() == 1);
  assert!(core::mem::offset_of!(ContextualEntryData, mark_index) == 0);
  assert!(core::mem::offset_of!(ContextualEntryData, current_index) == 2);
};

impl ContextualEntryData {
    pub fn mark_index(&self) -> Result<u16, Never> {
        self.mark_index.get()
    }

    pub fn current_index(&self) -> Result<u16, Never> {
        self.current_index.get()
    }
}

impl FixedSize for ContextualEntryData {
    const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for ContextualEntryData {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "ContextualEntryData",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "mark_index", value: FieldType::from(self.mark_index()) }),
                1u64 => Some(Field { name: "current_index", value: FieldType::from(self.current_index()) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct InsertionEntryData {
    pub current_insert_index: BigEndian<u16>,
    pub marked_insert_index: BigEndian<u16>,
}

const _: () = {
  assert!(core::mem::size_of::<InsertionEntryData>() == 4);
  assert!(core::mem::align_of::<InsertionEntryData>() == 1);
  assert!(core::mem::offset_of!(InsertionEntryData, current_insert_index) == 0);
  assert!(core::mem::offset_of!(InsertionEntryData, marked_insert_index) == 2);
};

impl InsertionEntryData {
    pub fn current_insert_index(&self) -> Result<u16, Never> {
        self.current_insert_index.get()
    }

    pub fn marked_insert_index(&self) -> Result<u16, Never> {
        self.marked_insert_index.get()
    }
}

impl FixedSize for InsertionEntryData {
    const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for InsertionEntryData {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "InsertionEntryData",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "current_insert_index", value: FieldType::from(self.current_insert_index()) }),
                1u64 => Some(Field { name: "marked_insert_index", value: FieldType::from(self.marked_insert_index()) }),
                _ => None,
            }),
            data,
        })
    }
}
