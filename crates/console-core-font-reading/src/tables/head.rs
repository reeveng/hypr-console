//! The head table borrows its validated wire bytes and decodes fixed-width fields.

use crate::table_ref::{MinByteRange, TopLevelTable};
use crate::{Emptiness, FontData, FontRead, Overlap, Presence, ReadError};
use console_core_never::Never;
use core::ops::Range;
use types::{BigEndian, Fixed, FixedSize, LongDateTime, MajorMinor, Scalar, Tag};

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck::AnyBitPattern)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct MacStyle {
  bits: u16,
}

impl MacStyle {
  const KNOWN: u16 = 127;

  pub const BOLD: Self = Self { bits: 0x0001 };

  pub const ITALIC: Self = Self { bits: 0x0002 };

  pub const UNDERLINE: Self = Self { bits: 0x0004 };

  pub const OUTLINE: Self = Self { bits: 0x0008 };

  pub const SHADOW: Self = Self { bits: 0x0010 };

  pub const CONDENSED: Self = Self { bits: 0x0020 };

  pub const EXTENDED: Self = Self { bits: 0x0040 };

  pub const fn empty() -> Result<Self, Never> {
    Ok(Self { bits: 0 })
  }

  pub const fn all() -> Result<Self, Never> {
    Ok(Self { bits: Self::KNOWN })
  }

  pub const fn bits(&self) -> Result<u16, Never> {
    Ok(self.bits)
  }

  pub const fn from_bits(bits: u16) -> Result<Option<Self>, Never> {
    Ok(match bits & !Self::KNOWN {
      0 => Some(Self { bits }),
      _unknown => None,
    })
  }

  pub const fn from_bits_truncate(bits: u16) -> Result<Self, Never> {
    Ok(Self {
      bits: bits & Self::KNOWN,
    })
  }

  pub const fn is_empty(&self) -> Result<Emptiness, Never> {
    Ok(match self.bits {
      0 => Emptiness::Empty,
      _set => Emptiness::NonEmpty,
    })
  }

  pub const fn intersects(&self, other: Self) -> Result<Overlap, Never> {
    Ok(match self.bits & other.bits {
      0 => Overlap::Disjoint,
      _shared => Overlap::Intersecting,
    })
  }

  pub const fn contains(&self, other: Self) -> Result<Presence, Never> {
    Ok(match other.bits & !self.bits {
      0 => Presence::Present,
      _missing => Presence::Absent,
    })
  }

  pub fn insert(&mut self, other: Self) -> Result<(), Never> {
    self.bits |= other.bits;

    Ok(())
  }

  pub fn remove(&mut self, other: Self) -> Result<(), Never> {
    self.bits &= !other.bits;

    Ok(())
  }

  pub fn toggle(&mut self, other: Self) -> Result<(), Never> {
    self.bits ^= other.bits;

    Ok(())
  }

  pub const fn intersection(self, other: Self) -> Result<Self, Never> {
    Ok(Self {
      bits: self.bits & other.bits,
    })
  }

  pub const fn union(self, other: Self) -> Result<Self, Never> {
    Ok(Self {
      bits: self.bits | other.bits,
    })
  }

  pub const fn difference(self, other: Self) -> Result<Self, Never> {
    Ok(Self {
      bits: self.bits & !other.bits,
    })
  }
}

impl core::ops::BitOr for MacStyle {
  type Output = Self;

  fn bitor(self, other: Self) -> Self {
    Self {
      bits: self.bits | other.bits,
    }
  }
}

impl core::ops::BitOrAssign for MacStyle {
  fn bitor_assign(&mut self, other: Self) {
    self.bits = self.bits | other.bits;
  }
}

impl core::ops::BitXor for MacStyle {
  type Output = Self;

  fn bitxor(self, other: Self) -> Self {
    Self {
      bits: self.bits ^ other.bits,
    }
  }
}

impl core::ops::BitXorAssign for MacStyle {
  fn bitxor_assign(&mut self, other: Self) {
    self.bits = self.bits ^ other.bits;
  }
}

impl core::ops::BitAnd for MacStyle {
  type Output = Self;

  fn bitand(self, other: Self) -> Self {
    Self {
      bits: self.bits & other.bits,
    }
  }
}

impl core::ops::BitAndAssign for MacStyle {
  fn bitand_assign(&mut self, other: Self) {
    self.bits = self.bits & other.bits;
  }
}

impl core::ops::Sub for MacStyle {
  type Output = Self;

  fn sub(self, other: Self) -> Self {
    Self {
      bits: self.bits & !other.bits,
    }
  }
}

