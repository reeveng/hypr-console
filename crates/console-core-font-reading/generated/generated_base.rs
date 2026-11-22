

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Base<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(vert_axis_offset_byte_range) = self.vert_axis_offset_byte_range();

        Ok(0..vert_axis_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Base<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"BASE"); value });
}

impl<'a> FontRead<'a> for Base<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Base<'a> {
    data: FontData<'a>,
}

impl<'a> Base<'a> {
    pub const MIN_SIZE: u64 =
        (MajorMinor::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<MajorMinor, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn horiz_axis_offset(&self) -> Result<Nullable<Offset16>, Never> {
        let Ok(range) = self.horiz_axis_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn horiz_axis(&self) -> Result<Option<Axis<'a>>, ReadError> {
        let data = self.data;
        let Ok(horiz_axis_offset) = self.horiz_axis_offset();

        horiz_axis_offset.resolve(data)
    }

    pub fn vert_axis_offset(&self) -> Result<Nullable<Offset16>, Never> {
        let Ok(range) = self.vert_axis_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn vert_axis(&self) -> Result<Option<Axis<'a>>, ReadError> {
        let data = self.data;
        let Ok(vert_axis_offset) = self.vert_axis_offset();

        vert_axis_offset.resolve(data)
    }

    pub fn item_var_store_offset(&self) -> Result<Option<Nullable<Offset32>>, Never> {
        let Ok(range) = self.item_var_store_offset_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn item_var_store(&self) -> Result<Option<ItemVariationStore<'a>>, ReadError> {
        let data = self.data;
        let Ok(read_item_var_store_offset) = self.item_var_store_offset();

        match read_item_var_store_offset { Some(x) => x.resolve(data), None => Ok(None) }
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(MajorMinor::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn horiz_axis_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn vert_axis_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.horiz_axis_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn item_var_store_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.vert_axis_offset_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility((1u16, 1u16));

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(Offset32::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Base::MIN_SIZE); held }));

impl Default for Base<'_> {
    fn default() -> Self {
        let Ok(value_3) = FontData::default_table_data();

        Self {
            data: value_3,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Base<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Base")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "horiz_axis_offset", value: FieldType::from(FieldType::offset(self.horiz_axis_offset(), self.horiz_axis())) }),
            2u64 => Some(Field { name: "vert_axis_offset", value: FieldType::from(FieldType::offset(self.vert_axis_offset(), self.vert_axis())) }),
            3u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility((1u16, 1u16));
                let Ok(item_var_store_offset) = self.item_var_store_offset();

                match compatibility {
                    Compatibility::Compatible => item_var_store_offset.map(|item_var_store_offset_2| Field { name: "item_var_store_offset", value: FieldType::from(FieldType::offset(item_var_store_offset_2, self.item_var_store())) }),
                    Compatibility::Incompatible => None,
                }
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Base<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for Axis<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(base_script_list_offset_byte_range) = self.base_script_list_offset_byte_range();

        Ok(0..base_script_list_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Axis<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_4) = data.len();

        match ::core::primitive::bool::then_some(value_4 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Axis<'a> {
    data: FontData<'a>,
}

impl<'a> Axis<'a> {
    pub const MIN_SIZE: u64 = (Offset16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn base_tag_list_offset(&self) -> Result<Nullable<Offset16>, Never> {
        let Ok(range) = self.base_tag_list_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn base_tag_list(&self) -> Result<Option<BaseTagList<'a>>, ReadError> {
        let data = self.data;
        let Ok(base_tag_list_offset) = self.base_tag_list_offset();

        base_tag_list_offset.resolve(data)
    }

    pub fn base_script_list_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.base_script_list_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn base_script_list(&self) -> Result<BaseScriptList<'a>, ReadError> {
        let data = self.data;
        let Ok(base_script_list_offset) = self.base_script_list_offset();

        base_script_list_offset.resolve(data)
    }

    pub fn base_tag_list_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn base_script_list_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.base_tag_list_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Axis::MIN_SIZE); held }));

