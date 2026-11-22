

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Colr<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(num_layer_records_byte_range) = self.num_layer_records_byte_range();

        Ok(0..num_layer_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Colr<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"COLR"); value });
}

impl<'a> FontRead<'a> for Colr<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Colr<'a> {
    data: FontData<'a>,
}

impl<'a> Colr<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN
        + Offset32::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<u16, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_base_glyph_records(&self) -> Result<u16, Never> {
        let Ok(range) = self.num_base_glyph_records_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn base_glyph_records_offset(&self) -> Result<Nullable<Offset32>, Never> {
        let Ok(range) = self.base_glyph_records_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn base_glyph_records(&self) -> Result<Option<&'a [BaseGlyph]>, ReadError> {
        let data = self.data;
        let Ok(args) = self.num_base_glyph_records();
        let Ok(base_glyph_records_offset) = self.base_glyph_records_offset();

        base_glyph_records_offset
            .resolve_with_args(data, &args)
    }

    pub fn layer_records_offset(&self) -> Result<Nullable<Offset32>, Never> {
        let Ok(range) = self.layer_records_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn layer_records(&self) -> Result<Option<&'a [Layer]>, ReadError> {
        let data = self.data;
        let Ok(args) = self.num_layer_records();
        let Ok(layer_records_offset) = self.layer_records_offset();

        layer_records_offset.resolve_with_args(data, &args)
    }

    pub fn num_layer_records(&self) -> Result<u16, Never> {
        let Ok(range) = self.num_layer_records_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn base_glyph_list_offset(&self) -> Result<Option<Nullable<Offset32>>, Never> {
        let Ok(range) = self.base_glyph_list_offset_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn base_glyph_list(&self) -> Result<Option<BaseGlyphList<'a>>, ReadError> {
        let data = self.data;
        let Ok(read_base_glyph_list_offset) = self.base_glyph_list_offset();

        match read_base_glyph_list_offset { Some(x) => x.resolve(data), None => Ok(None) }
    }

    pub fn layer_list_offset(&self) -> Result<Option<Nullable<Offset32>>, Never> {
        let Ok(range) = self.layer_list_offset_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn layer_list(&self) -> Result<Option<LayerList<'a>>, ReadError> {
        let data = self.data;
        let Ok(read_layer_list_offset) = self.layer_list_offset();

        match read_layer_list_offset { Some(x) => x.resolve(data), None => Ok(None) }
    }

    pub fn clip_list_offset(&self) -> Result<Option<Nullable<Offset32>>, Never> {
        let Ok(range) = self.clip_list_offset_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn clip_list(&self) -> Result<Option<ClipList<'a>>, ReadError> {
        let data = self.data;
        let Ok(read_clip_list_offset) = self.clip_list_offset();

        match read_clip_list_offset { Some(x) => x.resolve(data), None => Ok(None) }
    }

    pub fn var_index_map_offset(&self) -> Result<Option<Nullable<Offset32>>, Never> {
        let Ok(range) = self.var_index_map_offset_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn var_index_map(&self) -> Result<Option<DeltaSetIndexMap<'a>>, ReadError> {
        let data = self.data;
        let Ok(read_var_index_map_offset) = self.var_index_map_offset();

        match read_var_index_map_offset { Some(x) => x.resolve(data), None => Ok(None) }
    }

    pub fn item_variation_store_offset(&self) -> Result<Option<Nullable<Offset32>>, Never> {
        let Ok(range) = self.item_variation_store_offset_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn item_variation_store(&self) -> Result<Option<ItemVariationStore<'a>>, ReadError> {
        let data = self.data;
        let Ok(read_item_variation_store_offset) = self.item_variation_store_offset();

        match read_item_variation_store_offset { Some(x) => x.resolve(data), None => Ok(None) }
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_base_glyph_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn base_glyph_records_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.num_base_glyph_records_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn layer_records_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.base_glyph_records_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_layer_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.layer_records_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn base_glyph_list_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.num_layer_records_byte_range();
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

    pub fn layer_list_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.base_glyph_list_offset_byte_range();
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

    pub fn clip_list_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.layer_list_offset_byte_range();
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

    pub fn var_index_map_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.clip_list_offset_byte_range();
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

    pub fn item_variation_store_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.var_index_map_offset_byte_range();
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

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Colr::MIN_SIZE); held }));

impl Default for Colr<'_> {
    fn default() -> Self {
        let Ok(value_7) = FontData::default_table_data();

        Self {
            data: value_7,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Colr<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Colr")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "num_base_glyph_records", value: FieldType::from(self.num_base_glyph_records()) }),
            2u64 => {
                let Ok(value_8) = self.offset_data();

                Some(Field { name: "base_glyph_records_offset", value: FieldType::from(traversal::FieldType::offset_to_array_of_records(
                    self.base_glyph_records_offset(),
                    self.base_glyph_records(),
                    stringify!(BaseGlyph),
                    value_8,
                )) })
            },
            3u64 => {
                let Ok(value_9) = self.offset_data();

                Some(Field { name: "layer_records_offset", value: FieldType::from(traversal::FieldType::offset_to_array_of_records(
                    self.layer_records_offset(),
                    self.layer_records(),
                    stringify!(Layer),
                    value_9,
                )) })
            },
            4u64 => Some(Field { name: "num_layer_records", value: FieldType::from(self.num_layer_records()) }),
            5u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility(1u16);
                let Ok(base_glyph_list_offset) = self.base_glyph_list_offset();

                match compatibility {
                    Compatibility::Compatible => base_glyph_list_offset.map(|base_glyph_list_offset_2| Field { name: "base_glyph_list_offset", value: FieldType::from(FieldType::offset(
                        base_glyph_list_offset_2,
                        self.base_glyph_list(),
                    )) }),
                    Compatibility::Incompatible => None,
                }
            },
            6u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility(1u16);
                let Ok(layer_list_offset) = self.layer_list_offset();

                match compatibility {
                    Compatibility::Compatible => layer_list_offset.map(|layer_list_offset_2| Field { name: "layer_list_offset", value: FieldType::from(FieldType::offset(layer_list_offset_2, self.layer_list())) }),
                    Compatibility::Incompatible => None,
                }
            },
            7u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility(1u16);
                let Ok(clip_list_offset) = self.clip_list_offset();

                match compatibility {
                    Compatibility::Compatible => clip_list_offset.map(|clip_list_offset_2| Field { name: "clip_list_offset", value: FieldType::from(FieldType::offset(clip_list_offset_2, self.clip_list())) }),
                    Compatibility::Incompatible => None,
                }
            },
            8u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility(1u16);
                let Ok(var_index_map_offset) = self.var_index_map_offset();

                match compatibility {
                    Compatibility::Compatible => var_index_map_offset.map(|var_index_map_offset_2| Field { name: "var_index_map_offset", value: FieldType::from(FieldType::offset(var_index_map_offset_2, self.var_index_map())) }),
                    Compatibility::Incompatible => None,
                }
            },
            9u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility(1u16);
                let Ok(item_variation_store_offset) = self.item_variation_store_offset();

                match compatibility {
                    Compatibility::Compatible => item_variation_store_offset.map(|item_variation_store_offset_2| Field { name: "item_variation_store_offset", value: FieldType::from(FieldType::offset(
                        item_variation_store_offset_2,
                        self.item_variation_store(),
                    )) }),
                    Compatibility::Incompatible => None,
                }
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Colr<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct BaseGlyph {
    pub glyph_id: BigEndian<GlyphId16>,
    pub first_layer_index: BigEndian<u16>,
    pub num_layers: BigEndian<u16>,
}

const _: () = {
  assert!(core::mem::size_of::<BaseGlyph>() == 6);
  assert!(core::mem::align_of::<BaseGlyph>() == 1);
  assert!(core::mem::offset_of!(BaseGlyph, glyph_id) == 0);
  assert!(core::mem::offset_of!(BaseGlyph, first_layer_index) == 2);
  assert!(core::mem::offset_of!(BaseGlyph, num_layers) == 4);
};

impl BaseGlyph {
    pub fn glyph_id(&self) -> Result<GlyphId16, Never> {
        self.glyph_id.get()
    }

    pub fn first_layer_index(&self) -> Result<u16, Never> {
        self.first_layer_index.get()
    }

    pub fn num_layers(&self) -> Result<u16, Never> {
        self.num_layers.get()
    }
}

impl FixedSize for BaseGlyph {
    const RAW_BYTE_LEN: u64 = GlyphId16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for BaseGlyph {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "BaseGlyph",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "glyph_id", value: FieldType::from(self.glyph_id()) }),
                1u64 => Some(Field { name: "first_layer_index", value: FieldType::from(self.first_layer_index()) }),
                2u64 => Some(Field { name: "num_layers", value: FieldType::from(self.num_layers()) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct Layer {
    pub glyph_id: BigEndian<GlyphId16>,
    pub palette_index: BigEndian<u16>,
}

const _: () = {
  assert!(core::mem::size_of::<Layer>() == 4);
  assert!(core::mem::align_of::<Layer>() == 1);
  assert!(core::mem::offset_of!(Layer, glyph_id) == 0);
  assert!(core::mem::offset_of!(Layer, palette_index) == 2);
};

impl Layer {
    pub fn glyph_id(&self) -> Result<GlyphId16, Never> {
        self.glyph_id.get()
    }

    pub fn palette_index(&self) -> Result<u16, Never> {
        self.palette_index.get()
    }
}

impl FixedSize for Layer {
    const RAW_BYTE_LEN: u64 = GlyphId16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for Layer {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "Layer",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "glyph_id", value: FieldType::from(self.glyph_id()) }),
                1u64 => Some(Field { name: "palette_index", value: FieldType::from(self.palette_index()) }),
                _ => None,
            }),
            data,
        })
    }
}

impl<'a> MinByteRange<'a> for BaseGlyphList<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(base_glyph_paint_records_byte_range) = self.base_glyph_paint_records_byte_range();

        Ok(0..base_glyph_paint_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for BaseGlyphList<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_15) = data.len();

        match ::core::primitive::bool::then_some(value_15 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct BaseGlyphList<'a> {
    data: FontData<'a>,
}

impl<'a> BaseGlyphList<'a> {
    pub const MIN_SIZE: u64 = u32::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn num_base_glyph_paint_records(&self) -> Result<u32, Never> {
        let Ok(range) = self.num_base_glyph_paint_records_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn base_glyph_paint_records(&self) -> Result<&'a [BaseGlyphPaint], Never> {
        let Ok(range) = self.base_glyph_paint_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn num_base_glyph_paint_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn base_glyph_paint_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_base_glyph_paint_records) = self.num_base_glyph_paint_records();
        let Ok(previous) = self.num_base_glyph_paint_records_byte_range();
        let start = previous.end;
        let Ok(value_16) = transforms::to_u64(num_base_glyph_paint_records);

        let end = start.saturating_add(value_16
                .saturating_mul(BaseGlyphPaint::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(BaseGlyphList::MIN_SIZE); held }));

impl Default for BaseGlyphList<'_> {
    fn default() -> Self {
        let Ok(value_17) = FontData::default_table_data();

        Self {
            data: value_17,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for BaseGlyphList<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("BaseGlyphList")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "num_base_glyph_paint_records", value: FieldType::from(self.num_base_glyph_paint_records()) }),
            1u64 => {
                let Ok(value_18) = self.offset_data();

                Some(Field { name: "base_glyph_paint_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(BaseGlyphPaint),
                    { let Ok(explicit_value) = self.base_glyph_paint_records(); explicit_value },
                    value_18,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for BaseGlyphList<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct BaseGlyphPaint {
    pub glyph_id: BigEndian<GlyphId16>,
    pub paint_offset: BigEndian<Offset32>,
}

const _: () = {
  assert!(core::mem::size_of::<BaseGlyphPaint>() == 6);
  assert!(core::mem::align_of::<BaseGlyphPaint>() == 1);
  assert!(core::mem::offset_of!(BaseGlyphPaint, glyph_id) == 0);
  assert!(core::mem::offset_of!(BaseGlyphPaint, paint_offset) == 2);
};

impl BaseGlyphPaint {
    pub fn glyph_id(&self) -> Result<GlyphId16, Never> {
        self.glyph_id.get()
    }

    pub fn paint_offset(&self) -> Result<Offset32, Never> {
        self.paint_offset.get()
    }

    pub fn paint<'a>(&self, data: FontData<'a>) -> Result<Paint<'a>, ReadError> {
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }
}

impl FixedSize for BaseGlyphPaint {
    const RAW_BYTE_LEN: u64 = GlyphId16::RAW_BYTE_LEN + Offset32::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for BaseGlyphPaint {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "BaseGlyphPaint",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "glyph_id", value: FieldType::from(self.glyph_id()) }),
                1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint(_data)).map(Some))) }),
                _ => None,
            }),
            data,
        })
    }
}

