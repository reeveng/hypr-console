

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Cpal<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(color_record_indices_byte_range) = self.color_record_indices_byte_range();

        Ok(0..color_record_indices_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Cpal<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"CPAL"); value });
}

impl<'a> FontRead<'a> for Cpal<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Cpal<'a> {
    data: FontData<'a>,
}

impl<'a> Cpal<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<u16, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_palette_entries(&self) -> Result<u16, Never> {
        let Ok(range) = self.num_palette_entries_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_palettes(&self) -> Result<u16, Never> {
        let Ok(range) = self.num_palettes_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_color_records(&self) -> Result<u16, Never> {
        let Ok(range) = self.num_color_records_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn color_records_array_offset(&self) -> Result<Nullable<Offset32>, Never> {
        let Ok(range) = self.color_records_array_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn color_records_array(&self) -> Result<Option<&'a [ColorRecord]>, ReadError> {
        let data = self.data;
        let Ok(args) = self.num_color_records();
        let Ok(color_records_array_offset) = self.color_records_array_offset();

        color_records_array_offset
            .resolve_with_args(data, &args)
    }

    pub fn color_record_indices(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.color_record_indices_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn palette_types_array_offset(&self) -> Result<Option<Nullable<Offset32>>, Never> {
        let Ok(range) = self.palette_types_array_offset_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn palette_types_array(&self) -> Result<Option<&'a [BigEndian<PaletteType>]>, ReadError> {
        let data = self.data;
        let Ok(args) = self.num_palettes();
        let Ok(read_palette_types_array_offset) = self.palette_types_array_offset();

        match read_palette_types_array_offset { Some(x) => x.resolve_with_args(data, &args), None => Ok(None) }
    }

    pub fn palette_labels_array_offset(&self) -> Result<Option<Nullable<Offset32>>, Never> {
        let Ok(range) = self.palette_labels_array_offset_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn palette_labels_array(&self) -> Result<Option<&'a [BigEndian<NameId>]>, ReadError> {
        let data = self.data;
        let Ok(args) = self.num_palettes();
        let Ok(read_palette_labels_array_offset) = self.palette_labels_array_offset();

        match read_palette_labels_array_offset { Some(x) => x.resolve_with_args(data, &args), None => Ok(None) }
    }

    pub fn palette_entry_labels_array_offset(&self) -> Result<Option<Nullable<Offset32>>, Never> {
        let Ok(range) = self.palette_entry_labels_array_offset_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn palette_entry_labels_array(&self) -> Result<Option<&'a [BigEndian<NameId>]>, ReadError> {
        let data = self.data;
        let Ok(args) = self.num_palette_entries();
        let Ok(read_palette_entry_labels_array_offset) = self.palette_entry_labels_array_offset();

        match read_palette_entry_labels_array_offset { Some(x) => x.resolve_with_args(data, &args), None => Ok(None) }
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_palette_entries_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_palettes_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.num_palette_entries_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_color_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.num_palettes_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn color_records_array_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.num_color_records_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn color_record_indices_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_palettes) = self.num_palettes();
        let Ok(previous) = self.color_records_array_offset_byte_range();
        let start = previous.end;
        let Ok(value_2) = transforms::to_u64(num_palettes);

        let end = start.saturating_add(value_2.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn palette_types_array_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.color_record_indices_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility(1u16);

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(Offset32::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }

    pub fn palette_labels_array_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.palette_types_array_offset_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility(1u16);

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(Offset32::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }

    pub fn palette_entry_labels_array_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.palette_labels_array_offset_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility(1u16);

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(Offset32::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Cpal::MIN_SIZE); held }));

impl Default for Cpal<'_> {
    fn default() -> Self {
        let Ok(value_6) = FontData::default_table_data();

        Self {
            data: value_6,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Cpal<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Cpal")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "num_palette_entries", value: FieldType::from(self.num_palette_entries()) }),
            2u64 => Some(Field { name: "num_palettes", value: FieldType::from(self.num_palettes()) }),
            3u64 => Some(Field { name: "num_color_records", value: FieldType::from(self.num_color_records()) }),
            4u64 => {
                let Ok(value_7) = self.offset_data();

                Some(Field { name: "color_records_array_offset", value: FieldType::from(traversal::FieldType::offset_to_array_of_records(
                    self.color_records_array_offset(),
                    self.color_records_array(),
                    stringify!(ColorRecord),
                    value_7,
                )) })
            },
            5u64 => Some(Field { name: "color_record_indices", value: FieldType::from(self.color_record_indices()) }),
            6u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility(1u16);
                let Ok(palette_types_array_offset) = self.palette_types_array_offset();

                match compatibility {
                    Compatibility::Compatible => palette_types_array_offset.map(|palette_types_array_offset_2| Field { name: "palette_types_array_offset", value: FieldType::from(FieldType::offset_to_array_of_scalars(
                        palette_types_array_offset_2,
                        self.palette_types_array(),
                    )) }),
                    Compatibility::Incompatible => None,
                }
            },
            7u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility(1u16);
                let Ok(palette_labels_array_offset) = self.palette_labels_array_offset();

                match compatibility {
                    Compatibility::Compatible => palette_labels_array_offset.map(|palette_labels_array_offset_2| Field { name: "palette_labels_array_offset", value: FieldType::from(FieldType::offset_to_array_of_scalars(
                        palette_labels_array_offset_2,
                        self.palette_labels_array(),
                    )) }),
                    Compatibility::Incompatible => None,
                }
            },
            8u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility(1u16);
                let Ok(palette_entry_labels_array_offset) = self.palette_entry_labels_array_offset();

                match compatibility {
                    Compatibility::Compatible => palette_entry_labels_array_offset.map(|palette_entry_labels_array_offset_2| Field { name: "palette_entry_labels_array_offset", value: FieldType::from(FieldType::offset_to_array_of_scalars(
                        palette_entry_labels_array_offset_2,
                        self.palette_entry_labels_array(),
                    )) }),
                    Compatibility::Incompatible => None,
                }
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Cpal<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck :: AnyBitPattern)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct PaletteType {
    bits: u32,
}

