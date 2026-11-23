

use crate::codegen_prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct BitmapSize {
    pub index_subtable_list_offset: BigEndian<u32>,
    pub index_subtable_list_size: BigEndian<u32>,
    pub number_of_index_subtables: BigEndian<u32>,
    pub color_ref: BigEndian<u32>,
    pub hori: SbitLineMetrics,
    pub vert: SbitLineMetrics,
    pub start_glyph_index: BigEndian<GlyphId16>,
    pub end_glyph_index: BigEndian<GlyphId16>,
    pub ppem_x: u8,
    pub ppem_y: u8,
    pub bit_depth: u8,
    pub flags: BigEndian<BitmapFlags>,
}

const _: () = {
  assert!(core::mem::size_of::<BitmapSize>() == 48);
  assert!(core::mem::align_of::<BitmapSize>() == 1);
  assert!(core::mem::offset_of!(BitmapSize, index_subtable_list_offset) == 0);
  assert!(core::mem::offset_of!(BitmapSize, index_subtable_list_size) == 4);
  assert!(core::mem::offset_of!(BitmapSize, number_of_index_subtables) == 8);
  assert!(core::mem::offset_of!(BitmapSize, color_ref) == 12);
  assert!(core::mem::offset_of!(BitmapSize, hori) == 16);
  assert!(core::mem::offset_of!(BitmapSize, vert) == 28);
  assert!(core::mem::offset_of!(BitmapSize, start_glyph_index) == 40);
  assert!(core::mem::offset_of!(BitmapSize, end_glyph_index) == 42);
  assert!(core::mem::offset_of!(BitmapSize, ppem_x) == 44);
  assert!(core::mem::offset_of!(BitmapSize, ppem_y) == 45);
  assert!(core::mem::offset_of!(BitmapSize, bit_depth) == 46);
  assert!(core::mem::offset_of!(BitmapSize, flags) == 47);
};

impl BitmapSize {
    pub fn index_subtable_list_offset(&self) -> Result<u32, Never> {
        self.index_subtable_list_offset.get()
    }

    pub fn index_subtable_list_size(&self) -> Result<u32, Never> {
        self.index_subtable_list_size.get()
    }

    pub fn number_of_index_subtables(&self) -> Result<u32, Never> {
        self.number_of_index_subtables.get()
    }

    pub fn color_ref(&self) -> Result<u32, Never> {
        self.color_ref.get()
    }

    pub fn hori(&self) -> Result<&SbitLineMetrics, Never> {
        Ok(&self.hori)
    }

    pub fn vert(&self) -> Result<&SbitLineMetrics, Never> {
        Ok(&self.vert)
    }

    pub fn start_glyph_index(&self) -> Result<GlyphId16, Never> {
        self.start_glyph_index.get()
    }

    pub fn end_glyph_index(&self) -> Result<GlyphId16, Never> {
        self.end_glyph_index.get()
    }

    pub fn ppem_x(&self) -> Result<u8, Never> {
        Ok(self.ppem_x)
    }

    pub fn ppem_y(&self) -> Result<u8, Never> {
        Ok(self.ppem_y)
    }

    pub fn bit_depth(&self) -> Result<u8, Never> {
        Ok(self.bit_depth)
    }

    pub fn flags(&self) -> Result<BitmapFlags, Never> {
        self.flags.get()
    }
}