impl Default for Axis<'_> {
    fn default() -> Self {
        let Ok(value_5) = FontData::default_table_data();

        Self {
            data: value_5,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Axis<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Axis")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "base_tag_list_offset", value: FieldType::from(FieldType::offset(self.base_tag_list_offset(), self.base_tag_list())) }),
            1u64 => Some(Field { name: "base_script_list_offset", value: FieldType::from(FieldType::offset(self.base_script_list_offset(), (self.base_script_list()).map(Some))) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Axis<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for BaseTagList<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(baseline_tags_byte_range) = self.baseline_tags_byte_range();

        Ok(0..baseline_tags_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for BaseTagList<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_6) = data.len();

        match ::core::primitive::bool::then_some(value_6 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct BaseTagList<'a> {
    data: FontData<'a>,
}

impl<'a> BaseTagList<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn base_tag_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.base_tag_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn baseline_tags(&self) -> Result<&'a [BigEndian<Tag>], Never> {
        let Ok(range) = self.baseline_tags_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn base_tag_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn baseline_tags_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(base_tag_count) = self.base_tag_count();
        let Ok(previous) = self.base_tag_count_byte_range();
        let start = previous.end;
        let Ok(value_7) = transforms::to_u64(base_tag_count);

        let end = start.saturating_add(value_7.saturating_mul(Tag::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(BaseTagList::MIN_SIZE); held }));

impl Default for BaseTagList<'_> {
    fn default() -> Self {
        let Ok(value_8) = FontData::default_table_data();

        Self {
            data: value_8,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for BaseTagList<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("BaseTagList")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "base_tag_count", value: FieldType::from(self.base_tag_count()) }),
            1u64 => Some(Field { name: "baseline_tags", value: FieldType::from(self.baseline_tags()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for BaseTagList<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for BaseScriptList<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(base_script_records_byte_range) = self.base_script_records_byte_range();

        Ok(0..base_script_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for BaseScriptList<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_9) = data.len();

        match ::core::primitive::bool::then_some(value_9 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct BaseScriptList<'a> {
    data: FontData<'a>,
}

impl<'a> BaseScriptList<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn base_script_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.base_script_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn base_script_records(&self) -> Result<&'a [BaseScriptRecord], Never> {
        let Ok(range) = self.base_script_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn base_script_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn base_script_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(base_script_count) = self.base_script_count();
        let Ok(previous) = self.base_script_count_byte_range();
        let start = previous.end;
        let Ok(value_10) = transforms::to_u64(base_script_count);

        let end = start.saturating_add(value_10
                .saturating_mul(BaseScriptRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(BaseScriptList::MIN_SIZE); held }));

impl Default for BaseScriptList<'_> {
    fn default() -> Self {
        let Ok(value_11) = FontData::default_table_data();

        Self {
            data: value_11,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for BaseScriptList<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("BaseScriptList")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "base_script_count", value: FieldType::from(self.base_script_count()) }),
            1u64 => {
                let Ok(value_12) = self.offset_data();

                Some(Field { name: "base_script_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(BaseScriptRecord),
                    { let Ok(explicit_value) = self.base_script_records(); explicit_value },
                    value_12,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for BaseScriptList<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct BaseScriptRecord {
    pub base_script_tag: BigEndian<Tag>,
    pub base_script_offset: BigEndian<Offset16>,
}

const _: () = {
  assert!(core::mem::size_of::<BaseScriptRecord>() == 6);
  assert!(core::mem::align_of::<BaseScriptRecord>() == 1);
  assert!(core::mem::offset_of!(BaseScriptRecord, base_script_tag) == 0);
  assert!(core::mem::offset_of!(BaseScriptRecord, base_script_offset) == 4);
};

impl BaseScriptRecord {
    pub fn base_script_tag(&self) -> Result<Tag, Never> {
        self.base_script_tag.get()
    }

    pub fn base_script_offset(&self) -> Result<Offset16, Never> {
        self.base_script_offset.get()
    }

    pub fn base_script<'a>(&self, data: FontData<'a>) -> Result<BaseScript<'a>, ReadError> {
        let Ok(base_script_offset) = self.base_script_offset();

        base_script_offset.resolve(data)
    }
}

impl FixedSize for BaseScriptRecord {
    const RAW_BYTE_LEN: u64 = Tag::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for BaseScriptRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "BaseScriptRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "base_script_tag", value: FieldType::from(self.base_script_tag()) }),
                1u64 => Some(Field { name: "base_script_offset", value: FieldType::from(FieldType::offset(self.base_script_offset(), (self.base_script(_data)).map(Some))) }),
                _ => None,
            }),
            data,
        })
    }
}

impl<'a> MinByteRange<'a> for BaseScript<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(base_lang_sys_records_byte_range) = self.base_lang_sys_records_byte_range();

        Ok(0..base_lang_sys_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for BaseScript<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_15) = data.len();

        match ::core::primitive::bool::then_some(value_15 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct BaseScript<'a> {
    data: FontData<'a>,
}

impl<'a> BaseScript<'a> {
    pub const MIN_SIZE: u64 =
        (Offset16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn base_values_offset(&self) -> Result<Nullable<Offset16>, Never> {
        let Ok(range) = self.base_values_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn base_values(&self) -> Result<Option<BaseValues<'a>>, ReadError> {
        let data = self.data;
        let Ok(base_values_offset) = self.base_values_offset();

        base_values_offset.resolve(data)
    }

    pub fn default_min_max_offset(&self) -> Result<Nullable<Offset16>, Never> {
        let Ok(range) = self.default_min_max_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn default_min_max(&self) -> Result<Option<MinMax<'a>>, ReadError> {
        let data = self.data;
        let Ok(default_min_max_offset) = self.default_min_max_offset();

        default_min_max_offset.resolve(data)
    }

    pub fn base_lang_sys_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.base_lang_sys_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn base_lang_sys_records(&self) -> Result<&'a [BaseLangSysRecord], Never> {
        let Ok(range) = self.base_lang_sys_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn base_values_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn default_min_max_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.base_values_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn base_lang_sys_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.default_min_max_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn base_lang_sys_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(base_lang_sys_count) = self.base_lang_sys_count();
        let Ok(previous) = self.base_lang_sys_count_byte_range();
        let start = previous.end;
        let Ok(value_16) = transforms::to_u64(base_lang_sys_count);

        let end = start.saturating_add(value_16
                .saturating_mul(BaseLangSysRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(BaseScript::MIN_SIZE); held }));

impl Default for BaseScript<'_> {
    fn default() -> Self {
        let Ok(value_17) = FontData::default_table_data();

        Self {
            data: value_17,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for BaseScript<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("BaseScript")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "base_values_offset", value: FieldType::from(FieldType::offset(self.base_values_offset(), self.base_values())) }),
            1u64 => Some(Field { name: "default_min_max_offset", value: FieldType::from(FieldType::offset(self.default_min_max_offset(), self.default_min_max())) }),
            2u64 => Some(Field { name: "base_lang_sys_count", value: FieldType::from(self.base_lang_sys_count()) }),
            3u64 => {
                let Ok(value_18) = self.offset_data();

                Some(Field { name: "base_lang_sys_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(BaseLangSysRecord),
                    { let Ok(explicit_value) = self.base_lang_sys_records(); explicit_value },
                    value_18,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for BaseScript<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct BaseLangSysRecord {
    pub base_lang_sys_tag: BigEndian<Tag>,
    pub min_max_offset: BigEndian<Offset16>,
}

const _: () = {
  assert!(core::mem::size_of::<BaseLangSysRecord>() == 6);
  assert!(core::mem::align_of::<BaseLangSysRecord>() == 1);
  assert!(core::mem::offset_of!(BaseLangSysRecord, base_lang_sys_tag) == 0);
  assert!(core::mem::offset_of!(BaseLangSysRecord, min_max_offset) == 4);
};

impl BaseLangSysRecord {
    pub fn base_lang_sys_tag(&self) -> Result<Tag, Never> {
        self.base_lang_sys_tag.get()
    }

    pub fn min_max_offset(&self) -> Result<Offset16, Never> {
        self.min_max_offset.get()
    }

    pub fn min_max<'a>(&self, data: FontData<'a>) -> Result<MinMax<'a>, ReadError> {
        let Ok(min_max_offset) = self.min_max_offset();

        min_max_offset.resolve(data)
    }
}

impl FixedSize for BaseLangSysRecord {
    const RAW_BYTE_LEN: u64 = Tag::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for BaseLangSysRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "BaseLangSysRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "base_lang_sys_tag", value: FieldType::from(self.base_lang_sys_tag()) }),
                1u64 => Some(Field { name: "min_max_offset", value: FieldType::from(FieldType::offset(self.min_max_offset(), (self.min_max(_data)).map(Some))) }),
                _ => None,
            }),
            data,
        })
    }
}

