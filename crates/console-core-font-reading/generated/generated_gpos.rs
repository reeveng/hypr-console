use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Gpos<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(lookup_list_offset_byte_range) = self.lookup_list_offset_byte_range();

        Ok(0..lookup_list_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Gpos<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"GPOS"); value });
}

impl<'a> FontRead<'a> for Gpos<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Gpos<'a> {
    data: FontData<'a>,
}

impl<'a> Gpos<'a> {
    pub const MIN_SIZE: u64 = (MajorMinor::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<MajorMinor, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn script_list_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.script_list_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn script_list(&self) -> Result<ScriptList<'a>, ReadError> {
        let data = self.data;
        let Ok(script_list_offset) = self.script_list_offset();

        script_list_offset.resolve(data)
    }

    pub fn feature_list_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.feature_list_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn feature_list(&self) -> Result<FeatureList<'a>, ReadError> {
        let data = self.data;
        let Ok(feature_list_offset) = self.feature_list_offset();

        feature_list_offset.resolve(data)
    }

    pub fn lookup_list_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.lookup_list_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn lookup_list(&self) -> Result<PositionLookupList<'a>, ReadError> {
        let data = self.data;
        let Ok(lookup_list_offset) = self.lookup_list_offset();

        lookup_list_offset.resolve(data)
    }

    pub fn feature_variations_offset(&self) -> Result<Option<Nullable<Offset32>>, Never> {
        let Ok(range) = self.feature_variations_offset_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn feature_variations(&self) -> Result<Option<FeatureVariations<'a>>, ReadError> {
        let data = self.data;
        let Ok(read_feature_variations_offset) = self.feature_variations_offset();

        match read_feature_variations_offset { Some(x) => x.resolve(data), None => Ok(None) }
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(MajorMinor::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn script_list_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn feature_list_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.script_list_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn lookup_list_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.feature_list_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn feature_variations_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.lookup_list_offset_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility((1u16, 1u16));

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(Offset32::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Gpos::MIN_SIZE); held }));

impl Default for Gpos<'_> {
    fn default() -> Self {
        let Ok(value_3) = FontData::default_table_data();

        Self {
            data: value_3,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Gpos<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Gpos")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "script_list_offset", value: FieldType::from(FieldType::offset(self.script_list_offset(), (self.script_list()).map(Some))) }),
            2u64 => Some(Field { name: "feature_list_offset", value: FieldType::from(FieldType::offset(self.feature_list_offset(), (self.feature_list()).map(Some))) }),
            3u64 => Some(Field { name: "lookup_list_offset", value: FieldType::from(FieldType::offset(self.lookup_list_offset(), (self.lookup_list()).map(Some))) }),
            4u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility((1u16, 1u16));
                let Ok(feature_variations_offset) = self.feature_variations_offset();

                match compatibility {
                    Compatibility::Compatible => feature_variations_offset.map(|feature_variations_offset_2| Field { name: "feature_variations_offset", value: FieldType::from(FieldType::offset(
                        feature_variations_offset_2,
                        self.feature_variations(),
                    )) }),
                    Compatibility::Incompatible => None,
                }
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Gpos<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

pub enum PositionLookup<'a> {
    Single(Lookup<'a, SinglePos<'a>>),
    Pair(Lookup<'a, PairPos<'a>>),
    Cursive(Lookup<'a, CursivePosFormat1<'a>>),
    MarkToBase(Lookup<'a, MarkBasePosFormat1<'a>>),
    MarkToLig(Lookup<'a, MarkLigPosFormat1<'a>>),
    MarkToMark(Lookup<'a, MarkMarkPosFormat1<'a>>),
    Contextual(Lookup<'a, PositionSequenceContext<'a>>),
    ChainContextual(Lookup<'a, PositionChainContext<'a>>),
    Extension(Lookup<'a, ExtensionSubtable<'a>>),
}

impl Default for PositionLookup<'_> {
    fn default() -> Self {
        Self::Single(Default::default())
    }
}

impl<'a> FontRead<'a> for PositionLookup<'a> {
    fn read(bytes: FontData<'a>) -> Result<Self, ReadError> {
        let discriminant = Lookup::read_discriminant(bytes)?;

        match discriminant {
            1 => {
                let read = FontRead::read(bytes)?;

                Ok(PositionLookup::Single(read))
            }
            2 => {
                let read = FontRead::read(bytes)?;

                Ok(PositionLookup::Pair(read))
            }
            3 => {
                let read = FontRead::read(bytes)?;

                Ok(PositionLookup::Cursive(read))
            }
            4 => {
                let read = FontRead::read(bytes)?;

                Ok(PositionLookup::MarkToBase(read))
            }
            5 => {
                let read = FontRead::read(bytes)?;

                Ok(PositionLookup::MarkToLig(read))
            }
            6 => {
                let read = FontRead::read(bytes)?;

                Ok(PositionLookup::MarkToMark(read))
            }
            7 => {
                let read = FontRead::read(bytes)?;

                Ok(PositionLookup::Contextual(read))
            }
            8 => {
                let read = FontRead::read(bytes)?;

                Ok(PositionLookup::ChainContextual(read))
            }
            9 => {
                let read = FontRead::read(bytes)?;

                Ok(PositionLookup::Extension(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u16>>::from(other))),
        }
    }
}

impl<'a> PositionLookup<'a> {

    pub(crate) fn of_unit_type(&self) -> Result<Lookup<'a, ()>, Never> {
        match self {
            PositionLookup::Single(inner) => inner.of_unit_type(),
            PositionLookup::Pair(inner) => inner.of_unit_type(),
            PositionLookup::Cursive(inner) => inner.of_unit_type(),
            PositionLookup::MarkToBase(inner) => inner.of_unit_type(),
            PositionLookup::MarkToLig(inner) => inner.of_unit_type(),
            PositionLookup::MarkToMark(inner) => inner.of_unit_type(),
            PositionLookup::Contextual(inner) => inner.of_unit_type(),
            PositionLookup::ChainContextual(inner) => inner.of_unit_type(),
            PositionLookup::Extension(inner) => inner.of_unit_type(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> PositionLookup<'a> {
    fn dyn_inner(&self) -> Result<&(dyn SomeTable<'a> + 'a), Never> {
        Ok(match self {
            PositionLookup::Single(table) => table,
            PositionLookup::Pair(table) => table,
            PositionLookup::Cursive(table) => table,
            PositionLookup::MarkToBase(table) => table,
            PositionLookup::MarkToLig(table) => table,
            PositionLookup::MarkToMark(table) => table,
            PositionLookup::Contextual(table) => table,
            PositionLookup::ChainContextual(table) => table,
            PositionLookup::Extension(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PositionLookup<'a> {
    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }

    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for PositionLookup<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck :: AnyBitPattern)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct ValueFormat {
    bits: u16,
}

impl ValueFormat {
    pub const X_PLACEMENT: Self = Self { bits: 0x0001 };

    pub const Y_PLACEMENT: Self = Self { bits: 0x0002 };

    pub const X_ADVANCE: Self = Self { bits: 0x0004 };

    pub const Y_ADVANCE: Self = Self { bits: 0x0008 };

    pub const X_PLACEMENT_DEVICE: Self = Self { bits: 0x0010 };

    pub const Y_PLACEMENT_DEVICE: Self = Self { bits: 0x0020 };

    pub const X_ADVANCE_DEVICE: Self = Self { bits: 0x0040 };

    pub const Y_ADVANCE_DEVICE: Self = Self { bits: 0x0080 };
}

impl ValueFormat {
    const KNOWN: u16 = Self::X_PLACEMENT.bits
        | Self::Y_PLACEMENT.bits
        | Self::X_ADVANCE.bits
        | Self::Y_ADVANCE.bits
        | Self::X_PLACEMENT_DEVICE.bits
        | Self::Y_PLACEMENT_DEVICE.bits
        | Self::X_ADVANCE_DEVICE.bits
        | Self::Y_ADVANCE_DEVICE.bits;

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

impl std::ops::BitOr for ValueFormat {
    type Output = Self;

    #[inline]
    fn bitor(self, other: ValueFormat) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl std::ops::BitOrAssign for ValueFormat {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.bits |= other.bits;
    }
}

impl std::ops::BitXor for ValueFormat {
    type Output = Self;

    #[inline]
    fn bitxor(self, other: Self) -> Self {
        Self {
            bits: self.bits ^ other.bits,
        }
    }
}

impl std::ops::BitXorAssign for ValueFormat {
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        self.bits ^= other.bits;
    }
}

impl std::ops::BitAnd for ValueFormat {
    type Output = Self;

    #[inline]
    fn bitand(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }
}

impl std::ops::BitAndAssign for ValueFormat {
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        self.bits &= other.bits;
    }
}

impl std::ops::Sub for ValueFormat {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }
}

impl std::ops::SubAssign for ValueFormat {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.bits &= !other.bits;
    }
}

impl std::ops::Not for ValueFormat {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        Self { bits: !self.bits & Self::KNOWN }
    }
}

impl std::fmt::Debug for ValueFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let members: &[(&str, Self)] = &[
            ("X_PLACEMENT", Self::X_PLACEMENT),
            ("Y_PLACEMENT", Self::Y_PLACEMENT),
            ("X_ADVANCE", Self::X_ADVANCE),
            ("Y_ADVANCE", Self::Y_ADVANCE),
            ("X_PLACEMENT_DEVICE", Self::X_PLACEMENT_DEVICE),
            ("Y_PLACEMENT_DEVICE", Self::Y_PLACEMENT_DEVICE),
            ("X_ADVANCE_DEVICE", Self::X_ADVANCE_DEVICE),
            ("Y_ADVANCE_DEVICE", Self::Y_ADVANCE_DEVICE),
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

impl std::fmt::Binary for ValueFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Binary::fmt(&self.bits, f)
    }
}

