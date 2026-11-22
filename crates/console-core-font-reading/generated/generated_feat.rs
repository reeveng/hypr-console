

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Feat<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(names_byte_range) = self.names_byte_range();

        Ok(0..names_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Feat<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"feat"); value });
}

impl<'a> FontRead<'a> for Feat<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Feat<'a> {
    data: FontData<'a>,
}

impl<'a> Feat<'a> {
    pub const MIN_SIZE: u64 =
        (MajorMinor::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<MajorMinor, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn feature_name_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.feature_name_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn names(&self) -> Result<&'a [FeatureName], Never> {
        let Ok(range) = self.names_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(MajorMinor::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn feature_name_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn _reserved1_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.feature_name_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn _reserved2_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self._reserved1_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn names_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(feature_name_count) = self.feature_name_count();
        let Ok(previous) = self._reserved2_byte_range();
        let start = previous.end;
        let Ok(value_2) = transforms::to_u64(feature_name_count);

        let end = start.saturating_add(value_2.saturating_mul(FeatureName::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Feat::MIN_SIZE); held }));

impl Default for Feat<'_> {
    fn default() -> Self {
        let Ok(value_3) = FontData::default_table_data();

        Self {
            data: value_3,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Feat<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Feat")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "feature_name_count", value: FieldType::from(self.feature_name_count()) }),
            2u64 => {
                let Ok(value_4) = self.offset_data();

                Some(Field { name: "names", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(FeatureName),
                    { let Ok(explicit_value) = self.names(); explicit_value },
                    value_4,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Feat<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct FeatureName {
    pub feature: BigEndian<u16>,
    pub n_settings: BigEndian<u16>,
    pub setting_table_offset: BigEndian<Offset32>,
    pub feature_flags: BigEndian<u16>,
    pub name_index: BigEndian<NameId>,
}

const _: () = {
  assert!(core::mem::size_of::<FeatureName>() == 12);
  assert!(core::mem::align_of::<FeatureName>() == 1);
  assert!(core::mem::offset_of!(FeatureName, feature) == 0);
  assert!(core::mem::offset_of!(FeatureName, n_settings) == 2);
  assert!(core::mem::offset_of!(FeatureName, setting_table_offset) == 4);
  assert!(core::mem::offset_of!(FeatureName, feature_flags) == 8);
  assert!(core::mem::offset_of!(FeatureName, name_index) == 10);
};

impl FeatureName {
    pub fn feature(&self) -> Result<u16, Never> {
        self.feature.get()
    }

    pub fn n_settings(&self) -> Result<u16, Never> {
        self.n_settings.get()
    }

    pub fn setting_table_offset(&self) -> Result<Offset32, Never> {
        self.setting_table_offset.get()
    }

    pub fn setting_table<'a>(&self, data: FontData<'a>) -> Result<SettingNameArray<'a>, ReadError> {
        let Ok(args) = self.n_settings();
        let Ok(setting_table_offset) = self.setting_table_offset();

        setting_table_offset.resolve_with_args(data, &args)
    }

    pub fn feature_flags(&self) -> Result<u16, Never> {
        self.feature_flags.get()
    }

    pub fn name_index(&self) -> Result<NameId, Never> {
        self.name_index.get()
    }
}

impl FixedSize for FeatureName {
    const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + NameId::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for FeatureName {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "FeatureName",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "feature", value: FieldType::from(self.feature()) }),
                1u64 => Some(Field { name: "n_settings", value: FieldType::from(self.n_settings()) }),
                2u64 => Some(Field { name: "setting_table_offset", value: FieldType::from(FieldType::offset(self.setting_table_offset(), (self.setting_table(_data)).map(Some))) }),
                3u64 => Some(Field { name: "feature_flags", value: FieldType::from(self.feature_flags()) }),
                4u64 => Some(Field { name: "name_index", value: FieldType::from(self.name_index()) }),
                _ => None,
            }),
            data,
        })
    }
}

impl<'a> MinByteRange<'a> for SettingNameArray<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(settings_byte_range) = self.settings_byte_range();

        Ok(0..settings_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl ReadArgs for SettingNameArray<'_> {
    type Args = u16;
}

impl<'a> FontReadWithArgs<'a> for SettingNameArray<'a> {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let n_settings = *args;

        Ok(Self { data, n_settings })
    }
}

impl<'a> SettingNameArray<'a> {
    pub fn read(data: FontData<'a>, n_settings: u16) -> Result<Self, ReadError> {
        let args = n_settings;
        Self::read_with_args(data, &args)
    }
}

#[derive(Clone)]
pub struct SettingNameArray<'a> {
    data: FontData<'a>,
    n_settings: u16,
}

impl<'a> SettingNameArray<'a> {
    pub const MIN_SIZE: u64 = 0;
    basic_table_impls!(impl_the_methods);

    pub fn settings(&self) -> Result<&'a [SettingName], Never> {
        let Ok(range) = self.settings_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub(crate) fn n_settings(&self) -> Result<u16, Never> {
        Ok(self.n_settings)
    }

    pub fn settings_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(n_settings) = self.n_settings();
        let start = 0u64;
        let Ok(value_11) = transforms::to_u64(n_settings);

        let end =
            start.saturating_add(value_11.saturating_mul(SettingName::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    SettingNameArray::MIN_SIZE
); held }));

impl Default for SettingNameArray<'_> {
    fn default() -> Self {
        let Ok(value_12) = FontData::default_table_data();

        Self {
            data: value_12,
            n_settings: Default::default(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for SettingNameArray<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("SettingNameArray")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => {
                let Ok(value_13) = self.offset_data();

                Some(Field { name: "settings", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(SettingName),
                    { let Ok(explicit_value) = self.settings(); explicit_value },
                    value_13,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for SettingNameArray<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct SettingName {
    pub setting: BigEndian<u16>,
    pub name_index: BigEndian<NameId>,
}

const _: () = {
  assert!(core::mem::size_of::<SettingName>() == 4);
  assert!(core::mem::align_of::<SettingName>() == 1);
  assert!(core::mem::offset_of!(SettingName, setting) == 0);
  assert!(core::mem::offset_of!(SettingName, name_index) == 2);
};

impl SettingName {
    pub fn setting(&self) -> Result<u16, Never> {
        self.setting.get()
    }

    pub fn name_index(&self) -> Result<NameId, Never> {
        self.name_index.get()
    }
}

impl FixedSize for SettingName {
    const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN + NameId::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for SettingName {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "SettingName",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "setting", value: FieldType::from(self.setting()) }),
                1u64 => Some(Field { name: "name_index", value: FieldType::from(self.name_index()) }),
                _ => None,
            }),
            data,
        })
    }
}
