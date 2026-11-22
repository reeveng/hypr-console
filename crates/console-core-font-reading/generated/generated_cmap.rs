

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Cmap<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(encoding_records_byte_range) = self.encoding_records_byte_range();

        Ok(0..encoding_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Cmap<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"cmap"); value });
}

impl<'a> FontRead<'a> for Cmap<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Cmap<'a> {
    data: FontData<'a>,
}

impl<'a> Cmap<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<u16, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_tables(&self) -> Result<u16, Never> {
        let Ok(range) = self.num_tables_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn encoding_records(&self) -> Result<&'a [EncodingRecord], Never> {
        let Ok(range) = self.encoding_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_tables_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn encoding_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_tables) = self.num_tables();
        let Ok(previous) = self.num_tables_byte_range();
        let start = previous.end;
        let Ok(value_2) = transforms::to_u64(num_tables);

        let end =
            start.saturating_add(value_2.saturating_mul(EncodingRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Cmap::MIN_SIZE); held }));

impl Default for Cmap<'_> {
    fn default() -> Self {
        let Ok(value_3) = FontData::default_table_data();

        Self {
            data: value_3,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Cmap<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Cmap")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "num_tables", value: FieldType::from(self.num_tables()) }),
            2u64 => {
                let Ok(value_4) = self.offset_data();

                Some(Field { name: "encoding_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(EncodingRecord),
                    { let Ok(explicit_value) = self.encoding_records(); explicit_value },
                    value_4,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Cmap<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct EncodingRecord {
    pub platform_id: BigEndian<PlatformId>,
    pub encoding_id: BigEndian<u16>,
    pub subtable_offset: BigEndian<Offset32>,
}

const _: () = {
  assert!(core::mem::size_of::<EncodingRecord>() == 8);
  assert!(core::mem::align_of::<EncodingRecord>() == 1);
  assert!(core::mem::offset_of!(EncodingRecord, platform_id) == 0);
  assert!(core::mem::offset_of!(EncodingRecord, encoding_id) == 2);
  assert!(core::mem::offset_of!(EncodingRecord, subtable_offset) == 4);
};

impl EncodingRecord {
    pub fn platform_id(&self) -> Result<PlatformId, Never> {
        self.platform_id.get()
    }

    pub fn encoding_id(&self) -> Result<u16, Never> {
        self.encoding_id.get()
    }

    pub fn subtable_offset(&self) -> Result<Offset32, Never> {
        self.subtable_offset.get()
    }

    pub fn subtable<'a>(&self, data: FontData<'a>) -> Result<CmapSubtable<'a>, ReadError> {
        let Ok(subtable_offset) = self.subtable_offset();

        subtable_offset.resolve(data)
    }
}

impl FixedSize for EncodingRecord {
    const RAW_BYTE_LEN: u64 =
        PlatformId::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + Offset32::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for EncodingRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "EncodingRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "platform_id", value: FieldType::from(self.platform_id()) }),
                1u64 => Some(Field { name: "encoding_id", value: FieldType::from(self.encoding_id()) }),
                2u64 => Some(Field { name: "subtable_offset", value: FieldType::from(FieldType::offset(self.subtable_offset(), (self.subtable(_data)).map(Some))) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u16)]
#[cfg_attr(
    dylint_lib = "explicit070_spaced_discriminants",
    expect(explicit070_spaced_discriminants, reason = "OpenType cmap platform identifiers use these specified numeric values")
)]
pub enum PlatformId {
    #[default]
    Unicode = 0,
    Macintosh = 1,
    ISO = 2,
    Windows = 3,
    Custom = 4,
    #[doc(hidden)]
    Unknown,
}

impl PlatformId {
    fn discriminant_value(self) -> Result<u16, Never> {
        Ok(match self {
            Self::Unicode => 0,
            Self::Macintosh => 1,
            Self::ISO => 2,
            Self::Windows => 3,
            Self::Custom => 4,
            Self::Unknown => 5,
        })
    }

    pub fn new(raw: u16) -> Result<Self, Never> {
        Ok(match raw {
            0 => Self::Unicode,
            1 => Self::Macintosh,
            2 => Self::ISO,
            3 => Self::Windows,
            4 => Self::Custom,
            _ => Self::Unknown,
        })
    }
}

impl font_types::Scalar for PlatformId {
    type Raw = <u16 as font_types::Scalar>::Raw;

    fn to_raw(self) -> std::result::Result<Self::Raw, console_core_never::Never> {
        let Ok(value) = self.discriminant_value();
        value.to_raw()
    }

    fn from_raw(raw: Self::Raw) -> std::result::Result<Self, console_core_never::Never> {
        let Ok(value_9) = <u16>::from_raw(raw);

        let t = value_9;
        let value_10 = t;

        let Ok(platform_id) = Self::new(value_10);

        Ok(platform_id)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> From<PlatformId> for FieldType<'a> {
    fn from(src: PlatformId) -> FieldType<'a> {
        let Ok(value) = src.discriminant_value();
        FieldType::from(value)
    }
}

#[derive(Clone)]
pub enum CmapSubtable<'a> {
    Format0(Cmap0<'a>),
    Format2(Cmap2<'a>),
    Format4(Cmap4<'a>),
    Format6(Cmap6<'a>),
    Format8(Cmap8<'a>),
    Format10(Cmap10<'a>),
    Format12(Cmap12<'a>),
    Format13(Cmap13<'a>),
    Format14(Cmap14<'a>),
}

impl Default for CmapSubtable<'_> {
    fn default() -> Self {
        Self::Format0(Default::default())
    }
}

impl<'a> CmapSubtable<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Format0(item) => item.offset_data(),
            Self::Format2(item) => item.offset_data(),
            Self::Format4(item) => item.offset_data(),
            Self::Format6(item) => item.offset_data(),
            Self::Format8(item) => item.offset_data(),
            Self::Format10(item) => item.offset_data(),
            Self::Format12(item) => item.offset_data(),
            Self::Format13(item) => item.offset_data(),
            Self::Format14(item) => item.offset_data(),
        }
    }

    pub fn format(&self) -> Result<u16, Never> {
        match self {
            Self::Format0(item) => item.format(),
            Self::Format2(item) => item.format(),
            Self::Format4(item) => item.format(),
            Self::Format6(item) => item.format(),
            Self::Format8(item) => item.format(),
            Self::Format10(item) => item.format(),
            Self::Format12(item) => item.format(),
            Self::Format13(item) => item.format(),
            Self::Format14(item) => item.format(),
        }
    }
}

impl<'a> FontRead<'a> for CmapSubtable<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u16 = data.read_at(0u64)?;

        match format {
            Cmap0::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format0(read))
            }
            Cmap2::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format2(read))
            }
            Cmap4::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format4(read))
            }
            Cmap6::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format6(read))
            }
            Cmap8::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format8(read))
            }
            Cmap10::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format10(read))
            }
            Cmap12::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format12(read))
            }
            Cmap13::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format13(read))
            }
            Cmap14::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format14(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u16>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for CmapSubtable<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        match self {
            Self::Format0(item) => item.min_byte_range(),
            Self::Format2(item) => item.min_byte_range(),
            Self::Format4(item) => item.min_byte_range(),
            Self::Format6(item) => item.min_byte_range(),
            Self::Format8(item) => item.min_byte_range(),
            Self::Format10(item) => item.min_byte_range(),
            Self::Format12(item) => item.min_byte_range(),
            Self::Format13(item) => item.min_byte_range(),
            Self::Format14(item) => item.min_byte_range(),
        }
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        match self {
            Self::Format0(item) => item.min_table_bytes(),
            Self::Format2(item) => item.min_table_bytes(),
            Self::Format4(item) => item.min_table_bytes(),
            Self::Format6(item) => item.min_table_bytes(),
            Self::Format8(item) => item.min_table_bytes(),
            Self::Format10(item) => item.min_table_bytes(),
            Self::Format12(item) => item.min_table_bytes(),
            Self::Format13(item) => item.min_table_bytes(),
            Self::Format14(item) => item.min_table_bytes(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> CmapSubtable<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Format0(table) => table,
            Self::Format2(table) => table,
            Self::Format4(table) => table,
            Self::Format6(table) => table,
            Self::Format8(table) => table,
            Self::Format10(table) => table,
            Self::Format12(table) => table,
            Self::Format13(table) => table,
            Self::Format14(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for CmapSubtable<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for CmapSubtable<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}

impl Format<u16> for Cmap0<'_> {
    const FORMAT: u16 = 0;
}

impl<'a> MinByteRange<'a> for Cmap0<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(glyph_id_array_byte_range) = self.glyph_id_array_byte_range();

        Ok(0..glyph_id_array_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Cmap0<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_20) = data.len();

        match ::core::primitive::bool::then_some(value_20 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Cmap0<'a> {
    data: FontData<'a>,
}

impl<'a> Cmap0<'a> {
    pub const MIN_SIZE: u64 =
        (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u8::RAW_BYTE_LEN * 256_u64);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn length(&self) -> Result<u16, Never> {
        let Ok(range) = self.length_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn language(&self) -> Result<u16, Never> {
        let Ok(range) = self.language_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn glyph_id_array(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.glyph_id_array_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn length_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn language_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.length_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_id_array_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.language_byte_range();
        let start = previous.end;
        let Ok(value_87) = console_core_number_conversion::index(u8::RAW_BYTE_LEN);

        let Ok(value_77) = console_core_number_conversion::fitted::<_, u64>(value_87);

        let Ok(value_91) = console_core_number_conversion::fitted::<_, u64>((256_u64).saturating_mul(value_77));

        let end = start.saturating_add(value_91);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Cmap0::MIN_SIZE); held }));

impl Default for Cmap0<'_> {
    fn default() -> Self {
        let Ok(value_21) = FontData::default_table_data();

        Self {
            data: value_21,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Cmap0<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Cmap0")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "length", value: FieldType::from(self.length()) }),
            2u64 => Some(Field { name: "language", value: FieldType::from(self.language()) }),
            3u64 => Some(Field { name: "glyph_id_array", value: FieldType::from(self.glyph_id_array()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Cmap0<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for Cmap2<'_> {
    const FORMAT: u16 = 2;
}

impl<'a> MinByteRange<'a> for Cmap2<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(sub_header_keys_byte_range) = self.sub_header_keys_byte_range();

        Ok(0..sub_header_keys_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Cmap2<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_22) = data.len();

        match ::core::primitive::bool::then_some(value_22 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Cmap2<'a> {
    data: FontData<'a>,
}

impl<'a> Cmap2<'a> {
    pub const MIN_SIZE: u64 =
        (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN * 256_u64);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn length(&self) -> Result<u16, Never> {
        let Ok(range) = self.length_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn language(&self) -> Result<u16, Never> {
        let Ok(range) = self.language_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn sub_header_keys(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.sub_header_keys_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn length_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn language_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.length_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn sub_header_keys_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.language_byte_range();
        let start = previous.end;
        let Ok(value_88) = console_core_number_conversion::index(u16::RAW_BYTE_LEN);

        let Ok(value_78) = console_core_number_conversion::fitted::<_, u64>(value_88);

        let Ok(value_92) = console_core_number_conversion::fitted::<_, u64>((256_u64).saturating_mul(value_78));

        let end = start.saturating_add(value_92);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Cmap2<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Cmap2")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "length", value: FieldType::from(self.length()) }),
            2u64 => Some(Field { name: "language", value: FieldType::from(self.language()) }),
            3u64 => Some(Field { name: "sub_header_keys", value: FieldType::from(self.sub_header_keys()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Cmap2<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct SubHeader {
    pub first_code: BigEndian<u16>,
    pub entry_count: BigEndian<u16>,
    pub id_delta: BigEndian<i16>,
    pub id_range_offset: BigEndian<u16>,
}

const _: () = {
  assert!(core::mem::size_of::<SubHeader>() == 8);
  assert!(core::mem::align_of::<SubHeader>() == 1);
  assert!(core::mem::offset_of!(SubHeader, first_code) == 0);
  assert!(core::mem::offset_of!(SubHeader, entry_count) == 2);
  assert!(core::mem::offset_of!(SubHeader, id_delta) == 4);
  assert!(core::mem::offset_of!(SubHeader, id_range_offset) == 6);
};

impl SubHeader {
    pub fn first_code(&self) -> Result<u16, Never> {
        self.first_code.get()
    }

    pub fn entry_count(&self) -> Result<u16, Never> {
        self.entry_count.get()
    }

    pub fn id_delta(&self) -> Result<i16, Never> {
        self.id_delta.get()
    }

    pub fn id_range_offset(&self) -> Result<u16, Never> {
        self.id_range_offset.get()
    }
}

impl FixedSize for SubHeader {
    const RAW_BYTE_LEN: u64 =
        u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + i16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for SubHeader {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "SubHeader",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "first_code", value: FieldType::from(self.first_code()) }),
                1u64 => Some(Field { name: "entry_count", value: FieldType::from(self.entry_count()) }),
                2u64 => Some(Field { name: "id_delta", value: FieldType::from(self.id_delta()) }),
                3u64 => Some(Field { name: "id_range_offset", value: FieldType::from(self.id_range_offset()) }),
                _ => None,
            }),
            data,
        })
    }
}

impl Format<u16> for Cmap4<'_> {
    const FORMAT: u16 = 4;
}

impl<'a> MinByteRange<'a> for Cmap4<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(glyph_id_array_byte_range) = self.glyph_id_array_byte_range();

        Ok(0..glyph_id_array_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Cmap4<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_27) = data.len();

        match ::core::primitive::bool::then_some(value_27 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Cmap4<'a> {
    data: FontData<'a>,
}

impl<'a> Cmap4<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn length(&self) -> Result<u16, Never> {
        let Ok(range) = self.length_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn language(&self) -> Result<u16, Never> {
        let Ok(range) = self.language_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn seg_count_x2(&self) -> Result<u16, Never> {
        let Ok(range) = self.seg_count_x2_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn search_range(&self) -> Result<u16, Never> {
        let Ok(range) = self.search_range_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn entry_selector(&self) -> Result<u16, Never> {
        let Ok(range) = self.entry_selector_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn range_shift(&self) -> Result<u16, Never> {
        let Ok(range) = self.range_shift_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn end_code(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.end_code_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn start_code(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.start_code_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn id_delta(&self) -> Result<&'a [BigEndian<i16>], Never> {
        let Ok(range) = self.id_delta_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn id_range_offsets(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.id_range_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn glyph_id_array(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.glyph_id_array_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn length_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn language_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.length_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn seg_count_x2_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.language_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn search_range_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.seg_count_x2_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn entry_selector_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.search_range_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn range_shift_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.entry_selector_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn end_code_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(seg_count_x2) = self.seg_count_x2();
        let Ok(previous) = self.range_shift_byte_range();
        let start = previous.end;
        let Ok(value_28) = transforms::half(seg_count_x2);

        let end = start.saturating_add(value_28.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn reserved_pad_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.end_code_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn start_code_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(seg_count_x2) = self.seg_count_x2();
        let Ok(previous) = self.reserved_pad_byte_range();
        let start = previous.end;
        let Ok(value_29) = transforms::half(seg_count_x2);

        let end = start.saturating_add(value_29.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn id_delta_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(seg_count_x2) = self.seg_count_x2();
        let Ok(previous) = self.start_code_byte_range();
        let start = previous.end;
        let Ok(value_30) = transforms::half(seg_count_x2);

        let end = start.saturating_add(value_30.saturating_mul(i16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn id_range_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(seg_count_x2) = self.seg_count_x2();
        let Ok(previous) = self.id_delta_byte_range();
        let start = previous.end;
        let Ok(value_31) = transforms::half(seg_count_x2);

        let end = start.saturating_add(value_31.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn glyph_id_array_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.id_range_offsets_byte_range();
        let start = previous.end;
        let Ok(value_79) = self.data.len();

        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_79.saturating_sub(start), item_width: u16::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Cmap4<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Cmap4")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "length", value: FieldType::from(self.length()) }),
            2u64 => Some(Field { name: "language", value: FieldType::from(self.language()) }),
            3u64 => Some(Field { name: "seg_count_x2", value: FieldType::from(self.seg_count_x2()) }),
            4u64 => Some(Field { name: "search_range", value: FieldType::from(self.search_range()) }),
            5u64 => Some(Field { name: "entry_selector", value: FieldType::from(self.entry_selector()) }),
            6u64 => Some(Field { name: "range_shift", value: FieldType::from(self.range_shift()) }),
            7u64 => Some(Field { name: "end_code", value: FieldType::from(self.end_code()) }),
            8u64 => Some(Field { name: "start_code", value: FieldType::from(self.start_code()) }),
            9u64 => Some(Field { name: "id_delta", value: FieldType::from(self.id_delta()) }),
            10u64 => Some(Field { name: "id_range_offsets", value: FieldType::from(self.id_range_offsets()) }),
            11u64 => Some(Field { name: "glyph_id_array", value: FieldType::from(self.glyph_id_array()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Cmap4<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for Cmap6<'_> {
    const FORMAT: u16 = 6;
}

impl<'a> MinByteRange<'a> for Cmap6<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(glyph_id_array_byte_range) = self.glyph_id_array_byte_range();

        Ok(0..glyph_id_array_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Cmap6<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_32) = data.len();

        match ::core::primitive::bool::then_some(value_32 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Cmap6<'a> {
    data: FontData<'a>,
}

impl<'a> Cmap6<'a> {
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

    pub fn length(&self) -> Result<u16, Never> {
        let Ok(range) = self.length_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn language(&self) -> Result<u16, Never> {
        let Ok(range) = self.language_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn first_code(&self) -> Result<u16, Never> {
        let Ok(range) = self.first_code_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn entry_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.entry_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn glyph_id_array(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.glyph_id_array_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn length_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn language_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.length_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn first_code_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.language_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn entry_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.first_code_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_id_array_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(entry_count) = self.entry_count();
        let Ok(previous) = self.entry_count_byte_range();
        let start = previous.end;
        let Ok(value_33) = transforms::to_u64(entry_count);

        let end = start.saturating_add(value_33.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Cmap6<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Cmap6")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "length", value: FieldType::from(self.length()) }),
            2u64 => Some(Field { name: "language", value: FieldType::from(self.language()) }),
            3u64 => Some(Field { name: "first_code", value: FieldType::from(self.first_code()) }),
            4u64 => Some(Field { name: "entry_count", value: FieldType::from(self.entry_count()) }),
            5u64 => Some(Field { name: "glyph_id_array", value: FieldType::from(self.glyph_id_array()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Cmap6<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for Cmap8<'_> {
    const FORMAT: u16 = 8;
}

impl<'a> MinByteRange<'a> for Cmap8<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(groups_byte_range) = self.groups_byte_range();

        Ok(0..groups_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Cmap8<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_34) = data.len();

        match ::core::primitive::bool::then_some(value_34 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Cmap8<'a> {
    data: FontData<'a>,
}

impl<'a> Cmap8<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN * 8192_u64
        + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn length(&self) -> Result<u32, Never> {
        let Ok(range) = self.length_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn language(&self) -> Result<u32, Never> {
        let Ok(range) = self.language_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn is32(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.is32_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn num_groups(&self) -> Result<u32, Never> {
        let Ok(range) = self.num_groups_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn groups(&self) -> Result<&'a [SequentialMapGroup], Never> {
        let Ok(range) = self.groups_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn reserved_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn length_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.reserved_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn language_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.length_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn is32_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.language_byte_range();
        let start = previous.end;
        let Ok(value_89) = console_core_number_conversion::index(u8::RAW_BYTE_LEN);

        let Ok(value_80) = console_core_number_conversion::fitted::<_, u64>(value_89);

        let Ok(value_93) = console_core_number_conversion::fitted::<_, u64>((8192_u64).saturating_mul(value_80));

        let end = start.saturating_add(value_93);
        Ok(start..end)
    }

    pub fn num_groups_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.is32_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn groups_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_groups) = self.num_groups();
        let Ok(previous) = self.num_groups_byte_range();
        let start = previous.end;
        let Ok(value_35) = transforms::to_u64(num_groups);

        let end = start.saturating_add(value_35.saturating_mul(SequentialMapGroup::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Cmap8<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Cmap8")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "length", value: FieldType::from(self.length()) }),
            2u64 => Some(Field { name: "language", value: FieldType::from(self.language()) }),
            3u64 => Some(Field { name: "is32", value: FieldType::from(self.is32()) }),
            4u64 => Some(Field { name: "num_groups", value: FieldType::from(self.num_groups()) }),
            5u64 => {
                let Ok(value_36) = self.offset_data();

                Some(Field { name: "groups", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(SequentialMapGroup),
                    { let Ok(explicit_value) = self.groups(); explicit_value },
                    value_36,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Cmap8<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct SequentialMapGroup {
    pub start_char_code: BigEndian<u32>,
    pub end_char_code: BigEndian<u32>,
    pub start_glyph_id: BigEndian<u32>,
}

const _: () = {
  assert!(core::mem::size_of::<SequentialMapGroup>() == 12);
  assert!(core::mem::align_of::<SequentialMapGroup>() == 1);
  assert!(core::mem::offset_of!(SequentialMapGroup, start_char_code) == 0);
  assert!(core::mem::offset_of!(SequentialMapGroup, end_char_code) == 4);
  assert!(core::mem::offset_of!(SequentialMapGroup, start_glyph_id) == 8);
};

impl SequentialMapGroup {
    pub fn start_char_code(&self) -> Result<u32, Never> {
        self.start_char_code.get()
    }

    pub fn end_char_code(&self) -> Result<u32, Never> {
        self.end_char_code.get()
    }

    pub fn start_glyph_id(&self) -> Result<u32, Never> {
        self.start_glyph_id.get()
    }
}

impl FixedSize for SequentialMapGroup {
    const RAW_BYTE_LEN: u64 = u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for SequentialMapGroup {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "SequentialMapGroup",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "start_char_code", value: FieldType::from(self.start_char_code()) }),
                1u64 => Some(Field { name: "end_char_code", value: FieldType::from(self.end_char_code()) }),
                2u64 => Some(Field { name: "start_glyph_id", value: FieldType::from(self.start_glyph_id()) }),
                _ => None,
            }),
            data,
        })
    }
}

impl Format<u16> for Cmap10<'_> {
    const FORMAT: u16 = 10;
}

impl<'a> MinByteRange<'a> for Cmap10<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(glyph_id_array_byte_range) = self.glyph_id_array_byte_range();

        Ok(0..glyph_id_array_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Cmap10<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_40) = data.len();

        match ::core::primitive::bool::then_some(value_40 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Cmap10<'a> {
    data: FontData<'a>,
}

impl<'a> Cmap10<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn length(&self) -> Result<u32, Never> {
        let Ok(range) = self.length_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn language(&self) -> Result<u32, Never> {
        let Ok(range) = self.language_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn start_char_code(&self) -> Result<u32, Never> {
        let Ok(range) = self.start_char_code_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_chars(&self) -> Result<u32, Never> {
        let Ok(range) = self.num_chars_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn glyph_id_array(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.glyph_id_array_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn reserved_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn length_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.reserved_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn language_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.length_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn start_char_code_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.language_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_chars_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.start_char_code_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_id_array_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_chars) = self.num_chars();
        let Ok(previous) = self.num_chars_byte_range();
        let start = previous.end;
        let Ok(value_41) = transforms::to_u64(num_chars);

        let end = start.saturating_add(value_41.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Cmap10<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Cmap10")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "length", value: FieldType::from(self.length()) }),
            2u64 => Some(Field { name: "language", value: FieldType::from(self.language()) }),
            3u64 => Some(Field { name: "start_char_code", value: FieldType::from(self.start_char_code()) }),
            4u64 => Some(Field { name: "num_chars", value: FieldType::from(self.num_chars()) }),
            5u64 => Some(Field { name: "glyph_id_array", value: FieldType::from(self.glyph_id_array()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Cmap10<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for Cmap12<'_> {
    const FORMAT: u16 = 12;
}

impl<'a> MinByteRange<'a> for Cmap12<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(groups_byte_range) = self.groups_byte_range();

        Ok(0..groups_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Cmap12<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_42) = data.len();

        match ::core::primitive::bool::then_some(value_42 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Cmap12<'a> {
    data: FontData<'a>,
}

impl<'a> Cmap12<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn length(&self) -> Result<u32, Never> {
        let Ok(range) = self.length_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn language(&self) -> Result<u32, Never> {
        let Ok(range) = self.language_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_groups(&self) -> Result<u32, Never> {
        let Ok(range) = self.num_groups_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn groups(&self) -> Result<&'a [SequentialMapGroup], Never> {
        let Ok(range) = self.groups_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn reserved_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn length_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.reserved_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn language_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.length_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_groups_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.language_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn groups_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_groups) = self.num_groups();
        let Ok(previous) = self.num_groups_byte_range();
        let start = previous.end;
        let Ok(value_43) = transforms::to_u64(num_groups);

        let end = start.saturating_add(value_43.saturating_mul(SequentialMapGroup::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Cmap12<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Cmap12")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "length", value: FieldType::from(self.length()) }),
            2u64 => Some(Field { name: "language", value: FieldType::from(self.language()) }),
            3u64 => Some(Field { name: "num_groups", value: FieldType::from(self.num_groups()) }),
            4u64 => {
                let Ok(value_44) = self.offset_data();

                Some(Field { name: "groups", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(SequentialMapGroup),
                    { let Ok(explicit_value) = self.groups(); explicit_value },
                    value_44,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Cmap12<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for Cmap13<'_> {
    const FORMAT: u16 = 13;
}

impl<'a> MinByteRange<'a> for Cmap13<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(groups_byte_range) = self.groups_byte_range();

        Ok(0..groups_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Cmap13<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_45) = data.len();

        match ::core::primitive::bool::then_some(value_45 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Cmap13<'a> {
    data: FontData<'a>,
}

impl<'a> Cmap13<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn length(&self) -> Result<u32, Never> {
        let Ok(range) = self.length_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn language(&self) -> Result<u32, Never> {
        let Ok(range) = self.language_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_groups(&self) -> Result<u32, Never> {
        let Ok(range) = self.num_groups_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn groups(&self) -> Result<&'a [ConstantMapGroup], Never> {
        let Ok(range) = self.groups_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn reserved_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn length_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.reserved_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn language_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.length_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_groups_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.language_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn groups_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_groups) = self.num_groups();
        let Ok(previous) = self.num_groups_byte_range();
        let start = previous.end;
        let Ok(value_46) = transforms::to_u64(num_groups);

        let end = start.saturating_add(value_46.saturating_mul(ConstantMapGroup::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Cmap13<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Cmap13")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "length", value: FieldType::from(self.length()) }),
            2u64 => Some(Field { name: "language", value: FieldType::from(self.language()) }),
            3u64 => Some(Field { name: "num_groups", value: FieldType::from(self.num_groups()) }),
            4u64 => {
                let Ok(value_47) = self.offset_data();

                Some(Field { name: "groups", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(ConstantMapGroup),
                    { let Ok(explicit_value) = self.groups(); explicit_value },
                    value_47,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Cmap13<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct ConstantMapGroup {
    pub start_char_code: BigEndian<u32>,
    pub end_char_code: BigEndian<u32>,
    pub glyph_id: BigEndian<u32>,
}

const _: () = {
  assert!(core::mem::size_of::<ConstantMapGroup>() == 12);
  assert!(core::mem::align_of::<ConstantMapGroup>() == 1);
  assert!(core::mem::offset_of!(ConstantMapGroup, start_char_code) == 0);
  assert!(core::mem::offset_of!(ConstantMapGroup, end_char_code) == 4);
  assert!(core::mem::offset_of!(ConstantMapGroup, glyph_id) == 8);
};

impl ConstantMapGroup {
    pub fn start_char_code(&self) -> Result<u32, Never> {
        self.start_char_code.get()
    }

    pub fn end_char_code(&self) -> Result<u32, Never> {
        self.end_char_code.get()
    }

    pub fn glyph_id(&self) -> Result<u32, Never> {
        self.glyph_id.get()
    }
}

impl FixedSize for ConstantMapGroup {
    const RAW_BYTE_LEN: u64 = u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for ConstantMapGroup {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "ConstantMapGroup",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "start_char_code", value: FieldType::from(self.start_char_code()) }),
                1u64 => Some(Field { name: "end_char_code", value: FieldType::from(self.end_char_code()) }),
                2u64 => Some(Field { name: "glyph_id", value: FieldType::from(self.glyph_id()) }),
                _ => None,
            }),
            data,
        })
    }
}

impl Format<u16> for Cmap14<'_> {
    const FORMAT: u16 = 14;
}

impl<'a> MinByteRange<'a> for Cmap14<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(var_selector_byte_range) = self.var_selector_byte_range();

        Ok(0..var_selector_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Cmap14<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_51) = data.len();

        match ::core::primitive::bool::then_some(value_51 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Cmap14<'a> {
    data: FontData<'a>,
}

impl<'a> Cmap14<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn length(&self) -> Result<u32, Never> {
        let Ok(range) = self.length_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_var_selector_records(&self) -> Result<u32, Never> {
        let Ok(range) = self.num_var_selector_records_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn var_selector(&self) -> Result<&'a [VariationSelector], Never> {
        let Ok(range) = self.var_selector_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn length_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_var_selector_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.length_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn var_selector_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_var_selector_records) = self.num_var_selector_records();
        let Ok(previous) = self.num_var_selector_records_byte_range();
        let start = previous.end;
        let Ok(value_52) = transforms::to_u64(num_var_selector_records);

        let end = start.saturating_add(value_52
                .saturating_mul(VariationSelector::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Cmap14<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Cmap14")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "length", value: FieldType::from(self.length()) }),
            2u64 => Some(Field { name: "num_var_selector_records", value: FieldType::from(self.num_var_selector_records()) }),
            3u64 => {
                let Ok(value_53) = self.offset_data();

                Some(Field { name: "var_selector", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(VariationSelector),
                    { let Ok(explicit_value) = self.var_selector(); explicit_value },
                    value_53,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Cmap14<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct VariationSelector {
    pub var_selector: BigEndian<Uint24>,
    pub default_uvs_offset: BigEndian<Nullable<Offset32>>,
    pub non_default_uvs_offset: BigEndian<Nullable<Offset32>>,
}

const _: () = {
  assert!(core::mem::size_of::<VariationSelector>() == 11);
  assert!(core::mem::align_of::<VariationSelector>() == 1);
  assert!(core::mem::offset_of!(VariationSelector, var_selector) == 0);
  assert!(core::mem::offset_of!(VariationSelector, default_uvs_offset) == 3);
  assert!(core::mem::offset_of!(VariationSelector, non_default_uvs_offset) == 7);
};

impl VariationSelector {
    pub fn var_selector(&self) -> Result<Uint24, Never> {
        self.var_selector.get()
    }

    pub fn default_uvs_offset(&self) -> Result<Nullable<Offset32>, Never> {
        self.default_uvs_offset.get()
    }

    pub fn default_uvs<'a>(&self, data: FontData<'a>) -> Result<Option<DefaultUvs<'a>>, ReadError> {
        let Ok(default_uvs_offset) = self.default_uvs_offset();

        default_uvs_offset.resolve(data)
    }

    pub fn non_default_uvs_offset(&self) -> Result<Nullable<Offset32>, Never> {
        self.non_default_uvs_offset.get()
    }

    pub fn non_default_uvs<'a>(
        &self,
        data: FontData<'a>,
    ) -> Result<Option<NonDefaultUvs<'a>>, ReadError> {
        let Ok(non_default_uvs_offset) = self.non_default_uvs_offset();

        non_default_uvs_offset.resolve(data)
    }
}

impl FixedSize for VariationSelector {
    const RAW_BYTE_LEN: u64 =
        Uint24::RAW_BYTE_LEN + Offset32::RAW_BYTE_LEN + Offset32::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for VariationSelector {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "VariationSelector",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "var_selector", value: FieldType::from(self.var_selector()) }),
                1u64 => Some(Field { name: "default_uvs_offset", value: FieldType::from(FieldType::offset(self.default_uvs_offset(), self.default_uvs(_data))) }),
                2u64 => Some(Field { name: "non_default_uvs_offset", value: FieldType::from(FieldType::offset(self.non_default_uvs_offset(), self.non_default_uvs(_data))) }),
                _ => None,
            }),
            data,
        })
    }
}

impl<'a> MinByteRange<'a> for DefaultUvs<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(ranges_byte_range) = self.ranges_byte_range();

        Ok(0..ranges_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for DefaultUvs<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_57) = data.len();

        match ::core::primitive::bool::then_some(value_57 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct DefaultUvs<'a> {
    data: FontData<'a>,
}

impl<'a> DefaultUvs<'a> {
    pub const MIN_SIZE: u64 = u32::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn num_unicode_value_ranges(&self) -> Result<u32, Never> {
        let Ok(range) = self.num_unicode_value_ranges_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn ranges(&self) -> Result<&'a [UnicodeRange], Never> {
        let Ok(range) = self.ranges_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn num_unicode_value_ranges_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn ranges_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_unicode_value_ranges) = self.num_unicode_value_ranges();
        let Ok(previous) = self.num_unicode_value_ranges_byte_range();
        let start = previous.end;
        let Ok(value_58) = transforms::to_u64(num_unicode_value_ranges);

        let end = start.saturating_add(value_58
                .saturating_mul(UnicodeRange::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(DefaultUvs::MIN_SIZE); held }));

impl Default for DefaultUvs<'_> {
    fn default() -> Self {
        let Ok(value_59) = FontData::default_table_data();

        Self {
            data: value_59,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for DefaultUvs<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("DefaultUvs")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "num_unicode_value_ranges", value: FieldType::from(self.num_unicode_value_ranges()) }),
            1u64 => {
                let Ok(value_60) = self.offset_data();

                Some(Field { name: "ranges", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(UnicodeRange),
                    { let Ok(explicit_value) = self.ranges(); explicit_value },
                    value_60,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for DefaultUvs<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for NonDefaultUvs<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(uvs_mapping_byte_range) = self.uvs_mapping_byte_range();

        Ok(0..uvs_mapping_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for NonDefaultUvs<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_61) = data.len();

        match ::core::primitive::bool::then_some(value_61 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct NonDefaultUvs<'a> {
    data: FontData<'a>,
}

impl<'a> NonDefaultUvs<'a> {
    pub const MIN_SIZE: u64 = u32::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn num_uvs_mappings(&self) -> Result<u32, Never> {
        let Ok(range) = self.num_uvs_mappings_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn uvs_mapping(&self) -> Result<&'a [UvsMapping], Never> {
        let Ok(range) = self.uvs_mapping_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn num_uvs_mappings_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn uvs_mapping_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_uvs_mappings) = self.num_uvs_mappings();
        let Ok(previous) = self.num_uvs_mappings_byte_range();
        let start = previous.end;
        let Ok(value_62) = transforms::to_u64(num_uvs_mappings);

        let end = start.saturating_add(value_62.saturating_mul(UvsMapping::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(NonDefaultUvs::MIN_SIZE); held }));

impl Default for NonDefaultUvs<'_> {
    fn default() -> Self {
        let Ok(value_63) = FontData::default_table_data();

        Self {
            data: value_63,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for NonDefaultUvs<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("NonDefaultUvs")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "num_uvs_mappings", value: FieldType::from(self.num_uvs_mappings()) }),
            1u64 => {
                let Ok(value_64) = self.offset_data();

                Some(Field { name: "uvs_mapping", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(UvsMapping),
                    { let Ok(explicit_value) = self.uvs_mapping(); explicit_value },
                    value_64,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for NonDefaultUvs<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct UvsMapping {
    pub unicode_value: BigEndian<Uint24>,
    pub glyph_id: BigEndian<u16>,
}

const _: () = {
  assert!(core::mem::size_of::<UvsMapping>() == 5);
  assert!(core::mem::align_of::<UvsMapping>() == 1);
  assert!(core::mem::offset_of!(UvsMapping, unicode_value) == 0);
  assert!(core::mem::offset_of!(UvsMapping, glyph_id) == 3);
};

impl UvsMapping {
    pub fn unicode_value(&self) -> Result<Uint24, Never> {
        self.unicode_value.get()
    }

    pub fn glyph_id(&self) -> Result<u16, Never> {
        self.glyph_id.get()
    }
}

impl FixedSize for UvsMapping {
    const RAW_BYTE_LEN: u64 = Uint24::RAW_BYTE_LEN + u16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for UvsMapping {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "UvsMapping",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "unicode_value", value: FieldType::from(self.unicode_value()) }),
                1u64 => Some(Field { name: "glyph_id", value: FieldType::from(self.glyph_id()) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct UnicodeRange {
    pub start_unicode_value: BigEndian<Uint24>,
    pub additional_count: u8,
}

const _: () = {
  assert!(core::mem::size_of::<UnicodeRange>() == 4);
  assert!(core::mem::align_of::<UnicodeRange>() == 1);
  assert!(core::mem::offset_of!(UnicodeRange, start_unicode_value) == 0);
  assert!(core::mem::offset_of!(UnicodeRange, additional_count) == 3);
};

impl UnicodeRange {
    pub fn start_unicode_value(&self) -> Result<Uint24, Never> {
        self.start_unicode_value.get()
    }

    pub fn additional_count(&self) -> Result<u8, Never> {
        Ok(self.additional_count)
    }
}

impl FixedSize for UnicodeRange {
    const RAW_BYTE_LEN: u64 = Uint24::RAW_BYTE_LEN + u8::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for UnicodeRange {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "UnicodeRange",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "start_unicode_value", value: FieldType::from(self.start_unicode_value()) }),
                1u64 => Some(Field { name: "additional_count", value: FieldType::from(self.additional_count()) }),
                _ => None,
            }),
            data,
        })
    }
}
