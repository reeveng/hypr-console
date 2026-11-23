

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for TupleVariationHeader<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(intermediate_end_tuple_byte_range) = self.intermediate_end_tuple_byte_range();

        Ok(0..intermediate_end_tuple_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl ReadArgs for TupleVariationHeader<'_> {
    type Arguments = u16;
}

impl<'a> FontReadWithArgs<'a> for TupleVariationHeader<'a> {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let axis_count = *args;

        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data, axis_count })
    }
}

impl<'a> TupleVariationHeader<'a> {
    pub fn read(data: FontData<'a>, axis_count: u16) -> Result<Self, ReadError> {
        let args = axis_count;
        Self::read_with_args(data, &args)
    }
}

#[derive(Clone)]
pub struct TupleVariationHeader<'a> {
    data: FontData<'a>,
    axis_count: u16,
}

impl<'a> TupleVariationHeader<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + TupleIndex::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn variation_data_size(&self) -> Result<u16, Never> {
        let Ok(range) = self.variation_data_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn tuple_index(&self) -> Result<TupleIndex, Never> {
        let Ok(range) = self.tuple_index_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub(crate) fn axis_count(&self) -> Result<u16, Never> {
        Ok(self.axis_count)
    }

    pub fn variation_data_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn tuple_index_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.variation_data_size_byte_range();
        let start = previous.end;
        let end = start.saturating_add(TupleIndex::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn peak_tuple_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(tuple_index) = self.tuple_index();
        let Ok(axis_count) = self.axis_count();
        let Ok(previous) = self.tuple_index_byte_range();
        let start = previous.end;
        let Ok(value_10) = console_core_number_conversion::fitted::<_, u64>(0_u64);

        let Ok(tuple_len) = TupleIndex::tuple_len(tuple_index, axis_count, value_10);
        let end = start.saturating_add(tuple_len
                .saturating_mul(F2Dot14::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn intermediate_start_tuple_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(tuple_index) = self.tuple_index();
        let Ok(axis_count) = self.axis_count();
        let Ok(previous) = self.peak_tuple_byte_range();
        let start = previous.end;
        let Ok(value_11) = console_core_number_conversion::fitted::<_, u64>(1_u64);

        let Ok(tuple_len) = TupleIndex::tuple_len(tuple_index, axis_count, value_11);
        let end = start.saturating_add(tuple_len
                .saturating_mul(F2Dot14::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn intermediate_end_tuple_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(tuple_index) = self.tuple_index();
        let Ok(axis_count) = self.axis_count();
        let Ok(previous) = self.intermediate_start_tuple_byte_range();
        let start = previous.end;
        let Ok(value_33) = console_core_number_conversion::fitted::<_, u64>(1_u64);

        let Ok(tuple_len) = TupleIndex::tuple_len(tuple_index, axis_count, value_33);
        let end = start.saturating_add(tuple_len
                .saturating_mul(F2Dot14::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    TupleVariationHeader::MIN_SIZE
); held }));

impl Default for TupleVariationHeader<'_> {
    fn default() -> Self {
        let Ok(value_2) = FontData::default_table_data();

        Self {
            data: value_2,
            axis_count: Default::default(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for TupleVariationHeader<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("TupleVariationHeader")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "variation_data_size", value: FieldType::from(self.variation_data_size()) }),
            1u64 => {
                let Ok(value) = self.traverse_tuple_index();

                Some(Field { name: "tuple_index", value })
            }
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for TupleVariationHeader<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(Default)]
pub struct Tuple<'a> {
    pub values: &'a [BigEndian<F2Dot14>],
}

impl<'a> Tuple<'a> {
    pub fn values(&self) -> Result<&'a [BigEndian<F2Dot14>], Never> {
        Ok(self.values)
    }
}

impl ReadArgs for Tuple<'_> {
    type Arguments = u16;
}

impl ComputeSize for Tuple<'_> {

    fn compute_size(args: &u16) -> Result<u64, ReadError> {
        let axis_count = *args;
        let Ok(value_3) = transforms::to_u64(axis_count);

        Ok(value_3.saturating_mul(F2Dot14::RAW_BYTE_LEN))
    }
}

