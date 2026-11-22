

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for MajorMinorVersion<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(always_present_byte_range) = self.always_present_byte_range();

        Ok(0..always_present_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for MajorMinorVersion<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct MajorMinorVersion<'a> {
    data: FontData<'a>,
}

impl<'a> MajorMinorVersion<'a> {
    pub const MIN_SIZE: u64 = (MajorMinor::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<MajorMinor, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn always_present(&self) -> Result<u16, Never> {
        let Ok(range) = self.always_present_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn if_11(&self) -> Result<Option<u16>, Never> {
        let Ok(range) = self.if_11_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn if_20(&self) -> Result<Option<u32>, Never> {
        let Ok(range) = self.if_20_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(MajorMinor::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn always_present_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn if_11_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.always_present_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility((1u16, 1u16));

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(u16::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }

    pub fn if_20_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.if_11_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility((2u16, 0u16));

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(u32::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    MajorMinorVersion::MIN_SIZE
); held }));

impl Default for MajorMinorVersion<'_> {
    fn default() -> Self {
        let Ok(value_4) = FontData::default_table_data();

        Self {
            data: value_4,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for MajorMinorVersion<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("MajorMinorVersion")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "always_present", value: FieldType::from(self.always_present()) }),
            2u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility((1u16, 1u16));
                let Ok(if_11) = self.if_11();

                match compatibility {
                    Compatibility::Compatible => if_11.map(|if_11_2| Field { name: "if_11", value: FieldType::from(if_11_2) }),
                    Compatibility::Incompatible => None,
                }
            },
            3u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility((2u16, 0u16));
                let Ok(if_20) = self.if_20();

                match compatibility {
                    Compatibility::Compatible => if_20.map(|if_20_2| Field { name: "if_20", value: FieldType::from(if_20_2) }),
                    Compatibility::Incompatible => None,
                }
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for MajorMinorVersion<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck :: AnyBitPattern)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct GotFlags {
    bits: u16,
}

impl GotFlags {
    pub const FOO: Self = Self { bits: 0x0001 };

    pub const BAR: Self = Self { bits: 0x0002 };

    pub const BAZ: Self = Self { bits: 0x0004 };
}

impl GotFlags {
    const KNOWN: u16 = Self::FOO.bits
        | Self::BAR.bits
        | Self::BAZ.bits;

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

impl std::ops::BitOr for GotFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, other: GotFlags) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl std::ops::BitOrAssign for GotFlags {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.bits |= other.bits;
    }
}

impl std::ops::BitXor for GotFlags {
    type Output = Self;

    #[inline]
    fn bitxor(self, other: Self) -> Self {
        Self {
            bits: self.bits ^ other.bits,
        }
    }
}

impl std::ops::BitXorAssign for GotFlags {
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        self.bits ^= other.bits;
    }
}

impl std::ops::BitAnd for GotFlags {
    type Output = Self;

    #[inline]
    fn bitand(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }
}

impl std::ops::BitAndAssign for GotFlags {
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        self.bits &= other.bits;
    }
}

impl std::ops::Sub for GotFlags {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }
}

impl std::ops::SubAssign for GotFlags {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.bits &= !other.bits;
    }
}

impl std::ops::Not for GotFlags {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        Self { bits: !self.bits & Self::KNOWN }
    }
}

impl std::fmt::Debug for GotFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let members: &[(&str, Self)] =
            &[("FOO", Self::FOO), ("BAR", Self::BAR), ("BAZ", Self::BAZ)];
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

impl std::fmt::Binary for GotFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Binary::fmt(&self.bits, f)
    }
}

impl std::fmt::Octal for GotFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Octal::fmt(&self.bits, f)
    }
}

impl std::fmt::LowerHex for GotFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.bits, f)
    }
}

impl std::fmt::UpperHex for GotFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.bits, f)
    }
}

impl font_types::Scalar for GotFlags {
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
impl<'a> From<GotFlags> for FieldType<'a> {
    fn from(src: GotFlags) -> FieldType<'a> {
        <crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u16>>::from(src.bits)
    }
}

impl<'a> MinByteRange<'a> for FlagDay<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(flags_byte_range) = self.flags_byte_range();

        Ok(0..flags_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for FlagDay<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_8) = data.len();

        match ::core::primitive::bool::then_some(value_8 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct FlagDay<'a> {
    data: FontData<'a>,
}

impl<'a> FlagDay<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + GotFlags::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn volume(&self) -> Result<u16, Never> {
        let Ok(range) = self.volume_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn flags(&self) -> Result<GotFlags, Never> {
        let Ok(range) = self.flags_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn foo(&self) -> Result<Option<u16>, Never> {
        let Ok(range) = self.foo_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn bar(&self) -> Result<Option<u16>, Never> {
        let Ok(range) = self.bar_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn volume_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn flags_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.volume_byte_range();
        let start = previous.end;
        let end = start.saturating_add(GotFlags::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn foo_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.flags_byte_range();
        let start = previous.end;
        let Ok(flags) = self.flags();
        let Ok(presence) = flags.contains(GotFlags::FOO);
        let end = match presence {
            Presence::Present => {
                start.saturating_add(u16::RAW_BYTE_LEN)
            }
            Presence::Absent => start,
        };
        Ok(start..end)
    }

    pub fn bar_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.foo_byte_range();
        let start = previous.end;
        let Ok(flags) = self.flags();
        let Ok(presence) = flags.contains(GotFlags::BAR);
        let end = match presence {
            Presence::Present => {
                start.saturating_add(u16::RAW_BYTE_LEN)
            }
            Presence::Absent => start,
        };
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(FlagDay::MIN_SIZE); held }));

impl Default for FlagDay<'_> {
    fn default() -> Self {
        let Ok(value_9) = FontData::default_table_data();

        Self {
            data: value_9,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for FlagDay<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("FlagDay")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "volume", value: FieldType::from(self.volume()) }),
            1u64 => Some(Field { name: "flags", value: FieldType::from(self.flags()) }),
            2u64 => {
                let Ok(flags) = self.flags();
                let Ok(presence) = flags.contains(GotFlags::FOO);
                let Ok(read_foo) = self.foo();

                match presence {
                    Presence::Present => read_foo.map(|value| Field { name: "foo", value: FieldType::from(value) }),
                    Presence::Absent => None,
                }
            },
            3u64 => {
                let Ok(flags) = self.flags();
                let Ok(presence) = flags.contains(GotFlags::BAR);
                let Ok(bar) = self.bar();

                match presence {
                    Presence::Present => bar.map(|bar_2| Field { name: "bar", value: FieldType::from(bar_2) }),
                    Presence::Absent => None,
                }
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for FlagDay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
