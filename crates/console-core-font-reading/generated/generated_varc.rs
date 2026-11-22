

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Varc<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(var_composite_glyphs_offset_byte_range) = self.var_composite_glyphs_offset_byte_range();

        Ok(0..var_composite_glyphs_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Varc<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"VARC"); value });
}

impl<'a> FontRead<'a> for Varc<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Varc<'a> {
    data: FontData<'a>,
}

impl<'a> Varc<'a> {
    pub const MIN_SIZE: u64 = (MajorMinor::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<MajorMinor, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage_offset(&self) -> Result<Offset32, Never> {
        let Ok(range) = self.coverage_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage(&self) -> Result<CoverageTable<'a>, ReadError> {
        let data = self.data;
        let Ok(coverage_offset) = self.coverage_offset();

        coverage_offset.resolve(data)
    }

    pub fn multi_var_store_offset(&self) -> Result<Nullable<Offset32>, Never> {
        let Ok(range) = self.multi_var_store_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn multi_var_store(&self) -> Result<Option<MultiItemVariationStore<'a>>, ReadError> {
        let data = self.data;
        let Ok(multi_var_store_offset) = self.multi_var_store_offset();

        multi_var_store_offset.resolve(data)
    }

    pub fn condition_list_offset(&self) -> Result<Nullable<Offset32>, Never> {
        let Ok(range) = self.condition_list_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn condition_list(&self) -> Result<Option<ConditionList<'a>>, ReadError> {
        let data = self.data;
        let Ok(condition_list_offset) = self.condition_list_offset();

        condition_list_offset.resolve(data)
    }

    pub fn axis_indices_list_offset(&self) -> Result<Nullable<Offset32>, Never> {
        let Ok(range) = self.axis_indices_list_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn axis_indices_list(&self) -> Result<Option<Index2<'a>>, ReadError> {
        let data = self.data;
        let Ok(axis_indices_list_offset) = self.axis_indices_list_offset();

        axis_indices_list_offset.resolve(data)
    }

    pub fn var_composite_glyphs_offset(&self) -> Result<Offset32, Never> {
        let Ok(range) = self.var_composite_glyphs_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn var_composite_glyphs(&self) -> Result<Index2<'a>, ReadError> {
        let data = self.data;
        let Ok(var_composite_glyphs_offset) = self.var_composite_glyphs_offset();

        var_composite_glyphs_offset.resolve(data)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(MajorMinor::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coverage_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn multi_var_store_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn condition_list_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.multi_var_store_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn axis_indices_list_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.condition_list_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn var_composite_glyphs_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.axis_indices_list_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Varc::MIN_SIZE); held }));