impl std::fmt::Octal for ValueFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Octal::fmt(&self.bits, f)
    }
}

impl std::fmt::LowerHex for ValueFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.bits, f)
    }
}

impl std::fmt::UpperHex for ValueFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.bits, f)
    }
}

impl font_types::Scalar for ValueFormat {
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
impl<'a> From<ValueFormat> for FieldType<'a> {
    fn from(src: ValueFormat) -> FieldType<'a> {
        <crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u16>>::from(src.bits)
    }
}

#[derive(Clone)]
pub enum AnchorTable<'a> {
    Format1(AnchorFormat1<'a>),
    Format2(AnchorFormat2<'a>),
    Format3(AnchorFormat3<'a>),
}

impl Default for AnchorTable<'_> {
    fn default() -> Self {
        Self::Format1(Default::default())
    }
}

impl<'a> AnchorTable<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Format1(item) => item.offset_data(),
            Self::Format2(item) => item.offset_data(),
            Self::Format3(item) => item.offset_data(),
        }
    }

    pub fn anchor_format(&self) -> Result<u16, Never> {
        match self {
            Self::Format1(item) => item.anchor_format(),
            Self::Format2(item) => item.anchor_format(),
            Self::Format3(item) => item.anchor_format(),
        }
    }

    pub fn x_coordinate(&self) -> Result<i16, Never> {
        match self {
            Self::Format1(item) => item.x_coordinate(),
            Self::Format2(item) => item.x_coordinate(),
            Self::Format3(item) => item.x_coordinate(),
        }
    }

    pub fn y_coordinate(&self) -> Result<i16, Never> {
        match self {
            Self::Format1(item) => item.y_coordinate(),
            Self::Format2(item) => item.y_coordinate(),
            Self::Format3(item) => item.y_coordinate(),
        }
    }
}

