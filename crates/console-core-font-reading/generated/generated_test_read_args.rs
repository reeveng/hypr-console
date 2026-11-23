

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for BaseArray<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(face_records_byte_range) = self.face_records_byte_range();

        Ok(0..face_records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl ReadArgs for BaseArray<'_> {
    type Arguments = u16;
}

impl<'a> FontReadWithArgs<'a> for BaseArray<'a> {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let mark_class_count = *args;

        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self {
            data,
            mark_class_count,
        })
    }
}

impl<'a> BaseArray<'a> {
    pub fn read(data: FontData<'a>, mark_class_count: u16) -> Result<Self, ReadError> {
        let args = mark_class_count;
        Self::read_with_args(data, &args)
    }
}

#[derive(Clone)]
pub struct BaseArray<'a> {
    data: FontData<'a>,
    mark_class_count: u16,
}

impl<'a> BaseArray<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn base_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.base_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn base_records(&self) -> Result<ComputedArray<'a, BaseRecord<'a>>, Never> {
        let Ok(range) = self.base_records_byte_range();

        self.data.read_with_args_or_default(range, &({ let Ok(explicit_value) = self.mark_class_count(); explicit_value }))
    }

    pub fn face_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.face_count_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn face_records(&self) -> Result<ComputedArray<'a, FaceRecord<'a>>, Never> {
        let Ok(range) = self.face_records_byte_range();

        self.data.read_with_args_or_default(range, &({ let Ok(explicit_value) = self.mark_class_count(); explicit_value }))
    }

    pub(crate) fn mark_class_count(&self) -> Result<u16, Never> {
        Ok(self.mark_class_count)
    }

    pub fn base_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn base_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(base_count) = self.base_count();
        let Ok(previous) = self.base_count_byte_range();
        let start = previous.end;
        let Ok(value_2) = transforms::to_u64(base_count);

        let Ok(mark_class_count) = self.mark_class_count();
        let Ok(size_or_zero) = transforms::size_or_zero(<BaseRecord as ComputeSize>::compute_size(&mark_class_count));
        let end = start.saturating_add(value_2.saturating_mul(
                size_or_zero,
            ));
        Ok(start..end)
    }

    pub fn face_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.base_records_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn face_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(base_count) = self.base_count();
        let Ok(previous) = self.face_count_byte_range();
        let start = previous.end;
        let Ok(value_3) = transforms::to_u64(base_count);

        let Ok(mark_class_count) = self.mark_class_count();
        let Ok(size_or_zero) = transforms::size_or_zero(<FaceRecord as ComputeSize>::compute_size(&mark_class_count));
        let end = start.saturating_add(value_3.saturating_mul(
                size_or_zero,
            ));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(BaseArray::MIN_SIZE); held }));

impl Default for BaseArray<'_> {
    fn default() -> Self {
        let Ok(value_4) = FontData::default_table_data();

        Self {
            data: value_4,
            mark_class_count: Default::default(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for BaseArray<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("BaseArray")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "base_count", value: FieldType::from(self.base_count()) }),
            1u64 => {
                let Ok(value_5) = self.offset_data();

                let Ok(base_records) = self.base_records();

                Some(Field { name: "base_records", value: FieldType::from(traversal::FieldType::computed_array(
                    "BaseRecord",
                    base_records,
                    value_5,
                )) })
            },
            2u64 => Some(Field { name: "face_count", value: FieldType::from(self.face_count()) }),
            3u64 => {
                let Ok(value_6) = self.offset_data();

                let Ok(face_records) = self.face_records();

                Some(Field { name: "face_records", value: FieldType::from(traversal::FieldType::computed_array(
                    "FaceRecord",
                    face_records,
                    value_6,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for BaseArray<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BaseRecord<'a> {
    pub base_anchor_offsets: &'a [BigEndian<u16>],
}

impl<'a> BaseRecord<'a> {
    pub fn base_anchor_offsets(&self) -> Result<&'a [BigEndian<u16>], Never> {
        Ok(self.base_anchor_offsets)
    }
}

impl ReadArgs for BaseRecord<'_> {
    type Arguments = u16;
}

impl ComputeSize for BaseRecord<'_> {

    fn compute_size(args: &u16) -> Result<u64, ReadError> {
        let mark_class_count = *args;
        let Ok(value_7) = transforms::to_u64(mark_class_count);

        Ok(value_7.saturating_mul(u16::RAW_BYTE_LEN))
    }
}

impl<'a> FontReadWithArgs<'a> for BaseRecord<'a> {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let Ok(value_8) = data.cursor();

        let mut cursor = value_8;
        let mark_class_count = *args;
        let Ok(value_9) = transforms::to_u64(mark_class_count);

        let base_anchor_offsets = cursor.read_array(value_9)?;


        Ok(Self {

            base_anchor_offsets,

        })
    }
}

