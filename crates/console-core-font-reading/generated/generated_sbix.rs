

use crate::codegen_prelude::*;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck :: AnyBitPattern)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct HeaderFlags {
    bits: u16,
}

impl HeaderFlags {
    pub const ALWAYS_SET: Self = Self { bits: 0x0001 };

    pub const DRAW_OUTLINES: Self = Self { bits: 0x0002 };
}

impl HeaderFlags {
    const KNOWN: u16 = Self::ALWAYS_SET.bits
        | Self::DRAW_OUTLINES.bits;

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

impl std::ops::BitOr for HeaderFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, other: HeaderFlags) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl std::ops::BitOrAssign for HeaderFlags {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.bits |= other.bits;
    }
}

impl std::ops::BitXor for HeaderFlags {
    type Output = Self;

    #[inline]
    fn bitxor(self, other: Self) -> Self {
        Self {
            bits: self.bits ^ other.bits,
        }
    }
}

impl std::ops::BitXorAssign for HeaderFlags {
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        self.bits ^= other.bits;
    }
}

impl std::ops::BitAnd for HeaderFlags {
    type Output = Self;

    #[inline]
    fn bitand(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }
}

impl std::ops::BitAndAssign for HeaderFlags {
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        self.bits &= other.bits;
    }
}

impl std::ops::Sub for HeaderFlags {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }
}

impl std::ops::SubAssign for HeaderFlags {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.bits &= !other.bits;
    }
}

impl std::ops::Not for HeaderFlags {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        Self { bits: !self.bits & Self::KNOWN }
    }
}

impl std::fmt::Debug for HeaderFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let members: &[(&str, Self)] = &[
            ("ALWAYS_SET", Self::ALWAYS_SET),
            ("DRAW_OUTLINES", Self::DRAW_OUTLINES),
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

impl std::fmt::Binary for HeaderFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Binary::fmt(&self.bits, f)
    }
}

impl std::fmt::Octal for HeaderFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Octal::fmt(&self.bits, f)
    }
}

impl std::fmt::LowerHex for HeaderFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.bits, f)
    }
}

impl std::fmt::UpperHex for HeaderFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.bits, f)
    }
}

impl font_types::Scalar for HeaderFlags {
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
impl<'a> From<HeaderFlags> for FieldType<'a> {
    fn from(src: HeaderFlags) -> FieldType<'a> {
        <crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u16>>::from(src.bits)
    }
}

impl<'a> MinByteRange<'a> for Sbix<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(strike_offsets_byte_range) = self.strike_offsets_byte_range();

        Ok(0..strike_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Sbix<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"sbix"); value });
}

impl ReadArgs for Sbix<'_> {
    type Arguments = u16;
}

impl<'a> FontReadWithArgs<'a> for Sbix<'a> {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let num_glyphs = *args;

        let Ok(value_4) = data.len();

        match ::core::primitive::bool::then_some(value_4 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data, num_glyphs })
    }
}

impl<'a> Sbix<'a> {
    pub fn read(data: FontData<'a>, num_glyphs: u16) -> Result<Self, ReadError> {
        let args = num_glyphs;
        Self::read_with_args(data, &args)
    }
}

#[derive(Clone)]
pub struct Sbix<'a> {
    data: FontData<'a>,
    num_glyphs: u16,
}

impl<'a> Sbix<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + HeaderFlags::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<u16, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn flags(&self) -> Result<HeaderFlags, Never> {
        let Ok(range) = self.flags_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_strikes(&self) -> Result<u32, Never> {
        let Ok(range) = self.num_strikes_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn strike_offsets(&self) -> Result<&'a [BigEndian<Offset32>], Never> {
        let Ok(range) = self.strike_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn strikes(&self) -> Result<ArrayOfOffsets<'a, Strike<'a>, Offset32>, Never> {
        let data = self.data;
        let Ok(offsets) = self.strike_offsets();
        let Ok(args) = self.num_glyphs();
        ArrayOfOffsets::new(offsets, data, args)
    }

    pub(crate) fn num_glyphs(&self) -> Result<u16, Never> {
        Ok(self.num_glyphs)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn flags_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(HeaderFlags::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_strikes_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.flags_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn strike_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_strikes) = self.num_strikes();
        let Ok(previous) = self.num_strikes_byte_range();
        let start = previous.end;
        let Ok(value_5) = transforms::to_u64(num_strikes);

        let end =
            start.saturating_add(value_5.saturating_mul(Offset32::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Sbix::MIN_SIZE); held }));

