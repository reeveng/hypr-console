use crate::codegen_prelude::*;

#[derive(Clone)]
pub enum Ift<'a> {
    Format1(PatchMapFormat1<'a>),
    Format2(PatchMapFormat2<'a>),
}

impl Default for Ift<'_> {
    fn default() -> Self {
        Self::Format1(Default::default())
    }
}

impl<'a> Ift<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Format1(item) => item.offset_data(),
            Self::Format2(item) => item.offset_data(),
        }
    }

    pub fn format(&self) -> Result<u8, Never> {
        match self {
            Self::Format1(item) => item.format(),
            Self::Format2(item) => item.format(),
        }
    }

    pub fn field_flags(&self) -> Result<PatchMapFieldPresenceFlags, Never> {
        match self {
            Self::Format1(item) => item.field_flags(),
            Self::Format2(item) => item.field_flags(),
        }
    }

    pub fn compatibility_id(&self) -> Result<CompatibilityId, Never> {
        match self {
            Self::Format1(item) => item.compatibility_id(),
            Self::Format2(item) => item.compatibility_id(),
        }
    }

    pub fn url_template_length(&self) -> Result<u16, Never> {
        match self {
            Self::Format1(item) => item.url_template_length(),
            Self::Format2(item) => item.url_template_length(),
        }
    }

    pub fn url_template(&self) -> Result<&'a [u8], Never> {
        match self {
            Self::Format1(item) => item.url_template(),
            Self::Format2(item) => item.url_template(),
        }
    }

    pub fn cff_charstrings_offset(&self) -> Result<Option<u32>, Never> {
        match self {
            Self::Format1(item) => item.cff_charstrings_offset(),
            Self::Format2(item) => item.cff_charstrings_offset(),
        }
    }

    pub fn cff2_charstrings_offset(&self) -> Result<Option<u32>, Never> {
        match self {
            Self::Format1(item) => item.cff2_charstrings_offset(),
            Self::Format2(item) => item.cff2_charstrings_offset(),
        }
    }
}

impl<'a> FontRead<'a> for Ift<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u8 = data.read_at(0u64)?;

        match format {
            PatchMapFormat1::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format1(read))
            }
            PatchMapFormat2::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format2(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u8>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for Ift<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        match self {
            Self::Format1(item) => item.min_byte_range(),
            Self::Format2(item) => item.min_byte_range(),
        }
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        match self {
            Self::Format1(item) => item.min_table_bytes(),
            Self::Format2(item) => item.min_table_bytes(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> Ift<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Format1(table) => table,
            Self::Format2(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for Ift<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Ift<'a> {
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
pub struct PatchMapFieldPresenceFlags {
    bits: u8,
}

impl PatchMapFieldPresenceFlags {
    pub const CFF_CHARSTRINGS_OFFSET: Self = Self { bits: 0b00000001 };

    pub const CFF2_CHARSTRINGS_OFFSET: Self = Self { bits: 0b00000010 };
}

impl PatchMapFieldPresenceFlags {
    const KNOWN: u8 = Self::CFF_CHARSTRINGS_OFFSET.bits
        | Self::CFF2_CHARSTRINGS_OFFSET.bits;

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

impl std::ops::BitOr for PatchMapFieldPresenceFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, other: PatchMapFieldPresenceFlags) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl std::ops::BitOrAssign for PatchMapFieldPresenceFlags {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.bits |= other.bits;
    }
}

impl std::ops::BitXor for PatchMapFieldPresenceFlags {
    type Output = Self;

    #[inline]
    fn bitxor(self, other: Self) -> Self {
        Self {
            bits: self.bits ^ other.bits,
        }
    }
}

impl std::ops::BitXorAssign for PatchMapFieldPresenceFlags {
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        self.bits ^= other.bits;
    }
}

impl std::ops::BitAnd for PatchMapFieldPresenceFlags {
    type Output = Self;

    #[inline]
    fn bitand(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }
}

impl std::ops::BitAndAssign for PatchMapFieldPresenceFlags {
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        self.bits &= other.bits;
    }
}

impl std::ops::Sub for PatchMapFieldPresenceFlags {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }
}

impl std::ops::SubAssign for PatchMapFieldPresenceFlags {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.bits &= !other.bits;
    }
}

impl std::ops::Not for PatchMapFieldPresenceFlags {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        Self { bits: !self.bits & Self::KNOWN }
    }
}

impl std::fmt::Debug for PatchMapFieldPresenceFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let members: &[(&str, Self)] = &[
            ("CFF_CHARSTRINGS_OFFSET", Self::CFF_CHARSTRINGS_OFFSET),
            ("CFF2_CHARSTRINGS_OFFSET", Self::CFF2_CHARSTRINGS_OFFSET),
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

impl std::fmt::Binary for PatchMapFieldPresenceFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Binary::fmt(&self.bits, f)
    }
}

impl std::fmt::Octal for PatchMapFieldPresenceFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Octal::fmt(&self.bits, f)
    }
}

impl std::fmt::LowerHex for PatchMapFieldPresenceFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.bits, f)
    }
}

impl std::fmt::UpperHex for PatchMapFieldPresenceFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.bits, f)
    }
}

impl font_types::Scalar for PatchMapFieldPresenceFlags {
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
impl<'a> From<PatchMapFieldPresenceFlags> for FieldType<'a> {
    fn from(src: PatchMapFieldPresenceFlags) -> FieldType<'a> {
        <crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u8>>::from(src.bits)
    }
}

impl Format<u8> for PatchMapFormat1<'_> {
    const FORMAT: u8 = 1;
}

