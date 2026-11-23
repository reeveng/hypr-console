

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Gvar<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(glyph_variation_data_offsets_byte_range) = self.glyph_variation_data_offsets_byte_range();

        Ok(0..glyph_variation_data_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Gvar<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"gvar"); value });
}

impl<'a> FontRead<'a> for Gvar<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Gvar<'a> {
    data: FontData<'a>,
}

impl<'a> Gvar<'a> {
    pub const MIN_SIZE: u64 = (MajorMinor::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + GvarFlags::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<MajorMinor, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn axis_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.axis_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn shared_tuple_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.shared_tuple_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn shared_tuples_offset(&self) -> Result<Offset32, Never> {
        let Ok(range) = self.shared_tuples_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn shared_tuples(&self) -> Result<SharedTuples<'a>, ReadError> {
        let data = self.data;
        let Ok(shared_tuple_count) = self.shared_tuple_count();
        let Ok(axis_count) = self.axis_count();
        let args = (shared_tuple_count, axis_count);
        let Ok(shared_tuples_offset) = self.shared_tuples_offset();

        shared_tuples_offset.resolve_with_args(data, &args)
    }

    pub fn glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.glyph_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn flags(&self) -> Result<GvarFlags, Never> {
        let Ok(range) = self.flags_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn glyph_variation_data_array_offset(&self) -> Result<u32, Never> {
        let Ok(range) = self.glyph_variation_data_array_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn glyph_variation_data_offsets(&self) -> Result<ComputedArray<'a, U16Or32>, Never> {
        let Ok(range) = self.glyph_variation_data_offsets_byte_range();

        self.data.read_with_args_or_default(range, &({ let Ok(explicit_value) = self.flags(); explicit_value }))
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(MajorMinor::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn axis_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn shared_tuple_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.axis_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn shared_tuples_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.shared_tuple_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.shared_tuples_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn flags_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.glyph_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(GvarFlags::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_variation_data_array_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.flags_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_variation_data_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(glyph_count) = self.glyph_count();
        let Ok(previous) = self.glyph_variation_data_array_offset_byte_range();
        let start = previous.end;
        let Ok(value_2) = transforms::add(glyph_count, 1_u64);

        let Ok(flags) = self.flags();
        let Ok(size_or_zero) = transforms::size_or_zero(<U16Or32 as ComputeSize>::compute_size(&flags));
        let end = start.saturating_add(value_2
                .saturating_mul(size_or_zero));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Gvar::MIN_SIZE); held }));

impl Default for Gvar<'_> {
    fn default() -> Self {
        let Ok(value_3) = FontData::default_table_data();

        Self {
            data: value_3,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Gvar<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Gvar")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "axis_count", value: FieldType::from(self.axis_count()) }),
            2u64 => Some(Field { name: "shared_tuple_count", value: FieldType::from(self.shared_tuple_count()) }),
            3u64 => Some(Field { name: "shared_tuples_offset", value: FieldType::from(FieldType::offset(self.shared_tuples_offset(), (self.shared_tuples()).map(Some))) }),
            4u64 => Some(Field { name: "glyph_count", value: FieldType::from(self.glyph_count()) }),
            5u64 => Some(Field { name: "flags", value: FieldType::from(self.flags()) }),
            6u64 => Some(Field { name: "glyph_variation_data_array_offset", value: FieldType::from(self.glyph_variation_data_array_offset()) }),
            7u64 => Some(Field { name: "glyph_variation_data_offsets", value: traversal::FieldType::Unknown }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Gvar<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck :: AnyBitPattern)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct GvarFlags {
    bits: u16,
}

impl GvarFlags {
    pub const LONG_OFFSETS: Self = Self { bits: 1 };
}

impl GvarFlags {
    const KNOWN: u16 = Self::LONG_OFFSETS.bits;

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

impl std::ops::BitOr for GvarFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, other: GvarFlags) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl std::ops::BitOrAssign for GvarFlags {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.bits |= other.bits;
    }
}

impl std::ops::BitXor for GvarFlags {
    type Output = Self;

    #[inline]
    fn bitxor(self, other: Self) -> Self {
        Self {
            bits: self.bits ^ other.bits,
        }
    }
}

impl std::ops::BitXorAssign for GvarFlags {
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        self.bits ^= other.bits;
    }
}

impl std::ops::BitAnd for GvarFlags {
    type Output = Self;

    #[inline]
    fn bitand(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }
}

impl std::ops::BitAndAssign for GvarFlags {
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        self.bits &= other.bits;
    }
}

impl std::ops::Sub for GvarFlags {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }
}

impl std::ops::SubAssign for GvarFlags {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.bits &= !other.bits;
    }
}

impl std::ops::Not for GvarFlags {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        Self { bits: !self.bits & Self::KNOWN }
    }
}

impl std::fmt::Debug for GvarFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let members: &[(&str, Self)] = &[("LONG_OFFSETS", Self::LONG_OFFSETS)];
        let mut first = true;

        for (name, value) in members {
            let Ok(containing) = self.contains(*value);

            match containing { Presence::Present => {
                match ::core::primitive::bool::then_some(!first, ()) { ::core::option::Option::Some(()) => {
                    f.write_str(" | ")?;
                }, ::core::option::Option::None => {} }

                first = false;
                f.write_str(name)?;
            }, Presence::Absent => {} }
        }

        match ::core::primitive::bool::then_some(first, ()) { ::core::option::Option::Some(()) => {
            f.write_str("(empty)")?;
        }, ::core::option::Option::None => {} }

        Ok(())
    }
}

impl std::fmt::Binary for GvarFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Binary::fmt(&self.bits, f)
    }
}