impl<'a> MinByteRange<'a> for LayerList<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(paint_offsets_byte_range) = self.paint_offsets_byte_range();

        Ok(0..paint_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for LayerList<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_21) = data.len();

        match ::core::primitive::bool::then_some(value_21 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct LayerList<'a> {
    data: FontData<'a>,
}

impl<'a> LayerList<'a> {
    pub const MIN_SIZE: u64 = u32::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn num_layers(&self) -> Result<u32, Never> {
        let Ok(range) = self.num_layers_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offsets(&self) -> Result<&'a [BigEndian<Offset32>], Never> {
        let Ok(range) = self.paint_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn paints(&self) -> Result<ArrayOfOffsets<'a, Paint<'a>, Offset32>, Never> {
        let data = self.data;
        let Ok(offsets) = self.paint_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn num_layers_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_layers) = self.num_layers();
        let Ok(previous) = self.num_layers_byte_range();
        let start = previous.end;
        let Ok(value_22) = transforms::to_u64(num_layers);

        let end = start.saturating_add(value_22.saturating_mul(Offset32::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(LayerList::MIN_SIZE); held }));

impl Default for LayerList<'_> {
    fn default() -> Self {
        let Ok(value_23) = FontData::default_table_data();

        Self {
            data: value_23,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for LayerList<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("LayerList")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "num_layers", value: FieldType::from(self.num_layers()) }),
            1u64 => Some(Field { name: "paint_offsets", value: FieldType::from(self.paints()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for LayerList<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for ClipList<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(clips_byte_range) = self.clips_byte_range();

        Ok(0..clips_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ClipList<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_24) = data.len();

        match ::core::primitive::bool::then_some(value_24 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ClipList<'a> {
    data: FontData<'a>,
}

impl<'a> ClipList<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_clips(&self) -> Result<u32, Never> {
        let Ok(range) = self.num_clips_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn clips(&self) -> Result<&'a [Clip], Never> {
        let Ok(range) = self.clips_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_clips_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn clips_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_clips) = self.num_clips();
        let Ok(previous) = self.num_clips_byte_range();
        let start = previous.end;
        let Ok(value_25) = transforms::to_u64(num_clips);

        let end = start.saturating_add(value_25.saturating_mul(Clip::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(ClipList::MIN_SIZE); held }));

impl Default for ClipList<'_> {
    fn default() -> Self {
        let Ok(value_26) = FontData::default_table_data();

        Self {
            data: value_26,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ClipList<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ClipList")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "num_clips", value: FieldType::from(self.num_clips()) }),
            2u64 => {
                let Ok(value_27) = self.offset_data();

                Some(Field { name: "clips", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(Clip),
                    { let Ok(explicit_value) = self.clips(); explicit_value },
                    value_27,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ClipList<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct Clip {
    pub start_glyph_id: BigEndian<GlyphId16>,
    pub end_glyph_id: BigEndian<GlyphId16>,
    pub clip_box_offset: BigEndian<Offset24>,
}

const _: () = {
  assert!(core::mem::size_of::<Clip>() == 7);
  assert!(core::mem::align_of::<Clip>() == 1);
  assert!(core::mem::offset_of!(Clip, start_glyph_id) == 0);
  assert!(core::mem::offset_of!(Clip, end_glyph_id) == 2);
  assert!(core::mem::offset_of!(Clip, clip_box_offset) == 4);
};

impl Clip {
    pub fn start_glyph_id(&self) -> Result<GlyphId16, Never> {
        self.start_glyph_id.get()
    }

    pub fn end_glyph_id(&self) -> Result<GlyphId16, Never> {
        self.end_glyph_id.get()
    }

    pub fn clip_box_offset(&self) -> Result<Offset24, Never> {
        self.clip_box_offset.get()
    }

    pub fn clip_box<'a>(&self, data: FontData<'a>) -> Result<ClipBox<'a>, ReadError> {
        let Ok(clip_box_offset) = self.clip_box_offset();

        clip_box_offset.resolve(data)
    }
}

impl FixedSize for Clip {
    const RAW_BYTE_LEN: u64 =
        GlyphId16::RAW_BYTE_LEN + GlyphId16::RAW_BYTE_LEN + Offset24::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for Clip {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "Clip",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "start_glyph_id", value: FieldType::from(self.start_glyph_id()) }),
                1u64 => Some(Field { name: "end_glyph_id", value: FieldType::from(self.end_glyph_id()) }),
                2u64 => Some(Field { name: "clip_box_offset", value: FieldType::from(FieldType::offset(self.clip_box_offset(), (self.clip_box(_data)).map(Some))) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone)]
pub enum ClipBox<'a> {
    Format1(ClipBoxFormat1<'a>),
    Format2(ClipBoxFormat2<'a>),
}

impl Default for ClipBox<'_> {
    fn default() -> Self {
        Self::Format1(Default::default())
    }
}

impl<'a> ClipBox<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Format1(item) => item.offset_data(),
            Self::Format2(item) => item.offset_data(),
        }
    }

    pub fn format(&self) -> Result<u8, Never> {
        match self {
            Self::Format1(item) => item.format(),
            Self::Format2(item) => item.format(),
        }
    }

    pub fn x_min(&self) -> Result<FWord, Never> {
        match self {
            Self::Format1(item) => item.x_min(),
            Self::Format2(item) => item.x_min(),
        }
    }

    pub fn y_min(&self) -> Result<FWord, Never> {
        match self {
            Self::Format1(item) => item.y_min(),
            Self::Format2(item) => item.y_min(),
        }
    }

    pub fn x_max(&self) -> Result<FWord, Never> {
        match self {
            Self::Format1(item) => item.x_max(),
            Self::Format2(item) => item.x_max(),
        }
    }

    pub fn y_max(&self) -> Result<FWord, Never> {
        match self {
            Self::Format1(item) => item.y_max(),
            Self::Format2(item) => item.y_max(),
        }
    }
}

impl<'a> FontRead<'a> for ClipBox<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u8 = data.read_at(0u64)?;

        match format {
            ClipBoxFormat1::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format1(read))
            }
            ClipBoxFormat2::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format2(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u8>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for ClipBox<'a> {
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
impl<'a> ClipBox<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Format1(table) => table,
            Self::Format2(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for ClipBox<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ClipBox<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}

impl Format<u8> for ClipBoxFormat1<'_> {
    const FORMAT: u8 = 1;
}

impl<'a> MinByteRange<'a> for ClipBoxFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(y_max_byte_range) = self.y_max_byte_range();

        Ok(0..y_max_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ClipBoxFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_33) = data.len();