impl FixedSize for BitmapSize {
    const RAW_BYTE_LEN: u64 = u32::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + SbitLineMetrics::RAW_BYTE_LEN
        + SbitLineMetrics::RAW_BYTE_LEN
        + GlyphId16::RAW_BYTE_LEN
        + GlyphId16::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN
        + BitmapFlags::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for BitmapSize {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "BitmapSize",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "index_subtable_list_offset", value: FieldType::from(self.index_subtable_list_offset()) }),
                1u64 => Some(Field { name: "index_subtable_list_size", value: FieldType::from(self.index_subtable_list_size()) }),
                2u64 => Some(Field { name: "number_of_index_subtables", value: FieldType::from(self.number_of_index_subtables()) }),
                3u64 => Some(Field { name: "color_ref", value: FieldType::from(self.color_ref()) }),
                4u64 => {
                    let Ok(hori) = self.hori();

                    let Ok(value) = hori.traversal_type(_data);

                    Some(Field { name: "hori", value })
                }
                5u64 => {
                    let Ok(vert) = self.vert();

                    let Ok(value) = vert.traversal_type(_data);

                    Some(Field { name: "vert", value })
                }
                6u64 => Some(Field { name: "start_glyph_index", value: FieldType::from(self.start_glyph_index()) }),
                7u64 => Some(Field { name: "end_glyph_index", value: FieldType::from(self.end_glyph_index()) }),
                8u64 => Some(Field { name: "ppem_x", value: FieldType::from(self.ppem_x()) }),
                9u64 => Some(Field { name: "ppem_y", value: FieldType::from(self.ppem_y()) }),
                10u64 => Some(Field { name: "bit_depth", value: FieldType::from(self.bit_depth()) }),
                11u64 => Some(Field { name: "flags", value: FieldType::from(self.flags()) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct SbitLineMetrics {
    pub ascender: BigEndian<i8>,
    pub descender: BigEndian<i8>,
    pub width_max: u8,
    pub caret_slope_numerator: BigEndian<i8>,
    pub caret_slope_denominator: u8,
    pub caret_offset: BigEndian<i8>,
    pub min_origin_sb: BigEndian<i8>,
    pub min_advance_sb: BigEndian<i8>,
    pub max_before_bl: BigEndian<i8>,
    pub min_after_bl: BigEndian<i8>,
    pub pad1: BigEndian<i8>,
    pub pad2: BigEndian<i8>,
}

const _: () = {
  assert!(core::mem::size_of::<SbitLineMetrics>() == 12);
  assert!(core::mem::align_of::<SbitLineMetrics>() == 1);
  assert!(core::mem::offset_of!(SbitLineMetrics, ascender) == 0);
  assert!(core::mem::offset_of!(SbitLineMetrics, descender) == 1);
  assert!(core::mem::offset_of!(SbitLineMetrics, width_max) == 2);
  assert!(core::mem::offset_of!(SbitLineMetrics, caret_slope_numerator) == 3);
  assert!(core::mem::offset_of!(SbitLineMetrics, caret_slope_denominator) == 4);
  assert!(core::mem::offset_of!(SbitLineMetrics, caret_offset) == 5);
  assert!(core::mem::offset_of!(SbitLineMetrics, min_origin_sb) == 6);
  assert!(core::mem::offset_of!(SbitLineMetrics, min_advance_sb) == 7);
  assert!(core::mem::offset_of!(SbitLineMetrics, max_before_bl) == 8);
  assert!(core::mem::offset_of!(SbitLineMetrics, min_after_bl) == 9);
  assert!(core::mem::offset_of!(SbitLineMetrics, pad1) == 10);
  assert!(core::mem::offset_of!(SbitLineMetrics, pad2) == 11);
};

impl SbitLineMetrics {
    pub fn ascender(&self) -> Result<i8, Never> {
        self.ascender.get()
    }

    pub fn descender(&self) -> Result<i8, Never> {
        self.descender.get()
    }

    pub fn width_max(&self) -> Result<u8, Never> {
        Ok(self.width_max)
    }

    pub fn caret_slope_numerator(&self) -> Result<i8, Never> {
        self.caret_slope_numerator.get()
    }

    pub fn caret_slope_denominator(&self) -> Result<u8, Never> {
        Ok(self.caret_slope_denominator)
    }

    pub fn caret_offset(&self) -> Result<i8, Never> {
        self.caret_offset.get()
    }

    pub fn min_origin_sb(&self) -> Result<i8, Never> {
        self.min_origin_sb.get()
    }

    pub fn min_advance_sb(&self) -> Result<i8, Never> {
        self.min_advance_sb.get()
    }

    pub fn max_before_bl(&self) -> Result<i8, Never> {
        self.max_before_bl.get()
    }

    pub fn min_after_bl(&self) -> Result<i8, Never> {
        self.min_after_bl.get()
    }

    pub fn pad1(&self) -> Result<i8, Never> {
        self.pad1.get()
    }

    pub fn pad2(&self) -> Result<i8, Never> {
        self.pad2.get()
    }
}

impl FixedSize for SbitLineMetrics {
    const RAW_BYTE_LEN: u64 = i8::RAW_BYTE_LEN
        + i8::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN
        + i8::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN
        + i8::RAW_BYTE_LEN
        + i8::RAW_BYTE_LEN
        + i8::RAW_BYTE_LEN
        + i8::RAW_BYTE_LEN
        + i8::RAW_BYTE_LEN
        + i8::RAW_BYTE_LEN
        + i8::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for SbitLineMetrics {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "SbitLineMetrics",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "ascender", value: FieldType::from(self.ascender()) }),
                1u64 => Some(Field { name: "descender", value: FieldType::from(self.descender()) }),
                2u64 => Some(Field { name: "width_max", value: FieldType::from(self.width_max()) }),
                3u64 => Some(Field { name: "caret_slope_numerator", value: FieldType::from(self.caret_slope_numerator()) }),
                4u64 => Some(Field { name: "caret_slope_denominator", value: FieldType::from(self.caret_slope_denominator()) }),
                5u64 => Some(Field { name: "caret_offset", value: FieldType::from(self.caret_offset()) }),
                6u64 => Some(Field { name: "min_origin_sb", value: FieldType::from(self.min_origin_sb()) }),
                7u64 => Some(Field { name: "min_advance_sb", value: FieldType::from(self.min_advance_sb()) }),
                8u64 => Some(Field { name: "max_before_bl", value: FieldType::from(self.max_before_bl()) }),
                9u64 => Some(Field { name: "min_after_bl", value: FieldType::from(self.min_after_bl()) }),
                10u64 => Some(Field { name: "pad1", value: FieldType::from(self.pad1()) }),
                11u64 => Some(Field { name: "pad2", value: FieldType::from(self.pad2()) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck :: AnyBitPattern)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct BitmapFlags {
    bits: u8,
}

impl BitmapFlags {
    pub const HORIZONTAL_METRICS: Self = Self { bits: 0x01 };

    pub const VERTICAL_METRICS: Self = Self { bits: 0x02 };
}

impl BitmapFlags {
    const KNOWN: u8 = Self::HORIZONTAL_METRICS.bits
        | Self::VERTICAL_METRICS.bits;

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

impl std::ops::BitOr for BitmapFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, other: BitmapFlags) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl std::ops::BitOrAssign for BitmapFlags {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.bits |= other.bits;
    }
}

impl std::ops::BitXor for BitmapFlags {
    type Output = Self;

    #[inline]
    fn bitxor(self, other: Self) -> Self {
        Self {
            bits: self.bits ^ other.bits,
        }
    }
}

impl std::ops::BitXorAssign for BitmapFlags {
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        self.bits ^= other.bits;
    }
}

impl std::ops::BitAnd for BitmapFlags {
    type Output = Self;

    #[inline]
    fn bitand(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }
}

impl std::ops::BitAndAssign for BitmapFlags {
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        self.bits &= other.bits;
    }
}