impl<'a> BaseRecord<'a> {
    pub fn read(data: FontData<'a>, mark_class_count: u16) -> Result<Self, ReadError> {
        let args = mark_class_count;
        Self::read_with_args(data, &args)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for BaseRecord<'a> {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "BaseRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "base_anchor_offsets", value: FieldType::from(self.base_anchor_offsets()) }),
                _ => None,
            }),
            data,
        })
    }
}

#[derive(Clone, Debug)]
pub struct FaceRecord<'a> {
    pub face_offsets: &'a [BigEndian<Nullable<Offset16>>],
}

impl<'a> FaceRecord<'a> {
    pub fn face_offsets(&self) -> Result<&'a [BigEndian<Nullable<Offset16>>], Never> {
        Ok(self.face_offsets)
    }

    pub fn faces(&self, data: FontData<'a>) -> Result<ArrayOfNullableOffsets<'a, Face<'a>, Offset16>, Never> {
        let Ok(offsets) = self.face_offsets();
        ArrayOfNullableOffsets::new(offsets, data, ())
    }
}

impl ReadArgs for FaceRecord<'_> {
    type Arguments = u16;
}

impl ComputeSize for FaceRecord<'_> {

    fn compute_size(args: &u16) -> Result<u64, ReadError> {
        let mark_class_count = *args;
        let Ok(value_10) = transforms::to_u64(mark_class_count);

        Ok(value_10.saturating_mul(Offset16::RAW_BYTE_LEN))
    }
}

impl<'a> FontReadWithArgs<'a> for FaceRecord<'a> {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let Ok(value_11) = data.cursor();

        let mut cursor = value_11;
        let mark_class_count = *args;
        let Ok(value_12) = transforms::to_u64(mark_class_count);

        let face_offsets = cursor.read_array(value_12)?;


        Ok(Self {

            face_offsets,

        })
    }
}

impl<'a> FaceRecord<'a> {
    pub fn read(data: FontData<'a>, mark_class_count: u16) -> Result<Self, ReadError> {
        let args = mark_class_count;
        Self::read_with_args(data, &args)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for FaceRecord<'a> {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "FaceRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "face_offsets", value: FieldType::from(self.faces(_data)) }),
                _ => None,
            }),
            data,
        })
    }
}

impl<'a> MinByteRange<'a> for Face<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(field_byte_range) = self.field_byte_range();

        Ok(0..field_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for Face<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_13) = data.len();

        match ::core::primitive::bool::then_some(value_13 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Face<'a> {
    data: FontData<'a>,
}

impl<'a> Face<'a> {
    pub const MIN_SIZE: u64 = u16::RAW_BYTE_LEN;
    basic_table_impls!(impl_the_methods);

    pub fn field(&self) -> Result<u16, Never> {
        let Ok(range) = self.field_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn field_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Face::MIN_SIZE); held }));

impl Default for Face<'_> {
    fn default() -> Self {
        let Ok(value_14) = FontData::default_table_data();

        Self {
            data: value_14,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Face<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Face")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "field", value: FieldType::from(self.field()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Face<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