impl core::ops::SubAssign for MacStyle {
  fn sub_assign(&mut self, other: Self) {
    self.bits &= !other.bits;
  }
}

impl core::ops::Not for MacStyle {
  type Output = Self;

  fn not(self) -> Self {
    Self {
      bits: !self.bits & Self::KNOWN,
    }
  }
}

impl Scalar for MacStyle {
  type Raw = <u16 as Scalar>::Raw;

  fn to_raw(self) -> Result<Self::Raw, Never> {
    self.bits.to_raw()
  }

  fn from_raw(raw: Self::Raw) -> Result<Self, Never> {
    let Ok(bits) = u16::from_raw(raw);

    Self::from_bits_truncate(bits)
  }
}

impl core::fmt::Debug for MacStyle {
  fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    let members = [
      ("BOLD", 0x0001u16),
      ("ITALIC", 0x0002u16),
      ("UNDERLINE", 0x0004u16),
      ("OUTLINE", 0x0008u16),
      ("SHADOW", 0x0010u16),
      ("CONDENSED", 0x0020u16),
      ("EXTENDED", 0x0040u16),
    ];
    let mut names = members
      .iter()
      .filter(|(_name, bits)| self.bits & bits == *bits)
      .map(|(name, _bits)| *name)
      .peekable();

    match names.peek() {
      None => output.write_str("(empty)"),
      Some(_) => {
        let mut separator = "";

        for name in names {
          output.write_str(separator)?;
          output.write_str(name)?;
          separator = " | ";
        }

        Ok(())
      }
    }
  }
}

impl core::fmt::Binary for MacStyle {
  fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    core::fmt::Binary::fmt(&self.bits, output)
  }
}

impl core::fmt::Octal for MacStyle {
  fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    core::fmt::Octal::fmt(&self.bits, output)
  }
}

impl core::fmt::LowerHex for MacStyle {
  fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    core::fmt::LowerHex::fmt(&self.bits, output)
  }
}

impl core::fmt::UpperHex for MacStyle {
  fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    core::fmt::UpperHex::fmt(&self.bits, output)
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> From<MacStyle> for crate::traversal::FieldType<'a> {
  fn from(flags: MacStyle) -> Self {
    <crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u16>>::from(
      flags.bits,
    )
  }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck::AnyBitPattern)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct Flags {
  bits: u16,
}

impl Flags {
  const KNOWN: u16 = 30751;

  pub const BASELINE_AT_Y_0: Self = Self { bits: 0x0001 };

  pub const LSB_AT_X_0: Self = Self { bits: 0x0002 };

  pub const INSTRUCTIONS_MAY_DEPEND_ON_POINT_SIZE: Self = Self { bits: 0x0004 };

  pub const FORCE_INTEGER_PPEM: Self = Self { bits: 0x0008 };

  pub const INSTRUCTIONS_MAY_ALTER_ADVANCE_WIDTH: Self = Self { bits: 0x0010 };

  pub const LOSSLESS_TRANSFORMED_FONT_DATA: Self = Self { bits: 0x0800 };

  pub const CONVERTED_FONT: Self = Self { bits: 0x1000 };

  pub const OPTIMIZED_FOR_CLEARTYPE: Self = Self { bits: 0x2000 };

  pub const LAST_RESORT_FONT: Self = Self { bits: 0x4000 };

  pub const fn empty() -> Result<Self, Never> {
    Ok(Self { bits: 0 })
  }

  pub const fn all() -> Result<Self, Never> {
    Ok(Self { bits: Self::KNOWN })
  }

  pub const fn bits(&self) -> Result<u16, Never> {
    Ok(self.bits)
  }

  pub const fn from_bits(bits: u16) -> Result<Option<Self>, Never> {
    Ok(match bits & !Self::KNOWN {
      0 => Some(Self { bits }),
      _unknown => None,
    })
  }

  pub const fn from_bits_truncate(bits: u16) -> Result<Self, Never> {
    Ok(Self {
      bits: bits & Self::KNOWN,
    })
  }

  pub const fn is_empty(&self) -> Result<Emptiness, Never> {
    Ok(match self.bits {
      0 => Emptiness::Empty,
      _set => Emptiness::NonEmpty,
    })
  }

  pub const fn intersects(&self, other: Self) -> Result<Overlap, Never> {
    Ok(match self.bits & other.bits {
      0 => Overlap::Disjoint,
      _shared => Overlap::Intersecting,
    })
  }