impl std::ops::Sub for BitmapFlags {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }
}

impl std::ops::SubAssign for BitmapFlags {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.bits &= !other.bits;
    }
}

impl std::ops::Not for BitmapFlags {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        Self { bits: !self.bits & Self::KNOWN }
    }
}

impl std::fmt::Debug for BitmapFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let members: &[(&str, Self)] = &[
            ("HORIZONTAL_METRICS", Self::HORIZONTAL_METRICS),
            ("VERTICAL_METRICS", Self::VERTICAL_METRICS),
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

impl std::fmt::Binary for BitmapFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Binary::fmt(&self.bits, f)
    }
}

impl std::fmt::Octal for BitmapFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Octal::fmt(&self.bits, f)
    }
}

impl std::fmt::LowerHex for BitmapFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.bits, f)
    }
}

impl std::fmt::UpperHex for BitmapFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.bits, f)
    }
}

impl font_types::Scalar for BitmapFlags {
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
impl<'a> From<BitmapFlags> for FieldType<'a> {
    fn from(src: BitmapFlags) -> FieldType<'a> {
        <crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u8>>::from(src.bits)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct BigGlyphMetrics {
    pub height: u8,
    pub width: u8,
    pub hori_bearing_x: BigEndian<i8>,
    pub hori_bearing_y: BigEndian<i8>,
    pub hori_advance: u8,
    pub vert_bearing_x: BigEndian<i8>,
    pub vert_bearing_y: BigEndian<i8>,
    pub vert_advance: u8,
}

const _: () = {
  assert!(core::mem::size_of::<BigGlyphMetrics>() == 8);
  assert!(core::mem::align_of::<BigGlyphMetrics>() == 1);
  assert!(core::mem::offset_of!(BigGlyphMetrics, height) == 0);
  assert!(core::mem::offset_of!(BigGlyphMetrics, width) == 1);
  assert!(core::mem::offset_of!(BigGlyphMetrics, hori_bearing_x) == 2);
  assert!(core::mem::offset_of!(BigGlyphMetrics, hori_bearing_y) == 3);
  assert!(core::mem::offset_of!(BigGlyphMetrics, hori_advance) == 4);
  assert!(core::mem::offset_of!(BigGlyphMetrics, vert_bearing_x) == 5);
  assert!(core::mem::offset_of!(BigGlyphMetrics, vert_bearing_y) == 6);
  assert!(core::mem::offset_of!(BigGlyphMetrics, vert_advance) == 7);
};

impl BigGlyphMetrics {
    pub fn height(&self) -> Result<u8, Never> {
        Ok(self.height)
    }

    pub fn width(&self) -> Result<u8, Never> {
        Ok(self.width)
    }

    pub fn hori_bearing_x(&self) -> Result<i8, Never> {
        self.hori_bearing_x.get()
    }