        match ::core::primitive::bool::then_some(value_33 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ClipBoxFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> ClipBoxFormat1<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn x_min(&self) -> Result<FWord, Never> {
        let Ok(range) = self.x_min_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_min(&self) -> Result<FWord, Never> {
        let Ok(range) = self.y_min_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn x_max(&self) -> Result<FWord, Never> {
        let Ok(range) = self.x_max_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_max(&self) -> Result<FWord, Never> {
        let Ok(range) = self.y_max_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x_min_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_min_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x_min_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x_max_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y_min_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_max_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x_max_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(ClipBoxFormat1::MIN_SIZE); held }));

impl Default for ClipBoxFormat1<'_> {
    fn default() -> Self {
        let Ok(value_34) = FontData::default_format_1_u8_table_data();

        Self {
            data: value_34,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ClipBoxFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ClipBoxFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "x_min", value: FieldType::from(self.x_min()) }),
            2u64 => Some(Field { name: "y_min", value: FieldType::from(self.y_min()) }),
            3u64 => Some(Field { name: "x_max", value: FieldType::from(self.x_max()) }),
            4u64 => Some(Field { name: "y_max", value: FieldType::from(self.y_max()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ClipBoxFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for ClipBoxFormat2<'_> {
    const FORMAT: u8 = 2;
}

impl<'a> MinByteRange<'a> for ClipBoxFormat2<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(var_index_base_byte_range) = self.var_index_base_byte_range();

        Ok(0..var_index_base_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ClipBoxFormat2<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_35) = data.len();

        match ::core::primitive::bool::then_some(value_35 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ClipBoxFormat2<'a> {
    data: FontData<'a>,
}

impl<'a> ClipBoxFormat2<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn x_min(&self) -> Result<FWord, Never> {
        let Ok(range) = self.x_min_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_min(&self) -> Result<FWord, Never> {
        let Ok(range) = self.y_min_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn x_max(&self) -> Result<FWord, Never> {
        let Ok(range) = self.x_max_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_max(&self) -> Result<FWord, Never> {
        let Ok(range) = self.y_max_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn var_index_base(&self) -> Result<u32, Never> {
        let Ok(range) = self.var_index_base_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x_min_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_min_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x_min_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x_max_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y_min_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_max_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x_max_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn var_index_base_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y_max_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ClipBoxFormat2<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ClipBoxFormat2")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "x_min", value: FieldType::from(self.x_min()) }),
            2u64 => Some(Field { name: "y_min", value: FieldType::from(self.y_min()) }),
            3u64 => Some(Field { name: "x_max", value: FieldType::from(self.x_max()) }),
            4u64 => Some(Field { name: "y_max", value: FieldType::from(self.y_max()) }),
            5u64 => Some(Field { name: "var_index_base", value: FieldType::from(self.var_index_base()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ClipBoxFormat2<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct ColorIndex {
    pub palette_index: BigEndian<u16>,
    pub alpha: BigEndian<F2Dot14>,
}

const _: () = {
  assert!(core::mem::size_of::<ColorIndex>() == 4);
  assert!(core::mem::align_of::<ColorIndex>() == 1);
  assert!(core::mem::offset_of!(ColorIndex, palette_index) == 0);
  assert!(core::mem::offset_of!(ColorIndex, alpha) == 2);
};

impl ColorIndex {
    pub fn palette_index(&self) -> Result<u16, Never> {
        self.palette_index.get()
    }

    pub fn alpha(&self) -> Result<F2Dot14, Never> {
        self.alpha.get()
    }
}

impl FixedSize for ColorIndex {
    const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for ColorIndex {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "ColorIndex",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "palette_index", value: FieldType::from(self.palette_index()) }),
                1u64 => Some(Field { name: "alpha", value: FieldType::from(self.alpha()) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct VarColorIndex {
    pub palette_index: BigEndian<u16>,
    pub alpha: BigEndian<F2Dot14>,
    pub var_index_base: BigEndian<u32>,
}

const _: () = {
  assert!(core::mem::size_of::<VarColorIndex>() == 8);
  assert!(core::mem::align_of::<VarColorIndex>() == 1);
  assert!(core::mem::offset_of!(VarColorIndex, palette_index) == 0);
  assert!(core::mem::offset_of!(VarColorIndex, alpha) == 2);
  assert!(core::mem::offset_of!(VarColorIndex, var_index_base) == 4);
};

impl VarColorIndex {
    pub fn palette_index(&self) -> Result<u16, Never> {
        self.palette_index.get()
    }

    pub fn alpha(&self) -> Result<F2Dot14, Never> {
        self.alpha.get()
    }

    pub fn var_index_base(&self) -> Result<u32, Never> {
        self.var_index_base.get()
    }
}

impl FixedSize for VarColorIndex {
    const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN + u32::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for VarColorIndex {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "VarColorIndex",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "palette_index", value: FieldType::from(self.palette_index()) }),
                1u64 => Some(Field { name: "alpha", value: FieldType::from(self.alpha()) }),
                2u64 => Some(Field { name: "var_index_base", value: FieldType::from(self.var_index_base()) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct ColorStop {
    pub stop_offset: BigEndian<F2Dot14>,
    pub palette_index: BigEndian<u16>,
    pub alpha: BigEndian<F2Dot14>,
}

const _: () = {
  assert!(core::mem::size_of::<ColorStop>() == 6);
  assert!(core::mem::align_of::<ColorStop>() == 1);
  assert!(core::mem::offset_of!(ColorStop, stop_offset) == 0);
  assert!(core::mem::offset_of!(ColorStop, palette_index) == 2);
  assert!(core::mem::offset_of!(ColorStop, alpha) == 4);
};

impl ColorStop {
    pub fn stop_offset(&self) -> Result<F2Dot14, Never> {
        self.stop_offset.get()
    }

    pub fn palette_index(&self) -> Result<u16, Never> {
        self.palette_index.get()
    }

    pub fn alpha(&self) -> Result<F2Dot14, Never> {
        self.alpha.get()
    }
}

impl FixedSize for ColorStop {
    const RAW_BYTE_LEN: u64 = F2Dot14::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for ColorStop {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "ColorStop",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "stop_offset", value: FieldType::from(self.stop_offset()) }),
                1u64 => Some(Field { name: "palette_index", value: FieldType::from(self.palette_index()) }),
                2u64 => Some(Field { name: "alpha", value: FieldType::from(self.alpha()) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct VarColorStop {
    pub stop_offset: BigEndian<F2Dot14>,
    pub palette_index: BigEndian<u16>,
    pub alpha: BigEndian<F2Dot14>,
    pub var_index_base: BigEndian<u32>,
}

const _: () = {
  assert!(core::mem::size_of::<VarColorStop>() == 10);
  assert!(core::mem::align_of::<VarColorStop>() == 1);
  assert!(core::mem::offset_of!(VarColorStop, stop_offset) == 0);
  assert!(core::mem::offset_of!(VarColorStop, palette_index) == 2);
  assert!(core::mem::offset_of!(VarColorStop, alpha) == 4);
  assert!(core::mem::offset_of!(VarColorStop, var_index_base) == 6);
};

impl VarColorStop {
    pub fn stop_offset(&self) -> Result<F2Dot14, Never> {
        self.stop_offset.get()
    }

    pub fn palette_index(&self) -> Result<u16, Never> {
        self.palette_index.get()
    }

    pub fn alpha(&self) -> Result<F2Dot14, Never> {
        self.alpha.get()
    }

    pub fn var_index_base(&self) -> Result<u32, Never> {
        self.var_index_base.get()
    }
}

impl FixedSize for VarColorStop {
    const RAW_BYTE_LEN: u64 =
        F2Dot14::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN + u32::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for VarColorStop {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "VarColorStop",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "stop_offset", value: FieldType::from(self.stop_offset()) }),
                1u64 => Some(Field { name: "palette_index", value: FieldType::from(self.palette_index()) }),
                2u64 => Some(Field { name: "alpha", value: FieldType::from(self.alpha()) }),
                3u64 => Some(Field { name: "var_index_base", value: FieldType::from(self.var_index_base()) }),
                _ => None,
            }),
            data,
        })
    }
}

impl<'a> MinByteRange<'a> for ColorLine<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(color_stops_byte_range) = self.color_stops_byte_range();

        Ok(0..color_stops_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for ColorLine<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_48) = data.len();

        match ::core::primitive::bool::then_some(value_48 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct ColorLine<'a> {
    data: FontData<'a>,
}

impl<'a> ColorLine<'a> {
    pub const MIN_SIZE: u64 = (Extend::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn extend(&self) -> Result<Extend, Never> {
        let Ok(range) = self.extend_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_stops(&self) -> Result<u16, Never> {
        let Ok(range) = self.num_stops_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn color_stops(&self) -> Result<&'a [ColorStop], Never> {
        let Ok(range) = self.color_stops_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn extend_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(Extend::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_stops_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.extend_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn color_stops_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_stops) = self.num_stops();
        let Ok(previous) = self.num_stops_byte_range();
        let start = previous.end;
        let Ok(value_49) = transforms::to_u64(num_stops);

        let end = start.saturating_add(value_49.saturating_mul(ColorStop::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(ColorLine::MIN_SIZE); held }));

impl Default for ColorLine<'_> {
    fn default() -> Self {
        let Ok(value_50) = FontData::default_table_data();

        Self {
            data: value_50,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for ColorLine<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("ColorLine")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "extend", value: FieldType::from(self.extend()) }),
            1u64 => Some(Field { name: "num_stops", value: FieldType::from(self.num_stops()) }),
            2u64 => {
                let Ok(value_51) = self.offset_data();

                Some(Field { name: "color_stops", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(ColorStop),
                    { let Ok(explicit_value) = self.color_stops(); explicit_value },
                    value_51,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for ColorLine<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for VarColorLine<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(color_stops_byte_range) = self.color_stops_byte_range();

        Ok(0..color_stops_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for VarColorLine<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_52) = data.len();

        match ::core::primitive::bool::then_some(value_52 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct VarColorLine<'a> {
    data: FontData<'a>,
}

impl<'a> VarColorLine<'a> {
    pub const MIN_SIZE: u64 = (Extend::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn extend(&self) -> Result<Extend, Never> {
        let Ok(range) = self.extend_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_stops(&self) -> Result<u16, Never> {
        let Ok(range) = self.num_stops_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn color_stops(&self) -> Result<&'a [VarColorStop], Never> {
        let Ok(range) = self.color_stops_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn extend_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(Extend::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_stops_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.extend_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn color_stops_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_stops) = self.num_stops();
        let Ok(previous) = self.num_stops_byte_range();
        let start = previous.end;
        let Ok(value_53) = transforms::to_u64(num_stops);

        let end =
            start.saturating_add(value_53.saturating_mul(VarColorStop::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(VarColorLine::MIN_SIZE); held }));

impl Default for VarColorLine<'_> {
    fn default() -> Self {
        let Ok(value_54) = FontData::default_table_data();

        Self {
            data: value_54,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for VarColorLine<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("VarColorLine")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "extend", value: FieldType::from(self.extend()) }),
            1u64 => Some(Field { name: "num_stops", value: FieldType::from(self.num_stops()) }),
            2u64 => {
                let Ok(value_55) = self.offset_data();

                Some(Field { name: "color_stops", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(VarColorStop),
                    { let Ok(explicit_value) = self.color_stops(); explicit_value },
                    value_55,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for VarColorLine<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
#[cfg_attr(
    dylint_lib = "explicit070_spaced_discriminants",
    expect(explicit070_spaced_discriminants, reason = "OpenType COLR gradient extend modes use these specified byte values")
)]
pub enum Extend {
    #[default]
    Pad = 0,
    Repeat = 1,
    Reflect = 2,
    #[doc(hidden)]
    Unknown,
}

impl Extend {
    fn discriminant_value(self) -> Result<u8, Never> {
        Ok(match self {
            Self::Pad => 0,
            Self::Repeat => 1,
            Self::Reflect => 2,
            Self::Unknown => 3,
        })
    }

    pub fn new(raw: u8) -> Result<Self, Never> {
        Ok(match raw {
            0 => Self::Pad,
            1 => Self::Repeat,
            2 => Self::Reflect,
            _ => Self::Unknown,
        })
    }
}

impl font_types::Scalar for Extend {
    type Raw = <u8 as font_types::Scalar>::Raw;

    fn to_raw(self) -> std::result::Result<Self::Raw, console_core_never::Never> {
        let Ok(value) = self.discriminant_value();
        value.to_raw()
    }

    fn from_raw(raw: Self::Raw) -> std::result::Result<Self, console_core_never::Never> {
        let Ok(value_57) = <u8>::from_raw(raw);

        let t = value_57;
        let value_58 = t;

        let Ok(extend) = Self::new(value_58);

        Ok(extend)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> From<Extend> for FieldType<'a> {
    fn from(src: Extend) -> FieldType<'a> {
        let Ok(value) = src.discriminant_value();
        FieldType::from(value)
    }
}

#[derive(Clone)]
pub enum Paint<'a> {
    ColrLayers(PaintColrLayers<'a>),
    Solid(PaintSolid<'a>),
    VarSolid(PaintVarSolid<'a>),
    LinearGradient(PaintLinearGradient<'a>),
    VarLinearGradient(PaintVarLinearGradient<'a>),
    RadialGradient(PaintRadialGradient<'a>),
    VarRadialGradient(PaintVarRadialGradient<'a>),
    SweepGradient(PaintSweepGradient<'a>),
    VarSweepGradient(PaintVarSweepGradient<'a>),
    Glyph(PaintGlyph<'a>),
    ColrGlyph(PaintColrGlyph<'a>),
    Transform(PaintTransform<'a>),
    VarTransform(PaintVarTransform<'a>),
    Translate(PaintTranslate<'a>),
    VarTranslate(PaintVarTranslate<'a>),
    Scale(PaintScale<'a>),
    VarScale(PaintVarScale<'a>),
    ScaleAroundCenter(PaintScaleAroundCenter<'a>),
    VarScaleAroundCenter(PaintVarScaleAroundCenter<'a>),
    ScaleUniform(PaintScaleUniform<'a>),
    VarScaleUniform(PaintVarScaleUniform<'a>),
    ScaleUniformAroundCenter(PaintScaleUniformAroundCenter<'a>),
    VarScaleUniformAroundCenter(PaintVarScaleUniformAroundCenter<'a>),
    Rotate(PaintRotate<'a>),
    VarRotate(PaintVarRotate<'a>),
    RotateAroundCenter(PaintRotateAroundCenter<'a>),
    VarRotateAroundCenter(PaintVarRotateAroundCenter<'a>),
    Skew(PaintSkew<'a>),
    VarSkew(PaintVarSkew<'a>),
    SkewAroundCenter(PaintSkewAroundCenter<'a>),
    VarSkewAroundCenter(PaintVarSkewAroundCenter<'a>),
    Composite(PaintComposite<'a>),
}

impl Default for Paint<'_> {
    fn default() -> Self {
        Self::ColrLayers(Default::default())
    }
}

impl<'a> Paint<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::ColrLayers(item) => item.offset_data(),
            Self::Solid(item) => item.offset_data(),
            Self::VarSolid(item) => item.offset_data(),
            Self::LinearGradient(item) => item.offset_data(),
            Self::VarLinearGradient(item) => item.offset_data(),
            Self::RadialGradient(item) => item.offset_data(),
            Self::VarRadialGradient(item) => item.offset_data(),
            Self::SweepGradient(item) => item.offset_data(),
            Self::VarSweepGradient(item) => item.offset_data(),
            Self::Glyph(item) => item.offset_data(),
            Self::ColrGlyph(item) => item.offset_data(),
            Self::Transform(item) => item.offset_data(),
            Self::VarTransform(item) => item.offset_data(),
            Self::Translate(item) => item.offset_data(),
            Self::VarTranslate(item) => item.offset_data(),
            Self::Scale(item) => item.offset_data(),
            Self::VarScale(item) => item.offset_data(),
            Self::ScaleAroundCenter(item) => item.offset_data(),
            Self::VarScaleAroundCenter(item) => item.offset_data(),
            Self::ScaleUniform(item) => item.offset_data(),
            Self::VarScaleUniform(item) => item.offset_data(),
            Self::ScaleUniformAroundCenter(item) => item.offset_data(),
            Self::VarScaleUniformAroundCenter(item) => item.offset_data(),
            Self::Rotate(item) => item.offset_data(),
            Self::VarRotate(item) => item.offset_data(),
            Self::RotateAroundCenter(item) => item.offset_data(),
            Self::VarRotateAroundCenter(item) => item.offset_data(),
            Self::Skew(item) => item.offset_data(),
            Self::VarSkew(item) => item.offset_data(),
            Self::SkewAroundCenter(item) => item.offset_data(),
            Self::VarSkewAroundCenter(item) => item.offset_data(),
            Self::Composite(item) => item.offset_data(),
        }
    }

    pub fn format(&self) -> Result<u8, Never> {
        match self {
            Self::ColrLayers(item) => item.format(),
            Self::Solid(item) => item.format(),
            Self::VarSolid(item) => item.format(),
            Self::LinearGradient(item) => item.format(),
            Self::VarLinearGradient(item) => item.format(),
            Self::RadialGradient(item) => item.format(),
            Self::VarRadialGradient(item) => item.format(),
            Self::SweepGradient(item) => item.format(),
            Self::VarSweepGradient(item) => item.format(),
            Self::Glyph(item) => item.format(),
            Self::ColrGlyph(item) => item.format(),
            Self::Transform(item) => item.format(),
            Self::VarTransform(item) => item.format(),
            Self::Translate(item) => item.format(),
            Self::VarTranslate(item) => item.format(),
            Self::Scale(item) => item.format(),
            Self::VarScale(item) => item.format(),
            Self::ScaleAroundCenter(item) => item.format(),
            Self::VarScaleAroundCenter(item) => item.format(),
            Self::ScaleUniform(item) => item.format(),
            Self::VarScaleUniform(item) => item.format(),
            Self::ScaleUniformAroundCenter(item) => item.format(),
            Self::VarScaleUniformAroundCenter(item) => item.format(),
            Self::Rotate(item) => item.format(),
            Self::VarRotate(item) => item.format(),
            Self::RotateAroundCenter(item) => item.format(),
            Self::VarRotateAroundCenter(item) => item.format(),
            Self::Skew(item) => item.format(),
            Self::VarSkew(item) => item.format(),
            Self::SkewAroundCenter(item) => item.format(),
            Self::VarSkewAroundCenter(item) => item.format(),
            Self::Composite(item) => item.format(),
        }
    }
}

impl<'a> FontRead<'a> for Paint<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u8 = data.read_at(0u64)?;

        match format {
            PaintColrLayers::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::ColrLayers(read))
            }
            PaintSolid::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Solid(read))
            }
            PaintVarSolid::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::VarSolid(read))
            }
            PaintLinearGradient::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::LinearGradient(read))
            }
            PaintVarLinearGradient::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::VarLinearGradient(read))
            }
            PaintRadialGradient::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::RadialGradient(read))
            }
            PaintVarRadialGradient::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::VarRadialGradient(read))
            }
            PaintSweepGradient::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::SweepGradient(read))
            }
            PaintVarSweepGradient::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::VarSweepGradient(read))
            }
            PaintGlyph::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Glyph(read))
            }
            PaintColrGlyph::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::ColrGlyph(read))
            }
            PaintTransform::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Transform(read))
            }
            PaintVarTransform::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::VarTransform(read))
            }
            PaintTranslate::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Translate(read))
            }
            PaintVarTranslate::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::VarTranslate(read))
            }
            PaintScale::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Scale(read))
            }
            PaintVarScale::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::VarScale(read))
            }
            PaintScaleAroundCenter::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::ScaleAroundCenter(read))
            }
            PaintVarScaleAroundCenter::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::VarScaleAroundCenter(read))
            }
            PaintScaleUniform::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::ScaleUniform(read))
            }
            PaintVarScaleUniform::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::VarScaleUniform(read))
            }
            PaintScaleUniformAroundCenter::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::ScaleUniformAroundCenter(read))
            }
            PaintVarScaleUniformAroundCenter::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::VarScaleUniformAroundCenter(read))
            }
            PaintRotate::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Rotate(read))
            }
            PaintVarRotate::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::VarRotate(read))
            }
            PaintRotateAroundCenter::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::RotateAroundCenter(read))
            }
            PaintVarRotateAroundCenter::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::VarRotateAroundCenter(read))
            }
            PaintSkew::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Skew(read))
            }
            PaintVarSkew::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::VarSkew(read))
            }
            PaintSkewAroundCenter::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::SkewAroundCenter(read))
            }
            PaintVarSkewAroundCenter::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::VarSkewAroundCenter(read))
            }
            PaintComposite::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Composite(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u8>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for Paint<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        match self {
            Self::ColrLayers(item) => item.min_byte_range(),
            Self::Solid(item) => item.min_byte_range(),
            Self::VarSolid(item) => item.min_byte_range(),
            Self::LinearGradient(item) => item.min_byte_range(),
            Self::VarLinearGradient(item) => item.min_byte_range(),
            Self::RadialGradient(item) => item.min_byte_range(),
            Self::VarRadialGradient(item) => item.min_byte_range(),
            Self::SweepGradient(item) => item.min_byte_range(),
            Self::VarSweepGradient(item) => item.min_byte_range(),
            Self::Glyph(item) => item.min_byte_range(),
            Self::ColrGlyph(item) => item.min_byte_range(),
            Self::Transform(item) => item.min_byte_range(),
            Self::VarTransform(item) => item.min_byte_range(),
            Self::Translate(item) => item.min_byte_range(),
            Self::VarTranslate(item) => item.min_byte_range(),
            Self::Scale(item) => item.min_byte_range(),
            Self::VarScale(item) => item.min_byte_range(),
            Self::ScaleAroundCenter(item) => item.min_byte_range(),
            Self::VarScaleAroundCenter(item) => item.min_byte_range(),
            Self::ScaleUniform(item) => item.min_byte_range(),
            Self::VarScaleUniform(item) => item.min_byte_range(),
            Self::ScaleUniformAroundCenter(item) => item.min_byte_range(),
            Self::VarScaleUniformAroundCenter(item) => item.min_byte_range(),
            Self::Rotate(item) => item.min_byte_range(),
            Self::VarRotate(item) => item.min_byte_range(),
            Self::RotateAroundCenter(item) => item.min_byte_range(),
            Self::VarRotateAroundCenter(item) => item.min_byte_range(),
            Self::Skew(item) => item.min_byte_range(),
            Self::VarSkew(item) => item.min_byte_range(),
            Self::SkewAroundCenter(item) => item.min_byte_range(),
            Self::VarSkewAroundCenter(item) => item.min_byte_range(),
            Self::Composite(item) => item.min_byte_range(),
        }
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        match self {
            Self::ColrLayers(item) => item.min_table_bytes(),
            Self::Solid(item) => item.min_table_bytes(),
            Self::VarSolid(item) => item.min_table_bytes(),
            Self::LinearGradient(item) => item.min_table_bytes(),
            Self::VarLinearGradient(item) => item.min_table_bytes(),
            Self::RadialGradient(item) => item.min_table_bytes(),
            Self::VarRadialGradient(item) => item.min_table_bytes(),
            Self::SweepGradient(item) => item.min_table_bytes(),
            Self::VarSweepGradient(item) => item.min_table_bytes(),
            Self::Glyph(item) => item.min_table_bytes(),
            Self::ColrGlyph(item) => item.min_table_bytes(),
            Self::Transform(item) => item.min_table_bytes(),
            Self::VarTransform(item) => item.min_table_bytes(),
            Self::Translate(item) => item.min_table_bytes(),
            Self::VarTranslate(item) => item.min_table_bytes(),
            Self::Scale(item) => item.min_table_bytes(),
            Self::VarScale(item) => item.min_table_bytes(),
            Self::ScaleAroundCenter(item) => item.min_table_bytes(),
            Self::VarScaleAroundCenter(item) => item.min_table_bytes(),
            Self::ScaleUniform(item) => item.min_table_bytes(),
            Self::VarScaleUniform(item) => item.min_table_bytes(),
            Self::ScaleUniformAroundCenter(item) => item.min_table_bytes(),
            Self::VarScaleUniformAroundCenter(item) => item.min_table_bytes(),
            Self::Rotate(item) => item.min_table_bytes(),
            Self::VarRotate(item) => item.min_table_bytes(),
            Self::RotateAroundCenter(item) => item.min_table_bytes(),
            Self::VarRotateAroundCenter(item) => item.min_table_bytes(),
            Self::Skew(item) => item.min_table_bytes(),
            Self::VarSkew(item) => item.min_table_bytes(),
            Self::SkewAroundCenter(item) => item.min_table_bytes(),
            Self::VarSkewAroundCenter(item) => item.min_table_bytes(),
            Self::Composite(item) => item.min_table_bytes(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> Paint<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::ColrLayers(table) => table,
            Self::Solid(table) => table,
            Self::VarSolid(table) => table,
            Self::LinearGradient(table) => table,
            Self::VarLinearGradient(table) => table,
            Self::RadialGradient(table) => table,
            Self::VarRadialGradient(table) => table,
            Self::SweepGradient(table) => table,
            Self::VarSweepGradient(table) => table,
            Self::Glyph(table) => table,
            Self::ColrGlyph(table) => table,
            Self::Transform(table) => table,
            Self::VarTransform(table) => table,
            Self::Translate(table) => table,
            Self::VarTranslate(table) => table,
            Self::Scale(table) => table,
            Self::VarScale(table) => table,
            Self::ScaleAroundCenter(table) => table,
            Self::VarScaleAroundCenter(table) => table,
            Self::ScaleUniform(table) => table,
            Self::VarScaleUniform(table) => table,
            Self::ScaleUniformAroundCenter(table) => table,
            Self::VarScaleUniformAroundCenter(table) => table,
            Self::Rotate(table) => table,
            Self::VarRotate(table) => table,
            Self::RotateAroundCenter(table) => table,
            Self::VarRotateAroundCenter(table) => table,
            Self::Skew(table) => table,
            Self::VarSkew(table) => table,
            Self::SkewAroundCenter(table) => table,
            Self::VarSkewAroundCenter(table) => table,
            Self::Composite(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for Paint<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Paint<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}

impl Format<u8> for PaintColrLayers<'_> {
    const FORMAT: u8 = 1;
}

impl<'a> MinByteRange<'a> for PaintColrLayers<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(first_layer_index_byte_range) = self.first_layer_index_byte_range();

        Ok(0..first_layer_index_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintColrLayers<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_91) = data.len();

        match ::core::primitive::bool::then_some(value_91 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintColrLayers<'a> {
    data: FontData<'a>,
}

impl<'a> PaintColrLayers<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN + u8::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_layers(&self) -> Result<u8, Never> {
        let Ok(range) = self.num_layers_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn first_layer_index(&self) -> Result<u32, Never> {
        let Ok(range) = self.first_layer_index_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_layers_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn first_layer_index_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.num_layers_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    PaintColrLayers::MIN_SIZE
); held }));