impl<'a> FontRead<'a> for AnchorTable<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u16 = data.read_at(0u64)?;

        match format {
            AnchorFormat1::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format1(read))
            }
            AnchorFormat2::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format2(read))
            }
            AnchorFormat3::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format3(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u16>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for AnchorTable<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        match self {
            Self::Format1(item) => item.min_byte_range(),
            Self::Format2(item) => item.min_byte_range(),
            Self::Format3(item) => item.min_byte_range(),
        }
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        match self {
            Self::Format1(item) => item.min_table_bytes(),
            Self::Format2(item) => item.min_table_bytes(),
            Self::Format3(item) => item.min_table_bytes(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> AnchorTable<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Format1(table) => table,
            Self::Format2(table) => table,
            Self::Format3(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for AnchorTable<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for AnchorTable<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}

impl Format<u16> for AnchorFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for AnchorFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(y_coordinate_byte_range) = self.y_coordinate_byte_range();

        Ok(0..y_coordinate_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for AnchorFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_10) = data.len();

        match ::core::primitive::bool::then_some(value_10 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct AnchorFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> AnchorFormat1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + i16::RAW_BYTE_LEN + i16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn anchor_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.anchor_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn x_coordinate(&self) -> Result<i16, Never> {
        let Ok(range) = self.x_coordinate_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_coordinate(&self) -> Result<i16, Never> {
        let Ok(range) = self.y_coordinate_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn anchor_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x_coordinate_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.anchor_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_coordinate_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x_coordinate_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(AnchorFormat1::MIN_SIZE); held }));

impl Default for AnchorFormat1<'_> {
    fn default() -> Self {
        let Ok(value_11) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_11,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for AnchorFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("AnchorFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "anchor_format", value: FieldType::from(self.anchor_format()) }),
            1u64 => Some(Field { name: "x_coordinate", value: FieldType::from(self.x_coordinate()) }),
            2u64 => Some(Field { name: "y_coordinate", value: FieldType::from(self.y_coordinate()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for AnchorFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for AnchorFormat2<'_> {
    const FORMAT: u16 = 2;
}

impl<'a> MinByteRange<'a> for AnchorFormat2<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(anchor_point_byte_range) = self.anchor_point_byte_range();

        Ok(0..anchor_point_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for AnchorFormat2<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_12) = data.len();

        match ::core::primitive::bool::then_some(value_12 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct AnchorFormat2<'a> {
    data: FontData<'a>,
}

impl<'a> AnchorFormat2<'a> {
    pub const MIN_SIZE: u64 =
        (u16::RAW_BYTE_LEN + i16::RAW_BYTE_LEN + i16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn anchor_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.anchor_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn x_coordinate(&self) -> Result<i16, Never> {
        let Ok(range) = self.x_coordinate_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_coordinate(&self) -> Result<i16, Never> {
        let Ok(range) = self.y_coordinate_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn anchor_point(&self) -> Result<u16, Never> {
        let Ok(range) = self.anchor_point_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn anchor_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x_coordinate_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.anchor_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_coordinate_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x_coordinate_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn anchor_point_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y_coordinate_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for AnchorFormat2<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("AnchorFormat2")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "anchor_format", value: FieldType::from(self.anchor_format()) }),
            1u64 => Some(Field { name: "x_coordinate", value: FieldType::from(self.x_coordinate()) }),
            2u64 => Some(Field { name: "y_coordinate", value: FieldType::from(self.y_coordinate()) }),
            3u64 => Some(Field { name: "anchor_point", value: FieldType::from(self.anchor_point()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for AnchorFormat2<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for AnchorFormat3<'_> {
    const FORMAT: u16 = 3;
}

impl<'a> MinByteRange<'a> for AnchorFormat3<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(y_device_offset_byte_range) = self.y_device_offset_byte_range();

        Ok(0..y_device_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for AnchorFormat3<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_13) = data.len();

        match ::core::primitive::bool::then_some(value_13 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct AnchorFormat3<'a> {
    data: FontData<'a>,
}

impl<'a> AnchorFormat3<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + i16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn anchor_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.anchor_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn x_coordinate(&self) -> Result<i16, Never> {
        let Ok(range) = self.x_coordinate_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_coordinate(&self) -> Result<i16, Never> {
        let Ok(range) = self.y_coordinate_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn x_device_offset(&self) -> Result<Nullable<Offset16>, Never> {
        let Ok(range) = self.x_device_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn x_device(&self) -> Result<Option<DeviceOrVariationIndex<'a>>, ReadError> {
        let data = self.data;
        let Ok(x_device_offset) = self.x_device_offset();

        x_device_offset.resolve(data)
    }

    pub fn y_device_offset(&self) -> Result<Nullable<Offset16>, Never> {
        let Ok(range) = self.y_device_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_device(&self) -> Result<Option<DeviceOrVariationIndex<'a>>, ReadError> {
        let data = self.data;
        let Ok(y_device_offset) = self.y_device_offset();

        y_device_offset.resolve(data)
    }

    pub fn anchor_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x_coordinate_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.anchor_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_coordinate_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x_coordinate_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x_device_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y_coordinate_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_device_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x_device_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for AnchorFormat3<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("AnchorFormat3")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "anchor_format", value: FieldType::from(self.anchor_format()) }),
            1u64 => Some(Field { name: "x_coordinate", value: FieldType::from(self.x_coordinate()) }),
            2u64 => Some(Field { name: "y_coordinate", value: FieldType::from(self.y_coordinate()) }),
            3u64 => Some(Field { name: "x_device_offset", value: FieldType::from(FieldType::offset(self.x_device_offset(), self.x_device())) }),
            4u64 => Some(Field { name: "y_device_offset", value: FieldType::from(FieldType::offset(self.y_device_offset(), self.y_device())) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for AnchorFormat3<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for MarkArray<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(mark_records_byte_range) = self.mark_records_byte_range();

        Ok(0..mark_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for MarkArray<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_14) = data.len();

        match ::core::primitive::bool::then_some(value_14 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct MarkArray<'a> {
    data: FontData<'a>,
}

impl<'a> MarkArray<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn mark_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.mark_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn mark_records(&self) -> Result<&'a [MarkRecord], Never> {
        let Ok(range) = self.mark_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn mark_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn mark_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(mark_count) = self.mark_count();
        let Ok(previous) = self.mark_count_byte_range();
        let start = previous.end;
        let Ok(value_15) = transforms::to_u64(mark_count);

        let end =
            start.saturating_add(value_15.saturating_mul(MarkRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(MarkArray::MIN_SIZE); held }));

impl Default for MarkArray<'_> {
    fn default() -> Self {
        let Ok(value_16) = FontData::default_table_data();

        Self {
            data: value_16,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for MarkArray<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("MarkArray")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "mark_count", value: FieldType::from(self.mark_count()) }),
            1u64 => {
                let Ok(value_17) = self.offset_data();

                Some(Field { name: "mark_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(MarkRecord),
                    { let Ok(explicit_value) = self.mark_records(); explicit_value },
                    value_17,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for MarkArray<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct MarkRecord {
    pub mark_class: BigEndian<u16>,
    pub mark_anchor_offset: BigEndian<Offset16>,
}

const _: () = {
  assert!(core::mem::size_of::<MarkRecord>() == 4);
  assert!(core::mem::align_of::<MarkRecord>() == 1);
  assert!(core::mem::offset_of!(MarkRecord, mark_class) == 0);
  assert!(core::mem::offset_of!(MarkRecord, mark_anchor_offset) == 2);
};

impl MarkRecord {
    pub fn mark_class(&self) -> Result<u16, Never> {
        self.mark_class.get()
    }

    pub fn mark_anchor_offset(&self) -> Result<Offset16, Never> {
        self.mark_anchor_offset.get()
    }

    pub fn mark_anchor<'a>(&self, data: FontData<'a>) -> Result<AnchorTable<'a>, ReadError> {
        let Ok(mark_anchor_offset) = self.mark_anchor_offset();

        mark_anchor_offset.resolve(data)
    }
}

impl FixedSize for MarkRecord {
    const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for MarkRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "MarkRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "mark_class", value: FieldType::from(self.mark_class()) }),
                1u64 => Some(Field { name: "mark_anchor_offset", value: FieldType::from(FieldType::offset(self.mark_anchor_offset(), (self.mark_anchor(_data)).map(Some))) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone)]
pub enum SinglePos<'a> {
    Format1(SinglePosFormat1<'a>),
    Format2(SinglePosFormat2<'a>),
}

impl Default for SinglePos<'_> {
    fn default() -> Self {
        Self::Format1(Default::default())
    }
}

impl<'a> SinglePos<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Format1(item) => item.offset_data(),
            Self::Format2(item) => item.offset_data(),
        }
    }

    pub fn pos_format(&self) -> Result<u16, Never> {
        match self {
            Self::Format1(item) => item.pos_format(),
            Self::Format2(item) => item.pos_format(),
        }
    }

    pub fn coverage_offset(&self) -> Result<Offset16, Never> {
        match self {
            Self::Format1(item) => item.coverage_offset(),
            Self::Format2(item) => item.coverage_offset(),
        }
    }

    pub fn value_format(&self) -> Result<ValueFormat, Never> {
        match self {
            Self::Format1(item) => item.value_format(),
            Self::Format2(item) => item.value_format(),
        }
    }
}

impl<'a> FontRead<'a> for SinglePos<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u16 = data.read_at(0u64)?;

        match format {
            SinglePosFormat1::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format1(read))
            }
            SinglePosFormat2::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format2(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u16>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for SinglePos<'a> {
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
impl<'a> SinglePos<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Format1(table) => table,
            Self::Format2(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for SinglePos<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for SinglePos<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}

impl Format<u16> for SinglePosFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for SinglePosFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(value_record_byte_range) = self.value_record_byte_range();

        Ok(0..value_record_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for SinglePosFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_22) = data.len();

        match ::core::primitive::bool::then_some(value_22 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct SinglePosFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> SinglePosFormat1<'a> {
    pub const MIN_SIZE: u64 =
        (u16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN + ValueFormat::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn pos_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.pos_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.coverage_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage(&self) -> Result<CoverageTable<'a>, ReadError> {
        let data = self.data;
        let Ok(coverage_offset) = self.coverage_offset();

        coverage_offset.resolve(data)
    }

    pub fn value_format(&self) -> Result<ValueFormat, Never> {
        let Ok(range) = self.value_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn value_record(&self) -> Result<ValueRecord, Never> {
        let Ok(range) = self.value_record_byte_range();

        self.data.read_with_args_or_default(range, &({ let Ok(explicit_value) = self.value_format(); explicit_value }))
    }

    pub fn pos_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.pos_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn value_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(ValueFormat::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn value_record_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.value_format_byte_range();
        let start = previous.end;
        let Ok(value_format) = self.value_format();
        let Ok(size_or_zero) = transforms::size_or_zero(<ValueRecord as ComputeSize>::compute_size(&value_format));
        let end =
            start.saturating_add(size_or_zero);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    SinglePosFormat1::MIN_SIZE
); held }));

impl Default for SinglePosFormat1<'_> {
    fn default() -> Self {
        let Ok(value_23) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_23,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for SinglePosFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("SinglePosFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "pos_format", value: FieldType::from(self.pos_format()) }),
            1u64 => Some(Field { name: "coverage_offset", value: FieldType::from(FieldType::offset(self.coverage_offset(), (self.coverage()).map(Some))) }),
            2u64 => Some(Field { name: "value_format", value: FieldType::from(self.value_format()) }),
            3u64 => {
                let Ok(value_24) = self.offset_data();

                let Ok(value_record) = self.value_record();

                let Ok(value) = value_record.traversal_type(value_24);

                Some(Field { name: "value_record", value })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for SinglePosFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for SinglePosFormat2<'_> {
    const FORMAT: u16 = 2;
}

impl<'a> MinByteRange<'a> for SinglePosFormat2<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(value_records_byte_range) = self.value_records_byte_range();

        Ok(0..value_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for SinglePosFormat2<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_25) = data.len();

        match ::core::primitive::bool::then_some(value_25 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct SinglePosFormat2<'a> {
    data: FontData<'a>,
}

impl<'a> SinglePosFormat2<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + ValueFormat::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn pos_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.pos_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.coverage_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage(&self) -> Result<CoverageTable<'a>, ReadError> {
        let data = self.data;
        let Ok(coverage_offset) = self.coverage_offset();

        coverage_offset.resolve(data)
    }

    pub fn value_format(&self) -> Result<ValueFormat, Never> {
        let Ok(range) = self.value_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn value_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.value_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn value_records(&self) -> Result<ComputedArray<'a, ValueRecord>, Never> {
        let Ok(range) = self.value_records_byte_range();

        self.data.read_with_args_or_default(range, &({ let Ok(explicit_value) = self.value_format(); explicit_value }))
    }

    pub fn pos_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.pos_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn value_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(ValueFormat::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn value_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.value_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn value_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(value_count) = self.value_count();
        let Ok(previous) = self.value_count_byte_range();
        let start = previous.end;
        let Ok(value_26) = transforms::to_u64(value_count);

        let Ok(value_format) = self.value_format();
        let Ok(size_or_zero) = transforms::size_or_zero(<ValueRecord as ComputeSize>::compute_size(&value_format));
        let end = start.saturating_add(value_26.saturating_mul(
                size_or_zero,
            ));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for SinglePosFormat2<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("SinglePosFormat2")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "pos_format", value: FieldType::from(self.pos_format()) }),
            1u64 => Some(Field { name: "coverage_offset", value: FieldType::from(FieldType::offset(self.coverage_offset(), (self.coverage()).map(Some))) }),
            2u64 => Some(Field { name: "value_format", value: FieldType::from(self.value_format()) }),
            3u64 => Some(Field { name: "value_count", value: FieldType::from(self.value_count()) }),
            4u64 => {
                let Ok(value_27) = self.offset_data();

                let Ok(value_records) = self.value_records();

                Some(Field { name: "value_records", value: FieldType::from(traversal::FieldType::computed_array(
                    "ValueRecord",
                    value_records,
                    value_27,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for SinglePosFormat2<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone)]
pub enum PairPos<'a> {
    Format1(PairPosFormat1<'a>),
    Format2(PairPosFormat2<'a>),
}

impl Default for PairPos<'_> {
    fn default() -> Self {
        Self::Format1(Default::default())
    }
}

impl<'a> PairPos<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Format1(item) => item.offset_data(),
            Self::Format2(item) => item.offset_data(),
        }
    }

    pub fn pos_format(&self) -> Result<u16, Never> {
        match self {
            Self::Format1(item) => item.pos_format(),
            Self::Format2(item) => item.pos_format(),
        }
    }

    pub fn coverage_offset(&self) -> Result<Offset16, Never> {
        match self {
            Self::Format1(item) => item.coverage_offset(),
            Self::Format2(item) => item.coverage_offset(),
        }
    }

    pub fn value_format1(&self) -> Result<ValueFormat, Never> {
        match self {
            Self::Format1(item) => item.value_format1(),
            Self::Format2(item) => item.value_format1(),
        }
    }

    pub fn value_format2(&self) -> Result<ValueFormat, Never> {
        match self {
            Self::Format1(item) => item.value_format2(),
            Self::Format2(item) => item.value_format2(),
        }
    }
}

impl<'a> FontRead<'a> for PairPos<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u16 = data.read_at(0u64)?;

        match format {
            PairPosFormat1::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format1(read))
            }
            PairPosFormat2::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format2(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u16>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for PairPos<'a> {
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
impl<'a> PairPos<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Format1(table) => table,
            Self::Format2(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for PairPos<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PairPos<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}

impl Format<u16> for PairPosFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for PairPosFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(pair_set_offsets_byte_range) = self.pair_set_offsets_byte_range();

        Ok(0..pair_set_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PairPosFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_30) = data.len();

        match ::core::primitive::bool::then_some(value_30 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PairPosFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> PairPosFormat1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + ValueFormat::RAW_BYTE_LEN
        + ValueFormat::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn pos_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.pos_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.coverage_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage(&self) -> Result<CoverageTable<'a>, ReadError> {
        let data = self.data;
        let Ok(coverage_offset) = self.coverage_offset();

        coverage_offset.resolve(data)
    }

    pub fn value_format1(&self) -> Result<ValueFormat, Never> {
        let Ok(range) = self.value_format1_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn value_format2(&self) -> Result<ValueFormat, Never> {
        let Ok(range) = self.value_format2_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn pair_set_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.pair_set_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn pair_set_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.pair_set_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn pair_sets(&self) -> Result<ArrayOfOffsets<'a, PairSet<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.pair_set_offsets();
        let Ok(value_format1) = self.value_format1();
        let Ok(value_format2) = self.value_format2();
        let args = (value_format1, value_format2);
        ArrayOfOffsets::new(offsets, data, args)
    }

    pub fn pos_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.pos_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn value_format1_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(ValueFormat::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn value_format2_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.value_format1_byte_range();
        let start = previous.end;
        let end = start.saturating_add(ValueFormat::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn pair_set_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.value_format2_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn pair_set_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(pair_set_count) = self.pair_set_count();
        let Ok(previous) = self.pair_set_count_byte_range();
        let start = previous.end;
        let Ok(value_31) = transforms::to_u64(pair_set_count);

        let end =
            start.saturating_add(value_31.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(PairPosFormat1::MIN_SIZE); held }));

impl Default for PairPosFormat1<'_> {
    fn default() -> Self {
        let Ok(value_32) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_32,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PairPosFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PairPosFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "pos_format", value: FieldType::from(self.pos_format()) }),
            1u64 => Some(Field { name: "coverage_offset", value: FieldType::from(FieldType::offset(self.coverage_offset(), (self.coverage()).map(Some))) }),
            2u64 => Some(Field { name: "value_format1", value: FieldType::from(self.value_format1()) }),
            3u64 => Some(Field { name: "value_format2", value: FieldType::from(self.value_format2()) }),
            4u64 => Some(Field { name: "pair_set_count", value: FieldType::from(self.pair_set_count()) }),
            5u64 => Some(Field { name: "pair_set_offsets", value: FieldType::from(self.pair_sets()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PairPosFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for PairSet<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(pair_value_records_byte_range) = self.pair_value_records_byte_range();

        Ok(0..pair_value_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl ReadArgs for PairSet<'_> {
    type Arguments = (ValueFormat, ValueFormat);
}

impl<'a> FontReadWithArgs<'a> for PairSet<'a> {
    fn read_with_args(
        data: FontData<'a>,
        args: &(ValueFormat, ValueFormat),
    ) -> Result<Self, ReadError> {
        let (value_format1, value_format2) = *args;

        let Ok(value_33) = data.len();

        match ::core::primitive::bool::then_some(value_33 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self {
            data,
            value_format1,
            value_format2,
        })
    }
}

impl<'a> PairSet<'a> {
    pub fn read(
        data: FontData<'a>,
        value_format1: ValueFormat,
        value_format2: ValueFormat,
    ) -> Result<Self, ReadError> {
        let args = (value_format1, value_format2);
        Self::read_with_args(data, &args)
    }
}

#[derive(Clone)]
pub struct PairSet<'a> {
    data: FontData<'a>,
    value_format1: ValueFormat,
    value_format2: ValueFormat,
}

impl<'a> PairSet<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn pair_value_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.pair_value_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn pair_value_records(&self) -> Result<ComputedArray<'a, PairValueRecord>, Never> {
        let Ok(range) = self.pair_value_records_byte_range();

        self.data.read_with_args_or_default(range, &(({ let Ok(explicit_value) = self.value_format1(); explicit_value }), ({ let Ok(explicit_value) = self.value_format2(); explicit_value })))
    }

    pub(crate) fn value_format1(&self) -> Result<ValueFormat, Never> {
        Ok(self.value_format1)
    }

    pub(crate) fn value_format2(&self) -> Result<ValueFormat, Never> {
        Ok(self.value_format2)
    }

    pub fn pair_value_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn pair_value_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(pair_value_count) = self.pair_value_count();
        let Ok(previous) = self.pair_value_count_byte_range();
        let start = previous.end;
        let Ok(value_34) = transforms::to_u64(pair_value_count);

        let Ok(value_format1) = self.value_format1();
        let Ok(value_format2) = self.value_format2();
        let Ok(size_or_zero) = transforms::size_or_zero(<PairValueRecord as ComputeSize>::compute_size(&(
            value_format1,
            value_format2,
        )));
        let end = start.saturating_add(value_34.saturating_mul(
                size_or_zero,
            ));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(PairSet::MIN_SIZE); held }));

impl Default for PairSet<'_> {
    fn default() -> Self {
        let Ok(value_35) = FontData::default_table_data();

        Self {
            data: value_35,
            value_format1: Default::default(),
            value_format2: Default::default(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PairSet<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PairSet")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "pair_value_count", value: FieldType::from(self.pair_value_count()) }),
            1u64 => {
                let Ok(value_36) = self.offset_data();

                let Ok(pair_value_records) = self.pair_value_records();

                Some(Field { name: "pair_value_records", value: FieldType::from(traversal::FieldType::computed_array(
                    "PairValueRecord",
                    pair_value_records,
                    value_36,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PairSet<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug)]
pub struct PairValueRecord {
    pub second_glyph: BigEndian<GlyphId16>,
    pub value_record1: ValueRecord,
    pub value_record2: ValueRecord,
}

impl PairValueRecord {
    pub fn second_glyph(&self) -> Result<GlyphId16, Never> {
        self.second_glyph.get()
    }

    pub fn value_record1(&self) -> Result<&ValueRecord, Never> {
        Ok(&self.value_record1)
    }

    pub fn value_record2(&self) -> Result<&ValueRecord, Never> {
        Ok(&self.value_record2)
    }
}

impl ReadArgs for PairValueRecord {
    type Arguments = (ValueFormat, ValueFormat);
}

impl ComputeSize for PairValueRecord {
  fn compute_size(args: &(ValueFormat, ValueFormat)) -> Result<u64, ReadError> {
        let (value_format1, value_format2) = *args;
    let mut result = 0u64;
    let checked_value = result
            .checked_add(GlyphId16::RAW_BYTE_LEN)
            .ok_or(ReadError::OutOfBounds)?;
    result = checked_value;
    let checked_value_2 = result
            .checked_add(match <ValueRecord as ComputeSize>::compute_size(&value_format1) {
                Ok(width) => width,
                Err(_invalid_value_record_width) => 0,
            })
            .ok_or(ReadError::OutOfBounds)?;
    result = checked_value_2;
    let checked_value_3 = result
            .checked_add(match <ValueRecord as ComputeSize>::compute_size(&value_format2) {
                Ok(width) => width,
                Err(_invalid_value_record_width) => 0,
            })
            .ok_or(ReadError::OutOfBounds)?;
    result = checked_value_3;
        Ok(result)
    }
}

impl<'a> FontReadWithArgs<'a> for PairValueRecord {
    fn read_with_args(
        data: FontData<'a>,
        args: &(ValueFormat, ValueFormat),
    ) -> Result<Self, ReadError> {
        let Ok(value_38) = data.cursor();

        let mut cursor = value_38;
        let (value_format1, value_format2) = *args;
        let second_glyph = cursor.read_be()?;
        let value_record1 = cursor.read_with_args(&value_format1)?;
        let value_record2 = cursor.read_with_args(&value_format2)?;

        Ok(Self {
            second_glyph,
            value_record1,
            value_record2,
        })
    }
}

impl<'a> PairValueRecord {
    pub fn read(
        data: FontData<'a>,
        value_format1: ValueFormat,
        value_format2: ValueFormat,
    ) -> Result<Self, ReadError> {
        let args = (value_format1, value_format2);
        Self::read_with_args(data, &args)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for PairValueRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "PairValueRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "second_glyph", value: FieldType::from(self.second_glyph()) }),
                1u64 => {
                    let Ok(value_record1) = self.value_record1();

                    let Ok(value) = value_record1.traversal_type(_data);

                    Some(Field { name: "value_record1", value })
                }
                2u64 => {
                    let Ok(value_record2) = self.value_record2();

                    let Ok(value) = value_record2.traversal_type(_data);

                    Some(Field { name: "value_record2", value })
                }
                _ => None,
            }),
            data,
        })
    }
}

impl Format<u16> for PairPosFormat2<'_> {
    const FORMAT: u16 = 2;
}

impl<'a> MinByteRange<'a> for PairPosFormat2<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(class1_records_byte_range) = self.class1_records_byte_range();

        Ok(0..class1_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PairPosFormat2<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_39) = data.len();

        match ::core::primitive::bool::then_some(value_39 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PairPosFormat2<'a> {
    data: FontData<'a>,
}

impl<'a> PairPosFormat2<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + ValueFormat::RAW_BYTE_LEN
        + ValueFormat::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn pos_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.pos_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.coverage_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage(&self) -> Result<CoverageTable<'a>, ReadError> {
        let data = self.data;
        let Ok(coverage_offset) = self.coverage_offset();

        coverage_offset.resolve(data)
    }

    pub fn value_format1(&self) -> Result<ValueFormat, Never> {
        let Ok(range) = self.value_format1_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn value_format2(&self) -> Result<ValueFormat, Never> {
        let Ok(range) = self.value_format2_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn class_def1_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.class_def1_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn class_def1(&self) -> Result<ClassDef<'a>, ReadError> {
        let data = self.data;
        let Ok(class_def1_offset) = self.class_def1_offset();

        class_def1_offset.resolve(data)
    }

    pub fn class_def2_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.class_def2_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn class_def2(&self) -> Result<ClassDef<'a>, ReadError> {
        let data = self.data;
        let Ok(class_def2_offset) = self.class_def2_offset();

        class_def2_offset.resolve(data)
    }

    pub fn class1_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.class1_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn class2_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.class2_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn class1_records(&self) -> Result<ComputedArray<'a, Class1Record<'a>>, Never> {
        let Ok(range) = self.class1_records_byte_range();

        self.data.read_with_args_or_default(range, &(
                    ({ let Ok(explicit_value) = self.class2_count(); explicit_value }),
                    ({ let Ok(explicit_value) = self.value_format1(); explicit_value }),
                    ({ let Ok(explicit_value) = self.value_format2(); explicit_value }),
                ))
    }

    pub fn pos_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.pos_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn value_format1_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(ValueFormat::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn value_format2_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.value_format1_byte_range();
        let start = previous.end;
        let end = start.saturating_add(ValueFormat::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn class_def1_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.value_format2_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn class_def2_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.class_def1_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn class1_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.class_def2_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn class2_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.class1_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn class1_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(class1_count) = self.class1_count();
        let Ok(previous) = self.class2_count_byte_range();
        let start = previous.end;
        let Ok(value_40) = transforms::to_u64(class1_count);

        let Ok(class2_count) = self.class2_count();
        let Ok(value_format1) = self.value_format1();
        let Ok(value_format2) = self.value_format2();
        let Ok(size_or_zero) = transforms::size_or_zero(<Class1Record as ComputeSize>::compute_size(&(
            class2_count,
            value_format1,
            value_format2,
        )));
        let end = start.saturating_add(value_40.saturating_mul(
                size_or_zero,
            ));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PairPosFormat2<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PairPosFormat2")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "pos_format", value: FieldType::from(self.pos_format()) }),
            1u64 => Some(Field { name: "coverage_offset", value: FieldType::from(FieldType::offset(self.coverage_offset(), (self.coverage()).map(Some))) }),
            2u64 => Some(Field { name: "value_format1", value: FieldType::from(self.value_format1()) }),
            3u64 => Some(Field { name: "value_format2", value: FieldType::from(self.value_format2()) }),
            4u64 => Some(Field { name: "class_def1_offset", value: FieldType::from(FieldType::offset(self.class_def1_offset(), (self.class_def1()).map(Some))) }),
            5u64 => Some(Field { name: "class_def2_offset", value: FieldType::from(FieldType::offset(self.class_def2_offset(), (self.class_def2()).map(Some))) }),
            6u64 => Some(Field { name: "class1_count", value: FieldType::from(self.class1_count()) }),
            7u64 => Some(Field { name: "class2_count", value: FieldType::from(self.class2_count()) }),
            8u64 => {
                let Ok(value_41) = self.offset_data();

                let Ok(class1_records) = self.class1_records();

                Some(Field { name: "class1_records", value: FieldType::from(traversal::FieldType::computed_array(
                    "Class1Record",
                    class1_records,
                    value_41,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PairPosFormat2<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug)]
pub struct Class1Record<'a> {
    pub class2_records: ComputedArray<'a, Class2Record>,
}

impl<'a> Class1Record<'a> {
    pub fn class2_records(&self) -> Result<&ComputedArray<'a, Class2Record>, Never> {
        Ok(&self.class2_records)
    }
}

impl ReadArgs for Class1Record<'_> {
    type Arguments = (u16, ValueFormat, ValueFormat);
}

impl ComputeSize for Class1Record<'_> {

    fn compute_size(args: &(u16, ValueFormat, ValueFormat)) -> Result<u64, ReadError> {
        let (class2_count, value_format1, value_format2) = *args;
        let Ok(value_42) = transforms::to_u64(class2_count);

        Ok(value_42.saturating_mul(
            match <Class2Record as ComputeSize>::compute_size(&(value_format1, value_format2)) {
                Ok(width) => width,
                Err(_invalid_class_record_width) => 0,
            },
        ))
    }
}

impl<'a> FontReadWithArgs<'a> for Class1Record<'a> {
    fn read_with_args(
        data: FontData<'a>,
        args: &(u16, ValueFormat, ValueFormat),
    ) -> Result<Self, ReadError> {
        let Ok(value_43) = data.cursor();

        let mut cursor = value_43;
        let (class2_count, value_format1, value_format2) = *args;
        let Ok(value_44) = transforms::to_u64(class2_count);

        let class2_records = cursor.read_computed_array(
                value_44,
                &(value_format1, value_format2),
            )?;


        Ok(Self {

            class2_records,

        })
    }
}

impl<'a> Class1Record<'a> {
    pub fn read(
        data: FontData<'a>,
        class2_count: u16,
        value_format1: ValueFormat,
        value_format2: ValueFormat,
    ) -> Result<Self, ReadError> {
        let args = (class2_count, value_format1, value_format2);
        Self::read_with_args(data, &args)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for Class1Record<'a> {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "Class1Record",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "class2_records", value: FieldType::from(traversal::FieldType::computed_array(
                        "Class2Record",
                        { let Ok(explicit_value) = self.class2_records(); explicit_value.clone() },
                        { let Ok(explicit_value) = FontData::new(&[]); explicit_value },
                    )) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Class2Record {
    pub value_record1: ValueRecord,
    pub value_record2: ValueRecord,
}

impl Class2Record {
    pub fn value_record1(&self) -> Result<&ValueRecord, Never> {
        Ok(&self.value_record1)
    }

    pub fn value_record2(&self) -> Result<&ValueRecord, Never> {
        Ok(&self.value_record2)
    }
}

impl ReadArgs for Class2Record {
    type Arguments = (ValueFormat, ValueFormat);
}

impl ComputeSize for Class2Record {
  fn compute_size(args: &(ValueFormat, ValueFormat)) -> Result<u64, ReadError> {
        let (value_format1, value_format2) = *args;
    let mut result = 0u64;
    let checked_value = result
            .checked_add(match <ValueRecord as ComputeSize>::compute_size(&value_format1) {
                Ok(width) => width,
                Err(_invalid_value_record_width) => 0,
            })
            .ok_or(ReadError::OutOfBounds)?;
    result = checked_value;
    let checked_value_2 = result
            .checked_add(match <ValueRecord as ComputeSize>::compute_size(&value_format2) {
                Ok(width) => width,
                Err(_invalid_value_record_width) => 0,
            })
            .ok_or(ReadError::OutOfBounds)?;
    result = checked_value_2;
        Ok(result)
    }
}

impl<'a> FontReadWithArgs<'a> for Class2Record {
    fn read_with_args(
        data: FontData<'a>,
        args: &(ValueFormat, ValueFormat),
    ) -> Result<Self, ReadError> {
        let Ok(value_45) = data.cursor();

        let mut cursor = value_45;
        let (value_format1, value_format2) = *args;
        let value_record1 = cursor.read_with_args(&value_format1)?;
        let value_record2 = cursor.read_with_args(&value_format2)?;

        Ok(Self {
            value_record1,
            value_record2,
        })
    }
}

impl<'a> Class2Record {
    pub fn read(
        data: FontData<'a>,
        value_format1: ValueFormat,
        value_format2: ValueFormat,
    ) -> Result<Self, ReadError> {
        let args = (value_format1, value_format2);
        Self::read_with_args(data, &args)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for Class2Record {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "Class2Record",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => {
                    let Ok(value_record1) = self.value_record1();

                    let Ok(value) = value_record1.traversal_type(_data);

                    Some(Field { name: "value_record1", value })
                }
                1u64 => {
                    let Ok(value_record2) = self.value_record2();

                    let Ok(value) = value_record2.traversal_type(_data);

                    Some(Field { name: "value_record2", value })
                }
                _ => None,
            }),
            data,
        })
    }
}

impl Format<u16> for CursivePosFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for CursivePosFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(entry_exit_record_byte_range) = self.entry_exit_record_byte_range();

        Ok(0..entry_exit_record_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for CursivePosFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_46) = data.len();

        match ::core::primitive::bool::then_some(value_46 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct CursivePosFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> CursivePosFormat1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn pos_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.pos_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.coverage_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage(&self) -> Result<CoverageTable<'a>, ReadError> {
        let data = self.data;
        let Ok(coverage_offset) = self.coverage_offset();

        coverage_offset.resolve(data)
    }

    pub fn entry_exit_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.entry_exit_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn entry_exit_record(&self) -> Result<&'a [EntryExitRecord], Never> {
        let Ok(range) = self.entry_exit_record_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn pos_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.pos_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn entry_exit_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn entry_exit_record_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(entry_exit_count) = self.entry_exit_count();
        let Ok(previous) = self.entry_exit_count_byte_range();
        let start = previous.end;
        let Ok(value_47) = transforms::to_u64(entry_exit_count);

        let end = start.saturating_add(value_47
                .saturating_mul(EntryExitRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    CursivePosFormat1::MIN_SIZE
); held }));

impl Default for CursivePosFormat1<'_> {
    fn default() -> Self {
        let Ok(value_48) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_48,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for CursivePosFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("CursivePosFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "pos_format", value: FieldType::from(self.pos_format()) }),
            1u64 => Some(Field { name: "coverage_offset", value: FieldType::from(FieldType::offset(self.coverage_offset(), (self.coverage()).map(Some))) }),
            2u64 => Some(Field { name: "entry_exit_count", value: FieldType::from(self.entry_exit_count()) }),
            3u64 => {
                let Ok(value_49) = self.offset_data();

                Some(Field { name: "entry_exit_record", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(EntryExitRecord),
                    { let Ok(explicit_value) = self.entry_exit_record(); explicit_value },
                    value_49,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for CursivePosFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct EntryExitRecord {
    pub entry_anchor_offset: BigEndian<Nullable<Offset16>>,
    pub exit_anchor_offset: BigEndian<Nullable<Offset16>>,
}

const _: () = {
  assert!(core::mem::size_of::<EntryExitRecord>() == 4);
  assert!(core::mem::align_of::<EntryExitRecord>() == 1);
  assert!(core::mem::offset_of!(EntryExitRecord, entry_anchor_offset) == 0);
  assert!(core::mem::offset_of!(EntryExitRecord, exit_anchor_offset) == 2);
};

impl EntryExitRecord {
    pub fn entry_anchor_offset(&self) -> Result<Nullable<Offset16>, Never> {
        self.entry_anchor_offset.get()
    }

    pub fn entry_anchor<'a>(
        &self,
        data: FontData<'a>,
    ) -> Result<Option<AnchorTable<'a>>, ReadError> {
        let Ok(entry_anchor_offset) = self.entry_anchor_offset();

        entry_anchor_offset.resolve(data)
    }

    pub fn exit_anchor_offset(&self) -> Result<Nullable<Offset16>, Never> {
        self.exit_anchor_offset.get()
    }

    pub fn exit_anchor<'a>(
        &self,
        data: FontData<'a>,
    ) -> Result<Option<AnchorTable<'a>>, ReadError> {
        let Ok(exit_anchor_offset) = self.exit_anchor_offset();

        exit_anchor_offset.resolve(data)
    }
}

impl FixedSize for EntryExitRecord {
    const RAW_BYTE_LEN: u64 = Offset16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for EntryExitRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "EntryExitRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "entry_anchor_offset", value: FieldType::from(FieldType::offset(self.entry_anchor_offset(), self.entry_anchor(_data))) }),
                1u64 => Some(Field { name: "exit_anchor_offset", value: FieldType::from(FieldType::offset(self.exit_anchor_offset(), self.exit_anchor(_data))) }),
                _ => None,
            }),
            data,
        })
    }
}

