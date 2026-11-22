

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Gasp<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(gasp_ranges_byte_range) = self.gasp_ranges_byte_range();

        Ok(0..gasp_ranges_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Gasp<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"gasp"); value });
}

impl<'a> FontRead<'a> for Gasp<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Gasp<'a> {
    data: FontData<'a>,
}

impl<'a> Gasp<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<u16, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_ranges(&self) -> Result<u16, Never> {
        let Ok(range) = self.num_ranges_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn gasp_ranges(&self) -> Result<&'a [GaspRange], Never> {
        let Ok(range) = self.gasp_ranges_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_ranges_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn gasp_ranges_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_ranges) = self.num_ranges();
        let Ok(previous) = self.num_ranges_byte_range();
        let start = previous.end;
        let Ok(value_2) = transforms::to_u64(num_ranges);

        let end =
            start.saturating_add(value_2.saturating_mul(GaspRange::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Gasp::MIN_SIZE); held }));

impl Default for Gasp<'_> {
    fn default() -> Self {
        let Ok(value_3) = FontData::default_table_data();

        Self {
            data: value_3,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Gasp<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Gasp")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "num_ranges", value: FieldType::from(self.num_ranges()) }),
            2u64 => {
                let Ok(value_4) = self.offset_data();

                Some(Field { name: "gasp_ranges", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(GaspRange),
                    { let Ok(explicit_value) = self.gasp_ranges(); explicit_value },
                    value_4,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Gasp<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct GaspRange {
    pub range_max_ppem: BigEndian<u16>,
    pub range_gasp_behavior: BigEndian<GaspRangeBehavior>,
}

const _: () = {
  assert!(core::mem::size_of::<GaspRange>() == 4);
  assert!(core::mem::align_of::<GaspRange>() == 1);
  assert!(core::mem::offset_of!(GaspRange, range_max_ppem) == 0);
  assert!(core::mem::offset_of!(GaspRange, range_gasp_behavior) == 2);
};

impl GaspRange {
    pub fn range_max_ppem(&self) -> Result<u16, Never> {
        self.range_max_ppem.get()
    }

    pub fn range_gasp_behavior(&self) -> Result<GaspRangeBehavior, Never> {
        self.range_gasp_behavior.get()
    }
}

impl FixedSize for GaspRange {
    const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN + GaspRangeBehavior::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for GaspRange {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "GaspRange",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "range_max_ppem", value: FieldType::from(self.range_max_ppem()) }),
                1u64 => Some(Field { name: "range_gasp_behavior", value: FieldType::from(self.range_gasp_behavior()) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck :: AnyBitPattern)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct GaspRangeBehavior {
    bits: u16,
}

impl GaspRangeBehavior {
    pub const GASP_GRIDFIT: Self = Self { bits: 0x0001 };

    pub const GASP_DOGRAY: Self = Self { bits: 0x0002 };

    pub const GASP_SYMMETRIC_GRIDFIT: Self = Self { bits: 0x0004 };

    pub const GASP_SYMMETRIC_SMOOTHING: Self = Self { bits: 0x0008 };
}

impl GaspRangeBehavior {
    const KNOWN: u16 = Self::GASP_GRIDFIT.bits
        | Self::GASP_DOGRAY.bits
        | Self::GASP_SYMMETRIC_GRIDFIT.bits
        | Self::GASP_SYMMETRIC_SMOOTHING.bits;

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

impl std::ops::BitOr for GaspRangeBehavior {
    type Output = Self;

    #[inline]
    fn bitor(self, other: GaspRangeBehavior) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl std::ops::BitOrAssign for GaspRangeBehavior {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.bits |= other.bits;
    }
}

impl std::ops::BitXor for GaspRangeBehavior {
    type Output = Self;

    #[inline]
    fn bitxor(self, other: Self) -> Self {
        Self {
            bits: self.bits ^ other.bits,
        }
    }
}

impl std::ops::BitXorAssign for GaspRangeBehavior {
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        self.bits ^= other.bits;
    }
}

impl std::ops::BitAnd for GaspRangeBehavior {
    type Output = Self;

    #[inline]
    fn bitand(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }
}

impl std::ops::BitAndAssign for GaspRangeBehavior {
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        self.bits &= other.bits;
    }
}

impl std::ops::Sub for GaspRangeBehavior {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }
}

impl std::ops::SubAssign for GaspRangeBehavior {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.bits &= !other.bits;
    }
}

impl std::ops::Not for GaspRangeBehavior {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        Self { bits: !self.bits & Self::KNOWN }
    }
}

impl std::fmt::Debug for GaspRangeBehavior {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let members: &[(&str, Self)] = &[
            ("GASP_GRIDFIT", Self::GASP_GRIDFIT),
            ("GASP_DOGRAY", Self::GASP_DOGRAY),
            ("GASP_SYMMETRIC_GRIDFIT", Self::GASP_SYMMETRIC_GRIDFIT),
            ("GASP_SYMMETRIC_SMOOTHING", Self::GASP_SYMMETRIC_SMOOTHING),
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

impl std::fmt::Binary for GaspRangeBehavior {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Binary::fmt(&self.bits, f)
    }
}

impl std::fmt::Octal for GaspRangeBehavior {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Octal::fmt(&self.bits, f)
    }
}

impl std::fmt::LowerHex for GaspRangeBehavior {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.bits, f)
    }
}

impl std::fmt::UpperHex for GaspRangeBehavior {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.bits, f)
    }
}

impl font_types::Scalar for GaspRangeBehavior {
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
impl<'a> From<GaspRangeBehavior> for FieldType<'a> {
    fn from(src: GaspRangeBehavior) -> FieldType<'a> {
        <crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u16>>::from(src.bits)
    }
}