    pub fn hori_bearing_y(&self) -> Result<i8, Never> {
        self.hori_bearing_y.get()
    }

    pub fn hori_advance(&self) -> Result<u8, Never> {
        Ok(self.hori_advance)
    }

    pub fn vert_bearing_x(&self) -> Result<i8, Never> {
        self.vert_bearing_x.get()
    }

    pub fn vert_bearing_y(&self) -> Result<i8, Never> {
        self.vert_bearing_y.get()
    }

    pub fn vert_advance(&self) -> Result<u8, Never> {
        Ok(self.vert_advance)
    }
}

impl FixedSize for BigGlyphMetrics {
    const RAW_BYTE_LEN: u64 = u8::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN
        + i8::RAW_BYTE_LEN
        + i8::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN
        + i8::RAW_BYTE_LEN
        + i8::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for BigGlyphMetrics {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "BigGlyphMetrics",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "height", value: FieldType::from(self.height()) }),
                1u64 => Some(Field { name: "width", value: FieldType::from(self.width()) }),
                2u64 => Some(Field { name: "hori_bearing_x", value: FieldType::from(self.hori_bearing_x()) }),
                3u64 => Some(Field { name: "hori_bearing_y", value: FieldType::from(self.hori_bearing_y()) }),
                4u64 => Some(Field { name: "hori_advance", value: FieldType::from(self.hori_advance()) }),
                5u64 => Some(Field { name: "vert_bearing_x", value: FieldType::from(self.vert_bearing_x()) }),
                6u64 => Some(Field { name: "vert_bearing_y", value: FieldType::from(self.vert_bearing_y()) }),
                7u64 => Some(Field { name: "vert_advance", value: FieldType::from(self.vert_advance()) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct SmallGlyphMetrics {
    pub height: u8,
    pub width: u8,
    pub bearing_x: BigEndian<i8>,
    pub bearing_y: BigEndian<i8>,
    pub advance: u8,
}

const _: () = {
  assert!(core::mem::size_of::<SmallGlyphMetrics>() == 5);
  assert!(core::mem::align_of::<SmallGlyphMetrics>() == 1);
  assert!(core::mem::offset_of!(SmallGlyphMetrics, height) == 0);
  assert!(core::mem::offset_of!(SmallGlyphMetrics, width) == 1);
  assert!(core::mem::offset_of!(SmallGlyphMetrics, bearing_x) == 2);
  assert!(core::mem::offset_of!(SmallGlyphMetrics, bearing_y) == 3);
  assert!(core::mem::offset_of!(SmallGlyphMetrics, advance) == 4);
};

impl SmallGlyphMetrics {
    pub fn height(&self) -> Result<u8, Never> {
        Ok(self.height)
    }

    pub fn width(&self) -> Result<u8, Never> {
        Ok(self.width)
    }

    pub fn bearing_x(&self) -> Result<i8, Never> {
        self.bearing_x.get()
    }

    pub fn bearing_y(&self) -> Result<i8, Never> {
        self.bearing_y.get()
    }

    pub fn advance(&self) -> Result<u8, Never> {
        Ok(self.advance)
    }
}

impl FixedSize for SmallGlyphMetrics {
    const RAW_BYTE_LEN: u64 = u8::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN
        + i8::RAW_BYTE_LEN
        + i8::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for SmallGlyphMetrics {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "SmallGlyphMetrics",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "height", value: FieldType::from(self.height()) }),
                1u64 => Some(Field { name: "width", value: FieldType::from(self.width()) }),
                2u64 => Some(Field { name: "bearing_x", value: FieldType::from(self.bearing_x()) }),
                3u64 => Some(Field { name: "bearing_y", value: FieldType::from(self.bearing_y()) }),
                4u64 => Some(Field { name: "advance", value: FieldType::from(self.advance()) }),
                _ => None,
            }),
            data,
        })
    }
}

impl<'a> MinByteRange<'a> for IndexSubtableList<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(index_subtable_records_byte_range) = self.index_subtable_records_byte_range();

        Ok(0..index_subtable_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl ReadArgs for IndexSubtableList<'_> {
    type Arguments = u32;
}

impl<'a> FontReadWithArgs<'a> for IndexSubtableList<'a> {
    fn read_with_args(data: FontData<'a>, args: &u32) -> Result<Self, ReadError> {
        let number_of_index_subtables = *args;

        Ok(Self {
            data,
            number_of_index_subtables,
        })
    }
}

impl<'a> IndexSubtableList<'a> {
    pub fn read(data: FontData<'a>, number_of_index_subtables: u32) -> Result<Self, ReadError> {
        let args = number_of_index_subtables;
        Self::read_with_args(data, &args)
    }
}

#[derive(Clone)]
pub struct IndexSubtableList<'a> {
    data: FontData<'a>,
    number_of_index_subtables: u32,
}

