

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Svg<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        Ok(0..({ let Ok(explicit_value) = self._reserved_byte_range(); explicit_value }).end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Svg<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"SVG "); value });
}

impl<'a> FontRead<'a> for Svg<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Svg<'a> {
    data: FontData<'a>,
}

impl<'a> Svg<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + Offset32::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<u16, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn svg_document_list_offset(&self) -> Result<Offset32, Never> {
        let Ok(range) = self.svg_document_list_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn svg_document_list(&self) -> Result<SVGDocumentList<'a>, ReadError> {
        let data = self.data;
        let Ok(svg_document_list_offset) = self.svg_document_list_offset();

        svg_document_list_offset.resolve(data)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn svg_document_list_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn _reserved_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.svg_document_list_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Svg::MIN_SIZE); held }));

impl Default for Svg<'_> {
    fn default() -> Self {
        let Ok(value_2) = FontData::default_table_data();

        Self {
            data: value_2,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Svg<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Svg")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "svg_document_list_offset", value: FieldType::from(FieldType::offset(self.svg_document_list_offset(), (self.svg_document_list()).map(Some))) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Svg<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for SVGDocumentList<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(document_records_byte_range) = self.document_records_byte_range();

        Ok(0..document_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for SVGDocumentList<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_3) = data.len();

        match ::core::primitive::bool::then_some(value_3 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct SVGDocumentList<'a> {
    data: FontData<'a>,
}

impl<'a> SVGDocumentList<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn num_entries(&self) -> Result<u16, Never> {
        let Ok(range) = self.num_entries_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn document_records(&self) -> Result<&'a [SVGDocumentRecord], Never> {
        let Ok(range) = self.document_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn num_entries_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn document_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_entries) = self.num_entries();
        let Ok(previous) = self.num_entries_byte_range();
        let start = previous.end;
        let Ok(value_4) = transforms::to_u64(num_entries);

        let end = start.saturating_add(value_4.saturating_mul(SVGDocumentRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    SVGDocumentList::MIN_SIZE
); held }));

impl Default for SVGDocumentList<'_> {
    fn default() -> Self {
        let Ok(value_5) = FontData::default_table_data();

        Self {
            data: value_5,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for SVGDocumentList<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("SVGDocumentList")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "num_entries", value: FieldType::from(self.num_entries()) }),
            1u64 => {
                let Ok(value_6) = self.offset_data();

                Some(Field { name: "document_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(SVGDocumentRecord),
                    { let Ok(explicit_value) = self.document_records(); explicit_value },
                    value_6,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for SVGDocumentList<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct SVGDocumentRecord {
    pub start_glyph_id: BigEndian<GlyphId16>,
    pub end_glyph_id: BigEndian<GlyphId16>,
    pub svg_doc_offset: BigEndian<u32>,
    pub svg_doc_length: BigEndian<u32>,
}

const _: () = {
  assert!(core::mem::size_of::<SVGDocumentRecord>() == 12);
  assert!(core::mem::align_of::<SVGDocumentRecord>() == 1);
  assert!(core::mem::offset_of!(SVGDocumentRecord, start_glyph_id) == 0);
  assert!(core::mem::offset_of!(SVGDocumentRecord, end_glyph_id) == 2);
  assert!(core::mem::offset_of!(SVGDocumentRecord, svg_doc_offset) == 4);
  assert!(core::mem::offset_of!(SVGDocumentRecord, svg_doc_length) == 8);
};

impl SVGDocumentRecord {
    pub fn start_glyph_id(&self) -> Result<GlyphId16, Never> {
        self.start_glyph_id.get()
    }

    pub fn end_glyph_id(&self) -> Result<GlyphId16, Never> {
        self.end_glyph_id.get()
    }

    pub fn svg_doc_offset(&self) -> Result<u32, Never> {
        self.svg_doc_offset.get()
    }

    pub fn svg_doc_length(&self) -> Result<u32, Never> {
        self.svg_doc_length.get()
    }
}

impl FixedSize for SVGDocumentRecord {
    const RAW_BYTE_LEN: u64 =
        GlyphId16::RAW_BYTE_LEN + GlyphId16::RAW_BYTE_LEN + u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for SVGDocumentRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "SVGDocumentRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "start_glyph_id", value: FieldType::from(self.start_glyph_id()) }),
                1u64 => Some(Field { name: "end_glyph_id", value: FieldType::from(self.end_glyph_id()) }),
                2u64 => Some(Field { name: "svg_doc_offset", value: FieldType::from(self.svg_doc_offset()) }),
                3u64 => Some(Field { name: "svg_doc_length", value: FieldType::from(self.svg_doc_length()) }),
                _ => None,
            }),
            data,
        })
    }
}