  pub const fn contains(&self, other: Self) -> Result<Presence, Never> {
    Ok(match other.bits & !self.bits {
      0 => Presence::Present,
      _missing => Presence::Absent,
    })
  }

  pub fn insert(&mut self, other: Self) -> Result<(), Never> {
    self.bits |= other.bits;

    Ok(())
  }

  pub fn remove(&mut self, other: Self) -> Result<(), Never> {
    self.bits &= !other.bits;

    Ok(())
  }

  pub fn toggle(&mut self, other: Self) -> Result<(), Never> {
    self.bits ^= other.bits;

    Ok(())
  }

  pub const fn intersection(self, other: Self) -> Result<Self, Never> {
    Ok(Self {
      bits: self.bits & other.bits,
    })
  }

  pub const fn union(self, other: Self) -> Result<Self, Never> {
    Ok(Self {
      bits: self.bits | other.bits,
    })
  }

  pub const fn difference(self, other: Self) -> Result<Self, Never> {
    Ok(Self {
      bits: self.bits & !other.bits,
    })
  }
}

impl core::ops::BitOr for Flags {
  type Output = Self;

  fn bitor(self, other: Self) -> Self {
    Self {
      bits: self.bits | other.bits,
    }
  }
}

impl core::ops::BitOrAssign for Flags {
  fn bitor_assign(&mut self, other: Self) {
    self.bits = self.bits | other.bits;
  }
}

impl core::ops::BitXor for Flags {
  type Output = Self;

  fn bitxor(self, other: Self) -> Self {
    Self {
      bits: self.bits ^ other.bits,
    }
  }
}

impl core::ops::BitXorAssign for Flags {
  fn bitxor_assign(&mut self, other: Self) {
    self.bits = self.bits ^ other.bits;
  }
}

impl core::ops::BitAnd for Flags {
  type Output = Self;

  fn bitand(self, other: Self) -> Self {
    Self {
      bits: self.bits & other.bits,
    }
  }
}

impl core::ops::BitAndAssign for Flags {
  fn bitand_assign(&mut self, other: Self) {
    self.bits = self.bits & other.bits;
  }
}

impl core::ops::Sub for Flags {
  type Output = Self;

  fn sub(self, other: Self) -> Self {
    Self {
      bits: self.bits & !other.bits,
    }
  }
}

impl core::ops::SubAssign for Flags {
  fn sub_assign(&mut self, other: Self) {
    self.bits &= !other.bits;
  }
}

impl core::ops::Not for Flags {
  type Output = Self;

  fn not(self) -> Self {
    Self {
      bits: !self.bits & Self::KNOWN,
    }
  }
}

impl Scalar for Flags {
  type Raw = <u16 as Scalar>::Raw;

  fn to_raw(self) -> Result<Self::Raw, Never> {
    self.bits.to_raw()
  }

  fn from_raw(raw: Self::Raw) -> Result<Self, Never> {
    let Ok(bits) = u16::from_raw(raw);

    Self::from_bits_truncate(bits)
  }
}

impl core::fmt::Debug for Flags {
  fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    let members = [
      ("BASELINE_AT_Y_0", 0x0001u16),
      ("LSB_AT_X_0", 0x0002u16),
      ("INSTRUCTIONS_MAY_DEPEND_ON_POINT_SIZE", 0x0004u16),
      ("FORCE_INTEGER_PPEM", 0x0008u16),
      ("INSTRUCTIONS_MAY_ALTER_ADVANCE_WIDTH", 0x0010u16),
      ("LOSSLESS_TRANSFORMED_FONT_DATA", 0x0800u16),
      ("CONVERTED_FONT", 0x1000u16),
      ("OPTIMIZED_FOR_CLEARTYPE", 0x2000u16),
      ("LAST_RESORT_FONT", 0x4000u16),
    ];
    let mut names = members
      .iter()
      .filter(|(_name, bits)| self.bits & bits == *bits)
      .map(|(name, _bits)| *name)
      .peekable();

    match names.peek() {
      None => output.write_str("(empty)"),
      Some(_) => {
        let mut separator = "";

        for name in names {
          output.write_str(separator)?;
          output.write_str(name)?;
          separator = " | ";
        }

        Ok(())
      }
    }
  }
}

impl core::fmt::Binary for Flags {
  fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    core::fmt::Binary::fmt(&self.bits, output)
  }
}

impl core::fmt::Octal for Flags {
  fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    core::fmt::Octal::fmt(&self.bits, output)
  }
}

impl core::fmt::LowerHex for Flags {
  fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    core::fmt::LowerHex::fmt(&self.bits, output)
  }
}

