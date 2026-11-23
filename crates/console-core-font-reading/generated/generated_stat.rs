

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Stat<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(offset_to_axis_value_offsets_byte_range) = self.offset_to_axis_value_offsets_byte_range();

        Ok(0..offset_to_axis_value_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Stat<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"STAT"); value });
}

impl<'a> FontRead<'a> for Stat<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Stat<'a> {
    data: FontData<'a>,
}

impl<'a> Stat<'a> {
    pub const MIN_SIZE: u64 = (MajorMinor::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<MajorMinor, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn design_axis_size(&self) -> Result<u16, Never> {
        let Ok(range) = self.design_axis_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn design_axis_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.design_axis_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn design_axes_offset(&self) -> Result<Offset32, Never> {
        let Ok(range) = self.design_axes_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn design_axes(&self) -> Result<&'a [AxisRecord], ReadError> {
        let data = self.data;
        let Ok(args) = self.design_axis_count();
        let Ok(design_axes_offset) = self.design_axes_offset();

        design_axes_offset.resolve_with_args(data, &args)
    }

    pub fn axis_value_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.axis_value_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn offset_to_axis_value_offsets(&self) -> Result<Nullable<Offset32>, Never> {
        let Ok(range) = self.offset_to_axis_value_offsets_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn offset_to_axis_values(&self) -> Result<Option<AxisValueArray<'a>>, ReadError> {
        let data = self.data;
        let Ok(args) = self.axis_value_count();
        let Ok(offset_to_axis_value_offsets) = self.offset_to_axis_value_offsets();

        offset_to_axis_value_offsets
            .resolve_with_args(data, &args)
    }

    pub fn elided_fallback_name_id(&self) -> Result<Option<NameId>, Never> {
        let Ok(range) = self.elided_fallback_name_id_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(MajorMinor::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn design_axis_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn design_axis_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.design_axis_size_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn design_axes_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.design_axis_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn axis_value_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.design_axes_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn offset_to_axis_value_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.axis_value_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn elided_fallback_name_id_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.offset_to_axis_value_offsets_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility((1u16, 1u16));

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(NameId::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Stat::MIN_SIZE); held }));

impl Default for Stat<'_> {
    fn default() -> Self {
        let Ok(value_3) = FontData::default_table_data();

        Self {
            data: value_3,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Stat<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Stat")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "design_axis_size", value: FieldType::from(self.design_axis_size()) }),
            2u64 => Some(Field { name: "design_axis_count", value: FieldType::from(self.design_axis_count()) }),
            3u64 => {
                let Ok(value_4) = self.offset_data();

                Some(Field { name: "design_axes_offset", value: FieldType::from(traversal::FieldType::offset_to_array_of_records(
                    self.design_axes_offset(),
                    (self.design_axes()).map(Some),
                    stringify!(AxisRecord),
                    value_4,
                )) })
            },
            4u64 => Some(Field { name: "axis_value_count", value: FieldType::from(self.axis_value_count()) }),
            5u64 => Some(Field { name: "offset_to_axis_value_offsets", value: FieldType::from(FieldType::offset(
                    self.offset_to_axis_value_offsets(),
                    self.offset_to_axis_values(),
                )) }),
            6u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility((1u16, 1u16));
                let Ok(elided_fallback_name_id) = self.elided_fallback_name_id();

                match compatibility {
                    Compatibility::Compatible => elided_fallback_name_id.map(|elided_fallback_name_id_2| Field { name: "elided_fallback_name_id", value: FieldType::from(elided_fallback_name_id_2) }),
                    Compatibility::Incompatible => None,
                }
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Stat<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct AxisRecord {
    pub axis_tag: BigEndian<Tag>,
    pub axis_name_id: BigEndian<NameId>,
    pub axis_ordering: BigEndian<u16>,
}

const _: () = {
  assert!(core::mem::size_of::<AxisRecord>() == 8);
  assert!(core::mem::align_of::<AxisRecord>() == 1);
  assert!(core::mem::offset_of!(AxisRecord, axis_tag) == 0);
  assert!(core::mem::offset_of!(AxisRecord, axis_name_id) == 4);
  assert!(core::mem::offset_of!(AxisRecord, axis_ordering) == 6);
};

impl AxisRecord {
    pub fn axis_tag(&self) -> Result<Tag, Never> {
        self.axis_tag.get()
    }

    pub fn axis_name_id(&self) -> Result<NameId, Never> {
        self.axis_name_id.get()
    }

    pub fn axis_ordering(&self) -> Result<u16, Never> {
        self.axis_ordering.get()
    }
}

impl FixedSize for AxisRecord {
    const RAW_BYTE_LEN: u64 = Tag::RAW_BYTE_LEN + NameId::RAW_BYTE_LEN + u16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for AxisRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "AxisRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "axis_tag", value: FieldType::from(self.axis_tag()) }),
                1u64 => Some(Field { name: "axis_name_id", value: FieldType::from(self.axis_name_id()) }),
                2u64 => Some(Field { name: "axis_ordering", value: FieldType::from(self.axis_ordering()) }),
                _ => None,
            }),
            data,
        })
    }
}

