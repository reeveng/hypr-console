

use crate::codegen_prelude::*;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck :: AnyBitPattern)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct SelectionFlags {
    bits: u16,
}

impl SelectionFlags {
    pub const ITALIC: Self = Self { bits: 0x0001 };

    pub const UNDERSCORE: Self = Self { bits: 0x0002 };

    pub const NEGATIVE: Self = Self { bits: 0x0004 };

    pub const OUTLINED: Self = Self { bits: 0x0008 };

    pub const STRIKEOUT: Self = Self { bits: 0x0010 };

    pub const BOLD: Self = Self { bits: 0x0020 };

    pub const REGULAR: Self = Self { bits: 0x0040 };

    pub const USE_TYPO_METRICS: Self = Self { bits: 0x0080 };

    pub const WWS: Self = Self { bits: 0x0100 };

    pub const OBLIQUE: Self = Self { bits: 0x0200 };
}

impl SelectionFlags {
    const KNOWN: u16 = Self::ITALIC.bits
        | Self::UNDERSCORE.bits
        | Self::NEGATIVE.bits
        | Self::OUTLINED.bits
        | Self::STRIKEOUT.bits
        | Self::BOLD.bits
        | Self::REGULAR.bits
        | Self::USE_TYPO_METRICS.bits
        | Self::WWS.bits
        | Self::OBLIQUE.bits;

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

impl std::ops::BitOr for SelectionFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, other: SelectionFlags) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl std::ops::BitOrAssign for SelectionFlags {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.bits |= other.bits;
    }
}

impl std::ops::BitXor for SelectionFlags {
    type Output = Self;

    #[inline]
    fn bitxor(self, other: Self) -> Self {
        Self {
            bits: self.bits ^ other.bits,
        }
    }
}

impl std::ops::BitXorAssign for SelectionFlags {
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        self.bits ^= other.bits;
    }
}

impl std::ops::BitAnd for SelectionFlags {
    type Output = Self;

    #[inline]
    fn bitand(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }
}

impl std::ops::BitAndAssign for SelectionFlags {
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        self.bits &= other.bits;
    }
}

impl std::ops::Sub for SelectionFlags {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }
}

impl std::ops::SubAssign for SelectionFlags {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.bits &= !other.bits;
    }
}

impl std::ops::Not for SelectionFlags {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        Self { bits: !self.bits & Self::KNOWN }
    }
}

impl std::fmt::Debug for SelectionFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let members: &[(&str, Self)] = &[
            ("ITALIC", Self::ITALIC),
            ("UNDERSCORE", Self::UNDERSCORE),
            ("NEGATIVE", Self::NEGATIVE),
            ("OUTLINED", Self::OUTLINED),
            ("STRIKEOUT", Self::STRIKEOUT),
            ("BOLD", Self::BOLD),
            ("REGULAR", Self::REGULAR),
            ("USE_TYPO_METRICS", Self::USE_TYPO_METRICS),
            ("WWS", Self::WWS),
            ("OBLIQUE", Self::OBLIQUE),
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

impl std::fmt::Binary for SelectionFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Binary::fmt(&self.bits, f)
    }
}

impl std::fmt::Octal for SelectionFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Octal::fmt(&self.bits, f)
    }
}

impl std::fmt::LowerHex for SelectionFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.bits, f)
    }
}

impl std::fmt::UpperHex for SelectionFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.bits, f)
    }
}

impl font_types::Scalar for SelectionFlags {
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
impl<'a> From<SelectionFlags> for FieldType<'a> {
    fn from(src: SelectionFlags) -> FieldType<'a> {
        <crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u16>>::from(src.bits)
    }
}

impl<'a> MinByteRange<'a> for Os2<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(us_win_descent_byte_range) = self.us_win_descent_byte_range();

        Ok(0..us_win_descent_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Os2<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"OS/2"); value });
}

impl<'a> FontRead<'a> for Os2<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_4) = data.len();

        match ::core::primitive::bool::then_some(value_4 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Os2<'a> {
    data: FontData<'a>,
}

