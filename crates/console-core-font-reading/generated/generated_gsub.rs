

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Gsub<'a> {
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

impl TopLevelTable for Gsub<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"GSUB"); value });
}

impl<'a> FontRead<'a> for Gsub<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Gsub<'a> {
    data: FontData<'a>,
}

impl<'a> Gsub<'a> {
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

    pub fn lookup_list(&self) -> Result<SubstitutionLookupList<'a>, ReadError> {
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

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Gsub::MIN_SIZE); held }));

impl Default for Gsub<'_> {
    fn default() -> Self {
        let Ok(value_3) = FontData::default_table_data();

        Self {
            data: value_3,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Gsub<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Gsub")
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
impl<'a> std::fmt::Debug for Gsub<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

pub enum SubstitutionLookup<'a> {
    Single(Lookup<'a, SingleSubst<'a>>),
    Multiple(Lookup<'a, MultipleSubstFormat1<'a>>),
    Alternate(Lookup<'a, AlternateSubstFormat1<'a>>),
    Ligature(Lookup<'a, LigatureSubstFormat1<'a>>),
    Contextual(Lookup<'a, SubstitutionSequenceContext<'a>>),
    ChainContextual(Lookup<'a, SubstitutionChainContext<'a>>),
    Extension(Lookup<'a, ExtensionSubtable<'a>>),
    Reverse(Lookup<'a, ReverseChainSingleSubstFormat1<'a>>),
}

impl Default for SubstitutionLookup<'_> {
    fn default() -> Self {
        Self::Single(Default::default())
    }
}

impl<'a> FontRead<'a> for SubstitutionLookup<'a> {
    fn read(bytes: FontData<'a>) -> Result<Self, ReadError> {
        let discriminant = Lookup::read_discriminant(bytes)?;

        match discriminant {
            1 => {
                let read = FontRead::read(bytes)?;

                Ok(SubstitutionLookup::Single(read))
            }
            2 => {
                let read = FontRead::read(bytes)?;

                Ok(SubstitutionLookup::Multiple(read))
            }
            3 => {
                let read = FontRead::read(bytes)?;

                Ok(SubstitutionLookup::Alternate(read))
            }
            4 => {
                let read = FontRead::read(bytes)?;

                Ok(SubstitutionLookup::Ligature(read))
            }
            5 => {
                let read = FontRead::read(bytes)?;

                Ok(SubstitutionLookup::Contextual(read))
            }
            6 => {
                let read = FontRead::read(bytes)?;

                Ok(SubstitutionLookup::ChainContextual(read))
            }
            7 => {
                let read = FontRead::read(bytes)?;

                Ok(SubstitutionLookup::Extension(read))
            }
            8 => {
                let read = FontRead::read(bytes)?;

                Ok(SubstitutionLookup::Reverse(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u16>>::from(other))),
        }
    }
}

impl<'a> SubstitutionLookup<'a> {