impl<'a> MinByteRange<'a> for AxisValueArray<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(axis_value_offsets_byte_range) = self.axis_value_offsets_byte_range();

        Ok(0..axis_value_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl ReadArgs for AxisValueArray<'_> {
    type Arguments = u16;
}

impl<'a> FontReadWithArgs<'a> for AxisValueArray<'a> {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let axis_value_count = *args;

        Ok(Self {
            data,
            axis_value_count,
        })
    }
}

impl<'a> AxisValueArray<'a> {
    pub fn read(data: FontData<'a>, axis_value_count: u16) -> Result<Self, ReadError> {
        let args = axis_value_count;
        Self::read_with_args(data, &args)
    }
}

#[derive(Clone)]
pub struct AxisValueArray<'a> {
    data: FontData<'a>,
    axis_value_count: u16,
}

impl<'a> AxisValueArray<'a> {
    pub const MIN_SIZE: u64 = 0;
    basic_table_impls!(impl_the_methods);

    pub fn axis_value_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.axis_value_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn axis_values(&self) -> Result<ArrayOfOffsets<'a, AxisValue<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.axis_value_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub(crate) fn axis_value_count(&self) -> Result<u16, Never> {
        Ok(self.axis_value_count)
    }

    pub fn axis_value_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(axis_value_count) = self.axis_value_count();
        let start = 0u64;
        let Ok(value_9) = transforms::to_u64(axis_value_count);

        let end =
            start.saturating_add(value_9.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(AxisValueArray::MIN_SIZE); held }));

