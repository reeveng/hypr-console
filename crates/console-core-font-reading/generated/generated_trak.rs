

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Trak<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(reserved_byte_range) = self.reserved_byte_range();

        Ok(0..reserved_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Trak<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"trak"); value });
}

impl<'a> FontRead<'a> for Trak<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Trak<'a> {
    data: FontData<'a>,
}

impl<'a> Trak<'a> {
    pub const MIN_SIZE: u64 = (MajorMinor::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<MajorMinor, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn format(&self) -> Result<u16, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn horiz_offset(&self) -> Result<Nullable<Offset16>, Never> {
        let Ok(range) = self.horiz_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn horiz(&self) -> Result<Option<TrackData<'a>>, ReadError> {
        let data = self.data;
        let Ok(horiz_offset) = self.horiz_offset();

        horiz_offset.resolve(data)
    }

    pub fn vert_offset(&self) -> Result<Nullable<Offset16>, Never> {
        let Ok(range) = self.vert_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn vert(&self) -> Result<Option<TrackData<'a>>, ReadError> {
        let data = self.data;
        let Ok(vert_offset) = self.vert_offset();

        vert_offset.resolve(data)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(MajorMinor::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn horiz_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn vert_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.horiz_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn reserved_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.vert_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Trak::MIN_SIZE); held }));

impl Default for Trak<'_> {
    fn default() -> Self {
        let Ok(value_2) = FontData::default_table_data();

        Self {
            data: value_2,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Trak<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Trak")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            2u64 => Some(Field { name: "horiz_offset", value: FieldType::from(FieldType::offset(self.horiz_offset(), self.horiz())) }),
            3u64 => Some(Field { name: "vert_offset", value: FieldType::from(FieldType::offset(self.vert_offset(), self.vert())) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Trak<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for TrackData<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(track_table_byte_range) = self.track_table_byte_range();

        Ok(0..track_table_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for TrackData<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_3) = data.len();

        match ::core::primitive::bool::then_some(value_3 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct TrackData<'a> {
    data: FontData<'a>,
}

impl<'a> TrackData<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn n_tracks(&self) -> Result<u16, Never> {
        let Ok(range) = self.n_tracks_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn n_sizes(&self) -> Result<u16, Never> {
        let Ok(range) = self.n_sizes_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn size_table_offset(&self) -> Result<u32, Never> {
        let Ok(range) = self.size_table_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn track_table(&self) -> Result<&'a [TrackTableEntry], Never> {
        let Ok(range) = self.track_table_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn n_tracks_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn n_sizes_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.n_tracks_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn size_table_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.n_sizes_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn track_table_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(n_tracks) = self.n_tracks();
        let Ok(previous) = self.size_table_offset_byte_range();
        let start = previous.end;
        let Ok(value_4) = transforms::to_u64(n_tracks);

        let end =
            start.saturating_add(value_4.saturating_mul(TrackTableEntry::RAW_BYTE_LEN));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(TrackData::MIN_SIZE); held }));

impl Default for TrackData<'_> {
    fn default() -> Self {
        let Ok(value_5) = FontData::default_table_data();

        Self {
            data: value_5,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for TrackData<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("TrackData")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "n_tracks", value: FieldType::from(self.n_tracks()) }),
            1u64 => Some(Field { name: "n_sizes", value: FieldType::from(self.n_sizes()) }),
            2u64 => Some(Field { name: "size_table_offset", value: FieldType::from(self.size_table_offset()) }),
            3u64 => {
                let Ok(value_6) = self.offset_data();

                Some(Field { name: "track_table", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(TrackTableEntry),
                    { let Ok(explicit_value) = self.track_table(); explicit_value },
                    value_6,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for TrackData<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct TrackTableEntry {
    pub track: BigEndian<Fixed>,
    pub name_index: BigEndian<NameId>,
    pub offset: BigEndian<u16>,
}

const _: () = {
  assert!(core::mem::size_of::<TrackTableEntry>() == 8);
  assert!(core::mem::align_of::<TrackTableEntry>() == 1);
  assert!(core::mem::offset_of!(TrackTableEntry, track) == 0);
  assert!(core::mem::offset_of!(TrackTableEntry, name_index) == 4);
  assert!(core::mem::offset_of!(TrackTableEntry, offset) == 6);
};

impl TrackTableEntry {
    pub fn track(&self) -> Result<Fixed, Never> {
        self.track.get()
    }

    pub fn name_index(&self) -> Result<NameId, Never> {
        self.name_index.get()
    }

    pub fn offset(&self) -> Result<u16, Never> {
        self.offset.get()
    }
}

impl FixedSize for TrackTableEntry {
    const RAW_BYTE_LEN: u64 = Fixed::RAW_BYTE_LEN + NameId::RAW_BYTE_LEN + u16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for TrackTableEntry {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "TrackTableEntry",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "track", value: FieldType::from(self.track()) }),
                1u64 => Some(Field { name: "name_index", value: FieldType::from(self.name_index()) }),
                2u64 => Some(Field { name: "offset", value: FieldType::from(self.offset()) }),
                _ => None,
            }),
            data,
        })
    }
}
