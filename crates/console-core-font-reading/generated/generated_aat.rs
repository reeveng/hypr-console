

use crate::codegen_prelude::*;

#[derive(Clone)]
pub enum Lookup<'a> {
    Format0(Lookup0<'a>),
    Format2(Lookup2<'a>),
    Format4(Lookup4<'a>),
    Format6(Lookup6<'a>),
    Format8(Lookup8<'a>),
    Format10(Lookup10<'a>),
}

impl Default for Lookup<'_> {
    fn default() -> Self {
        Self::Format0(Default::default())
    }
}

impl<'a> Lookup<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Format0(item) => item.offset_data(),
            Self::Format2(item) => item.offset_data(),
            Self::Format4(item) => item.offset_data(),
            Self::Format6(item) => item.offset_data(),
            Self::Format8(item) => item.offset_data(),
            Self::Format10(item) => item.offset_data(),
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
        }
    }
}

impl<'a> FontRead<'a> for Lookup<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u16 = data.read_at(0u64)?;

        match format {
            Lookup0::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format0(read))
            }
            Lookup2::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format2(read))
            }
            Lookup4::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format4(read))
            }
            Lookup6::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format6(read))
            }
            Lookup8::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format8(read))
            }
            Lookup10::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format10(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u16>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for Lookup<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        match self {
            Self::Format0(item) => item.min_byte_range(),
            Self::Format2(item) => item.min_byte_range(),
            Self::Format4(item) => item.min_byte_range(),
            Self::Format6(item) => item.min_byte_range(),
            Self::Format8(item) => item.min_byte_range(),
            Self::Format10(item) => item.min_byte_range(),
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
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> Lookup<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Format0(table) => table,
            Self::Format2(table) => table,
            Self::Format4(table) => table,
            Self::Format6(table) => table,
            Self::Format8(table) => table,
            Self::Format10(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for Lookup<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Lookup<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}

impl Format<u16> for Lookup0<'_> {
    const FORMAT: u16 = 0;
}

impl<'a> MinByteRange<'a> for Lookup0<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(values_data_byte_range) = self.values_data_byte_range();

        Ok(0..values_data_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Lookup0<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_7) = data.len();

        match ::core::primitive::bool::then_some(value_7 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Lookup0<'a> {
    data: FontData<'a>,
}

impl<'a> Lookup0<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn values_data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.values_data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn values_data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let Ok(value_45) = self.data.len();

        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_45.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Lookup0::MIN_SIZE); held }));