impl Default for AxisValueArray<'_> {
    fn default() -> Self {
        let Ok(value_10) = FontData::default_table_data();

        Self {
            data: value_10,
            axis_value_count: Default::default(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for AxisValueArray<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("AxisValueArray")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "axis_value_offsets", value: FieldType::from(self.axis_values()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for AxisValueArray<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone)]
pub enum AxisValue<'a> {
    Format1(AxisValueFormat1<'a>),
    Format2(AxisValueFormat2<'a>),
    Format3(AxisValueFormat3<'a>),
    Format4(AxisValueFormat4<'a>),
}

impl Default for AxisValue<'_> {
    fn default() -> Self {
        Self::Format1(Default::default())
    }
}

impl<'a> AxisValue<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Format1(item) => item.offset_data(),
            Self::Format2(item) => item.offset_data(),
            Self::Format3(item) => item.offset_data(),
            Self::Format4(item) => item.offset_data(),
        }
    }

    pub fn format(&self) -> Result<u16, Never> {
        match self {
            Self::Format1(item) => item.format(),
            Self::Format2(item) => item.format(),
            Self::Format3(item) => item.format(),
            Self::Format4(item) => item.format(),
        }
    }

    pub fn flags(&self) -> Result<AxisValueTableFlags, Never> {
        match self {
            Self::Format1(item) => item.flags(),
            Self::Format2(item) => item.flags(),
            Self::Format3(item) => item.flags(),
            Self::Format4(item) => item.flags(),
        }
    }

    pub fn value_name_id(&self) -> Result<NameId, Never> {
        match self {
            Self::Format1(item) => item.value_name_id(),
            Self::Format2(item) => item.value_name_id(),
            Self::Format3(item) => item.value_name_id(),
            Self::Format4(item) => item.value_name_id(),
        }
    }
}

impl<'a> FontRead<'a> for AxisValue<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u16 = data.read_at(0u64)?;

        match format {
            AxisValueFormat1::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format1(read))
            }
            AxisValueFormat2::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format2(read))
            }
            AxisValueFormat3::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format3(read))
            }
            AxisValueFormat4::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format4(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u16>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for AxisValue<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        match self {
            Self::Format1(item) => item.min_byte_range(),
            Self::Format2(item) => item.min_byte_range(),
            Self::Format3(item) => item.min_byte_range(),
            Self::Format4(item) => item.min_byte_range(),
        }
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        match self {
            Self::Format1(item) => item.min_table_bytes(),
            Self::Format2(item) => item.min_table_bytes(),
            Self::Format3(item) => item.min_table_bytes(),
            Self::Format4(item) => item.min_table_bytes(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> AxisValue<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Format1(table) => table,
            Self::Format2(table) => table,
            Self::Format3(table) => table,
            Self::Format4(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for AxisValue<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for AxisValue<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}

impl Format<u16> for AxisValueFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for AxisValueFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(value_byte_range) = self.value_byte_range();

        Ok(0..value_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for AxisValueFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_15) = data.len();

        match ::core::primitive::bool::then_some(value_15 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct AxisValueFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> AxisValueFormat1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + AxisValueTableFlags::RAW_BYTE_LEN
        + NameId::RAW_BYTE_LEN
        + Fixed::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn axis_index(&self) -> Result<u16, Never> {
        let Ok(range) = self.axis_index_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn flags(&self) -> Result<AxisValueTableFlags, Never> {
        let Ok(range) = self.flags_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn value_name_id(&self) -> Result<NameId, Never> {
        let Ok(range) = self.value_name_id_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn value(&self) -> Result<Fixed, Never> {
        let Ok(range) = self.value_byte_range();

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

    pub fn flags_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.axis_index_byte_range();
        let start = previous.end;
        let end = start.saturating_add(AxisValueTableFlags::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn value_name_id_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.flags_byte_range();
        let start = previous.end;
        let end = start.saturating_add(NameId::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn value_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.value_name_id_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Fixed::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    AxisValueFormat1::MIN_SIZE
); held }));

impl Default for AxisValueFormat1<'_> {
    fn default() -> Self {
        let Ok(value_16) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_16,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for AxisValueFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("AxisValueFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "axis_index", value: FieldType::from(self.axis_index()) }),
            2u64 => Some(Field { name: "flags", value: FieldType::from(self.flags()) }),
            3u64 => Some(Field { name: "value_name_id", value: FieldType::from(self.value_name_id()) }),
            4u64 => Some(Field { name: "value", value: FieldType::from(self.value()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for AxisValueFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for AxisValueFormat2<'_> {
    const FORMAT: u16 = 2;
}

impl<'a> MinByteRange<'a> for AxisValueFormat2<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(range_max_value_byte_range) = self.range_max_value_byte_range();

        Ok(0..range_max_value_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for AxisValueFormat2<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_17) = data.len();

        match ::core::primitive::bool::then_some(value_17 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct AxisValueFormat2<'a> {
    data: FontData<'a>,
}

impl<'a> AxisValueFormat2<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + AxisValueTableFlags::RAW_BYTE_LEN
        + NameId::RAW_BYTE_LEN
        + Fixed::RAW_BYTE_LEN
        + Fixed::RAW_BYTE_LEN
        + Fixed::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn axis_index(&self) -> Result<u16, Never> {
        let Ok(range) = self.axis_index_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn flags(&self) -> Result<AxisValueTableFlags, Never> {
        let Ok(range) = self.flags_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn value_name_id(&self) -> Result<NameId, Never> {
        let Ok(range) = self.value_name_id_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn nominal_value(&self) -> Result<Fixed, Never> {
        let Ok(range) = self.nominal_value_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn range_min_value(&self) -> Result<Fixed, Never> {
        let Ok(range) = self.range_min_value_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn range_max_value(&self) -> Result<Fixed, Never> {
        let Ok(range) = self.range_max_value_byte_range();

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

    pub fn flags_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.axis_index_byte_range();
        let start = previous.end;
        let end = start.saturating_add(AxisValueTableFlags::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn value_name_id_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.flags_byte_range();
        let start = previous.end;
        let end = start.saturating_add(NameId::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn nominal_value_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.value_name_id_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Fixed::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn range_min_value_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.nominal_value_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Fixed::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn range_max_value_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.range_min_value_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Fixed::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for AxisValueFormat2<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("AxisValueFormat2")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "axis_index", value: FieldType::from(self.axis_index()) }),
            2u64 => Some(Field { name: "flags", value: FieldType::from(self.flags()) }),
            3u64 => Some(Field { name: "value_name_id", value: FieldType::from(self.value_name_id()) }),
            4u64 => Some(Field { name: "nominal_value", value: FieldType::from(self.nominal_value()) }),
            5u64 => Some(Field { name: "range_min_value", value: FieldType::from(self.range_min_value()) }),
            6u64 => Some(Field { name: "range_max_value", value: FieldType::from(self.range_max_value()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for AxisValueFormat2<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for AxisValueFormat3<'_> {
    const FORMAT: u16 = 3;
}

impl<'a> MinByteRange<'a> for AxisValueFormat3<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(linked_value_byte_range) = self.linked_value_byte_range();

        Ok(0..linked_value_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for AxisValueFormat3<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_18) = data.len();

        match ::core::primitive::bool::then_some(value_18 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct AxisValueFormat3<'a> {
    data: FontData<'a>,
}

impl<'a> AxisValueFormat3<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + AxisValueTableFlags::RAW_BYTE_LEN
        + NameId::RAW_BYTE_LEN
        + Fixed::RAW_BYTE_LEN
        + Fixed::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn axis_index(&self) -> Result<u16, Never> {
        let Ok(range) = self.axis_index_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn flags(&self) -> Result<AxisValueTableFlags, Never> {
        let Ok(range) = self.flags_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn value_name_id(&self) -> Result<NameId, Never> {
        let Ok(range) = self.value_name_id_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn value(&self) -> Result<Fixed, Never> {
        let Ok(range) = self.value_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn linked_value(&self) -> Result<Fixed, Never> {
        let Ok(range) = self.linked_value_byte_range();

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

    pub fn flags_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.axis_index_byte_range();
        let start = previous.end;
        let end = start.saturating_add(AxisValueTableFlags::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn value_name_id_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.flags_byte_range();
        let start = previous.end;
        let end = start.saturating_add(NameId::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn value_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.value_name_id_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Fixed::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn linked_value_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.value_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Fixed::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for AxisValueFormat3<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("AxisValueFormat3")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "axis_index", value: FieldType::from(self.axis_index()) }),
            2u64 => Some(Field { name: "flags", value: FieldType::from(self.flags()) }),
            3u64 => Some(Field { name: "value_name_id", value: FieldType::from(self.value_name_id()) }),
            4u64 => Some(Field { name: "value", value: FieldType::from(self.value()) }),
            5u64 => Some(Field { name: "linked_value", value: FieldType::from(self.linked_value()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for AxisValueFormat3<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for AxisValueFormat4<'_> {
    const FORMAT: u16 = 4;
}

impl<'a> MinByteRange<'a> for AxisValueFormat4<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(axis_values_byte_range) = self.axis_values_byte_range();

        Ok(0..axis_values_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for AxisValueFormat4<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_19) = data.len();

        match ::core::primitive::bool::then_some(value_19 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct AxisValueFormat4<'a> {
    data: FontData<'a>,
}

impl<'a> AxisValueFormat4<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + AxisValueTableFlags::RAW_BYTE_LEN
        + NameId::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn axis_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.axis_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn flags(&self) -> Result<AxisValueTableFlags, Never> {
        let Ok(range) = self.flags_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn value_name_id(&self) -> Result<NameId, Never> {
        let Ok(range) = self.value_name_id_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn axis_values(&self) -> Result<&'a [AxisValueRecord], Never> {
        let Ok(range) = self.axis_values_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn axis_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn flags_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.axis_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(AxisValueTableFlags::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn value_name_id_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.flags_byte_range();
        let start = previous.end;
        let end = start.saturating_add(NameId::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn axis_values_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(axis_count) = self.axis_count();
        let Ok(previous) = self.value_name_id_byte_range();
        let start = previous.end;
        let Ok(value_20) = transforms::to_u64(axis_count);

        let end = start.saturating_add(value_20.saturating_mul(AxisValueRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for AxisValueFormat4<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("AxisValueFormat4")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "axis_count", value: FieldType::from(self.axis_count()) }),
            2u64 => Some(Field { name: "flags", value: FieldType::from(self.flags()) }),
            3u64 => Some(Field { name: "value_name_id", value: FieldType::from(self.value_name_id()) }),
            4u64 => {
                let Ok(value_21) = self.offset_data();

                Some(Field { name: "axis_values", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(AxisValueRecord),
                    { let Ok(explicit_value) = self.axis_values(); explicit_value },
                    value_21,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for AxisValueFormat4<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct AxisValueRecord {
    pub axis_index: BigEndian<u16>,
    pub value: BigEndian<Fixed>,
}

const _: () = {
  assert!(core::mem::size_of::<AxisValueRecord>() == 6);
  assert!(core::mem::align_of::<AxisValueRecord>() == 1);
  assert!(core::mem::offset_of!(AxisValueRecord, axis_index) == 0);
  assert!(core::mem::offset_of!(AxisValueRecord, value) == 2);
};

impl AxisValueRecord {
    pub fn axis_index(&self) -> Result<u16, Never> {
        self.axis_index.get()
    }

    pub fn value(&self) -> Result<Fixed, Never> {
        self.value.get()
    }
}

impl FixedSize for AxisValueRecord {
    const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN + Fixed::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for AxisValueRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "AxisValueRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "axis_index", value: FieldType::from(self.axis_index()) }),
                1u64 => Some(Field { name: "value", value: FieldType::from(self.value()) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck :: AnyBitPattern)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct AxisValueTableFlags {
    bits: u16,
}

impl AxisValueTableFlags {
    pub const OLDER_SIBLING_FONT_ATTRIBUTE: Self = Self { bits: 0x0001 };

    pub const ELIDABLE_AXIS_VALUE_NAME: Self = Self { bits: 0x0002 };
}

impl AxisValueTableFlags {
    const KNOWN: u16 = Self::OLDER_SIBLING_FONT_ATTRIBUTE.bits
        | Self::ELIDABLE_AXIS_VALUE_NAME.bits;

    #[inline]
    pub const fn empty() -> Result<Self, Never> {
        Ok(Self { bits: 0 })
    }

    #[inline]
    pub const fn all() -> Result<Self, Never> {
        Ok(Self { bits: Self::KNOWN })
    }

    #[inline]
    pub const fn bits(&self) -> Result<u16, Never> {
        Ok(self.bits)
    }

    #[inline]
    pub const fn from_bits(bits: u16) -> Result<Option<Self>, Never> {
        Ok(match bits & !Self::KNOWN {
            0 => Some(Self { bits }),
            _unknown => None,
        })
    }

    #[inline]
    pub const fn from_bits_truncate(bits: u16) -> Result<Self, Never> {
        Ok(Self {
            bits: bits & Self::KNOWN,
        })
    }

    #[inline]
    pub const fn is_empty(&self) -> Result<Emptiness, Never> {
        Ok(match self.bits {
            0 => Emptiness::Empty,
            _set => Emptiness::NonEmpty,
        })
    }

    #[inline]
    pub const fn intersects(&self, other: Self) -> Result<Overlap, Never> {
        Ok(match self.bits & other.bits {
            0 => Overlap::Disjoint,
            _shared => Overlap::Intersecting,
        })
    }

    #[inline]
    pub const fn contains(&self, other: Self) -> Result<Presence, Never> {
        Ok(match other.bits & !self.bits {
            0 => Presence::Present,
            _missing => Presence::Absent,
        })
    }

    #[inline]
    pub fn insert(&mut self, other: Self) -> Result<(), Never> {
        self.bits |= other.bits;

        Ok(())
    }

    #[inline]
    pub fn remove(&mut self, other: Self) -> Result<(), Never> {
        self.bits &= !other.bits;

        Ok(())
    }

    #[inline]
    pub fn toggle(&mut self, other: Self) -> Result<(), Never> {
        self.bits ^= other.bits;

        Ok(())
    }

    #[inline]
    pub const fn intersection(self, other: Self) -> Result<Self, Never> {
        Ok(Self {
            bits: self.bits & other.bits,
        })
    }

    #[inline]
    pub const fn union(self, other: Self) -> Result<Self, Never> {
        Ok(Self {
            bits: self.bits | other.bits,
        })
    }

    #[inline]
    pub const fn difference(self, other: Self) -> Result<Self, Never> {
        Ok(Self {
            bits: self.bits & !other.bits,
        })
    }
}

impl std::ops::BitOr for AxisValueTableFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, other: AxisValueTableFlags) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl std::ops::BitOrAssign for AxisValueTableFlags {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.bits |= other.bits;
    }
}

impl std::ops::BitXor for AxisValueTableFlags {
    type Output = Self;

    #[inline]
    fn bitxor(self, other: Self) -> Self {
        Self {
            bits: self.bits ^ other.bits,
        }
    }
}

impl std::ops::BitXorAssign for AxisValueTableFlags {
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        self.bits ^= other.bits;
    }
}

impl std::ops::BitAnd for AxisValueTableFlags {
    type Output = Self;

    #[inline]
    fn bitand(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }
}

impl std::ops::BitAndAssign for AxisValueTableFlags {
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        self.bits &= other.bits;
    }
}

impl std::ops::Sub for AxisValueTableFlags {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }
}

impl std::ops::SubAssign for AxisValueTableFlags {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.bits &= !other.bits;
    }
}

impl std::ops::Not for AxisValueTableFlags {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        Self { bits: !self.bits & Self::KNOWN }
    }
}

impl std::fmt::Debug for AxisValueTableFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let members: &[(&str, Self)] = &[
            (
                "OLDER_SIBLING_FONT_ATTRIBUTE",
                Self::OLDER_SIBLING_FONT_ATTRIBUTE,
            ),
            ("ELIDABLE_AXIS_VALUE_NAME", Self::ELIDABLE_AXIS_VALUE_NAME),
        ];
        let mut separator = "";

        for (name, value) in members {
            let Ok(presence) = self.contains(*value);

            match presence {
                Presence::Present => {
                    f.write_str(separator)?;
                    f.write_str(name)?;
                    separator = " | ";
                }
                Presence::Absent => {}
            }
        }

        match ::core::primitive::bool::then_some(separator.is_empty(), ()) {
            ::core::option::Option::Some(()) => f.write_str("(empty)"),
            ::core::option::Option::None => Ok(()),
        }
    }
}

impl std::fmt::Binary for AxisValueTableFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Binary::fmt(&self.bits, f)
    }
}

impl std::fmt::Octal for AxisValueTableFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Octal::fmt(&self.bits, f)
    }
}

impl std::fmt::LowerHex for AxisValueTableFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.bits, f)
    }
}

impl std::fmt::UpperHex for AxisValueTableFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.bits, f)
    }
}

impl font_types::Scalar for AxisValueTableFlags {
    type Raw = <u16 as font_types::Scalar>::Raw;

    fn to_raw(self) -> std::result::Result<Self::Raw, console_core_never::Never> {
        self.bits.to_raw()
    }

    fn from_raw(raw: Self::Raw) -> std::result::Result<Self, console_core_never::Never> {
        let Ok(bits) = <u16>::from_raw(raw);

        Self::from_bits_truncate(bits)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> From<AxisValueTableFlags> for FieldType<'a> {
    fn from(src: AxisValueTableFlags) -> FieldType<'a> {
        <crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u16>>::from(src.bits)
    }
}