impl<'a> Os2<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN * 10_u64
        + u32::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + Tag::RAW_BYTE_LEN
        + SelectionFlags::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<u16, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn x_avg_char_width(&self) -> Result<i16, Never> {
        let Ok(range) = self.x_avg_char_width_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn us_weight_class(&self) -> Result<u16, Never> {
        let Ok(range) = self.us_weight_class_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn us_width_class(&self) -> Result<u16, Never> {
        let Ok(range) = self.us_width_class_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn fs_type(&self) -> Result<u16, Never> {
        let Ok(range) = self.fs_type_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_subscript_x_size(&self) -> Result<i16, Never> {
        let Ok(range) = self.y_subscript_x_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_subscript_y_size(&self) -> Result<i16, Never> {
        let Ok(range) = self.y_subscript_y_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_subscript_x_offset(&self) -> Result<i16, Never> {
        let Ok(range) = self.y_subscript_x_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_subscript_y_offset(&self) -> Result<i16, Never> {
        let Ok(range) = self.y_subscript_y_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_superscript_x_size(&self) -> Result<i16, Never> {
        let Ok(range) = self.y_superscript_x_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_superscript_y_size(&self) -> Result<i16, Never> {
        let Ok(range) = self.y_superscript_y_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_superscript_x_offset(&self) -> Result<i16, Never> {
        let Ok(range) = self.y_superscript_x_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_superscript_y_offset(&self) -> Result<i16, Never> {
        let Ok(range) = self.y_superscript_y_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_strikeout_size(&self) -> Result<i16, Never> {
        let Ok(range) = self.y_strikeout_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_strikeout_position(&self) -> Result<i16, Never> {
        let Ok(range) = self.y_strikeout_position_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn s_family_class(&self) -> Result<i16, Never> {
        let Ok(range) = self.s_family_class_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn panose_10(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.panose_10_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn ul_unicode_range_1(&self) -> Result<u32, Never> {
        let Ok(range) = self.ul_unicode_range_1_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn ul_unicode_range_2(&self) -> Result<u32, Never> {
        let Ok(range) = self.ul_unicode_range_2_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn ul_unicode_range_3(&self) -> Result<u32, Never> {
        let Ok(range) = self.ul_unicode_range_3_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn ul_unicode_range_4(&self) -> Result<u32, Never> {
        let Ok(range) = self.ul_unicode_range_4_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn ach_vend_id(&self) -> Result<Tag, Never> {
        let Ok(range) = self.ach_vend_id_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn fs_selection(&self) -> Result<SelectionFlags, Never> {
        let Ok(range) = self.fs_selection_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn us_first_char_index(&self) -> Result<u16, Never> {
        let Ok(range) = self.us_first_char_index_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn us_last_char_index(&self) -> Result<u16, Never> {
        let Ok(range) = self.us_last_char_index_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn s_typo_ascender(&self) -> Result<i16, Never> {
        let Ok(range) = self.s_typo_ascender_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn s_typo_descender(&self) -> Result<i16, Never> {
        let Ok(range) = self.s_typo_descender_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn s_typo_line_gap(&self) -> Result<i16, Never> {
        let Ok(range) = self.s_typo_line_gap_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn us_win_ascent(&self) -> Result<u16, Never> {
        let Ok(range) = self.us_win_ascent_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn us_win_descent(&self) -> Result<u16, Never> {
        let Ok(range) = self.us_win_descent_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn ul_code_page_range_1(&self) -> Result<Option<u32>, Never> {
        let Ok(range) = self.ul_code_page_range_1_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn ul_code_page_range_2(&self) -> Result<Option<u32>, Never> {
        let Ok(range) = self.ul_code_page_range_2_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn sx_height(&self) -> Result<Option<i16>, Never> {
        let Ok(range) = self.sx_height_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn s_cap_height(&self) -> Result<Option<i16>, Never> {
        let Ok(range) = self.s_cap_height_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn us_default_char(&self) -> Result<Option<u16>, Never> {
        let Ok(range) = self.us_default_char_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn us_break_char(&self) -> Result<Option<u16>, Never> {
        let Ok(range) = self.us_break_char_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn us_max_context(&self) -> Result<Option<u16>, Never> {
        let Ok(range) = self.us_max_context_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn us_lower_optical_point_size(&self) -> Result<Option<u16>, Never> {
        let Ok(range) = self.us_lower_optical_point_size_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn us_upper_optical_point_size(&self) -> Result<Option<u16>, Never> {
        let Ok(range) = self.us_upper_optical_point_size_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x_avg_char_width_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn us_weight_class_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x_avg_char_width_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn us_width_class_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.us_weight_class_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn fs_type_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.us_width_class_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_subscript_x_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.fs_type_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_subscript_y_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y_subscript_x_size_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_subscript_x_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y_subscript_y_size_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_subscript_y_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y_subscript_x_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_superscript_x_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y_subscript_y_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_superscript_y_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y_superscript_x_size_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_superscript_x_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y_superscript_y_size_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_superscript_y_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y_superscript_x_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_strikeout_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y_superscript_y_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_strikeout_position_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y_strikeout_size_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn s_family_class_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y_strikeout_position_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn panose_10_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.s_family_class_byte_range();
        let start = previous.end;
        let Ok(value_15) = console_core_number_conversion::index(u8::RAW_BYTE_LEN);

        let Ok(value_17) = console_core_number_conversion::fitted::<_, u64>(value_15);

        let Ok(value_16) = console_core_number_conversion::fitted::<_, u64>((10_u64).saturating_mul(value_17));

        let end = start.saturating_add(value_16);
        Ok(start..end)
    }

    pub fn ul_unicode_range_1_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.panose_10_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn ul_unicode_range_2_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.ul_unicode_range_1_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn ul_unicode_range_3_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.ul_unicode_range_2_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn ul_unicode_range_4_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.ul_unicode_range_3_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn ach_vend_id_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.ul_unicode_range_4_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Tag::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn fs_selection_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.ach_vend_id_byte_range();
        let start = previous.end;
        let end = start.saturating_add(SelectionFlags::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn us_first_char_index_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.fs_selection_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn us_last_char_index_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.us_first_char_index_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn s_typo_ascender_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.us_last_char_index_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn s_typo_descender_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.s_typo_ascender_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn s_typo_line_gap_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.s_typo_descender_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn us_win_ascent_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.s_typo_line_gap_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn us_win_descent_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.us_win_ascent_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn ul_code_page_range_1_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.us_win_descent_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility(1u16);

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(u32::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }

    pub fn ul_code_page_range_2_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.ul_code_page_range_1_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility(1u16);

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(u32::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }

    pub fn sx_height_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.ul_code_page_range_2_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility(2u16);

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(i16::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }

    pub fn s_cap_height_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.sx_height_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility(2u16);

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(i16::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }

    pub fn us_default_char_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.s_cap_height_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility(2u16);

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(u16::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }

    pub fn us_break_char_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.us_default_char_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility(2u16);

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(u16::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }

    pub fn us_max_context_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.us_break_char_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility(2u16);

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(u16::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }

    pub fn us_lower_optical_point_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.us_max_context_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility(5u16);

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(u16::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }

    pub fn us_upper_optical_point_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.us_lower_optical_point_size_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility(5u16);

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(u16::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Os2::MIN_SIZE); held }));

impl Default for Os2<'_> {
    fn default() -> Self {
        let Ok(value_14) = FontData::default_table_data();

        Self {
            data: value_14,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Os2<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Os2")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "x_avg_char_width", value: FieldType::from(self.x_avg_char_width()) }),
            2u64 => Some(Field { name: "us_weight_class", value: FieldType::from(self.us_weight_class()) }),
            3u64 => Some(Field { name: "us_width_class", value: FieldType::from(self.us_width_class()) }),
            4u64 => Some(Field { name: "fs_type", value: FieldType::from(self.fs_type()) }),
            5u64 => Some(Field { name: "y_subscript_x_size", value: FieldType::from(self.y_subscript_x_size()) }),
            6u64 => Some(Field { name: "y_subscript_y_size", value: FieldType::from(self.y_subscript_y_size()) }),
            7u64 => Some(Field { name: "y_subscript_x_offset", value: FieldType::from(self.y_subscript_x_offset()) }),
            8u64 => Some(Field { name: "y_subscript_y_offset", value: FieldType::from(self.y_subscript_y_offset()) }),
            9u64 => Some(Field { name: "y_superscript_x_size", value: FieldType::from(self.y_superscript_x_size()) }),
            10u64 => Some(Field { name: "y_superscript_y_size", value: FieldType::from(self.y_superscript_y_size()) }),
            11u64 => Some(Field { name: "y_superscript_x_offset", value: FieldType::from(self.y_superscript_x_offset()) }),
            12u64 => Some(Field { name: "y_superscript_y_offset", value: FieldType::from(self.y_superscript_y_offset()) }),
            13u64 => Some(Field { name: "y_strikeout_size", value: FieldType::from(self.y_strikeout_size()) }),
            14u64 => Some(Field { name: "y_strikeout_position", value: FieldType::from(self.y_strikeout_position()) }),
            15u64 => Some(Field { name: "s_family_class", value: FieldType::from(self.s_family_class()) }),
            16u64 => Some(Field { name: "panose_10", value: FieldType::from(self.panose_10()) }),
            17u64 => Some(Field { name: "ul_unicode_range_1", value: FieldType::from(self.ul_unicode_range_1()) }),
            18u64 => Some(Field { name: "ul_unicode_range_2", value: FieldType::from(self.ul_unicode_range_2()) }),
            19u64 => Some(Field { name: "ul_unicode_range_3", value: FieldType::from(self.ul_unicode_range_3()) }),
            20u64 => Some(Field { name: "ul_unicode_range_4", value: FieldType::from(self.ul_unicode_range_4()) }),
            21u64 => Some(Field { name: "ach_vend_id", value: FieldType::from(self.ach_vend_id()) }),
            22u64 => Some(Field { name: "fs_selection", value: FieldType::from(self.fs_selection()) }),
            23u64 => Some(Field { name: "us_first_char_index", value: FieldType::from(self.us_first_char_index()) }),
            24u64 => Some(Field { name: "us_last_char_index", value: FieldType::from(self.us_last_char_index()) }),
            25u64 => Some(Field { name: "s_typo_ascender", value: FieldType::from(self.s_typo_ascender()) }),
            26u64 => Some(Field { name: "s_typo_descender", value: FieldType::from(self.s_typo_descender()) }),
            27u64 => Some(Field { name: "s_typo_line_gap", value: FieldType::from(self.s_typo_line_gap()) }),
            28u64 => Some(Field { name: "us_win_ascent", value: FieldType::from(self.us_win_ascent()) }),
            29u64 => Some(Field { name: "us_win_descent", value: FieldType::from(self.us_win_descent()) }),
            30u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility(1u16);
                let Ok(ul_code_page_range_1) = self.ul_code_page_range_1();

                match compatibility {
                    Compatibility::Compatible => ul_code_page_range_1.map(|ul_code_page_range_1_2| Field { name: "ul_code_page_range_1", value: FieldType::from(ul_code_page_range_1_2) }),
                    Compatibility::Incompatible => None,
                }
            },
            31u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility(1u16);
                let Ok(ul_code_page_range_2) = self.ul_code_page_range_2();

                match compatibility {
                    Compatibility::Compatible => ul_code_page_range_2.map(|ul_code_page_range_2_2| Field { name: "ul_code_page_range_2", value: FieldType::from(ul_code_page_range_2_2) }),
                    Compatibility::Incompatible => None,
                }
            },
            32u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility(2u16);
                let Ok(sx_height) = self.sx_height();

                match compatibility {
                    Compatibility::Compatible => sx_height.map(|sx_height_2| Field { name: "sx_height", value: FieldType::from(sx_height_2) }),
                    Compatibility::Incompatible => None,
                }
            },
            33u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility(2u16);
                let Ok(s_cap_height) = self.s_cap_height();

                match compatibility {
                    Compatibility::Compatible => s_cap_height.map(|s_cap_height_2| Field { name: "s_cap_height", value: FieldType::from(s_cap_height_2) }),
                    Compatibility::Incompatible => None,
                }
            },
            34u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility(2u16);
                let Ok(us_default_char) = self.us_default_char();

                match compatibility {
                    Compatibility::Compatible => us_default_char.map(|us_default_char_2| Field { name: "us_default_char", value: FieldType::from(us_default_char_2) }),
                    Compatibility::Incompatible => None,
                }
            },
            35u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility(2u16);
                let Ok(us_break_char) = self.us_break_char();

                match compatibility {
                    Compatibility::Compatible => us_break_char.map(|us_break_char_2| Field { name: "us_break_char", value: FieldType::from(us_break_char_2) }),
                    Compatibility::Incompatible => None,
                }
            },
            36u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility(2u16);
                let Ok(us_max_context) = self.us_max_context();

                match compatibility {
                    Compatibility::Compatible => us_max_context.map(|us_max_context_2| Field { name: "us_max_context", value: FieldType::from(us_max_context_2) }),
                    Compatibility::Incompatible => None,
                }
            },
            37u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility(5u16);
                let Ok(us_lower_optical_point_size) = self.us_lower_optical_point_size();

                match compatibility {
                    Compatibility::Compatible => us_lower_optical_point_size.map(|us_lower_optical_point_size_2| Field { name: "us_lower_optical_point_size", value: FieldType::from(us_lower_optical_point_size_2) }),
                    Compatibility::Incompatible => None,
                }
            },
            38u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility(5u16);
                let Ok(us_upper_optical_point_size) = self.us_upper_optical_point_size();

                match compatibility {
                    Compatibility::Compatible => us_upper_optical_point_size.map(|us_upper_optical_point_size_2| Field { name: "us_upper_optical_point_size", value: FieldType::from(us_upper_optical_point_size_2) }),
                    Compatibility::Incompatible => None,
                }
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Os2<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