impl Default for Sbix<'_> {
    fn default() -> Self {
        let Ok(value_6) = FontData::default_table_data();

        Self {
            data: value_6,
            num_glyphs: Default::default(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Sbix<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Sbix")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "flags", value: FieldType::from(self.flags()) }),
            2u64 => Some(Field { name: "num_strikes", value: FieldType::from(self.num_strikes()) }),
            3u64 => Some(Field { name: "strike_offsets", value: FieldType::from(self.strikes()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Sbix<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for Strike<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(glyph_data_offsets_byte_range) = self.glyph_data_offsets_byte_range();

        Ok(0..glyph_data_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl ReadArgs for Strike<'_> {
    type Arguments = u16;
}

impl<'a> FontReadWithArgs<'a> for Strike<'a> {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let num_glyphs = *args;

        let Ok(value_7) = data.len();

        match ::core::primitive::bool::then_some(value_7 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data, num_glyphs })
    }
}

impl<'a> Strike<'a> {
    pub fn read(data: FontData<'a>, num_glyphs: u16) -> Result<Self, ReadError> {
        let args = num_glyphs;
        Self::read_with_args(data, &args)
    }
}

#[derive(Clone)]
pub struct Strike<'a> {
    data: FontData<'a>,
    num_glyphs: u16,
}

impl<'a> Strike<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn ppem(&self) -> Result<u16, Never> {
        let Ok(range) = self.ppem_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn ppi(&self) -> Result<u16, Never> {
        let Ok(range) = self.ppi_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn glyph_data_offsets(&self) -> Result<&'a [BigEndian<u32>], Never> {
        let Ok(range) = self.glyph_data_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub(crate) fn num_glyphs(&self) -> Result<u16, Never> {
        Ok(self.num_glyphs)
    }

    pub fn ppem_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn ppi_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.ppem_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_data_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_glyphs) = self.num_glyphs();
        let Ok(previous) = self.ppi_byte_range();
        let start = previous.end;
        let Ok(value_8) = transforms::add(num_glyphs, 1_u64);

        let end = start.saturating_add(value_8.saturating_mul(u32::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Strike::MIN_SIZE); held }));

impl Default for Strike<'_> {
    fn default() -> Self {
        let Ok(value_9) = FontData::default_table_data();

        Self {
            data: value_9,
            num_glyphs: Default::default(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Strike<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Strike")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "ppem", value: FieldType::from(self.ppem()) }),
            1u64 => Some(Field { name: "ppi", value: FieldType::from(self.ppi()) }),
            2u64 => Some(Field { name: "glyph_data_offsets", value: FieldType::from(self.glyph_data_offsets()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Strike<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for GlyphData<'a> {
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

impl<'a> FontRead<'a> for GlyphData<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_10) = data.len();

        match ::core::primitive::bool::then_some(value_10 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct GlyphData<'a> {
    data: FontData<'a>,
}

impl<'a> GlyphData<'a> {
    pub const MIN_SIZE: u64 = (i16::RAW_BYTE_LEN + i16::RAW_BYTE_LEN + Tag::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn origin_offset_x(&self) -> Result<i16, Never> {
        let Ok(range) = self.origin_offset_x_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn origin_offset_y(&self) -> Result<i16, Never> {
        let Ok(range) = self.origin_offset_y_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn graphic_type(&self) -> Result<Tag, Never> {
        let Ok(range) = self.graphic_type_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn origin_offset_x_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn origin_offset_y_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.origin_offset_x_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn graphic_type_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.origin_offset_y_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Tag::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.graphic_type_byte_range();
        let start = previous.end;
        let Ok(value_12) = self.data.len();

        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_12.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(GlyphData::MIN_SIZE); held }));

impl Default for GlyphData<'_> {
    fn default() -> Self {
        let Ok(value_11) = FontData::default_table_data();

        Self {
            data: value_11,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for GlyphData<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("GlyphData")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "origin_offset_x", value: FieldType::from(self.origin_offset_x()) }),
            1u64 => Some(Field { name: "origin_offset_y", value: FieldType::from(self.origin_offset_y()) }),
            2u64 => Some(Field { name: "graphic_type", value: FieldType::from(self.graphic_type()) }),
            3u64 => Some(Field { name: "data", value: FieldType::from(self.data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for GlyphData<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