impl Default for Varc<'_> {
    fn default() -> Self {
        let Ok(value_2) = FontData::default_table_data();

        Self {
            data: value_2,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Varc<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Varc")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "coverage_offset", value: FieldType::from(FieldType::offset(self.coverage_offset(), (self.coverage()).map(Some))) }),
            2u64 => Some(Field { name: "multi_var_store_offset", value: FieldType::from(FieldType::offset(self.multi_var_store_offset(), self.multi_var_store())) }),
            3u64 => Some(Field { name: "condition_list_offset", value: FieldType::from(FieldType::offset(self.condition_list_offset(), self.condition_list())) }),
            4u64 => Some(Field { name: "axis_indices_list_offset", value: FieldType::from(FieldType::offset(self.axis_indices_list_offset(), self.axis_indices_list())) }),
            5u64 => Some(Field { name: "var_composite_glyphs_offset", value: FieldType::from(FieldType::offset(
                    self.var_composite_glyphs_offset(),
                    (self.var_composite_glyphs()).map(Some),
                )) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Varc<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u16> for MultiItemVariationStore<'_> {
    const FORMAT: u16 = 1;
}

impl<'a> MinByteRange<'a> for MultiItemVariationStore<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(variation_data_offsets_byte_range) = self.variation_data_offsets_byte_range();

        Ok(0..variation_data_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for MultiItemVariationStore<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_3) = data.len();

        match ::core::primitive::bool::then_some(value_3 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct MultiItemVariationStore<'a> {
    data: FontData<'a>,
}

impl<'a> MultiItemVariationStore<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + Offset32::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn region_list_offset(&self) -> Result<Offset32, Never> {
        let Ok(range) = self.region_list_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn region_list(&self) -> Result<SparseVariationRegionList<'a>, ReadError> {
        let data = self.data;
        let Ok(region_list_offset) = self.region_list_offset();

        region_list_offset.resolve(data)
    }

    pub fn variation_data_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.variation_data_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn variation_data_offsets(&self) -> Result<&'a [BigEndian<Offset32>], Never> {
        let Ok(range) = self.variation_data_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn variation_data(&self) -> Result<ArrayOfOffsets<'a, MultiItemVariationData<'a>, Offset32>, Never> {
        let data = self.data;
        let Ok(offsets) = self.variation_data_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn region_list_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn variation_data_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.region_list_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn variation_data_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(variation_data_count) = self.variation_data_count();
        let Ok(previous) = self.variation_data_count_byte_range();
        let start = previous.end;
        let Ok(value_4) = transforms::to_u64(variation_data_count);

        let end = start.saturating_add(value_4.saturating_mul(Offset32::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    MultiItemVariationStore::MIN_SIZE
); held }));

impl Default for MultiItemVariationStore<'_> {
    fn default() -> Self {
        let Ok(value_5) = FontData::default_format_1_u16_table_data();

        Self {
            data: value_5,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for MultiItemVariationStore<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("MultiItemVariationStore")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "region_list_offset", value: FieldType::from(FieldType::offset(self.region_list_offset(), (self.region_list()).map(Some))) }),
            2u64 => Some(Field { name: "variation_data_count", value: FieldType::from(self.variation_data_count()) }),
            3u64 => Some(Field { name: "variation_data_offsets", value: FieldType::from(self.variation_data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for MultiItemVariationStore<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for SparseVariationRegionList<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(region_offsets_byte_range) = self.region_offsets_byte_range();

        Ok(0..region_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for SparseVariationRegionList<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_6) = data.len();

        match ::core::primitive::bool::then_some(value_6 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct SparseVariationRegionList<'a> {
    data: FontData<'a>,
}

impl<'a> SparseVariationRegionList<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn region_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.region_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn region_offsets(&self) -> Result<&'a [BigEndian<Offset32>], Never> {
        let Ok(range) = self.region_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn regions(&self) -> Result<ArrayOfOffsets<'a, SparseVariationRegion<'a>, Offset32>, Never> {
        let data = self.data;
        let Ok(offsets) = self.region_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn region_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn region_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(region_count) = self.region_count();
        let Ok(previous) = self.region_count_byte_range();
        let start = previous.end;
        let Ok(value_7) = transforms::to_u64(region_count);

        let end =
            start.saturating_add(value_7.saturating_mul(Offset32::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    SparseVariationRegionList::MIN_SIZE
); held }));

impl Default for SparseVariationRegionList<'_> {
    fn default() -> Self {
        let Ok(value_8) = FontData::default_table_data();

        Self {
            data: value_8,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for SparseVariationRegionList<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("SparseVariationRegionList")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "region_count", value: FieldType::from(self.region_count()) }),
            1u64 => Some(Field { name: "region_offsets", value: FieldType::from(self.regions()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for SparseVariationRegionList<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for SparseVariationRegion<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(region_axes_byte_range) = self.region_axes_byte_range();

        Ok(0..region_axes_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for SparseVariationRegion<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_9) = data.len();

        match ::core::primitive::bool::then_some(value_9 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct SparseVariationRegion<'a> {
    data: FontData<'a>,
}

impl<'a> SparseVariationRegion<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn region_axis_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.region_axis_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn region_axes(&self) -> Result<&'a [SparseRegionAxisCoordinates], Never> {
        let Ok(range) = self.region_axes_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn region_axis_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn region_axes_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(region_axis_count) = self.region_axis_count();
        let Ok(previous) = self.region_axis_count_byte_range();
        let start = previous.end;
        let Ok(value_10) = transforms::to_u64(region_axis_count);

        let end = start.saturating_add(value_10
                .saturating_mul(SparseRegionAxisCoordinates::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    SparseVariationRegion::MIN_SIZE
); held }));

impl Default for SparseVariationRegion<'_> {
    fn default() -> Self {
        let Ok(value_11) = FontData::default_table_data();

        Self {
            data: value_11,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for SparseVariationRegion<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("SparseVariationRegion")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "region_axis_count", value: FieldType::from(self.region_axis_count()) }),
            1u64 => {
                let Ok(value_12) = self.offset_data();

                Some(Field { name: "region_axes", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(SparseRegionAxisCoordinates),
                    { let Ok(explicit_value) = self.region_axes(); explicit_value },
                    value_12,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for SparseVariationRegion<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct SparseRegionAxisCoordinates {
    pub axis_index: BigEndian<u16>,
    pub start: BigEndian<F2Dot14>,
    pub peak: BigEndian<F2Dot14>,
    pub end: BigEndian<F2Dot14>,
}

const _: () = {
  assert!(core::mem::size_of::<SparseRegionAxisCoordinates>() == 8);
  assert!(core::mem::align_of::<SparseRegionAxisCoordinates>() == 1);
  assert!(core::mem::offset_of!(SparseRegionAxisCoordinates, axis_index) == 0);
  assert!(core::mem::offset_of!(SparseRegionAxisCoordinates, start) == 2);
  assert!(core::mem::offset_of!(SparseRegionAxisCoordinates, peak) == 4);
  assert!(core::mem::offset_of!(SparseRegionAxisCoordinates, end) == 6);
};

impl SparseRegionAxisCoordinates {
    pub fn axis_index(&self) -> Result<u16, Never> {
        self.axis_index.get()
    }

    pub fn start(&self) -> Result<F2Dot14, Never> {
        self.start.get()
    }

    pub fn peak(&self) -> Result<F2Dot14, Never> {
        self.peak.get()
    }

    pub fn end(&self) -> Result<F2Dot14, Never> {
        self.end.get()
    }
}

impl FixedSize for SparseRegionAxisCoordinates {
    const RAW_BYTE_LEN: u64 =
        u16::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for SparseRegionAxisCoordinates {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "SparseRegionAxisCoordinates",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "axis_index", value: FieldType::from(self.axis_index()) }),
                1u64 => Some(Field { name: "start", value: FieldType::from(self.start()) }),
                2u64 => Some(Field { name: "peak", value: FieldType::from(self.peak()) }),
                3u64 => Some(Field { name: "end", value: FieldType::from(self.end()) }),
                _ => None,
            }),
            data,
        })
    }
}

impl Format<u8> for MultiItemVariationData<'_> {
    const FORMAT: u8 = 1;
}

impl<'a> MinByteRange<'a> for MultiItemVariationData<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(raw_delta_sets_byte_range) = self.raw_delta_sets_byte_range();

        Ok(0..raw_delta_sets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for MultiItemVariationData<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_17) = data.len();

        match ::core::primitive::bool::then_some(value_17 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct MultiItemVariationData<'a> {
    data: FontData<'a>,
}

impl<'a> MultiItemVariationData<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn region_index_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.region_index_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn region_indices(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.region_indices_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn raw_delta_sets(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.raw_delta_sets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn region_index_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn region_indices_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(region_index_count) = self.region_index_count();
        let Ok(previous) = self.region_index_count_byte_range();
        let start = previous.end;
        let Ok(value_18) = transforms::to_u64(region_index_count);

        let end =
            start.saturating_add(value_18.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn raw_delta_sets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.region_indices_byte_range();
        let start = previous.end;
        let Ok(value_26) = self.data.len();

        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_26.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    MultiItemVariationData::MIN_SIZE
); held }));

