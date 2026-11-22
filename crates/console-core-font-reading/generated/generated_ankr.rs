

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Ankr<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(glyph_data_table_offset_byte_range) = self.glyph_data_table_offset_byte_range();

        Ok(0..glyph_data_table_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Ankr<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"ankr"); value });
}

impl<'a> FontRead<'a> for Ankr<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Ankr<'a> {
    data: FontData<'a>,
}

impl<'a> Ankr<'a> {
    pub const MIN_SIZE: u64 =
        (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + Offset32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<u16, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn flags(&self) -> Result<u16, Never> {
        let Ok(range) = self.flags_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn lookup_table_offset(&self) -> Result<Offset32, Never> {
        let Ok(range) = self.lookup_table_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn lookup_table(&self) -> Result<LookupU16<'a>, ReadError> {
        let data = self.data;
        let Ok(lookup_table_offset) = self.lookup_table_offset();

        lookup_table_offset.resolve(data)
    }

    pub fn glyph_data_table_offset(&self) -> Result<u32, Never> {
        let Ok(range) = self.glyph_data_table_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn flags_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn lookup_table_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.flags_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_data_table_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.lookup_table_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Ankr::MIN_SIZE); held }));

impl Default for Ankr<'_> {
    fn default() -> Self {
        let Ok(value_2) = FontData::default_table_data();

        Self {
            data: value_2,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Ankr<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Ankr")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "flags", value: FieldType::from(self.flags()) }),
            2u64 => Some(Field { name: "lookup_table_offset", value: FieldType::from(FieldType::offset(self.lookup_table_offset(), (self.lookup_table()).map(Some))) }),
            3u64 => Some(Field { name: "glyph_data_table_offset", value: FieldType::from(self.glyph_data_table_offset()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Ankr<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for GlyphDataEntry<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(anchor_points_byte_range) = self.anchor_points_byte_range();

        Ok(0..anchor_points_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for GlyphDataEntry<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_3) = data.len();

        match ::core::primitive::bool::then_some(value_3 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct GlyphDataEntry<'a> {
    data: FontData<'a>,
}

impl<'a> GlyphDataEntry<'a> {
    pub const MIN_SIZE: u64 = u32::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn num_points(&self) -> Result<u32, Never> {
        let Ok(range) = self.num_points_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn anchor_points(&self) -> Result<&'a [AnchorPoint], Never> {
        let Ok(range) = self.anchor_points_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn num_points_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn anchor_points_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_points) = self.num_points();
        let Ok(previous) = self.num_points_byte_range();
        let start = previous.end;
        let Ok(value_4) = transforms::to_u64(num_points);

        let end =
            start.saturating_add(value_4.saturating_mul(AnchorPoint::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(GlyphDataEntry::MIN_SIZE); held }));

impl Default for GlyphDataEntry<'_> {
    fn default() -> Self {
        let Ok(value_5) = FontData::default_table_data();

        Self {
            data: value_5,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for GlyphDataEntry<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("GlyphDataEntry")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "num_points", value: FieldType::from(self.num_points()) }),
            1u64 => {
                let Ok(value_6) = self.offset_data();

                Some(Field { name: "anchor_points", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(AnchorPoint),
                    { let Ok(explicit_value) = self.anchor_points(); explicit_value },
                    value_6,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for GlyphDataEntry<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct AnchorPoint {
    pub x: BigEndian<i16>,
    pub y: BigEndian<i16>,
}

const _: () = {
  assert!(core::mem::size_of::<AnchorPoint>() == 4);
  assert!(core::mem::align_of::<AnchorPoint>() == 1);
  assert!(core::mem::offset_of!(AnchorPoint, x) == 0);
  assert!(core::mem::offset_of!(AnchorPoint, y) == 2);
};

impl AnchorPoint {
    pub fn x(&self) -> Result<i16, Never> {
        self.x.get()
    }

    pub fn y(&self) -> Result<i16, Never> {
        self.y.get()
    }
}

impl FixedSize for AnchorPoint {
    const RAW_BYTE_LEN: u64 = i16::RAW_BYTE_LEN + i16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for AnchorPoint {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "AnchorPoint",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "x", value: FieldType::from(self.x()) }),
                1u64 => Some(Field { name: "y", value: FieldType::from(self.y()) }),
                _ => None,
            }),
            data,
        })
    }
}
