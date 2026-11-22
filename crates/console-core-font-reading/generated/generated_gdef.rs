

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Gdef<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(mark_attach_class_def_offset_byte_range) = self.mark_attach_class_def_offset_byte_range();

        Ok(0..mark_attach_class_def_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Gdef<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"GDEF"); value });
}

impl<'a> FontRead<'a> for Gdef<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Gdef<'a> {
    data: FontData<'a>,
}

impl<'a> Gdef<'a> {
    pub const MIN_SIZE: u64 = (MajorMinor::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<MajorMinor, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn glyph_class_def_offset(&self) -> Result<Nullable<Offset16>, Never> {
        let Ok(range) = self.glyph_class_def_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn glyph_class_def(&self) -> Result<Option<ClassDef<'a>>, ReadError> {
        let data = self.data;
        let Ok(glyph_class_def_offset) = self.glyph_class_def_offset();

        glyph_class_def_offset.resolve(data)
    }

    pub fn attach_list_offset(&self) -> Result<Nullable<Offset16>, Never> {
        let Ok(range) = self.attach_list_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn attach_list(&self) -> Result<Option<AttachList<'a>>, ReadError> {
        let data = self.data;
        let Ok(attach_list_offset) = self.attach_list_offset();

        attach_list_offset.resolve(data)
    }

    pub fn lig_caret_list_offset(&self) -> Result<Nullable<Offset16>, Never> {
        let Ok(range) = self.lig_caret_list_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn lig_caret_list(&self) -> Result<Option<LigCaretList<'a>>, ReadError> {
        let data = self.data;
        let Ok(lig_caret_list_offset) = self.lig_caret_list_offset();

        lig_caret_list_offset.resolve(data)
    }

    pub fn mark_attach_class_def_offset(&self) -> Result<Nullable<Offset16>, Never> {
        let Ok(range) = self.mark_attach_class_def_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn mark_attach_class_def(&self) -> Result<Option<ClassDef<'a>>, ReadError> {
        let data = self.data;
        let Ok(mark_attach_class_def_offset) = self.mark_attach_class_def_offset();

        mark_attach_class_def_offset.resolve(data)
    }

    pub fn mark_glyph_sets_def_offset(&self) -> Result<Option<Nullable<Offset16>>, Never> {
        let Ok(range) = self.mark_glyph_sets_def_offset_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn mark_glyph_sets_def(&self) -> Result<Option<MarkGlyphSets<'a>>, ReadError> {
        let data = self.data;
        let Ok(read_mark_glyph_sets_def_offset) = self.mark_glyph_sets_def_offset();

        match read_mark_glyph_sets_def_offset { Some(x) => x.resolve(data), None => Ok(None) }
    }

    pub fn item_var_store_offset(&self) -> Result<Option<Nullable<Offset32>>, Never> {
        let Ok(range) = self.item_var_store_offset_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn item_var_store(&self) -> Result<Option<ItemVariationStore<'a>>, ReadError> {
        let data = self.data;
        let Ok(read_item_var_store_offset) = self.item_var_store_offset();

        match read_item_var_store_offset { Some(x) => x.resolve(data), None => Ok(None) }
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(MajorMinor::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_class_def_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn attach_list_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.glyph_class_def_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn lig_caret_list_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.attach_list_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn mark_attach_class_def_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.lig_caret_list_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn mark_glyph_sets_def_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.mark_attach_class_def_offset_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility((1u16, 2u16));

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(Offset16::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }

    pub fn item_var_store_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.mark_glyph_sets_def_offset_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility((1u16, 3u16));

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(Offset32::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Gdef::MIN_SIZE); held }));

impl Default for Gdef<'_> {
    fn default() -> Self {
        let Ok(value_4) = FontData::default_table_data();

        Self {
            data: value_4,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Gdef<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Gdef")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "glyph_class_def_offset", value: FieldType::from(FieldType::offset(self.glyph_class_def_offset(), self.glyph_class_def())) }),
            2u64 => Some(Field { name: "attach_list_offset", value: FieldType::from(FieldType::offset(self.attach_list_offset(), self.attach_list())) }),
            3u64 => Some(Field { name: "lig_caret_list_offset", value: FieldType::from(FieldType::offset(self.lig_caret_list_offset(), self.lig_caret_list())) }),
            4u64 => Some(Field { name: "mark_attach_class_def_offset", value: FieldType::from(FieldType::offset(
                    self.mark_attach_class_def_offset(),
                    self.mark_attach_class_def(),
                )) }),
            5u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility((1u16, 2u16));
                let Ok(mark_glyph_sets_def_offset) = self.mark_glyph_sets_def_offset();

                match compatibility {
                    Compatibility::Compatible => mark_glyph_sets_def_offset.map(|mark_glyph_sets_def_offset_2| Field { name: "mark_glyph_sets_def_offset", value: FieldType::from(FieldType::offset(
                        mark_glyph_sets_def_offset_2,
                        self.mark_glyph_sets_def(),
                    )) }),
                    Compatibility::Incompatible => None,
                }
            },
            6u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility((1u16, 3u16));
                let Ok(item_var_store_offset) = self.item_var_store_offset();

                match compatibility {
                    Compatibility::Compatible => item_var_store_offset.map(|item_var_store_offset_2| Field { name: "item_var_store_offset", value: FieldType::from(FieldType::offset(item_var_store_offset_2, self.item_var_store())) }),
                    Compatibility::Incompatible => None,
                }
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Gdef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u16)]
#[cfg_attr(
    dylint_lib = "explicit070_spaced_discriminants",
    expect(explicit070_spaced_discriminants, reason = "the glyph class values are the OpenType GDEF numeric encoding")
)]
pub enum GlyphClassDef {
    #[default]
    Base = 1,
    Ligature = 2,
    Mark = 3,
    Component = 4,
    #[doc(hidden)]
    Unknown,
}