impl<'a> IndexSubtableList<'a> {
    pub const MIN_SIZE: u64 = 0;
    basic_table_impls!(impl_the_methods);

    pub fn index_subtable_records(&self) -> Result<&'a [IndexSubtableRecord], Never> {
        let Ok(range) = self.index_subtable_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub(crate) fn number_of_index_subtables(&self) -> Result<u32, Never> {
        Ok(self.number_of_index_subtables)
    }

    pub fn index_subtable_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(number_of_index_subtables) = self.number_of_index_subtables();
        let start = 0u64;
        let Ok(value_28) = transforms::to_u64(number_of_index_subtables);

        let end = start.saturating_add(value_28
                .saturating_mul(IndexSubtableRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    IndexSubtableList::MIN_SIZE
); held }));

impl Default for IndexSubtableList<'_> {
    fn default() -> Self {
        let Ok(value_29) = FontData::default_table_data();

        Self {
            data: value_29,
            number_of_index_subtables: Default::default(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for IndexSubtableList<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("IndexSubtableList")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => {
                let Ok(value_30) = self.offset_data();

                Some(Field { name: "index_subtable_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(IndexSubtableRecord),
                    { let Ok(explicit_value) = self.index_subtable_records(); explicit_value },
                    value_30,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for IndexSubtableList<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct IndexSubtableRecord {
    pub first_glyph_index: BigEndian<GlyphId16>,
    pub last_glyph_index: BigEndian<GlyphId16>,
    pub index_subtable_offset: BigEndian<Offset32>,
}

const _: () = {
  assert!(core::mem::size_of::<IndexSubtableRecord>() == 8);
  assert!(core::mem::align_of::<IndexSubtableRecord>() == 1);
  assert!(core::mem::offset_of!(IndexSubtableRecord, first_glyph_index) == 0);
  assert!(core::mem::offset_of!(IndexSubtableRecord, last_glyph_index) == 2);
  assert!(core::mem::offset_of!(IndexSubtableRecord, index_subtable_offset) == 4);
};

impl IndexSubtableRecord {
    pub fn first_glyph_index(&self) -> Result<GlyphId16, Never> {
        self.first_glyph_index.get()
    }

    pub fn last_glyph_index(&self) -> Result<GlyphId16, Never> {
        self.last_glyph_index.get()
    }

    pub fn index_subtable_offset(&self) -> Result<Offset32, Never> {
        self.index_subtable_offset.get()
    }

    pub fn index_subtable<'a>(&self, data: FontData<'a>) -> Result<IndexSubtable<'a>, ReadError> {
        let Ok(last_glyph_index) = self.last_glyph_index();
        let Ok(first_glyph_index) = self.first_glyph_index();
        let args = (last_glyph_index, first_glyph_index);
        let Ok(index_subtable_offset) = self.index_subtable_offset();

        index_subtable_offset.resolve_with_args(data, &args)
    }
}

impl FixedSize for IndexSubtableRecord {
    const RAW_BYTE_LEN: u64 =
        GlyphId16::RAW_BYTE_LEN + GlyphId16::RAW_BYTE_LEN + Offset32::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for IndexSubtableRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "IndexSubtableRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "first_glyph_index", value: FieldType::from(self.first_glyph_index()) }),
                1u64 => Some(Field { name: "last_glyph_index", value: FieldType::from(self.last_glyph_index()) }),
                2u64 => Some(Field { name: "index_subtable_offset", value: FieldType::from(FieldType::offset(self.index_subtable_offset(), (self.index_subtable(_data)).map(Some))) }),
                _ => None,
            }),
            data,
        })
    }
}

impl Format<u16> for IndexSubtable1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for IndexSubtable1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(sbit_offsets_byte_range) = self.sbit_offsets_byte_range();

        Ok(0..sbit_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl ReadArgs for IndexSubtable1<'_> {
    type Arguments = (GlyphId16, GlyphId16);
}

impl<'a> FontReadWithArgs<'a> for IndexSubtable1<'a> {
    fn read_with_args(
        data: FontData<'a>,
        args: &(GlyphId16, GlyphId16),
    ) -> Result<Self, ReadError> {
        let (last_glyph_index, first_glyph_index) = *args;

        let Ok(value_34) = data.len();

        match ::core::primitive::bool::then_some(value_34 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self {
            data,
            last_glyph_index,
            first_glyph_index,
        })
    }
}

impl<'a> IndexSubtable1<'a> {
    pub fn read(
        data: FontData<'a>,
        last_glyph_index: GlyphId16,
        first_glyph_index: GlyphId16,
    ) -> Result<Self, ReadError> {
        let args = (last_glyph_index, first_glyph_index);
        Self::read_with_args(data, &args)
    }
}