impl<'a> FontReadWithArgs<'a> for Tuple<'a> {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let Ok(value_4) = data.cursor();

        let mut cursor = value_4;
        let axis_count = *args;
        let Ok(value_5) = transforms::to_u64(axis_count);

        let values = cursor.read_array(value_5)?;


        Ok(Self {

            values,

        })
    }
}

impl<'a> Tuple<'a> {
    pub fn read(data: FontData<'a>, axis_count: u16) -> Result<Self, ReadError> {
        let args = axis_count;
        Self::read_with_args(data, &args)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for Tuple<'a> {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "Tuple",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "values", value: FieldType::from(self.values()) }),
                _ => None,
            }),
            data,
        })
    }
}

impl Format<u8> for DeltaSetIndexMapFormat0<'_> {
    const FORMAT: u8 = 0;
}

impl<'a> MinByteRange<'a> for DeltaSetIndexMapFormat0<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(map_data_byte_range) = self.map_data_byte_range();

        Ok(0..map_data_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for DeltaSetIndexMapFormat0<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_6) = data.len();

        match ::core::primitive::bool::then_some(value_6 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct DeltaSetIndexMapFormat0<'a> {
    data: FontData<'a>,
}

impl<'a> DeltaSetIndexMapFormat0<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN + EntryFormat::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn entry_format(&self) -> Result<EntryFormat, Never> {
        let Ok(range) = self.entry_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn map_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.map_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn map_data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.map_data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn entry_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(EntryFormat::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn map_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.entry_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn map_data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(entry_format) = self.entry_format();
        let Ok(map_count) = self.map_count();
        let Ok(previous) = self.map_count_byte_range();
        let start = previous.end;
        let Ok(map_size) = EntryFormat::map_size(entry_format, map_count);
        let end = start.saturating_add(map_size.saturating_mul(u8::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    DeltaSetIndexMapFormat0::MIN_SIZE
); held }));