impl<'a> MinByteRange<'a> for PatchMapFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(patch_format_byte_range) = self.patch_format_byte_range();

        Ok(0..patch_format_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PatchMapFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_6) = data.len();

        match ::core::primitive::bool::then_some(value_6 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PatchMapFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> PatchMapFormat1<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN
        + PatchMapFieldPresenceFlags::RAW_BYTE_LEN
        + CompatibilityId::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + Uint24::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn field_flags(&self) -> Result<PatchMapFieldPresenceFlags, Never> {
        let Ok(range) = self.field_flags_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn compatibility_id(&self) -> Result<CompatibilityId, Never> {
        let Ok(range) = self.compatibility_id_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn max_entry_index(&self) -> Result<u16, Never> {
        let Ok(range) = self.max_entry_index_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn max_glyph_map_entry_index(&self) -> Result<u16, Never> {
        let Ok(range) = self.max_glyph_map_entry_index_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn glyph_count(&self) -> Result<Uint24, Never> {
        let Ok(range) = self.glyph_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn glyph_map_offset(&self) -> Result<Offset32, Never> {
        let Ok(range) = self.glyph_map_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn glyph_map(&self) -> Result<GlyphMap<'a>, ReadError> {
        let data = self.data;
        let Ok(glyph_count) = self.glyph_count();
        let Ok(max_entry_index) = self.max_entry_index();
        let args = (glyph_count, max_entry_index);
        let Ok(glyph_map_offset) = self.glyph_map_offset();

        glyph_map_offset.resolve_with_args(data, &args)
    }

    pub fn feature_map_offset(&self) -> Result<Nullable<Offset32>, Never> {
        let Ok(range) = self.feature_map_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn feature_map(&self) -> Result<Option<FeatureMap<'a>>, ReadError> {
        let data = self.data;
        let Ok(args) = self.max_entry_index();
        let Ok(feature_map_offset) = self.feature_map_offset();

        feature_map_offset.resolve_with_args(data, &args)
    }

    pub fn applied_entries_bitmap(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.applied_entries_bitmap_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn url_template_length(&self) -> Result<u16, Never> {
        let Ok(range) = self.url_template_length_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn url_template(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.url_template_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn patch_format(&self) -> Result<u8, Never> {
        let Ok(range) = self.patch_format_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn cff_charstrings_offset(&self) -> Result<Option<u32>, Never> {
        let Ok(range) = self.cff_charstrings_offset_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn cff2_charstrings_offset(&self) -> Result<Option<u32>, Never> {
        let Ok(range) = self.cff2_charstrings_offset_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn _reserved_0_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn _reserved_1_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self._reserved_0_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn _reserved_2_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self._reserved_1_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn field_flags_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self._reserved_2_byte_range();
        let start = previous.end;
        let end = start.saturating_add(PatchMapFieldPresenceFlags::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn compatibility_id_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.field_flags_byte_range();
        let start = previous.end;
        let end = start.saturating_add(CompatibilityId::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn max_entry_index_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.compatibility_id_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn max_glyph_map_entry_index_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.max_entry_index_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.max_glyph_map_entry_index_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Uint24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_map_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.glyph_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn feature_map_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.glyph_map_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn applied_entries_bitmap_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(max_entry_index) = self.max_entry_index();
        let Ok(previous) = self.feature_map_offset_byte_range();
        let start = previous.end;
        let Ok(value_7) = transforms::max_value_bitmap_len(max_entry_index);

        let end = start.saturating_add(value_7.saturating_mul(u8::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn url_template_length_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.applied_entries_bitmap_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn url_template_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(url_template_length) = self.url_template_length();
        let Ok(previous) = self.url_template_length_byte_range();
        let start = previous.end;
        let Ok(value_8) = transforms::to_u64(url_template_length);

        let end =
            start.saturating_add(value_8.saturating_mul(u8::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn patch_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.url_template_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn cff_charstrings_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.patch_format_byte_range();
        let start = previous.end;
        let Ok(field_flags) = self.field_flags();
        let Ok(presence) = field_flags.contains(PatchMapFieldPresenceFlags::CFF_CHARSTRINGS_OFFSET);
        let end = match presence {
            Presence::Present => {
                start.saturating_add(u32::RAW_BYTE_LEN)
            }
            Presence::Absent => start,
        };
        Ok(start..end)
    }

    pub fn cff2_charstrings_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.cff_charstrings_offset_byte_range();
        let start = previous.end;
        let Ok(field_flags) = self.field_flags();
        let Ok(presence) = field_flags.contains(PatchMapFieldPresenceFlags::CFF2_CHARSTRINGS_OFFSET);
        let end = match presence {
            Presence::Present => {
                start.saturating_add(u32::RAW_BYTE_LEN)
            }
            Presence::Absent => start,
        };
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    PatchMapFormat1::MIN_SIZE
); held }));

impl Default for PatchMapFormat1<'_> {
    fn default() -> Self {
        let Ok(value_9) = FontData::default_format_1_u8_table_data();

        Self {
            data: value_9,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PatchMapFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PatchMapFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "field_flags", value: FieldType::from(self.field_flags()) }),
            2u64 => Some(Field { name: "compatibility_id", value: traversal::FieldType::Unknown }),
            3u64 => Some(Field { name: "max_entry_index", value: FieldType::from(self.max_entry_index()) }),
            4u64 => Some(Field { name: "max_glyph_map_entry_index", value: FieldType::from(self.max_glyph_map_entry_index()) }),
            5u64 => Some(Field { name: "glyph_count", value: FieldType::from(self.glyph_count()) }),
            6u64 => Some(Field { name: "glyph_map_offset", value: FieldType::from(FieldType::offset(self.glyph_map_offset(), (self.glyph_map()).map(Some))) }),
            7u64 => Some(Field { name: "feature_map_offset", value: FieldType::from(FieldType::offset(self.feature_map_offset(), self.feature_map())) }),
            8u64 => Some(Field { name: "applied_entries_bitmap", value: FieldType::from(self.applied_entries_bitmap()) }),
            9u64 => Some(Field { name: "url_template_length", value: FieldType::from(self.url_template_length()) }),
            10u64 => Some(Field { name: "url_template", value: FieldType::from(self.url_template()) }),
            11u64 => Some(Field { name: "patch_format", value: FieldType::from(self.patch_format()) }),
            12u64 => {
                let Ok(field_flags) = self.field_flags();
                let Ok(presence) = field_flags.contains(PatchMapFieldPresenceFlags::CFF_CHARSTRINGS_OFFSET);
                let Ok(cff_charstrings_offset) = self.cff_charstrings_offset();

                match presence {
                    Presence::Present => cff_charstrings_offset.map(|cff_charstrings_offset_2| Field { name: "cff_charstrings_offset", value: FieldType::from(cff_charstrings_offset_2) }),
                    Presence::Absent => None,
                }
            },
            13u64 => {
                let Ok(field_flags) = self.field_flags();
                let Ok(presence) = field_flags.contains(PatchMapFieldPresenceFlags::CFF2_CHARSTRINGS_OFFSET);
                let Ok(cff2_charstrings_offset) = self.cff2_charstrings_offset();

                match presence {
                    Presence::Present => cff2_charstrings_offset.map(|cff2_charstrings_offset_2| Field { name: "cff2_charstrings_offset", value: FieldType::from(cff2_charstrings_offset_2) }),
                    Presence::Absent => None,
                }
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PatchMapFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for GlyphMap<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(entry_index_byte_range) = self.entry_index_byte_range();

        Ok(0..entry_index_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl ReadArgs for GlyphMap<'_> {
    type Args = (Uint24, u16);
}

impl<'a> FontReadWithArgs<'a> for GlyphMap<'a> {
    fn read_with_args(data: FontData<'a>, args: &(Uint24, u16)) -> Result<Self, ReadError> {
        let (glyph_count, max_entry_index) = *args;

        let Ok(value_10) = data.len();

        match ::core::primitive::bool::then_some(value_10 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self {
            data,
            glyph_count,
            max_entry_index,
        })
    }
}

impl<'a> GlyphMap<'a> {
    pub fn read(
        data: FontData<'a>,
        glyph_count: Uint24,
        max_entry_index: u16,
    ) -> Result<Self, ReadError> {
        let args = (glyph_count, max_entry_index);
        Self::read_with_args(data, &args)
    }
}

#[derive(Clone)]
pub struct GlyphMap<'a> {
    data: FontData<'a>,
    glyph_count: Uint24,
    max_entry_index: u16,
}

impl<'a> GlyphMap<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn first_mapped_glyph(&self) -> Result<u16, Never> {
        let Ok(range) = self.first_mapped_glyph_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn entry_index(&self) -> Result<ComputedArray<'a, U8Or16>, Never> {
        let Ok(range) = self.entry_index_byte_range();

        self.data.read_with_args_or_default(range, &({ let Ok(explicit_value) = self.max_entry_index(); explicit_value }))
    }

    pub(crate) fn glyph_count(&self) -> Result<Uint24, Never> {
        Ok(self.glyph_count)
    }

    pub(crate) fn max_entry_index(&self) -> Result<u16, Never> {
        Ok(self.max_entry_index)
    }

    pub fn first_mapped_glyph_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn entry_index_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(own_glyph_count) = self.glyph_count();
        let Ok(glyph_count) = own_glyph_count.to_u32();
        let Ok(first_mapped_glyph) = self.first_mapped_glyph();
        let Ok(previous) = self.first_mapped_glyph_byte_range();
        let start = previous.end;
        let Ok(value_11) = transforms::subtract(glyph_count, first_mapped_glyph);

        let Ok(max_entry_index) = self.max_entry_index();
        let Ok(size_or_zero) = transforms::size_or_zero(<U8Or16 as ComputeSize>::compute_size(&max_entry_index));
        let end = start.saturating_add(value_11.saturating_mul(
                size_or_zero,
            ));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(GlyphMap::MIN_SIZE); held }));

impl Default for GlyphMap<'_> {
    fn default() -> Self {
        let Ok(value_12) = FontData::default_table_data();

        Self {
            data: value_12,
            glyph_count: Default::default(),
            max_entry_index: Default::default(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for GlyphMap<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("GlyphMap")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "first_mapped_glyph", value: FieldType::from(self.first_mapped_glyph()) }),
            1u64 => Some(Field { name: "entry_index", value: traversal::FieldType::Unknown }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for GlyphMap<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for FeatureMap<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(entry_map_data_byte_range) = self.entry_map_data_byte_range();

        Ok(0..entry_map_data_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl ReadArgs for FeatureMap<'_> {
    type Args = u16;
}

impl<'a> FontReadWithArgs<'a> for FeatureMap<'a> {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let max_entry_index = *args;

        let Ok(value_13) = data.len();

        match ::core::primitive::bool::then_some(value_13 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self {
            data,
            max_entry_index,
        })
    }
}

impl<'a> FeatureMap<'a> {
    pub fn read(data: FontData<'a>, max_entry_index: u16) -> Result<Self, ReadError> {
        let args = max_entry_index;
        Self::read_with_args(data, &args)
    }
}

#[derive(Clone)]
pub struct FeatureMap<'a> {
    data: FontData<'a>,
    max_entry_index: u16,
}

impl<'a> FeatureMap<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn feature_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.feature_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn feature_records(&self) -> Result<ComputedArray<'a, FeatureRecord>, Never> {
        let Ok(range) = self.feature_records_byte_range();

        self.data.read_with_args_or_default(range, &({ let Ok(explicit_value) = self.max_entry_index(); explicit_value }))
    }

    pub fn entry_map_data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.entry_map_data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub(crate) fn max_entry_index(&self) -> Result<u16, Never> {
        Ok(self.max_entry_index)
    }

    pub fn feature_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn feature_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(feature_count) = self.feature_count();
        let Ok(previous) = self.feature_count_byte_range();
        let start = previous.end;
        let Ok(value_14) = transforms::to_u64(feature_count);

        let Ok(max_entry_index) = self.max_entry_index();
        let Ok(size_or_zero) = transforms::size_or_zero(<FeatureRecord as ComputeSize>::compute_size(&max_entry_index));
        let end = start.saturating_add(value_14.saturating_mul(
                size_or_zero,
            ));
        Ok(start..end)
    }

    pub fn entry_map_data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.feature_records_byte_range();
        let start = previous.end;
        let Ok(length) = self.data.len();
        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: length.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(FeatureMap::MIN_SIZE); held }));

impl Default for FeatureMap<'_> {
    fn default() -> Self {
        let Ok(value_15) = FontData::default_table_data();

        Self {
            data: value_15,
            max_entry_index: Default::default(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for FeatureMap<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("FeatureMap")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "feature_count", value: FieldType::from(self.feature_count()) }),
            1u64 => Some(Field { name: "feature_records", value: traversal::FieldType::Unknown }),
            2u64 => Some(Field { name: "entry_map_data", value: FieldType::from(self.entry_map_data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for FeatureMap<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug)]
pub struct FeatureRecord {
    pub feature_tag: BigEndian<Tag>,
    pub first_new_entry_index: U8Or16,
    pub entry_map_count: U8Or16,
}

impl FeatureRecord {
    pub fn feature_tag(&self) -> Result<Tag, Never> {
        self.feature_tag.get()
    }

    pub fn first_new_entry_index(&self) -> Result<&U8Or16, Never> {
        Ok(&self.first_new_entry_index)
    }

    pub fn entry_map_count(&self) -> Result<&U8Or16, Never> {
        Ok(&self.entry_map_count)
    }
}

impl ReadArgs for FeatureRecord {
    type Args = u16;
}

impl ComputeSize for FeatureRecord {
  fn compute_size(args: &u16) -> Result<u64, ReadError> {
        let max_entry_index = *args;
    let mut result = 0u64;
    let checked_value = result
            .checked_add(Tag::RAW_BYTE_LEN)
            .ok_or(ReadError::OutOfBounds)?;
    result = checked_value;
    let checked_value_2 = result
            .checked_add(match <U8Or16 as ComputeSize>::compute_size(&max_entry_index) {
                Ok(width) => width,
                Err(_invalid_patch_entry_width) => 0,
            })
            .ok_or(ReadError::OutOfBounds)?;
    result = checked_value_2;
    let checked_value_3 = result
            .checked_add(match <U8Or16 as ComputeSize>::compute_size(&max_entry_index) {
                Ok(width) => width,
                Err(_invalid_patch_entry_width) => 0,
            })
            .ok_or(ReadError::OutOfBounds)?;
    result = checked_value_3;
        Ok(result)
    }
}

impl<'a> FontReadWithArgs<'a> for FeatureRecord {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let Ok(value_17) = data.cursor();

        let mut cursor = value_17;
        let max_entry_index = *args;
        let feature_tag = cursor.read_be()?;
        let first_new_entry_index = cursor.read_with_args(&max_entry_index)?;
        let entry_map_count = cursor.read_with_args(&max_entry_index)?;

        Ok(Self {
            feature_tag,
            first_new_entry_index,
            entry_map_count,
        })
    }
}

impl<'a> FeatureRecord {
    pub fn read(data: FontData<'a>, max_entry_index: u16) -> Result<Self, ReadError> {
        let args = max_entry_index;
        Self::read_with_args(data, &args)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for FeatureRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "FeatureRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "feature_tag", value: FieldType::from(self.feature_tag()) }),
                1u64 => Some(Field { name: "first_new_entry_index", value: traversal::FieldType::Unknown }),
                2u64 => Some(Field { name: "entry_map_count", value: traversal::FieldType::Unknown }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Debug)]
pub struct EntryMapRecord {
    pub first_entry_index: U8Or16,
    pub last_entry_index: U8Or16,
}

impl EntryMapRecord {
    pub fn first_entry_index(&self) -> Result<&U8Or16, Never> {
        Ok(&self.first_entry_index)
    }

    pub fn last_entry_index(&self) -> Result<&U8Or16, Never> {
        Ok(&self.last_entry_index)
    }
}

impl ReadArgs for EntryMapRecord {
    type Args = u16;
}

impl ComputeSize for EntryMapRecord {
  fn compute_size(args: &u16) -> Result<u64, ReadError> {
        let max_entry_index = *args;
    let mut result = 0u64;
    let checked_value = result
            .checked_add(match <U8Or16 as ComputeSize>::compute_size(&max_entry_index) {
                Ok(width) => width,
                Err(_invalid_patch_entry_width) => 0,
            })
            .ok_or(ReadError::OutOfBounds)?;
    result = checked_value;
    let checked_value_2 = result
            .checked_add(match <U8Or16 as ComputeSize>::compute_size(&max_entry_index) {
                Ok(width) => width,
                Err(_invalid_patch_entry_width) => 0,
            })
            .ok_or(ReadError::OutOfBounds)?;
    result = checked_value_2;
        Ok(result)
    }
}

impl<'a> FontReadWithArgs<'a> for EntryMapRecord {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let Ok(value_18) = data.cursor();

        let mut cursor = value_18;
        let max_entry_index = *args;
        let first_entry_index = cursor.read_with_args(&max_entry_index)?;
        let last_entry_index = cursor.read_with_args(&max_entry_index)?;

        Ok(Self {
            first_entry_index,
            last_entry_index,
        })
    }
}

impl<'a> EntryMapRecord {
    pub fn read(data: FontData<'a>, max_entry_index: u16) -> Result<Self, ReadError> {
        let args = max_entry_index;
        Self::read_with_args(data, &args)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for EntryMapRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "EntryMapRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "first_entry_index", value: traversal::FieldType::Unknown }),
                1u64 => Some(Field { name: "last_entry_index", value: traversal::FieldType::Unknown }),
                _ => None,
            }),
            data,
        })
    }
}

