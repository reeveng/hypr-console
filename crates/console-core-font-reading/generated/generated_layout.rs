

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for ScriptList<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(script_records_byte_range) = self.script_records_byte_range();

        Ok(0..script_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ScriptList<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ScriptList<'a> {
    data: FontData<'a>,
}

impl<'a> ScriptList<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn script_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.script_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn script_records(&self) -> Result<&'a [ScriptRecord], Never> {
        let Ok(range) = self.script_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn script_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn script_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(script_count) = self.script_count();
        let Ok(previous) = self.script_count_byte_range();
        let start = previous.end;
        let Ok(value_2) = transforms::to_u64(script_count);

        let end =
            start.saturating_add(value_2.saturating_mul(ScriptRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(ScriptList::MIN_SIZE); held }));

impl Default for ScriptList<'_> {
    fn default() -> Self {
        let Ok(value_3) = FontData::default_table_data();

        Self {
            data: value_3,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ScriptList<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ScriptList")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "script_count", value: FieldType::from(self.script_count()) }),
            1u64 => {
                let Ok(value_4) = self.offset_data();

                Some(Field { name: "script_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(ScriptRecord),
                    { let Ok(explicit_value) = self.script_records(); explicit_value },
                    value_4,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ScriptList<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct ScriptRecord {
    pub script_tag: BigEndian<Tag>,
    pub script_offset: BigEndian<Offset16>,
}

const _: () = {
  assert!(core::mem::size_of::<ScriptRecord>() == 6);
  assert!(core::mem::align_of::<ScriptRecord>() == 1);
  assert!(core::mem::offset_of!(ScriptRecord, script_tag) == 0);
  assert!(core::mem::offset_of!(ScriptRecord, script_offset) == 4);
};

impl ScriptRecord {
    pub fn script_tag(&self) -> Result<Tag, Never> {
        self.script_tag.get()
    }

    pub fn script_offset(&self) -> Result<Offset16, Never> {
        self.script_offset.get()
    }

    pub fn script<'a>(&self, data: FontData<'a>) -> Result<Script<'a>, ReadError> {
        let Ok(script_offset) = self.script_offset();

        script_offset.resolve(data)
    }
}

impl FixedSize for ScriptRecord {
    const RAW_BYTE_LEN: u64 = Tag::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for ScriptRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "ScriptRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "script_tag", value: FieldType::from(self.script_tag()) }),
                1u64 => Some(Field { name: "script_offset", value: FieldType::from(FieldType::offset(self.script_offset(), (self.script(_data)).map(Some))) }),
                _ => None,
            }),
            data,
        })
    }
}

impl<'a> MinByteRange<'a> for Script<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(lang_sys_records_byte_range) = self.lang_sys_records_byte_range();

        Ok(0..lang_sys_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Script<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_7) = data.len();

        match ::core::primitive::bool::then_some(value_7 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Script<'a> {
    data: FontData<'a>,
}

impl<'a> Script<'a> {
    pub const MIN_SIZE: u64 = (Offset16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn default_lang_sys_offset(&self) -> Result<Nullable<Offset16>, Never> {
        let Ok(range) = self.default_lang_sys_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn default_lang_sys(&self) -> Result<Option<LangSys<'a>>, ReadError> {
        let data = self.data;
        let Ok(default_lang_sys_offset) = self.default_lang_sys_offset();

        default_lang_sys_offset.resolve(data)
    }

    pub fn lang_sys_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.lang_sys_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn lang_sys_records(&self) -> Result<&'a [LangSysRecord], Never> {
        let Ok(range) = self.lang_sys_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn default_lang_sys_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn lang_sys_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.default_lang_sys_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn lang_sys_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(lang_sys_count) = self.lang_sys_count();
        let Ok(previous) = self.lang_sys_count_byte_range();
        let start = previous.end;
        let Ok(value_8) = transforms::to_u64(lang_sys_count);

        let end = start.saturating_add(value_8.saturating_mul(LangSysRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Script::MIN_SIZE); held }));

impl Default for Script<'_> {
    fn default() -> Self {
        let Ok(value_9) = FontData::default_table_data();

        Self {
            data: value_9,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Script<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Script")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "default_lang_sys_offset", value: FieldType::from(FieldType::offset(self.default_lang_sys_offset(), self.default_lang_sys())) }),
            1u64 => Some(Field { name: "lang_sys_count", value: FieldType::from(self.lang_sys_count()) }),
            2u64 => {
                let Ok(value_10) = self.offset_data();

                Some(Field { name: "lang_sys_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(LangSysRecord),
                    { let Ok(explicit_value) = self.lang_sys_records(); explicit_value },
                    value_10,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Script<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct LangSysRecord {
    pub lang_sys_tag: BigEndian<Tag>,
    pub lang_sys_offset: BigEndian<Offset16>,
}

const _: () = {
  assert!(core::mem::size_of::<LangSysRecord>() == 6);
  assert!(core::mem::align_of::<LangSysRecord>() == 1);
  assert!(core::mem::offset_of!(LangSysRecord, lang_sys_tag) == 0);
  assert!(core::mem::offset_of!(LangSysRecord, lang_sys_offset) == 4);
};

impl LangSysRecord {
    pub fn lang_sys_tag(&self) -> Result<Tag, Never> {
        self.lang_sys_tag.get()
    }

    pub fn lang_sys_offset(&self) -> Result<Offset16, Never> {
        self.lang_sys_offset.get()
    }

    pub fn lang_sys<'a>(&self, data: FontData<'a>) -> Result<LangSys<'a>, ReadError> {
        let Ok(lang_sys_offset) = self.lang_sys_offset();

        lang_sys_offset.resolve(data)
    }
}

impl FixedSize for LangSysRecord {
    const RAW_BYTE_LEN: u64 = Tag::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for LangSysRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "LangSysRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "lang_sys_tag", value: FieldType::from(self.lang_sys_tag()) }),
                1u64 => Some(Field { name: "lang_sys_offset", value: FieldType::from(FieldType::offset(self.lang_sys_offset(), (self.lang_sys(_data)).map(Some))) }),
                _ => None,
            }),
            data,
        })
    }
}

impl<'a> MinByteRange<'a> for LangSys<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(feature_indices_byte_range) = self.feature_indices_byte_range();

        Ok(0..feature_indices_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for LangSys<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_13) = data.len();

        match ::core::primitive::bool::then_some(value_13 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct LangSys<'a> {
    data: FontData<'a>,
}

impl<'a> LangSys<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn required_feature_index(&self) -> Result<u16, Never> {
        let Ok(range) = self.required_feature_index_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn feature_index_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.feature_index_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn feature_indices(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.feature_indices_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn lookup_order_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn required_feature_index_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.lookup_order_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn feature_index_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.required_feature_index_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn feature_indices_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(feature_index_count) = self.feature_index_count();
        let Ok(previous) = self.feature_index_count_byte_range();
        let start = previous.end;
        let Ok(value_14) = transforms::to_u64(feature_index_count);

        let end =
            start.saturating_add(value_14.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(LangSys::MIN_SIZE); held }));

impl Default for LangSys<'_> {
    fn default() -> Self {
        let Ok(value_15) = FontData::default_table_data();

        Self {
            data: value_15,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for LangSys<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("LangSys")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "required_feature_index", value: FieldType::from(self.required_feature_index()) }),
            1u64 => Some(Field { name: "feature_index_count", value: FieldType::from(self.feature_index_count()) }),
            2u64 => Some(Field { name: "feature_indices", value: FieldType::from(self.feature_indices()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for LangSys<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for FeatureList<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(feature_records_byte_range) = self.feature_records_byte_range();

        Ok(0..feature_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for FeatureList<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_16) = data.len();

        match ::core::primitive::bool::then_some(value_16 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct FeatureList<'a> {
    data: FontData<'a>,
}

impl<'a> FeatureList<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn feature_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.feature_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn feature_records(&self) -> Result<&'a [FeatureRecord], Never> {
        let Ok(range) = self.feature_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn feature_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn feature_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(feature_count) = self.feature_count();
        let Ok(previous) = self.feature_count_byte_range();
        let start = previous.end;
        let Ok(value_17) = transforms::to_u64(feature_count);

        let end = start.saturating_add(value_17.saturating_mul(FeatureRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(FeatureList::MIN_SIZE); held }));

impl Default for FeatureList<'_> {
    fn default() -> Self {
        let Ok(value_18) = FontData::default_table_data();

        Self {
            data: value_18,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for FeatureList<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("FeatureList")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "feature_count", value: FieldType::from(self.feature_count()) }),
            1u64 => {
                let Ok(value_19) = self.offset_data();

                Some(Field { name: "feature_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(FeatureRecord),
                    { let Ok(explicit_value) = self.feature_records(); explicit_value },
                    value_19,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for FeatureList<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct FeatureRecord {
    pub feature_tag: BigEndian<Tag>,
    pub feature_offset: BigEndian<Offset16>,
}

const _: () = {
  assert!(core::mem::size_of::<FeatureRecord>() == 6);
  assert!(core::mem::align_of::<FeatureRecord>() == 1);
  assert!(core::mem::offset_of!(FeatureRecord, feature_tag) == 0);
  assert!(core::mem::offset_of!(FeatureRecord, feature_offset) == 4);
};

impl FeatureRecord {
    pub fn feature_tag(&self) -> Result<Tag, Never> {
        self.feature_tag.get()
    }

    pub fn feature_offset(&self) -> Result<Offset16, Never> {
        self.feature_offset.get()
    }

    pub fn feature<'a>(&self, data: FontData<'a>) -> Result<Feature<'a>, ReadError> {
        let Ok(args) = self.feature_tag();
        let Ok(feature_offset) = self.feature_offset();

        feature_offset.resolve_with_args(data, &args)
    }
}

impl FixedSize for FeatureRecord {
    const RAW_BYTE_LEN: u64 = Tag::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for FeatureRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "FeatureRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "feature_tag", value: FieldType::from(self.feature_tag()) }),
                1u64 => Some(Field { name: "feature_offset", value: FieldType::from(FieldType::offset(self.feature_offset(), (self.feature(_data)).map(Some))) }),
                _ => None,
            }),
            data,
        })
    }
}

impl<'a> MinByteRange<'a> for Feature<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(lookup_list_indices_byte_range) = self.lookup_list_indices_byte_range();

        Ok(0..lookup_list_indices_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl ReadArgs for Feature<'_> {
    type Args = Tag;
}

impl<'a> FontReadWithArgs<'a> for Feature<'a> {
    fn read_with_args(data: FontData<'a>, args: &Tag) -> Result<Self, ReadError> {
        let feature_tag = *args;

        let Ok(value_22) = data.len();

        match ::core::primitive::bool::then_some(value_22 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data, feature_tag })
    }
}

impl<'a> Feature<'a> {
    pub fn read(data: FontData<'a>, feature_tag: Tag) -> Result<Self, ReadError> {
        let args = feature_tag;
        Self::read_with_args(data, &args)
    }
}

#[derive(Clone)]
pub struct Feature<'a> {
    data: FontData<'a>,
    feature_tag: Tag,
}

impl<'a> Feature<'a> {
    pub const MIN_SIZE: u64 = (Offset16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn feature_params_offset(&self) -> Result<Nullable<Offset16>, Never> {
        let Ok(range) = self.feature_params_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn feature_params(&self) -> Result<Option<FeatureParams<'a>>, ReadError> {
        let data = self.data;
        let Ok(args) = self.feature_tag();
        let Ok(feature_params_offset) = self.feature_params_offset();

        feature_params_offset.resolve_with_args(data, &args)
    }

    pub fn lookup_index_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.lookup_index_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn lookup_list_indices(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.lookup_list_indices_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub(crate) fn feature_tag(&self) -> Result<Tag, Never> {
        Ok(self.feature_tag)
    }

    pub fn feature_params_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn lookup_index_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.feature_params_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn lookup_list_indices_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(lookup_index_count) = self.lookup_index_count();
        let Ok(previous) = self.lookup_index_count_byte_range();
        let start = previous.end;
        let Ok(value_23) = transforms::to_u64(lookup_index_count);

        let end =
            start.saturating_add(value_23.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Feature::MIN_SIZE); held }));