#[derive(Clone)]
pub struct IndexSubtable1<'a> {
    data: FontData<'a>,
    last_glyph_index: GlyphId16,
    first_glyph_index: GlyphId16,
}

impl<'a> IndexSubtable1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn index_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.index_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn image_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.image_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn image_data_offset(&self) -> Result<u32, Never> {
        let Ok(range) = self.image_data_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn sbit_offsets(&self) -> Result<&'a [BigEndian<u32>], Never> {
        let Ok(range) = self.sbit_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub(crate) fn last_glyph_index(&self) -> Result<GlyphId16, Never> {
        Ok(self.last_glyph_index)
    }

    pub(crate) fn first_glyph_index(&self) -> Result<GlyphId16, Never> {
        Ok(self.first_glyph_index)
    }

    pub fn index_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn image_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.index_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn image_data_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.image_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn sbit_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(last_glyph_index) = self.last_glyph_index();
        let Ok(last_glyph_index_2) = last_glyph_index.to_u16();
        let Ok(first_glyph_index) = self.first_glyph_index();
        let Ok(first_glyph_index_2) = first_glyph_index.to_u16();
        let Ok(previous) = self.image_data_offset_byte_range();
        let start = previous.end;
        let Ok(value_35) = transforms::subtract_add_two(last_glyph_index_2, first_glyph_index_2);

        let end = start.saturating_add(value_35
                .saturating_mul(u32::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(IndexSubtable1::MIN_SIZE); held }));