impl Format<u8> for PatchMapFormat2<'_> {
    const FORMAT: u8 = 2;
}

impl<'a> MinByteRange<'a> for PatchMapFormat2<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(url_template_byte_range) = self.url_template_byte_range();

        Ok(0..url_template_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PatchMapFormat2<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_19) = data.len();

        match ::core::primitive::bool::then_some(value_19 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PatchMapFormat2<'a> {
    data: FontData<'a>,
}

impl<'a> PatchMapFormat2<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN
        + PatchMapFieldPresenceFlags::RAW_BYTE_LEN
        + CompatibilityId::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN
        + Uint24::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn field_flags(&self) -> Result<PatchMapFieldPresenceFlags, Never> {
        let Ok(range) = self.field_flags_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn compatibility_id(&self) -> Result<CompatibilityId, Never> {
        let Ok(range) = self.compatibility_id_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn default_patch_format(&self) -> Result<u8, Never> {
        let Ok(range) = self.default_patch_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn entry_count(&self) -> Result<Uint24, Never> {
        let Ok(range) = self.entry_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn entries_offset(&self) -> Result<Offset32, Never> {
        let Ok(range) = self.entries_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn entries(&self) -> Result<MappingEntries<'a>, ReadError> {
        let data = self.data;
        let Ok(entries_offset) = self.entries_offset();

        entries_offset.resolve(data)
    }

    pub fn entry_id_string_data_offset(&self) -> Result<Nullable<Offset32>, Never> {
        let Ok(range) = self.entry_id_string_data_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn entry_id_string_data(&self) -> Result<Option<IdStringData<'a>>, ReadError> {
        let data = self.data;
        let Ok(entry_id_string_data_offset) = self.entry_id_string_data_offset();

        entry_id_string_data_offset.resolve(data)
    }

    pub fn url_template_length(&self) -> Result<u16, Never> {
        let Ok(range) = self.url_template_length_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn url_template(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.url_template_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn cff_charstrings_offset(&self) -> Result<Option<u32>, Never> {
        let Ok(range) = self.cff_charstrings_offset_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn cff2_charstrings_offset(&self) -> Result<Option<u32>, Never> {
        let Ok(range) = self.cff2_charstrings_offset_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn _reserved_0_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn _reserved_1_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self._reserved_0_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn _reserved_2_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self._reserved_1_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn field_flags_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self._reserved_2_byte_range();
        let start = previous.end;
        let end = start.saturating_add(PatchMapFieldPresenceFlags::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn compatibility_id_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.field_flags_byte_range();
        let start = previous.end;
        let end = start.saturating_add(CompatibilityId::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn default_patch_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.compatibility_id_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn entry_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.default_patch_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Uint24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn entries_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.entry_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn entry_id_string_data_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.entries_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn url_template_length_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.entry_id_string_data_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn url_template_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(url_template_length) = self.url_template_length();
        let Ok(previous) = self.url_template_length_byte_range();
        let start = previous.end;
        let Ok(value_20) = transforms::to_u64(url_template_length);

        let end =
            start.saturating_add(value_20.saturating_mul(u8::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn cff_charstrings_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.url_template_byte_range();
        let start = previous.end;
        let Ok(field_flags) = self.field_flags();
        let Ok(presence) = field_flags.contains(PatchMapFieldPresenceFlags::CFF_CHARSTRINGS_OFFSET);
        let end = match presence {
            Presence::Present => {
                start.saturating_add(u32::RAW_BYTE_LEN)
            }
            Presence::Absent => start,
        };
        Ok(start..end)
    }

    pub fn cff2_charstrings_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.cff_charstrings_offset_byte_range();
        let start = previous.end;
        let Ok(field_flags) = self.field_flags();
        let Ok(presence) = field_flags.contains(PatchMapFieldPresenceFlags::CFF2_CHARSTRINGS_OFFSET);
        let end = match presence {
            Presence::Present => {
                start.saturating_add(u32::RAW_BYTE_LEN)
            }
            Presence::Absent => start,
        };
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PatchMapFormat2<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PatchMapFormat2")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "field_flags", value: FieldType::from(self.field_flags()) }),
            2u64 => Some(Field { name: "compatibility_id", value: traversal::FieldType::Unknown }),
            3u64 => Some(Field { name: "default_patch_format", value: FieldType::from(self.default_patch_format()) }),
            4u64 => Some(Field { name: "entry_count", value: FieldType::from(self.entry_count()) }),
            5u64 => Some(Field { name: "entries_offset", value: FieldType::from(FieldType::offset(self.entries_offset(), (self.entries()).map(Some))) }),
            6u64 => Some(Field { name: "entry_id_string_data_offset", value: FieldType::from(FieldType::offset(
                    self.entry_id_string_data_offset(),
                    self.entry_id_string_data(),
                )) }),
            7u64 => Some(Field { name: "url_template_length", value: FieldType::from(self.url_template_length()) }),
            8u64 => Some(Field { name: "url_template", value: FieldType::from(self.url_template()) }),
            9u64 => {
                let Ok(field_flags) = self.field_flags();
                let Ok(presence) = field_flags.contains(PatchMapFieldPresenceFlags::CFF_CHARSTRINGS_OFFSET);
                let Ok(cff_charstrings_offset) = self.cff_charstrings_offset();

                match presence {
                    Presence::Present => cff_charstrings_offset.map(|cff_charstrings_offset_2| Field { name: "cff_charstrings_offset", value: FieldType::from(cff_charstrings_offset_2) }),
                    Presence::Absent => None,
                }
            },
            10u64 => {
                let Ok(field_flags) = self.field_flags();
                let Ok(presence) = field_flags.contains(PatchMapFieldPresenceFlags::CFF2_CHARSTRINGS_OFFSET);
                let Ok(cff2_charstrings_offset) = self.cff2_charstrings_offset();

                match presence {
                    Presence::Present => cff2_charstrings_offset.map(|cff2_charstrings_offset_2| Field { name: "cff2_charstrings_offset", value: FieldType::from(cff2_charstrings_offset_2) }),
                    Presence::Absent => None,
                }
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PatchMapFormat2<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for MappingEntries<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(entry_data_byte_range) = self.entry_data_byte_range();

        Ok(0..entry_data_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for MappingEntries<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct MappingEntries<'a> {
    data: FontData<'a>,
}

impl<'a> MappingEntries<'a> {
    pub const MIN_SIZE: u64 = 0;
    basic_table_impls!(impl_the_methods);

    pub fn entry_data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.entry_data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn entry_data_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let Ok(length) = self.data.len();
        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: length.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(MappingEntries::MIN_SIZE); held }));

impl Default for MappingEntries<'_> {
    fn default() -> Self {
        let Ok(value_22) = FontData::default_table_data();

        Self {
            data: value_22,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for MappingEntries<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("MappingEntries")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "entry_data", value: FieldType::from(self.entry_data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for MappingEntries<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for EntryData<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(trailing_data_byte_range) = self.trailing_data_byte_range();

        Ok(0..trailing_data_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for EntryData<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_23) = data.len();

        match ::core::primitive::bool::then_some(value_23 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct EntryData<'a> {
    data: FontData<'a>,
}

impl<'a> EntryData<'a> {
    pub const MIN_SIZE: u64 = EntryFormatFlags::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn format_flags(&self) -> Result<EntryFormatFlags, Never> {
        let Ok(range) = self.format_flags_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn feature_count(&self) -> Result<Option<u8>, Never> {
        let Ok(range) = self.feature_count_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn feature_tags(&self) -> Result<Option<&'a [BigEndian<Tag>]>, Never> {
        let Ok(range) = self.feature_tags_byte_range();

        self.data.read_array_if_present(range)
    }

    pub fn design_space_count(&self) -> Result<Option<u16>, Never> {
        let Ok(range) = self.design_space_count_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn design_space_segments(&self) -> Result<Option<&'a [DesignSpaceSegment]>, Never> {
        let Ok(range) = self.design_space_segments_byte_range();

        self.data.read_array_if_present(range)
    }

    pub fn match_mode_and_count(&self) -> Result<Option<MatchModeAndCount>, Never> {
        let Ok(range) = self.match_mode_and_count_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn child_indices(&self) -> Result<Option<&'a [BigEndian<Uint24>]>, Never> {
        let Ok(range) = self.child_indices_byte_range();

        self.data.read_array_if_present(range)
    }

    pub fn format_flags_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(EntryFormatFlags::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn feature_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_flags_byte_range();
        let start = previous.end;
        let Ok(format_flags) = self.format_flags();
        let Ok(presence) = format_flags.contains(EntryFormatFlags::FEATURES_AND_DESIGN_SPACE);
        let end = match presence {
            Presence::Present => {
                start.saturating_add(u8::RAW_BYTE_LEN)
            }
            Presence::Absent => start,
        };
        Ok(start..end)
    }

    pub fn feature_tags_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(own_feature_count) = self.feature_count();
        let feature_count = match own_feature_count {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => 0,
        };
        let Ok(previous) = self.feature_count_byte_range();
        let start = previous.end;
        let Ok(format_flags) = self.format_flags();
        let Ok(presence) = format_flags.contains(EntryFormatFlags::FEATURES_AND_DESIGN_SPACE);
        let end = match presence {
            Presence::Present => {
                let Ok(value_24) = transforms::to_u64(feature_count);

            start.saturating_add(value_24.saturating_mul(Tag::RAW_BYTE_LEN))
            }
            Presence::Absent => start,
        };
        Ok(start..end)
    }

    pub fn design_space_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.feature_tags_byte_range();
        let start = previous.end;
        let Ok(format_flags) = self.format_flags();
        let Ok(presence) = format_flags.contains(EntryFormatFlags::FEATURES_AND_DESIGN_SPACE);
        let end = match presence {
            Presence::Present => {
                start.saturating_add(u16::RAW_BYTE_LEN)
            }
            Presence::Absent => start,
        };
        Ok(start..end)
    }

    pub fn design_space_segments_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(own_design_space_count) = self.design_space_count();
        let design_space_count = match own_design_space_count {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => 0,
        };
        let Ok(previous) = self.design_space_count_byte_range();
        let start = previous.end;
        let Ok(format_flags) = self.format_flags();
        let Ok(presence) = format_flags.contains(EntryFormatFlags::FEATURES_AND_DESIGN_SPACE);
        let end = match presence {
            Presence::Present => {
                let Ok(value_25) = transforms::to_u64(design_space_count);

            start.saturating_add(value_25
                    .saturating_mul(DesignSpaceSegment::RAW_BYTE_LEN))
            }
            Presence::Absent => start,
        };
        Ok(start..end)
    }

    pub fn match_mode_and_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.design_space_segments_byte_range();
        let start = previous.end;
        let Ok(format_flags) = self.format_flags();
        let Ok(presence) = format_flags.contains(EntryFormatFlags::CHILD_INDICES);
        let end = match presence {
            Presence::Present => {
                start.saturating_add(MatchModeAndCount::RAW_BYTE_LEN)
            }
            Presence::Absent => start,
        };
        Ok(start..end)
    }

    pub fn child_indices_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(own_match_mode_and_count) = self.match_mode_and_count();
        let match_mode_and_count = match own_match_mode_and_count {
            Some(mode_and_count) => mode_and_count,
            None => MatchModeAndCount::default(),
        };
        let Ok(previous) = self.match_mode_and_count_byte_range();
        let start = previous.end;
        let Ok(format_flags) = self.format_flags();
        let Ok(presence) = format_flags.contains(EntryFormatFlags::CHILD_INDICES);
        let end = match presence {
            Presence::Present => {
                let Ok(value_26) = transforms::to_u64(match_mode_and_count.count());

            start.saturating_add(value_26.saturating_mul(Uint24::RAW_BYTE_LEN))
            }
            Presence::Absent => start,
        };
        Ok(start..end)
    }

    pub fn trailing_data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.child_indices_byte_range();
        let start = previous.end;
        let Ok(length) = self.data.len();
        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: length.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(EntryData::MIN_SIZE); held }));

impl Default for EntryData<'_> {
    fn default() -> Self {
        let Ok(value_27) = FontData::default_table_data();

        Self {
            data: value_27,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for EntryData<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("EntryData")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format_flags", value: FieldType::from(self.format_flags()) }),
            1u64 => {
                let Ok(format_flags) = self.format_flags();
                let Ok(presence) = format_flags.contains(EntryFormatFlags::FEATURES_AND_DESIGN_SPACE);
                let Ok(feature_count) = self.feature_count();

                match presence {
                    Presence::Present => feature_count.map(|feature_count_2| Field { name: "feature_count", value: FieldType::from(feature_count_2) }),
                    Presence::Absent => None,
                }
            },
            2u64 => {
                let Ok(format_flags) = self.format_flags();
                let Ok(presence) = format_flags.contains(EntryFormatFlags::FEATURES_AND_DESIGN_SPACE);
                let Ok(feature_tags) = self.feature_tags();

                match presence {
                    Presence::Present => feature_tags.map(|feature_tags_2| Field { name: "feature_tags", value: FieldType::from(feature_tags_2) }),
                    Presence::Absent => None,
                }
            },
            3u64 => {
                let Ok(format_flags) = self.format_flags();
                let Ok(presence) = format_flags.contains(EntryFormatFlags::FEATURES_AND_DESIGN_SPACE);
                let Ok(design_space_count) = self.design_space_count();

                match presence {
                    Presence::Present => design_space_count.map(|design_space_count_2| Field { name: "design_space_count", value: FieldType::from(design_space_count_2) }),
                    Presence::Absent => None,
                }
            },
            4u64 => {
                let Ok(format_flags) = self.format_flags();
                let Ok(presence) = format_flags.contains(EntryFormatFlags::FEATURES_AND_DESIGN_SPACE);
                let Ok(value_28) = self.offset_data();
                let Ok(design_space_segments) = self.design_space_segments();

                match presence {
                    Presence::Present => design_space_segments.map(|design_space_segments_2| Field { name: "design_space_segments", value: FieldType::from(traversal::FieldType::array_of_records(
                            stringify!(DesignSpaceSegment),
                            design_space_segments_2,
                            value_28,
                        )) }),
                    Presence::Absent => None,
                }
            },
            5u64 => {
                let Ok(format_flags) = self.format_flags();
                let Ok(presence) = format_flags.contains(EntryFormatFlags::CHILD_INDICES);

                match presence {
                    Presence::Present => {
                    Some(Field { name: "match_mode_and_count", value: traversal::FieldType::Unknown })
                },
                    Presence::Absent => None,
                }
            },
            6u64 => {
                let Ok(format_flags) = self.format_flags();
                let Ok(presence) = format_flags.contains(EntryFormatFlags::CHILD_INDICES);
                let Ok(child_indices) = self.child_indices();

                match presence {
                    Presence::Present => child_indices.map(|child_indices_2| Field { name: "child_indices", value: FieldType::from(child_indices_2) }),
                    Presence::Absent => None,
                }
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for EntryData<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck :: AnyBitPattern)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct EntryFormatFlags {
    bits: u8,
}

impl EntryFormatFlags {
    pub const FEATURES_AND_DESIGN_SPACE: Self = Self { bits: 0b00000001 };

    pub const CHILD_INDICES: Self = Self { bits: 0b00000010 };

    pub const ENTRY_ID_DELTA: Self = Self { bits: 0b00000100 };

    pub const PATCH_FORMAT: Self = Self { bits: 0b00001000 };

    pub const CODEPOINTS_BIT_1: Self = Self { bits: 0b00010000 };

    pub const CODEPOINTS_BIT_2: Self = Self { bits: 0b00100000 };

    pub const IGNORED: Self = Self { bits: 0b01000000 };

    pub const RESERVED: Self = Self { bits: 0b10000000 };
}

impl EntryFormatFlags {
    const KNOWN: u8 = Self::FEATURES_AND_DESIGN_SPACE.bits
        | Self::CHILD_INDICES.bits
        | Self::ENTRY_ID_DELTA.bits
        | Self::PATCH_FORMAT.bits
        | Self::CODEPOINTS_BIT_1.bits
        | Self::CODEPOINTS_BIT_2.bits
        | Self::IGNORED.bits
        | Self::RESERVED.bits;

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

impl std::ops::BitOr for EntryFormatFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, other: EntryFormatFlags) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl std::ops::BitOrAssign for EntryFormatFlags {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.bits |= other.bits;
    }
}

impl std::ops::BitXor for EntryFormatFlags {
    type Output = Self;

    #[inline]
    fn bitxor(self, other: Self) -> Self {
        Self {
            bits: self.bits ^ other.bits,
        }
    }
}

impl std::ops::BitXorAssign for EntryFormatFlags {
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        self.bits ^= other.bits;
    }
}

impl std::ops::BitAnd for EntryFormatFlags {
    type Output = Self;

    #[inline]
    fn bitand(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }
}

impl std::ops::BitAndAssign for EntryFormatFlags {
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        self.bits &= other.bits;
    }
}

impl std::ops::Sub for EntryFormatFlags {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }
}

impl std::ops::SubAssign for EntryFormatFlags {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.bits &= !other.bits;
    }
}

impl std::ops::Not for EntryFormatFlags {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        Self { bits: !self.bits & Self::KNOWN }
    }
}

impl std::fmt::Debug for EntryFormatFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let members: &[(&str, Self)] = &[
            ("FEATURES_AND_DESIGN_SPACE", Self::FEATURES_AND_DESIGN_SPACE),
            ("CHILD_INDICES", Self::CHILD_INDICES),
            ("ENTRY_ID_DELTA", Self::ENTRY_ID_DELTA),
            ("PATCH_FORMAT", Self::PATCH_FORMAT),
            ("CODEPOINTS_BIT_1", Self::CODEPOINTS_BIT_1),
            ("CODEPOINTS_BIT_2", Self::CODEPOINTS_BIT_2),
            ("IGNORED", Self::IGNORED),
            ("RESERVED", Self::RESERVED),
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

impl std::fmt::Binary for EntryFormatFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Binary::fmt(&self.bits, f)
    }
}

impl std::fmt::Octal for EntryFormatFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Octal::fmt(&self.bits, f)
    }
}

