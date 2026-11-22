

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Avar<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(axis_segment_maps_byte_range) = self.axis_segment_maps_byte_range();

        Ok(0..axis_segment_maps_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Avar<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"avar"); value });
}

impl<'a> FontRead<'a> for Avar<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Avar<'a> {
    data: FontData<'a>,
}

impl<'a> Avar<'a> {
    pub const MIN_SIZE: u64 = (MajorMinor::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<MajorMinor, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn axis_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.axis_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn axis_segment_maps(&self) -> Result<VarLenArray<'a, SegmentMaps<'a>>, Never> {
        let Ok(range) = self.axis_segment_maps_byte_range();

        self.data.read_var_array_or_empty(range.start)
    }

    pub fn axis_index_map_offset(&self) -> Result<Option<Nullable<Offset32>>, Never> {
        let Ok(range) = self.axis_index_map_offset_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn axis_index_map(&self) -> Result<Option<DeltaSetIndexMap<'a>>, ReadError> {
        let data = self.data;
        let Ok(read_axis_index_map_offset) = self.axis_index_map_offset();

        match read_axis_index_map_offset { Some(x) => x.resolve(data), None => Ok(None) }
    }

    pub fn var_store_offset(&self) -> Result<Option<Nullable<Offset32>>, Never> {
        let Ok(range) = self.var_store_offset_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn var_store(&self) -> Result<Option<ItemVariationStore<'a>>, ReadError> {
        let data = self.data;
        let Ok(read_var_store_offset) = self.var_store_offset();

        match read_var_store_offset { Some(x) => x.resolve(data), None => Ok(None) }
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(MajorMinor::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn _reserved_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn axis_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self._reserved_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn axis_segment_maps_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(axis_count) = self.axis_count();
        let Ok(previous) = self.axis_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add({ let Ok(explicit_value) = transforms::total_len_for_count::<SegmentMaps, _>(self.data, start, axis_count); explicit_value });
        Ok(start..end)
    }

    pub fn axis_index_map_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.axis_segment_maps_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility((2u16, 0u16));

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(Offset32::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }

    pub fn var_store_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.axis_index_map_offset_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility((2u16, 0u16));

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(Offset32::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Avar::MIN_SIZE); held }));

impl Default for Avar<'_> {
    fn default() -> Self {
        let Ok(value_5) = FontData::default_table_data();

        Self {
            data: value_5,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Avar<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Avar")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "axis_count", value: FieldType::from(self.axis_count()) }),
            2u64 => {
                let Ok(value_6) = self.offset_data();

                let Ok(axis_segment_maps) = self.axis_segment_maps();

                Some(Field { name: "axis_segment_maps", value: FieldType::from(traversal::FieldType::var_array(
                    "SegmentMaps",
                    axis_segment_maps,
                    value_6,
                )) })
            },
            3u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility((2u16, 0u16));
                let Ok(axis_index_map_offset) = self.axis_index_map_offset();

                match compatibility {
                    Compatibility::Compatible => axis_index_map_offset.map(|axis_index_map_offset_2| Field { name: "axis_index_map_offset", value: FieldType::from(FieldType::offset(axis_index_map_offset_2, self.axis_index_map())) }),
                    Compatibility::Incompatible => None,
                }
            },
            4u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility((2u16, 0u16));
                let Ok(var_store_offset) = self.var_store_offset();

                match compatibility {
                    Compatibility::Compatible => var_store_offset.map(|var_store_offset_2| Field { name: "var_store_offset", value: FieldType::from(FieldType::offset(var_store_offset_2, self.var_store())) }),
                    Compatibility::Incompatible => None,
                }
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Avar<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SegmentMaps<'a> {
    pub position_map_count: BigEndian<u16>,
    pub axis_value_maps: &'a [AxisValueMap],
}

impl<'a> SegmentMaps<'a> {
    pub fn position_map_count(&self) -> Result<u16, Never> {
        self.position_map_count.get()
    }

    pub fn axis_value_maps(&self) -> Result<&'a [AxisValueMap], Never> {
        Ok(self.axis_value_maps)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for SegmentMaps<'a> {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "SegmentMaps",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "position_map_count", value: FieldType::from(self.position_map_count()) }),
                1u64 => Some(Field { name: "axis_value_maps", value: FieldType::from(traversal::FieldType::array_of_records(
                        stringify!(AxisValueMap),
                        { let Ok(explicit_value) = self.axis_value_maps(); explicit_value },
                        _data,
                    )) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct AxisValueMap {
    pub from_coordinate: BigEndian<F2Dot14>,
    pub to_coordinate: BigEndian<F2Dot14>,
}

const _: () = {
  assert!(core::mem::size_of::<AxisValueMap>() == 4);
  assert!(core::mem::align_of::<AxisValueMap>() == 1);
  assert!(core::mem::offset_of!(AxisValueMap, from_coordinate) == 0);
  assert!(core::mem::offset_of!(AxisValueMap, to_coordinate) == 2);
};

impl AxisValueMap {
    pub fn from_coordinate(&self) -> Result<F2Dot14, Never> {
        self.from_coordinate.get()
    }

    pub fn to_coordinate(&self) -> Result<F2Dot14, Never> {
        self.to_coordinate.get()
    }
}

impl FixedSize for AxisValueMap {
    const RAW_BYTE_LEN: u64 = F2Dot14::RAW_BYTE_LEN + F2Dot14::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for AxisValueMap {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "AxisValueMap",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "from_coordinate", value: FieldType::from(self.from_coordinate()) }),
                1u64 => Some(Field { name: "to_coordinate", value: FieldType::from(self.to_coordinate()) }),
                _ => None,
            }),
            data,
        })
    }
}