impl Default for IndexSubtable1<'_> {
    fn default() -> Self {
        let Ok(value_36) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_36,
            last_glyph_index: Default::default(),
            first_glyph_index: Default::default(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for IndexSubtable1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("IndexSubtable1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "index_format", value: FieldType::from(self.index_format()) }),
            1u64 => Some(Field { name: "image_format", value: FieldType::from(self.image_format()) }),
            2u64 => Some(Field { name: "image_data_offset", value: FieldType::from(self.image_data_offset()) }),
            3u64 => Some(Field { name: "sbit_offsets", value: FieldType::from(self.sbit_offsets()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for IndexSubtable1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for IndexSubtable2<'_> {
    const FORMAT: u16 = 2;
}

impl<'a> MinByteRange<'a> for IndexSubtable2<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(big_metrics_byte_range) = self.big_metrics_byte_range();

        Ok(0..big_metrics_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for IndexSubtable2<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_37) = data.len();

        match ::core::primitive::bool::then_some(value_37 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct IndexSubtable2<'a> {
    data: FontData<'a>,
}

impl<'a> IndexSubtable2<'a> {
    pub const MIN_SIZE: u64 =
        (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn index_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.index_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn image_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.image_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn image_data_offset(&self) -> Result<u32, Never> {
        let Ok(range) = self.image_data_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn image_size(&self) -> Result<u32, Never> {
        let Ok(range) = self.image_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn big_metrics(&self) -> Result<&'a [BigGlyphMetrics], Never> {
        let Ok(range) = self.big_metrics_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn index_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn image_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.index_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn image_data_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.image_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn image_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.image_data_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn big_metrics_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.image_size_byte_range();
        let start = previous.end;
        let end = start.saturating_add(BigGlyphMetrics::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for IndexSubtable2<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("IndexSubtable2")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "index_format", value: FieldType::from(self.index_format()) }),
            1u64 => Some(Field { name: "image_format", value: FieldType::from(self.image_format()) }),
            2u64 => Some(Field { name: "image_data_offset", value: FieldType::from(self.image_data_offset()) }),
            3u64 => Some(Field { name: "image_size", value: FieldType::from(self.image_size()) }),
            4u64 => {
                let Ok(value_38) = self.offset_data();

                Some(Field { name: "big_metrics", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(BigGlyphMetrics),
                    { let Ok(explicit_value) = self.big_metrics(); explicit_value },
                    value_38,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for IndexSubtable2<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for IndexSubtable3<'_> {
    const FORMAT: u16 = 3;
}

impl<'a> MinByteRange<'a> for IndexSubtable3<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(sbit_offsets_byte_range) = self.sbit_offsets_byte_range();

        Ok(0..sbit_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl ReadArgs for IndexSubtable3<'_> {
    type Arguments = (GlyphId16, GlyphId16);
}

impl<'a> FontReadWithArgs<'a> for IndexSubtable3<'a> {
    fn read_with_args(
        data: FontData<'a>,
        args: &(GlyphId16, GlyphId16),
    ) -> Result<Self, ReadError> {
        let (last_glyph_index, first_glyph_index) = *args;

        let Ok(value_39) = data.len();

        match ::core::primitive::bool::then_some(value_39 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self {
            data,
            last_glyph_index,
            first_glyph_index,
        })
    }
}

impl<'a> IndexSubtable3<'a> {
    pub fn read(
        data: FontData<'a>,
        last_glyph_index: GlyphId16,
        first_glyph_index: GlyphId16,
    ) -> Result<Self, ReadError> {
        let args = (last_glyph_index, first_glyph_index);
        Self::read_with_args(data, &args)
    }
}

#[derive(Clone)]
pub struct IndexSubtable3<'a> {
    data: FontData<'a>,
    last_glyph_index: GlyphId16,
    first_glyph_index: GlyphId16,
}

impl<'a> IndexSubtable3<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn index_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.index_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn image_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.image_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn image_data_offset(&self) -> Result<u32, Never> {
        let Ok(range) = self.image_data_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn sbit_offsets(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.sbit_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub(crate) fn last_glyph_index(&self) -> Result<GlyphId16, Never> {
        Ok(self.last_glyph_index)
    }

    pub(crate) fn first_glyph_index(&self) -> Result<GlyphId16, Never> {
        Ok(self.first_glyph_index)
    }

    pub fn index_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn image_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.index_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn image_data_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.image_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn sbit_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(last_glyph_index) = self.last_glyph_index();
        let Ok(last_glyph_index_2) = last_glyph_index.to_u16();
        let Ok(first_glyph_index) = self.first_glyph_index();
        let Ok(first_glyph_index_2) = first_glyph_index.to_u16();
        let Ok(previous) = self.image_data_offset_byte_range();
        let start = previous.end;
        let Ok(value_40) = transforms::subtract_add_two(last_glyph_index_2, first_glyph_index_2);

        let end = start.saturating_add(value_40
                .saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for IndexSubtable3<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("IndexSubtable3")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "index_format", value: FieldType::from(self.index_format()) }),
            1u64 => Some(Field { name: "image_format", value: FieldType::from(self.image_format()) }),
            2u64 => Some(Field { name: "image_data_offset", value: FieldType::from(self.image_data_offset()) }),
            3u64 => Some(Field { name: "sbit_offsets", value: FieldType::from(self.sbit_offsets()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for IndexSubtable3<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for IndexSubtable4<'_> {
    const FORMAT: u16 = 4;
}

impl<'a> MinByteRange<'a> for IndexSubtable4<'a> {
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

impl<'a> FontRead<'a> for IndexSubtable4<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_41) = data.len();

        match ::core::primitive::bool::then_some(value_41 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct IndexSubtable4<'a> {
    data: FontData<'a>,
}

impl<'a> IndexSubtable4<'a> {
    pub const MIN_SIZE: u64 =
        (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn index_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.index_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn image_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.image_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn image_data_offset(&self) -> Result<u32, Never> {
        let Ok(range) = self.image_data_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_glyphs(&self) -> Result<u32, Never> {
        let Ok(range) = self.num_glyphs_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn glyph_array(&self) -> Result<&'a [GlyphIdOffsetPair], Never> {
        let Ok(range) = self.glyph_array_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn index_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn image_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.index_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn image_data_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.image_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_glyphs_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.image_data_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_array_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_glyphs) = self.num_glyphs();
        let Ok(previous) = self.num_glyphs_byte_range();
        let start = previous.end;
        let Ok(value_42) = transforms::add(num_glyphs, 1_u64);

        let end = start.saturating_add(value_42
                .saturating_mul(GlyphIdOffsetPair::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for IndexSubtable4<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("IndexSubtable4")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "index_format", value: FieldType::from(self.index_format()) }),
            1u64 => Some(Field { name: "image_format", value: FieldType::from(self.image_format()) }),
            2u64 => Some(Field { name: "image_data_offset", value: FieldType::from(self.image_data_offset()) }),
            3u64 => Some(Field { name: "num_glyphs", value: FieldType::from(self.num_glyphs()) }),
            4u64 => {
                let Ok(value_43) = self.offset_data();

                Some(Field { name: "glyph_array", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(GlyphIdOffsetPair),
                    { let Ok(explicit_value) = self.glyph_array(); explicit_value },
                    value_43,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for IndexSubtable4<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct GlyphIdOffsetPair {
    pub glyph_id: BigEndian<GlyphId16>,
    pub sbit_offset: BigEndian<u16>,
}

const _: () = {
  assert!(core::mem::size_of::<GlyphIdOffsetPair>() == 4);
  assert!(core::mem::align_of::<GlyphIdOffsetPair>() == 1);
  assert!(core::mem::offset_of!(GlyphIdOffsetPair, glyph_id) == 0);
  assert!(core::mem::offset_of!(GlyphIdOffsetPair, sbit_offset) == 2);
};

impl GlyphIdOffsetPair {
    pub fn glyph_id(&self) -> Result<GlyphId16, Never> {
        self.glyph_id.get()
    }

    pub fn sbit_offset(&self) -> Result<u16, Never> {
        self.sbit_offset.get()
    }
}

impl FixedSize for GlyphIdOffsetPair {
    const RAW_BYTE_LEN: u64 = GlyphId16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for GlyphIdOffsetPair {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "GlyphIdOffsetPair",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "glyph_id", value: FieldType::from(self.glyph_id()) }),
                1u64 => Some(Field { name: "sbit_offset", value: FieldType::from(self.sbit_offset()) }),
                _ => None,
            }),
            data,
        })
    }
}

impl Format<u16> for IndexSubtable5<'_> {
    const FORMAT: u16 = 5;
}

impl<'a> MinByteRange<'a> for IndexSubtable5<'a> {
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

impl<'a> FontRead<'a> for IndexSubtable5<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_46) = data.len();

        match ::core::primitive::bool::then_some(value_46 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct IndexSubtable5<'a> {
    data: FontData<'a>,
}

impl<'a> IndexSubtable5<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn index_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.index_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn image_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.image_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn image_data_offset(&self) -> Result<u32, Never> {
        let Ok(range) = self.image_data_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn image_size(&self) -> Result<u32, Never> {
        let Ok(range) = self.image_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn big_metrics(&self) -> Result<&'a [BigGlyphMetrics], Never> {
        let Ok(range) = self.big_metrics_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn num_glyphs(&self) -> Result<u32, Never> {
        let Ok(range) = self.num_glyphs_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn glyph_array(&self) -> Result<&'a [BigEndian<GlyphId16>], Never> {
        let Ok(range) = self.glyph_array_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn index_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn image_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.index_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn image_data_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.image_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn image_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.image_data_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn big_metrics_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.image_size_byte_range();
        let start = previous.end;
        let end = start.saturating_add(BigGlyphMetrics::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_glyphs_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.big_metrics_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_array_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_glyphs) = self.num_glyphs();
        let Ok(previous) = self.num_glyphs_byte_range();
        let start = previous.end;
        let Ok(value_47) = transforms::to_u64(num_glyphs);

        let end =
            start.saturating_add(value_47.saturating_mul(GlyphId16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for IndexSubtable5<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("IndexSubtable5")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "index_format", value: FieldType::from(self.index_format()) }),
            1u64 => Some(Field { name: "image_format", value: FieldType::from(self.image_format()) }),
            2u64 => Some(Field { name: "image_data_offset", value: FieldType::from(self.image_data_offset()) }),
            3u64 => Some(Field { name: "image_size", value: FieldType::from(self.image_size()) }),
            4u64 => {
                let Ok(value_48) = self.offset_data();

                Some(Field { name: "big_metrics", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(BigGlyphMetrics),
                    { let Ok(explicit_value) = self.big_metrics(); explicit_value },
                    value_48,
                )) })
            },
            5u64 => Some(Field { name: "num_glyphs", value: FieldType::from(self.num_glyphs()) }),
            6u64 => Some(Field { name: "glyph_array", value: FieldType::from(self.glyph_array()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for IndexSubtable5<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct BdtComponent {
    pub glyph_id: BigEndian<GlyphId16>,
    pub x_offset: BigEndian<i8>,
    pub y_offset: BigEndian<i8>,
}

const _: () = {
  assert!(core::mem::size_of::<BdtComponent>() == 4);
  assert!(core::mem::align_of::<BdtComponent>() == 1);
  assert!(core::mem::offset_of!(BdtComponent, glyph_id) == 0);
  assert!(core::mem::offset_of!(BdtComponent, x_offset) == 2);
  assert!(core::mem::offset_of!(BdtComponent, y_offset) == 3);
};

impl BdtComponent {
    pub fn glyph_id(&self) -> Result<GlyphId16, Never> {
        self.glyph_id.get()
    }

    pub fn x_offset(&self) -> Result<i8, Never> {
        self.x_offset.get()
    }

    pub fn y_offset(&self) -> Result<i8, Never> {
        self.y_offset.get()
    }
}

impl FixedSize for BdtComponent {
    const RAW_BYTE_LEN: u64 = GlyphId16::RAW_BYTE_LEN + i8::RAW_BYTE_LEN + i8::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for BdtComponent {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "BdtComponent",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "glyph_id", value: FieldType::from(self.glyph_id()) }),
                1u64 => Some(Field { name: "x_offset", value: FieldType::from(self.x_offset()) }),
                2u64 => Some(Field { name: "y_offset", value: FieldType::from(self.y_offset()) }),
                _ => None,
            }),
            data,
        })
    }
}