impl std::fmt::LowerHex for EntryFormatFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.bits, f)
    }
}

impl std::fmt::UpperHex for EntryFormatFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.bits, f)
    }
}

impl font_types::Scalar for EntryFormatFlags {
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
impl<'a> From<EntryFormatFlags> for FieldType<'a> {
    fn from(src: EntryFormatFlags) -> FieldType<'a> {
        <crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u8>>::from(src.bits)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct DesignSpaceSegment {
    pub axis_tag: BigEndian<Tag>,
    pub start: BigEndian<Fixed>,
    pub end: BigEndian<Fixed>,
}

const _: () = {
  assert!(core::mem::size_of::<DesignSpaceSegment>() == 12);
  assert!(core::mem::align_of::<DesignSpaceSegment>() == 1);
  assert!(core::mem::offset_of!(DesignSpaceSegment, axis_tag) == 0);
  assert!(core::mem::offset_of!(DesignSpaceSegment, start) == 4);
  assert!(core::mem::offset_of!(DesignSpaceSegment, end) == 8);
};

impl DesignSpaceSegment {
    pub fn axis_tag(&self) -> Result<Tag, Never> {
        self.axis_tag.get()
    }

    pub fn start(&self) -> Result<Fixed, Never> {
        self.start.get()
    }

