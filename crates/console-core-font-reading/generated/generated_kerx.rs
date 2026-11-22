

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Kerx<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(subtables_byte_range) = self.subtables_byte_range();

        Ok(0..subtables_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Kerx<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"kerx"); value });
}

impl<'a> FontRead<'a> for Kerx<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Kerx<'a> {
    data: FontData<'a>,
}

impl<'a> Kerx<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<u16, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn n_tables(&self) -> Result<u32, Never> {
        let Ok(range) = self.n_tables_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn subtables(&self) -> Result<VarLenArray<'a, Subtable<'a>>, Never> {
        let Ok(range) = self.subtables_byte_range();

        self.data.read_var_array_or_empty(range.start)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn padding_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn n_tables_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.padding_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn subtables_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(n_tables) = self.n_tables();
        let Ok(previous) = self.n_tables_byte_range();
        let start = previous.end;
        let end = start.saturating_add({ let Ok(explicit_value) = transforms::total_len_for_count::<Subtable, _>(self.data, start, n_tables); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Kerx::MIN_SIZE); held }));

impl Default for Kerx<'_> {
    fn default() -> Self {
        let Ok(value_3) = FontData::default_table_data();

        Self {
            data: value_3,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Kerx<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Kerx")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "n_tables", value: FieldType::from(self.n_tables()) }),
            2u64 => {
                let Ok(value_4) = self.offset_data();

                let Ok(subtables) = self.subtables();

                Some(Field { name: "subtables", value: FieldType::from(traversal::FieldType::var_array("Subtable", subtables, value_4)) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Kerx<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for Subtable<'a> {
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

impl<'a> FontRead<'a> for Subtable<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_5) = data.len();

        match ::core::primitive::bool::then_some(value_5 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Subtable<'a> {
    data: FontData<'a>,
}

impl<'a> Subtable<'a> {
    pub const MIN_SIZE: u64 = (u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn length(&self) -> Result<u32, Never> {
        let Ok(range) = self.length_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn coverage(&self) -> Result<u32, Never> {
        let Ok(range) = self.coverage_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn tuple_count(&self) -> Result<u32, Never> {
        let Ok(range) = self.tuple_count_byte_range();

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
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn tuple_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.coverage_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.tuple_count_byte_range();
        let start = previous.end;
        let Ok(value_14) = self.data.len();

        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_14.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Subtable::MIN_SIZE); held }));

impl Default for Subtable<'_> {
    fn default() -> Self {
        let Ok(value_6) = FontData::default_table_data();

        Self {
            data: value_6,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Subtable<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Subtable")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "length", value: FieldType::from(self.length()) }),
            1u64 => Some(Field { name: "coverage", value: FieldType::from(self.coverage()) }),
            2u64 => Some(Field { name: "tuple_count", value: FieldType::from(self.tuple_count()) }),
            3u64 => Some(Field { name: "data", value: FieldType::from(self.data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Subtable<'a> {
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
        let Ok(value_7) = data.len();

        match ::core::primitive::bool::then_some(value_7 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
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
        (u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn n_pairs(&self) -> Result<u32, Never> {
        let Ok(range) = self.n_pairs_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn search_range(&self) -> Result<u32, Never> {
        let Ok(range) = self.search_range_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn entry_selector(&self) -> Result<u32, Never> {
        let Ok(range) = self.entry_selector_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn range_shift(&self) -> Result<u32, Never> {
        let Ok(range) = self.range_shift_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn pairs(&self) -> Result<&'a [Subtable0Pair], Never> {
        let Ok(range) = self.pairs_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn n_pairs_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn search_range_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.n_pairs_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn entry_selector_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.search_range_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn range_shift_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.entry_selector_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn pairs_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(n_pairs) = self.n_pairs();
        let Ok(previous) = self.range_shift_byte_range();
        let start = previous.end;
        let Ok(value_8) = transforms::to_u64(n_pairs);

        let end =
            start.saturating_add(value_8.saturating_mul(Subtable0Pair::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Subtable0::MIN_SIZE); held }));

impl Default for Subtable0<'_> {
    fn default() -> Self {
        let Ok(value_9) = FontData::default_table_data();

        Self {
            data: value_9,
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
                let Ok(value_10) = self.offset_data();

                Some(Field { name: "pairs", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(Subtable0Pair),
                    { let Ok(explicit_value) = self.pairs(); explicit_value },
                    value_10,
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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct Subtable0Pair {
    pub left: BigEndian<GlyphId16>,
    pub right: BigEndian<GlyphId16>,
    pub value: BigEndian<i16>,
}

const _: () = {
  assert!(core::mem::size_of::<Subtable0Pair>() == 6);
  assert!(core::mem::align_of::<Subtable0Pair>() == 1);
  assert!(core::mem::offset_of!(Subtable0Pair, left) == 0);
  assert!(core::mem::offset_of!(Subtable0Pair, right) == 2);
  assert!(core::mem::offset_of!(Subtable0Pair, value) == 4);
};

impl Subtable0Pair {
    pub fn left(&self) -> Result<GlyphId16, Never> {
        self.left.get()
    }

    pub fn right(&self) -> Result<GlyphId16, Never> {
        self.right.get()
    }

    pub fn value(&self) -> Result<i16, Never> {
        self.value.get()
    }
}

impl FixedSize for Subtable0Pair {
    const RAW_BYTE_LEN: u64 =
        GlyphId16::RAW_BYTE_LEN + GlyphId16::RAW_BYTE_LEN + i16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for Subtable0Pair {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "Subtable0Pair",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "left", value: FieldType::from(self.left()) }),
                1u64 => Some(Field { name: "right", value: FieldType::from(self.right()) }),
                2u64 => Some(Field { name: "value", value: FieldType::from(self.value()) }),
                _ => None,
            }),
            data,
        })
    }
}