impl Default for DeltaSetIndexMapFormat0<'_> {
    fn default() -> Self {
        let Ok(value_7) = FontData::default_table_data();

        Self {
            data: value_7,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for DeltaSetIndexMapFormat0<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("DeltaSetIndexMapFormat0")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "entry_format", value: FieldType::from(self.entry_format()) }),
            2u64 => Some(Field { name: "map_count", value: FieldType::from(self.map_count()) }),
            3u64 => Some(Field { name: "map_data", value: FieldType::from(self.map_data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for DeltaSetIndexMapFormat0<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for DeltaSetIndexMapFormat1<'_> {
    const FORMAT: u8 = 1;
}

impl<'a> MinByteRange<'a> for DeltaSetIndexMapFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(map_data_byte_range) = self.map_data_byte_range();

        Ok(0..map_data_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for DeltaSetIndexMapFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_8) = data.len();

        match ::core::primitive::bool::then_some(value_8 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct DeltaSetIndexMapFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> DeltaSetIndexMapFormat1<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN + EntryFormat::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn entry_format(&self) -> Result<EntryFormat, Never> {
        let Ok(range) = self.entry_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn map_count(&self) -> Result<u32, Never> {
        let Ok(range) = self.map_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn map_data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.map_data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn entry_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(EntryFormat::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn map_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.entry_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn map_data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(entry_format) = self.entry_format();
        let Ok(map_count) = self.map_count();
        let Ok(previous) = self.map_count_byte_range();
        let start = previous.end;
        let Ok(map_size) = EntryFormat::map_size(entry_format, map_count);
        let end = start.saturating_add(map_size.saturating_mul(u8::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    DeltaSetIndexMapFormat1::MIN_SIZE
); held }));

impl Default for DeltaSetIndexMapFormat1<'_> {
    fn default() -> Self {
        let Ok(value_9) = FontData::default_format_1_u8_table_data();

        Self {
            data: value_9,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for DeltaSetIndexMapFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("DeltaSetIndexMapFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "entry_format", value: FieldType::from(self.entry_format()) }),
            2u64 => Some(Field { name: "map_count", value: FieldType::from(self.map_count()) }),
            3u64 => Some(Field { name: "map_data", value: FieldType::from(self.map_data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for DeltaSetIndexMapFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone)]
pub enum DeltaSetIndexMap<'a> {
    Format0(DeltaSetIndexMapFormat0<'a>),
    Format1(DeltaSetIndexMapFormat1<'a>),
}

impl Default for DeltaSetIndexMap<'_> {
    fn default() -> Self {
        Self::Format0(Default::default())
    }
}

impl<'a> DeltaSetIndexMap<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Format0(item) => item.offset_data(),
            Self::Format1(item) => item.offset_data(),
        }
    }

    pub fn format(&self) -> Result<u8, Never> {
        match self {
            Self::Format0(item) => item.format(),
            Self::Format1(item) => item.format(),
        }
    }

    pub fn entry_format(&self) -> Result<EntryFormat, Never> {
        match self {
            Self::Format0(item) => item.entry_format(),
            Self::Format1(item) => item.entry_format(),
        }
    }

    pub fn map_data(&self) -> Result<&'a [u8], Never> {
        match self {
            Self::Format0(item) => item.map_data(),
            Self::Format1(item) => item.map_data(),
        }
    }
}

impl<'a> FontRead<'a> for DeltaSetIndexMap<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u8 = data.read_at(0u64)?;

        match format {
            DeltaSetIndexMapFormat0::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format0(read))
            }
            DeltaSetIndexMapFormat1::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format1(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u8>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for DeltaSetIndexMap<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        match self {
            Self::Format0(item) => item.min_byte_range(),
            Self::Format1(item) => item.min_byte_range(),
        }
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        match self {
            Self::Format0(item) => item.min_table_bytes(),
            Self::Format1(item) => item.min_table_bytes(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> DeltaSetIndexMap<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Format0(table) => table,
            Self::Format1(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for DeltaSetIndexMap<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for DeltaSetIndexMap<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck :: AnyBitPattern)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct EntryFormat {
    bits: u8,
}

impl EntryFormat {
    pub const INNER_INDEX_BIT_COUNT_MASK: Self = Self { bits: 0x0F };

    pub const MAP_ENTRY_SIZE_MASK: Self = Self { bits: 0x30 };
}

impl EntryFormat {
    const KNOWN: u8 = Self::INNER_INDEX_BIT_COUNT_MASK.bits
        | Self::MAP_ENTRY_SIZE_MASK.bits;

    #[inline]
    pub const fn empty() -> Result<Self, Never> {
        Ok(Self { bits: 0 })
    }

    #[inline]
    pub const fn all() -> Result<Self, Never> {
        Ok(Self { bits: Self::KNOWN })
    }

    #[inline]
    pub const fn bits(&self) -> Result<u8, Never> {
        Ok(self.bits)
    }

    #[inline]
    pub const fn from_bits(bits: u8) -> Result<Option<Self>, Never> {
        Ok(match bits & !Self::KNOWN {
            0 => Some(Self { bits }),
            _unknown => None,
        })
    }

    #[inline]
    pub const fn from_bits_truncate(bits: u8) -> Result<Self, Never> {
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

impl std::ops::BitOr for EntryFormat {
    type Output = Self;

    #[inline]
    fn bitor(self, other: EntryFormat) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl std::ops::BitOrAssign for EntryFormat {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.bits |= other.bits;
    }
}

impl std::ops::BitXor for EntryFormat {
    type Output = Self;

    #[inline]
    fn bitxor(self, other: Self) -> Self {
        Self {
            bits: self.bits ^ other.bits,
        }
    }
}

impl std::ops::BitXorAssign for EntryFormat {
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        self.bits ^= other.bits;
    }
}

impl std::ops::BitAnd for EntryFormat {
    type Output = Self;

    #[inline]
    fn bitand(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }
}

impl std::ops::BitAndAssign for EntryFormat {
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        self.bits &= other.bits;
    }
}

impl std::ops::Sub for EntryFormat {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }
}

impl std::ops::SubAssign for EntryFormat {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.bits &= !other.bits;
    }
}

impl std::ops::Not for EntryFormat {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        Self { bits: !self.bits & Self::KNOWN }
    }
}

impl std::fmt::Debug for EntryFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let members: &[(&str, Self)] = &[
            (
                "INNER_INDEX_BIT_COUNT_MASK",
                Self::INNER_INDEX_BIT_COUNT_MASK,
            ),
            ("MAP_ENTRY_SIZE_MASK", Self::MAP_ENTRY_SIZE_MASK),
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

impl std::fmt::Binary for EntryFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Binary::fmt(&self.bits, f)
    }
}

impl std::fmt::Octal for EntryFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Octal::fmt(&self.bits, f)
    }
}