    pub fn end(&self) -> Result<Fixed, Never> {
        self.end.get()
    }
}

impl FixedSize for DesignSpaceSegment {
    const RAW_BYTE_LEN: u64 = Tag::RAW_BYTE_LEN + Fixed::RAW_BYTE_LEN + Fixed::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for DesignSpaceSegment {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "DesignSpaceSegment",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "axis_tag", value: FieldType::from(self.axis_tag()) }),
                1u64 => Some(Field { name: "start", value: FieldType::from(self.start()) }),
                2u64 => Some(Field { name: "end", value: FieldType::from(self.end()) }),
                _ => None,
            }),
            data,
        })
    }
}

impl<'a> MinByteRange<'a> for IdStringData<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(id_data_byte_range) = self.id_data_byte_range();

        Ok(0..id_data_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for IdStringData<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct IdStringData<'a> {
    data: FontData<'a>,
}

impl<'a> IdStringData<'a> {
    pub const MIN_SIZE: u64 = 0;
    basic_table_impls!(impl_the_methods);

    pub fn id_data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.id_data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn id_data_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let Ok(length) = self.data.len();
        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: length.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(IdStringData::MIN_SIZE); held }));

impl Default for IdStringData<'_> {
    fn default() -> Self {
        let Ok(value_36) = FontData::default_table_data();

        Self {
            data: value_36,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for IdStringData<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("IdStringData")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "id_data", value: FieldType::from(self.id_data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for IdStringData<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for TableKeyedPatch<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(patch_offsets_byte_range) = self.patch_offsets_byte_range();

        Ok(0..patch_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for TableKeyedPatch<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_37) = data.len();

        match ::core::primitive::bool::then_some(value_37 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct TableKeyedPatch<'a> {
    data: FontData<'a>,
}

impl<'a> TableKeyedPatch<'a> {
    pub const MIN_SIZE: u64 =
        (Tag::RAW_BYTE_LEN + u32::RAW_BYTE_LEN + CompatibilityId::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<Tag, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn compatibility_id(&self) -> Result<CompatibilityId, Never> {
        let Ok(range) = self.compatibility_id_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn patches_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.patches_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn patch_offsets(&self) -> Result<&'a [BigEndian<Offset32>], Never> {
        let Ok(range) = self.patch_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn patches(&self) -> Result<ArrayOfOffsets<'a, TablePatch<'a>, Offset32>, Never> {
        let data = self.data;
        let Ok(offsets) = self.patch_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(Tag::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn _reserved_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn compatibility_id_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self._reserved_byte_range();
        let start = previous.end;
        let end = start.saturating_add(CompatibilityId::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn patches_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.compatibility_id_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn patch_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(patches_count) = self.patches_count();
        let Ok(previous) = self.patches_count_byte_range();
        let start = previous.end;
        let Ok(value_38) = transforms::add(patches_count, 1_u64);

        let end = start.saturating_add(value_38.saturating_mul(Offset32::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    TableKeyedPatch::MIN_SIZE
); held }));

impl Default for TableKeyedPatch<'_> {
    fn default() -> Self {
        let Ok(value_39) = FontData::default_table_data();

        Self {
            data: value_39,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for TableKeyedPatch<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("TableKeyedPatch")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "compatibility_id", value: traversal::FieldType::Unknown }),
            2u64 => Some(Field { name: "patches_count", value: FieldType::from(self.patches_count()) }),
            3u64 => Some(Field { name: "patch_offsets", value: FieldType::from(self.patches()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for TableKeyedPatch<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for TablePatch<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(brotli_stream_byte_range) = self.brotli_stream_byte_range();

        Ok(0..brotli_stream_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for TablePatch<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_40) = data.len();

        match ::core::primitive::bool::then_some(value_40 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct TablePatch<'a> {
    data: FontData<'a>,
}

impl<'a> TablePatch<'a> {
    pub const MIN_SIZE: u64 =
        (Tag::RAW_BYTE_LEN + TablePatchFlags::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn tag(&self) -> Result<Tag, Never> {
        let Ok(range) = self.tag_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn flags(&self) -> Result<TablePatchFlags, Never> {
        let Ok(range) = self.flags_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn max_uncompressed_length(&self) -> Result<u32, Never> {
        let Ok(range) = self.max_uncompressed_length_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn brotli_stream(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.brotli_stream_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn tag_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(Tag::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn flags_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.tag_byte_range();
        let start = previous.end;
        let end = start.saturating_add(TablePatchFlags::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn max_uncompressed_length_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.flags_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn brotli_stream_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.max_uncompressed_length_byte_range();
        let start = previous.end;
        let Ok(length) = self.data.len();
        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: length.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(TablePatch::MIN_SIZE); held }));

impl Default for TablePatch<'_> {
    fn default() -> Self {
        let Ok(value_41) = FontData::default_table_data();

        Self {
            data: value_41,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for TablePatch<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("TablePatch")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "tag", value: FieldType::from(self.tag()) }),
            1u64 => Some(Field { name: "flags", value: FieldType::from(self.flags()) }),
            2u64 => Some(Field { name: "max_uncompressed_length", value: FieldType::from(self.max_uncompressed_length()) }),
            3u64 => Some(Field { name: "brotli_stream", value: FieldType::from(self.brotli_stream()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for TablePatch<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck :: AnyBitPattern)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct TablePatchFlags {
    bits: u8,
}

impl TablePatchFlags {
    pub const REPLACE_TABLE: Self = Self { bits: 0b01 };

    pub const DROP_TABLE: Self = Self { bits: 0b10 };
}

impl TablePatchFlags {
    const KNOWN: u8 = Self::REPLACE_TABLE.bits
        | Self::DROP_TABLE.bits;

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

impl std::ops::BitOr for TablePatchFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, other: TablePatchFlags) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl std::ops::BitOrAssign for TablePatchFlags {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.bits |= other.bits;
    }
}

impl std::ops::BitXor for TablePatchFlags {
    type Output = Self;

    #[inline]
    fn bitxor(self, other: Self) -> Self {
        Self {
            bits: self.bits ^ other.bits,
        }
    }
}

impl std::ops::BitXorAssign for TablePatchFlags {
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        self.bits ^= other.bits;
    }
}

impl std::ops::BitAnd for TablePatchFlags {
    type Output = Self;

    #[inline]
    fn bitand(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }
}

impl std::ops::BitAndAssign for TablePatchFlags {
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        self.bits &= other.bits;
    }
}

impl std::ops::Sub for TablePatchFlags {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }
}

impl std::ops::SubAssign for TablePatchFlags {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.bits &= !other.bits;
    }
}

impl std::ops::Not for TablePatchFlags {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        Self { bits: !self.bits & Self::KNOWN }
    }
}

impl std::fmt::Debug for TablePatchFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let members: &[(&str, Self)] = &[
            ("REPLACE_TABLE", Self::REPLACE_TABLE),
            ("DROP_TABLE", Self::DROP_TABLE),
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

impl std::fmt::Binary for TablePatchFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Binary::fmt(&self.bits, f)
    }
}

impl std::fmt::Octal for TablePatchFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Octal::fmt(&self.bits, f)
    }
}

impl std::fmt::LowerHex for TablePatchFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.bits, f)
    }
}

impl std::fmt::UpperHex for TablePatchFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.bits, f)
    }
}

impl font_types::Scalar for TablePatchFlags {
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
impl<'a> From<TablePatchFlags> for FieldType<'a> {
    fn from(src: TablePatchFlags) -> FieldType<'a> {
        <crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u8>>::from(src.bits)
    }
}

impl<'a> MinByteRange<'a> for GlyphKeyedPatch<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(brotli_stream_byte_range) = self.brotli_stream_byte_range();

        Ok(0..brotli_stream_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for GlyphKeyedPatch<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_45) = data.len();

        match ::core::primitive::bool::then_some(value_45 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct GlyphKeyedPatch<'a> {
    data: FontData<'a>,
}

impl<'a> GlyphKeyedPatch<'a> {
    pub const MIN_SIZE: u64 = (Tag::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN
        + GlyphKeyedFlags::RAW_BYTE_LEN
        + CompatibilityId::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<Tag, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn flags(&self) -> Result<GlyphKeyedFlags, Never> {
        let Ok(range) = self.flags_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn compatibility_id(&self) -> Result<CompatibilityId, Never> {
        let Ok(range) = self.compatibility_id_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn max_uncompressed_length(&self) -> Result<u32, Never> {
        let Ok(range) = self.max_uncompressed_length_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn brotli_stream(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.brotli_stream_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(Tag::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn _reserved_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn flags_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self._reserved_byte_range();
        let start = previous.end;
        let end = start.saturating_add(GlyphKeyedFlags::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn compatibility_id_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.flags_byte_range();
        let start = previous.end;
        let end = start.saturating_add(CompatibilityId::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn max_uncompressed_length_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.compatibility_id_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn brotli_stream_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.max_uncompressed_length_byte_range();
        let start = previous.end;
        let Ok(length) = self.data.len();
        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: length.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    GlyphKeyedPatch::MIN_SIZE
); held }));

impl Default for GlyphKeyedPatch<'_> {
    fn default() -> Self {
        let Ok(value_46) = FontData::default_table_data();

        Self {
            data: value_46,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for GlyphKeyedPatch<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("GlyphKeyedPatch")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "flags", value: FieldType::from(self.flags()) }),
            2u64 => Some(Field { name: "compatibility_id", value: traversal::FieldType::Unknown }),
            3u64 => Some(Field { name: "max_uncompressed_length", value: FieldType::from(self.max_uncompressed_length()) }),
            4u64 => Some(Field { name: "brotli_stream", value: FieldType::from(self.brotli_stream()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for GlyphKeyedPatch<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck :: AnyBitPattern)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct GlyphKeyedFlags {
    bits: u8,
}

impl GlyphKeyedFlags {
    pub const NONE: Self = Self { bits: 0b0 };

    pub const WIDE_GLYPH_IDS: Self = Self { bits: 0b1 };
}

impl GlyphKeyedFlags {
    const KNOWN: u8 = Self::NONE.bits
        | Self::WIDE_GLYPH_IDS.bits;

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

impl std::ops::BitOr for GlyphKeyedFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, other: GlyphKeyedFlags) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl std::ops::BitOrAssign for GlyphKeyedFlags {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.bits |= other.bits;
    }
}

impl std::ops::BitXor for GlyphKeyedFlags {
    type Output = Self;

    #[inline]
    fn bitxor(self, other: Self) -> Self {
        Self {
            bits: self.bits ^ other.bits,
        }
    }
}

impl std::ops::BitXorAssign for GlyphKeyedFlags {
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        self.bits ^= other.bits;
    }
}

impl std::ops::BitAnd for GlyphKeyedFlags {
    type Output = Self;

    #[inline]
    fn bitand(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }
}

impl std::ops::BitAndAssign for GlyphKeyedFlags {
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        self.bits &= other.bits;
    }
}

impl std::ops::Sub for GlyphKeyedFlags {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }
}

impl std::ops::SubAssign for GlyphKeyedFlags {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.bits &= !other.bits;
    }
}

impl std::ops::Not for GlyphKeyedFlags {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        Self { bits: !self.bits & Self::KNOWN }
    }
}

impl std::fmt::Debug for GlyphKeyedFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let members: &[(&str, Self)] = &[
            ("NONE", Self::NONE),
            ("WIDE_GLYPH_IDS", Self::WIDE_GLYPH_IDS),
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

impl std::fmt::Binary for GlyphKeyedFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Binary::fmt(&self.bits, f)
    }
}

impl std::fmt::Octal for GlyphKeyedFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Octal::fmt(&self.bits, f)
    }
}