impl Default for PaintColrLayers<'_> {
    fn default() -> Self {
        let Ok(value_92) = FontData::default_format_1_u8_table_data();

        Self {
            data: value_92,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintColrLayers<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintColrLayers")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "num_layers", value: FieldType::from(self.num_layers()) }),
            2u64 => Some(Field { name: "first_layer_index", value: FieldType::from(self.first_layer_index()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintColrLayers<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintSolid<'_> {
    const FORMAT: u8 = 2;
}

impl<'a> MinByteRange<'a> for PaintSolid<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(alpha_byte_range) = self.alpha_byte_range();

        Ok(0..alpha_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintSolid<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_93) = data.len();

        match ::core::primitive::bool::then_some(value_93 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintSolid<'a> {
    data: FontData<'a>,
}

impl<'a> PaintSolid<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn palette_index(&self) -> Result<u16, Never> {
        let Ok(range) = self.palette_index_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn alpha(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.alpha_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn palette_index_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn alpha_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.palette_index_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintSolid<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintSolid")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "palette_index", value: FieldType::from(self.palette_index()) }),
            2u64 => Some(Field { name: "alpha", value: FieldType::from(self.alpha()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintSolid<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintVarSolid<'_> {
    const FORMAT: u8 = 3;
}

impl<'a> MinByteRange<'a> for PaintVarSolid<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(var_index_base_byte_range) = self.var_index_base_byte_range();

        Ok(0..var_index_base_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintVarSolid<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_94) = data.len();

        match ::core::primitive::bool::then_some(value_94 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintVarSolid<'a> {
    data: FontData<'a>,
}

impl<'a> PaintVarSolid<'a> {
    pub const MIN_SIZE: u64 =
        (u8::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn palette_index(&self) -> Result<u16, Never> {
        let Ok(range) = self.palette_index_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn alpha(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.alpha_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn var_index_base(&self) -> Result<u32, Never> {
        let Ok(range) = self.var_index_base_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn palette_index_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn alpha_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.palette_index_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn var_index_base_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.alpha_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintVarSolid<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintVarSolid")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "palette_index", value: FieldType::from(self.palette_index()) }),
            2u64 => Some(Field { name: "alpha", value: FieldType::from(self.alpha()) }),
            3u64 => Some(Field { name: "var_index_base", value: FieldType::from(self.var_index_base()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintVarSolid<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintLinearGradient<'_> {
    const FORMAT: u8 = 4;
}

impl<'a> MinByteRange<'a> for PaintLinearGradient<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(y2_byte_range) = self.y2_byte_range();

        Ok(0..y2_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintLinearGradient<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_95) = data.len();

        match ::core::primitive::bool::then_some(value_95 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintLinearGradient<'a> {
    data: FontData<'a>,
}

impl<'a> PaintLinearGradient<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + Offset24::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn color_line_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.color_line_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn color_line(&self) -> Result<ColorLine<'a>, ReadError> {
        let data = self.data;
        let Ok(color_line_offset) = self.color_line_offset();

        color_line_offset.resolve(data)
    }

    pub fn x0(&self) -> Result<FWord, Never> {
        let Ok(range) = self.x0_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y0(&self) -> Result<FWord, Never> {
        let Ok(range) = self.y0_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn x1(&self) -> Result<FWord, Never> {
        let Ok(range) = self.x1_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y1(&self) -> Result<FWord, Never> {
        let Ok(range) = self.y1_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn x2(&self) -> Result<FWord, Never> {
        let Ok(range) = self.x2_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y2(&self) -> Result<FWord, Never> {
        let Ok(range) = self.y2_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn color_line_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x0_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.color_line_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y0_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x0_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x1_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y0_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y1_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x1_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x2_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y1_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y2_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x2_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintLinearGradient<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintLinearGradient")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "color_line_offset", value: FieldType::from(FieldType::offset(self.color_line_offset(), (self.color_line()).map(Some))) }),
            2u64 => Some(Field { name: "x0", value: FieldType::from(self.x0()) }),
            3u64 => Some(Field { name: "y0", value: FieldType::from(self.y0()) }),
            4u64 => Some(Field { name: "x1", value: FieldType::from(self.x1()) }),
            5u64 => Some(Field { name: "y1", value: FieldType::from(self.y1()) }),
            6u64 => Some(Field { name: "x2", value: FieldType::from(self.x2()) }),
            7u64 => Some(Field { name: "y2", value: FieldType::from(self.y2()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintLinearGradient<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintVarLinearGradient<'_> {
    const FORMAT: u8 = 5;
}

impl<'a> MinByteRange<'a> for PaintVarLinearGradient<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(var_index_base_byte_range) = self.var_index_base_byte_range();

        Ok(0..var_index_base_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintVarLinearGradient<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_96) = data.len();

        match ::core::primitive::bool::then_some(value_96 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintVarLinearGradient<'a> {
    data: FontData<'a>,
}

impl<'a> PaintVarLinearGradient<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + Offset24::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn color_line_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.color_line_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn color_line(&self) -> Result<VarColorLine<'a>, ReadError> {
        let data = self.data;
        let Ok(color_line_offset) = self.color_line_offset();

        color_line_offset.resolve(data)
    }

    pub fn x0(&self) -> Result<FWord, Never> {
        let Ok(range) = self.x0_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y0(&self) -> Result<FWord, Never> {
        let Ok(range) = self.y0_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn x1(&self) -> Result<FWord, Never> {
        let Ok(range) = self.x1_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y1(&self) -> Result<FWord, Never> {
        let Ok(range) = self.y1_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn x2(&self) -> Result<FWord, Never> {
        let Ok(range) = self.x2_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y2(&self) -> Result<FWord, Never> {
        let Ok(range) = self.y2_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn var_index_base(&self) -> Result<u32, Never> {
        let Ok(range) = self.var_index_base_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn color_line_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x0_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.color_line_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y0_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x0_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x1_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y0_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y1_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x1_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x2_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y1_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y2_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x2_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn var_index_base_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y2_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintVarLinearGradient<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintVarLinearGradient")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "color_line_offset", value: FieldType::from(FieldType::offset(self.color_line_offset(), (self.color_line()).map(Some))) }),
            2u64 => Some(Field { name: "x0", value: FieldType::from(self.x0()) }),
            3u64 => Some(Field { name: "y0", value: FieldType::from(self.y0()) }),
            4u64 => Some(Field { name: "x1", value: FieldType::from(self.x1()) }),
            5u64 => Some(Field { name: "y1", value: FieldType::from(self.y1()) }),
            6u64 => Some(Field { name: "x2", value: FieldType::from(self.x2()) }),
            7u64 => Some(Field { name: "y2", value: FieldType::from(self.y2()) }),
            8u64 => Some(Field { name: "var_index_base", value: FieldType::from(self.var_index_base()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintVarLinearGradient<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintRadialGradient<'_> {
    const FORMAT: u8 = 6;
}

impl<'a> MinByteRange<'a> for PaintRadialGradient<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(radius1_byte_range) = self.radius1_byte_range();

        Ok(0..radius1_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintRadialGradient<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_97) = data.len();

        match ::core::primitive::bool::then_some(value_97 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintRadialGradient<'a> {
    data: FontData<'a>,
}

impl<'a> PaintRadialGradient<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + Offset24::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + UfWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + UfWord::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn color_line_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.color_line_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn color_line(&self) -> Result<ColorLine<'a>, ReadError> {
        let data = self.data;
        let Ok(color_line_offset) = self.color_line_offset();

        color_line_offset.resolve(data)
    }

    pub fn x0(&self) -> Result<FWord, Never> {
        let Ok(range) = self.x0_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y0(&self) -> Result<FWord, Never> {
        let Ok(range) = self.y0_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn radius0(&self) -> Result<UfWord, Never> {
        let Ok(range) = self.radius0_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn x1(&self) -> Result<FWord, Never> {
        let Ok(range) = self.x1_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y1(&self) -> Result<FWord, Never> {
        let Ok(range) = self.y1_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn radius1(&self) -> Result<UfWord, Never> {
        let Ok(range) = self.radius1_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn color_line_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x0_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.color_line_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y0_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x0_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn radius0_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y0_byte_range();
        let start = previous.end;
        let end = start.saturating_add(UfWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x1_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.radius0_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y1_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x1_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn radius1_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y1_byte_range();
        let start = previous.end;
        let end = start.saturating_add(UfWord::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintRadialGradient<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintRadialGradient")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "color_line_offset", value: FieldType::from(FieldType::offset(self.color_line_offset(), (self.color_line()).map(Some))) }),
            2u64 => Some(Field { name: "x0", value: FieldType::from(self.x0()) }),
            3u64 => Some(Field { name: "y0", value: FieldType::from(self.y0()) }),
            4u64 => Some(Field { name: "radius0", value: FieldType::from(self.radius0()) }),
            5u64 => Some(Field { name: "x1", value: FieldType::from(self.x1()) }),
            6u64 => Some(Field { name: "y1", value: FieldType::from(self.y1()) }),
            7u64 => Some(Field { name: "radius1", value: FieldType::from(self.radius1()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintRadialGradient<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintVarRadialGradient<'_> {
    const FORMAT: u8 = 7;
}

impl<'a> MinByteRange<'a> for PaintVarRadialGradient<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(var_index_base_byte_range) = self.var_index_base_byte_range();

        Ok(0..var_index_base_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintVarRadialGradient<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_98) = data.len();

        match ::core::primitive::bool::then_some(value_98 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintVarRadialGradient<'a> {
    data: FontData<'a>,
}

impl<'a> PaintVarRadialGradient<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + Offset24::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + UfWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + UfWord::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn color_line_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.color_line_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn color_line(&self) -> Result<VarColorLine<'a>, ReadError> {
        let data = self.data;
        let Ok(color_line_offset) = self.color_line_offset();

        color_line_offset.resolve(data)
    }

    pub fn x0(&self) -> Result<FWord, Never> {
        let Ok(range) = self.x0_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y0(&self) -> Result<FWord, Never> {
        let Ok(range) = self.y0_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn radius0(&self) -> Result<UfWord, Never> {
        let Ok(range) = self.radius0_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn x1(&self) -> Result<FWord, Never> {
        let Ok(range) = self.x1_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y1(&self) -> Result<FWord, Never> {
        let Ok(range) = self.y1_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn radius1(&self) -> Result<UfWord, Never> {
        let Ok(range) = self.radius1_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn var_index_base(&self) -> Result<u32, Never> {
        let Ok(range) = self.var_index_base_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn color_line_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x0_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.color_line_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y0_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x0_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn radius0_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y0_byte_range();
        let start = previous.end;
        let end = start.saturating_add(UfWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x1_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.radius0_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y1_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x1_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn radius1_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y1_byte_range();
        let start = previous.end;
        let end = start.saturating_add(UfWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn var_index_base_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.radius1_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintVarRadialGradient<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintVarRadialGradient")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "color_line_offset", value: FieldType::from(FieldType::offset(self.color_line_offset(), (self.color_line()).map(Some))) }),
            2u64 => Some(Field { name: "x0", value: FieldType::from(self.x0()) }),
            3u64 => Some(Field { name: "y0", value: FieldType::from(self.y0()) }),
            4u64 => Some(Field { name: "radius0", value: FieldType::from(self.radius0()) }),
            5u64 => Some(Field { name: "x1", value: FieldType::from(self.x1()) }),
            6u64 => Some(Field { name: "y1", value: FieldType::from(self.y1()) }),
            7u64 => Some(Field { name: "radius1", value: FieldType::from(self.radius1()) }),
            8u64 => Some(Field { name: "var_index_base", value: FieldType::from(self.var_index_base()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintVarRadialGradient<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintSweepGradient<'_> {
    const FORMAT: u8 = 8;
}

impl<'a> MinByteRange<'a> for PaintSweepGradient<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(end_angle_byte_range) = self.end_angle_byte_range();

        Ok(0..end_angle_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintSweepGradient<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_99) = data.len();

        match ::core::primitive::bool::then_some(value_99 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintSweepGradient<'a> {
    data: FontData<'a>,
}

impl<'a> PaintSweepGradient<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + Offset24::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + F2Dot14::RAW_BYTE_LEN
        + F2Dot14::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn color_line_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.color_line_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn color_line(&self) -> Result<ColorLine<'a>, ReadError> {
        let data = self.data;
        let Ok(color_line_offset) = self.color_line_offset();

        color_line_offset.resolve(data)
    }

    pub fn center_x(&self) -> Result<FWord, Never> {
        let Ok(range) = self.center_x_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn center_y(&self) -> Result<FWord, Never> {
        let Ok(range) = self.center_y_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn start_angle(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.start_angle_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn end_angle(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.end_angle_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn color_line_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn center_x_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.color_line_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn center_y_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.center_x_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn start_angle_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.center_y_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn end_angle_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.start_angle_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintSweepGradient<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintSweepGradient")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "color_line_offset", value: FieldType::from(FieldType::offset(self.color_line_offset(), (self.color_line()).map(Some))) }),
            2u64 => Some(Field { name: "center_x", value: FieldType::from(self.center_x()) }),
            3u64 => Some(Field { name: "center_y", value: FieldType::from(self.center_y()) }),
            4u64 => Some(Field { name: "start_angle", value: FieldType::from(self.start_angle()) }),
            5u64 => Some(Field { name: "end_angle", value: FieldType::from(self.end_angle()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintSweepGradient<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintVarSweepGradient<'_> {
    const FORMAT: u8 = 9;
}

impl<'a> MinByteRange<'a> for PaintVarSweepGradient<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(var_index_base_byte_range) = self.var_index_base_byte_range();

        Ok(0..var_index_base_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintVarSweepGradient<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_100) = data.len();

        match ::core::primitive::bool::then_some(value_100 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintVarSweepGradient<'a> {
    data: FontData<'a>,
}

impl<'a> PaintVarSweepGradient<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + Offset24::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + F2Dot14::RAW_BYTE_LEN
        + F2Dot14::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn color_line_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.color_line_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn color_line(&self) -> Result<VarColorLine<'a>, ReadError> {
        let data = self.data;
        let Ok(color_line_offset) = self.color_line_offset();

        color_line_offset.resolve(data)
    }

    pub fn center_x(&self) -> Result<FWord, Never> {
        let Ok(range) = self.center_x_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn center_y(&self) -> Result<FWord, Never> {
        let Ok(range) = self.center_y_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn start_angle(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.start_angle_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn end_angle(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.end_angle_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn var_index_base(&self) -> Result<u32, Never> {
        let Ok(range) = self.var_index_base_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn color_line_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn center_x_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.color_line_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn center_y_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.center_x_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn start_angle_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.center_y_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn end_angle_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.start_angle_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn var_index_base_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.end_angle_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintVarSweepGradient<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintVarSweepGradient")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "color_line_offset", value: FieldType::from(FieldType::offset(self.color_line_offset(), (self.color_line()).map(Some))) }),
            2u64 => Some(Field { name: "center_x", value: FieldType::from(self.center_x()) }),
            3u64 => Some(Field { name: "center_y", value: FieldType::from(self.center_y()) }),
            4u64 => Some(Field { name: "start_angle", value: FieldType::from(self.start_angle()) }),
            5u64 => Some(Field { name: "end_angle", value: FieldType::from(self.end_angle()) }),
            6u64 => Some(Field { name: "var_index_base", value: FieldType::from(self.var_index_base()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintVarSweepGradient<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintGlyph<'_> {
    const FORMAT: u8 = 10;
}

impl<'a> MinByteRange<'a> for PaintGlyph<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(glyph_id_byte_range) = self.glyph_id_byte_range();

        Ok(0..glyph_id_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintGlyph<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_101) = data.len();

        match ::core::primitive::bool::then_some(value_101 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintGlyph<'a> {
    data: FontData<'a>,
}

impl<'a> PaintGlyph<'a> {
    pub const MIN_SIZE: u64 =
        (u8::RAW_BYTE_LEN + Offset24::RAW_BYTE_LEN + GlyphId16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }

    pub fn glyph_id(&self) -> Result<GlyphId16, Never> {
        let Ok(range) = self.glyph_id_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_id_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(GlyphId16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintGlyph<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintGlyph")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint()).map(Some))) }),
            2u64 => Some(Field { name: "glyph_id", value: FieldType::from(self.glyph_id()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintGlyph<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintColrGlyph<'_> {
    const FORMAT: u8 = 11;
}

impl<'a> MinByteRange<'a> for PaintColrGlyph<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(glyph_id_byte_range) = self.glyph_id_byte_range();

        Ok(0..glyph_id_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintColrGlyph<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_102) = data.len();

        match ::core::primitive::bool::then_some(value_102 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintColrGlyph<'a> {
    data: FontData<'a>,
}

impl<'a> PaintColrGlyph<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN + GlyphId16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn glyph_id(&self) -> Result<GlyphId16, Never> {
        let Ok(range) = self.glyph_id_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_id_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(GlyphId16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintColrGlyph<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintColrGlyph")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "glyph_id", value: FieldType::from(self.glyph_id()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintColrGlyph<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintTransform<'_> {
    const FORMAT: u8 = 12;
}

impl<'a> MinByteRange<'a> for PaintTransform<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(transform_offset_byte_range) = self.transform_offset_byte_range();

        Ok(0..transform_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintTransform<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_103) = data.len();

        match ::core::primitive::bool::then_some(value_103 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintTransform<'a> {
    data: FontData<'a>,
}

impl<'a> PaintTransform<'a> {
    pub const MIN_SIZE: u64 =
        (u8::RAW_BYTE_LEN + Offset24::RAW_BYTE_LEN + Offset24::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }

    pub fn transform_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.transform_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn transform(&self) -> Result<Affine2x3<'a>, ReadError> {
        let data = self.data;
        let Ok(transform_offset) = self.transform_offset();

        transform_offset.resolve(data)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn transform_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintTransform<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintTransform")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint()).map(Some))) }),
            2u64 => Some(Field { name: "transform_offset", value: FieldType::from(FieldType::offset(self.transform_offset(), (self.transform()).map(Some))) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintTransform<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintVarTransform<'_> {
    const FORMAT: u8 = 13;
}

impl<'a> MinByteRange<'a> for PaintVarTransform<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(transform_offset_byte_range) = self.transform_offset_byte_range();

        Ok(0..transform_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintVarTransform<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_104) = data.len();

        match ::core::primitive::bool::then_some(value_104 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintVarTransform<'a> {
    data: FontData<'a>,
}

impl<'a> PaintVarTransform<'a> {
    pub const MIN_SIZE: u64 =
        (u8::RAW_BYTE_LEN + Offset24::RAW_BYTE_LEN + Offset24::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }

    pub fn transform_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.transform_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn transform(&self) -> Result<VarAffine2x3<'a>, ReadError> {
        let data = self.data;
        let Ok(transform_offset) = self.transform_offset();

        transform_offset.resolve(data)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn transform_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintVarTransform<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintVarTransform")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint()).map(Some))) }),
            2u64 => Some(Field { name: "transform_offset", value: FieldType::from(FieldType::offset(self.transform_offset(), (self.transform()).map(Some))) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintVarTransform<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for Affine2x3<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(dy_byte_range) = self.dy_byte_range();

        Ok(0..dy_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Affine2x3<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_105) = data.len();

        match ::core::primitive::bool::then_some(value_105 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Affine2x3<'a> {
    data: FontData<'a>,
}

impl<'a> Affine2x3<'a> {
    pub const MIN_SIZE: u64 = (Fixed::RAW_BYTE_LEN
        + Fixed::RAW_BYTE_LEN
        + Fixed::RAW_BYTE_LEN
        + Fixed::RAW_BYTE_LEN
        + Fixed::RAW_BYTE_LEN
        + Fixed::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn xx(&self) -> Result<Fixed, Never> {
        let Ok(range) = self.xx_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn yx(&self) -> Result<Fixed, Never> {
        let Ok(range) = self.yx_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn xy(&self) -> Result<Fixed, Never> {
        let Ok(range) = self.xy_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn yy(&self) -> Result<Fixed, Never> {
        let Ok(range) = self.yy_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn dx(&self) -> Result<Fixed, Never> {
        let Ok(range) = self.dx_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn dy(&self) -> Result<Fixed, Never> {
        let Ok(range) = self.dy_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn xx_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(Fixed::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn yx_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.xx_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Fixed::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn xy_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.yx_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Fixed::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn yy_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.xy_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Fixed::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn dx_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.yy_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Fixed::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn dy_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.dx_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Fixed::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Affine2x3::MIN_SIZE); held }));

impl Default for Affine2x3<'_> {
    fn default() -> Self {
        let Ok(value_106) = FontData::default_table_data();

        Self {
            data: value_106,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Affine2x3<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Affine2x3")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "xx", value: FieldType::from(self.xx()) }),
            1u64 => Some(Field { name: "yx", value: FieldType::from(self.yx()) }),
            2u64 => Some(Field { name: "xy", value: FieldType::from(self.xy()) }),
            3u64 => Some(Field { name: "yy", value: FieldType::from(self.yy()) }),
            4u64 => Some(Field { name: "dx", value: FieldType::from(self.dx()) }),
            5u64 => Some(Field { name: "dy", value: FieldType::from(self.dy()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Affine2x3<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for VarAffine2x3<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(var_index_base_byte_range) = self.var_index_base_byte_range();

        Ok(0..var_index_base_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for VarAffine2x3<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_107) = data.len();

        match ::core::primitive::bool::then_some(value_107 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct VarAffine2x3<'a> {
    data: FontData<'a>,
}

impl<'a> VarAffine2x3<'a> {
    pub const MIN_SIZE: u64 = (Fixed::RAW_BYTE_LEN
        + Fixed::RAW_BYTE_LEN
        + Fixed::RAW_BYTE_LEN
        + Fixed::RAW_BYTE_LEN
        + Fixed::RAW_BYTE_LEN
        + Fixed::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn xx(&self) -> Result<Fixed, Never> {
        let Ok(range) = self.xx_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn yx(&self) -> Result<Fixed, Never> {
        let Ok(range) = self.yx_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn xy(&self) -> Result<Fixed, Never> {
        let Ok(range) = self.xy_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn yy(&self) -> Result<Fixed, Never> {
        let Ok(range) = self.yy_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn dx(&self) -> Result<Fixed, Never> {
        let Ok(range) = self.dx_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn dy(&self) -> Result<Fixed, Never> {
        let Ok(range) = self.dy_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn var_index_base(&self) -> Result<u32, Never> {
        let Ok(range) = self.var_index_base_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn xx_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(Fixed::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn yx_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.xx_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Fixed::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn xy_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.yx_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Fixed::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn yy_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.xy_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Fixed::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn dx_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.yy_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Fixed::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn dy_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.dx_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Fixed::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn var_index_base_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.dy_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(VarAffine2x3::MIN_SIZE); held }));

impl Default for VarAffine2x3<'_> {
    fn default() -> Self {
        let Ok(value_108) = FontData::default_table_data();

        Self {
            data: value_108,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for VarAffine2x3<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("VarAffine2x3")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "xx", value: FieldType::from(self.xx()) }),
            1u64 => Some(Field { name: "yx", value: FieldType::from(self.yx()) }),
            2u64 => Some(Field { name: "xy", value: FieldType::from(self.xy()) }),
            3u64 => Some(Field { name: "yy", value: FieldType::from(self.yy()) }),
            4u64 => Some(Field { name: "dx", value: FieldType::from(self.dx()) }),
            5u64 => Some(Field { name: "dy", value: FieldType::from(self.dy()) }),
            6u64 => Some(Field { name: "var_index_base", value: FieldType::from(self.var_index_base()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for VarAffine2x3<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintTranslate<'_> {
    const FORMAT: u8 = 14;
}

impl<'a> MinByteRange<'a> for PaintTranslate<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(dy_byte_range) = self.dy_byte_range();

        Ok(0..dy_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintTranslate<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_109) = data.len();

        match ::core::primitive::bool::then_some(value_109 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintTranslate<'a> {
    data: FontData<'a>,
}

impl<'a> PaintTranslate<'a> {
    pub const MIN_SIZE: u64 =
        (u8::RAW_BYTE_LEN + Offset24::RAW_BYTE_LEN + FWord::RAW_BYTE_LEN + FWord::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }

    pub fn dx(&self) -> Result<FWord, Never> {
        let Ok(range) = self.dx_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn dy(&self) -> Result<FWord, Never> {
        let Ok(range) = self.dy_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn dx_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn dy_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.dx_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintTranslate<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintTranslate")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint()).map(Some))) }),
            2u64 => Some(Field { name: "dx", value: FieldType::from(self.dx()) }),
            3u64 => Some(Field { name: "dy", value: FieldType::from(self.dy()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintTranslate<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintVarTranslate<'_> {
    const FORMAT: u8 = 15;
}

impl<'a> MinByteRange<'a> for PaintVarTranslate<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(var_index_base_byte_range) = self.var_index_base_byte_range();

        Ok(0..var_index_base_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintVarTranslate<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_110) = data.len();

        match ::core::primitive::bool::then_some(value_110 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintVarTranslate<'a> {
    data: FontData<'a>,
}

impl<'a> PaintVarTranslate<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + Offset24::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }

    pub fn dx(&self) -> Result<FWord, Never> {
        let Ok(range) = self.dx_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn dy(&self) -> Result<FWord, Never> {
        let Ok(range) = self.dy_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn var_index_base(&self) -> Result<u32, Never> {
        let Ok(range) = self.var_index_base_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn dx_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn dy_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.dx_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn var_index_base_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.dy_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintVarTranslate<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintVarTranslate")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint()).map(Some))) }),
            2u64 => Some(Field { name: "dx", value: FieldType::from(self.dx()) }),
            3u64 => Some(Field { name: "dy", value: FieldType::from(self.dy()) }),
            4u64 => Some(Field { name: "var_index_base", value: FieldType::from(self.var_index_base()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintVarTranslate<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintScale<'_> {
    const FORMAT: u8 = 16;
}

impl<'a> MinByteRange<'a> for PaintScale<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(scale_y_byte_range) = self.scale_y_byte_range();

        Ok(0..scale_y_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintScale<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_111) = data.len();

        match ::core::primitive::bool::then_some(value_111 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintScale<'a> {
    data: FontData<'a>,
}

impl<'a> PaintScale<'a> {
    pub const MIN_SIZE: u64 =
        (u8::RAW_BYTE_LEN + Offset24::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }

    pub fn scale_x(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.scale_x_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn scale_y(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.scale_y_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn scale_x_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn scale_y_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.scale_x_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintScale<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintScale")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint()).map(Some))) }),
            2u64 => Some(Field { name: "scale_x", value: FieldType::from(self.scale_x()) }),
            3u64 => Some(Field { name: "scale_y", value: FieldType::from(self.scale_y()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintScale<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintVarScale<'_> {
    const FORMAT: u8 = 17;
}

impl<'a> MinByteRange<'a> for PaintVarScale<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(var_index_base_byte_range) = self.var_index_base_byte_range();

        Ok(0..var_index_base_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintVarScale<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_112) = data.len();

        match ::core::primitive::bool::then_some(value_112 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintVarScale<'a> {
    data: FontData<'a>,
}

impl<'a> PaintVarScale<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + Offset24::RAW_BYTE_LEN
        + F2Dot14::RAW_BYTE_LEN
        + F2Dot14::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }

    pub fn scale_x(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.scale_x_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn scale_y(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.scale_y_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn var_index_base(&self) -> Result<u32, Never> {
        let Ok(range) = self.var_index_base_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn scale_x_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn scale_y_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.scale_x_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn var_index_base_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.scale_y_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintVarScale<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintVarScale")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint()).map(Some))) }),
            2u64 => Some(Field { name: "scale_x", value: FieldType::from(self.scale_x()) }),
            3u64 => Some(Field { name: "scale_y", value: FieldType::from(self.scale_y()) }),
            4u64 => Some(Field { name: "var_index_base", value: FieldType::from(self.var_index_base()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintVarScale<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintScaleAroundCenter<'_> {
    const FORMAT: u8 = 18;
}

impl<'a> MinByteRange<'a> for PaintScaleAroundCenter<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(center_y_byte_range) = self.center_y_byte_range();

        Ok(0..center_y_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintScaleAroundCenter<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_113) = data.len();

        match ::core::primitive::bool::then_some(value_113 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintScaleAroundCenter<'a> {
    data: FontData<'a>,
}

impl<'a> PaintScaleAroundCenter<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + Offset24::RAW_BYTE_LEN
        + F2Dot14::RAW_BYTE_LEN
        + F2Dot14::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }

    pub fn scale_x(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.scale_x_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn scale_y(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.scale_y_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn center_x(&self) -> Result<FWord, Never> {
        let Ok(range) = self.center_x_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn center_y(&self) -> Result<FWord, Never> {
        let Ok(range) = self.center_y_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn scale_x_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn scale_y_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.scale_x_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn center_x_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.scale_y_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn center_y_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.center_x_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintScaleAroundCenter<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintScaleAroundCenter")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint()).map(Some))) }),
            2u64 => Some(Field { name: "scale_x", value: FieldType::from(self.scale_x()) }),
            3u64 => Some(Field { name: "scale_y", value: FieldType::from(self.scale_y()) }),
            4u64 => Some(Field { name: "center_x", value: FieldType::from(self.center_x()) }),
            5u64 => Some(Field { name: "center_y", value: FieldType::from(self.center_y()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintScaleAroundCenter<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintVarScaleAroundCenter<'_> {
    const FORMAT: u8 = 19;
}

impl<'a> MinByteRange<'a> for PaintVarScaleAroundCenter<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(var_index_base_byte_range) = self.var_index_base_byte_range();

        Ok(0..var_index_base_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintVarScaleAroundCenter<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_114) = data.len();

        match ::core::primitive::bool::then_some(value_114 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintVarScaleAroundCenter<'a> {
    data: FontData<'a>,
}

impl<'a> PaintVarScaleAroundCenter<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + Offset24::RAW_BYTE_LEN
        + F2Dot14::RAW_BYTE_LEN
        + F2Dot14::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }

    pub fn scale_x(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.scale_x_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn scale_y(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.scale_y_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn center_x(&self) -> Result<FWord, Never> {
        let Ok(range) = self.center_x_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn center_y(&self) -> Result<FWord, Never> {
        let Ok(range) = self.center_y_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn var_index_base(&self) -> Result<u32, Never> {
        let Ok(range) = self.var_index_base_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn scale_x_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn scale_y_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.scale_x_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn center_x_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.scale_y_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn center_y_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.center_x_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn var_index_base_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.center_y_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintVarScaleAroundCenter<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintVarScaleAroundCenter")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint()).map(Some))) }),
            2u64 => Some(Field { name: "scale_x", value: FieldType::from(self.scale_x()) }),
            3u64 => Some(Field { name: "scale_y", value: FieldType::from(self.scale_y()) }),
            4u64 => Some(Field { name: "center_x", value: FieldType::from(self.center_x()) }),
            5u64 => Some(Field { name: "center_y", value: FieldType::from(self.center_y()) }),
            6u64 => Some(Field { name: "var_index_base", value: FieldType::from(self.var_index_base()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintVarScaleAroundCenter<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintScaleUniform<'_> {
    const FORMAT: u8 = 20;
}

impl<'a> MinByteRange<'a> for PaintScaleUniform<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(scale_byte_range) = self.scale_byte_range();

        Ok(0..scale_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintScaleUniform<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_115) = data.len();

        match ::core::primitive::bool::then_some(value_115 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintScaleUniform<'a> {
    data: FontData<'a>,
}

impl<'a> PaintScaleUniform<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN + Offset24::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }

    pub fn scale(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.scale_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn scale_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintScaleUniform<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintScaleUniform")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint()).map(Some))) }),
            2u64 => Some(Field { name: "scale", value: FieldType::from(self.scale()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintScaleUniform<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintVarScaleUniform<'_> {
    const FORMAT: u8 = 21;
}

impl<'a> MinByteRange<'a> for PaintVarScaleUniform<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(var_index_base_byte_range) = self.var_index_base_byte_range();

        Ok(0..var_index_base_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintVarScaleUniform<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_116) = data.len();

        match ::core::primitive::bool::then_some(value_116 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintVarScaleUniform<'a> {
    data: FontData<'a>,
}

impl<'a> PaintVarScaleUniform<'a> {
    pub const MIN_SIZE: u64 =
        (u8::RAW_BYTE_LEN + Offset24::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }

    pub fn scale(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.scale_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn var_index_base(&self) -> Result<u32, Never> {
        let Ok(range) = self.var_index_base_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn scale_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn var_index_base_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.scale_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintVarScaleUniform<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintVarScaleUniform")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint()).map(Some))) }),
            2u64 => Some(Field { name: "scale", value: FieldType::from(self.scale()) }),
            3u64 => Some(Field { name: "var_index_base", value: FieldType::from(self.var_index_base()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintVarScaleUniform<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintScaleUniformAroundCenter<'_> {
    const FORMAT: u8 = 22;
}

impl<'a> MinByteRange<'a> for PaintScaleUniformAroundCenter<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(center_y_byte_range) = self.center_y_byte_range();

        Ok(0..center_y_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintScaleUniformAroundCenter<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_117) = data.len();

        match ::core::primitive::bool::then_some(value_117 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintScaleUniformAroundCenter<'a> {
    data: FontData<'a>,
}

impl<'a> PaintScaleUniformAroundCenter<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + Offset24::RAW_BYTE_LEN
        + F2Dot14::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }

    pub fn scale(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.scale_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn center_x(&self) -> Result<FWord, Never> {
        let Ok(range) = self.center_x_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn center_y(&self) -> Result<FWord, Never> {
        let Ok(range) = self.center_y_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn scale_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn center_x_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.scale_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn center_y_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.center_x_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintScaleUniformAroundCenter<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintScaleUniformAroundCenter")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint()).map(Some))) }),
            2u64 => Some(Field { name: "scale", value: FieldType::from(self.scale()) }),
            3u64 => Some(Field { name: "center_x", value: FieldType::from(self.center_x()) }),
            4u64 => Some(Field { name: "center_y", value: FieldType::from(self.center_y()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintScaleUniformAroundCenter<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintVarScaleUniformAroundCenter<'_> {
    const FORMAT: u8 = 23;
}

impl<'a> MinByteRange<'a> for PaintVarScaleUniformAroundCenter<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(var_index_base_byte_range) = self.var_index_base_byte_range();

        Ok(0..var_index_base_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintVarScaleUniformAroundCenter<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_118) = data.len();

        match ::core::primitive::bool::then_some(value_118 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintVarScaleUniformAroundCenter<'a> {
    data: FontData<'a>,
}

impl<'a> PaintVarScaleUniformAroundCenter<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + Offset24::RAW_BYTE_LEN
        + F2Dot14::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }

    pub fn scale(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.scale_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn center_x(&self) -> Result<FWord, Never> {
        let Ok(range) = self.center_x_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn center_y(&self) -> Result<FWord, Never> {
        let Ok(range) = self.center_y_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn var_index_base(&self) -> Result<u32, Never> {
        let Ok(range) = self.var_index_base_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn scale_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn center_x_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.scale_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn center_y_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.center_x_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn var_index_base_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.center_y_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintVarScaleUniformAroundCenter<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintVarScaleUniformAroundCenter")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint()).map(Some))) }),
            2u64 => Some(Field { name: "scale", value: FieldType::from(self.scale()) }),
            3u64 => Some(Field { name: "center_x", value: FieldType::from(self.center_x()) }),
            4u64 => Some(Field { name: "center_y", value: FieldType::from(self.center_y()) }),
            5u64 => Some(Field { name: "var_index_base", value: FieldType::from(self.var_index_base()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintVarScaleUniformAroundCenter<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintRotate<'_> {
    const FORMAT: u8 = 24;
}

impl<'a> MinByteRange<'a> for PaintRotate<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(angle_byte_range) = self.angle_byte_range();

        Ok(0..angle_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintRotate<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_119) = data.len();

        match ::core::primitive::bool::then_some(value_119 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintRotate<'a> {
    data: FontData<'a>,
}

impl<'a> PaintRotate<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN + Offset24::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }

    pub fn angle(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.angle_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn angle_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintRotate<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintRotate")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint()).map(Some))) }),
            2u64 => Some(Field { name: "angle", value: FieldType::from(self.angle()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintRotate<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintVarRotate<'_> {
    const FORMAT: u8 = 25;
}

impl<'a> MinByteRange<'a> for PaintVarRotate<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(var_index_base_byte_range) = self.var_index_base_byte_range();

        Ok(0..var_index_base_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintVarRotate<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_120) = data.len();

        match ::core::primitive::bool::then_some(value_120 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintVarRotate<'a> {
    data: FontData<'a>,
}

impl<'a> PaintVarRotate<'a> {
    pub const MIN_SIZE: u64 =
        (u8::RAW_BYTE_LEN + Offset24::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }

    pub fn angle(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.angle_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn var_index_base(&self) -> Result<u32, Never> {
        let Ok(range) = self.var_index_base_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn angle_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn var_index_base_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.angle_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintVarRotate<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintVarRotate")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint()).map(Some))) }),
            2u64 => Some(Field { name: "angle", value: FieldType::from(self.angle()) }),
            3u64 => Some(Field { name: "var_index_base", value: FieldType::from(self.var_index_base()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintVarRotate<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintRotateAroundCenter<'_> {
    const FORMAT: u8 = 26;
}

impl<'a> MinByteRange<'a> for PaintRotateAroundCenter<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(center_y_byte_range) = self.center_y_byte_range();

        Ok(0..center_y_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintRotateAroundCenter<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_121) = data.len();

        match ::core::primitive::bool::then_some(value_121 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintRotateAroundCenter<'a> {
    data: FontData<'a>,
}

impl<'a> PaintRotateAroundCenter<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + Offset24::RAW_BYTE_LEN
        + F2Dot14::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }

    pub fn angle(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.angle_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn center_x(&self) -> Result<FWord, Never> {
        let Ok(range) = self.center_x_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn center_y(&self) -> Result<FWord, Never> {
        let Ok(range) = self.center_y_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn angle_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn center_x_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.angle_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn center_y_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.center_x_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintRotateAroundCenter<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintRotateAroundCenter")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint()).map(Some))) }),
            2u64 => Some(Field { name: "angle", value: FieldType::from(self.angle()) }),
            3u64 => Some(Field { name: "center_x", value: FieldType::from(self.center_x()) }),
            4u64 => Some(Field { name: "center_y", value: FieldType::from(self.center_y()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintRotateAroundCenter<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintVarRotateAroundCenter<'_> {
    const FORMAT: u8 = 27;
}

impl<'a> MinByteRange<'a> for PaintVarRotateAroundCenter<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(var_index_base_byte_range) = self.var_index_base_byte_range();

        Ok(0..var_index_base_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintVarRotateAroundCenter<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_122) = data.len();

        match ::core::primitive::bool::then_some(value_122 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintVarRotateAroundCenter<'a> {
    data: FontData<'a>,
}

impl<'a> PaintVarRotateAroundCenter<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + Offset24::RAW_BYTE_LEN
        + F2Dot14::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }

    pub fn angle(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.angle_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn center_x(&self) -> Result<FWord, Never> {
        let Ok(range) = self.center_x_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn center_y(&self) -> Result<FWord, Never> {
        let Ok(range) = self.center_y_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn var_index_base(&self) -> Result<u32, Never> {
        let Ok(range) = self.var_index_base_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn angle_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn center_x_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.angle_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn center_y_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.center_x_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn var_index_base_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.center_y_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintVarRotateAroundCenter<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintVarRotateAroundCenter")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint()).map(Some))) }),
            2u64 => Some(Field { name: "angle", value: FieldType::from(self.angle()) }),
            3u64 => Some(Field { name: "center_x", value: FieldType::from(self.center_x()) }),
            4u64 => Some(Field { name: "center_y", value: FieldType::from(self.center_y()) }),
            5u64 => Some(Field { name: "var_index_base", value: FieldType::from(self.var_index_base()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintVarRotateAroundCenter<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintSkew<'_> {
    const FORMAT: u8 = 28;
}

impl<'a> MinByteRange<'a> for PaintSkew<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(y_skew_angle_byte_range) = self.y_skew_angle_byte_range();

        Ok(0..y_skew_angle_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintSkew<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_123) = data.len();

        match ::core::primitive::bool::then_some(value_123 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintSkew<'a> {
    data: FontData<'a>,
}

impl<'a> PaintSkew<'a> {
    pub const MIN_SIZE: u64 =
        (u8::RAW_BYTE_LEN + Offset24::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }

    pub fn x_skew_angle(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.x_skew_angle_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_skew_angle(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.y_skew_angle_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x_skew_angle_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_skew_angle_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x_skew_angle_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintSkew<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintSkew")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint()).map(Some))) }),
            2u64 => Some(Field { name: "x_skew_angle", value: FieldType::from(self.x_skew_angle()) }),
            3u64 => Some(Field { name: "y_skew_angle", value: FieldType::from(self.y_skew_angle()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintSkew<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintVarSkew<'_> {
    const FORMAT: u8 = 29;
}

impl<'a> MinByteRange<'a> for PaintVarSkew<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(var_index_base_byte_range) = self.var_index_base_byte_range();

        Ok(0..var_index_base_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintVarSkew<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_124) = data.len();

        match ::core::primitive::bool::then_some(value_124 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintVarSkew<'a> {
    data: FontData<'a>,
}

impl<'a> PaintVarSkew<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + Offset24::RAW_BYTE_LEN
        + F2Dot14::RAW_BYTE_LEN
        + F2Dot14::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }

    pub fn x_skew_angle(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.x_skew_angle_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_skew_angle(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.y_skew_angle_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn var_index_base(&self) -> Result<u32, Never> {
        let Ok(range) = self.var_index_base_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x_skew_angle_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_skew_angle_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x_skew_angle_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn var_index_base_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y_skew_angle_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintVarSkew<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintVarSkew")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint()).map(Some))) }),
            2u64 => Some(Field { name: "x_skew_angle", value: FieldType::from(self.x_skew_angle()) }),
            3u64 => Some(Field { name: "y_skew_angle", value: FieldType::from(self.y_skew_angle()) }),
            4u64 => Some(Field { name: "var_index_base", value: FieldType::from(self.var_index_base()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintVarSkew<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintSkewAroundCenter<'_> {
    const FORMAT: u8 = 30;
}

impl<'a> MinByteRange<'a> for PaintSkewAroundCenter<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(center_y_byte_range) = self.center_y_byte_range();

        Ok(0..center_y_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintSkewAroundCenter<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_125) = data.len();

        match ::core::primitive::bool::then_some(value_125 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintSkewAroundCenter<'a> {
    data: FontData<'a>,
}

impl<'a> PaintSkewAroundCenter<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + Offset24::RAW_BYTE_LEN
        + F2Dot14::RAW_BYTE_LEN
        + F2Dot14::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }

    pub fn x_skew_angle(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.x_skew_angle_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_skew_angle(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.y_skew_angle_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn center_x(&self) -> Result<FWord, Never> {
        let Ok(range) = self.center_x_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn center_y(&self) -> Result<FWord, Never> {
        let Ok(range) = self.center_y_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x_skew_angle_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_skew_angle_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x_skew_angle_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn center_x_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y_skew_angle_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn center_y_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.center_x_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintSkewAroundCenter<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintSkewAroundCenter")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint()).map(Some))) }),
            2u64 => Some(Field { name: "x_skew_angle", value: FieldType::from(self.x_skew_angle()) }),
            3u64 => Some(Field { name: "y_skew_angle", value: FieldType::from(self.y_skew_angle()) }),
            4u64 => Some(Field { name: "center_x", value: FieldType::from(self.center_x()) }),
            5u64 => Some(Field { name: "center_y", value: FieldType::from(self.center_y()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintSkewAroundCenter<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintVarSkewAroundCenter<'_> {
    const FORMAT: u8 = 31;
}

impl<'a> MinByteRange<'a> for PaintVarSkewAroundCenter<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(var_index_base_byte_range) = self.var_index_base_byte_range();

        Ok(0..var_index_base_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintVarSkewAroundCenter<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_126) = data.len();

        match ::core::primitive::bool::then_some(value_126 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintVarSkewAroundCenter<'a> {
    data: FontData<'a>,
}

impl<'a> PaintVarSkewAroundCenter<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + Offset24::RAW_BYTE_LEN
        + F2Dot14::RAW_BYTE_LEN
        + F2Dot14::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + FWord::RAW_BYTE_LEN
        + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(paint_offset) = self.paint_offset();

        paint_offset.resolve(data)
    }

    pub fn x_skew_angle(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.x_skew_angle_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn y_skew_angle(&self) -> Result<F2Dot14, Never> {
        let Ok(range) = self.y_skew_angle_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn center_x(&self) -> Result<FWord, Never> {
        let Ok(range) = self.center_x_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn center_y(&self) -> Result<FWord, Never> {
        let Ok(range) = self.center_y_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn var_index_base(&self) -> Result<u32, Never> {
        let Ok(range) = self.var_index_base_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn x_skew_angle_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn y_skew_angle_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.x_skew_angle_byte_range();
        let start = previous.end;
        let end = start.saturating_add(F2Dot14::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn center_x_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.y_skew_angle_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn center_y_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.center_x_byte_range();
        let start = previous.end;
        let end = start.saturating_add(FWord::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn var_index_base_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.center_y_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintVarSkewAroundCenter<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintVarSkewAroundCenter")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "paint_offset", value: FieldType::from(FieldType::offset(self.paint_offset(), (self.paint()).map(Some))) }),
            2u64 => Some(Field { name: "x_skew_angle", value: FieldType::from(self.x_skew_angle()) }),
            3u64 => Some(Field { name: "y_skew_angle", value: FieldType::from(self.y_skew_angle()) }),
            4u64 => Some(Field { name: "center_x", value: FieldType::from(self.center_x()) }),
            5u64 => Some(Field { name: "center_y", value: FieldType::from(self.center_y()) }),
            6u64 => Some(Field { name: "var_index_base", value: FieldType::from(self.var_index_base()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintVarSkewAroundCenter<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for PaintComposite<'_> {
    const FORMAT: u8 = 32;
}

impl<'a> MinByteRange<'a> for PaintComposite<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(backdrop_paint_offset_byte_range) = self.backdrop_paint_offset_byte_range();

        Ok(0..backdrop_paint_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for PaintComposite<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_127) = data.len();

        match ::core::primitive::bool::then_some(value_127 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct PaintComposite<'a> {
    data: FontData<'a>,
}

impl<'a> PaintComposite<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN
        + Offset24::RAW_BYTE_LEN
        + CompositeMode::RAW_BYTE_LEN
        + Offset24::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn source_paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.source_paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn source_paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(source_paint_offset) = self.source_paint_offset();

        source_paint_offset.resolve(data)
    }

    pub fn composite_mode(&self) -> Result<CompositeMode, Never> {
        let Ok(range) = self.composite_mode_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn backdrop_paint_offset(&self) -> Result<Offset24, Never> {
        let Ok(range) = self.backdrop_paint_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn backdrop_paint(&self) -> Result<Paint<'a>, ReadError> {
        let data = self.data;
        let Ok(backdrop_paint_offset) = self.backdrop_paint_offset();

        backdrop_paint_offset.resolve(data)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn source_paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn composite_mode_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.source_paint_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(CompositeMode::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn backdrop_paint_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.composite_mode_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset24::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for PaintComposite<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("PaintComposite")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "source_paint_offset", value: FieldType::from(FieldType::offset(self.source_paint_offset(), (self.source_paint()).map(Some))) }),
            2u64 => Some(Field { name: "composite_mode", value: FieldType::from(self.composite_mode()) }),
            3u64 => Some(Field { name: "backdrop_paint_offset", value: FieldType::from(FieldType::offset(self.backdrop_paint_offset(), (self.backdrop_paint()).map(Some))) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for PaintComposite<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
#[cfg_attr(
    dylint_lib = "explicit070_spaced_discriminants",
    expect(explicit070_spaced_discriminants, reason = "OpenType COLR composite modes use these specified byte values")
)]
pub enum CompositeMode {
    Clear = 0,
    Src = 1,
    Dest = 2,
    #[default]
    SrcOver = 3,
    DestOver = 4,
    SrcIn = 5,
    DestIn = 6,
    SrcOut = 7,
    DestOut = 8,
    SrcAtop = 9,
    DestAtop = 10,
    Xor = 11,
    Plus = 12,
    Screen = 13,
    Overlay = 14,
    Darken = 15,
    Lighten = 16,
    ColorDodge = 17,
    ColorBurn = 18,
    HardLight = 19,
    SoftLight = 20,
    Difference = 21,
    Exclusion = 22,
    Multiply = 23,
    HslHue = 24,
    HslSaturation = 25,
    HslColor = 26,
    HslLuminosity = 27,
    #[doc(hidden)]
    Unknown,
}

impl CompositeMode {
    fn discriminant_value(self) -> Result<u8, Never> {
        Ok(match self {
            Self::Clear => 0,
            Self::Src => 1,
            Self::Dest => 2,
            Self::SrcOver => 3,
            Self::DestOver => 4,
            Self::SrcIn => 5,
            Self::DestIn => 6,
            Self::SrcOut => 7,
            Self::DestOut => 8,
            Self::SrcAtop => 9,
            Self::DestAtop => 10,
            Self::Xor => 11,
            Self::Plus => 12,
            Self::Screen => 13,
            Self::Overlay => 14,
            Self::Darken => 15,
            Self::Lighten => 16,
            Self::ColorDodge => 17,
            Self::ColorBurn => 18,
            Self::HardLight => 19,
            Self::SoftLight => 20,
            Self::Difference => 21,
            Self::Exclusion => 22,
            Self::Multiply => 23,
            Self::HslHue => 24,
            Self::HslSaturation => 25,
            Self::HslColor => 26,
            Self::HslLuminosity => 27,
            Self::Unknown => 28,
        })
    }

    pub fn new(raw: u8) -> Result<Self, Never> {
        Ok(match raw {
            0 => Self::Clear,
            1 => Self::Src,
            2 => Self::Dest,
            3 => Self::SrcOver,
            4 => Self::DestOver,
            5 => Self::SrcIn,
            6 => Self::DestIn,
            7 => Self::SrcOut,
            8 => Self::DestOut,
            9 => Self::SrcAtop,
            10 => Self::DestAtop,
            11 => Self::Xor,
            12 => Self::Plus,
            13 => Self::Screen,
            14 => Self::Overlay,
            15 => Self::Darken,
            16 => Self::Lighten,
            17 => Self::ColorDodge,
            18 => Self::ColorBurn,
            19 => Self::HardLight,
            20 => Self::SoftLight,
            21 => Self::Difference,
            22 => Self::Exclusion,
            23 => Self::Multiply,
            24 => Self::HslHue,
            25 => Self::HslSaturation,
            26 => Self::HslColor,
            27 => Self::HslLuminosity,
            _ => Self::Unknown,
        })
    }
}

impl font_types::Scalar for CompositeMode {
    type Raw = <u8 as font_types::Scalar>::Raw;

    fn to_raw(self) -> std::result::Result<Self::Raw, console_core_never::Never> {
        let Ok(value) = self.discriminant_value();
        value.to_raw()
    }

    fn from_raw(raw: Self::Raw) -> std::result::Result<Self, console_core_never::Never> {
        let Ok(value_129) = <u8>::from_raw(raw);

        let t = value_129;
        let value_130 = t;

        let Ok(composite_mode) = Self::new(value_130);

        Ok(composite_mode)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> From<CompositeMode> for FieldType<'a> {
    fn from(src: CompositeMode) -> FieldType<'a> {
        let Ok(value) = src.discriminant_value();
        FieldType::from(value)
    }
}
