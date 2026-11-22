

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Name<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(name_record_byte_range) = self.name_record_byte_range();

        Ok(0..name_record_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Name<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"name"); value });
}

impl<'a> FontRead<'a> for Name<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Name<'a> {
    data: FontData<'a>,
}

impl<'a> Name<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<u16, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn count(&self) -> Result<u16, Never> {
        let Ok(range) = self.count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn storage_offset(&self) -> Result<u16, Never> {
        let Ok(range) = self.storage_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn name_record(&self) -> Result<&'a [NameRecord], Never> {
        let Ok(range) = self.name_record_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn lang_tag_count(&self) -> Result<Option<u16>, Never> {
        let Ok(range) = self.lang_tag_count_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn lang_tag_record(&self) -> Result<Option<&'a [LangTagRecord]>, Never> {
        let Ok(range) = self.lang_tag_record_byte_range();

        self.data.read_array_if_present(range)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn storage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn name_record_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(count) = self.count();
        let Ok(previous) = self.storage_offset_byte_range();
        let start = previous.end;
        let Ok(value_2) = transforms::to_u64(count);

        let end = start.saturating_add(value_2.saturating_mul(NameRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn lang_tag_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.name_record_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility(1u16);

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(u16::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }

    pub fn lang_tag_record_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(own_lang_tag_count) = self.lang_tag_count();
        let lang_tag_count = match own_lang_tag_count {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => 0,
        };
        let Ok(previous) = self.lang_tag_count_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility(1u16);

        let end = match compatibility {
            Compatibility::Compatible => {
                let Ok(value_5) = transforms::to_u64(lang_tag_count);

            start.saturating_add(value_5.saturating_mul(LangTagRecord::RAW_BYTE_LEN))
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Name::MIN_SIZE); held }));

impl Default for Name<'_> {
    fn default() -> Self {
        let Ok(value_6) = FontData::default_table_data();

        Self {
            data: value_6,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Name<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Name")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "count", value: FieldType::from(self.count()) }),
            2u64 => Some(Field { name: "storage_offset", value: FieldType::from(self.storage_offset()) }),
            3u64 => Some(Field { name: "name_record", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(NameRecord),
                    { let Ok(explicit_value) = self.name_record(); explicit_value },
                    self.string_data(),
                )) }),
            4u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility(1u16);
                let Ok(lang_tag_count) = self.lang_tag_count();

                match compatibility {
                    Compatibility::Compatible => lang_tag_count.map(|lang_tag_count_2| Field { name: "lang_tag_count", value: FieldType::from(lang_tag_count_2) }),
                    Compatibility::Incompatible => None,
                }
            },
            5u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility(1u16);
                let Ok(lang_tag_record) = self.lang_tag_record();

                match compatibility {
                    Compatibility::Compatible => lang_tag_record.map(|lang_tag_record_2| Field { name: "lang_tag_record", value: FieldType::from(traversal::FieldType::array_of_records(
                        stringify!(LangTagRecord),
                        lang_tag_record_2,
                        self.string_data(),
                    )) }),
                    Compatibility::Incompatible => None,
                }
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Name<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct LangTagRecord {
    pub length: BigEndian<u16>,
    pub lang_tag_offset: BigEndian<Offset16>,
}

const _: () = {
  assert!(core::mem::size_of::<LangTagRecord>() == 4);
  assert!(core::mem::align_of::<LangTagRecord>() == 1);
  assert!(core::mem::offset_of!(LangTagRecord, length) == 0);
  assert!(core::mem::offset_of!(LangTagRecord, lang_tag_offset) == 2);
};

impl LangTagRecord {
    pub fn length(&self) -> Result<u16, Never> {
        self.length.get()
    }

    pub fn lang_tag_offset(&self) -> Result<Offset16, Never> {
        self.lang_tag_offset.get()
    }
}

impl FixedSize for LangTagRecord {
    const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for LangTagRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "LangTagRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "length", value: FieldType::from(self.length()) }),
                1u64 => {
                    let Ok(value) = self.traverse_lang_tag(_data);

                    Some(Field { name: "lang_tag_offset", value })
                }
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Debug, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct NameRecord {
    pub platform_id: BigEndian<u16>,
    pub encoding_id: BigEndian<u16>,
    pub language_id: BigEndian<u16>,
    pub name_id: BigEndian<NameId>,
    pub length: BigEndian<u16>,
    pub string_offset: BigEndian<Offset16>,
}

const _: () = {
  assert!(core::mem::size_of::<NameRecord>() == 12);
  assert!(core::mem::align_of::<NameRecord>() == 1);
  assert!(core::mem::offset_of!(NameRecord, platform_id) == 0);
  assert!(core::mem::offset_of!(NameRecord, encoding_id) == 2);
  assert!(core::mem::offset_of!(NameRecord, language_id) == 4);
  assert!(core::mem::offset_of!(NameRecord, name_id) == 6);
  assert!(core::mem::offset_of!(NameRecord, length) == 8);
  assert!(core::mem::offset_of!(NameRecord, string_offset) == 10);
};

impl NameRecord {
    pub fn platform_id(&self) -> Result<u16, Never> {
        self.platform_id.get()
    }

    pub fn encoding_id(&self) -> Result<u16, Never> {
        self.encoding_id.get()
    }

    pub fn language_id(&self) -> Result<u16, Never> {
        self.language_id.get()
    }

    pub fn name_id(&self) -> Result<NameId, Never> {
        self.name_id.get()
    }

    pub fn length(&self) -> Result<u16, Never> {
        self.length.get()
    }

    pub fn string_offset(&self) -> Result<Offset16, Never> {
        self.string_offset.get()
    }
}

impl FixedSize for NameRecord {
    const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + NameId::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for NameRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "NameRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "platform_id", value: FieldType::from(self.platform_id()) }),
                1u64 => Some(Field { name: "encoding_id", value: FieldType::from(self.encoding_id()) }),
                2u64 => Some(Field { name: "language_id", value: FieldType::from(self.language_id()) }),
                3u64 => Some(Field { name: "name_id", value: FieldType::from(self.name_id()) }),
                4u64 => Some(Field { name: "length", value: FieldType::from(self.length()) }),
                5u64 => {
                    let Ok(value) = self.traverse_string(_data);

                    Some(Field { name: "string_offset", value })
                }
                _ => None,
            }),
            data,
        })
    }
}