impl GlyphClassDef {
    fn discriminant_value(self) -> Result<u16, Never> {
        Ok(match self {
            Self::Base => 1,
            Self::Ligature => 2,
            Self::Mark => 3,
            Self::Component => 4,
            Self::Unknown => 5,
        })
    }

    pub fn new(raw: u16) -> Result<Self, Never> {
        Ok(match raw {
            1 => Self::Base,
            2 => Self::Ligature,
            3 => Self::Mark,
            4 => Self::Component,
            _ => Self::Unknown,
        })
    }
}

impl font_types::Scalar for GlyphClassDef {
    type Raw = <u16 as font_types::Scalar>::Raw;

    fn to_raw(self) -> std::result::Result<Self::Raw, console_core_never::Never> {
        let Ok(value) = self.discriminant_value();
        value.to_raw()
    }

    fn from_raw(raw: Self::Raw) -> std::result::Result<Self, console_core_never::Never> {
        let Ok(value_6) = <u16>::from_raw(raw);

        let t = value_6;
        let value_7 = t;

        let Ok(glyph_class_def) = Self::new(value_7);

        Ok(glyph_class_def)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> From<GlyphClassDef> for FieldType<'a> {
    fn from(src: GlyphClassDef) -> FieldType<'a> {
        let Ok(value) = src.discriminant_value();
        FieldType::from(value)
    }
}

impl<'a> MinByteRange<'a> for AttachList<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(attach_point_offsets_byte_range) = self.attach_point_offsets_byte_range();

        Ok(0..attach_point_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for AttachList<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_8) = data.len();

        match ::core::primitive::bool::then_some(value_8 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct AttachList<'a> {
    data: FontData<'a>,
}

impl<'a> AttachList<'a> {
    pub const MIN_SIZE: u64 = (Offset16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn coverage_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.coverage_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage(&self) -> Result<CoverageTable<'a>, ReadError> {
        let data = self.data;
        let Ok(coverage_offset) = self.coverage_offset();

        coverage_offset.resolve(data)
    }

    pub fn glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.glyph_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn attach_point_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.attach_point_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn attach_points(&self) -> Result<ArrayOfOffsets<'a, AttachPoint<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.attach_point_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn attach_point_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(glyph_count) = self.glyph_count();
        let Ok(previous) = self.glyph_count_byte_range();
        let start = previous.end;
        let Ok(value_9) = transforms::to_u64(glyph_count);

        let end =
            start.saturating_add(value_9.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(AttachList::MIN_SIZE); held }));