impl Format<u16> for MarkBasePosFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for MarkBasePosFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(base_array_offset_byte_range) = self.base_array_offset_byte_range();

        Ok(0..base_array_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for MarkBasePosFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_52) = data.len();

        match ::core::primitive::bool::then_some(value_52 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct MarkBasePosFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> MarkBasePosFormat1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn pos_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.pos_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn mark_coverage_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.mark_coverage_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn mark_coverage(&self) -> Result<CoverageTable<'a>, ReadError> {
        let data = self.data;
        let Ok(mark_coverage_offset) = self.mark_coverage_offset();

        mark_coverage_offset.resolve(data)
    }

    pub fn base_coverage_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.base_coverage_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn base_coverage(&self) -> Result<CoverageTable<'a>, ReadError> {
        let data = self.data;
        let Ok(base_coverage_offset) = self.base_coverage_offset();

        base_coverage_offset.resolve(data)
    }

    pub fn mark_class_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.mark_class_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn mark_array_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.mark_array_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn mark_array(&self) -> Result<MarkArray<'a>, ReadError> {
        let data = self.data;
        let Ok(mark_array_offset) = self.mark_array_offset();

        mark_array_offset.resolve(data)
    }

    pub fn base_array_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.base_array_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn base_array(&self) -> Result<BaseArray<'a>, ReadError> {
        let data = self.data;
        let Ok(args) = self.mark_class_count();
        let Ok(base_array_offset) = self.base_array_offset();

        base_array_offset.resolve_with_args(data, &args)
    }

    pub fn pos_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn mark_coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.pos_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn base_coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.mark_coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn mark_class_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.base_coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn mark_array_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.mark_class_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn base_array_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.mark_array_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    MarkBasePosFormat1::MIN_SIZE
); held }));