impl core::fmt::UpperHex for Flags {
  fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    core::fmt::UpperHex::fmt(&self.bits, output)
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> From<Flags> for crate::traversal::FieldType<'a> {
  fn from(flags: Flags) -> Self {
    <crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u16>>::from(
      flags.bits,
    )
  }
}

#[derive(Clone, Copy, Default, bytemuck::AnyBitPattern)]
#[repr(C)]
struct Header {
  version: BigEndian<MajorMinor>,
  font_revision: BigEndian<Fixed>,
  checksum_adjustment: BigEndian<u32>,
  magic_number: BigEndian<u32>,
  flags: BigEndian<Flags>,
  units_per_em: BigEndian<u16>,
  created: BigEndian<LongDateTime>,
  modified: BigEndian<LongDateTime>,
  x_min: BigEndian<i16>,
  y_min: BigEndian<i16>,
  x_max: BigEndian<i16>,
  y_max: BigEndian<i16>,
  mac_style: BigEndian<MacStyle>,
  lowest_rec_ppem: BigEndian<u16>,
  font_direction_hint: BigEndian<i16>,
  index_to_loc_format: BigEndian<i16>,
  glyph_data_format: BigEndian<i16>,
}

impl FixedSize for Header {
  const RAW_BYTE_LEN: u64 = 54;
}

#[derive(Clone)]
pub struct Head<'a> {
  data: FontData<'a>,
  header: Header,
  minimum: &'a [u8],
}

impl TopLevelTable for Head<'_> {
  const TAG: Tag = {
    let Ok(tag) = Tag::new(b"head");
    tag
  };
}

impl<'a> FontRead<'a> for Head<'a> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    let header = data.read_ref_at::<Header>(0)?;
    let header_2 = *header;
    let Ok(bytes) = data.as_bytes();
    let minimum = bytes.get(..54).ok_or(ReadError::OutOfBounds)?;

    Ok(Self {
      data,
      header: header_2,
      minimum,
    })
  }
}

impl<'a> Head<'a> {
  pub const MIN_SIZE: u64 = 54;

  pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
    Ok(self.data)
  }

  pub fn shape(&self) -> Result<&Self, Never> {
    Ok(self)
  }

  pub fn version(&self) -> Result<MajorMinor, Never> {
    self.header.version.get()
  }

  pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(0..4)
  }

  pub fn font_revision(&self) -> Result<Fixed, Never> {
    self.header.font_revision.get()
  }

  pub fn font_revision_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(4..8)
  }

  pub fn checksum_adjustment(&self) -> Result<u32, Never> {
    self.header.checksum_adjustment.get()
  }

  pub fn checksum_adjustment_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(8..12)
  }

  pub fn magic_number(&self) -> Result<u32, Never> {
    self.header.magic_number.get()
  }

  pub fn magic_number_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(12..16)
  }

  pub fn flags(&self) -> Result<Flags, Never> {
    self.header.flags.get()
  }

  pub fn flags_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(16..18)
  }

  pub fn units_per_em(&self) -> Result<u16, Never> {
    self.header.units_per_em.get()
  }

  pub fn units_per_em_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(18..20)
  }

  pub fn created(&self) -> Result<LongDateTime, Never> {
    self.header.created.get()
  }

  pub fn created_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(20..28)
  }

  pub fn modified(&self) -> Result<LongDateTime, Never> {
    self.header.modified.get()
  }

  pub fn modified_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(28..36)
  }

  pub fn x_min(&self) -> Result<i16, Never> {
    self.header.x_min.get()
  }

  pub fn x_min_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(36..38)
  }

  pub fn y_min(&self) -> Result<i16, Never> {
    self.header.y_min.get()
  }

  pub fn y_min_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(38..40)
  }

  pub fn x_max(&self) -> Result<i16, Never> {
    self.header.x_max.get()
  }

  pub fn x_max_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(40..42)
  }

  pub fn y_max(&self) -> Result<i16, Never> {
    self.header.y_max.get()
  }

  pub fn y_max_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(42..44)
  }

  pub fn mac_style(&self) -> Result<MacStyle, Never> {
    self.header.mac_style.get()
  }

  pub fn mac_style_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(44..46)
  }

  pub fn lowest_rec_ppem(&self) -> Result<u16, Never> {
    self.header.lowest_rec_ppem.get()
  }

  pub fn lowest_rec_ppem_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(46..48)
  }

  pub fn font_direction_hint(&self) -> Result<i16, Never> {
    self.header.font_direction_hint.get()
  }

  pub fn font_direction_hint_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(48..50)
  }

  pub fn index_to_loc_format(&self) -> Result<i16, Never> {
    self.header.index_to_loc_format.get()
  }

  pub fn index_to_loc_format_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(50..52)
  }

  pub fn glyph_data_format(&self) -> Result<i16, Never> {
    self.header.glyph_data_format.get()
  }

  pub fn glyph_data_format_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(52..54)
  }
}