impl Default for AttachList<'_> {
    fn default() -> Self {
        let Ok(value_10) = FontData::default_table_data();

        Self {
            data: value_10,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for AttachList<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("AttachList")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "coverage_offset", value: FieldType::from(FieldType::offset(self.coverage_offset(), (self.coverage()).map(Some))) }),
            1u64 => Some(Field { name: "glyph_count", value: FieldType::from(self.glyph_count()) }),
            2u64 => Some(Field { name: "attach_point_offsets", value: FieldType::from(self.attach_points()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for AttachList<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for AttachPoint<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(point_indices_byte_range) = self.point_indices_byte_range();

        Ok(0..point_indices_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for AttachPoint<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_11) = data.len();

        match ::core::primitive::bool::then_some(value_11 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct AttachPoint<'a> {
    data: FontData<'a>,
}

impl<'a> AttachPoint<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn point_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.point_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn point_indices(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.point_indices_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn point_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn point_indices_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(point_count) = self.point_count();
        let Ok(previous) = self.point_count_byte_range();
        let start = previous.end;
        let Ok(value_12) = transforms::to_u64(point_count);

        let end = start.saturating_add(value_12.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(AttachPoint::MIN_SIZE); held }));

impl Default for AttachPoint<'_> {
    fn default() -> Self {
        let Ok(value_13) = FontData::default_table_data();

        Self {
            data: value_13,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for AttachPoint<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("AttachPoint")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "point_count", value: FieldType::from(self.point_count()) }),
            1u64 => Some(Field { name: "point_indices", value: FieldType::from(self.point_indices()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for AttachPoint<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for LigCaretList<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(lig_glyph_offsets_byte_range) = self.lig_glyph_offsets_byte_range();

        Ok(0..lig_glyph_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for LigCaretList<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_14) = data.len();

        match ::core::primitive::bool::then_some(value_14 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct LigCaretList<'a> {
    data: FontData<'a>,
}

impl<'a> LigCaretList<'a> {
    pub const MIN_SIZE: u64 = (Offset16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn coverage_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.coverage_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage(&self) -> Result<CoverageTable<'a>, ReadError> {
        let data = self.data;
        let Ok(coverage_offset) = self.coverage_offset();

        coverage_offset.resolve(data)
    }

    pub fn lig_glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.lig_glyph_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn lig_glyph_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.lig_glyph_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn lig_glyphs(&self) -> Result<ArrayOfOffsets<'a, LigGlyph<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.lig_glyph_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn lig_glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn lig_glyph_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(lig_glyph_count) = self.lig_glyph_count();
        let Ok(previous) = self.lig_glyph_count_byte_range();
        let start = previous.end;
        let Ok(value_15) = transforms::to_u64(lig_glyph_count);

        let end =
            start.saturating_add(value_15.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(LigCaretList::MIN_SIZE); held }));

impl Default for LigCaretList<'_> {
    fn default() -> Self {
        let Ok(value_16) = FontData::default_table_data();

        Self {
            data: value_16,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for LigCaretList<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("LigCaretList")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "coverage_offset", value: FieldType::from(FieldType::offset(self.coverage_offset(), (self.coverage()).map(Some))) }),
            1u64 => Some(Field { name: "lig_glyph_count", value: FieldType::from(self.lig_glyph_count()) }),
            2u64 => Some(Field { name: "lig_glyph_offsets", value: FieldType::from(self.lig_glyphs()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for LigCaretList<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for LigGlyph<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(caret_value_offsets_byte_range) = self.caret_value_offsets_byte_range();

        Ok(0..caret_value_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for LigGlyph<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_17) = data.len();

        match ::core::primitive::bool::then_some(value_17 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct LigGlyph<'a> {
    data: FontData<'a>,
}

impl<'a> LigGlyph<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn caret_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.caret_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn caret_value_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.caret_value_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn caret_values(&self) -> Result<ArrayOfOffsets<'a, CaretValue<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.caret_value_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn caret_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn caret_value_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(caret_count) = self.caret_count();
        let Ok(previous) = self.caret_count_byte_range();
        let start = previous.end;
        let Ok(value_18) = transforms::to_u64(caret_count);

        let end =
            start.saturating_add(value_18.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(LigGlyph::MIN_SIZE); held }));

impl Default for LigGlyph<'_> {
    fn default() -> Self {
        let Ok(value_19) = FontData::default_table_data();

        Self {
            data: value_19,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for LigGlyph<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("LigGlyph")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "caret_count", value: FieldType::from(self.caret_count()) }),
            1u64 => Some(Field { name: "caret_value_offsets", value: FieldType::from(self.caret_values()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for LigGlyph<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone)]
pub enum CaretValue<'a> {
    Format1(CaretValueFormat1<'a>),
    Format2(CaretValueFormat2<'a>),
    Format3(CaretValueFormat3<'a>),
}

impl Default for CaretValue<'_> {
    fn default() -> Self {
        Self::Format1(Default::default())
    }
}

impl<'a> CaretValue<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Format1(item) => item.offset_data(),
            Self::Format2(item) => item.offset_data(),
            Self::Format3(item) => item.offset_data(),
        }
    }

    pub fn caret_value_format(&self) -> Result<u16, Never> {
        match self {
            Self::Format1(item) => item.caret_value_format(),
            Self::Format2(item) => item.caret_value_format(),
            Self::Format3(item) => item.caret_value_format(),
        }
    }
}

impl<'a> FontRead<'a> for CaretValue<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u16 = data.read_at(0u64)?;

        match format {
            CaretValueFormat1::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format1(read))
            }
            CaretValueFormat2::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format2(read))
            }
            CaretValueFormat3::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format3(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u16>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for CaretValue<'a> {
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
impl<'a> CaretValue<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Format1(table) => table,
            Self::Format2(table) => table,
            Self::Format3(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for CaretValue<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for CaretValue<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}

impl Format<u16> for CaretValueFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for CaretValueFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(coordinate_byte_range) = self.coordinate_byte_range();

        Ok(0..coordinate_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for CaretValueFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_23) = data.len();

        match ::core::primitive::bool::then_some(value_23 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct CaretValueFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> CaretValueFormat1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + i16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn caret_value_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.caret_value_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coordinate(&self) -> Result<i16, Never> {
        let Ok(range) = self.coordinate_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn caret_value_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coordinate_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.caret_value_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    CaretValueFormat1::MIN_SIZE
); held }));

