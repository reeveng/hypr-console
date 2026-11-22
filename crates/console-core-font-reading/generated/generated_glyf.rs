

use crate::codegen_prelude::*;

impl TopLevelTable for Glyf<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"glyf"); value });
}

impl<'a> FontRead<'a> for Glyf<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Glyf<'a> {
    data: FontData<'a>,
}

impl<'a> Glyf<'a> {
    pub const MIN_SIZE: u64 = 0;
    basic_table_impls!(impl_the_methods);
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Glyf::MIN_SIZE); held }));

impl Default for Glyf<'_> {
    fn default() -> Self {
        let Ok(value_2) = FontData::default_table_data();

        Self {
            data: value_2,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Glyf<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Glyf")
    }

    fn get_field(&self, _idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(None)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Glyf<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for SimpleGlyph<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(glyph_data_byte_range) = self.glyph_data_byte_range();

        Ok(0..glyph_data_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for SimpleGlyph<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_3) = data.len();

        match ::core::primitive::bool::then_some(value_3 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct SimpleGlyph<'a> {
    data: FontData<'a>,
}

impl<'a> SimpleGlyph<'a> {
    pub const MIN_SIZE: u64 = (i16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn number_of_contours(&self) -> Result<i16, Never> {
        let Ok(range) = self.number_of_contours_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn x_min(&self) -> Result<i16, Never> {
        let Ok(range) = self.x_min_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_min(&self) -> Result<i16, Never> {
        let Ok(range) = self.y_min_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn x_max(&self) -> Result<i16, Never> {
        let Ok(range) = self.x_max_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_max(&self) -> Result<i16, Never> {
        let Ok(range) = self.y_max_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn end_pts_of_contours(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.end_pts_of_contours_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn instruction_length(&self) -> Result<u16, Never> {
        let Ok(range) = self.instruction_length_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn instructions(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.instructions_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn glyph_data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.glyph_data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn number_of_contours_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x_min_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.number_of_contours_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_min_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x_min_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x_max_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y_min_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_max_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x_max_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn end_pts_of_contours_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(number_of_contours) = self.number_of_contours();
        let Ok(previous) = self.y_max_byte_range();
        let start = previous.end;
        let Ok(value_4) = transforms::to_u64(number_of_contours);

        let end =
            start.saturating_add(value_4.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn instruction_length_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.end_pts_of_contours_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn instructions_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(instruction_length) = self.instruction_length();
        let Ok(previous) = self.instruction_length_byte_range();
        let start = previous.end;
        let Ok(value_5) = transforms::to_u64(instruction_length);

        let end =
            start.saturating_add(value_5.saturating_mul(u8::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn glyph_data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.instructions_byte_range();
        let start = previous.end;
        let Ok(value_15) = self.data.len();

        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_15.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(SimpleGlyph::MIN_SIZE); held }));

impl Default for SimpleGlyph<'_> {
    fn default() -> Self {
        let Ok(value_6) = FontData::default_table_data();

        Self {
            data: value_6,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for SimpleGlyph<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("SimpleGlyph")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "number_of_contours", value: FieldType::from(self.number_of_contours()) }),
            1u64 => Some(Field { name: "x_min", value: FieldType::from(self.x_min()) }),
            2u64 => Some(Field { name: "y_min", value: FieldType::from(self.y_min()) }),
            3u64 => Some(Field { name: "x_max", value: FieldType::from(self.x_max()) }),
            4u64 => Some(Field { name: "y_max", value: FieldType::from(self.y_max()) }),
            5u64 => Some(Field { name: "end_pts_of_contours", value: FieldType::from(self.end_pts_of_contours()) }),
            6u64 => Some(Field { name: "instruction_length", value: FieldType::from(self.instruction_length()) }),
            7u64 => Some(Field { name: "instructions", value: FieldType::from(self.instructions()) }),
            8u64 => Some(Field { name: "glyph_data", value: FieldType::from(self.glyph_data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for SimpleGlyph<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck :: AnyBitPattern)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct SimpleGlyphFlags {
    bits: u8,
}

impl SimpleGlyphFlags {
    pub const ON_CURVE_POINT: Self = Self { bits: 0x01 };

    pub const X_SHORT_VECTOR: Self = Self { bits: 0x02 };

    pub const Y_SHORT_VECTOR: Self = Self { bits: 0x04 };

    pub const REPEAT_FLAG: Self = Self { bits: 0x08 };

    pub const X_IS_SAME_OR_POSITIVE_X_SHORT_VECTOR: Self = Self { bits: 0x10 };

    pub const Y_IS_SAME_OR_POSITIVE_Y_SHORT_VECTOR: Self = Self { bits: 0x20 };

    pub const OVERLAP_SIMPLE: Self = Self { bits: 0x40 };

    pub const CUBIC: Self = Self { bits: 0x80 };
}

impl SimpleGlyphFlags {
    const KNOWN: u8 = Self::ON_CURVE_POINT.bits
        | Self::X_SHORT_VECTOR.bits
        | Self::Y_SHORT_VECTOR.bits
        | Self::REPEAT_FLAG.bits
        | Self::X_IS_SAME_OR_POSITIVE_X_SHORT_VECTOR.bits
        | Self::Y_IS_SAME_OR_POSITIVE_Y_SHORT_VECTOR.bits
        | Self::OVERLAP_SIMPLE.bits
        | Self::CUBIC.bits;

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

impl std::ops::BitOr for SimpleGlyphFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, other: SimpleGlyphFlags) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl std::ops::BitOrAssign for SimpleGlyphFlags {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.bits |= other.bits;
    }
}

impl std::ops::BitXor for SimpleGlyphFlags {
    type Output = Self;

    #[inline]
    fn bitxor(self, other: Self) -> Self {
        Self {
            bits: self.bits ^ other.bits,
        }
    }
}

impl std::ops::BitXorAssign for SimpleGlyphFlags {
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        self.bits ^= other.bits;
    }
}

impl std::ops::BitAnd for SimpleGlyphFlags {
    type Output = Self;

    #[inline]
    fn bitand(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }
}

impl std::ops::BitAndAssign for SimpleGlyphFlags {
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        self.bits &= other.bits;
    }
}

impl std::ops::Sub for SimpleGlyphFlags {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }
}

impl std::ops::SubAssign for SimpleGlyphFlags {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.bits &= !other.bits;
    }
}

impl std::ops::Not for SimpleGlyphFlags {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        Self { bits: !self.bits & Self::KNOWN }
    }
}

impl std::fmt::Debug for SimpleGlyphFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let members: &[(&str, Self)] = &[
            ("ON_CURVE_POINT", Self::ON_CURVE_POINT),
            ("X_SHORT_VECTOR", Self::X_SHORT_VECTOR),
            ("Y_SHORT_VECTOR", Self::Y_SHORT_VECTOR),
            ("REPEAT_FLAG", Self::REPEAT_FLAG),
            (
                "X_IS_SAME_OR_POSITIVE_X_SHORT_VECTOR",
                Self::X_IS_SAME_OR_POSITIVE_X_SHORT_VECTOR,
            ),
            (
                "Y_IS_SAME_OR_POSITIVE_Y_SHORT_VECTOR",
                Self::Y_IS_SAME_OR_POSITIVE_Y_SHORT_VECTOR,
            ),
            ("OVERLAP_SIMPLE", Self::OVERLAP_SIMPLE),
            ("CUBIC", Self::CUBIC),
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

impl std::fmt::Binary for SimpleGlyphFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Binary::fmt(&self.bits, f)
    }
}

impl std::fmt::Octal for SimpleGlyphFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Octal::fmt(&self.bits, f)
    }
}

impl std::fmt::LowerHex for SimpleGlyphFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.bits, f)
    }
}

impl std::fmt::UpperHex for SimpleGlyphFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.bits, f)
    }
}