impl Default for Lookup0<'_> {
    fn default() -> Self {
        let Ok(value_8) = FontData::default_table_data();

        Self {
            data: value_8,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Lookup0<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Lookup0")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "values_data", value: FieldType::from(self.values_data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Lookup0<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for Lookup2<'_> {
    const FORMAT: u16 = 2;
}

impl<'a> MinByteRange<'a> for Lookup2<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(segments_data_byte_range) = self.segments_data_byte_range();

        Ok(0..segments_data_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Lookup2<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_9) = data.len();

        match ::core::primitive::bool::then_some(value_9 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Lookup2<'a> {
    data: FontData<'a>,
}

impl<'a> Lookup2<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
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

    pub fn unit_size(&self) -> Result<u16, Never> {
        let Ok(range) = self.unit_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn n_units(&self) -> Result<u16, Never> {
        let Ok(range) = self.n_units_byte_range();

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

    pub fn segments_data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.segments_data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn unit_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn n_units_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.unit_size_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn search_range_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.n_units_byte_range();
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

    pub fn segments_data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(unit_size) = self.unit_size();
        let Ok(n_units) = self.n_units();
        let Ok(previous) = self.range_shift_byte_range();
        let start = previous.end;
        let Ok(value_10) = transforms::add_multiply(unit_size, 0_u64, n_units);

        let end = start.saturating_add(value_10
                .saturating_mul(u8::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Lookup2<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Lookup2")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "unit_size", value: FieldType::from(self.unit_size()) }),
            2u64 => Some(Field { name: "n_units", value: FieldType::from(self.n_units()) }),
            3u64 => Some(Field { name: "search_range", value: FieldType::from(self.search_range()) }),
            4u64 => Some(Field { name: "entry_selector", value: FieldType::from(self.entry_selector()) }),
            5u64 => Some(Field { name: "range_shift", value: FieldType::from(self.range_shift()) }),
            6u64 => Some(Field { name: "segments_data", value: FieldType::from(self.segments_data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Lookup2<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for Lookup4<'_> {
    const FORMAT: u16 = 4;
}

impl<'a> MinByteRange<'a> for Lookup4<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(segments_byte_range) = self.segments_byte_range();

        Ok(0..segments_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Lookup4<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_11) = data.len();

        match ::core::primitive::bool::then_some(value_11 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Lookup4<'a> {
    data: FontData<'a>,
}

impl<'a> Lookup4<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
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

    pub fn unit_size(&self) -> Result<u16, Never> {
        let Ok(range) = self.unit_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn n_units(&self) -> Result<u16, Never> {
        let Ok(range) = self.n_units_byte_range();

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

    pub fn segments(&self) -> Result<&'a [LookupSegment4], Never> {
        let Ok(range) = self.segments_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn unit_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn n_units_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.unit_size_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn search_range_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.n_units_byte_range();
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

    pub fn segments_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(n_units) = self.n_units();
        let Ok(previous) = self.range_shift_byte_range();
        let start = previous.end;
        let Ok(value_12) = transforms::to_u64(n_units);

        let end =
            start.saturating_add(value_12.saturating_mul(LookupSegment4::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Lookup4<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Lookup4")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "unit_size", value: FieldType::from(self.unit_size()) }),
            2u64 => Some(Field { name: "n_units", value: FieldType::from(self.n_units()) }),
            3u64 => Some(Field { name: "search_range", value: FieldType::from(self.search_range()) }),
            4u64 => Some(Field { name: "entry_selector", value: FieldType::from(self.entry_selector()) }),
            5u64 => Some(Field { name: "range_shift", value: FieldType::from(self.range_shift()) }),
            6u64 => {
                let Ok(value_13) = self.offset_data();

                Some(Field { name: "segments", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(LookupSegment4),
                    { let Ok(explicit_value) = self.segments(); explicit_value },
                    value_13,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Lookup4<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct LookupSegment4 {
    pub last_glyph: BigEndian<u16>,
    pub first_glyph: BigEndian<u16>,
    pub value_offset: BigEndian<u16>,
}

const _: () = {
  assert!(core::mem::size_of::<LookupSegment4>() == 6);
  assert!(core::mem::align_of::<LookupSegment4>() == 1);
  assert!(core::mem::offset_of!(LookupSegment4, last_glyph) == 0);
  assert!(core::mem::offset_of!(LookupSegment4, first_glyph) == 2);
  assert!(core::mem::offset_of!(LookupSegment4, value_offset) == 4);
};

impl LookupSegment4 {
    pub fn last_glyph(&self) -> Result<u16, Never> {
        self.last_glyph.get()
    }

    pub fn first_glyph(&self) -> Result<u16, Never> {
        self.first_glyph.get()
    }

    pub fn value_offset(&self) -> Result<u16, Never> {
        self.value_offset.get()
    }
}

impl FixedSize for LookupSegment4 {
    const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for LookupSegment4 {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "LookupSegment4",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "last_glyph", value: FieldType::from(self.last_glyph()) }),
                1u64 => Some(Field { name: "first_glyph", value: FieldType::from(self.first_glyph()) }),
                2u64 => Some(Field { name: "value_offset", value: FieldType::from(self.value_offset()) }),
                _ => None,
            }),
            data,
        })
    }
}

impl Format<u16> for Lookup6<'_> {
    const FORMAT: u16 = 6;
}

impl<'a> MinByteRange<'a> for Lookup6<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(entries_data_byte_range) = self.entries_data_byte_range();

        Ok(0..entries_data_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Lookup6<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_17) = data.len();

        match ::core::primitive::bool::then_some(value_17 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Lookup6<'a> {
    data: FontData<'a>,
}

impl<'a> Lookup6<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
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

    pub fn unit_size(&self) -> Result<u16, Never> {
        let Ok(range) = self.unit_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn n_units(&self) -> Result<u16, Never> {
        let Ok(range) = self.n_units_byte_range();

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

    pub fn entries_data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.entries_data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn unit_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn n_units_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.unit_size_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn search_range_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.n_units_byte_range();
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

    pub fn entries_data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(unit_size) = self.unit_size();
        let Ok(n_units) = self.n_units();
        let Ok(previous) = self.range_shift_byte_range();
        let start = previous.end;
        let Ok(value_18) = transforms::add_multiply(unit_size, 0_u64, n_units);

        let end = start.saturating_add(value_18
                .saturating_mul(u8::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Lookup6<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Lookup6")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "unit_size", value: FieldType::from(self.unit_size()) }),
            2u64 => Some(Field { name: "n_units", value: FieldType::from(self.n_units()) }),
            3u64 => Some(Field { name: "search_range", value: FieldType::from(self.search_range()) }),
            4u64 => Some(Field { name: "entry_selector", value: FieldType::from(self.entry_selector()) }),
            5u64 => Some(Field { name: "range_shift", value: FieldType::from(self.range_shift()) }),
            6u64 => Some(Field { name: "entries_data", value: FieldType::from(self.entries_data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Lookup6<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for Lookup8<'_> {
    const FORMAT: u16 = 8;
}

impl<'a> MinByteRange<'a> for Lookup8<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(value_array_byte_range) = self.value_array_byte_range();

        Ok(0..value_array_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Lookup8<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_19) = data.len();

        match ::core::primitive::bool::then_some(value_19 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Lookup8<'a> {
    data: FontData<'a>,
}

impl<'a> Lookup8<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn first_glyph(&self) -> Result<u16, Never> {
        let Ok(range) = self.first_glyph_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.glyph_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn value_array(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.value_array_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn first_glyph_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.first_glyph_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn value_array_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(glyph_count) = self.glyph_count();
        let Ok(previous) = self.glyph_count_byte_range();
        let start = previous.end;
        let Ok(value_20) = transforms::to_u64(glyph_count);

        let end = start.saturating_add(value_20.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Lookup8<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Lookup8")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "first_glyph", value: FieldType::from(self.first_glyph()) }),
            2u64 => Some(Field { name: "glyph_count", value: FieldType::from(self.glyph_count()) }),
            3u64 => Some(Field { name: "value_array", value: FieldType::from(self.value_array()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Lookup8<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for Lookup10<'_> {
    const FORMAT: u16 = 10;
}

impl<'a> MinByteRange<'a> for Lookup10<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(values_data_byte_range) = self.values_data_byte_range();

        Ok(0..values_data_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Lookup10<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_21) = data.len();

        match ::core::primitive::bool::then_some(value_21 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Lookup10<'a> {
    data: FontData<'a>,
}

impl<'a> Lookup10<'a> {
    pub const MIN_SIZE: u64 =
        (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn unit_size(&self) -> Result<u16, Never> {
        let Ok(range) = self.unit_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn first_glyph(&self) -> Result<u16, Never> {
        let Ok(range) = self.first_glyph_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.glyph_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn values_data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.values_data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn unit_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn first_glyph_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.unit_size_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.first_glyph_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn values_data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(glyph_count) = self.glyph_count();
        let Ok(unit_size) = self.unit_size();
        let Ok(previous) = self.glyph_count_byte_range();
        let start = previous.end;
        let Ok(value_22) = transforms::add_multiply(glyph_count, 0_u64, unit_size);

        let end = start.saturating_add(value_22
                .saturating_mul(u8::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Lookup10<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Lookup10")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "unit_size", value: FieldType::from(self.unit_size()) }),
            2u64 => Some(Field { name: "first_glyph", value: FieldType::from(self.first_glyph()) }),
            3u64 => Some(Field { name: "glyph_count", value: FieldType::from(self.glyph_count()) }),
            4u64 => Some(Field { name: "values_data", value: FieldType::from(self.values_data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Lookup10<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for StateHeader<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(entry_table_offset_byte_range) = self.entry_table_offset_byte_range();

        Ok(0..entry_table_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for StateHeader<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_23) = data.len();

        match ::core::primitive::bool::then_some(value_23 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct StateHeader<'a> {
    data: FontData<'a>,
}

impl<'a> StateHeader<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn state_size(&self) -> Result<u16, Never> {
        let Ok(range) = self.state_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn class_table_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.class_table_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn class_table(&self) -> Result<ClassSubtable<'a>, ReadError> {
        let data = self.data;
        let Ok(class_table_offset) = self.class_table_offset();

        class_table_offset.resolve(data)
    }

    pub fn state_array_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.state_array_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn state_array(&self) -> Result<RawBytes<'a>, ReadError> {
        let data = self.data;
        let Ok(state_array_offset) = self.state_array_offset();

        state_array_offset.resolve(data)
    }

    pub fn entry_table_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.entry_table_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn entry_table(&self) -> Result<RawBytes<'a>, ReadError> {
        let data = self.data;
        let Ok(entry_table_offset) = self.entry_table_offset();

        entry_table_offset.resolve(data)
    }

    pub fn state_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn class_table_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.state_size_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn state_array_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.class_table_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn entry_table_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.state_array_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(StateHeader::MIN_SIZE); held }));

impl Default for StateHeader<'_> {
    fn default() -> Self {
        let Ok(value_24) = FontData::default_table_data();

        Self {
            data: value_24,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for StateHeader<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("StateHeader")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "state_size", value: FieldType::from(self.state_size()) }),
            1u64 => Some(Field { name: "class_table_offset", value: FieldType::from(FieldType::offset(self.class_table_offset(), (self.class_table()).map(Some))) }),
            2u64 => Some(Field { name: "state_array_offset", value: FieldType::from(FieldType::offset(self.state_array_offset(), (self.state_array()).map(Some))) }),
            3u64 => Some(Field { name: "entry_table_offset", value: FieldType::from(FieldType::offset(self.entry_table_offset(), (self.entry_table()).map(Some))) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for StateHeader<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for ClassSubtable<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(class_array_byte_range) = self.class_array_byte_range();

        Ok(0..class_array_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ClassSubtable<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_25) = data.len();

        match ::core::primitive::bool::then_some(value_25 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ClassSubtable<'a> {
    data: FontData<'a>,
}

impl<'a> ClassSubtable<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn first_glyph(&self) -> Result<u16, Never> {
        let Ok(range) = self.first_glyph_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn n_glyphs(&self) -> Result<u16, Never> {
        let Ok(range) = self.n_glyphs_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn class_array(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.class_array_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn first_glyph_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn n_glyphs_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.first_glyph_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn class_array_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(n_glyphs) = self.n_glyphs();
        let Ok(previous) = self.n_glyphs_byte_range();
        let start = previous.end;
        let Ok(value_26) = transforms::to_u64(n_glyphs);

        let end = start.saturating_add(value_26.saturating_mul(u8::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(ClassSubtable::MIN_SIZE); held }));

impl Default for ClassSubtable<'_> {
    fn default() -> Self {
        let Ok(value_27) = FontData::default_table_data();

        Self {
            data: value_27,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ClassSubtable<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ClassSubtable")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "first_glyph", value: FieldType::from(self.first_glyph()) }),
            1u64 => Some(Field { name: "n_glyphs", value: FieldType::from(self.n_glyphs()) }),
            2u64 => Some(Field { name: "class_array", value: FieldType::from(self.class_array()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ClassSubtable<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for RawBytes<'a> {
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

impl<'a> FontRead<'a> for RawBytes<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct RawBytes<'a> {
    data: FontData<'a>,
}

impl<'a> RawBytes<'a> {
    pub const MIN_SIZE: u64 = 0;
    basic_table_impls!(impl_the_methods);

    pub fn data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn data_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let Ok(value_46) = self.data.len();

        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_46.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(RawBytes::MIN_SIZE); held }));

impl Default for RawBytes<'_> {
    fn default() -> Self {
        let Ok(value_29) = FontData::default_table_data();

        Self {
            data: value_29,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for RawBytes<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("RawBytes")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "data", value: FieldType::from(self.data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for RawBytes<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for StxHeader<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(entry_table_offset_byte_range) = self.entry_table_offset_byte_range();

        Ok(0..entry_table_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for StxHeader<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_30) = data.len();

        match ::core::primitive::bool::then_some(value_30 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct StxHeader<'a> {
    data: FontData<'a>,
}

impl<'a> StxHeader<'a> {
    pub const MIN_SIZE: u64 = (u32::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn n_classes(&self) -> Result<u32, Never> {
        let Ok(range) = self.n_classes_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn class_table_offset(&self) -> Result<Offset32, Never> {
        let Ok(range) = self.class_table_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn class_table(&self) -> Result<LookupU16<'a>, ReadError> {
        let data = self.data;
        let Ok(class_table_offset) = self.class_table_offset();

        class_table_offset.resolve(data)
    }

    pub fn state_array_offset(&self) -> Result<Offset32, Never> {
        let Ok(range) = self.state_array_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn state_array(&self) -> Result<RawWords<'a>, ReadError> {
        let data = self.data;
        let Ok(state_array_offset) = self.state_array_offset();

        state_array_offset.resolve(data)
    }

    pub fn entry_table_offset(&self) -> Result<Offset32, Never> {
        let Ok(range) = self.entry_table_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn entry_table(&self) -> Result<RawBytes<'a>, ReadError> {
        let data = self.data;
        let Ok(entry_table_offset) = self.entry_table_offset();

        entry_table_offset.resolve(data)
    }

    pub fn n_classes_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn class_table_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.n_classes_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn state_array_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.class_table_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn entry_table_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.state_array_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(StxHeader::MIN_SIZE); held }));

impl Default for StxHeader<'_> {
    fn default() -> Self {
        let Ok(value_31) = FontData::default_table_data();

        Self {
            data: value_31,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for StxHeader<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("StxHeader")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "n_classes", value: FieldType::from(self.n_classes()) }),
            1u64 => Some(Field { name: "class_table_offset", value: FieldType::from(FieldType::offset(self.class_table_offset(), (self.class_table()).map(Some))) }),
            2u64 => Some(Field { name: "state_array_offset", value: FieldType::from(FieldType::offset(self.state_array_offset(), (self.state_array()).map(Some))) }),
            3u64 => Some(Field { name: "entry_table_offset", value: FieldType::from(FieldType::offset(self.entry_table_offset(), (self.entry_table()).map(Some))) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for StxHeader<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for RawWords<'a> {
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

impl<'a> FontRead<'a> for RawWords<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct RawWords<'a> {
    data: FontData<'a>,
}

impl<'a> RawWords<'a> {
    pub const MIN_SIZE: u64 = 0;
    basic_table_impls!(impl_the_methods);

    pub fn data(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn data_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let Ok(value_47) = self.data.len();

        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_47.saturating_sub(start), item_width: u16::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(RawWords::MIN_SIZE); held }));

impl Default for RawWords<'_> {
    fn default() -> Self {
        let Ok(value_33) = FontData::default_table_data();

        Self {
            data: value_33,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for RawWords<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("RawWords")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "data", value: FieldType::from(self.data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for RawWords<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
