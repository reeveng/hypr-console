

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Vorg<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(vert_origin_y_metrics_byte_range) = self.vert_origin_y_metrics_byte_range();

        Ok(0..vert_origin_y_metrics_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Vorg<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"VORG"); value });
}

impl<'a> FontRead<'a> for Vorg<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Vorg<'a> {
    data: FontData<'a>,
}

impl<'a> Vorg<'a> {
    pub const MIN_SIZE: u64 = (MajorMinor::RAW_BYTE_LEN + i16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<MajorMinor, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn default_vert_origin_y(&self) -> Result<i16, Never> {
        let Ok(range) = self.default_vert_origin_y_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_vert_origin_y_metrics(&self) -> Result<u16, Never> {
        let Ok(range) = self.num_vert_origin_y_metrics_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn vert_origin_y_metrics(&self) -> Result<&'a [VertOriginYMetrics], Never> {
        let Ok(range) = self.vert_origin_y_metrics_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(MajorMinor::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn default_vert_origin_y_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(i16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_vert_origin_y_metrics_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.default_vert_origin_y_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn vert_origin_y_metrics_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_vert_origin_y_metrics) = self.num_vert_origin_y_metrics();
        let Ok(previous) = self.num_vert_origin_y_metrics_byte_range();
        let start = previous.end;
        let Ok(value_2) = transforms::to_u64(num_vert_origin_y_metrics);

        let end = start.saturating_add(value_2
                .saturating_mul(VertOriginYMetrics::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Vorg::MIN_SIZE); held }));

impl Default for Vorg<'_> {
    fn default() -> Self {
        let Ok(value_3) = FontData::default_table_data();

        Self {
            data: value_3,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Vorg<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Vorg")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "default_vert_origin_y", value: FieldType::from(self.default_vert_origin_y()) }),
            2u64 => Some(Field { name: "num_vert_origin_y_metrics", value: FieldType::from(self.num_vert_origin_y_metrics()) }),
            3u64 => {
                let Ok(value_4) = self.offset_data();

                Some(Field { name: "vert_origin_y_metrics", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(VertOriginYMetrics),
                    { let Ok(explicit_value) = self.vert_origin_y_metrics(); explicit_value },
                    value_4,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Vorg<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct VertOriginYMetrics {
    pub glyph_index: BigEndian<GlyphId16>,
    pub vert_origin_y: BigEndian<i16>,
}

const _: () = {
  assert!(core::mem::size_of::<VertOriginYMetrics>() == 4);
  assert!(core::mem::align_of::<VertOriginYMetrics>() == 1);
  assert!(core::mem::offset_of!(VertOriginYMetrics, glyph_index) == 0);
  assert!(core::mem::offset_of!(VertOriginYMetrics, vert_origin_y) == 2);
};

impl VertOriginYMetrics {
    pub fn glyph_index(&self) -> Result<GlyphId16, Never> {
        self.glyph_index.get()
    }

    pub fn vert_origin_y(&self) -> Result<i16, Never> {
        self.vert_origin_y.get()
    }
}

impl FixedSize for VertOriginYMetrics {
    const RAW_BYTE_LEN: u64 = GlyphId16::RAW_BYTE_LEN + i16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for VertOriginYMetrics {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "VertOriginYMetrics",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "glyph_index", value: FieldType::from(self.glyph_index()) }),
                1u64 => Some(Field { name: "vert_origin_y", value: FieldType::from(self.vert_origin_y()) }),
                _ => None,
            }),
            data,
        })
    }
}
