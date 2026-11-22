

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for OtKern<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(subtable_data_byte_range) = self.subtable_data_byte_range();

        Ok(0..subtable_data_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for OtKern<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct OtKern<'a> {
    data: FontData<'a>,
}

impl<'a> OtKern<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<u16, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn n_tables(&self) -> Result<u16, Never> {
        let Ok(range) = self.n_tables_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn subtable_data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.subtable_data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn n_tables_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn subtable_data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.n_tables_byte_range();
        let start = previous.end;
        let Ok(value_22) = self.data.len();

        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_22.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(OtKern::MIN_SIZE); held }));

impl Default for OtKern<'_> {
    fn default() -> Self {
        let Ok(value_2) = FontData::default_table_data();

        Self {
            data: value_2,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for OtKern<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("OtKern")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "n_tables", value: FieldType::from(self.n_tables()) }),
            2u64 => Some(Field { name: "subtable_data", value: FieldType::from(self.subtable_data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for OtKern<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for AatKern<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(subtable_data_byte_range) = self.subtable_data_byte_range();

        Ok(0..subtable_data_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for AatKern<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_3) = data.len();

        match ::core::primitive::bool::then_some(value_3 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct AatKern<'a> {
    data: FontData<'a>,
}

impl<'a> AatKern<'a> {
    pub const MIN_SIZE: u64 = (MajorMinor::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<MajorMinor, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn n_tables(&self) -> Result<u32, Never> {
        let Ok(range) = self.n_tables_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn subtable_data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.subtable_data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(MajorMinor::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn n_tables_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn subtable_data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.n_tables_byte_range();
        let start = previous.end;
        let Ok(value_23) = self.data.len();

        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_23.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(AatKern::MIN_SIZE); held }));

impl Default for AatKern<'_> {
    fn default() -> Self {
        let Ok(value_4) = FontData::default_table_data();

        Self {
            data: value_4,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for AatKern<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("AatKern")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "n_tables", value: FieldType::from(self.n_tables()) }),
            2u64 => Some(Field { name: "subtable_data", value: FieldType::from(self.subtable_data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for AatKern<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for OtSubtable<'a> {
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

impl<'a> FontRead<'a> for OtSubtable<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_5) = data.len();

        match ::core::primitive::bool::then_some(value_5 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct OtSubtable<'a> {
    data: FontData<'a>,
}

impl<'a> OtSubtable<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<u16, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn length(&self) -> Result<u16, Never> {
        let Ok(range) = self.length_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage(&self) -> Result<u16, Never> {
        let Ok(range) = self.coverage_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn length_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coverage_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.length_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_byte_range();
        let start = previous.end;
        let Ok(value_24) = self.data.len();

        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_24.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(OtSubtable::MIN_SIZE); held }));

impl Default for OtSubtable<'_> {
    fn default() -> Self {
        let Ok(value_6) = FontData::default_table_data();

        Self {
            data: value_6,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for OtSubtable<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("OtSubtable")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "length", value: FieldType::from(self.length()) }),
            2u64 => Some(Field { name: "coverage", value: FieldType::from(self.coverage()) }),
            3u64 => Some(Field { name: "data", value: FieldType::from(self.data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for OtSubtable<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for AatSubtable<'a> {
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

impl<'a> FontRead<'a> for AatSubtable<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_7) = data.len();

        match ::core::primitive::bool::then_some(value_7 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct AatSubtable<'a> {
    data: FontData<'a>,
}

impl<'a> AatSubtable<'a> {
    pub const MIN_SIZE: u64 = (u32::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn length(&self) -> Result<u32, Never> {
        let Ok(range) = self.length_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage(&self) -> Result<u16, Never> {
        let Ok(range) = self.coverage_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn tuple_index(&self) -> Result<u16, Never> {
        let Ok(range) = self.tuple_index_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn length_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn coverage_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.length_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn tuple_index_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.tuple_index_byte_range();
        let start = previous.end;
        let Ok(value_25) = self.data.len();

        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_25.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(AatSubtable::MIN_SIZE); held }));

impl Default for AatSubtable<'_> {
    fn default() -> Self {
        let Ok(value_8) = FontData::default_table_data();

        Self {
            data: value_8,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for AatSubtable<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("AatSubtable")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "length", value: FieldType::from(self.length()) }),
            1u64 => Some(Field { name: "coverage", value: FieldType::from(self.coverage()) }),
            2u64 => Some(Field { name: "tuple_index", value: FieldType::from(self.tuple_index()) }),
            3u64 => Some(Field { name: "data", value: FieldType::from(self.data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for AatSubtable<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for Subtable0<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(pairs_byte_range) = self.pairs_byte_range();

        Ok(0..pairs_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Subtable0<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_9) = data.len();

        match ::core::primitive::bool::then_some(value_9 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Subtable0<'a> {
    data: FontData<'a>,
}

impl<'a> Subtable0<'a> {
    pub const MIN_SIZE: u64 =
        (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn n_pairs(&self) -> Result<u16, Never> {
        let Ok(range) = self.n_pairs_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn search_range(&self) -> Result<u16, Never> {
        let Ok(range) = self.search_range_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn entry_selector(&self) -> Result<u16, Never> {
        let Ok(range) = self.entry_selector_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn range_shift(&self) -> Result<u16, Never> {
        let Ok(range) = self.range_shift_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn pairs(&self) -> Result<&'a [Subtable0Pair], Never> {
        let Ok(range) = self.pairs_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn n_pairs_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn search_range_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.n_pairs_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn entry_selector_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.search_range_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn range_shift_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.entry_selector_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn pairs_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(n_pairs) = self.n_pairs();
        let Ok(previous) = self.range_shift_byte_range();
        let start = previous.end;
        let Ok(value_10) = transforms::to_u64(n_pairs);

        let end =
            start.saturating_add(value_10.saturating_mul(Subtable0Pair::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Subtable0::MIN_SIZE); held }));

impl Default for Subtable0<'_> {
    fn default() -> Self {
        let Ok(value_11) = FontData::default_table_data();

        Self {
            data: value_11,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Subtable0<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Subtable0")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "n_pairs", value: FieldType::from(self.n_pairs()) }),
            1u64 => Some(Field { name: "search_range", value: FieldType::from(self.search_range()) }),
            2u64 => Some(Field { name: "entry_selector", value: FieldType::from(self.entry_selector()) }),
            3u64 => Some(Field { name: "range_shift", value: FieldType::from(self.range_shift()) }),
            4u64 => {
                let Ok(value_12) = self.offset_data();

                Some(Field { name: "pairs", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(Subtable0Pair),
                    { let Ok(explicit_value) = self.pairs(); explicit_value },
                    value_12,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Subtable0<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for Subtable2ClassTable<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(offsets_byte_range) = self.offsets_byte_range();

        Ok(0..offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Subtable2ClassTable<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_13) = data.len();

        match ::core::primitive::bool::then_some(value_13 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Subtable2ClassTable<'a> {
    data: FontData<'a>,
}

impl<'a> Subtable2ClassTable<'a> {
    pub const MIN_SIZE: u64 = (GlyphId16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn first_glyph(&self) -> Result<GlyphId16, Never> {
        let Ok(range) = self.first_glyph_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn n_glyphs(&self) -> Result<u16, Never> {
        let Ok(range) = self.n_glyphs_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn offsets(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn first_glyph_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(GlyphId16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn n_glyphs_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.first_glyph_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(n_glyphs) = self.n_glyphs();
        let Ok(previous) = self.n_glyphs_byte_range();
        let start = previous.end;
        let Ok(value_14) = transforms::to_u64(n_glyphs);

        let end = start.saturating_add(value_14.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    Subtable2ClassTable::MIN_SIZE
); held }));

impl Default for Subtable2ClassTable<'_> {
    fn default() -> Self {
        let Ok(value_15) = FontData::default_table_data();

        Self {
            data: value_15,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Subtable2ClassTable<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Subtable2ClassTable")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "first_glyph", value: FieldType::from(self.first_glyph()) }),
            1u64 => Some(Field { name: "n_glyphs", value: FieldType::from(self.n_glyphs()) }),
            2u64 => Some(Field { name: "offsets", value: FieldType::from(self.offsets()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Subtable2ClassTable<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for Subtable3<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(kern_index_byte_range) = self.kern_index_byte_range();

        Ok(0..kern_index_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Subtable3<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_16) = data.len();

        match ::core::primitive::bool::then_some(value_16 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Subtable3<'a> {
    data: FontData<'a>,
}

impl<'a> Subtable3<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN
        + u8::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn glyph_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.glyph_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn kern_value_count(&self) -> Result<u8, Never> {
        let Ok(range) = self.kern_value_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn left_class_count(&self) -> Result<u8, Never> {
        let Ok(range) = self.left_class_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn right_class_count(&self) -> Result<u8, Never> {
        let Ok(range) = self.right_class_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn flags(&self) -> Result<u8, Never> {
        let Ok(range) = self.flags_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn kern_value(&self) -> Result<&'a [BigEndian<i16>], Never> {
        let Ok(range) = self.kern_value_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn left_class(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.left_class_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn right_class(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.right_class_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn kern_index(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.kern_index_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn glyph_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn kern_value_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.glyph_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn left_class_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.kern_value_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn right_class_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.left_class_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn flags_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.right_class_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn kern_value_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(kern_value_count) = self.kern_value_count();
        let Ok(previous) = self.flags_byte_range();
        let start = previous.end;
        let Ok(value_17) = transforms::to_u64(kern_value_count);

        let end =
            start.saturating_add(value_17.saturating_mul(i16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn left_class_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(glyph_count) = self.glyph_count();
        let Ok(previous) = self.kern_value_byte_range();
        let start = previous.end;
        let Ok(value_18) = transforms::to_u64(glyph_count);

        let end = start.saturating_add(value_18.saturating_mul(u8::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn right_class_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(glyph_count) = self.glyph_count();
        let Ok(previous) = self.left_class_byte_range();
        let start = previous.end;
        let Ok(value_19) = transforms::to_u64(glyph_count);

        let end = start.saturating_add(value_19.saturating_mul(u8::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn kern_index_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(left_class_count) = self.left_class_count();
        let Ok(right_class_count) = self.right_class_count();
        let Ok(previous) = self.right_class_byte_range();
        let start = previous.end;
        let Ok(value_20) = transforms::add_multiply(left_class_count, 0_u64, right_class_count);

        let end = start.saturating_add(value_20
                .saturating_mul(u8::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Subtable3::MIN_SIZE); held }));

impl Default for Subtable3<'_> {
    fn default() -> Self {
        let Ok(value_21) = FontData::default_table_data();

        Self {
            data: value_21,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Subtable3<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Subtable3")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "glyph_count", value: FieldType::from(self.glyph_count()) }),
            1u64 => Some(Field { name: "kern_value_count", value: FieldType::from(self.kern_value_count()) }),
            2u64 => Some(Field { name: "left_class_count", value: FieldType::from(self.left_class_count()) }),
            3u64 => Some(Field { name: "right_class_count", value: FieldType::from(self.right_class_count()) }),
            4u64 => Some(Field { name: "flags", value: FieldType::from(self.flags()) }),
            5u64 => Some(Field { name: "kern_value", value: FieldType::from(self.kern_value()) }),
            6u64 => Some(Field { name: "left_class", value: FieldType::from(self.left_class()) }),
            7u64 => Some(Field { name: "right_class", value: FieldType::from(self.right_class()) }),
            8u64 => Some(Field { name: "kern_index", value: FieldType::from(self.kern_index()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Subtable3<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
