use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for BasicTable<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(array_records_byte_range) = self.array_records_byte_range();

        Ok(0..array_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for BasicTable<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct BasicTable<'a> {
    data: FontData<'a>,
}

impl<'a> BasicTable<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn simple_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.simple_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn simple_records(&self) -> Result<&'a [SimpleRecord], Never> {
        let Ok(range) = self.simple_records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn arrays_inner_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.arrays_inner_count_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn array_records_count(&self) -> Result<u32, Never> {
        let Ok(range) = self.array_records_count_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn array_records(&self) -> Result<ComputedArray<'a, ContainsArrays<'a>>, Never> {
        let Ok(range) = self.array_records_byte_range();

        self.data.read_with_args_or_default(range, &({ let Ok(explicit_value) = self.arrays_inner_count(); explicit_value }))
    }

    pub fn simple_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn simple_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(simple_count) = self.simple_count();
        let Ok(previous) = self.simple_count_byte_range();
        let start = previous.end;
        let Ok(value_2) = transforms::to_u64(simple_count);

        let end =
            start.saturating_add(value_2.saturating_mul(SimpleRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn arrays_inner_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.simple_records_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn array_records_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.arrays_inner_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn array_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(array_records_count) = self.array_records_count();
        let Ok(previous) = self.array_records_count_byte_range();
        let start = previous.end;
        let Ok(value_3) = transforms::to_u64(array_records_count);

        let Ok(arrays_inner_count) = self.arrays_inner_count();
        let Ok(size_or_zero) = transforms::size_or_zero(<ContainsArrays as ComputeSize>::compute_size(&arrays_inner_count));
        let end = start.saturating_add(value_3.saturating_mul(
                size_or_zero,
            ));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(BasicTable::MIN_SIZE); held }));

impl Default for BasicTable<'_> {
    fn default() -> Self {
        let Ok(value_4) = FontData::default_table_data();

        Self {
            data: value_4,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for BasicTable<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("BasicTable")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "simple_count", value: FieldType::from(self.simple_count()) }),
            1u64 => {
                let Ok(value_5) = self.offset_data();

                Some(Field { name: "simple_records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(SimpleRecord),
                    { let Ok(explicit_value) = self.simple_records(); explicit_value },
                    value_5,
                )) })
            },
            2u64 => Some(Field { name: "arrays_inner_count", value: FieldType::from(self.arrays_inner_count()) }),
            3u64 => Some(Field { name: "array_records_count", value: FieldType::from(self.array_records_count()) }),
            4u64 => {
                let Ok(value_6) = self.offset_data();

                let Ok(array_records) = self.array_records();

                Some(Field { name: "array_records", value: FieldType::from(traversal::FieldType::computed_array(
                    "ContainsArrays",
                    array_records,
                    value_6,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for BasicTable<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct SimpleRecord {
    pub val1: BigEndian<u16>,
    pub va2: BigEndian<u32>,
}

const _: () = {
  assert!(core::mem::size_of::<SimpleRecord>() == 6);
  assert!(core::mem::align_of::<SimpleRecord>() == 1);
  assert!(core::mem::offset_of!(SimpleRecord, val1) == 0);
  assert!(core::mem::offset_of!(SimpleRecord, va2) == 2);
};

impl SimpleRecord {
    pub fn val1(&self) -> Result<u16, Never> {
        self.val1.get()
    }

    pub fn va2(&self) -> Result<u32, Never> {
        self.va2.get()
    }
}

impl FixedSize for SimpleRecord {
    const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN + u32::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for SimpleRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "SimpleRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "val1", value: FieldType::from(self.val1()) }),
                1u64 => Some(Field { name: "va2", value: FieldType::from(self.va2()) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContainsArrays<'a> {
    pub scalars: &'a [BigEndian<u16>],
    pub records: &'a [SimpleRecord],
}

impl<'a> ContainsArrays<'a> {
    pub fn scalars(&self) -> Result<&'a [BigEndian<u16>], Never> {
        Ok(self.scalars)
    }

    pub fn records(&self) -> Result<&'a [SimpleRecord], Never> {
        Ok(self.records)
    }
}

impl ReadArgs for ContainsArrays<'_> {
    type Arguments = u16;
}

impl ComputeSize for ContainsArrays<'_> {
  fn compute_size(args: &u16) -> Result<u64, ReadError> {
        let array_len = *args;
        let mut result = 0u64;
        let Ok(value_9) = transforms::to_u64(array_len);

    let checked_value = result
            .checked_add(value_9.saturating_mul(u16::RAW_BYTE_LEN))
            .ok_or(ReadError::OutOfBounds)?;
    result = checked_value;
        let Ok(value_10) = transforms::to_u64(array_len);

    let checked_value_2 = result
            .checked_add(
                value_10.saturating_mul(SimpleRecord::RAW_BYTE_LEN),
            )
            .ok_or(ReadError::OutOfBounds)?;
    result = checked_value_2;
        Ok(result)
    }
}

impl<'a> FontReadWithArgs<'a> for ContainsArrays<'a> {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let Ok(value_11) = data.cursor();

        let mut cursor = value_11;
        let array_len = *args;
        let Ok(value_12) = transforms::to_u64(array_len);
        let Ok(value_13) = transforms::to_u64(array_len);

        let scalars = cursor.read_array(value_12)?;

        let records = cursor.read_array(value_13)?;


        Ok(Self {

            scalars,

            records,

        })
    }
}

impl<'a> ContainsArrays<'a> {
    pub fn read(data: FontData<'a>, array_len: u16) -> Result<Self, ReadError> {
        let args = array_len;
        Self::read_with_args(data, &args)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for ContainsArrays<'a> {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "ContainsArrays",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "scalars", value: FieldType::from(self.scalars()) }),
                1u64 => Some(Field { name: "records", value: FieldType::from(traversal::FieldType::array_of_records(
                        stringify!(SimpleRecord),
                        { let Ok(explicit_value) = self.records(); explicit_value },
                        _data,
                    )) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Debug, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct ContainsOffsets {
    pub off_array_count: BigEndian<u16>,
    pub array_offset: BigEndian<Offset16>,
    pub other_offset: BigEndian<Offset32>,
}

const _: () = {
  assert!(core::mem::size_of::<ContainsOffsets>() == 8);
  assert!(core::mem::align_of::<ContainsOffsets>() == 1);
  assert!(core::mem::offset_of!(ContainsOffsets, off_array_count) == 0);
  assert!(core::mem::offset_of!(ContainsOffsets, array_offset) == 2);
  assert!(core::mem::offset_of!(ContainsOffsets, other_offset) == 4);
};

impl ContainsOffsets {
    pub fn off_array_count(&self) -> Result<u16, Never> {
        self.off_array_count.get()
    }

    pub fn array_offset(&self) -> Result<Offset16, Never> {
        self.array_offset.get()
    }

    pub fn array<'a>(&self, data: FontData<'a>) -> Result<&'a [SimpleRecord], ReadError> {
        let Ok(args) = self.off_array_count();
        let Ok(array_offset) = self.array_offset();

        array_offset.resolve_with_args(data, &args)
    }

    pub fn other_offset(&self) -> Result<Offset32, Never> {
        self.other_offset.get()
    }

    pub fn other<'a>(&self, data: FontData<'a>) -> Result<BasicTable<'a>, ReadError> {
        let Ok(other_offset) = self.other_offset();

        other_offset.resolve(data)
    }
}

impl FixedSize for ContainsOffsets {
    const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN + Offset16::RAW_BYTE_LEN + Offset32::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for ContainsOffsets {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "ContainsOffsets",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "off_array_count", value: FieldType::from(self.off_array_count()) }),
                1u64 => Some(Field { name: "array_offset", value: FieldType::from(traversal::FieldType::offset_to_array_of_records(
                        self.array_offset(),
                        (self.array(_data)).map(Some),
                        stringify!(SimpleRecord),
                        _data,
                    )) }),
                2u64 => Some(Field { name: "other_offset", value: FieldType::from(FieldType::offset(self.other_offset(), (self.other(_data)).map(Some))) }),
                _ => None,
            }),
            data,
        })
    }
}

impl<'a> MinByteRange<'a> for VarLenItem<'a> {
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

impl<'a> FontRead<'a> for VarLenItem<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_17) = data.len();

        match ::core::primitive::bool::then_some(value_17 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct VarLenItem<'a> {
    data: FontData<'a>,
}

impl<'a> VarLenItem<'a> {
    pub const MIN_SIZE: u64 = u32::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn length(&self) -> Result<u32, Never> {
        let Ok(range) = self.length_byte_range();

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

    pub fn data_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.length_byte_range();
        let start = previous.end;
        let Ok(length) = self.data.len();
        let end =
            start.saturating_add({ let Ok(explicit_value) = transforms::whole_records(transforms::RecordExtent { bytes: length.saturating_sub(start), item_width: u8::RAW_BYTE_LEN }); explicit_value });
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(VarLenItem::MIN_SIZE); held }));

impl Default for VarLenItem<'_> {
    fn default() -> Self {
        let Ok(value_18) = FontData::default_table_data();

        Self {
            data: value_18,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for VarLenItem<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("VarLenItem")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "length", value: FieldType::from(self.length()) }),
            1u64 => Some(Field { name: "data", value: FieldType::from(self.data()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for VarLenItem<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