impl Default for Feature<'_> {
    fn default() -> Self {
        let Ok(value_24) = FontData::default_table_data();

        Self {
            data: value_24,
            feature_tag: Default::default(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Feature<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Feature")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "feature_params_offset", value: FieldType::from(FieldType::offset(self.feature_params_offset(), self.feature_params())) }),
            1u64 => Some(Field { name: "lookup_index_count", value: FieldType::from(self.lookup_index_count()) }),
            2u64 => Some(Field { name: "lookup_list_indices", value: FieldType::from(self.lookup_list_indices()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Feature<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a, T> MinByteRange<'a> for LookupList<'a, T> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(lookup_offsets_byte_range) = self.lookup_offsets_byte_range();

        Ok(0..lookup_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a, T> FontRead<'a> for LookupList<'a, T> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_25) = data.len();

        match ::core::primitive::bool::then_some(value_25 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self {
            data,
            offset_type: std::marker::PhantomData,
        })
    }
}

impl<'a, T> LookupList<'a, T> {

}

#[derive(Clone)]
pub struct LookupList<'a, T = ()> {
    data: FontData<'a>,
    offset_type: std::marker::PhantomData<*const T>,
}

impl<'a, T> LookupList<'a, T> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn lookup_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.lookup_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn lookup_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.lookup_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn lookups(&self) -> Result<ArrayOfOffsets<'a, T, Offset16>, Never>
    where
        T: FontRead<'a>,
    {
        let data = self.data;
        let Ok(offsets) = self.lookup_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn lookup_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn lookup_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(lookup_count) = self.lookup_count();
        let Ok(previous) = self.lookup_count_byte_range();
        let start = previous.end;
        let Ok(value_26) = transforms::to_u64(lookup_count);

        let end =
            start.saturating_add(value_26.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    LookupList::<()>::MIN_SIZE
); held }));

impl<T> Default for LookupList<'_, T> {
    fn default() -> Self {
        let Ok(value_27) = FontData::default_table_data();

        Self {
            data: value_27,
            offset_type: std::marker::PhantomData,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a, T: FontRead<'a> + SomeTable<'a> + 'a> SomeTable<'a> for LookupList<'a, T> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("LookupList")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "lookup_count", value: FieldType::from(self.lookup_count()) }),
            1u64 => Some(Field { name: "lookup_offsets", value: FieldType::from(self.lookups()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a, T: FontRead<'a> + SomeTable<'a> + 'a> std::fmt::Debug for LookupList<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Discriminant for Lookup<'_, ()> {
    fn read_discriminant(data: FontData<'_>) -> Result<u16, ReadError> {
        data.read_at(0)
    }
}

impl<'a, T> MinByteRange<'a> for Lookup<'a, T> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(subtable_offsets_byte_range) = self.subtable_offsets_byte_range();

        Ok(0..subtable_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a, T> FontRead<'a> for Lookup<'a, T> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_28) = data.len();

        match ::core::primitive::bool::then_some(value_28 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self {
            data,
            offset_type: std::marker::PhantomData,
        })
    }
}

impl<'a, T> Lookup<'a, T> {

    pub(crate) fn of_unit_type(&self) -> Result<Lookup<'a, ()>, Never> {
        Ok(Lookup {
            data: self.data,
            offset_type: std::marker::PhantomData,
        })
    }
}

#[derive(Clone)]
pub struct Lookup<'a, T = ()> {
    data: FontData<'a>,
    offset_type: std::marker::PhantomData<*const T>,
}

impl<'a, T> Lookup<'a, T> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + LookupFlag::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn lookup_type(&self) -> Result<u16, Never> {
        let Ok(range) = self.lookup_type_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn lookup_flag(&self) -> Result<LookupFlag, Never> {
        let Ok(range) = self.lookup_flag_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn sub_table_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.sub_table_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn subtable_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.subtable_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn subtables(&self) -> Result<ArrayOfOffsets<'a, T, Offset16>, Never>
    where
        T: FontRead<'a>,
    {
        let data = self.data;
        let Ok(offsets) = self.subtable_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn mark_filtering_set(&self) -> Result<Option<u16>, Never> {
        let Ok(range) = self.mark_filtering_set_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn lookup_type_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn lookup_flag_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.lookup_type_byte_range();
        let start = previous.end;
        let end = start.saturating_add(LookupFlag::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn sub_table_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.lookup_flag_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn subtable_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(sub_table_count) = self.sub_table_count();
        let Ok(previous) = self.sub_table_count_byte_range();
        let start = previous.end;
        let Ok(value_29) = transforms::to_u64(sub_table_count);

        let end =
            start.saturating_add(value_29.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn mark_filtering_set_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.subtable_offsets_byte_range();
        let start = previous.end;
        let Ok(lookup_flag) = self.lookup_flag();
        let Ok(presence) = lookup_flag.contains(LookupFlag::USE_MARK_FILTERING_SET);
        let end = match presence {
            Presence::Present => {
                start.saturating_add(u16::RAW_BYTE_LEN)
            }
            Presence::Absent => start,
        };
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Lookup::<()>::MIN_SIZE); held }));

impl<T> Default for Lookup<'_, T> {
    fn default() -> Self {
        let Ok(value_30) = FontData::default_table_data();

        Self {
            data: value_30,
            offset_type: std::marker::PhantomData,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a, T: FontRead<'a> + SomeTable<'a> + 'a> SomeTable<'a> for Lookup<'a, T> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Lookup")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "lookup_type", value: FieldType::from(self.lookup_type()) }),
            1u64 => {
                let Ok(value) = self.traverse_lookup_flag();

                Some(Field { name: "lookup_flag", value })
            }
            2u64 => Some(Field { name: "sub_table_count", value: FieldType::from(self.sub_table_count()) }),
            3u64 => Some(Field { name: "subtable_offsets", value: FieldType::from(self.subtables()) }),
            4u64 => {
                let Ok(lookup_flag) = self.lookup_flag();
                let Ok(presence) = lookup_flag.contains(LookupFlag::USE_MARK_FILTERING_SET);
                let Ok(mark_filtering_set) = self.mark_filtering_set();

                match presence {
                    Presence::Present => mark_filtering_set.map(|mark_filtering_set_2| Field { name: "mark_filtering_set", value: FieldType::from(mark_filtering_set_2) }),
                    Presence::Absent => None,
                }
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a, T: FontRead<'a> + SomeTable<'a> + 'a> std::fmt::Debug for Lookup<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for CoverageFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for CoverageFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(glyph_array_byte_range) = self.glyph_array_byte_range();

        Ok(0..glyph_array_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for CoverageFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_31) = data.len();

        match ::core::primitive::bool::then_some(value_31 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct CoverageFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> CoverageFormat1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn coverage_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.coverage_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.glyph_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn glyph_array(&self) -> Result<&'a [BigEndian<GlyphId16>], Never> {
        let Ok(range) = self.glyph_array_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn coverage_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_array_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(glyph_count) = self.glyph_count();
        let Ok(previous) = self.glyph_count_byte_range();
        let start = previous.end;
        let Ok(value_32) = transforms::to_u64(glyph_count);

        let end =
            start.saturating_add(value_32.saturating_mul(GlyphId16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    CoverageFormat1::MIN_SIZE
); held }));

impl Default for CoverageFormat1<'_> {
    fn default() -> Self {
        let Ok(value_33) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_33,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for CoverageFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("CoverageFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "coverage_format", value: FieldType::from(self.coverage_format()) }),
            1u64 => Some(Field { name: "glyph_count", value: FieldType::from(self.glyph_count()) }),
            2u64 => Some(Field { name: "glyph_array", value: FieldType::from(self.glyph_array()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for CoverageFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for CoverageFormat2<'_> {
    const FORMAT: u16 = 2;
}

impl<'a> MinByteRange<'a> for CoverageFormat2<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(range_records_byte_range) = self.range_records_byte_range();

        Ok(0..range_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for CoverageFormat2<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_34) = data.len();

        match ::core::primitive::bool::then_some(value_34 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct CoverageFormat2<'a> {
    data: FontData<'a>,
}

impl<'a> CoverageFormat2<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn coverage_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.coverage_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn range_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.range_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn range_records(&self) -> Result<&'a [RangeRecord], Never> {
        let Ok(range) = self.range_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn coverage_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn range_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn range_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(range_count) = self.range_count();
        let Ok(previous) = self.range_count_byte_range();
        let start = previous.end;
        let Ok(value_35) = transforms::to_u64(range_count);

        let end =
            start.saturating_add(value_35.saturating_mul(RangeRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for CoverageFormat2<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("CoverageFormat2")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "coverage_format", value: FieldType::from(self.coverage_format()) }),
            1u64 => Some(Field { name: "range_count", value: FieldType::from(self.range_count()) }),
            2u64 => {
                let Ok(value_36) = self.offset_data();

                Some(Field { name: "range_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(RangeRecord),
                    { let Ok(explicit_value) = self.range_records(); explicit_value },
                    value_36,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for CoverageFormat2<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct RangeRecord {
    pub start_glyph_id: BigEndian<GlyphId16>,
    pub end_glyph_id: BigEndian<GlyphId16>,
    pub start_coverage_index: BigEndian<u16>,
}

const _: () = {
  assert!(core::mem::size_of::<RangeRecord>() == 6);
  assert!(core::mem::align_of::<RangeRecord>() == 1);
  assert!(core::mem::offset_of!(RangeRecord, start_glyph_id) == 0);
  assert!(core::mem::offset_of!(RangeRecord, end_glyph_id) == 2);
  assert!(core::mem::offset_of!(RangeRecord, start_coverage_index) == 4);
};

impl RangeRecord {
    pub fn start_glyph_id(&self) -> Result<GlyphId16, Never> {
        self.start_glyph_id.get()
    }

    pub fn end_glyph_id(&self) -> Result<GlyphId16, Never> {
        self.end_glyph_id.get()
    }

    pub fn start_coverage_index(&self) -> Result<u16, Never> {
        self.start_coverage_index.get()
    }
}

impl FixedSize for RangeRecord {
    const RAW_BYTE_LEN: u64 =
        GlyphId16::RAW_BYTE_LEN + GlyphId16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for RangeRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "RangeRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "start_glyph_id", value: FieldType::from(self.start_glyph_id()) }),
                1u64 => Some(Field { name: "end_glyph_id", value: FieldType::from(self.end_glyph_id()) }),
                2u64 => Some(Field { name: "start_coverage_index", value: FieldType::from(self.start_coverage_index()) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone)]
pub enum CoverageTable<'a> {
    Format1(CoverageFormat1<'a>),
    Format2(CoverageFormat2<'a>),
}

impl Default for CoverageTable<'_> {
    fn default() -> Self {
        Self::Format1(Default::default())
    }
}

impl<'a> CoverageTable<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Format1(item) => item.offset_data(),
            Self::Format2(item) => item.offset_data(),
        }
    }

    pub fn coverage_format(&self) -> Result<u16, Never> {
        match self {
            Self::Format1(item) => item.coverage_format(),
            Self::Format2(item) => item.coverage_format(),
        }
    }
}

impl<'a> FontRead<'a> for CoverageTable<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u16 = data.read_at(0u64)?;

        match format {
            CoverageFormat1::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format1(read))
            }
            CoverageFormat2::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format2(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u16>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for CoverageTable<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        match self {
            Self::Format1(item) => item.min_byte_range(),
            Self::Format2(item) => item.min_byte_range(),
        }
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        match self {
            Self::Format1(item) => item.min_table_bytes(),
            Self::Format2(item) => item.min_table_bytes(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> CoverageTable<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Format1(table) => table,
            Self::Format2(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for CoverageTable<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for CoverageTable<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}

impl Format<u16> for ClassDefFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for ClassDefFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(class_value_array_byte_range) = self.class_value_array_byte_range();

        Ok(0..class_value_array_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ClassDefFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_42) = data.len();

        match ::core::primitive::bool::then_some(value_42 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ClassDefFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> ClassDefFormat1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + GlyphId16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn class_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.class_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn start_glyph_id(&self) -> Result<GlyphId16, Never> {
        let Ok(range) = self.start_glyph_id_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.glyph_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn class_value_array(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.class_value_array_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn class_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn start_glyph_id_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.class_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(GlyphId16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.start_glyph_id_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn class_value_array_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(glyph_count) = self.glyph_count();
        let Ok(previous) = self.glyph_count_byte_range();
        let start = previous.end;
        let Ok(value_43) = transforms::to_u64(glyph_count);

        let end = start.saturating_add(value_43.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    ClassDefFormat1::MIN_SIZE
); held }));

impl Default for ClassDefFormat1<'_> {
    fn default() -> Self {
        let Ok(value_44) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_44,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ClassDefFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ClassDefFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "class_format", value: FieldType::from(self.class_format()) }),
            1u64 => Some(Field { name: "start_glyph_id", value: FieldType::from(self.start_glyph_id()) }),
            2u64 => Some(Field { name: "glyph_count", value: FieldType::from(self.glyph_count()) }),
            3u64 => Some(Field { name: "class_value_array", value: FieldType::from(self.class_value_array()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ClassDefFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for ClassDefFormat2<'_> {
    const FORMAT: u16 = 2;
}

impl<'a> MinByteRange<'a> for ClassDefFormat2<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(class_range_records_byte_range) = self.class_range_records_byte_range();

        Ok(0..class_range_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ClassDefFormat2<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_45) = data.len();

        match ::core::primitive::bool::then_some(value_45 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ClassDefFormat2<'a> {
    data: FontData<'a>,
}

impl<'a> ClassDefFormat2<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn class_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.class_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn class_range_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.class_range_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn class_range_records(&self) -> Result<&'a [ClassRangeRecord], Never> {
        let Ok(range) = self.class_range_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn class_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn class_range_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.class_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn class_range_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(class_range_count) = self.class_range_count();
        let Ok(previous) = self.class_range_count_byte_range();
        let start = previous.end;
        let Ok(value_46) = transforms::to_u64(class_range_count);

        let end = start.saturating_add(value_46
                .saturating_mul(ClassRangeRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ClassDefFormat2<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ClassDefFormat2")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "class_format", value: FieldType::from(self.class_format()) }),
            1u64 => Some(Field { name: "class_range_count", value: FieldType::from(self.class_range_count()) }),
            2u64 => {
                let Ok(value_47) = self.offset_data();

                Some(Field { name: "class_range_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(ClassRangeRecord),
                    { let Ok(explicit_value) = self.class_range_records(); explicit_value },
                    value_47,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ClassDefFormat2<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct ClassRangeRecord {
    pub start_glyph_id: BigEndian<GlyphId16>,
    pub end_glyph_id: BigEndian<GlyphId16>,
    pub class: BigEndian<u16>,
}

const _: () = {
  assert!(core::mem::size_of::<ClassRangeRecord>() == 6);
  assert!(core::mem::align_of::<ClassRangeRecord>() == 1);
  assert!(core::mem::offset_of!(ClassRangeRecord, start_glyph_id) == 0);
  assert!(core::mem::offset_of!(ClassRangeRecord, end_glyph_id) == 2);
  assert!(core::mem::offset_of!(ClassRangeRecord, class) == 4);
};

impl ClassRangeRecord {
    pub fn start_glyph_id(&self) -> Result<GlyphId16, Never> {
        self.start_glyph_id.get()
    }

    pub fn end_glyph_id(&self) -> Result<GlyphId16, Never> {
        self.end_glyph_id.get()
    }

    pub fn class(&self) -> Result<u16, Never> {
        self.class.get()
    }
}

impl FixedSize for ClassRangeRecord {
    const RAW_BYTE_LEN: u64 =
        GlyphId16::RAW_BYTE_LEN + GlyphId16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for ClassRangeRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "ClassRangeRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "start_glyph_id", value: FieldType::from(self.start_glyph_id()) }),
                1u64 => Some(Field { name: "end_glyph_id", value: FieldType::from(self.end_glyph_id()) }),
                2u64 => Some(Field { name: "class", value: FieldType::from(self.class()) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone)]
pub enum ClassDef<'a> {
    Format1(ClassDefFormat1<'a>),
    Format2(ClassDefFormat2<'a>),
}

impl Default for ClassDef<'_> {
    fn default() -> Self {
        Self::Format1(Default::default())
    }
}

impl<'a> ClassDef<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Format1(item) => item.offset_data(),
            Self::Format2(item) => item.offset_data(),
        }
    }

    pub fn class_format(&self) -> Result<u16, Never> {
        match self {
            Self::Format1(item) => item.class_format(),
            Self::Format2(item) => item.class_format(),
        }
    }
}

impl<'a> FontRead<'a> for ClassDef<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u16 = data.read_at(0u64)?;

        match format {
            ClassDefFormat1::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format1(read))
            }
            ClassDefFormat2::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format2(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u16>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for ClassDef<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        match self {
            Self::Format1(item) => item.min_byte_range(),
            Self::Format2(item) => item.min_byte_range(),
        }
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        match self {
            Self::Format1(item) => item.min_table_bytes(),
            Self::Format2(item) => item.min_table_bytes(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> ClassDef<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Format1(table) => table,
            Self::Format2(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for ClassDef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ClassDef<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct SequenceLookupRecord {
    pub sequence_index: BigEndian<u16>,
    pub lookup_list_index: BigEndian<u16>,
}

const _: () = {
  assert!(core::mem::size_of::<SequenceLookupRecord>() == 4);
  assert!(core::mem::align_of::<SequenceLookupRecord>() == 1);
  assert!(core::mem::offset_of!(SequenceLookupRecord, sequence_index) == 0);
  assert!(core::mem::offset_of!(SequenceLookupRecord, lookup_list_index) == 2);
};

impl SequenceLookupRecord {
    pub fn sequence_index(&self) -> Result<u16, Never> {
        self.sequence_index.get()
    }

    pub fn lookup_list_index(&self) -> Result<u16, Never> {
        self.lookup_list_index.get()
    }
}

impl FixedSize for SequenceLookupRecord {
    const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for SequenceLookupRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "SequenceLookupRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "sequence_index", value: FieldType::from(self.sequence_index()) }),
                1u64 => Some(Field { name: "lookup_list_index", value: FieldType::from(self.lookup_list_index()) }),
                _ => None,
            }),
            data,
        })
    }
}

impl Format<u16> for SequenceContextFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for SequenceContextFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(seq_rule_set_offsets_byte_range) = self.seq_rule_set_offsets_byte_range();

        Ok(0..seq_rule_set_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for SequenceContextFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_55) = data.len();

        match ::core::primitive::bool::then_some(value_55 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct SequenceContextFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> SequenceContextFormat1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.coverage_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage(&self) -> Result<CoverageTable<'a>, ReadError> {
        let data = self.data;
        let Ok(coverage_offset) = self.coverage_offset();

        coverage_offset.resolve(data)
    }

    pub fn seq_rule_set_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.seq_rule_set_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn seq_rule_set_offsets(&self) -> Result<&'a [BigEndian<Nullable<Offset16>>], Never> {
        let Ok(range) = self.seq_rule_set_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn seq_rule_sets(&self) -> Result<ArrayOfNullableOffsets<'a, SequenceRuleSet<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.seq_rule_set_offsets();
        ArrayOfNullableOffsets::new(offsets, data, ())
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn seq_rule_set_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn seq_rule_set_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(seq_rule_set_count) = self.seq_rule_set_count();
        let Ok(previous) = self.seq_rule_set_count_byte_range();
        let start = previous.end;
        let Ok(value_56) = transforms::to_u64(seq_rule_set_count);

        let end = start.saturating_add(value_56.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    SequenceContextFormat1::MIN_SIZE
); held }));

impl Default for SequenceContextFormat1<'_> {
    fn default() -> Self {
        let Ok(value_57) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_57,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for SequenceContextFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("SequenceContextFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "coverage_offset", value: FieldType::from(FieldType::offset(self.coverage_offset(), (self.coverage()).map(Some))) }),
            2u64 => Some(Field { name: "seq_rule_set_count", value: FieldType::from(self.seq_rule_set_count()) }),
            3u64 => Some(Field { name: "seq_rule_set_offsets", value: FieldType::from(self.seq_rule_sets()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for SequenceContextFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for SequenceRuleSet<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(seq_rule_offsets_byte_range) = self.seq_rule_offsets_byte_range();

        Ok(0..seq_rule_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for SequenceRuleSet<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_58) = data.len();

        match ::core::primitive::bool::then_some(value_58 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct SequenceRuleSet<'a> {
    data: FontData<'a>,
}

impl<'a> SequenceRuleSet<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn seq_rule_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.seq_rule_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn seq_rule_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.seq_rule_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn seq_rules(&self) -> Result<ArrayOfOffsets<'a, SequenceRule<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.seq_rule_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn seq_rule_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn seq_rule_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(seq_rule_count) = self.seq_rule_count();
        let Ok(previous) = self.seq_rule_count_byte_range();
        let start = previous.end;
        let Ok(value_59) = transforms::to_u64(seq_rule_count);

        let end =
            start.saturating_add(value_59.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    SequenceRuleSet::MIN_SIZE
); held }));

impl Default for SequenceRuleSet<'_> {
    fn default() -> Self {
        let Ok(value_60) = FontData::default_table_data();

        Self {
            data: value_60,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for SequenceRuleSet<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("SequenceRuleSet")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "seq_rule_count", value: FieldType::from(self.seq_rule_count()) }),
            1u64 => Some(Field { name: "seq_rule_offsets", value: FieldType::from(self.seq_rules()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for SequenceRuleSet<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for SequenceRule<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(seq_lookup_records_byte_range) = self.seq_lookup_records_byte_range();

        Ok(0..seq_lookup_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for SequenceRule<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_61) = data.len();

        match ::core::primitive::bool::then_some(value_61 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct SequenceRule<'a> {
    data: FontData<'a>,
}

impl<'a> SequenceRule<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.glyph_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn seq_lookup_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.seq_lookup_count_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn input_sequence(&self) -> Result<&'a [BigEndian<GlyphId16>], Never> {
        let Ok(range) = self.input_sequence_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn seq_lookup_records(&self) -> Result<&'a [SequenceLookupRecord], Never> {
        let Ok(range) = self.seq_lookup_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn seq_lookup_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.glyph_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn input_sequence_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(glyph_count) = self.glyph_count();
        let Ok(previous) = self.seq_lookup_count_byte_range();
        let start = previous.end;
        let Ok(value_62) = transforms::subtract(glyph_count, 1_u64);

        let end = start.saturating_add(value_62.saturating_mul(GlyphId16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn seq_lookup_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(seq_lookup_count) = self.seq_lookup_count();
        let Ok(previous) = self.input_sequence_byte_range();
        let start = previous.end;
        let Ok(value_63) = transforms::to_u64(seq_lookup_count);

        let end = start.saturating_add(value_63
                .saturating_mul(SequenceLookupRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(SequenceRule::MIN_SIZE); held }));

impl Default for SequenceRule<'_> {
    fn default() -> Self {
        let Ok(value_64) = FontData::default_table_data();

        Self {
            data: value_64,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for SequenceRule<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("SequenceRule")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "glyph_count", value: FieldType::from(self.glyph_count()) }),
            1u64 => Some(Field { name: "seq_lookup_count", value: FieldType::from(self.seq_lookup_count()) }),
            2u64 => Some(Field { name: "input_sequence", value: FieldType::from(self.input_sequence()) }),
            3u64 => {
                let Ok(value_65) = self.offset_data();

                Some(Field { name: "seq_lookup_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(SequenceLookupRecord),
                    { let Ok(explicit_value) = self.seq_lookup_records(); explicit_value },
                    value_65,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for SequenceRule<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for SequenceContextFormat2<'_> {
    const FORMAT: u16 = 2;
}

impl<'a> MinByteRange<'a> for SequenceContextFormat2<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(class_seq_rule_set_offsets_byte_range) = self.class_seq_rule_set_offsets_byte_range();

        Ok(0..class_seq_rule_set_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for SequenceContextFormat2<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_66) = data.len();

        match ::core::primitive::bool::then_some(value_66 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct SequenceContextFormat2<'a> {
    data: FontData<'a>,
}

impl<'a> SequenceContextFormat2<'a> {
    pub const MIN_SIZE: u64 =
        (u16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.coverage_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage(&self) -> Result<CoverageTable<'a>, ReadError> {
        let data = self.data;
        let Ok(coverage_offset) = self.coverage_offset();

        coverage_offset.resolve(data)
    }

    pub fn class_def_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.class_def_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn class_def(&self) -> Result<ClassDef<'a>, ReadError> {
        let data = self.data;
        let Ok(class_def_offset) = self.class_def_offset();

        class_def_offset.resolve(data)
    }

    pub fn class_seq_rule_set_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.class_seq_rule_set_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn class_seq_rule_set_offsets(&self) -> Result<&'a [BigEndian<Nullable<Offset16>>], Never> {
        let Ok(range) = self.class_seq_rule_set_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn class_seq_rule_sets(
        &self,
    ) -> Result<ArrayOfNullableOffsets<'a, ClassSequenceRuleSet<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.class_seq_rule_set_offsets();
        ArrayOfNullableOffsets::new(offsets, data, ())
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn class_def_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn class_seq_rule_set_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.class_def_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn class_seq_rule_set_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(class_seq_rule_set_count) = self.class_seq_rule_set_count();
        let Ok(previous) = self.class_seq_rule_set_count_byte_range();
        let start = previous.end;
        let Ok(value_67) = transforms::to_u64(class_seq_rule_set_count);

        let end = start.saturating_add(value_67
                .saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for SequenceContextFormat2<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("SequenceContextFormat2")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "coverage_offset", value: FieldType::from(FieldType::offset(self.coverage_offset(), (self.coverage()).map(Some))) }),
            2u64 => Some(Field { name: "class_def_offset", value: FieldType::from(FieldType::offset(self.class_def_offset(), (self.class_def()).map(Some))) }),
            3u64 => Some(Field { name: "class_seq_rule_set_count", value: FieldType::from(self.class_seq_rule_set_count()) }),
            4u64 => Some(Field { name: "class_seq_rule_set_offsets", value: FieldType::from(self.class_seq_rule_sets()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for SequenceContextFormat2<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for ClassSequenceRuleSet<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(class_seq_rule_offsets_byte_range) = self.class_seq_rule_offsets_byte_range();

        Ok(0..class_seq_rule_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ClassSequenceRuleSet<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_68) = data.len();

        match ::core::primitive::bool::then_some(value_68 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ClassSequenceRuleSet<'a> {
    data: FontData<'a>,
}

impl<'a> ClassSequenceRuleSet<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn class_seq_rule_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.class_seq_rule_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn class_seq_rule_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.class_seq_rule_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn class_seq_rules(&self) -> Result<ArrayOfOffsets<'a, ClassSequenceRule<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.class_seq_rule_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn class_seq_rule_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn class_seq_rule_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(class_seq_rule_count) = self.class_seq_rule_count();
        let Ok(previous) = self.class_seq_rule_count_byte_range();
        let start = previous.end;
        let Ok(value_69) = transforms::to_u64(class_seq_rule_count);

        let end = start.saturating_add(value_69.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    ClassSequenceRuleSet::MIN_SIZE
); held }));

impl Default for ClassSequenceRuleSet<'_> {
    fn default() -> Self {
        let Ok(value_70) = FontData::default_table_data();

        Self {
            data: value_70,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ClassSequenceRuleSet<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ClassSequenceRuleSet")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "class_seq_rule_count", value: FieldType::from(self.class_seq_rule_count()) }),
            1u64 => Some(Field { name: "class_seq_rule_offsets", value: FieldType::from(self.class_seq_rules()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ClassSequenceRuleSet<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for ClassSequenceRule<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(seq_lookup_records_byte_range) = self.seq_lookup_records_byte_range();

        Ok(0..seq_lookup_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ClassSequenceRule<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_71) = data.len();

        match ::core::primitive::bool::then_some(value_71 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ClassSequenceRule<'a> {
    data: FontData<'a>,
}

impl<'a> ClassSequenceRule<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.glyph_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn seq_lookup_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.seq_lookup_count_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn input_sequence(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.input_sequence_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn seq_lookup_records(&self) -> Result<&'a [SequenceLookupRecord], Never> {
        let Ok(range) = self.seq_lookup_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn seq_lookup_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.glyph_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn input_sequence_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(glyph_count) = self.glyph_count();
        let Ok(previous) = self.seq_lookup_count_byte_range();
        let start = previous.end;
        let Ok(value_72) = transforms::subtract(glyph_count, 1_u64);

        let end =
            start.saturating_add(value_72.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn seq_lookup_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(seq_lookup_count) = self.seq_lookup_count();
        let Ok(previous) = self.input_sequence_byte_range();
        let start = previous.end;
        let Ok(value_73) = transforms::to_u64(seq_lookup_count);

        let end = start.saturating_add(value_73
                .saturating_mul(SequenceLookupRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    ClassSequenceRule::MIN_SIZE
); held }));

impl Default for ClassSequenceRule<'_> {
    fn default() -> Self {
        let Ok(value_74) = FontData::default_table_data();

        Self {
            data: value_74,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ClassSequenceRule<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ClassSequenceRule")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "glyph_count", value: FieldType::from(self.glyph_count()) }),
            1u64 => Some(Field { name: "seq_lookup_count", value: FieldType::from(self.seq_lookup_count()) }),
            2u64 => Some(Field { name: "input_sequence", value: FieldType::from(self.input_sequence()) }),
            3u64 => {
                let Ok(value_75) = self.offset_data();

                Some(Field { name: "seq_lookup_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(SequenceLookupRecord),
                    { let Ok(explicit_value) = self.seq_lookup_records(); explicit_value },
                    value_75,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ClassSequenceRule<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for SequenceContextFormat3<'_> {
    const FORMAT: u16 = 3;
}

impl<'a> MinByteRange<'a> for SequenceContextFormat3<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(seq_lookup_records_byte_range) = self.seq_lookup_records_byte_range();

        Ok(0..seq_lookup_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for SequenceContextFormat3<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_76) = data.len();

        match ::core::primitive::bool::then_some(value_76 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct SequenceContextFormat3<'a> {
    data: FontData<'a>,
}

impl<'a> SequenceContextFormat3<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.glyph_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn seq_lookup_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.seq_lookup_count_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn coverage_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.coverage_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn coverages(&self) -> Result<ArrayOfOffsets<'a, CoverageTable<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.coverage_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn seq_lookup_records(&self) -> Result<&'a [SequenceLookupRecord], Never> {
        let Ok(range) = self.seq_lookup_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn seq_lookup_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.glyph_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coverage_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(glyph_count) = self.glyph_count();
        let Ok(previous) = self.seq_lookup_count_byte_range();
        let start = previous.end;
        let Ok(value_77) = transforms::to_u64(glyph_count);

        let end =
            start.saturating_add(value_77.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn seq_lookup_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(seq_lookup_count) = self.seq_lookup_count();
        let Ok(previous) = self.coverage_offsets_byte_range();
        let start = previous.end;
        let Ok(value_78) = transforms::to_u64(seq_lookup_count);

        let end = start.saturating_add(value_78
                .saturating_mul(SequenceLookupRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for SequenceContextFormat3<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("SequenceContextFormat3")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "glyph_count", value: FieldType::from(self.glyph_count()) }),
            2u64 => Some(Field { name: "seq_lookup_count", value: FieldType::from(self.seq_lookup_count()) }),
            3u64 => Some(Field { name: "coverage_offsets", value: FieldType::from(self.coverages()) }),
            4u64 => {
                let Ok(value_79) = self.offset_data();

                Some(Field { name: "seq_lookup_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(SequenceLookupRecord),
                    { let Ok(explicit_value) = self.seq_lookup_records(); explicit_value },
                    value_79,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for SequenceContextFormat3<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone)]
pub enum SequenceContext<'a> {
    Format1(SequenceContextFormat1<'a>),
    Format2(SequenceContextFormat2<'a>),
    Format3(SequenceContextFormat3<'a>),
}

impl Default for SequenceContext<'_> {
    fn default() -> Self {
        Self::Format1(Default::default())
    }
}

impl<'a> SequenceContext<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Format1(item) => item.offset_data(),
            Self::Format2(item) => item.offset_data(),
            Self::Format3(item) => item.offset_data(),
        }
    }

    pub fn format(&self) -> Result<u16, Never> {
        match self {
            Self::Format1(item) => item.format(),
            Self::Format2(item) => item.format(),
            Self::Format3(item) => item.format(),
        }
    }
}

impl<'a> FontRead<'a> for SequenceContext<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u16 = data.read_at(0u64)?;

        match format {
            SequenceContextFormat1::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format1(read))
            }
            SequenceContextFormat2::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format2(read))
            }
            SequenceContextFormat3::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format3(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u16>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for SequenceContext<'a> {
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
impl<'a> SequenceContext<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Format1(table) => table,
            Self::Format2(table) => table,
            Self::Format3(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for SequenceContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for SequenceContext<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}

impl Format<u16> for ChainedSequenceContextFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for ChainedSequenceContextFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(chained_seq_rule_set_offsets_byte_range) = self.chained_seq_rule_set_offsets_byte_range();

        Ok(0..chained_seq_rule_set_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ChainedSequenceContextFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_83) = data.len();

        match ::core::primitive::bool::then_some(value_83 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ChainedSequenceContextFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> ChainedSequenceContextFormat1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.coverage_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage(&self) -> Result<CoverageTable<'a>, ReadError> {
        let data = self.data;
        let Ok(coverage_offset) = self.coverage_offset();

        coverage_offset.resolve(data)
    }

    pub fn chained_seq_rule_set_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.chained_seq_rule_set_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn chained_seq_rule_set_offsets(&self) -> Result<&'a [BigEndian<Nullable<Offset16>>], Never> {
        let Ok(range) = self.chained_seq_rule_set_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn chained_seq_rule_sets(
        &self,
    ) -> Result<ArrayOfNullableOffsets<'a, ChainedSequenceRuleSet<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.chained_seq_rule_set_offsets();
        ArrayOfNullableOffsets::new(offsets, data, ())
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn chained_seq_rule_set_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn chained_seq_rule_set_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(chained_seq_rule_set_count) = self.chained_seq_rule_set_count();
        let Ok(previous) = self.chained_seq_rule_set_count_byte_range();
        let start = previous.end;
        let Ok(value_84) = transforms::to_u64(chained_seq_rule_set_count);

        let end = start.saturating_add(value_84
                .saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    ChainedSequenceContextFormat1::MIN_SIZE
); held }));

impl Default for ChainedSequenceContextFormat1<'_> {
    fn default() -> Self {
        let Ok(value_85) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_85,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ChainedSequenceContextFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ChainedSequenceContextFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "coverage_offset", value: FieldType::from(FieldType::offset(self.coverage_offset(), (self.coverage()).map(Some))) }),
            2u64 => Some(Field { name: "chained_seq_rule_set_count", value: FieldType::from(self.chained_seq_rule_set_count()) }),
            3u64 => Some(Field { name: "chained_seq_rule_set_offsets", value: FieldType::from(self.chained_seq_rule_sets()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ChainedSequenceContextFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for ChainedSequenceRuleSet<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(chained_seq_rule_offsets_byte_range) = self.chained_seq_rule_offsets_byte_range();

        Ok(0..chained_seq_rule_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ChainedSequenceRuleSet<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_86) = data.len();

        match ::core::primitive::bool::then_some(value_86 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ChainedSequenceRuleSet<'a> {
    data: FontData<'a>,
}

impl<'a> ChainedSequenceRuleSet<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn chained_seq_rule_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.chained_seq_rule_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn chained_seq_rule_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.chained_seq_rule_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn chained_seq_rules(&self) -> Result<ArrayOfOffsets<'a, ChainedSequenceRule<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.chained_seq_rule_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn chained_seq_rule_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn chained_seq_rule_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(chained_seq_rule_count) = self.chained_seq_rule_count();
        let Ok(previous) = self.chained_seq_rule_count_byte_range();
        let start = previous.end;
        let Ok(value_87) = transforms::to_u64(chained_seq_rule_count);

        let end = start.saturating_add(value_87.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    ChainedSequenceRuleSet::MIN_SIZE
); held }));

impl Default for ChainedSequenceRuleSet<'_> {
    fn default() -> Self {
        let Ok(value_88) = FontData::default_table_data();

        Self {
            data: value_88,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ChainedSequenceRuleSet<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ChainedSequenceRuleSet")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "chained_seq_rule_count", value: FieldType::from(self.chained_seq_rule_count()) }),
            1u64 => Some(Field { name: "chained_seq_rule_offsets", value: FieldType::from(self.chained_seq_rules()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ChainedSequenceRuleSet<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for ChainedSequenceRule<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(seq_lookup_records_byte_range) = self.seq_lookup_records_byte_range();

        Ok(0..seq_lookup_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ChainedSequenceRule<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_89) = data.len();

        match ::core::primitive::bool::then_some(value_89 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ChainedSequenceRule<'a> {
    data: FontData<'a>,
}

impl<'a> ChainedSequenceRule<'a> {
    pub const MIN_SIZE: u64 =
        (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn backtrack_glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.backtrack_glyph_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn backtrack_sequence(&self) -> Result<&'a [BigEndian<GlyphId16>], Never> {
        let Ok(range) = self.backtrack_sequence_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn input_glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.input_glyph_count_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn input_sequence(&self) -> Result<&'a [BigEndian<GlyphId16>], Never> {
        let Ok(range) = self.input_sequence_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn lookahead_glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.lookahead_glyph_count_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn lookahead_sequence(&self) -> Result<&'a [BigEndian<GlyphId16>], Never> {
        let Ok(range) = self.lookahead_sequence_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn seq_lookup_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.seq_lookup_count_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn seq_lookup_records(&self) -> Result<&'a [SequenceLookupRecord], Never> {
        let Ok(range) = self.seq_lookup_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn backtrack_glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn backtrack_sequence_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(backtrack_glyph_count) = self.backtrack_glyph_count();
        let Ok(previous) = self.backtrack_glyph_count_byte_range();
        let start = previous.end;
        let Ok(value_90) = transforms::to_u64(backtrack_glyph_count);

        let end = start.saturating_add(value_90.saturating_mul(GlyphId16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn input_glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.backtrack_sequence_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn input_sequence_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(input_glyph_count) = self.input_glyph_count();
        let Ok(previous) = self.input_glyph_count_byte_range();
        let start = previous.end;
        let Ok(value_91) = transforms::subtract(input_glyph_count, 1_u64);

        let end = start.saturating_add(value_91
                .saturating_mul(GlyphId16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn lookahead_glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.input_sequence_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn lookahead_sequence_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(lookahead_glyph_count) = self.lookahead_glyph_count();
        let Ok(previous) = self.lookahead_glyph_count_byte_range();
        let start = previous.end;
        let Ok(value_92) = transforms::to_u64(lookahead_glyph_count);

        let end = start.saturating_add(value_92.saturating_mul(GlyphId16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn seq_lookup_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.lookahead_sequence_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn seq_lookup_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(seq_lookup_count) = self.seq_lookup_count();
        let Ok(previous) = self.seq_lookup_count_byte_range();
        let start = previous.end;
        let Ok(value_93) = transforms::to_u64(seq_lookup_count);

        let end = start.saturating_add(value_93
                .saturating_mul(SequenceLookupRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    ChainedSequenceRule::MIN_SIZE
); held }));

impl Default for ChainedSequenceRule<'_> {
    fn default() -> Self {
        let Ok(value_94) = FontData::default_table_data();

        Self {
            data: value_94,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ChainedSequenceRule<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ChainedSequenceRule")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "backtrack_glyph_count", value: FieldType::from(self.backtrack_glyph_count()) }),
            1u64 => Some(Field { name: "backtrack_sequence", value: FieldType::from(self.backtrack_sequence()) }),
            2u64 => Some(Field { name: "input_glyph_count", value: FieldType::from(self.input_glyph_count()) }),
            3u64 => Some(Field { name: "input_sequence", value: FieldType::from(self.input_sequence()) }),
            4u64 => Some(Field { name: "lookahead_glyph_count", value: FieldType::from(self.lookahead_glyph_count()) }),
            5u64 => Some(Field { name: "lookahead_sequence", value: FieldType::from(self.lookahead_sequence()) }),
            6u64 => Some(Field { name: "seq_lookup_count", value: FieldType::from(self.seq_lookup_count()) }),
            7u64 => {
                let Ok(value_95) = self.offset_data();

                Some(Field { name: "seq_lookup_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(SequenceLookupRecord),
                    { let Ok(explicit_value) = self.seq_lookup_records(); explicit_value },
                    value_95,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ChainedSequenceRule<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for ChainedSequenceContextFormat2<'_> {
    const FORMAT: u16 = 2;
}

impl<'a> MinByteRange<'a> for ChainedSequenceContextFormat2<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(chained_class_seq_rule_set_offsets_byte_range) = self.chained_class_seq_rule_set_offsets_byte_range();

        Ok(0..chained_class_seq_rule_set_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ChainedSequenceContextFormat2<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_96) = data.len();

        match ::core::primitive::bool::then_some(value_96 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ChainedSequenceContextFormat2<'a> {
    data: FontData<'a>,
}

impl<'a> ChainedSequenceContextFormat2<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.coverage_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage(&self) -> Result<CoverageTable<'a>, ReadError> {
        let data = self.data;
        let Ok(coverage_offset) = self.coverage_offset();

        coverage_offset.resolve(data)
    }

    pub fn backtrack_class_def_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.backtrack_class_def_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn backtrack_class_def(&self) -> Result<ClassDef<'a>, ReadError> {
        let data = self.data;
        let Ok(backtrack_class_def_offset) = self.backtrack_class_def_offset();

        backtrack_class_def_offset.resolve(data)
    }

    pub fn input_class_def_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.input_class_def_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn input_class_def(&self) -> Result<ClassDef<'a>, ReadError> {
        let data = self.data;
        let Ok(input_class_def_offset) = self.input_class_def_offset();

        input_class_def_offset.resolve(data)
    }

    pub fn lookahead_class_def_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.lookahead_class_def_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn lookahead_class_def(&self) -> Result<ClassDef<'a>, ReadError> {
        let data = self.data;
        let Ok(lookahead_class_def_offset) = self.lookahead_class_def_offset();

        lookahead_class_def_offset.resolve(data)
    }

    pub fn chained_class_seq_rule_set_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.chained_class_seq_rule_set_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn chained_class_seq_rule_set_offsets(&self) -> Result<&'a [BigEndian<Nullable<Offset16>>], Never> {
        let Ok(range) = self.chained_class_seq_rule_set_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn chained_class_seq_rule_sets(
        &self,
    ) -> Result<ArrayOfNullableOffsets<'a, ChainedClassSequenceRuleSet<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.chained_class_seq_rule_set_offsets();
        ArrayOfNullableOffsets::new(offsets, data, ())
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn backtrack_class_def_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn input_class_def_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.backtrack_class_def_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn lookahead_class_def_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.input_class_def_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn chained_class_seq_rule_set_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.lookahead_class_def_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn chained_class_seq_rule_set_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(chained_class_seq_rule_set_count) = self.chained_class_seq_rule_set_count();
        let Ok(previous) = self.chained_class_seq_rule_set_count_byte_range();
        let start = previous.end;
        let Ok(value_97) = transforms::to_u64(chained_class_seq_rule_set_count);

        let end = start.saturating_add(value_97
                .saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ChainedSequenceContextFormat2<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ChainedSequenceContextFormat2")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "coverage_offset", value: FieldType::from(FieldType::offset(self.coverage_offset(), (self.coverage()).map(Some))) }),
            2u64 => Some(Field { name: "backtrack_class_def_offset", value: FieldType::from(FieldType::offset(
                    self.backtrack_class_def_offset(),
                    (self.backtrack_class_def()).map(Some),
                )) }),
            3u64 => Some(Field { name: "input_class_def_offset", value: FieldType::from(FieldType::offset(self.input_class_def_offset(), (self.input_class_def()).map(Some))) }),
            4u64 => Some(Field { name: "lookahead_class_def_offset", value: FieldType::from(FieldType::offset(
                    self.lookahead_class_def_offset(),
                    (self.lookahead_class_def()).map(Some),
                )) }),
            5u64 => Some(Field { name: "chained_class_seq_rule_set_count", value: FieldType::from(self.chained_class_seq_rule_set_count()) }),
            6u64 => Some(Field { name: "chained_class_seq_rule_set_offsets", value: FieldType::from(self.chained_class_seq_rule_sets()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ChainedSequenceContextFormat2<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for ChainedClassSequenceRuleSet<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(chained_class_seq_rule_offsets_byte_range) = self.chained_class_seq_rule_offsets_byte_range();

        Ok(0..chained_class_seq_rule_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ChainedClassSequenceRuleSet<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_98) = data.len();

        match ::core::primitive::bool::then_some(value_98 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ChainedClassSequenceRuleSet<'a> {
    data: FontData<'a>,
}

impl<'a> ChainedClassSequenceRuleSet<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn chained_class_seq_rule_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.chained_class_seq_rule_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn chained_class_seq_rule_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.chained_class_seq_rule_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn chained_class_seq_rules(
        &self,
    ) -> Result<ArrayOfOffsets<'a, ChainedClassSequenceRule<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.chained_class_seq_rule_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn chained_class_seq_rule_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn chained_class_seq_rule_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(chained_class_seq_rule_count) = self.chained_class_seq_rule_count();
        let Ok(previous) = self.chained_class_seq_rule_count_byte_range();
        let start = previous.end;
        let Ok(value_99) = transforms::to_u64(chained_class_seq_rule_count);

        let end = start.saturating_add(value_99
                .saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    ChainedClassSequenceRuleSet::MIN_SIZE
); held }));

impl Default for ChainedClassSequenceRuleSet<'_> {
    fn default() -> Self {
        let Ok(value_100) = FontData::default_table_data();

        Self {
            data: value_100,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ChainedClassSequenceRuleSet<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ChainedClassSequenceRuleSet")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "chained_class_seq_rule_count", value: FieldType::from(self.chained_class_seq_rule_count()) }),
            1u64 => Some(Field { name: "chained_class_seq_rule_offsets", value: FieldType::from(self.chained_class_seq_rules()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ChainedClassSequenceRuleSet<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for ChainedClassSequenceRule<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(seq_lookup_records_byte_range) = self.seq_lookup_records_byte_range();

        Ok(0..seq_lookup_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ChainedClassSequenceRule<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_101) = data.len();

        match ::core::primitive::bool::then_some(value_101 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ChainedClassSequenceRule<'a> {
    data: FontData<'a>,
}

impl<'a> ChainedClassSequenceRule<'a> {
    pub const MIN_SIZE: u64 =
        (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn backtrack_glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.backtrack_glyph_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn backtrack_sequence(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.backtrack_sequence_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn input_glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.input_glyph_count_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn input_sequence(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.input_sequence_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn lookahead_glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.lookahead_glyph_count_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn lookahead_sequence(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.lookahead_sequence_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn seq_lookup_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.seq_lookup_count_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn seq_lookup_records(&self) -> Result<&'a [SequenceLookupRecord], Never> {
        let Ok(range) = self.seq_lookup_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn backtrack_glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn backtrack_sequence_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(backtrack_glyph_count) = self.backtrack_glyph_count();
        let Ok(previous) = self.backtrack_glyph_count_byte_range();
        let start = previous.end;
        let Ok(value_102) = transforms::to_u64(backtrack_glyph_count);

        let end =
            start.saturating_add(value_102.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn input_glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.backtrack_sequence_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn input_sequence_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(input_glyph_count) = self.input_glyph_count();
        let Ok(previous) = self.input_glyph_count_byte_range();
        let start = previous.end;
        let Ok(value_103) = transforms::subtract(input_glyph_count, 1_u64);

        let end = start.saturating_add(value_103.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn lookahead_glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.input_sequence_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn lookahead_sequence_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(lookahead_glyph_count) = self.lookahead_glyph_count();
        let Ok(previous) = self.lookahead_glyph_count_byte_range();
        let start = previous.end;
        let Ok(value_104) = transforms::to_u64(lookahead_glyph_count);

        let end =
            start.saturating_add(value_104.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn seq_lookup_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.lookahead_sequence_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn seq_lookup_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(seq_lookup_count) = self.seq_lookup_count();
        let Ok(previous) = self.seq_lookup_count_byte_range();
        let start = previous.end;
        let Ok(value_105) = transforms::to_u64(seq_lookup_count);

        let end = start.saturating_add(value_105
                .saturating_mul(SequenceLookupRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    ChainedClassSequenceRule::MIN_SIZE
); held }));

impl Default for ChainedClassSequenceRule<'_> {
    fn default() -> Self {
        let Ok(value_106) = FontData::default_table_data();

        Self {
            data: value_106,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ChainedClassSequenceRule<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ChainedClassSequenceRule")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "backtrack_glyph_count", value: FieldType::from(self.backtrack_glyph_count()) }),
            1u64 => Some(Field { name: "backtrack_sequence", value: FieldType::from(self.backtrack_sequence()) }),
            2u64 => Some(Field { name: "input_glyph_count", value: FieldType::from(self.input_glyph_count()) }),
            3u64 => Some(Field { name: "input_sequence", value: FieldType::from(self.input_sequence()) }),
            4u64 => Some(Field { name: "lookahead_glyph_count", value: FieldType::from(self.lookahead_glyph_count()) }),
            5u64 => Some(Field { name: "lookahead_sequence", value: FieldType::from(self.lookahead_sequence()) }),
            6u64 => Some(Field { name: "seq_lookup_count", value: FieldType::from(self.seq_lookup_count()) }),
            7u64 => {
                let Ok(value_107) = self.offset_data();

                Some(Field { name: "seq_lookup_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(SequenceLookupRecord),
                    { let Ok(explicit_value) = self.seq_lookup_records(); explicit_value },
                    value_107,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ChainedClassSequenceRule<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for ChainedSequenceContextFormat3<'_> {
    const FORMAT: u16 = 3;
}

impl<'a> MinByteRange<'a> for ChainedSequenceContextFormat3<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(seq_lookup_records_byte_range) = self.seq_lookup_records_byte_range();

        Ok(0..seq_lookup_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ChainedSequenceContextFormat3<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_108) = data.len();

        match ::core::primitive::bool::then_some(value_108 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ChainedSequenceContextFormat3<'a> {
    data: FontData<'a>,
}

impl<'a> ChainedSequenceContextFormat3<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn backtrack_glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.backtrack_glyph_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn backtrack_coverage_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.backtrack_coverage_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn backtrack_coverages(&self) -> Result<ArrayOfOffsets<'a, CoverageTable<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.backtrack_coverage_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn input_glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.input_glyph_count_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn input_coverage_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.input_coverage_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn input_coverages(&self) -> Result<ArrayOfOffsets<'a, CoverageTable<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.input_coverage_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn lookahead_glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.lookahead_glyph_count_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn lookahead_coverage_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.lookahead_coverage_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn lookahead_coverages(&self) -> Result<ArrayOfOffsets<'a, CoverageTable<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.lookahead_coverage_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn seq_lookup_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.seq_lookup_count_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn seq_lookup_records(&self) -> Result<&'a [SequenceLookupRecord], Never> {
        let Ok(range) = self.seq_lookup_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn backtrack_glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn backtrack_coverage_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(backtrack_glyph_count) = self.backtrack_glyph_count();
        let Ok(previous) = self.backtrack_glyph_count_byte_range();
        let start = previous.end;
        let Ok(value_109) = transforms::to_u64(backtrack_glyph_count);

        let end = start.saturating_add(value_109.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn input_glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.backtrack_coverage_offsets_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn input_coverage_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(input_glyph_count) = self.input_glyph_count();
        let Ok(previous) = self.input_glyph_count_byte_range();
        let start = previous.end;
        let Ok(value_110) = transforms::to_u64(input_glyph_count);

        let end = start.saturating_add(value_110.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn lookahead_glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.input_coverage_offsets_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn lookahead_coverage_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(lookahead_glyph_count) = self.lookahead_glyph_count();
        let Ok(previous) = self.lookahead_glyph_count_byte_range();
        let start = previous.end;
        let Ok(value_111) = transforms::to_u64(lookahead_glyph_count);

        let end = start.saturating_add(value_111.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn seq_lookup_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.lookahead_coverage_offsets_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn seq_lookup_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(seq_lookup_count) = self.seq_lookup_count();
        let Ok(previous) = self.seq_lookup_count_byte_range();
        let start = previous.end;
        let Ok(value_112) = transforms::to_u64(seq_lookup_count);

        let end = start.saturating_add(value_112
                .saturating_mul(SequenceLookupRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ChainedSequenceContextFormat3<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ChainedSequenceContextFormat3")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "backtrack_glyph_count", value: FieldType::from(self.backtrack_glyph_count()) }),
            2u64 => Some(Field { name: "backtrack_coverage_offsets", value: FieldType::from(self.backtrack_coverages()) }),
            3u64 => Some(Field { name: "input_glyph_count", value: FieldType::from(self.input_glyph_count()) }),
            4u64 => Some(Field { name: "input_coverage_offsets", value: FieldType::from(self.input_coverages()) }),
            5u64 => Some(Field { name: "lookahead_glyph_count", value: FieldType::from(self.lookahead_glyph_count()) }),
            6u64 => Some(Field { name: "lookahead_coverage_offsets", value: FieldType::from(self.lookahead_coverages()) }),
            7u64 => Some(Field { name: "seq_lookup_count", value: FieldType::from(self.seq_lookup_count()) }),
            8u64 => {
                let Ok(value_113) = self.offset_data();

                Some(Field { name: "seq_lookup_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(SequenceLookupRecord),
                    { let Ok(explicit_value) = self.seq_lookup_records(); explicit_value },
                    value_113,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ChainedSequenceContextFormat3<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone)]
pub enum ChainedSequenceContext<'a> {
    Format1(ChainedSequenceContextFormat1<'a>),
    Format2(ChainedSequenceContextFormat2<'a>),
    Format3(ChainedSequenceContextFormat3<'a>),
}

impl Default for ChainedSequenceContext<'_> {
    fn default() -> Self {
        Self::Format1(Default::default())
    }
}

impl<'a> ChainedSequenceContext<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Format1(item) => item.offset_data(),
            Self::Format2(item) => item.offset_data(),
            Self::Format3(item) => item.offset_data(),
        }
    }

    pub fn format(&self) -> Result<u16, Never> {
        match self {
            Self::Format1(item) => item.format(),
            Self::Format2(item) => item.format(),
            Self::Format3(item) => item.format(),
        }
    }
}

impl<'a> FontRead<'a> for ChainedSequenceContext<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u16 = data.read_at(0u64)?;

        match format {
            ChainedSequenceContextFormat1::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format1(read))
            }
            ChainedSequenceContextFormat2::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format2(read))
            }
            ChainedSequenceContextFormat3::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format3(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u16>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for ChainedSequenceContext<'a> {
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
impl<'a> ChainedSequenceContext<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Format1(table) => table,
            Self::Format2(table) => table,
            Self::Format3(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for ChainedSequenceContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ChainedSequenceContext<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u16)]
#[cfg_attr(
    dylint_lib = "explicit070_spaced_discriminants",
    expect(explicit070_spaced_discriminants, reason = "the OpenType device delta format and variation index have these wire codes")
)]
pub enum DeltaFormat {
    #[default]
    Local2BitDeltas = 0x0001,
    Local4BitDeltas = 0x0002,
    Local8BitDeltas = 0x0003,
    VariationIndex = 0x8000,
    #[doc(hidden)]
    Unknown,
}

impl DeltaFormat {
    fn discriminant_value(self) -> Result<u16, Never> {
        Ok(match self {
            Self::Local2BitDeltas => 0x0001,
            Self::Local4BitDeltas => 0x0002,
            Self::Local8BitDeltas => 0x0003,
            Self::VariationIndex => 0x8000,
            Self::Unknown => 32769,
        })
    }

    pub fn new(raw: u16) -> Result<Self, Never> {
        Ok(match raw {
            0x0001 => Self::Local2BitDeltas,
            0x0002 => Self::Local4BitDeltas,
            0x0003 => Self::Local8BitDeltas,
            0x8000 => Self::VariationIndex,
            _ => Self::Unknown,
        })
    }
}

impl font_types::Scalar for DeltaFormat {
    type Raw = <u16 as font_types::Scalar>::Raw;

    fn to_raw(self) -> std::result::Result<Self::Raw, console_core_never::Never> {
        let Ok(value) = self.discriminant_value();
        value.to_raw()
    }

    fn from_raw(raw: Self::Raw) -> std::result::Result<Self, console_core_never::Never> {
        let Ok(value_118) = <u16>::from_raw(raw);

        let t = value_118;
        let value_119 = t;

        let Ok(delta_format) = Self::new(value_119);

        Ok(delta_format)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> From<DeltaFormat> for FieldType<'a> {
    fn from(src: DeltaFormat) -> FieldType<'a> {
        let Ok(value) = src.discriminant_value();
        FieldType::from(value)
    }
}

impl<'a> MinByteRange<'a> for Device<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(delta_value_byte_range) = self.delta_value_byte_range();

        Ok(0..delta_value_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Device<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_120) = data.len();

        match ::core::primitive::bool::then_some(value_120 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Device<'a> {
    data: FontData<'a>,
}

impl<'a> Device<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + DeltaFormat::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn start_size(&self) -> Result<u16, Never> {
        let Ok(range) = self.start_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn end_size(&self) -> Result<u16, Never> {
        let Ok(range) = self.end_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn delta_format(&self) -> Result<DeltaFormat, Never> {
        let Ok(range) = self.delta_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn delta_value(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.delta_value_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn start_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn end_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.start_size_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn delta_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.end_size_byte_range();
        let start = previous.end;
        let end = start.saturating_add(DeltaFormat::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn delta_value_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(delta_format) = self.delta_format();
        let Ok(start_size) = self.start_size();
        let Ok(end_size) = self.end_size();
        let Ok(previous) = self.delta_format_byte_range();
        let start = previous.end;
        let Ok(value_count) = DeltaFormat::value_count(delta_format, start_size..=end_size);
        let end = start.saturating_add(value_count
                .saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Device::MIN_SIZE); held }));

impl Default for Device<'_> {
    fn default() -> Self {
        let Ok(value_121) = FontData::default_table_data();

        Self {
            data: value_121,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Device<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Device")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "start_size", value: FieldType::from(self.start_size()) }),
            1u64 => Some(Field { name: "end_size", value: FieldType::from(self.end_size()) }),
            2u64 => Some(Field { name: "delta_format", value: FieldType::from(self.delta_format()) }),
            3u64 => Some(Field { name: "delta_value", value: FieldType::from(self.delta_value()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Device<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for VariationIndex<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(delta_format_byte_range) = self.delta_format_byte_range();

        Ok(0..delta_format_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for VariationIndex<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_122) = data.len();

        match ::core::primitive::bool::then_some(value_122 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct VariationIndex<'a> {
    data: FontData<'a>,
}

impl<'a> VariationIndex<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + DeltaFormat::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn delta_set_outer_index(&self) -> Result<u16, Never> {
        let Ok(range) = self.delta_set_outer_index_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn delta_set_inner_index(&self) -> Result<u16, Never> {
        let Ok(range) = self.delta_set_inner_index_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn delta_format(&self) -> Result<DeltaFormat, Never> {
        let Ok(range) = self.delta_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn delta_set_outer_index_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn delta_set_inner_index_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.delta_set_outer_index_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn delta_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.delta_set_inner_index_byte_range();
        let start = previous.end;
        let end = start.saturating_add(DeltaFormat::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(VariationIndex::MIN_SIZE); held }));

impl Default for VariationIndex<'_> {
    fn default() -> Self {
        let Ok(value_123) = FontData::default_table_data();

        Self {
            data: value_123,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for VariationIndex<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("VariationIndex")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "delta_set_outer_index", value: FieldType::from(self.delta_set_outer_index()) }),
            1u64 => Some(Field { name: "delta_set_inner_index", value: FieldType::from(self.delta_set_inner_index()) }),
            2u64 => Some(Field { name: "delta_format", value: FieldType::from(self.delta_format()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for VariationIndex<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone)]
pub enum DeviceOrVariationIndex<'a> {
    Device(Device<'a>),
    VariationIndex(VariationIndex<'a>),
}

impl Default for DeviceOrVariationIndex<'_> {
    fn default() -> Self {
        Self::Device(Default::default())
    }
}

impl<'a> DeviceOrVariationIndex<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Device(item) => item.offset_data(),
            Self::VariationIndex(item) => item.offset_data(),
        }
    }
}

impl<'a> FontRead<'a> for DeviceOrVariationIndex<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: DeltaFormat = data.read_at(4u64)?;

        match format {
            DeltaFormat::Local2BitDeltas | DeltaFormat::Local4BitDeltas
            | DeltaFormat::Local8BitDeltas | DeltaFormat::Unknown => {
                let read = FontRead::read(data)?;

                Ok(Self::Device(read))
            }
            DeltaFormat::VariationIndex => {
                let read = FontRead::read(data)?;

                Ok(Self::VariationIndex(read))
            }
        }
    }
}

impl<'a> MinByteRange<'a> for DeviceOrVariationIndex<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        match self {
            Self::Device(item) => item.min_byte_range(),
            Self::VariationIndex(item) => item.min_byte_range(),
        }
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        match self {
            Self::Device(item) => item.min_table_bytes(),
            Self::VariationIndex(item) => item.min_table_bytes(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> DeviceOrVariationIndex<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Device(table) => table,
            Self::VariationIndex(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for DeviceOrVariationIndex<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for DeviceOrVariationIndex<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}

impl<'a> MinByteRange<'a> for FeatureVariations<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(feature_variation_records_byte_range) = self.feature_variation_records_byte_range();

        Ok(0..feature_variation_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for FeatureVariations<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_126) = data.len();

        match ::core::primitive::bool::then_some(value_126 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct FeatureVariations<'a> {
    data: FontData<'a>,
}

impl<'a> FeatureVariations<'a> {
    pub const MIN_SIZE: u64 = (MajorMinor::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<MajorMinor, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn feature_variation_record_count(&self) -> Result<u32, Never> {
        let Ok(range) = self.feature_variation_record_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn feature_variation_records(&self) -> Result<&'a [FeatureVariationRecord], Never> {
        let Ok(range) = self.feature_variation_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(MajorMinor::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn feature_variation_record_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn feature_variation_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(feature_variation_record_count) = self.feature_variation_record_count();
        let Ok(previous) = self.feature_variation_record_count_byte_range();
        let start = previous.end;
        let Ok(value_127) = transforms::to_u64(feature_variation_record_count);

        let end = start.saturating_add(value_127
                .saturating_mul(FeatureVariationRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    FeatureVariations::MIN_SIZE
); held }));

impl Default for FeatureVariations<'_> {
    fn default() -> Self {
        let Ok(value_128) = FontData::default_table_data();

        Self {
            data: value_128,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for FeatureVariations<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("FeatureVariations")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "feature_variation_record_count", value: FieldType::from(self.feature_variation_record_count()) }),
            2u64 => {
                let Ok(value_129) = self.offset_data();

                Some(Field { name: "feature_variation_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(FeatureVariationRecord),
                    { let Ok(explicit_value) = self.feature_variation_records(); explicit_value },
                    value_129,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for FeatureVariations<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct FeatureVariationRecord {
    pub condition_set_offset: BigEndian<Nullable<Offset32>>,
    pub feature_table_substitution_offset: BigEndian<Nullable<Offset32>>,
}

const _: () = {
  assert!(core::mem::size_of::<FeatureVariationRecord>() == 8);
  assert!(core::mem::align_of::<FeatureVariationRecord>() == 1);
  assert!(core::mem::offset_of!(FeatureVariationRecord, condition_set_offset) == 0);
  assert!(core::mem::offset_of!(FeatureVariationRecord, feature_table_substitution_offset) == 4);
};

impl FeatureVariationRecord {
    pub fn condition_set_offset(&self) -> Result<Nullable<Offset32>, Never> {
        self.condition_set_offset.get()
    }

    pub fn condition_set<'a>(
        &self,
        data: FontData<'a>,
    ) -> Result<Option<ConditionSet<'a>>, ReadError> {
        let Ok(condition_set_offset) = self.condition_set_offset();

        condition_set_offset.resolve(data)
    }

    pub fn feature_table_substitution_offset(&self) -> Result<Nullable<Offset32>, Never> {
        self.feature_table_substitution_offset.get()
    }

    pub fn feature_table_substitution<'a>(
        &self,
        data: FontData<'a>,
    ) -> Result<Option<FeatureTableSubstitution<'a>>, ReadError> {
        let Ok(feature_table_substitution_offset) = self.feature_table_substitution_offset();

        feature_table_substitution_offset.resolve(data)
    }
}

impl FixedSize for FeatureVariationRecord {
    const RAW_BYTE_LEN: u64 = Offset32::RAW_BYTE_LEN + Offset32::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for FeatureVariationRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "FeatureVariationRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "condition_set_offset", value: FieldType::from(FieldType::offset(self.condition_set_offset(), self.condition_set(_data))) }),
                1u64 => Some(Field { name: "feature_table_substitution_offset", value: FieldType::from(FieldType::offset(
                        self.feature_table_substitution_offset(),
                        self.feature_table_substitution(_data),
                    )) }),
                _ => None,
            }),
            data,
        })
    }
}

impl<'a> MinByteRange<'a> for ConditionSet<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(condition_offsets_byte_range) = self.condition_offsets_byte_range();

        Ok(0..condition_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ConditionSet<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_132) = data.len();

        match ::core::primitive::bool::then_some(value_132 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ConditionSet<'a> {
    data: FontData<'a>,
}

impl<'a> ConditionSet<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn condition_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.condition_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn condition_offsets(&self) -> Result<&'a [BigEndian<Offset32>], Never> {
        let Ok(range) = self.condition_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn conditions(&self) -> Result<ArrayOfOffsets<'a, Condition<'a>, Offset32>, Never> {
        let data = self.data;
        let Ok(offsets) = self.condition_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn condition_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn condition_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(condition_count) = self.condition_count();
        let Ok(previous) = self.condition_count_byte_range();
        let start = previous.end;
        let Ok(value_133) = transforms::to_u64(condition_count);

        let end =
            start.saturating_add(value_133.saturating_mul(Offset32::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(ConditionSet::MIN_SIZE); held }));

impl Default for ConditionSet<'_> {
    fn default() -> Self {
        let Ok(value_134) = FontData::default_table_data();

        Self {
            data: value_134,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ConditionSet<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ConditionSet")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "condition_count", value: FieldType::from(self.condition_count()) }),
            1u64 => Some(Field { name: "condition_offsets", value: FieldType::from(self.conditions()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ConditionSet<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone)]
pub enum Condition<'a> {
    Format1AxisRange(ConditionFormat1<'a>),
    Format2VariableValue(ConditionFormat2<'a>),
    Format3And(ConditionFormat3<'a>),
    Format4Or(ConditionFormat4<'a>),
    Format5Negate(ConditionFormat5<'a>),
}

impl Default for Condition<'_> {
    fn default() -> Self {
        Self::Format1AxisRange(Default::default())
    }
}

impl<'a> Condition<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Format1AxisRange(item) => item.offset_data(),
            Self::Format2VariableValue(item) => item.offset_data(),
            Self::Format3And(item) => item.offset_data(),
            Self::Format4Or(item) => item.offset_data(),
            Self::Format5Negate(item) => item.offset_data(),
        }
    }

    pub fn format(&self) -> Result<u16, Never> {
        match self {
            Self::Format1AxisRange(item) => item.format(),
            Self::Format2VariableValue(item) => item.format(),
            Self::Format3And(item) => item.format(),
            Self::Format4Or(item) => item.format(),
            Self::Format5Negate(item) => item.format(),
        }
    }
}

impl<'a> FontRead<'a> for Condition<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u16 = data.read_at(0u64)?;

        match format {
            ConditionFormat1::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format1AxisRange(read))
            }
            ConditionFormat2::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format2VariableValue(read))
            }
            ConditionFormat3::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format3And(read))
            }
            ConditionFormat4::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format4Or(read))
            }
            ConditionFormat5::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format5Negate(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u16>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for Condition<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        match self {
            Self::Format1AxisRange(item) => item.min_byte_range(),
            Self::Format2VariableValue(item) => item.min_byte_range(),
            Self::Format3And(item) => item.min_byte_range(),
            Self::Format4Or(item) => item.min_byte_range(),
            Self::Format5Negate(item) => item.min_byte_range(),
        }
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        match self {
            Self::Format1AxisRange(item) => item.min_table_bytes(),
            Self::Format2VariableValue(item) => item.min_table_bytes(),
            Self::Format3And(item) => item.min_table_bytes(),
            Self::Format4Or(item) => item.min_table_bytes(),
            Self::Format5Negate(item) => item.min_table_bytes(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> Condition<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Format1AxisRange(table) => table,
            Self::Format2VariableValue(table) => table,
            Self::Format3And(table) => table,
            Self::Format4Or(table) => table,
            Self::Format5Negate(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for Condition<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Condition<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}

impl Format<u16> for ConditionFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for ConditionFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(filter_range_max_value_byte_range) = self.filter_range_max_value_byte_range();

        Ok(0..filter_range_max_value_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ConditionFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_140) = data.len();

        match ::core::primitive::bool::then_some(value_140 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ConditionFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> ConditionFormat1<'a> {
    pub const MIN_SIZE: u64 =
        (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn axis_index(&self) -> Result<u16, Never> {
        let Ok(range) = self.axis_index_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn filter_range_min_value(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.filter_range_min_value_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn filter_range_max_value(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.filter_range_max_value_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn axis_index_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn filter_range_min_value_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.axis_index_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn filter_range_max_value_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.filter_range_min_value_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    ConditionFormat1::MIN_SIZE
); held }));

impl Default for ConditionFormat1<'_> {
    fn default() -> Self {
        let Ok(value_141) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_141,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ConditionFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ConditionFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "axis_index", value: FieldType::from(self.axis_index()) }),
            2u64 => Some(Field { name: "filter_range_min_value", value: FieldType::from(self.filter_range_min_value()) }),
            3u64 => Some(Field { name: "filter_range_max_value", value: FieldType::from(self.filter_range_max_value()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ConditionFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for ConditionFormat2<'_> {
    const FORMAT: u16 = 2;
}

impl<'a> MinByteRange<'a> for ConditionFormat2<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(var_index_byte_range) = self.var_index_byte_range();

        Ok(0..var_index_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ConditionFormat2<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_142) = data.len();

        match ::core::primitive::bool::then_some(value_142 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ConditionFormat2<'a> {
    data: FontData<'a>,
}

impl<'a> ConditionFormat2<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + i16::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn default_value(&self) -> Result<i16, Never> {
        let Ok(range) = self.default_value_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn var_index(&self) -> Result<u32, Never> {
        let Ok(range) = self.var_index_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn default_value_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn var_index_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.default_value_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ConditionFormat2<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ConditionFormat2")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "default_value", value: FieldType::from(self.default_value()) }),
            2u64 => Some(Field { name: "var_index", value: FieldType::from(self.var_index()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ConditionFormat2<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for ConditionFormat3<'_> {
    const FORMAT: u16 = 3;
}

impl<'a> MinByteRange<'a> for ConditionFormat3<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(condition_offsets_byte_range) = self.condition_offsets_byte_range();

        Ok(0..condition_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ConditionFormat3<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_143) = data.len();

        match ::core::primitive::bool::then_some(value_143 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ConditionFormat3<'a> {
    data: FontData<'a>,
}

impl<'a> ConditionFormat3<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u8::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn condition_count(&self) -> Result<u8, Never> {
        let Ok(range) = self.condition_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn condition_offsets(&self) -> Result<&'a [BigEndian<Offset24>], Never> {
        let Ok(range) = self.condition_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn conditions(&self) -> Result<ArrayOfOffsets<'a, Condition<'a>, Offset24>, Never> {
        let data = self.data;
        let Ok(offsets) = self.condition_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn condition_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn condition_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(condition_count) = self.condition_count();
        let Ok(previous) = self.condition_count_byte_range();
        let start = previous.end;
        let Ok(value_144) = transforms::to_u64(condition_count);

        let end =
            start.saturating_add(value_144.saturating_mul(Offset24::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ConditionFormat3<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ConditionFormat3")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "condition_count", value: FieldType::from(self.condition_count()) }),
            2u64 => Some(Field { name: "condition_offsets", value: FieldType::from(self.conditions()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ConditionFormat3<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for ConditionFormat4<'_> {
    const FORMAT: u16 = 4;
}

impl<'a> MinByteRange<'a> for ConditionFormat4<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(condition_offsets_byte_range) = self.condition_offsets_byte_range();

        Ok(0..condition_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ConditionFormat4<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_145) = data.len();

        match ::core::primitive::bool::then_some(value_145 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ConditionFormat4<'a> {
    data: FontData<'a>,
}

impl<'a> ConditionFormat4<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u8::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn condition_count(&self) -> Result<u8, Never> {
        let Ok(range) = self.condition_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn condition_offsets(&self) -> Result<&'a [BigEndian<Offset24>], Never> {
        let Ok(range) = self.condition_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn conditions(&self) -> Result<ArrayOfOffsets<'a, Condition<'a>, Offset24>, Never> {
        let data = self.data;
        let Ok(offsets) = self.condition_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn condition_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn condition_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(condition_count) = self.condition_count();
        let Ok(previous) = self.condition_count_byte_range();
        let start = previous.end;
        let Ok(value_146) = transforms::to_u64(condition_count);

        let end =
            start.saturating_add(value_146.saturating_mul(Offset24::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ConditionFormat4<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ConditionFormat4")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "condition_count", value: FieldType::from(self.condition_count()) }),
            2u64 => Some(Field { name: "condition_offsets", value: FieldType::from(self.conditions()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ConditionFormat4<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for ConditionFormat5<'_> {
    const FORMAT: u16 = 5;
}

impl<'a> MinByteRange<'a> for ConditionFormat5<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(condition_offset_byte_range) = self.condition_offset_byte_range();

        Ok(0..condition_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ConditionFormat5<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_147) = data.len();

        match ::core::primitive::bool::then_some(value_147 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ConditionFormat5<'a> {
    data: FontData<'a>,
}

impl<'a> ConditionFormat5<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + Offset24::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn condition_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.condition_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn condition(&self) -> Result<Condition<'a>, ReadError> {
        let data = self.data;
        let Ok(condition_offset) = self.condition_offset();

        condition_offset.resolve(data)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn condition_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ConditionFormat5<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ConditionFormat5")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "condition_offset", value: FieldType::from(FieldType::offset(self.condition_offset(), (self.condition()).map(Some))) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ConditionFormat5<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for FeatureTableSubstitution<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(substitutions_byte_range) = self.substitutions_byte_range();

        Ok(0..substitutions_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for FeatureTableSubstitution<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_148) = data.len();

        match ::core::primitive::bool::then_some(value_148 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct FeatureTableSubstitution<'a> {
    data: FontData<'a>,
}

impl<'a> FeatureTableSubstitution<'a> {
    pub const MIN_SIZE: u64 = (MajorMinor::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<MajorMinor, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn substitution_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.substitution_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn substitutions(&self) -> Result<&'a [FeatureTableSubstitutionRecord], Never> {
        let Ok(range) = self.substitutions_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(MajorMinor::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn substitution_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn substitutions_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(substitution_count) = self.substitution_count();
        let Ok(previous) = self.substitution_count_byte_range();
        let start = previous.end;
        let Ok(value_149) = transforms::to_u64(substitution_count);

        let end = start.saturating_add(value_149
                .saturating_mul(FeatureTableSubstitutionRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    FeatureTableSubstitution::MIN_SIZE
); held }));

impl Default for FeatureTableSubstitution<'_> {
    fn default() -> Self {
        let Ok(value_150) = FontData::default_table_data();

        Self {
            data: value_150,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for FeatureTableSubstitution<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("FeatureTableSubstitution")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "substitution_count", value: FieldType::from(self.substitution_count()) }),
            2u64 => {
                let Ok(value_151) = self.offset_data();

                Some(Field { name: "substitutions", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(FeatureTableSubstitutionRecord),
                    { let Ok(explicit_value) = self.substitutions(); explicit_value },
                    value_151,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for FeatureTableSubstitution<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct FeatureTableSubstitutionRecord {
    pub feature_index: BigEndian<u16>,
    pub alternate_feature_offset: BigEndian<Offset32>,
}

const _: () = {
  assert!(core::mem::size_of::<FeatureTableSubstitutionRecord>() == 6);
  assert!(core::mem::align_of::<FeatureTableSubstitutionRecord>() == 1);
  assert!(core::mem::offset_of!(FeatureTableSubstitutionRecord, feature_index) == 0);
  assert!(core::mem::offset_of!(FeatureTableSubstitutionRecord, alternate_feature_offset) == 2);
};

impl FeatureTableSubstitutionRecord {
    pub fn feature_index(&self) -> Result<u16, Never> {
        self.feature_index.get()
    }

    pub fn alternate_feature_offset(&self) -> Result<Offset32, Never> {
        self.alternate_feature_offset.get()
    }
}

impl FixedSize for FeatureTableSubstitutionRecord {
    const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN + Offset32::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for FeatureTableSubstitutionRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "FeatureTableSubstitutionRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "feature_index", value: FieldType::from(self.feature_index()) }),
                1u64 => Some(Field { name: "alternate_feature_offset", value: FieldType::from(FieldType::offset(
                        self.alternate_feature_offset(),
                        (self.alternate_feature(_data)).map(Some),
                    )) }),
                _ => None,
            }),
            data,
        })
    }
}

impl<'a> MinByteRange<'a> for SizeParams<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(range_end_byte_range) = self.range_end_byte_range();

        Ok(0..range_end_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for SizeParams<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_154) = data.len();

        match ::core::primitive::bool::then_some(value_154 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct SizeParams<'a> {
    data: FontData<'a>,
}

impl<'a> SizeParams<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn design_size(&self) -> Result<u16, Never> {
        let Ok(range) = self.design_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn identifier(&self) -> Result<u16, Never> {
        let Ok(range) = self.identifier_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn name_entry(&self) -> Result<u16, Never> {
        let Ok(range) = self.name_entry_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn range_start(&self) -> Result<u16, Never> {
        let Ok(range) = self.range_start_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn range_end(&self) -> Result<u16, Never> {
        let Ok(range) = self.range_end_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn design_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn identifier_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.design_size_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn name_entry_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.identifier_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn range_start_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.name_entry_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn range_end_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.range_start_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(SizeParams::MIN_SIZE); held }));

impl Default for SizeParams<'_> {
    fn default() -> Self {
        let Ok(value_155) = FontData::default_table_data();

        Self {
            data: value_155,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for SizeParams<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("SizeParams")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "design_size", value: FieldType::from(self.design_size()) }),
            1u64 => Some(Field { name: "identifier", value: FieldType::from(self.identifier()) }),
            2u64 => Some(Field { name: "name_entry", value: FieldType::from(self.name_entry()) }),
            3u64 => Some(Field { name: "range_start", value: FieldType::from(self.range_start()) }),
            4u64 => Some(Field { name: "range_end", value: FieldType::from(self.range_end()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for SizeParams<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for StylisticSetParams<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(ui_name_id_byte_range) = self.ui_name_id_byte_range();

        Ok(0..ui_name_id_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for StylisticSetParams<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_156) = data.len();

        match ::core::primitive::bool::then_some(value_156 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct StylisticSetParams<'a> {
    data: FontData<'a>,
}

impl<'a> StylisticSetParams<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + NameId::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<u16, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn ui_name_id(&self) -> Result<NameId, Never> {
        let Ok(range) = self.ui_name_id_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn ui_name_id_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(NameId::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    StylisticSetParams::MIN_SIZE
); held }));

impl Default for StylisticSetParams<'_> {
    fn default() -> Self {
        let Ok(value_157) = FontData::default_table_data();

        Self {
            data: value_157,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for StylisticSetParams<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("StylisticSetParams")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "ui_name_id", value: FieldType::from(self.ui_name_id()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for StylisticSetParams<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for CharacterVariantParams<'_> {
    const FORMAT: u16 = 0;
}

impl<'a> MinByteRange<'a> for CharacterVariantParams<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(character_byte_range) = self.character_byte_range();

        Ok(0..character_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for CharacterVariantParams<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_158) = data.len();

        match ::core::primitive::bool::then_some(value_158 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct CharacterVariantParams<'a> {
    data: FontData<'a>,
}

impl<'a> CharacterVariantParams<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + NameId::RAW_BYTE_LEN
        + NameId::RAW_BYTE_LEN
        + NameId::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + NameId::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn feat_ui_label_name_id(&self) -> Result<NameId, Never> {
        let Ok(range) = self.feat_ui_label_name_id_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn feat_ui_tooltip_text_name_id(&self) -> Result<NameId, Never> {
        let Ok(range) = self.feat_ui_tooltip_text_name_id_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn sample_text_name_id(&self) -> Result<NameId, Never> {
        let Ok(range) = self.sample_text_name_id_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_named_parameters(&self) -> Result<u16, Never> {
        let Ok(range) = self.num_named_parameters_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn first_param_ui_label_name_id(&self) -> Result<NameId, Never> {
        let Ok(range) = self.first_param_ui_label_name_id_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn char_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.char_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn character(&self) -> Result<&'a [BigEndian<Uint24>], Never> {
        let Ok(range) = self.character_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn feat_ui_label_name_id_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(NameId::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn feat_ui_tooltip_text_name_id_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.feat_ui_label_name_id_byte_range();
        let start = previous.end;
        let end = start.saturating_add(NameId::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn sample_text_name_id_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.feat_ui_tooltip_text_name_id_byte_range();
        let start = previous.end;
        let end = start.saturating_add(NameId::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_named_parameters_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.sample_text_name_id_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn first_param_ui_label_name_id_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.num_named_parameters_byte_range();
        let start = previous.end;
        let end = start.saturating_add(NameId::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn char_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.first_param_ui_label_name_id_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn character_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(char_count) = self.char_count();
        let Ok(previous) = self.char_count_byte_range();
        let start = previous.end;
        let Ok(value_159) = transforms::to_u64(char_count);

        let end = start.saturating_add(value_159.saturating_mul(Uint24::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    CharacterVariantParams::MIN_SIZE
); held }));

impl Default for CharacterVariantParams<'_> {
    fn default() -> Self {
        let Ok(value_160) = FontData::default_table_data();

        Self {
            data: value_160,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for CharacterVariantParams<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("CharacterVariantParams")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "feat_ui_label_name_id", value: FieldType::from(self.feat_ui_label_name_id()) }),
            2u64 => Some(Field { name: "feat_ui_tooltip_text_name_id", value: FieldType::from(self.feat_ui_tooltip_text_name_id()) }),
            3u64 => Some(Field { name: "sample_text_name_id", value: FieldType::from(self.sample_text_name_id()) }),
            4u64 => Some(Field { name: "num_named_parameters", value: FieldType::from(self.num_named_parameters()) }),
            5u64 => Some(Field { name: "first_param_ui_label_name_id", value: FieldType::from(self.first_param_ui_label_name_id()) }),
            6u64 => Some(Field { name: "char_count", value: FieldType::from(self.char_count()) }),
            7u64 => Some(Field { name: "character", value: FieldType::from(self.character()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for CharacterVariantParams<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