impl std::fmt::LowerHex for EntryFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.bits, f)
    }
}

impl std::fmt::UpperHex for EntryFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.bits, f)
    }
}

impl font_types::Scalar for EntryFormat {
    type Raw = <u8 as font_types::Scalar>::Raw;

    fn to_raw(self) -> std::result::Result<Self::Raw, console_core_never::Never> {
        self.bits.to_raw()
    }

    fn from_raw(raw: Self::Raw) -> std::result::Result<Self, console_core_never::Never> {
        let Ok(bits) = <u8>::from_raw(raw);

        Self::from_bits_truncate(bits)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> From<EntryFormat> for FieldType<'a> {
    fn from(src: EntryFormat) -> FieldType<'a> {
        <crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u8>>::from(src.bits)
    }
}

impl<'a> MinByteRange<'a> for VariationRegionList<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(variation_regions_byte_range) = self.variation_regions_byte_range();

        Ok(0..variation_regions_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for VariationRegionList<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_15) = data.len();

        match ::core::primitive::bool::then_some(value_15 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct VariationRegionList<'a> {
    data: FontData<'a>,
}

impl<'a> VariationRegionList<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn axis_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.axis_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn region_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.region_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn variation_regions(&self) -> Result<ComputedArray<'a, VariationRegion<'a>>, Never> {
        let Ok(range) = self.variation_regions_byte_range();

        self.data.read_with_args_or_default(range, &({ let Ok(explicit_value) = self.axis_count(); explicit_value }))
    }

    pub fn axis_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn region_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.axis_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn variation_regions_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(region_count) = self.region_count();
        let Ok(previous) = self.region_count_byte_range();
        let start = previous.end;
        let Ok(value_16) = transforms::to_u64(region_count);

        let Ok(axis_count) = self.axis_count();
        let Ok(size_or_zero) = transforms::size_or_zero(<VariationRegion as ComputeSize>::compute_size(&axis_count));
        let end = start.saturating_add(value_16.saturating_mul(
                size_or_zero,
            ));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    VariationRegionList::MIN_SIZE
); held }));