impl font_types::Scalar for SimpleGlyphFlags {
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
impl<'a> From<SimpleGlyphFlags> for FieldType<'a> {
    fn from(src: SimpleGlyphFlags) -> FieldType<'a> {
        <crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u8>>::from(src.bits)
    }
}

impl<'a> MinByteRange<'a> for CompositeGlyph<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(component_data_byte_range) = self.component_data_byte_range();

        Ok(0..component_data_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for CompositeGlyph<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_10) = data.len();

        match ::core::primitive::bool::then_some(value_10 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct CompositeGlyph<'a> {
    data: FontData<'a>,
}

impl<'a> CompositeGlyph<'a> {
    pub const MIN_SIZE: u64 = (i16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn number_of_contours(&self) -> Result<i16, Never> {
        let Ok(range) = self.number_of_contours_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn x_min(&self) -> Result<i16, Never> {
        let Ok(range) = self.x_min_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_min(&self) -> Result<i16, Never> {
        let Ok(range) = self.y_min_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn x_max(&self) -> Result<i16, Never> {
        let Ok(range) = self.x_max_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_max(&self) -> Result<i16, Never> {
        let Ok(range) = self.y_max_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn component_data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.component_data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn number_of_contours_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x_min_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.number_of_contours_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_min_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x_min_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x_max_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y_min_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_max_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x_max_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn component_data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y_max_byte_range();
        let start = previous.end;
        let Ok(value_16) = self.data.len();

        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_16.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(CompositeGlyph::MIN_SIZE); held }));

impl Default for CompositeGlyph<'_> {
    fn default() -> Self {
        let Ok(value_11) = FontData::default_table_data();

        Self {
            data: value_11,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for CompositeGlyph<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("CompositeGlyph")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "number_of_contours", value: FieldType::from(self.number_of_contours()) }),
            1u64 => Some(Field { name: "x_min", value: FieldType::from(self.x_min()) }),
            2u64 => Some(Field { name: "y_min", value: FieldType::from(self.y_min()) }),
            3u64 => Some(Field { name: "x_max", value: FieldType::from(self.x_max()) }),
            4u64 => Some(Field { name: "y_max", value: FieldType::from(self.y_max()) }),
            5u64 => Some(Field { name: "component_data", value: FieldType::from(self.component_data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for CompositeGlyph<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck :: AnyBitPattern)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct CompositeGlyphFlags {
    bits: u16,
}

impl CompositeGlyphFlags {
    pub const ARG_1_AND_2_ARE_WORDS: Self = Self { bits: 0x0001 };

    pub const ARGS_ARE_XY_VALUES: Self = Self { bits: 0x0002 };

    pub const ROUND_XY_TO_GRID: Self = Self { bits: 0x0004 };

    pub const WE_HAVE_A_SCALE: Self = Self { bits: 0x0008 };

    pub const MORE_COMPONENTS: Self = Self { bits: 0x0020 };

    pub const WE_HAVE_AN_X_AND_Y_SCALE: Self = Self { bits: 0x0040 };

    pub const WE_HAVE_A_TWO_BY_TWO: Self = Self { bits: 0x0080 };

    pub const WE_HAVE_INSTRUCTIONS: Self = Self { bits: 0x0100 };

    pub const USE_MY_METRICS: Self = Self { bits: 0x0200 };

    pub const OVERLAP_COMPOUND: Self = Self { bits: 0x0400 };

    pub const SCALED_COMPONENT_OFFSET: Self = Self { bits: 0x0800 };

    pub const UNSCALED_COMPONENT_OFFSET: Self = Self { bits: 0x1000 };
}

impl CompositeGlyphFlags {
    const KNOWN: u16 = Self::ARG_1_AND_2_ARE_WORDS.bits
        | Self::ARGS_ARE_XY_VALUES.bits
        | Self::ROUND_XY_TO_GRID.bits
        | Self::WE_HAVE_A_SCALE.bits
        | Self::MORE_COMPONENTS.bits
        | Self::WE_HAVE_AN_X_AND_Y_SCALE.bits
        | Self::WE_HAVE_A_TWO_BY_TWO.bits
        | Self::WE_HAVE_INSTRUCTIONS.bits
        | Self::USE_MY_METRICS.bits
        | Self::OVERLAP_COMPOUND.bits
        | Self::SCALED_COMPONENT_OFFSET.bits
        | Self::UNSCALED_COMPONENT_OFFSET.bits;

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

impl std::ops::BitOr for CompositeGlyphFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, other: CompositeGlyphFlags) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl std::ops::BitOrAssign for CompositeGlyphFlags {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.bits |= other.bits;
    }
}

impl std::ops::BitXor for CompositeGlyphFlags {
    type Output = Self;

    #[inline]
    fn bitxor(self, other: Self) -> Self {
        Self {
            bits: self.bits ^ other.bits,
        }
    }
}

impl std::ops::BitXorAssign for CompositeGlyphFlags {
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        self.bits ^= other.bits;
    }
}

impl std::ops::BitAnd for CompositeGlyphFlags {
    type Output = Self;

    #[inline]
    fn bitand(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }
}

impl std::ops::BitAndAssign for CompositeGlyphFlags {
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        self.bits &= other.bits;
    }
}

impl std::ops::Sub for CompositeGlyphFlags {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }
}

impl std::ops::SubAssign for CompositeGlyphFlags {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.bits &= !other.bits;
    }
}

impl std::ops::Not for CompositeGlyphFlags {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        Self { bits: !self.bits & Self::KNOWN }
    }
}