impl Default for MarkBasePosFormat1<'_> {
    fn default() -> Self {
        let Ok(value_53) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_53,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for MarkBasePosFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("MarkBasePosFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "pos_format", value: FieldType::from(self.pos_format()) }),
            1u64 => Some(Field { name: "mark_coverage_offset", value: FieldType::from(FieldType::offset(self.mark_coverage_offset(), (self.mark_coverage()).map(Some))) }),
            2u64 => Some(Field { name: "base_coverage_offset", value: FieldType::from(FieldType::offset(self.base_coverage_offset(), (self.base_coverage()).map(Some))) }),
            3u64 => Some(Field { name: "mark_class_count", value: FieldType::from(self.mark_class_count()) }),
            4u64 => Some(Field { name: "mark_array_offset", value: FieldType::from(FieldType::offset(self.mark_array_offset(), (self.mark_array()).map(Some))) }),
            5u64 => Some(Field { name: "base_array_offset", value: FieldType::from(FieldType::offset(self.base_array_offset(), (self.base_array()).map(Some))) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for MarkBasePosFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for BaseArray<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(base_records_byte_range) = self.base_records_byte_range();

        Ok(0..base_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl ReadArgs for BaseArray<'_> {
    type Arguments = u16;
}

impl<'a> FontReadWithArgs<'a> for BaseArray<'a> {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let mark_class_count = *args;

        let Ok(value_54) = data.len();

        match ::core::primitive::bool::then_some(value_54 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self {
            data,
            mark_class_count,
        })
    }
}

impl<'a> BaseArray<'a> {
    pub fn read(data: FontData<'a>, mark_class_count: u16) -> Result<Self, ReadError> {
        let args = mark_class_count;
        Self::read_with_args(data, &args)
    }
}

#[derive(Clone)]
pub struct BaseArray<'a> {
    data: FontData<'a>,
    mark_class_count: u16,
}

impl<'a> BaseArray<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn base_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.base_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn base_records(&self) -> Result<ComputedArray<'a, BaseRecord<'a>>, Never> {
        let Ok(range) = self.base_records_byte_range();

        self.data.read_with_args_or_default(range, &({ let Ok(explicit_value) = self.mark_class_count(); explicit_value }))
    }

    pub(crate) fn mark_class_count(&self) -> Result<u16, Never> {
        Ok(self.mark_class_count)
    }

    pub fn base_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn base_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(base_count) = self.base_count();
        let Ok(previous) = self.base_count_byte_range();
        let start = previous.end;
        let Ok(value_55) = transforms::to_u64(base_count);

        let Ok(mark_class_count) = self.mark_class_count();
        let Ok(size_or_zero) = transforms::size_or_zero(<BaseRecord as ComputeSize>::compute_size(&mark_class_count));
        let end = start.saturating_add(value_55.saturating_mul(
                size_or_zero,
            ));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(BaseArray::MIN_SIZE); held }));