impl Default for VariationRegionList<'_> {
    fn default() -> Self {
        let Ok(value_17) = FontData::default_table_data();

        Self {
            data: value_17,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for VariationRegionList<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("VariationRegionList")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "axis_count", value: FieldType::from(self.axis_count()) }),
            1u64 => Some(Field { name: "region_count", value: FieldType::from(self.region_count()) }),
            2u64 => {
                let Ok(value_18) = self.offset_data();

                let Ok(variation_regions) = self.variation_regions();

                Some(Field { name: "variation_regions", value: FieldType::from(traversal::FieldType::computed_array(
                    "VariationRegion",
                    variation_regions,
                    value_18,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for VariationRegionList<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VariationRegion<'a> {
    pub region_axes: &'a [RegionAxisCoordinates],
}

impl<'a> VariationRegion<'a> {
    pub fn region_axes(&self) -> Result<&'a [RegionAxisCoordinates], Never> {
        Ok(self.region_axes)
    }
}

impl ReadArgs for VariationRegion<'_> {
    type Arguments = u16;
}

impl ComputeSize for VariationRegion<'_> {

    fn compute_size(args: &u16) -> Result<u64, ReadError> {
        let axis_count = *args;
        let Ok(value_19) = transforms::to_u64(axis_count);

        Ok(value_19.saturating_mul(RegionAxisCoordinates::RAW_BYTE_LEN))
    }
}

impl<'a> FontReadWithArgs<'a> for VariationRegion<'a> {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let Ok(value_20) = data.cursor();

        let mut cursor = value_20;
        let axis_count = *args;
        let Ok(value_21) = transforms::to_u64(axis_count);

        let region_axes = cursor.read_array(value_21)?;


        Ok(Self {

            region_axes,

        })
    }
}

impl<'a> VariationRegion<'a> {
    pub fn read(data: FontData<'a>, axis_count: u16) -> Result<Self, ReadError> {
        let args = axis_count;
        Self::read_with_args(data, &args)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for VariationRegion<'a> {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "VariationRegion",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "region_axes", value: FieldType::from(traversal::FieldType::array_of_records(
                        stringify!(RegionAxisCoordinates),
                        { let Ok(explicit_value) = self.region_axes(); explicit_value },
                        _data,
                    )) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct RegionAxisCoordinates {
    pub start_coord: BigEndian<F2Dot14>,
    pub peak_coord: BigEndian<F2Dot14>,
    pub end_coord: BigEndian<F2Dot14>,
}

const _: () = {
  assert!(core::mem::size_of::<RegionAxisCoordinates>() == 6);
  assert!(core::mem::align_of::<RegionAxisCoordinates>() == 1);
  assert!(core::mem::offset_of!(RegionAxisCoordinates, start_coord) == 0);
  assert!(core::mem::offset_of!(RegionAxisCoordinates, peak_coord) == 2);
  assert!(core::mem::offset_of!(RegionAxisCoordinates, end_coord) == 4);
};

impl RegionAxisCoordinates {
    pub fn start_coord(&self) -> Result<F2Dot14, Never> {
        self.start_coord.get()
    }

    pub fn peak_coord(&self) -> Result<F2Dot14, Never> {
        self.peak_coord.get()
    }

    pub fn end_coord(&self) -> Result<F2Dot14, Never> {
        self.end_coord.get()
    }
}

impl FixedSize for RegionAxisCoordinates {
    const RAW_BYTE_LEN: u64 =
        F2Dot14::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for RegionAxisCoordinates {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "RegionAxisCoordinates",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "start_coord", value: FieldType::from(self.start_coord()) }),
                1u64 => Some(Field { name: "peak_coord", value: FieldType::from(self.peak_coord()) }),
                2u64 => Some(Field { name: "end_coord", value: FieldType::from(self.end_coord()) }),
                _ => None,
            }),
            data,
        })
    }
}

impl<'a> MinByteRange<'a> for ItemVariationStore<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(item_variation_data_offsets_byte_range) = self.item_variation_data_offsets_byte_range();

        Ok(0..item_variation_data_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ItemVariationStore<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_25) = data.len();

        match ::core::primitive::bool::then_some(value_25 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ItemVariationStore<'a> {
    data: FontData<'a>,
}