    pub(crate) fn of_unit_type(&self) -> Result<Lookup<'a, ()>, Never> {
        match self {
            SubstitutionLookup::Single(inner) => inner.of_unit_type(),
            SubstitutionLookup::Multiple(inner) => inner.of_unit_type(),
            SubstitutionLookup::Alternate(inner) => inner.of_unit_type(),
            SubstitutionLookup::Ligature(inner) => inner.of_unit_type(),
            SubstitutionLookup::Contextual(inner) => inner.of_unit_type(),
            SubstitutionLookup::ChainContextual(inner) => inner.of_unit_type(),
            SubstitutionLookup::Extension(inner) => inner.of_unit_type(),
            SubstitutionLookup::Reverse(inner) => inner.of_unit_type(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SubstitutionLookup<'a> {
    fn dyn_inner(&self) -> Result<&(dyn SomeTable<'a> + 'a), Never> {
        Ok(match self {
            SubstitutionLookup::Single(table) => table,
            SubstitutionLookup::Multiple(table) => table,
            SubstitutionLookup::Alternate(table) => table,
            SubstitutionLookup::Ligature(table) => table,
            SubstitutionLookup::Contextual(table) => table,
            SubstitutionLookup::ChainContextual(table) => table,
            SubstitutionLookup::Extension(table) => table,
            SubstitutionLookup::Reverse(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for SubstitutionLookup<'a> {
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
impl std::fmt::Debug for SubstitutionLookup<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[derive(Clone)]
pub enum SingleSubst<'a> {
    Format1(SingleSubstFormat1<'a>),
    Format2(SingleSubstFormat2<'a>),
}

impl Default for SingleSubst<'_> {
    fn default() -> Self {
        Self::Format1(Default::default())
    }
}

impl<'a> SingleSubst<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Format1(item) => item.offset_data(),
            Self::Format2(item) => item.offset_data(),
        }
    }

    pub fn subst_format(&self) -> Result<u16, Never> {
        match self {
            Self::Format1(item) => item.subst_format(),
            Self::Format2(item) => item.subst_format(),
        }
    }

    pub fn coverage_offset(&self) -> Result<Offset16, Never> {
        match self {
            Self::Format1(item) => item.coverage_offset(),
            Self::Format2(item) => item.coverage_offset(),
        }
    }
}

impl<'a> FontRead<'a> for SingleSubst<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u16 = data.read_at(0u64)?;

        match format {
            SingleSubstFormat1::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format1(read))
            }
            SingleSubstFormat2::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format2(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u16>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for SingleSubst<'a> {
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
impl<'a> SingleSubst<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Format1(table) => table,
            Self::Format2(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for SingleSubst<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for SingleSubst<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}

impl Format<u16> for SingleSubstFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for SingleSubstFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(delta_glyph_id_byte_range) = self.delta_glyph_id_byte_range();

        Ok(0..delta_glyph_id_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for SingleSubstFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_6) = data.len();

        match ::core::primitive::bool::then_some(value_6 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct SingleSubstFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> SingleSubstFormat1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN + i16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn subst_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.subst_format_byte_range();

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

    pub fn delta_glyph_id(&self) -> Result<i16, Never> {
        let Ok(range) = self.delta_glyph_id_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn subst_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.subst_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn delta_glyph_id_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    SingleSubstFormat1::MIN_SIZE
); held }));

impl Default for SingleSubstFormat1<'_> {
    fn default() -> Self {
        let Ok(value_7) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_7,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for SingleSubstFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("SingleSubstFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "subst_format", value: FieldType::from(self.subst_format()) }),
            1u64 => Some(Field { name: "coverage_offset", value: FieldType::from(FieldType::offset(self.coverage_offset(), (self.coverage()).map(Some))) }),
            2u64 => Some(Field { name: "delta_glyph_id", value: FieldType::from(self.delta_glyph_id()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for SingleSubstFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for SingleSubstFormat2<'_> {
    const FORMAT: u16 = 2;
}

impl<'a> MinByteRange<'a> for SingleSubstFormat2<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(substitute_glyph_ids_byte_range) = self.substitute_glyph_ids_byte_range();

        Ok(0..substitute_glyph_ids_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for SingleSubstFormat2<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_8) = data.len();

        match ::core::primitive::bool::then_some(value_8 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct SingleSubstFormat2<'a> {
    data: FontData<'a>,
}

impl<'a> SingleSubstFormat2<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn subst_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.subst_format_byte_range();

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

    pub fn glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.glyph_count_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn substitute_glyph_ids(&self) -> Result<&'a [BigEndian<GlyphId16>], Never> {
        let Ok(range) = self.substitute_glyph_ids_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn subst_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.subst_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn substitute_glyph_ids_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(glyph_count) = self.glyph_count();
        let Ok(previous) = self.glyph_count_byte_range();
        let start = previous.end;
        let Ok(value_9) = transforms::to_u64(glyph_count);

        let end =
            start.saturating_add(value_9.saturating_mul(GlyphId16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for SingleSubstFormat2<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("SingleSubstFormat2")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "subst_format", value: FieldType::from(self.subst_format()) }),
            1u64 => Some(Field { name: "coverage_offset", value: FieldType::from(FieldType::offset(self.coverage_offset(), (self.coverage()).map(Some))) }),
            2u64 => Some(Field { name: "glyph_count", value: FieldType::from(self.glyph_count()) }),
            3u64 => Some(Field { name: "substitute_glyph_ids", value: FieldType::from(self.substitute_glyph_ids()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for SingleSubstFormat2<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for MultipleSubstFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for MultipleSubstFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(sequence_offsets_byte_range) = self.sequence_offsets_byte_range();

        Ok(0..sequence_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for MultipleSubstFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_10) = data.len();

        match ::core::primitive::bool::then_some(value_10 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct MultipleSubstFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> MultipleSubstFormat1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn subst_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.subst_format_byte_range();

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

    pub fn sequence_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.sequence_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn sequence_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.sequence_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn sequences(&self) -> Result<ArrayOfOffsets<'a, Sequence<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.sequence_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn subst_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.subst_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn sequence_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn sequence_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(sequence_count) = self.sequence_count();
        let Ok(previous) = self.sequence_count_byte_range();
        let start = previous.end;
        let Ok(value_11) = transforms::to_u64(sequence_count);

        let end =
            start.saturating_add(value_11.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    MultipleSubstFormat1::MIN_SIZE
); held }));

impl Default for MultipleSubstFormat1<'_> {
    fn default() -> Self {
        let Ok(value_12) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_12,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for MultipleSubstFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("MultipleSubstFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "subst_format", value: FieldType::from(self.subst_format()) }),
            1u64 => Some(Field { name: "coverage_offset", value: FieldType::from(FieldType::offset(self.coverage_offset(), (self.coverage()).map(Some))) }),
            2u64 => Some(Field { name: "sequence_count", value: FieldType::from(self.sequence_count()) }),
            3u64 => Some(Field { name: "sequence_offsets", value: FieldType::from(self.sequences()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for MultipleSubstFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for Sequence<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(substitute_glyph_ids_byte_range) = self.substitute_glyph_ids_byte_range();

        Ok(0..substitute_glyph_ids_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Sequence<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_13) = data.len();

        match ::core::primitive::bool::then_some(value_13 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Sequence<'a> {
    data: FontData<'a>,
}

impl<'a> Sequence<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.glyph_count_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn substitute_glyph_ids(&self) -> Result<&'a [BigEndian<GlyphId16>], Never> {
        let Ok(range) = self.substitute_glyph_ids_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn substitute_glyph_ids_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(glyph_count) = self.glyph_count();
        let Ok(previous) = self.glyph_count_byte_range();
        let start = previous.end;
        let Ok(value_14) = transforms::to_u64(glyph_count);

        let end =
            start.saturating_add(value_14.saturating_mul(GlyphId16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Sequence::MIN_SIZE); held }));

impl Default for Sequence<'_> {
    fn default() -> Self {
        let Ok(value_15) = FontData::default_table_data();

        Self {
            data: value_15,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Sequence<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Sequence")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "glyph_count", value: FieldType::from(self.glyph_count()) }),
            1u64 => Some(Field { name: "substitute_glyph_ids", value: FieldType::from(self.substitute_glyph_ids()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Sequence<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for AlternateSubstFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for AlternateSubstFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(alternate_set_offsets_byte_range) = self.alternate_set_offsets_byte_range();

        Ok(0..alternate_set_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for AlternateSubstFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_16) = data.len();

        match ::core::primitive::bool::then_some(value_16 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct AlternateSubstFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> AlternateSubstFormat1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn subst_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.subst_format_byte_range();

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

    pub fn alternate_set_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.alternate_set_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn alternate_set_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.alternate_set_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn alternate_sets(&self) -> Result<ArrayOfOffsets<'a, AlternateSet<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.alternate_set_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn subst_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.subst_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn alternate_set_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn alternate_set_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(alternate_set_count) = self.alternate_set_count();
        let Ok(previous) = self.alternate_set_count_byte_range();
        let start = previous.end;
        let Ok(value_17) = transforms::to_u64(alternate_set_count);

        let end = start.saturating_add(value_17.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    AlternateSubstFormat1::MIN_SIZE
); held }));

impl Default for AlternateSubstFormat1<'_> {
    fn default() -> Self {
        let Ok(value_18) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_18,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for AlternateSubstFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("AlternateSubstFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "subst_format", value: FieldType::from(self.subst_format()) }),
            1u64 => Some(Field { name: "coverage_offset", value: FieldType::from(FieldType::offset(self.coverage_offset(), (self.coverage()).map(Some))) }),
            2u64 => Some(Field { name: "alternate_set_count", value: FieldType::from(self.alternate_set_count()) }),
            3u64 => Some(Field { name: "alternate_set_offsets", value: FieldType::from(self.alternate_sets()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for AlternateSubstFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for AlternateSet<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(alternate_glyph_ids_byte_range) = self.alternate_glyph_ids_byte_range();

        Ok(0..alternate_glyph_ids_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for AlternateSet<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_19) = data.len();

        match ::core::primitive::bool::then_some(value_19 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct AlternateSet<'a> {
    data: FontData<'a>,
}

impl<'a> AlternateSet<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.glyph_count_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn alternate_glyph_ids(&self) -> Result<&'a [BigEndian<GlyphId16>], Never> {
        let Ok(range) = self.alternate_glyph_ids_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn alternate_glyph_ids_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(glyph_count) = self.glyph_count();
        let Ok(previous) = self.glyph_count_byte_range();
        let start = previous.end;
        let Ok(value_20) = transforms::to_u64(glyph_count);

        let end =
            start.saturating_add(value_20.saturating_mul(GlyphId16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(AlternateSet::MIN_SIZE); held }));

impl Default for AlternateSet<'_> {
    fn default() -> Self {
        let Ok(value_21) = FontData::default_table_data();

        Self {
            data: value_21,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for AlternateSet<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("AlternateSet")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "glyph_count", value: FieldType::from(self.glyph_count()) }),
            1u64 => Some(Field { name: "alternate_glyph_ids", value: FieldType::from(self.alternate_glyph_ids()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for AlternateSet<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for LigatureSubstFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for LigatureSubstFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(ligature_set_offsets_byte_range) = self.ligature_set_offsets_byte_range();

        Ok(0..ligature_set_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for LigatureSubstFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_22) = data.len();

        match ::core::primitive::bool::then_some(value_22 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct LigatureSubstFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> LigatureSubstFormat1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn subst_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.subst_format_byte_range();

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

    pub fn ligature_set_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.ligature_set_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn ligature_set_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.ligature_set_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn ligature_sets(&self) -> Result<ArrayOfOffsets<'a, LigatureSet<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.ligature_set_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn subst_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.subst_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn ligature_set_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn ligature_set_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(ligature_set_count) = self.ligature_set_count();
        let Ok(previous) = self.ligature_set_count_byte_range();
        let start = previous.end;
        let Ok(value_23) = transforms::to_u64(ligature_set_count);

        let end = start.saturating_add(value_23.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    LigatureSubstFormat1::MIN_SIZE
); held }));

impl Default for LigatureSubstFormat1<'_> {
    fn default() -> Self {
        let Ok(value_24) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_24,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for LigatureSubstFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("LigatureSubstFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "subst_format", value: FieldType::from(self.subst_format()) }),
            1u64 => Some(Field { name: "coverage_offset", value: FieldType::from(FieldType::offset(self.coverage_offset(), (self.coverage()).map(Some))) }),
            2u64 => Some(Field { name: "ligature_set_count", value: FieldType::from(self.ligature_set_count()) }),
            3u64 => Some(Field { name: "ligature_set_offsets", value: FieldType::from(self.ligature_sets()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for LigatureSubstFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for LigatureSet<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(ligature_offsets_byte_range) = self.ligature_offsets_byte_range();

        Ok(0..ligature_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for LigatureSet<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_25) = data.len();

        match ::core::primitive::bool::then_some(value_25 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct LigatureSet<'a> {
    data: FontData<'a>,
}

impl<'a> LigatureSet<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn ligature_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.ligature_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn ligature_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.ligature_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn ligatures(&self) -> Result<ArrayOfOffsets<'a, Ligature<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.ligature_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn ligature_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn ligature_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(ligature_count) = self.ligature_count();
        let Ok(previous) = self.ligature_count_byte_range();
        let start = previous.end;
        let Ok(value_26) = transforms::to_u64(ligature_count);

        let end =
            start.saturating_add(value_26.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(LigatureSet::MIN_SIZE); held }));

impl Default for LigatureSet<'_> {
    fn default() -> Self {
        let Ok(value_27) = FontData::default_table_data();

        Self {
            data: value_27,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for LigatureSet<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("LigatureSet")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "ligature_count", value: FieldType::from(self.ligature_count()) }),
            1u64 => Some(Field { name: "ligature_offsets", value: FieldType::from(self.ligatures()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for LigatureSet<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for Ligature<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(component_glyph_ids_byte_range) = self.component_glyph_ids_byte_range();

        Ok(0..component_glyph_ids_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Ligature<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_28) = data.len();

        match ::core::primitive::bool::then_some(value_28 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Ligature<'a> {
    data: FontData<'a>,
}

impl<'a> Ligature<'a> {
    pub const MIN_SIZE: u64 = (GlyphId16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn ligature_glyph(&self) -> Result<GlyphId16, Never> {
        let Ok(range) = self.ligature_glyph_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn component_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.component_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn component_glyph_ids(&self) -> Result<&'a [BigEndian<GlyphId16>], Never> {
        let Ok(range) = self.component_glyph_ids_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn ligature_glyph_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(GlyphId16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn component_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.ligature_glyph_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn component_glyph_ids_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(component_count) = self.component_count();
        let Ok(previous) = self.component_count_byte_range();
        let start = previous.end;
        let Ok(value_29) = transforms::subtract(component_count, 1_u64);

        let end = start.saturating_add(value_29
                .saturating_mul(GlyphId16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Ligature::MIN_SIZE); held }));

impl Default for Ligature<'_> {
    fn default() -> Self {
        let Ok(value_30) = FontData::default_table_data();

        Self {
            data: value_30,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Ligature<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Ligature")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "ligature_glyph", value: FieldType::from(self.ligature_glyph()) }),
            1u64 => Some(Field { name: "component_count", value: FieldType::from(self.component_count()) }),
            2u64 => Some(Field { name: "component_glyph_ids", value: FieldType::from(self.component_glyph_ids()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Ligature<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for ExtensionSubstFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl Discriminant for ExtensionSubstFormat1<'_, ()> {
    fn read_discriminant(data: FontData<'_>) -> Result<u16, ReadError> {
        data.read_at(u16::RAW_BYTE_LEN)
    }
}

impl<'a, T> MinByteRange<'a> for ExtensionSubstFormat1<'a, T> {
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

impl<'a, T> FontRead<'a> for ExtensionSubstFormat1<'a, T> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_31) = data.len();

        match ::core::primitive::bool::then_some(value_31 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self {
            data,
            offset_type: std::marker::PhantomData,
        })
    }
}

impl<'a, T> ExtensionSubstFormat1<'a, T> {

}

#[derive(Clone)]
pub struct ExtensionSubstFormat1<'a, T = ()> {
    data: FontData<'a>,
    offset_type: std::marker::PhantomData<*const T>,
}

impl<'a, T> ExtensionSubstFormat1<'a, T> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + Offset32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn subst_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.subst_format_byte_range();

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

    pub fn subst_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn extension_lookup_type_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.subst_format_byte_range();
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
    ExtensionSubstFormat1::<()>::MIN_SIZE
); held }));

impl<T> Default for ExtensionSubstFormat1<'_, T> {
    fn default() -> Self {
        let Ok(value_32) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_32,
            offset_type: std::marker::PhantomData,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a, T: FontRead<'a> + SomeTable<'a> + 'a> SomeTable<'a> for ExtensionSubstFormat1<'a, T> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ExtensionSubstFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "subst_format", value: FieldType::from(self.subst_format()) }),
            1u64 => Some(Field { name: "extension_lookup_type", value: FieldType::from(self.extension_lookup_type()) }),
            2u64 => Some(Field { name: "extension_offset", value: FieldType::from(FieldType::offset(self.extension_offset(), (self.extension()).map(Some))) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a, T: FontRead<'a> + SomeTable<'a> + 'a> std::fmt::Debug for ExtensionSubstFormat1<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

pub enum ExtensionSubtable<'a> {
    Single(ExtensionSubstFormat1<'a, SingleSubst<'a>>),
    Multiple(ExtensionSubstFormat1<'a, MultipleSubstFormat1<'a>>),
    Alternate(ExtensionSubstFormat1<'a, AlternateSubstFormat1<'a>>),
    Ligature(ExtensionSubstFormat1<'a, LigatureSubstFormat1<'a>>),
    Contextual(ExtensionSubstFormat1<'a, SubstitutionSequenceContext<'a>>),
    ChainContextual(ExtensionSubstFormat1<'a, SubstitutionChainContext<'a>>),
    Reverse(ExtensionSubstFormat1<'a, ReverseChainSingleSubstFormat1<'a>>),
}

impl Default for ExtensionSubtable<'_> {
    fn default() -> Self {
        Self::Single(Default::default())
    }
}

impl<'a> FontRead<'a> for ExtensionSubtable<'a> {
    fn read(bytes: FontData<'a>) -> Result<Self, ReadError> {
        let discriminant = ExtensionSubstFormat1::read_discriminant(bytes)?;

        match discriminant {
            1 => {
                let read = FontRead::read(bytes)?;

                Ok(ExtensionSubtable::Single(read))
            }
            2 => {
                let read = FontRead::read(bytes)?;

                Ok(ExtensionSubtable::Multiple(read))
            }
            3 => {
                let read = FontRead::read(bytes)?;

                Ok(ExtensionSubtable::Alternate(read))
            }
            4 => {
                let read = FontRead::read(bytes)?;

                Ok(ExtensionSubtable::Ligature(read))
            }
            5 => {
                let read = FontRead::read(bytes)?;

                Ok(ExtensionSubtable::Contextual(read))
            }
            6 => {
                let read = FontRead::read(bytes)?;

                Ok(ExtensionSubtable::ChainContextual(read))
            }
            8 => {
                let read = FontRead::read(bytes)?;

                Ok(ExtensionSubtable::Reverse(read))
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
            ExtensionSubtable::Multiple(table) => table,
            ExtensionSubtable::Alternate(table) => table,
            ExtensionSubtable::Ligature(table) => table,
            ExtensionSubtable::Contextual(table) => table,
            ExtensionSubtable::ChainContextual(table) => table,
            ExtensionSubtable::Reverse(table) => table,
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

impl Format<u16> for ReverseChainSingleSubstFormat1<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for ReverseChainSingleSubstFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(substitute_glyph_ids_byte_range) = self.substitute_glyph_ids_byte_range();

        Ok(0..substitute_glyph_ids_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ReverseChainSingleSubstFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_33) = data.len();

        match ::core::primitive::bool::then_some(value_33 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ReverseChainSingleSubstFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> ReverseChainSingleSubstFormat1<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn subst_format(&self) -> Result<u16, Never> {
        let Ok(range) = self.subst_format_byte_range();

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

    pub fn backtrack_glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.backtrack_glyph_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn backtrack_coverage_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.backtrack_coverage_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn backtrack_coverages(&self) -> Result<ArrayOfOffsets<'a, CoverageTable<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.backtrack_coverage_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn lookahead_glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.lookahead_glyph_count_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn lookahead_coverage_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.lookahead_coverage_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn lookahead_coverages(&self) -> Result<ArrayOfOffsets<'a, CoverageTable<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.lookahead_coverage_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.glyph_count_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn substitute_glyph_ids(&self) -> Result<&'a [BigEndian<GlyphId16>], Never> {
        let Ok(range) = self.substitute_glyph_ids_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn subst_format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.subst_format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn backtrack_glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn backtrack_coverage_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(backtrack_glyph_count) = self.backtrack_glyph_count();
        let Ok(previous) = self.backtrack_glyph_count_byte_range();
        let start = previous.end;
        let Ok(value_34) = transforms::to_u64(backtrack_glyph_count);

        let end = start.saturating_add(value_34.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn lookahead_glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.backtrack_coverage_offsets_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn lookahead_coverage_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(lookahead_glyph_count) = self.lookahead_glyph_count();
        let Ok(previous) = self.lookahead_glyph_count_byte_range();
        let start = previous.end;
        let Ok(value_35) = transforms::to_u64(lookahead_glyph_count);

        let end = start.saturating_add(value_35.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.lookahead_coverage_offsets_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn substitute_glyph_ids_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(glyph_count) = self.glyph_count();
        let Ok(previous) = self.glyph_count_byte_range();
        let start = previous.end;
        let Ok(value_36) = transforms::to_u64(glyph_count);

        let end =
            start.saturating_add(value_36.saturating_mul(GlyphId16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    ReverseChainSingleSubstFormat1::MIN_SIZE
); held }));

impl Default for ReverseChainSingleSubstFormat1<'_> {
    fn default() -> Self {
        let Ok(value_37) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_37,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ReverseChainSingleSubstFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ReverseChainSingleSubstFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "subst_format", value: FieldType::from(self.subst_format()) }),
            1u64 => Some(Field { name: "coverage_offset", value: FieldType::from(FieldType::offset(self.coverage_offset(), (self.coverage()).map(Some))) }),
            2u64 => Some(Field { name: "backtrack_glyph_count", value: FieldType::from(self.backtrack_glyph_count()) }),
            3u64 => Some(Field { name: "backtrack_coverage_offsets", value: FieldType::from(self.backtrack_coverages()) }),
            4u64 => Some(Field { name: "lookahead_glyph_count", value: FieldType::from(self.lookahead_glyph_count()) }),
            5u64 => Some(Field { name: "lookahead_coverage_offsets", value: FieldType::from(self.lookahead_coverages()) }),
            6u64 => Some(Field { name: "glyph_count", value: FieldType::from(self.glyph_count()) }),
            7u64 => Some(Field { name: "substitute_glyph_ids", value: FieldType::from(self.substitute_glyph_ids()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ReverseChainSingleSubstFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
