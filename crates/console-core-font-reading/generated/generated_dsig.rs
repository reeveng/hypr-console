

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Dsig<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(signature_records_byte_range) = self.signature_records_byte_range();

        Ok(0..signature_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Dsig<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"DSIG"); value });
}

impl<'a> FontRead<'a> for Dsig<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Dsig<'a> {
    data: FontData<'a>,
}

impl<'a> Dsig<'a> {
    pub const MIN_SIZE: u64 =
        (u32::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + PermissionFlags::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<u32, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_signatures(&self) -> Result<u16, Never> {
        let Ok(range) = self.num_signatures_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn flags(&self) -> Result<PermissionFlags, Never> {
        let Ok(range) = self.flags_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn signature_records(&self) -> Result<&'a [SignatureRecord], Never> {
        let Ok(range) = self.signature_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_signatures_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn flags_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.num_signatures_byte_range();
        let start = previous.end;
        let end = start.saturating_add(PermissionFlags::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn signature_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_signatures) = self.num_signatures();
        let Ok(previous) = self.flags_byte_range();
        let start = previous.end;
        let Ok(value_2) = transforms::to_u64(num_signatures);

        let end = start.saturating_add(value_2.saturating_mul(SignatureRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Dsig::MIN_SIZE); held }));

impl Default for Dsig<'_> {
    fn default() -> Self {
        let Ok(value_3) = FontData::default_table_data();

        Self {
            data: value_3,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Dsig<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Dsig")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "num_signatures", value: FieldType::from(self.num_signatures()) }),
            2u64 => Some(Field { name: "flags", value: FieldType::from(self.flags()) }),
            3u64 => {
                let Ok(value_4) = self.offset_data();

                Some(Field { name: "signature_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(SignatureRecord),
                    { let Ok(explicit_value) = self.signature_records(); explicit_value },
                    value_4,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Dsig<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck :: AnyBitPattern)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct PermissionFlags {
    bits: u16,
}

impl PermissionFlags {
    pub const CANNOT_BE_RESIGNED: Self = Self {
        bits: 0b0000_0000_0000_0001,
    };
}

impl PermissionFlags {
    const KNOWN: u16 = Self::CANNOT_BE_RESIGNED.bits;

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

impl std::ops::BitOr for PermissionFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, other: PermissionFlags) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl std::ops::BitOrAssign for PermissionFlags {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.bits |= other.bits;
    }
}

impl std::ops::BitXor for PermissionFlags {
    type Output = Self;

    #[inline]
    fn bitxor(self, other: Self) -> Self {
        Self {
            bits: self.bits ^ other.bits,
        }
    }
}

impl std::ops::BitXorAssign for PermissionFlags {
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        self.bits ^= other.bits;
    }
}

impl std::ops::BitAnd for PermissionFlags {
    type Output = Self;

    #[inline]
    fn bitand(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }
}

impl std::ops::BitAndAssign for PermissionFlags {
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        self.bits &= other.bits;
    }
}

impl std::ops::Sub for PermissionFlags {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }
}

impl std::ops::SubAssign for PermissionFlags {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.bits &= !other.bits;
    }
}

impl std::ops::Not for PermissionFlags {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        Self { bits: !self.bits & Self::KNOWN }
    }
}

impl std::fmt::Debug for PermissionFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let members: &[(&str, Self)] = &[("CANNOT_BE_RESIGNED", Self::CANNOT_BE_RESIGNED)];
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

impl std::fmt::Binary for PermissionFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Binary::fmt(&self.bits, f)
    }
}

impl std::fmt::Octal for PermissionFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Octal::fmt(&self.bits, f)
    }
}

impl std::fmt::LowerHex for PermissionFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.bits, f)
    }
}

impl std::fmt::UpperHex for PermissionFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.bits, f)
    }
}

impl font_types::Scalar for PermissionFlags {
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
impl<'a> From<PermissionFlags> for FieldType<'a> {
    fn from(src: PermissionFlags) -> FieldType<'a> {
        <crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u16>>::from(src.bits)
    }
}

#[derive(Clone, Debug, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct SignatureRecord {
    pub format: BigEndian<u32>,
    pub length: BigEndian<u32>,
    pub signature_block_offset: BigEndian<Offset32>,
}

const _: () = {
  assert!(core::mem::size_of::<SignatureRecord>() == 12);
  assert!(core::mem::align_of::<SignatureRecord>() == 1);
  assert!(core::mem::offset_of!(SignatureRecord, format) == 0);
  assert!(core::mem::offset_of!(SignatureRecord, length) == 4);
  assert!(core::mem::offset_of!(SignatureRecord, signature_block_offset) == 8);
};

impl SignatureRecord {
    pub fn format(&self) -> Result<u32, Never> {
        self.format.get()
    }

    pub fn length(&self) -> Result<u32, Never> {
        self.length.get()
    }

    pub fn signature_block_offset(&self) -> Result<Offset32, Never> {
        self.signature_block_offset.get()
    }
}

impl FixedSize for SignatureRecord {
    const RAW_BYTE_LEN: u64 = u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN + Offset32::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for SignatureRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "SignatureRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
                1u64 => Some(Field { name: "length", value: FieldType::from(self.length()) }),
                2u64 => Some(Field { name: "signature_block_offset", value: FieldType::from(FieldType::offset(self.signature_block_offset(), (self.signature_block(_data)).map(Some))) }),
                _ => None,
            }),
            data,
        })
    }
}

impl<'a> MinByteRange<'a> for SignatureBlockFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(signature_byte_range) = self.signature_byte_range();

        Ok(0..signature_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for SignatureBlockFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_11) = data.len();

        match ::core::primitive::bool::then_some(value_11 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct SignatureBlockFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> SignatureBlockFormat1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn signature_length(&self) -> Result<u32, Never> {
        let Ok(range) = self.signature_length_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn signature(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.signature_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn _reserved1_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn _reserved2_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self._reserved1_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn signature_length_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self._reserved2_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn signature_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(signature_length) = self.signature_length();
        let Ok(previous) = self.signature_length_byte_range();
        let start = previous.end;
        let Ok(value_12) = transforms::to_u64(signature_length);

        let end = start.saturating_add(value_12.saturating_mul(u8::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    SignatureBlockFormat1::MIN_SIZE
); held }));

impl Default for SignatureBlockFormat1<'_> {
    fn default() -> Self {
        let Ok(value_13) = FontData::default_table_data();

        Self {
            data: value_13,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for SignatureBlockFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("SignatureBlockFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "signature_length", value: FieldType::from(self.signature_length()) }),
            1u64 => Some(Field { name: "signature", value: FieldType::from(self.signature()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for SignatureBlockFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