impl<'a> ItemVariationStore<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + Offset32::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn variation_region_list_offset(&self) -> Result<Offset32, Never> {
        let Ok(range) = self.variation_region_list_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn variation_region_list(&self) -> Result<VariationRegionList<'a>, ReadError> {
        let data = self.data;
        let Ok(variation_region_list_offset) = self.variation_region_list_offset();

        variation_region_list_offset.resolve(data)
    }

    pub fn item_variation_data_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.item_variation_data_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn item_variation_data_offsets(&self) -> Result<&'a [BigEndian<Nullable<Offset32>>], Never> {
        let Ok(range) = self.item_variation_data_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn item_variation_data(
        &self,
    ) -> Result<ArrayOfNullableOffsets<'a, ItemVariationData<'a>, Offset32>, Never> {
        let data = self.data;
        let Ok(offsets) = self.item_variation_data_offsets();
        ArrayOfNullableOffsets::new(offsets, data, ())
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn variation_region_list_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn item_variation_data_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.variation_region_list_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn item_variation_data_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(item_variation_data_count) = self.item_variation_data_count();
        let Ok(previous) = self.item_variation_data_count_byte_range();
        let start = previous.end;
        let Ok(value_26) = transforms::to_u64(item_variation_data_count);

        let end = start.saturating_add(value_26
                .saturating_mul(Offset32::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    ItemVariationStore::MIN_SIZE
); held }));

impl Default for ItemVariationStore<'_> {
    fn default() -> Self {
        let Ok(value_27) = FontData::default_table_data();

        Self {
            data: value_27,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ItemVariationStore<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ItemVariationStore")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "variation_region_list_offset", value: FieldType::from(FieldType::offset(
                    self.variation_region_list_offset(),
                    (self.variation_region_list()).map(Some),
                )) }),
            2u64 => Some(Field { name: "item_variation_data_count", value: FieldType::from(self.item_variation_data_count()) }),
            3u64 => Some(Field { name: "item_variation_data_offsets", value: FieldType::from(self.item_variation_data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ItemVariationStore<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for ItemVariationData<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(delta_sets_byte_range) = self.delta_sets_byte_range();

        Ok(0..delta_sets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ItemVariationData<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_28) = data.len();

        match ::core::primitive::bool::then_some(value_28 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ItemVariationData<'a> {
    data: FontData<'a>,
}

impl<'a> ItemVariationData<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn item_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.item_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn word_delta_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.word_delta_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn region_index_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.region_index_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn region_indexes(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.region_indexes_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn delta_sets(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.delta_sets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn item_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn word_delta_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.item_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn region_index_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.word_delta_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn region_indexes_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(region_index_count) = self.region_index_count();
        let Ok(previous) = self.region_index_count_byte_range();
        let start = previous.end;
        let Ok(value_29) = transforms::to_u64(region_index_count);

        let end =
            start.saturating_add(value_29.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn delta_sets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(item_count) = self.item_count();
        let Ok(word_delta_count) = self.word_delta_count();
        let Ok(region_index_count) = self.region_index_count();
        let Ok(previous) = self.region_indexes_byte_range();
        let start = previous.end;
        let end = start.saturating_add((ItemVariationData::delta_sets_len(item_count, word_delta_count, region_index_count))
                .saturating_mul(u8::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    ItemVariationData::MIN_SIZE
); held }));

impl Default for ItemVariationData<'_> {
    fn default() -> Self {
        let Ok(value_30) = FontData::default_table_data();

        Self {
            data: value_30,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ItemVariationData<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ItemVariationData")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "item_count", value: FieldType::from(self.item_count()) }),
            1u64 => Some(Field { name: "word_delta_count", value: FieldType::from(self.word_delta_count()) }),
            2u64 => Some(Field { name: "region_index_count", value: FieldType::from(self.region_index_count()) }),
            3u64 => Some(Field { name: "region_indexes", value: FieldType::from(self.region_indexes()) }),
            4u64 => Some(Field { name: "delta_sets", value: FieldType::from(self.delta_sets()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ItemVariationData<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