impl Default for BaseArray<'_> {
    fn default() -> Self {
        let Ok(value_56) = FontData::default_table_data();

        Self {
            data: value_56,
            mark_class_count: Default::default(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for BaseArray<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("BaseArray")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "base_count", value: FieldType::from(self.base_count()) }),
            1u64 => {
                let Ok(value_57) = self.offset_data();

                let Ok(base_records) = self.base_records();

                Some(Field { name: "base_records", value: FieldType::from(traversal::FieldType::computed_array(
                    "BaseRecord",
                    base_records,
                    value_57,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for BaseArray<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug)]
pub struct BaseRecord<'a> {
    pub base_anchor_offsets: &'a [BigEndian<Nullable<Offset16>>],
}

impl<'a> BaseRecord<'a> {
    pub fn base_anchor_offsets(&self) -> Result<&'a [BigEndian<Nullable<Offset16>>], Never> {
        Ok(self.base_anchor_offsets)
    }

    pub fn base_anchors(
        &self,
        data: FontData<'a>,
    ) -> Result<ArrayOfNullableOffsets<'a, AnchorTable<'a>, Offset16>, Never> {
        let Ok(offsets) = self.base_anchor_offsets();
        ArrayOfNullableOffsets::new(offsets, data, ())
    }
}

impl ReadArgs for BaseRecord<'_> {
    type Arguments = u16;
}

impl ComputeSize for BaseRecord<'_> {

    fn compute_size(args: &u16) -> Result<u64, ReadError> {
        let mark_class_count = *args;
        let Ok(value_58) = transforms::to_u64(mark_class_count);

        Ok(value_58.saturating_mul(Offset16::RAW_BYTE_LEN))
    }
}

impl<'a> FontReadWithArgs<'a> for BaseRecord<'a> {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let Ok(value_59) = data.cursor();

        let mut cursor = value_59;
        let mark_class_count = *args;
        let Ok(value_60) = transforms::to_u64(mark_class_count);

        let base_anchor_offsets = cursor.read_array(value_60)?;


        Ok(Self {

            base_anchor_offsets,

        })
    }
}

impl<'a> BaseRecord<'a> {
    pub fn read(data: FontData<'a>, mark_class_count: u16) -> Result<Self, ReadError> {
        let args = mark_class_count;
        Self::read_with_args(data, &args)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for BaseRecord<'a> {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "BaseRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "base_anchor_offsets", value: FieldType::from(self.base_anchors(_data)) }),
                _ => None,
            }),
            data,
        })
    }
}

impl Format<u16> for MarkLigPosFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for MarkLigPosFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(ligature_array_offset_byte_range) = self.ligature_array_offset_byte_range();

        Ok(0..ligature_array_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for MarkLigPosFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_61) = data.len();

        match ::core::primitive::bool::then_some(value_61 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct MarkLigPosFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> MarkLigPosFormat1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn pos_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.pos_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn mark_coverage_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.mark_coverage_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn mark_coverage(&self) -> Result<CoverageTable<'a>, ReadError> {
        let data = self.data;
        let Ok(mark_coverage_offset) = self.mark_coverage_offset();

        mark_coverage_offset.resolve(data)
    }

    pub fn ligature_coverage_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.ligature_coverage_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn ligature_coverage(&self) -> Result<CoverageTable<'a>, ReadError> {
        let data = self.data;
        let Ok(ligature_coverage_offset) = self.ligature_coverage_offset();

        ligature_coverage_offset.resolve(data)
    }

    pub fn mark_class_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.mark_class_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn mark_array_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.mark_array_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn mark_array(&self) -> Result<MarkArray<'a>, ReadError> {
        let data = self.data;
        let Ok(mark_array_offset) = self.mark_array_offset();

        mark_array_offset.resolve(data)
    }

    pub fn ligature_array_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.ligature_array_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn ligature_array(&self) -> Result<LigatureArray<'a>, ReadError> {
        let data = self.data;
        let Ok(args) = self.mark_class_count();
        let Ok(ligature_array_offset) = self.ligature_array_offset();

        ligature_array_offset.resolve_with_args(data, &args)
    }

    pub fn pos_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn mark_coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.pos_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn ligature_coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.mark_coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn mark_class_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.ligature_coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn mark_array_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.mark_class_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn ligature_array_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.mark_array_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    MarkLigPosFormat1::MIN_SIZE
); held }));