impl<'a> MinByteRange<'a> for BaseValues<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(base_coord_offsets_byte_range) = self.base_coord_offsets_byte_range();

        Ok(0..base_coord_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for BaseValues<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_21) = data.len();

        match ::core::primitive::bool::then_some(value_21 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct BaseValues<'a> {
    data: FontData<'a>,
}

impl<'a> BaseValues<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn default_baseline_index(&self) -> Result<u16, Never> {
        let Ok(range) = self.default_baseline_index_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn base_coord_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.base_coord_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn base_coord_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.base_coord_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn base_coords(&self) -> Result<ArrayOfOffsets<'a, BaseCoord<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.base_coord_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn default_baseline_index_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn base_coord_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.default_baseline_index_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn base_coord_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(base_coord_count) = self.base_coord_count();
        let Ok(previous) = self.base_coord_count_byte_range();
        let start = previous.end;
        let Ok(value_22) = transforms::to_u64(base_coord_count);

        let end =
            start.saturating_add(value_22.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(BaseValues::MIN_SIZE); held }));

impl Default for BaseValues<'_> {
    fn default() -> Self {
        let Ok(value_23) = FontData::default_table_data();

        Self {
            data: value_23,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for BaseValues<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("BaseValues")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "default_baseline_index", value: FieldType::from(self.default_baseline_index()) }),
            1u64 => Some(Field { name: "base_coord_count", value: FieldType::from(self.base_coord_count()) }),
            2u64 => Some(Field { name: "base_coord_offsets", value: FieldType::from(self.base_coords()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for BaseValues<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for MinMax<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(feat_min_max_records_byte_range) = self.feat_min_max_records_byte_range();

        Ok(0..feat_min_max_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for MinMax<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_24) = data.len();

        match ::core::primitive::bool::then_some(value_24 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct MinMax<'a> {
    data: FontData<'a>,
}

impl<'a> MinMax<'a> {
    pub const MIN_SIZE: u64 =
        (Offset16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn min_coord_offset(&self) -> Result<Nullable<Offset16>, Never> {
        let Ok(range) = self.min_coord_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn min_coord(&self) -> Result<Option<BaseCoord<'a>>, ReadError> {
        let data = self.data;
        let Ok(min_coord_offset) = self.min_coord_offset();

        min_coord_offset.resolve(data)
    }

    pub fn max_coord_offset(&self) -> Result<Nullable<Offset16>, Never> {
        let Ok(range) = self.max_coord_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn max_coord(&self) -> Result<Option<BaseCoord<'a>>, ReadError> {
        let data = self.data;
        let Ok(max_coord_offset) = self.max_coord_offset();

        max_coord_offset.resolve(data)
    }

    pub fn feat_min_max_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.feat_min_max_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn feat_min_max_records(&self) -> Result<&'a [FeatMinMaxRecord], Never> {
        let Ok(range) = self.feat_min_max_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn min_coord_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn max_coord_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.min_coord_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn feat_min_max_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.max_coord_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn feat_min_max_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(feat_min_max_count) = self.feat_min_max_count();
        let Ok(previous) = self.feat_min_max_count_byte_range();
        let start = previous.end;
        let Ok(value_25) = transforms::to_u64(feat_min_max_count);

        let end = start.saturating_add(value_25
                .saturating_mul(FeatMinMaxRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(MinMax::MIN_SIZE); held }));

impl Default for MinMax<'_> {
    fn default() -> Self {
        let Ok(value_26) = FontData::default_table_data();

        Self {
            data: value_26,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for MinMax<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("MinMax")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "min_coord_offset", value: FieldType::from(FieldType::offset(self.min_coord_offset(), self.min_coord())) }),
            1u64 => Some(Field { name: "max_coord_offset", value: FieldType::from(FieldType::offset(self.max_coord_offset(), self.max_coord())) }),
            2u64 => Some(Field { name: "feat_min_max_count", value: FieldType::from(self.feat_min_max_count()) }),
            3u64 => {
                let Ok(value_27) = self.offset_data();

                Some(Field { name: "feat_min_max_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(FeatMinMaxRecord),
                    { let Ok(explicit_value) = self.feat_min_max_records(); explicit_value },
                    value_27,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for MinMax<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct FeatMinMaxRecord {
    pub feature_table_tag: BigEndian<Tag>,
    pub min_coord_offset: BigEndian<Nullable<Offset16>>,
    pub max_coord_offset: BigEndian<Nullable<Offset16>>,
}

const _: () = {
  assert!(core::mem::size_of::<FeatMinMaxRecord>() == 8);
  assert!(core::mem::align_of::<FeatMinMaxRecord>() == 1);
  assert!(core::mem::offset_of!(FeatMinMaxRecord, feature_table_tag) == 0);
  assert!(core::mem::offset_of!(FeatMinMaxRecord, min_coord_offset) == 4);
  assert!(core::mem::offset_of!(FeatMinMaxRecord, max_coord_offset) == 6);
};

impl FeatMinMaxRecord {
    pub fn feature_table_tag(&self) -> Result<Tag, Never> {
        self.feature_table_tag.get()
    }

    pub fn min_coord_offset(&self) -> Result<Nullable<Offset16>, Never> {
        self.min_coord_offset.get()
    }

    pub fn min_coord<'a>(&self, data: FontData<'a>) -> Result<Option<BaseCoord<'a>>, ReadError> {
        let Ok(min_coord_offset) = self.min_coord_offset();

        min_coord_offset.resolve(data)
    }

    pub fn max_coord_offset(&self) -> Result<Nullable<Offset16>, Never> {
        self.max_coord_offset.get()
    }

    pub fn max_coord<'a>(&self, data: FontData<'a>) -> Result<Option<BaseCoord<'a>>, ReadError> {
        let Ok(max_coord_offset) = self.max_coord_offset();

        max_coord_offset.resolve(data)
    }
}

impl FixedSize for FeatMinMaxRecord {
    const RAW_BYTE_LEN: u64 = Tag::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for FeatMinMaxRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "FeatMinMaxRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "feature_table_tag", value: FieldType::from(self.feature_table_tag()) }),
                1u64 => Some(Field { name: "min_coord_offset", value: FieldType::from(FieldType::offset(self.min_coord_offset(), self.min_coord(_data))) }),
                2u64 => Some(Field { name: "max_coord_offset", value: FieldType::from(FieldType::offset(self.max_coord_offset(), self.max_coord(_data))) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone)]
pub enum BaseCoord<'a> {
    Format1(BaseCoordFormat1<'a>),
    Format2(BaseCoordFormat2<'a>),
    Format3(BaseCoordFormat3<'a>),
}

impl Default for BaseCoord<'_> {
    fn default() -> Self {
        Self::Format1(Default::default())
    }
}

impl<'a> BaseCoord<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Format1(item) => item.offset_data(),
            Self::Format2(item) => item.offset_data(),
            Self::Format3(item) => item.offset_data(),
        }
    }

    pub fn base_coord_format(&self) -> Result<u16, Never> {
        match self {
            Self::Format1(item) => item.base_coord_format(),
            Self::Format2(item) => item.base_coord_format(),
            Self::Format3(item) => item.base_coord_format(),
        }
    }

    pub fn coordinate(&self) -> Result<i16, Never> {
        match self {
            Self::Format1(item) => item.coordinate(),
            Self::Format2(item) => item.coordinate(),
            Self::Format3(item) => item.coordinate(),
        }
    }
}

impl<'a> FontRead<'a> for BaseCoord<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u16 = data.read_at(0u64)?;

        match format {
            BaseCoordFormat1::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format1(read))
            }
            BaseCoordFormat2::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format2(read))
            }
            BaseCoordFormat3::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format3(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u16>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for BaseCoord<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        match self {
            Self::Format1(item) => item.min_byte_range(),
            Self::Format2(item) => item.min_byte_range(),
            Self::Format3(item) => item.min_byte_range(),
        }
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        match self {
            Self::Format1(item) => item.min_table_bytes(),
            Self::Format2(item) => item.min_table_bytes(),
            Self::Format3(item) => item.min_table_bytes(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> BaseCoord<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Format1(table) => table,
            Self::Format2(table) => table,
            Self::Format3(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for BaseCoord<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for BaseCoord<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}

impl Format<u16> for BaseCoordFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for BaseCoordFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(coordinate_byte_range) = self.coordinate_byte_range();

        Ok(0..coordinate_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for BaseCoordFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_34) = data.len();

        match ::core::primitive::bool::then_some(value_34 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct BaseCoordFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> BaseCoordFormat1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + i16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn base_coord_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.base_coord_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coordinate(&self) -> Result<i16, Never> {
        let Ok(range) = self.coordinate_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn base_coord_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coordinate_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.base_coord_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    BaseCoordFormat1::MIN_SIZE
); held }));

impl Default for BaseCoordFormat1<'_> {
    fn default() -> Self {
        let Ok(value_35) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_35,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for BaseCoordFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("BaseCoordFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "base_coord_format", value: FieldType::from(self.base_coord_format()) }),
            1u64 => Some(Field { name: "coordinate", value: FieldType::from(self.coordinate()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for BaseCoordFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for BaseCoordFormat2<'_> {
    const FORMAT: u16 = 2;
}

impl<'a> MinByteRange<'a> for BaseCoordFormat2<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(base_coord_point_byte_range) = self.base_coord_point_byte_range();

        Ok(0..base_coord_point_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for BaseCoordFormat2<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_36) = data.len();

        match ::core::primitive::bool::then_some(value_36 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct BaseCoordFormat2<'a> {
    data: FontData<'a>,
}

impl<'a> BaseCoordFormat2<'a> {
    pub const MIN_SIZE: u64 =
        (u16::RAW_BYTE_LEN + i16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn base_coord_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.base_coord_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coordinate(&self) -> Result<i16, Never> {
        let Ok(range) = self.coordinate_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn reference_glyph(&self) -> Result<u16, Never> {
        let Ok(range) = self.reference_glyph_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn base_coord_point(&self) -> Result<u16, Never> {
        let Ok(range) = self.base_coord_point_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn base_coord_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coordinate_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.base_coord_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn reference_glyph_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coordinate_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn base_coord_point_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.reference_glyph_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for BaseCoordFormat2<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("BaseCoordFormat2")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "base_coord_format", value: FieldType::from(self.base_coord_format()) }),
            1u64 => Some(Field { name: "coordinate", value: FieldType::from(self.coordinate()) }),
            2u64 => Some(Field { name: "reference_glyph", value: FieldType::from(self.reference_glyph()) }),
            3u64 => Some(Field { name: "base_coord_point", value: FieldType::from(self.base_coord_point()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for BaseCoordFormat2<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for BaseCoordFormat3<'_> {
    const FORMAT: u16 = 3;
}

impl<'a> MinByteRange<'a> for BaseCoordFormat3<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(device_offset_byte_range) = self.device_offset_byte_range();

        Ok(0..device_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for BaseCoordFormat3<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_37) = data.len();

        match ::core::primitive::bool::then_some(value_37 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct BaseCoordFormat3<'a> {
    data: FontData<'a>,
}

impl<'a> BaseCoordFormat3<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + i16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn base_coord_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.base_coord_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coordinate(&self) -> Result<i16, Never> {
        let Ok(range) = self.coordinate_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn device_offset(&self) -> Result<Nullable<Offset16>, Never> {
        let Ok(range) = self.device_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn device(&self) -> Result<Option<DeviceOrVariationIndex<'a>>, ReadError> {
        let data = self.data;
        let Ok(device_offset) = self.device_offset();

        device_offset.resolve(data)
    }

    pub fn base_coord_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coordinate_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.base_coord_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn device_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coordinate_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for BaseCoordFormat3<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("BaseCoordFormat3")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "base_coord_format", value: FieldType::from(self.base_coord_format()) }),
            1u64 => Some(Field { name: "coordinate", value: FieldType::from(self.coordinate()) }),
            2u64 => Some(Field { name: "device_offset", value: FieldType::from(FieldType::offset(self.device_offset(), self.device())) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for BaseCoordFormat3<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