impl std::fmt::Debug for CompositeGlyphFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let members: &[(&str, Self)] = &[
            ("ARG_1_AND_2_ARE_WORDS", Self::ARG_1_AND_2_ARE_WORDS),
            ("ARGS_ARE_XY_VALUES", Self::ARGS_ARE_XY_VALUES),
            ("ROUND_XY_TO_GRID", Self::ROUND_XY_TO_GRID),
            ("WE_HAVE_A_SCALE", Self::WE_HAVE_A_SCALE),
            ("MORE_COMPONENTS", Self::MORE_COMPONENTS),
            ("WE_HAVE_AN_X_AND_Y_SCALE", Self::WE_HAVE_AN_X_AND_Y_SCALE),
            ("WE_HAVE_A_TWO_BY_TWO", Self::WE_HAVE_A_TWO_BY_TWO),
            ("WE_HAVE_INSTRUCTIONS", Self::WE_HAVE_INSTRUCTIONS),
            ("USE_MY_METRICS", Self::USE_MY_METRICS),
            ("OVERLAP_COMPOUND", Self::OVERLAP_COMPOUND),
            ("SCALED_COMPONENT_OFFSET", Self::SCALED_COMPONENT_OFFSET),
            ("UNSCALED_COMPONENT_OFFSET", Self::UNSCALED_COMPONENT_OFFSET),
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

impl std::fmt::Binary for CompositeGlyphFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Binary::fmt(&self.bits, f)
    }
}