impl std::fmt::LowerHex for GlyphKeyedFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.bits, f)
    }
}

impl std::fmt::UpperHex for GlyphKeyedFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.bits, f)
    }
}

impl font_types::Scalar for GlyphKeyedFlags {
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
impl<'a> From<GlyphKeyedFlags> for FieldType<'a> {
    fn from(src: GlyphKeyedFlags) -> FieldType<'a> {
        <crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u8>>::from(src.bits)
    }
}

impl<'a> MinByteRange<'a> for GlyphPatches<'a> {
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

impl ReadArgs for GlyphPatches<'_> {
    type Args = GlyphKeyedFlags;
}

impl<'a> FontReadWithArgs<'a> for GlyphPatches<'a> {
    fn read_with_args(data: FontData<'a>, args: &GlyphKeyedFlags) -> Result<Self, ReadError> {
        let flags = *args;

        let Ok(value_50) = data.len();

        match ::core::primitive::bool::then_some(value_50 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data, flags })
    }
}

impl<'a> GlyphPatches<'a> {
    pub fn read(data: FontData<'a>, flags: GlyphKeyedFlags) -> Result<Self, ReadError> {
        let args = flags;
        Self::read_with_args(data, &args)
    }
}

#[derive(Clone)]
pub struct GlyphPatches<'a> {
    data: FontData<'a>,
    flags: GlyphKeyedFlags,
}