impl PaletteType {
    pub const USABLE_WITH_LIGHT_BACKGROUND: Self = Self { bits: 0x0001 };

    pub const USABLE_WITH_DARK_BACKGROUND: Self = Self { bits: 0x0002 };
}

impl PaletteType {
    const KNOWN: u32 = Self::USABLE_WITH_LIGHT_BACKGROUND.bits
        | Self::USABLE_WITH_DARK_BACKGROUND.bits;

    #[inline]
    pub const fn empty() -> Result<Self, Never> {
        Ok(Self { bits: 0 })
    }

    #[inline]
    pub const fn all() -> Result<Self, Never> {
        Ok(Self { bits: Self::KNOWN })
    }

    #[inline]
    pub const fn bits(&self) -> Result<u32, Never> {
        Ok(self.bits)
    }

    #[inline]
    pub const fn from_bits(bits: u32) -> Result<Option<Self>, Never> {
        Ok(match bits & !Self::KNOWN {
            0 => Some(Self { bits }),
            _unknown => None,
        })
    }

    #[inline]
    pub const fn from_bits_truncate(bits: u32) -> Result<Self, Never> {
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

impl std::ops::BitOr for PaletteType {
    type Output = Self;

    #[inline]
    fn bitor(self, other: PaletteType) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl std::ops::BitOrAssign for PaletteType {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.bits |= other.bits;
    }
}

impl std::ops::BitXor for PaletteType {
    type Output = Self;

    #[inline]
    fn bitxor(self, other: Self) -> Self {
        Self {
            bits: self.bits ^ other.bits,
        }
    }
}

impl std::ops::BitXorAssign for PaletteType {
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        self.bits ^= other.bits;
    }
}

impl std::ops::BitAnd for PaletteType {
    type Output = Self;

    #[inline]
    fn bitand(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }
}

impl std::ops::BitAndAssign for PaletteType {
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        self.bits &= other.bits;
    }
}

impl std::ops::Sub for PaletteType {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }
}

impl std::ops::SubAssign for PaletteType {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.bits &= !other.bits;
    }
}

impl std::ops::Not for PaletteType {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        Self { bits: !self.bits & Self::KNOWN }
    }
}

impl std::fmt::Debug for PaletteType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let members: &[(&str, Self)] = &[
            (
                "USABLE_WITH_LIGHT_BACKGROUND",
                Self::USABLE_WITH_LIGHT_BACKGROUND,
            ),
            (
                "USABLE_WITH_DARK_BACKGROUND",
                Self::USABLE_WITH_DARK_BACKGROUND,
            ),
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

impl std::fmt::Binary for PaletteType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Binary::fmt(&self.bits, f)
    }
}

impl std::fmt::Octal for PaletteType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Octal::fmt(&self.bits, f)
    }
}

impl std::fmt::LowerHex for PaletteType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.bits, f)
    }
}

impl std::fmt::UpperHex for PaletteType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.bits, f)
    }
}

impl font_types::Scalar for PaletteType {
    type Raw = <u32 as font_types::Scalar>::Raw;

    fn to_raw(self) -> std::result::Result<Self::Raw, console_core_never::Never> {
        self.bits.to_raw()
    }

    fn from_raw(raw: Self::Raw) -> std::result::Result<Self, console_core_never::Never> {
        let Ok(bits) = <u32>::from_raw(raw);

        Self::from_bits_truncate(bits)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> From<PaletteType> for FieldType<'a> {
    fn from(src: PaletteType) -> FieldType<'a> {
        <crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u32>>::from(src.bits)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct ColorRecord {
    pub blue: u8,
    pub green: u8,
    pub red: u8,
    pub alpha: u8,
}

const _: () = {
  assert!(core::mem::size_of::<ColorRecord>() == 4);
  assert!(core::mem::align_of::<ColorRecord>() == 1);
  assert!(core::mem::offset_of!(ColorRecord, blue) == 0);
  assert!(core::mem::offset_of!(ColorRecord, green) == 1);
  assert!(core::mem::offset_of!(ColorRecord, red) == 2);
  assert!(core::mem::offset_of!(ColorRecord, alpha) == 3);
};

impl ColorRecord {
    pub fn blue(&self) -> Result<u8, Never> {
        Ok(self.blue)
    }

    pub fn green(&self) -> Result<u8, Never> {
        Ok(self.green)
    }

    pub fn red(&self) -> Result<u8, Never> {
        Ok(self.red)
    }

    pub fn alpha(&self) -> Result<u8, Never> {
        Ok(self.alpha)
    }
}

impl FixedSize for ColorRecord {
    const RAW_BYTE_LEN: u64 =
        u8::RAW_BYTE_LEN + u8::RAW_BYTE_LEN + u8::RAW_BYTE_LEN + u8::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for ColorRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "ColorRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "blue", value: FieldType::from(self.blue()) }),
                1u64 => Some(Field { name: "green", value: FieldType::from(self.green()) }),
                2u64 => Some(Field { name: "red", value: FieldType::from(self.red()) }),
                3u64 => Some(Field { name: "alpha", value: FieldType::from(self.alpha()) }),
                _ => None,
            }),
            data,
        })
    }
}