impl std::fmt::Octal for CompositeGlyphFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Octal::fmt(&self.bits, f)
    }
}

impl std::fmt::LowerHex for CompositeGlyphFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.bits, f)
    }
}

impl std::fmt::UpperHex for CompositeGlyphFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.bits, f)
    }
}

impl font_types::Scalar for CompositeGlyphFlags {
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
impl<'a> From<CompositeGlyphFlags> for FieldType<'a> {
    fn from(src: CompositeGlyphFlags) -> FieldType<'a> {
        <crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u16>>::from(src.bits)
    }
}

#[derive(Clone)]
pub enum Glyph<'a> {
    Simple(SimpleGlyph<'a>),
    Composite(CompositeGlyph<'a>),
}

impl Default for Glyph<'_> {
    fn default() -> Self {
        Self::Simple(Default::default())
    }
}

impl<'a> Glyph<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Simple(item) => item.offset_data(),
            Self::Composite(item) => item.offset_data(),
        }
    }

    pub fn number_of_contours(&self) -> Result<i16, Never> {
        match self {
            Self::Simple(item) => item.number_of_contours(),
            Self::Composite(item) => item.number_of_contours(),
        }
    }

    pub fn x_min(&self) -> Result<i16, Never> {
        match self {
            Self::Simple(item) => item.x_min(),
            Self::Composite(item) => item.x_min(),
        }
    }

    pub fn y_min(&self) -> Result<i16, Never> {
        match self {
            Self::Simple(item) => item.y_min(),
            Self::Composite(item) => item.y_min(),
        }
    }

    pub fn x_max(&self) -> Result<i16, Never> {
        match self {
            Self::Simple(item) => item.x_max(),
            Self::Composite(item) => item.x_max(),
        }
    }

    pub fn y_max(&self) -> Result<i16, Never> {
        match self {
            Self::Simple(item) => item.y_max(),
            Self::Composite(item) => item.y_max(),
        }
    }
}

impl<'a> FontRead<'a> for Glyph<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: i16 = data.read_at(0u64)?;

        match format {
            0.. => {
                let read = FontRead::read(data)?;

                Ok(Self::Simple(read))
            }
            ..0 => {
                let read = FontRead::read(data)?;

                Ok(Self::Composite(read))
            }
        }
    }
}

impl<'a> MinByteRange<'a> for Glyph<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        match self {
            Self::Simple(item) => item.min_byte_range(),
            Self::Composite(item) => item.min_byte_range(),
        }
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        match self {
            Self::Simple(item) => item.min_table_bytes(),
            Self::Composite(item) => item.min_table_bytes(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> Glyph<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Simple(table) => table,
            Self::Composite(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for Glyph<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Glyph<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}