impl std::fmt::Octal for GvarFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Octal::fmt(&self.bits, f)
    }
}

impl std::fmt::LowerHex for GvarFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.bits, f)
    }
}

impl std::fmt::UpperHex for GvarFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.bits, f)
    }
}

impl font_types::Scalar for GvarFlags {
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
impl<'a> From<GvarFlags> for FieldType<'a> {
    fn from(src: GvarFlags) -> FieldType<'a> {
        <crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u16>>::from(src.bits)
    }
}

impl<'a> MinByteRange<'a> for SharedTuples<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(tuples_byte_range) = self.tuples_byte_range();

        Ok(0..tuples_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl ReadArgs for SharedTuples<'_> {
    type Arguments = (u16, u16);
}

impl<'a> FontReadWithArgs<'a> for SharedTuples<'a> {
    fn read_with_args(data: FontData<'a>, args: &(u16, u16)) -> Result<Self, ReadError> {
        let (shared_tuple_count, axis_count) = *args;

        Ok(Self {
            data,
            shared_tuple_count,
            axis_count,
        })
    }
}

impl<'a> SharedTuples<'a> {
    #[cfg_attr(
      dylint_lib = "explicit024_no_swappable_params",
      expect(explicit024_no_swappable_params, reason = "the published `SharedTuples::read` method retains the positional argument order for `data`, `shared_tuple_count`, `axis_count`")
    )]
    pub fn read(
        data: FontData<'a>,
        shared_tuple_count: u16,
        axis_count: u16,
    ) -> Result<Self, ReadError> {
        let args = (shared_tuple_count, axis_count);
        Self::read_with_args(data, &args)
    }
}

#[derive(Clone)]
pub struct SharedTuples<'a> {
    data: FontData<'a>,
    shared_tuple_count: u16,
    axis_count: u16,
}

impl<'a> SharedTuples<'a> {
    pub const MIN_SIZE: u64 = 0;
    basic_table_impls!(impl_the_methods);

    pub fn tuples(&self) -> Result<ComputedArray<'a, Tuple<'a>>, Never> {
        let Ok(range) = self.tuples_byte_range();

        self.data.read_with_args_or_default(range, &({ let Ok(explicit_value) = self.axis_count(); explicit_value }))
    }

    pub(crate) fn shared_tuple_count(&self) -> Result<u16, Never> {
        Ok(self.shared_tuple_count)
    }

    pub(crate) fn axis_count(&self) -> Result<u16, Never> {
        Ok(self.axis_count)
    }

    pub fn tuples_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(shared_tuple_count) = self.shared_tuple_count();
        let start = 0u64;
        let Ok(value_8) = transforms::to_u64(shared_tuple_count);

        let Ok(axis_count) = self.axis_count();
        let Ok(size_or_zero) = transforms::size_or_zero(<Tuple as ComputeSize>::compute_size(&axis_count));
        let end = start.saturating_add(value_8.saturating_mul(
                size_or_zero,
            ));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(SharedTuples::MIN_SIZE); held }));

impl Default for SharedTuples<'_> {
    fn default() -> Self {
        let Ok(value_9) = FontData::default_table_data();

        Self {
            data: value_9,
            shared_tuple_count: Default::default(),
            axis_count: Default::default(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for SharedTuples<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("SharedTuples")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => {
                let Ok(value_10) = self.offset_data();

                let Ok(tuples) = self.tuples();

                Some(Field { name: "tuples", value: FieldType::from(traversal::FieldType::computed_array("Tuple", tuples, value_10)) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for SharedTuples<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for GlyphVariationDataHeader<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(tuple_variation_headers_byte_range) = self.tuple_variation_headers_byte_range();

        Ok(0..tuple_variation_headers_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for GlyphVariationDataHeader<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_11) = data.len();

        match ::core::primitive::bool::then_some(value_11 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct GlyphVariationDataHeader<'a> {
    data: FontData<'a>,
}

impl<'a> GlyphVariationDataHeader<'a> {
    pub const MIN_SIZE: u64 = (TupleVariationCount::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn tuple_variation_count(&self) -> Result<TupleVariationCount, Never> {
        let Ok(range) = self.tuple_variation_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn serialized_data_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.serialized_data_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn serialized_data(&self) -> Result<FontData<'a>, ReadError> {
        let data = self.data;
        let Ok(serialized_data_offset) = self.serialized_data_offset();

        serialized_data_offset.resolve(data)
    }

    pub fn tuple_variation_headers(&self) -> Result<VarLenArray<'a, TupleVariationHeader<'a>>, Never> {
        let Ok(range) = self.tuple_variation_headers_byte_range();

        self.data.read_var_array_or_empty(range.start)
    }

    pub fn tuple_variation_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(TupleVariationCount::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn serialized_data_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.tuple_variation_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn tuple_variation_headers_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.serialized_data_offset_byte_range();
        let start = previous.end;
        let Ok(value_13) = self.data.len();

        let end = start.saturating_add(value_13.saturating_sub(start));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    GlyphVariationDataHeader::MIN_SIZE
); held }));

impl Default for GlyphVariationDataHeader<'_> {
    fn default() -> Self {
        let Ok(value_12) = FontData::default_table_data();

        Self {
            data: value_12,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for GlyphVariationDataHeader<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("GlyphVariationDataHeader")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "tuple_variation_count", value: traversal::FieldType::Unknown }),
            1u64 => Some(Field { name: "serialized_data_offset", value: traversal::FieldType::Unknown }),
            2u64 => Some(Field { name: "tuple_variation_headers", value: traversal::FieldType::Unknown }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for GlyphVariationDataHeader<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
