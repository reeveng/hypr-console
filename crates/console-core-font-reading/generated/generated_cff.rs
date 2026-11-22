

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for CffHeader<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(trailing_data_byte_range) = self.trailing_data_byte_range();

        Ok(0..trailing_data_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for CffHeader<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct CffHeader<'a> {
    data: FontData<'a>,
}

impl<'a> CffHeader<'a> {
    pub const MIN_SIZE: u64 =
        (u8::RAW_BYTE_LEN + u8::RAW_BYTE_LEN + u8::RAW_BYTE_LEN + u8::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn major(&self) -> Result<u8, Never> {
        let Ok(range) = self.major_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn minor(&self) -> Result<u8, Never> {
        let Ok(range) = self.minor_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn hdr_size(&self) -> Result<u8, Never> {
        let Ok(range) = self.hdr_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn off_size(&self) -> Result<u8, Never> {
        let Ok(range) = self.off_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn _padding(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self._padding_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn trailing_data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.trailing_data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn major_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn minor_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.major_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn hdr_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.minor_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn off_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.hdr_size_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn _padding_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(hdr_size) = self.hdr_size();
        let Ok(previous) = self.off_size_byte_range();
        let start = previous.end;
        let Ok(value_2) = transforms::subtract(hdr_size, 4_u64);

        let end =
            start.saturating_add(value_2.saturating_mul(u8::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn trailing_data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self._padding_byte_range();
        let start = previous.end;
        let Ok(value_7) = self.data.len();

        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_7.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(CffHeader::MIN_SIZE); held }));

impl Default for CffHeader<'_> {
    fn default() -> Self {
        let Ok(value_3) = FontData::default_table_data();

        Self {
            data: value_3,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for CffHeader<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("CffHeader")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "major", value: FieldType::from(self.major()) }),
            1u64 => Some(Field { name: "minor", value: FieldType::from(self.minor()) }),
            2u64 => Some(Field { name: "hdr_size", value: FieldType::from(self.hdr_size()) }),
            3u64 => Some(Field { name: "off_size", value: FieldType::from(self.off_size()) }),
            4u64 => Some(Field { name: "_padding", value: FieldType::from(self._padding()) }),
            5u64 => Some(Field { name: "trailing_data", value: FieldType::from(self.trailing_data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for CffHeader<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for Index<'a> {
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

impl<'a> FontRead<'a> for Index<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_4) = data.len();

        match ::core::primitive::bool::then_some(value_4 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Index<'a> {
    data: FontData<'a>,
}

impl<'a> Index<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u8::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn count(&self) -> Result<u16, Never> {
        let Ok(range) = self.count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn off_size(&self) -> Result<u8, Never> {
        let Ok(range) = self.off_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn offsets(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn data(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.data_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn off_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(count) = self.count();
        let Ok(off_size) = self.off_size();
        let Ok(previous) = self.off_size_byte_range();
        let start = previous.end;
        let Ok(value_5) = transforms::add_multiply(count, 1_u64, off_size);

        let end = start.saturating_add(value_5.saturating_mul(u8::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.offsets_byte_range();
        let start = previous.end;
        let Ok(value_8) = self.data.len();

        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_8.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Index::MIN_SIZE); held }));

impl Default for Index<'_> {
    fn default() -> Self {
        let Ok(value_6) = FontData::default_table_data();

        Self {
            data: value_6,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Index<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Index")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "count", value: FieldType::from(self.count()) }),
            1u64 => Some(Field { name: "off_size", value: FieldType::from(self.off_size()) }),
            2u64 => Some(Field { name: "offsets", value: FieldType::from(self.offsets()) }),
            3u64 => Some(Field { name: "data", value: FieldType::from(self.data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Index<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone)]
pub enum FdSelect<'a> {
    Format0(FdSelectFormat0<'a>),
    Format3(FdSelectFormat3<'a>),
    Format4(FdSelectFormat4<'a>),
}

impl Default for FdSelect<'_> {
    fn default() -> Self {
        Self::Format0(Default::default())
    }
}

impl<'a> FdSelect<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Format0(item) => item.offset_data(),
            Self::Format3(item) => item.offset_data(),
            Self::Format4(item) => item.offset_data(),
        }
    }

    pub fn format(&self) -> Result<u8, Never> {
        match self {
            Self::Format0(item) => item.format(),
            Self::Format3(item) => item.format(),
            Self::Format4(item) => item.format(),
        }
    }
}

impl<'a> FontRead<'a> for FdSelect<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u8 = data.read_at(0u64)?;

        match format {
            FdSelectFormat0::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format0(read))
            }
            FdSelectFormat3::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format3(read))
            }
            FdSelectFormat4::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format4(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u8>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for FdSelect<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        match self {
            Self::Format0(item) => item.min_byte_range(),
            Self::Format3(item) => item.min_byte_range(),
            Self::Format4(item) => item.min_byte_range(),
        }
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        match self {
            Self::Format0(item) => item.min_table_bytes(),
            Self::Format3(item) => item.min_table_bytes(),
            Self::Format4(item) => item.min_table_bytes(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> FdSelect<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Format0(table) => table,
            Self::Format3(table) => table,
            Self::Format4(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for FdSelect<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for FdSelect<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}

impl Format<u8> for FdSelectFormat0<'_> {
    const FORMAT: u8 = 0;
}

impl<'a> MinByteRange<'a> for FdSelectFormat0<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(fds_byte_range) = self.fds_byte_range();

        Ok(0..fds_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for FdSelectFormat0<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_10) = data.len();

        match ::core::primitive::bool::then_some(value_10 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct FdSelectFormat0<'a> {
    data: FontData<'a>,
}

impl<'a> FdSelectFormat0<'a> {
    pub const MIN_SIZE: u64 = u8::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn fds(&self) -> Result<&'a [u8], Never> {
        let Ok(range) = self.fds_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn fds_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let Ok(value_42) = self.data.len();

        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_42.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    FdSelectFormat0::MIN_SIZE
); held }));

impl Default for FdSelectFormat0<'_> {
    fn default() -> Self {
        let Ok(value_11) = FontData::default_table_data();

        Self {
            data: value_11,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for FdSelectFormat0<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("FdSelectFormat0")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "fds", value: FieldType::from(self.fds()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for FdSelectFormat0<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for FdSelectFormat3<'_> {
    const FORMAT: u8 = 3;
}

impl<'a> MinByteRange<'a> for FdSelectFormat3<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(sentinel_byte_range) = self.sentinel_byte_range();

        Ok(0..sentinel_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for FdSelectFormat3<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_12) = data.len();

        match ::core::primitive::bool::then_some(value_12 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct FdSelectFormat3<'a> {
    data: FontData<'a>,
}

impl<'a> FdSelectFormat3<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn n_ranges(&self) -> Result<u16, Never> {
        let Ok(range) = self.n_ranges_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn ranges(&self) -> Result<&'a [FdSelectRange3], Never> {
        let Ok(range) = self.ranges_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn sentinel(&self) -> Result<u16, Never> {
        let Ok(range) = self.sentinel_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn n_ranges_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn ranges_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(n_ranges) = self.n_ranges();
        let Ok(previous) = self.n_ranges_byte_range();
        let start = previous.end;
        let Ok(value_13) = transforms::to_u64(n_ranges);

        let end =
            start.saturating_add(value_13.saturating_mul(FdSelectRange3::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn sentinel_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.ranges_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for FdSelectFormat3<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("FdSelectFormat3")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "n_ranges", value: FieldType::from(self.n_ranges()) }),
            2u64 => {
                let Ok(value_14) = self.offset_data();

                Some(Field { name: "ranges", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(FdSelectRange3),
                    { let Ok(explicit_value) = self.ranges(); explicit_value },
                    value_14,
                )) })
            },
            3u64 => Some(Field { name: "sentinel", value: FieldType::from(self.sentinel()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for FdSelectFormat3<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct FdSelectRange3 {
    pub first: BigEndian<u16>,
    pub fd: u8,
}

const _: () = {
  assert!(core::mem::size_of::<FdSelectRange3>() == 3);
  assert!(core::mem::align_of::<FdSelectRange3>() == 1);
  assert!(core::mem::offset_of!(FdSelectRange3, first) == 0);
  assert!(core::mem::offset_of!(FdSelectRange3, fd) == 2);
};

impl FdSelectRange3 {
    pub fn first(&self) -> Result<u16, Never> {
        self.first.get()
    }

    pub fn fd(&self) -> Result<u8, Never> {
        Ok(self.fd)
    }
}

impl FixedSize for FdSelectRange3 {
    const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN + u8::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for FdSelectRange3 {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "FdSelectRange3",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "first", value: FieldType::from(self.first()) }),
                1u64 => Some(Field { name: "fd", value: FieldType::from(self.fd()) }),
                _ => None,
            }),
            data,
        })
    }
}

impl Format<u8> for FdSelectFormat4<'_> {
    const FORMAT: u8 = 4;
}

impl<'a> MinByteRange<'a> for FdSelectFormat4<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(sentinel_byte_range) = self.sentinel_byte_range();

        Ok(0..sentinel_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for FdSelectFormat4<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_16) = data.len();

        match ::core::primitive::bool::then_some(value_16 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct FdSelectFormat4<'a> {
    data: FontData<'a>,
}

impl<'a> FdSelectFormat4<'a> {
    pub const MIN_SIZE: u64 = (u8::RAW_BYTE_LEN + u32::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn n_ranges(&self) -> Result<u32, Never> {
        let Ok(range) = self.n_ranges_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn ranges(&self) -> Result<&'a [FdSelectRange4], Never> {
        let Ok(range) = self.ranges_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn sentinel(&self) -> Result<u32, Never> {
        let Ok(range) = self.sentinel_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn n_ranges_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn ranges_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(n_ranges) = self.n_ranges();
        let Ok(previous) = self.n_ranges_byte_range();
        let start = previous.end;
        let Ok(value_17) = transforms::to_u64(n_ranges);

        let end =
            start.saturating_add(value_17.saturating_mul(FdSelectRange4::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn sentinel_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.ranges_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for FdSelectFormat4<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("FdSelectFormat4")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "n_ranges", value: FieldType::from(self.n_ranges()) }),
            2u64 => {
                let Ok(value_18) = self.offset_data();

                Some(Field { name: "ranges", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(FdSelectRange4),
                    { let Ok(explicit_value) = self.ranges(); explicit_value },
                    value_18,
                )) })
            },
            3u64 => Some(Field { name: "sentinel", value: FieldType::from(self.sentinel()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for FdSelectFormat4<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct FdSelectRange4 {
    pub first: BigEndian<u32>,
    pub fd: BigEndian<u16>,
}

const _: () = {
  assert!(core::mem::size_of::<FdSelectRange4>() == 6);
  assert!(core::mem::align_of::<FdSelectRange4>() == 1);
  assert!(core::mem::offset_of!(FdSelectRange4, first) == 0);
  assert!(core::mem::offset_of!(FdSelectRange4, fd) == 4);
};

impl FdSelectRange4 {
    pub fn first(&self) -> Result<u32, Never> {
        self.first.get()
    }

    pub fn fd(&self) -> Result<u16, Never> {
        self.fd.get()
    }
}

impl FixedSize for FdSelectRange4 {
    const RAW_BYTE_LEN: u64 = u32::RAW_BYTE_LEN + u16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for FdSelectRange4 {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "FdSelectRange4",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "first", value: FieldType::from(self.first()) }),
                1u64 => Some(Field { name: "fd", value: FieldType::from(self.fd()) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone)]
pub enum CustomCharset<'a> {
    Format0(CharsetFormat0<'a>),
    Format1(CharsetFormat1<'a>),
    Format2(CharsetFormat2<'a>),
}

impl Default for CustomCharset<'_> {
    fn default() -> Self {
        Self::Format0(Default::default())
    }
}

impl<'a> CustomCharset<'a> {
    pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
        match self {
            Self::Format0(item) => item.offset_data(),
            Self::Format1(item) => item.offset_data(),
            Self::Format2(item) => item.offset_data(),
        }
    }

    pub fn format(&self) -> Result<u8, Never> {
        match self {
            Self::Format0(item) => item.format(),
            Self::Format1(item) => item.format(),
            Self::Format2(item) => item.format(),
        }
    }
}

impl<'a> FontRead<'a> for CustomCharset<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let format: u8 = data.read_at(0u64)?;

        match format {
            CharsetFormat0::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format0(read))
            }
            CharsetFormat1::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format1(read))
            }
            CharsetFormat2::FORMAT => {
                let read = FontRead::read(data)?;

                Ok(Self::Format2(read))
            }
            other => Err(ReadError::InvalidFormat(<::core::primitive::i64 as ::core::convert::From<::core::primitive::u8>>::from(other))),
        }
    }
}

impl<'a> MinByteRange<'a> for CustomCharset<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        match self {
            Self::Format0(item) => item.min_byte_range(),
            Self::Format1(item) => item.min_byte_range(),
            Self::Format2(item) => item.min_byte_range(),
        }
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        match self {
            Self::Format0(item) => item.min_table_bytes(),
            Self::Format1(item) => item.min_table_bytes(),
            Self::Format2(item) => item.min_table_bytes(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> CustomCharset<'a> {
    fn dyn_inner<'b>(&'b self) -> Result<&'b dyn SomeTable<'a>, Never> {
        Ok(match self {
            Self::Format0(table) => table,
            Self::Format1(table) => table,
            Self::Format2(table) => table,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl std::fmt::Debug for CustomCharset<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dyn_inner().fmt(f)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for CustomCharset<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.type_name()
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        let Ok(inner) = self.dyn_inner();

        inner.get_field(idx)
    }
}

impl Format<u8> for CharsetFormat0<'_> {
    const FORMAT: u8 = 0;
}

impl<'a> MinByteRange<'a> for CharsetFormat0<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(glyph_byte_range) = self.glyph_byte_range();

        Ok(0..glyph_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for CharsetFormat0<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_24) = data.len();

        match ::core::primitive::bool::then_some(value_24 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct CharsetFormat0<'a> {
    data: FontData<'a>,
}

impl<'a> CharsetFormat0<'a> {
    pub const MIN_SIZE: u64 = u8::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn glyph(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.glyph_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn glyph_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let Ok(value_48) = self.data.len();

        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_48.saturating_sub(start), item_width: u16::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(CharsetFormat0::MIN_SIZE); held }));

impl Default for CharsetFormat0<'_> {
    fn default() -> Self {
        let Ok(value_25) = FontData::default_table_data();

        Self {
            data: value_25,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for CharsetFormat0<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("CharsetFormat0")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => Some(Field { name: "glyph", value: FieldType::from(self.glyph()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for CharsetFormat0<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl Format<u8> for CharsetFormat1<'_> {
    const FORMAT: u8 = 1;
}

impl<'a> MinByteRange<'a> for CharsetFormat1<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(ranges_byte_range) = self.ranges_byte_range();

        Ok(0..ranges_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for CharsetFormat1<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_26) = data.len();

        match ::core::primitive::bool::then_some(value_26 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct CharsetFormat1<'a> {
    data: FontData<'a>,
}

impl<'a> CharsetFormat1<'a> {
    pub const MIN_SIZE: u64 = u8::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn ranges(&self) -> Result<&'a [CharsetRange1], Never> {
        let Ok(range) = self.ranges_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn ranges_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let Ok(value_49) = self.data.len();

        let end = start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_49.saturating_sub(start), item_width: CharsetRange1::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(CharsetFormat1::MIN_SIZE); held }));

impl Default for CharsetFormat1<'_> {
    fn default() -> Self {
        let Ok(value_27) = FontData::default_format_1_u8_table_data();

        Self {
            data: value_27,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for CharsetFormat1<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("CharsetFormat1")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => {
                let Ok(value_28) = self.offset_data();

                Some(Field { name: "ranges", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(CharsetRange1),
                    { let Ok(explicit_value) = self.ranges(); explicit_value },
                    value_28,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for CharsetFormat1<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct CharsetRange1 {
    pub first: BigEndian<u16>,
    pub n_left: u8,
}

const _: () = {
  assert!(core::mem::size_of::<CharsetRange1>() == 3);
  assert!(core::mem::align_of::<CharsetRange1>() == 1);
  assert!(core::mem::offset_of!(CharsetRange1, first) == 0);
  assert!(core::mem::offset_of!(CharsetRange1, n_left) == 2);
};

impl CharsetRange1 {
    pub fn first(&self) -> Result<u16, Never> {
        self.first.get()
    }

    pub fn n_left(&self) -> Result<u8, Never> {
        Ok(self.n_left)
    }
}

impl FixedSize for CharsetRange1 {
    const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN + u8::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for CharsetRange1 {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "CharsetRange1",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "first", value: FieldType::from(self.first()) }),
                1u64 => Some(Field { name: "n_left", value: FieldType::from(self.n_left()) }),
                _ => None,
            }),
            data,
        })
    }
}

impl Format<u8> for CharsetFormat2<'_> {
    const FORMAT: u8 = 2;
}

impl<'a> MinByteRange<'a> for CharsetFormat2<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(ranges_byte_range) = self.ranges_byte_range();

        Ok(0..ranges_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for CharsetFormat2<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_30) = data.len();

        match ::core::primitive::bool::then_some(value_30 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct CharsetFormat2<'a> {
    data: FontData<'a>,
}

impl<'a> CharsetFormat2<'a> {
    pub const MIN_SIZE: u64 = u8::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn format(&self) -> Result<u8, Never> {
        let Ok(range) = self.format_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn ranges(&self) -> Result<&'a [CharsetRange2], Never> {
        let Ok(range) = self.ranges_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn format_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u8::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn ranges_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.format_byte_range();
        let start = previous.end;
        let Ok(value_50) = self.data.len();

        let end = start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: value_50.saturating_sub(start), item_width: CharsetRange2::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for CharsetFormat2<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("CharsetFormat2")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "format", value: FieldType::from(self.format()) }),
            1u64 => {
                let Ok(value_31) = self.offset_data();

                Some(Field { name: "ranges", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(CharsetRange2),
                    { let Ok(explicit_value) = self.ranges(); explicit_value },
                    value_31,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for CharsetFormat2<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct CharsetRange2 {
    pub first: BigEndian<u16>,
    pub n_left: BigEndian<u16>,
}

const _: () = {
  assert!(core::mem::size_of::<CharsetRange2>() == 4);
  assert!(core::mem::align_of::<CharsetRange2>() == 1);
  assert!(core::mem::offset_of!(CharsetRange2, first) == 0);
  assert!(core::mem::offset_of!(CharsetRange2, n_left) == 2);
};

impl CharsetRange2 {
    pub fn first(&self) -> Result<u16, Never> {
        self.first.get()
    }

    pub fn n_left(&self) -> Result<u16, Never> {
        self.n_left.get()
    }
}

impl FixedSize for CharsetRange2 {
    const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for CharsetRange2 {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "CharsetRange2",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "first", value: FieldType::from(self.first()) }),
                1u64 => Some(Field { name: "n_left", value: FieldType::from(self.n_left()) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct EncodingRange1 {
    pub first: u8,
    pub n_left: u8,
}

const _: () = {
  assert!(core::mem::size_of::<EncodingRange1>() == 2);
  assert!(core::mem::align_of::<EncodingRange1>() == 1);
  assert!(core::mem::offset_of!(EncodingRange1, first) == 0);
  assert!(core::mem::offset_of!(EncodingRange1, n_left) == 1);
};

impl EncodingRange1 {
    pub fn first(&self) -> Result<u8, Never> {
        Ok(self.first)
    }

    pub fn n_left(&self) -> Result<u8, Never> {
        Ok(self.n_left)
    }
}

impl FixedSize for EncodingRange1 {
    const RAW_BYTE_LEN: u64 = u8::RAW_BYTE_LEN + u8::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for EncodingRange1 {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "EncodingRange1",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "first", value: FieldType::from(self.first()) }),
                1u64 => Some(Field { name: "n_left", value: FieldType::from(self.n_left()) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct EncodingSupplement {
    pub code: u8,
    pub glyph: BigEndian<u16>,
}

const _: () = {
  assert!(core::mem::size_of::<EncodingSupplement>() == 3);
  assert!(core::mem::align_of::<EncodingSupplement>() == 1);
  assert!(core::mem::offset_of!(EncodingSupplement, code) == 0);
  assert!(core::mem::offset_of!(EncodingSupplement, glyph) == 1);
};

impl EncodingSupplement {
    pub fn code(&self) -> Result<u8, Never> {
        Ok(self.code)
    }

    pub fn glyph(&self) -> Result<u16, Never> {
        self.glyph.get()
    }
}

impl FixedSize for EncodingSupplement {
    const RAW_BYTE_LEN: u64 = u8::RAW_BYTE_LEN + u16::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for EncodingSupplement {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "EncodingSupplement",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "code", value: FieldType::from(self.code()) }),
                1u64 => Some(Field { name: "glyph", value: FieldType::from(self.glyph()) }),
                _ => None,
            }),
            data,
        })
    }
}