impl<'a> GlyphPatches<'a> {
    pub const MIN_SIZE: u64 = (u32::RAW_BYTE_LEN + u8::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn glyph_count(&self) -> Result<u32, Never> {
        let Ok(range) = self.glyph_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn table_count(&self) -> Result<u8, Never> {
        let Ok(range) = self.table_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn glyph_ids(&self) -> Result<ComputedArray<'a, U16Or24>, Never> {
        let Ok(range) = self.glyph_ids_byte_range();

        self.data.read_with_args_or_default(range, &({ let Ok(explicit_value) = self.flags(); explicit_value }))
    }

    pub fn tables(&self) -> Result<&'a [BigEndian<Tag>], Never> {
        let Ok(range) = self.tables_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn glyph_data_offsets(&self) -> Result<&'a [BigEndian<Offset32>], Never> {
        let Ok(range) = self.glyph_data_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn glyph_data(&self) -> Result<ArrayOfOffsets<'a, GlyphData<'a>, Offset32>, Never> {
        let data = self.data;
        let Ok(offsets) = self.glyph_data_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub(crate) fn flags(&self) -> Result<GlyphKeyedFlags, Never> {
        Ok(self.flags)
    }

    pub fn glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn table_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.glyph_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_ids_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(glyph_count) = self.glyph_count();
        let Ok(previous) = self.table_count_byte_range();
        let start = previous.end;
        let Ok(value_51) = transforms::to_u64(glyph_count);

        let Ok(flags) = self.flags();
        let Ok(size_or_zero) = transforms::size_or_zero(<U16Or24 as ComputeSize>::compute_size(&flags));
        let end = start.saturating_add(value_51
                .saturating_mul(size_or_zero));
        Ok(start..end)
    }

    pub fn tables_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(table_count) = self.table_count();
        let Ok(previous) = self.glyph_ids_byte_range();
        let start = previous.end;
        let Ok(value_52) = transforms::to_u64(table_count);

        let end = start.saturating_add(value_52.saturating_mul(Tag::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn glyph_data_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(glyph_count) = self.glyph_count();
        let Ok(table_count) = self.table_count();
        let Ok(previous) = self.tables_byte_range();
        let start = previous.end;
        let Ok(value_53) = transforms::multiply_add(glyph_count, table_count, 1_u64);

        let end = start.saturating_add(value_53
                .saturating_mul(Offset32::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(GlyphPatches::MIN_SIZE); held }));

impl Default for GlyphPatches<'_> {
    fn default() -> Self {
        let Ok(value_54) = FontData::default_table_data();

        Self {
            data: value_54,
            flags: Default::default(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for GlyphPatches<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("GlyphPatches")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "glyph_count", value: FieldType::from(self.glyph_count()) }),
            1u64 => Some(Field { name: "table_count", value: FieldType::from(self.table_count()) }),
            2u64 => Some(Field { name: "glyph_ids", value: traversal::FieldType::Unknown }),
            3u64 => Some(Field { name: "tables", value: FieldType::from(self.tables()) }),
            4u64 => Some(Field { name: "glyph_data_offsets", value: FieldType::from(self.glyph_data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for GlyphPatches<'a> {
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

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct GlyphData<'a> {
    data: FontData<'a>,
}

impl<'a> GlyphData<'a> {
    pub const MIN_SIZE: u64 = 0;
    basic_table_impls!(impl_the_methods);

    pub fn data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn data_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let Ok(length) = self.data.len();
        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: length.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(GlyphData::MIN_SIZE); held }));

impl Default for GlyphData<'_> {
    fn default() -> Self {
        let Ok(value_56) = FontData::default_table_data();

        Self {
            data: value_56,
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
            0u64 => Some(Field { name: "data", value: FieldType::from(self.data()) }),
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