impl Default for MultiItemVariationData<'_> {
    fn default() -> Self {
        let Ok(value_19) = FontData::default_format_1_u8_table_data();

        Self {
            data: value_19,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for MultiItemVariationData<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("MultiItemVariationData")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "region_index_count", value: FieldType::from(self.region_index_count()) }),
            2u64 => Some(Field { name: "region_indices", value: FieldType::from(self.region_indices()) }),
            3u64 => Some(Field { name: "raw_delta_sets", value: FieldType::from(self.raw_delta_sets()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for MultiItemVariationData<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for ConditionList<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(condition_offsets_byte_range) = self.condition_offsets_byte_range();

        Ok(0..condition_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ConditionList<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_20) = data.len();

        match ::core::primitive::bool::then_some(value_20 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ConditionList<'a> {
    data: FontData<'a>,
}

impl<'a> ConditionList<'a> {
    pub const MIN_SIZE: u64 = u32::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn condition_count(&self) -> Result<u32, Never> {
        let Ok(range) = self.condition_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn condition_offsets(&self) -> Result<&'a [BigEndian<Offset32>], Never> {
        let Ok(range) = self.condition_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn conditions(&self) -> Result<ArrayOfOffsets<'a, Condition<'a>, Offset32>, Never> {
        let data = self.data;
        let Ok(offsets) = self.condition_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn condition_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn condition_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(condition_count) = self.condition_count();
        let Ok(previous) = self.condition_count_byte_range();
        let start = previous.end;
        let Ok(value_21) = transforms::to_u64(condition_count);

        let end =
            start.saturating_add(value_21.saturating_mul(Offset32::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(ConditionList::MIN_SIZE); held }));

impl Default for ConditionList<'_> {
    fn default() -> Self {
        let Ok(value_22) = FontData::default_table_data();

        Self {
            data: value_22,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ConditionList<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ConditionList")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "condition_count", value: FieldType::from(self.condition_count()) }),
            1u64 => Some(Field { name: "condition_offsets", value: FieldType::from(self.conditions()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ConditionList<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck :: AnyBitPattern)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct VarcFlags {
    bits: u32,
}

impl VarcFlags {
    pub const RESET_UNSPECIFIED_AXES: Self = Self {
        bits: 0b0000_0000_0000_0001,
    };

    pub const HAVE_AXES: Self = Self {
        bits: 0b0000_0000_0000_0010,
    };

    pub const AXIS_VALUES_HAVE_VARIATION: Self = Self {
        bits: 0b0000_0000_0000_0100,
    };

    pub const TRANSFORM_HAS_VARIATION: Self = Self {
        bits: 0b0000_0000_0000_1000,
    };

    pub const HAVE_TRANSLATE_X: Self = Self {
        bits: 0b0000_0000_0001_0000,
    };

    pub const HAVE_TRANSLATE_Y: Self = Self {
        bits: 0b0000_0000_0010_0000,
    };

    pub const HAVE_ROTATION: Self = Self {
        bits: 0b0000_0000_0100_0000,
    };

    pub const HAVE_CONDITION: Self = Self {
        bits: 0b0000_0000_1000_0000,
    };

    pub const HAVE_SCALE_X: Self = Self {
        bits: 0b0000_0001_0000_0000,
    };

    pub const HAVE_SCALE_Y: Self = Self {
        bits: 0b0000_0010_0000_0000,
    };

    pub const HAVE_TCENTER_X: Self = Self {
        bits: 0b0000_0100_0000_0000,
    };

    pub const HAVE_TCENTER_Y: Self = Self {
        bits: 0b0000_1000_0000_0000,
    };

    pub const GID_IS_24BIT: Self = Self {
        bits: 0b0001_0000_0000_0000,
    };

    pub const HAVE_SKEW_X: Self = Self {
        bits: 0b0010_0000_0000_0000,
    };

    pub const HAVE_SKEW_Y: Self = Self {
        bits: 0b0100_0000_0000_0000,
    };

    pub const RESERVED_MASK: Self = Self { bits: 0xFFFF8000 };
}

impl VarcFlags {
    const KNOWN: u32 = Self::RESET_UNSPECIFIED_AXES.bits
        | Self::HAVE_AXES.bits
        | Self::AXIS_VALUES_HAVE_VARIATION.bits
        | Self::TRANSFORM_HAS_VARIATION.bits
        | Self::HAVE_TRANSLATE_X.bits
        | Self::HAVE_TRANSLATE_Y.bits
        | Self::HAVE_ROTATION.bits
        | Self::HAVE_CONDITION.bits
        | Self::HAVE_SCALE_X.bits
        | Self::HAVE_SCALE_Y.bits
        | Self::HAVE_TCENTER_X.bits
        | Self::HAVE_TCENTER_Y.bits
        | Self::GID_IS_24BIT.bits
        | Self::HAVE_SKEW_X.bits
        | Self::HAVE_SKEW_Y.bits
        | Self::RESERVED_MASK.bits;

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

impl std::ops::BitOr for VarcFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, other: VarcFlags) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl std::ops::BitOrAssign for VarcFlags {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.bits |= other.bits;
    }
}

impl std::ops::BitXor for VarcFlags {
    type Output = Self;

    #[inline]
    fn bitxor(self, other: Self) -> Self {
        Self {
            bits: self.bits ^ other.bits,
        }
    }
}

impl std::ops::BitXorAssign for VarcFlags {
    #[inline]
    fn bitxor_assign(&mut self, other: Self) {
        self.bits ^= other.bits;
    }
}

impl std::ops::BitAnd for VarcFlags {
    type Output = Self;

    #[inline]
    fn bitand(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }
}

impl std::ops::BitAndAssign for VarcFlags {
    #[inline]
    fn bitand_assign(&mut self, other: Self) {
        self.bits &= other.bits;
    }
}

impl std::ops::Sub for VarcFlags {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }
}

impl std::ops::SubAssign for VarcFlags {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.bits &= !other.bits;
    }
}

impl std::ops::Not for VarcFlags {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        Self { bits: !self.bits & Self::KNOWN }
    }
}

impl std::fmt::Debug for VarcFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let members: &[(&str, Self)] = &[
            ("RESET_UNSPECIFIED_AXES", Self::RESET_UNSPECIFIED_AXES),
            ("HAVE_AXES", Self::HAVE_AXES),
            (
                "AXIS_VALUES_HAVE_VARIATION",
                Self::AXIS_VALUES_HAVE_VARIATION,
            ),
            ("TRANSFORM_HAS_VARIATION", Self::TRANSFORM_HAS_VARIATION),
            ("HAVE_TRANSLATE_X", Self::HAVE_TRANSLATE_X),
            ("HAVE_TRANSLATE_Y", Self::HAVE_TRANSLATE_Y),
            ("HAVE_ROTATION", Self::HAVE_ROTATION),
            ("HAVE_CONDITION", Self::HAVE_CONDITION),
            ("HAVE_SCALE_X", Self::HAVE_SCALE_X),
            ("HAVE_SCALE_Y", Self::HAVE_SCALE_Y),
            ("HAVE_TCENTER_X", Self::HAVE_TCENTER_X),
            ("HAVE_TCENTER_Y", Self::HAVE_TCENTER_Y),
            ("GID_IS_24BIT", Self::GID_IS_24BIT),
            ("HAVE_SKEW_X", Self::HAVE_SKEW_X),
            ("HAVE_SKEW_Y", Self::HAVE_SKEW_Y),
            ("RESERVED_MASK", Self::RESERVED_MASK),
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

impl std::fmt::Binary for VarcFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Binary::fmt(&self.bits, f)
    }
}

impl std::fmt::Octal for VarcFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Octal::fmt(&self.bits, f)
    }
}

impl std::fmt::LowerHex for VarcFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.bits, f)
    }
}

impl std::fmt::UpperHex for VarcFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.bits, f)
    }
}

impl font_types::Scalar for VarcFlags {
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
impl<'a> From<VarcFlags> for FieldType<'a> {
    fn from(src: VarcFlags) -> FieldType<'a> {
        <crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u32>>::from(src.bits)
    }
}