impl Default for Head<'_> {
  fn default() -> Self {
    let minimum = &[0; 54];
    let Ok(data) = FontData::default_table_data();

    Self {
      data,
      header: Header::default(),
      minimum,
    }
  }
}

impl<'a> MinByteRange<'a> for Head<'a> {
  fn min_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(0..54)
  }

  fn min_table_bytes(&self) -> Result<&'a [u8], ReadError> {
    Ok(self.minimum)
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> crate::traversal::SomeTable<'a> for Head<'a> {
  fn type_name(&self) -> Result<&str, Never> {
    Ok("Head")
  }

  fn get_field(&self, idx: u64) -> Result<Option<crate::traversal::Field<'a>>, Never> {
    Ok(match idx {
      0u64 => {
        let Ok(field) = crate::traversal::Field::new("version", {
          let Ok(explicit_value) = self.version();
          explicit_value
        });

        Some(field)
      }
      1u64 => {
        let Ok(field) = crate::traversal::Field::new("font_revision", {
          let Ok(explicit_value) = self.font_revision();
          explicit_value
        });

        Some(field)
      }
      2u64 => {
        let Ok(field) = crate::traversal::Field::new("checksum_adjustment", {
          let Ok(explicit_value) = self.checksum_adjustment();
          explicit_value
        });

        Some(field)
      }
      3u64 => {
        let Ok(field) = crate::traversal::Field::new("magic_number", {
          let Ok(explicit_value) = self.magic_number();
          explicit_value
        });

        Some(field)
      }
      4u64 => {
        let Ok(field) = crate::traversal::Field::new("flags", {
          let Ok(explicit_value) = self.flags();
          explicit_value
        });

        Some(field)
      }
      5u64 => {
        let Ok(field) = crate::traversal::Field::new("units_per_em", {
          let Ok(explicit_value) = self.units_per_em();
          explicit_value
        });

        Some(field)
      }
      6u64 => {
        let Ok(field) = crate::traversal::Field::new("created", {
          let Ok(explicit_value) = self.created();
          explicit_value
        });

        Some(field)
      }
      7u64 => {
        let Ok(field) = crate::traversal::Field::new("modified", {
          let Ok(explicit_value) = self.modified();
          explicit_value
        });

        Some(field)
      }
      8u64 => {
        let Ok(field) = crate::traversal::Field::new("x_min", {
          let Ok(explicit_value) = self.x_min();
          explicit_value
        });

        Some(field)
      }
      9u64 => {
        let Ok(field) = crate::traversal::Field::new("y_min", {
          let Ok(explicit_value) = self.y_min();
          explicit_value
        });

        Some(field)
      }
      10u64 => {
        let Ok(field) = crate::traversal::Field::new("x_max", {
          let Ok(explicit_value) = self.x_max();
          explicit_value
        });

        Some(field)
      }
      11u64 => {
        let Ok(field) = crate::traversal::Field::new("y_max", {
          let Ok(explicit_value) = self.y_max();
          explicit_value
        });

        Some(field)
      }
      12u64 => {
        let Ok(field) = crate::traversal::Field::new("mac_style", {
          let Ok(explicit_value) = self.mac_style();
          explicit_value
        });

        Some(field)
      }
      13u64 => {
        let Ok(field) = crate::traversal::Field::new("lowest_rec_ppem", {
          let Ok(explicit_value) = self.lowest_rec_ppem();
          explicit_value
        });

        Some(field)
      }
      14u64 => {
        let Ok(field) = crate::traversal::Field::new("font_direction_hint", {
          let Ok(explicit_value) = self.font_direction_hint();
          explicit_value
        });

        Some(field)
      }
      15u64 => {
        let Ok(field) = crate::traversal::Field::new("index_to_loc_format", {
          let Ok(explicit_value) = self.index_to_loc_format();
          explicit_value
        });

        Some(field)
      }
      16u64 => {
        let Ok(field) = crate::traversal::Field::new("glyph_data_format", {
          let Ok(explicit_value) = self.glyph_data_format();
          explicit_value
        });

        Some(field)
      }
      _ => None,
    })
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Head<'a> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    ::core::convert::identity::<&dyn crate::traversal::SomeTable<'a>>(self).fmt(f)
  }
}