impl Default for MarkLigPosFormat1<'_> {
    fn default() -> Self {
        let Ok(value_62) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_62,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for MarkLigPosFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("MarkLigPosFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "pos_format", value: FieldType::from(self.pos_format()) }),
            1u64 => Some(Field { name: "mark_coverage_offset", value: FieldType::from(FieldType::offset(self.mark_coverage_offset(), (self.mark_coverage()).map(Some))) }),
            2u64 => Some(Field { name: "ligature_coverage_offset", value: FieldType::from(FieldType::offset(self.ligature_coverage_offset(), (self.ligature_coverage()).map(Some))) }),
            3u64 => Some(Field { name: "mark_class_count", value: FieldType::from(self.mark_class_count()) }),
            4u64 => Some(Field { name: "mark_array_offset", value: FieldType::from(FieldType::offset(self.mark_array_offset(), (self.mark_array()).map(Some))) }),
            5u64 => Some(Field { name: "ligature_array_offset", value: FieldType::from(FieldType::offset(self.ligature_array_offset(), (self.ligature_array()).map(Some))) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for MarkLigPosFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for LigatureArray<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(ligature_attach_offsets_byte_range) = self.ligature_attach_offsets_byte_range();

        Ok(0..ligature_attach_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl ReadArgs for LigatureArray<'_> {
    type Arguments = u16;
}

impl<'a> FontReadWithArgs<'a> for LigatureArray<'a> {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let mark_class_count = *args;

        let Ok(value_63) = data.len();

        match ::core::primitive::bool::then_some(value_63 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self {
            data,
            mark_class_count,
        })
    }
}

impl<'a> LigatureArray<'a> {
    pub fn read(data: FontData<'a>, mark_class_count: u16) -> Result<Self, ReadError> {
        let args = mark_class_count;
        Self::read_with_args(data, &args)
    }
}

#[derive(Clone)]
pub struct LigatureArray<'a> {
    data: FontData<'a>,
    mark_class_count: u16,
}

impl<'a> LigatureArray<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn ligature_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.ligature_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn ligature_attach_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.ligature_attach_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn ligature_attaches(&self) -> Result<ArrayOfOffsets<'a, LigatureAttach<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.ligature_attach_offsets();
        let Ok(args) = self.mark_class_count();
        ArrayOfOffsets::new(offsets, data, args)
    }

    pub(crate) fn mark_class_count(&self) -> Result<u16, Never> {
        Ok(self.mark_class_count)
    }

    pub fn ligature_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn ligature_attach_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(ligature_count) = self.ligature_count();
        let Ok(previous) = self.ligature_count_byte_range();
        let start = previous.end;
        let Ok(value_64) = transforms::to_u64(ligature_count);

        let end =
            start.saturating_add(value_64.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(LigatureArray::MIN_SIZE); held }));

impl Default for LigatureArray<'_> {
    fn default() -> Self {
        let Ok(value_65) = FontData::default_table_data();

        Self {
            data: value_65,
            mark_class_count: Default::default(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for LigatureArray<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("LigatureArray")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "ligature_count", value: FieldType::from(self.ligature_count()) }),
            1u64 => Some(Field { name: "ligature_attach_offsets", value: FieldType::from(self.ligature_attaches()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for LigatureArray<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for LigatureAttach<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(component_records_byte_range) = self.component_records_byte_range();

        Ok(0..component_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl ReadArgs for LigatureAttach<'_> {
    type Arguments = u16;
}

impl<'a> FontReadWithArgs<'a> for LigatureAttach<'a> {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let mark_class_count = *args;

        let Ok(value_66) = data.len();

        match ::core::primitive::bool::then_some(value_66 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self {
            data,
            mark_class_count,
        })
    }
}

impl<'a> LigatureAttach<'a> {
    pub fn read(data: FontData<'a>, mark_class_count: u16) -> Result<Self, ReadError> {
        let args = mark_class_count;
        Self::read_with_args(data, &args)
    }
}

#[derive(Clone)]
pub struct LigatureAttach<'a> {
    data: FontData<'a>,
    mark_class_count: u16,
}

impl<'a> LigatureAttach<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn component_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.component_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn component_records(&self) -> Result<ComputedArray<'a, ComponentRecord<'a>>, Never> {
        let Ok(range) = self.component_records_byte_range();

        self.data.read_with_args_or_default(range, &({ let Ok(explicit_value) = self.mark_class_count(); explicit_value }))
    }

    pub(crate) fn mark_class_count(&self) -> Result<u16, Never> {
        Ok(self.mark_class_count)
    }

    pub fn component_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn component_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(component_count) = self.component_count();
        let Ok(previous) = self.component_count_byte_range();
        let start = previous.end;
        let Ok(value_67) = transforms::to_u64(component_count);

        let Ok(mark_class_count) = self.mark_class_count();
        let Ok(size_or_zero) = transforms::size_or_zero(<ComponentRecord as ComputeSize>::compute_size(&mark_class_count));
        let end = start.saturating_add(value_67.saturating_mul(
                size_or_zero,
            ));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(LigatureAttach::MIN_SIZE); held }));

impl Default for LigatureAttach<'_> {
    fn default() -> Self {
        let Ok(value_68) = FontData::default_table_data();

        Self {
            data: value_68,
            mark_class_count: Default::default(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for LigatureAttach<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("LigatureAttach")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "component_count", value: FieldType::from(self.component_count()) }),
            1u64 => {
                let Ok(value_69) = self.offset_data();

                let Ok(component_records) = self.component_records();

                Some(Field { name: "component_records", value: FieldType::from(traversal::FieldType::computed_array(
                    "ComponentRecord",
                    component_records,
                    value_69,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for LigatureAttach<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug)]
pub struct ComponentRecord<'a> {
    pub ligature_anchor_offsets: &'a [BigEndian<Nullable<Offset16>>],
}

impl<'a> ComponentRecord<'a> {
    pub fn ligature_anchor_offsets(&self) -> Result<&'a [BigEndian<Nullable<Offset16>>], Never> {
        Ok(self.ligature_anchor_offsets)
    }

    pub fn ligature_anchors(
        &self,
        data: FontData<'a>,
    ) -> Result<ArrayOfNullableOffsets<'a, AnchorTable<'a>, Offset16>, Never> {
        let Ok(offsets) = self.ligature_anchor_offsets();
        ArrayOfNullableOffsets::new(offsets, data, ())
    }
}

impl ReadArgs for ComponentRecord<'_> {
    type Arguments = u16;
}

impl ComputeSize for ComponentRecord<'_> {

    fn compute_size(args: &u16) -> Result<u64, ReadError> {
        let mark_class_count = *args;
        let Ok(value_70) = transforms::to_u64(mark_class_count);

        Ok(value_70.saturating_mul(Offset16::RAW_BYTE_LEN))
    }
}

impl<'a> FontReadWithArgs<'a> for ComponentRecord<'a> {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let Ok(value_71) = data.cursor();

        let mut cursor = value_71;
        let mark_class_count = *args;
        let Ok(value_72) = transforms::to_u64(mark_class_count);

        let ligature_anchor_offsets = cursor.read_array(value_72)?;


        Ok(Self {

            ligature_anchor_offsets,

        })
    }
}

impl<'a> ComponentRecord<'a> {
    pub fn read(data: FontData<'a>, mark_class_count: u16) -> Result<Self, ReadError> {
        let args = mark_class_count;
        Self::read_with_args(data, &args)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for ComponentRecord<'a> {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "ComponentRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "ligature_anchor_offsets", value: FieldType::from(self.ligature_anchors(_data)) }),
                _ => None,
            }),
            data,
        })
    }
}

impl Format<u16> for MarkMarkPosFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for MarkMarkPosFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(mark2_array_offset_byte_range) = self.mark2_array_offset_byte_range();

        Ok(0..mark2_array_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for MarkMarkPosFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_73) = data.len();

        match ::core::primitive::bool::then_some(value_73 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct MarkMarkPosFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> MarkMarkPosFormat1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn pos_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.pos_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn mark1_coverage_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.mark1_coverage_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn mark1_coverage(&self) -> Result<CoverageTable<'a>, ReadError> {
        let data = self.data;
        let Ok(mark1_coverage_offset) = self.mark1_coverage_offset();

        mark1_coverage_offset.resolve(data)
    }

    pub fn mark2_coverage_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.mark2_coverage_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn mark2_coverage(&self) -> Result<CoverageTable<'a>, ReadError> {
        let data = self.data;
        let Ok(mark2_coverage_offset) = self.mark2_coverage_offset();

        mark2_coverage_offset.resolve(data)
    }

    pub fn mark_class_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.mark_class_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn mark1_array_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.mark1_array_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn mark1_array(&self) -> Result<MarkArray<'a>, ReadError> {
        let data = self.data;
        let Ok(mark1_array_offset) = self.mark1_array_offset();

        mark1_array_offset.resolve(data)
    }

    pub fn mark2_array_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.mark2_array_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn mark2_array(&self) -> Result<Mark2Array<'a>, ReadError> {
        let data = self.data;
        let Ok(args) = self.mark_class_count();
        let Ok(mark2_array_offset) = self.mark2_array_offset();

        mark2_array_offset.resolve_with_args(data, &args)
    }

    pub fn pos_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn mark1_coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.pos_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn mark2_coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.mark1_coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn mark_class_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.mark2_coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn mark1_array_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.mark_class_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn mark2_array_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.mark1_array_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    MarkMarkPosFormat1::MIN_SIZE
); held }));