impl Default for CaretValueFormat1<'_> {
    fn default() -> Self {
        let Ok(value_24) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_24,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for CaretValueFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("CaretValueFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "caret_value_format", value: FieldType::from(self.caret_value_format()) }),
            1u64 => Some(Field { name: "coordinate", value: FieldType::from(self.coordinate()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for CaretValueFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for CaretValueFormat2<'_> {
    const FORMAT: u16 = 2;
}

impl<'a> MinByteRange<'a> for CaretValueFormat2<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(caret_value_point_index_byte_range) = self.caret_value_point_index_byte_range();

        Ok(0..caret_value_point_index_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for CaretValueFormat2<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_25) = data.len();

        match ::core::primitive::bool::then_some(value_25 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct CaretValueFormat2<'a> {
    data: FontData<'a>,
}

impl<'a> CaretValueFormat2<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn caret_value_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.caret_value_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn caret_value_point_index(&self) -> Result<u16, Never> {
        let Ok(range) = self.caret_value_point_index_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn caret_value_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn caret_value_point_index_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.caret_value_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for CaretValueFormat2<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("CaretValueFormat2")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "caret_value_format", value: FieldType::from(self.caret_value_format()) }),
            1u64 => Some(Field { name: "caret_value_point_index", value: FieldType::from(self.caret_value_point_index()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for CaretValueFormat2<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for CaretValueFormat3<'_> {
    const FORMAT: u16 = 3;
}

impl<'a> MinByteRange<'a> for CaretValueFormat3<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(device_offset_byte_range) = self.device_offset_byte_range();

        Ok(0..device_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for CaretValueFormat3<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_26) = data.len();

        match ::core::primitive::bool::then_some(value_26 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct CaretValueFormat3<'a> {
    data: FontData<'a>,
}

impl<'a> CaretValueFormat3<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + i16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn caret_value_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.caret_value_format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coordinate(&self) -> Result<i16, Never> {
        let Ok(range) = self.coordinate_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn device_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.device_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn device(&self) -> Result<DeviceOrVariationIndex<'a>, ReadError> {
        let data = self.data;
        let Ok(device_offset) = self.device_offset();

        device_offset.resolve(data)
    }

    pub fn caret_value_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coordinate_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.caret_value_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn device_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coordinate_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for CaretValueFormat3<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("CaretValueFormat3")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "caret_value_format", value: FieldType::from(self.caret_value_format()) }),
            1u64 => Some(Field { name: "coordinate", value: FieldType::from(self.coordinate()) }),
            2u64 => Some(Field { name: "device_offset", value: FieldType::from(FieldType::offset(self.device_offset(), (self.device()).map(Some))) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for CaretValueFormat3<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for MarkGlyphSets<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for MarkGlyphSets<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(coverage_offsets_byte_range) = self.coverage_offsets_byte_range();

        Ok(0..coverage_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for MarkGlyphSets<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_27) = data.len();

        match ::core::primitive::bool::then_some(value_27 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct MarkGlyphSets<'a> {
    data: FontData<'a>,
}

impl<'a> MarkGlyphSets<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn mark_glyph_set_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.mark_glyph_set_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage_offsets(&self) -> Result<&'a [BigEndian<Offset32>], Never> {
        let Ok(range) = self.coverage_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn coverages(&self) -> Result<ArrayOfOffsets<'a, CoverageTable<'a>, Offset32>, Never> {
        let data = self.data;
        let Ok(offsets) = self.coverage_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn mark_glyph_set_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coverage_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(mark_glyph_set_count) = self.mark_glyph_set_count();
        let Ok(previous) = self.mark_glyph_set_count_byte_range();
        let start = previous.end;
        let Ok(value_28) = transforms::to_u64(mark_glyph_set_count);

        let end = start.saturating_add(value_28.saturating_mul(Offset32::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(MarkGlyphSets::MIN_SIZE); held }));

impl Default for MarkGlyphSets<'_> {
    fn default() -> Self {
        let Ok(value_29) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_29,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for MarkGlyphSets<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("MarkGlyphSets")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "mark_glyph_set_count", value: FieldType::from(self.mark_glyph_set_count()) }),
            2u64 => Some(Field { name: "coverage_offsets", value: FieldType::from(self.coverages()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for MarkGlyphSets<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