impl Default for MarkMarkPosFormat1<'_> {
    fn default() -> Self {
        let Ok(value_74) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_74,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for MarkMarkPosFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("MarkMarkPosFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "pos_format", value: FieldType::from(self.pos_format()) }),
            1u64 => Some(Field { name: "mark1_coverage_offset", value: FieldType::from(FieldType::offset(self.mark1_coverage_offset(), (self.mark1_coverage()).map(Some))) }),
            2u64 => Some(Field { name: "mark2_coverage_offset", value: FieldType::from(FieldType::offset(self.mark2_coverage_offset(), (self.mark2_coverage()).map(Some))) }),
            3u64 => Some(Field { name: "mark_class_count", value: FieldType::from(self.mark_class_count()) }),
            4u64 => Some(Field { name: "mark1_array_offset", value: FieldType::from(FieldType::offset(self.mark1_array_offset(), (self.mark1_array()).map(Some))) }),
            5u64 => Some(Field { name: "mark2_array_offset", value: FieldType::from(FieldType::offset(self.mark2_array_offset(), (self.mark2_array()).map(Some))) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for MarkMarkPosFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for Mark2Array<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(mark2_records_byte_range) = self.mark2_records_byte_range();

        Ok(0..mark2_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl ReadArgs for Mark2Array<'_> {
    type Arguments = u16;
}

impl<'a> FontReadWithArgs<'a> for Mark2Array<'a> {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let mark_class_count = *args;

        let Ok(value_75) = data.len();

        match ::core::primitive::bool::then_some(value_75 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self {
            data,
            mark_class_count,
        })
    }
}

impl<'a> Mark2Array<'a> {
    pub fn read(data: FontData<'a>, mark_class_count: u16) -> Result<Self, ReadError> {
        let args = mark_class_count;
        Self::read_with_args(data, &args)
    }
}

#[derive(Clone)]
pub struct Mark2Array<'a> {
    data: FontData<'a>,
    mark_class_count: u16,
}

impl<'a> Mark2Array<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn mark2_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.mark2_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn mark2_records(&self) -> Result<ComputedArray<'a, Mark2Record<'a>>, Never> {
        let Ok(range) = self.mark2_records_byte_range();

        self.data.read_with_args_or_default(range, &({ let Ok(explicit_value) = self.mark_class_count(); explicit_value }))
    }

    pub(crate) fn mark_class_count(&self) -> Result<u16, Never> {
        Ok(self.mark_class_count)
    }

    pub fn mark2_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn mark2_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(mark2_count) = self.mark2_count();
        let Ok(previous) = self.mark2_count_byte_range();
        let start = previous.end;
        let Ok(value_76) = transforms::to_u64(mark2_count);

        let Ok(mark_class_count) = self.mark_class_count();
        let Ok(size_or_zero) = transforms::size_or_zero(<Mark2Record as ComputeSize>::compute_size(&mark_class_count));
        let end = start.saturating_add(value_76.saturating_mul(
                size_or_zero,
            ));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Mark2Array::MIN_SIZE); held }));

impl Default for Mark2Array<'_> {
    fn default() -> Self {
        let Ok(value_77) = FontData::default_table_data();

        Self {
            data: value_77,
            mark_class_count: Default::default(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Mark2Array<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Mark2Array")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "mark2_count", value: FieldType::from(self.mark2_count()) }),
            1u64 => {
                let Ok(value_78) = self.offset_data();

                let Ok(mark2_records) = self.mark2_records();

                Some(Field { name: "mark2_records", value: FieldType::from(traversal::FieldType::computed_array(
                    "Mark2Record",
                    mark2_records,
                    value_78,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Mark2Array<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug)]
pub struct Mark2Record<'a> {
    pub mark2_anchor_offsets: &'a [BigEndian<Nullable<Offset16>>],
}

impl<'a> Mark2Record<'a> {
    pub fn mark2_anchor_offsets(&self) -> Result<&'a [BigEndian<Nullable<Offset16>>], Never> {
        Ok(self.mark2_anchor_offsets)
    }

    pub fn mark2_anchors(
        &self,
        data: FontData<'a>,
    ) -> Result<ArrayOfNullableOffsets<'a, AnchorTable<'a>, Offset16>, Never> {
        let Ok(offsets) = self.mark2_anchor_offsets();
        ArrayOfNullableOffsets::new(offsets, data, ())
    }
}

impl ReadArgs for Mark2Record<'_> {
    type Arguments = u16;
}

impl ComputeSize for Mark2Record<'_> {

    fn compute_size(args: &u16) -> Result<u64, ReadError> {
        let mark_class_count = *args;
        let Ok(value_79) = transforms::to_u64(mark_class_count);

        Ok(value_79.saturating_mul(Offset16::RAW_BYTE_LEN))
    }
}

impl<'a> FontReadWithArgs<'a> for Mark2Record<'a> {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let Ok(value_80) = data.cursor();

        let mut cursor = value_80;
        let mark_class_count = *args;
        let Ok(value_81) = transforms::to_u64(mark_class_count);

        let mark2_anchor_offsets = cursor.read_array(value_81)?;


        Ok(Self {

            mark2_anchor_offsets,

        })
    }
}

impl<'a> Mark2Record<'a> {
    pub fn read(data: FontData<'a>, mark_class_count: u16) -> Result<Self, ReadError> {
        let args = mark_class_count;
        Self::read_with_args(data, &args)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for Mark2Record<'a> {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "Mark2Record",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "mark2_anchor_offsets", value: FieldType::from(self.mark2_anchors(_data)) }),
                _ => None,
            }),
            data,
        })
    }
}

impl Format<u16> for ExtensionPosFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl Discriminant for ExtensionPosFormat1<'_, ()> {
    fn read_discriminant(data: FontData<'_>) -> Result<u16, ReadError> {
        data.read_at(u16::RAW_BYTE_LEN)
    }
}

impl<'a, T> MinByteRange<'a> for ExtensionPosFormat1<'a, T> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(extension_offset_byte_range) = self.extension_offset_byte_range();

        Ok(0..extension_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a, T> FontRead<'a> for ExtensionPosFormat1<'a, T> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_82) = data.len();

        match ::core::primitive::bool::then_some(value_82 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self {
            data,
            offset_type: std::marker::PhantomData,
        })
    }
}

impl<'a, T> ExtensionPosFormat1<'a, T> {

}

#[derive(Clone)]
pub struct ExtensionPosFormat1<'a, T = ()> {
    data: FontData<'a>,
    offset_type: std::marker::PhantomData<*const T>,
}

impl<'a, T> ExtensionPosFormat1<'a, T> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + Offset32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn pos_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.pos_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn extension_lookup_type(&self) -> Result<u16, Never> {
        let Ok(range) = self.extension_lookup_type_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn extension_offset(&self) -> Result<Offset32, Never> {
        let Ok(range) = self.extension_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn extension(&self) -> Result<T, ReadError>
    where
        T: FontRead<'a>,
    {
        let data = self.data;
        let Ok(extension_offset) = self.extension_offset();

        extension_offset.resolve(data)
    }

    pub fn pos_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn extension_lookup_type_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.pos_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn extension_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.extension_lookup_type_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    ExtensionPosFormat1::<()>::MIN_SIZE
); held }));

impl<T> Default for ExtensionPosFormat1<'_, T> {
    fn default() -> Self {
        let Ok(value_83) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_83,
            offset_type: std::marker::PhantomData,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a, T: FontRead<'a> + SomeTable<'a> + 'a> SomeTable<'a> for ExtensionPosFormat1<'a, T> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ExtensionPosFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "pos_format", value: FieldType::from(self.pos_format()) }),
            1u64 => Some(Field { name: "extension_lookup_type", value: FieldType::from(self.extension_lookup_type()) }),
            2u64 => Some(Field { name: "extension_offset", value: FieldType::from(FieldType::offset(self.extension_offset(), (self.extension()).map(Some))) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a, T: FontRead<'a> + SomeTable<'a> + 'a> std::fmt::Debug for ExtensionPosFormat1<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

pub enum ExtensionSubtable<'a> {
    Single(ExtensionPosFormat1<'a, SinglePos<'a>>),
    Pair(ExtensionPosFormat1<'a, PairPos<'a>>),
    Cursive(ExtensionPosFormat1<'a, CursivePosFormat1<'a>>),
    MarkToBase(ExtensionPosFormat1<'a, MarkBasePosFormat1<'a>>),
    MarkToLig(ExtensionPosFormat1<'a, MarkLigPosFormat1<'a>>),
    MarkToMark(ExtensionPosFormat1<'a, MarkMarkPosFormat1<'a>>),
    Contextual(ExtensionPosFormat1<'a, PositionSequenceContext<'a>>),
    ChainContextual(ExtensionPosFormat1<'a, PositionChainContext<'a>>),
}

impl Default for ExtensionSubtable<'_> {
    fn default() -> Self {
        Self::Single(Default::default())
    }
}

impl<'a> FontRead<'a> for ExtensionSubtable<'a> {
    fn read(bytes: FontData<'a>) -> Result<Self, ReadError> {
        let discriminant = ExtensionPosFormat1::read_discriminant(bytes)?;

        match discriminant {
            1 => {
                let read = FontRead::read(bytes)?;

                Ok(ExtensionSubtable::Single(read))
            }
            2 => {
                let read = FontRead::read(bytes)?;

                Ok(ExtensionSubtable::Pair(read))
            }
            3 => {
                let read = FontRead::read(bytes)?;

                Ok(ExtensionSubtable::Cursive(read))
            }
            4 => {
                let read = FontRead::read(bytes)?;

                Ok(ExtensionSubtable::MarkToBase(read))
            }
            5 => {
                let read = FontRead::read(bytes)?;

                Ok(ExtensionSubtable::MarkToLig(read))
            }
            6 => {
                let read = FontRead::read(bytes)?;

                Ok(ExtensionSubtable::MarkToMark(read))
            }
            7 => {
                let read = FontRead::read(bytes)?;

                Ok(ExtensionSubtable::Contextual(read))
            }
            8 => {
                let read = FontRead::read(bytes)?;

                Ok(ExtensionSubtable::ChainContextual(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u16>>::from(other))),
        }
    }
}

impl<'a> ExtensionSubtable<'a> {

}

#[cfg(feature = "experimental_traverse")]
impl<'a> ExtensionSubtable<'a> {
    fn dyn_inner(&self) -> Result<&(dyn SomeTable<'a> + 'a), Never> {
        Ok(match self {
            ExtensionSubtable::Single(table) => table,
            ExtensionSubtable::Pair(table) => table,
            ExtensionSubtable::Cursive(table) => table,
            ExtensionSubtable::MarkToBase(table) => table,
            ExtensionSubtable::MarkToLig(table) => table,
            ExtensionSubtable::MarkToMark(table) => table,
            ExtensionSubtable::Contextual(table) => table,
            ExtensionSubtable::ChainContextual(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ExtensionSubtable<'a> {
    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }

    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for ExtensionSubtable<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}
