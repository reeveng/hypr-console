use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for KindsOfOffsets<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(record_array_offset_byte_range) = self.record_array_offset_byte_range();

        Ok(0..record_array_offset_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for KindsOfOffsets<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct KindsOfOffsets<'a> {
    data: FontData<'a>,
}

impl<'a> KindsOfOffsets<'a> {
    pub const MIN_SIZE: u64 = (MajorMinor::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<MajorMinor, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn nonnullable_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.nonnullable_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn nonnullable(&self) -> Result<Dummy<'a>, ReadError> {
        let data = self.data;
        let Ok(nonnullable_offset) = self.nonnullable_offset();

        nonnullable_offset.resolve(data)
    }

    pub fn nullable_offset(&self) -> Result<Nullable<Offset16>, Never> {
        let Ok(range) = self.nullable_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn nullable(&self) -> Result<Option<Dummy<'a>>, ReadError> {
        let data = self.data;
        let Ok(nullable_offset) = self.nullable_offset();

        nullable_offset.resolve(data)
    }

    pub fn array_offset_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.array_offset_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn array_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.array_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn array(&self) -> Result<&'a [BigEndian<u16>], ReadError> {
        let data = self.data;
        let Ok(args) = self.array_offset_count();
        let Ok(array_offset) = self.array_offset();

        array_offset.resolve_with_args(data, &args)
    }

    pub fn record_array_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.record_array_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn record_array(&self) -> Result<&'a [Shmecord], ReadError> {
        let data = self.data;
        let Ok(args) = self.array_offset_count();
        let Ok(record_array_offset) = self.record_array_offset();

        record_array_offset.resolve_with_args(data, &args)
    }

    pub fn versioned_nullable_record_array_offset(&self) -> Result<Option<Nullable<Offset16>>, Never> {
        let Ok(range) = self.versioned_nullable_record_array_offset_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn versioned_nullable_record_array(&self) -> Result<Option<&'a [Shmecord]>, ReadError> {
        let data = self.data;
        let Ok(args) = self.array_offset_count();
        let Ok(read_versioned_nullable_record_array_offset) = self.versioned_nullable_record_array_offset();

        match read_versioned_nullable_record_array_offset { Some(x) => x.resolve_with_args(data, &args), None => Ok(None) }
    }

    pub fn versioned_nonnullable_offset(&self) -> Result<Option<Offset16>, Never> {
        let Ok(range) = self.versioned_nonnullable_offset_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn versioned_nonnullable(&self) -> Result<Option<Dummy<'a>>, ReadError> {
        let data = self.data;
        let Ok(read_versioned_nonnullable_offset) = self.versioned_nonnullable_offset();

        match read_versioned_nonnullable_offset {Some(offset) => offset.resolve(data).map(Some), None => Ok(None)}
    }

    pub fn versioned_nullable_offset(&self) -> Result<Option<Nullable<Offset32>>, Never> {
        let Ok(range) = self.versioned_nullable_offset_byte_range();

        self.data.read_at_if_present(range)
    }

    pub fn versioned_nullable(&self) -> Result<Option<Dummy<'a>>, ReadError> {
        let data = self.data;
        let Ok(read_versioned_nullable_offset) = self.versioned_nullable_offset();

        match read_versioned_nullable_offset { Some(x) => x.resolve(data), None => Ok(None) }
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(MajorMinor::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn nonnullable_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn nullable_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.nonnullable_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn array_offset_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.nullable_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn array_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.array_offset_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn record_array_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.array_offset_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn versioned_nullable_record_array_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.record_array_offset_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility((1u16, 1u16));

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(Offset16::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }

    pub fn versioned_nonnullable_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.versioned_nullable_record_array_offset_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility((1u16, 1u16));

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(Offset16::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }

    pub fn versioned_nullable_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.versioned_nonnullable_offset_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility((1u16, 1u16));

        let end = match compatibility {
            Compatibility::Compatible => {
                start.saturating_add(Offset32::RAW_BYTE_LEN)
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(KindsOfOffsets::MIN_SIZE); held }));

impl Default for KindsOfOffsets<'_> {
    fn default() -> Self {
        let Ok(value_5) = FontData::default_table_data();

        Self {
            data: value_5,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for KindsOfOffsets<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("KindsOfOffsets")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "nonnullable_offset", value: FieldType::from(FieldType::offset(self.nonnullable_offset(), (self.nonnullable()).map(Some))) }),
            2u64 => Some(Field { name: "nullable_offset", value: FieldType::from(FieldType::offset(self.nullable_offset(), self.nullable())) }),
            3u64 => Some(Field { name: "array_offset_count", value: FieldType::from(self.array_offset_count()) }),
            4u64 => Some(Field { name: "array_offset", value: FieldType::from(FieldType::offset_to_array_of_scalars(self.array_offset(), (self.array()).map(Some))) }),
            5u64 => {
                let Ok(value_6) = self.offset_data();

                Some(Field { name: "record_array_offset", value: FieldType::from(traversal::FieldType::offset_to_array_of_records(
                    self.record_array_offset(),
                    (self.record_array()).map(Some),
                    stringify!(Shmecord),
                    value_6,
                )) })
            },
            6u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility((1u16, 1u16));
                let Ok(value_7) = self.offset_data();
                let Ok(versioned_nullable_record_array_offset) = self.versioned_nullable_record_array_offset();

                match compatibility {
                    Compatibility::Compatible => versioned_nullable_record_array_offset.map(|versioned_nullable_record_array_offset_2| Field { name: "versioned_nullable_record_array_offset", value: FieldType::from(traversal::FieldType::offset_to_array_of_records(
                        versioned_nullable_record_array_offset_2,
                        self.versioned_nullable_record_array(),
                        stringify!(Shmecord),
                        value_7,
                    )) }),
                    Compatibility::Incompatible => None,
                }
            },
            7u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility((1u16, 1u16));
                let Ok(versioned_nonnullable_offset) = self.versioned_nonnullable_offset();

                match compatibility {
                    Compatibility::Compatible => versioned_nonnullable_offset.map(|versioned_nonnullable_offset_2| Field { name: "versioned_nonnullable_offset", value: FieldType::from(FieldType::offset(
                        versioned_nonnullable_offset_2,
                        self.versioned_nonnullable(),
                    )) }),
                    Compatibility::Incompatible => None,
                }
            },
            8u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility((1u16, 1u16));
                let Ok(versioned_nullable_offset) = self.versioned_nullable_offset();

                match compatibility {
                    Compatibility::Compatible => versioned_nullable_offset.map(|versioned_nullable_offset_2| Field { name: "versioned_nullable_offset", value: FieldType::from(FieldType::offset(
                        versioned_nullable_offset_2,
                        self.versioned_nullable(),
                    )) }),
                    Compatibility::Incompatible => None,
                }
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for KindsOfOffsets<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for KindsOfArraysOfOffsets<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(nullable_offsets_byte_range) = self.nullable_offsets_byte_range();

        Ok(0..nullable_offsets_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for KindsOfArraysOfOffsets<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_8) = data.len();

        match ::core::primitive::bool::then_some(value_8 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct KindsOfArraysOfOffsets<'a> {
    data: FontData<'a>,
}

impl<'a> KindsOfArraysOfOffsets<'a> {
    pub const MIN_SIZE: u64 = (MajorMinor::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<MajorMinor, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn count(&self) -> Result<u16, Never> {
        let Ok(range) = self.count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn nonnullable_offsets(&self) -> Result<&'a [BigEndian<Offset16>], Never> {
        let Ok(range) = self.nonnullable_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn nonnullables(&self) -> Result<ArrayOfOffsets<'a, Dummy<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.nonnullable_offsets();
        ArrayOfOffsets::new(offsets, data, ())
    }

    pub fn nullable_offsets(&self) -> Result<&'a [BigEndian<Nullable<Offset16>>], Never> {
        let Ok(range) = self.nullable_offsets_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn nullables(&self) -> Result<ArrayOfNullableOffsets<'a, Dummy<'a>, Offset16>, Never> {
        let data = self.data;
        let Ok(offsets) = self.nullable_offsets();
        ArrayOfNullableOffsets::new(offsets, data, ())
    }

    pub fn versioned_nonnullable_offsets(&self) -> Result<Option<&'a [BigEndian<Offset16>]>, Never> {
        let Ok(range) = self.versioned_nonnullable_offsets_byte_range();

        self.data.read_array_if_present(range)
    }

    pub fn versioned_nonnullables(&self) -> Result<Option<ArrayOfOffsets<'a, Dummy<'a>, Offset16>>, Never> {
        let data = self.data;
        let Ok(offsets) = self.versioned_nonnullable_offsets();
        Ok(offsets.map(|offsets_2| {
            let Ok(value_39) = ArrayOfOffsets::new(offsets_2, data, ());

            value_39
        }))
    }

    pub fn versioned_nullable_offsets(&self) -> Result<Option<&'a [BigEndian<Nullable<Offset16>>]>, Never> {
        let Ok(range) = self.versioned_nullable_offsets_byte_range();

        self.data.read_array_if_present(range)
    }

    pub fn versioned_nullables(&self) -> Result<Option<ArrayOfNullableOffsets<'a, Dummy<'a>, Offset16>>, Never> {
        let data = self.data;
        let Ok(offsets) = self.versioned_nullable_offsets();
        Ok(offsets.map(|offsets_2| {
            let Ok(value_40) = ArrayOfNullableOffsets::new(offsets_2, data, ());

            value_40
        }))
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(MajorMinor::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn nonnullable_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(count) = self.count();
        let Ok(previous) = self.count_byte_range();
        let start = previous.end;
        let Ok(value_9) = transforms::to_u64(count);

        let end = start.saturating_add(value_9.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn nullable_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(count) = self.count();
        let Ok(previous) = self.nonnullable_offsets_byte_range();
        let start = previous.end;
        let Ok(value_10) = transforms::to_u64(count);

        let end = start.saturating_add(value_10.saturating_mul(Offset16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn versioned_nonnullable_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(count) = self.count();
        let Ok(previous) = self.nullable_offsets_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility((1u16, 1u16));

        let end = match compatibility {
            Compatibility::Compatible => {
                let Ok(value_12) = transforms::to_u64(count);

            start.saturating_add(value_12.saturating_mul(Offset16::RAW_BYTE_LEN))
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }

    pub fn versioned_nullable_offsets_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(count) = self.count();
        let Ok(previous) = self.versioned_nonnullable_offsets_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility((1u16, 1u16));

        let end = match compatibility {
            Compatibility::Compatible => {
                let Ok(value_14) = transforms::to_u64(count);

            start.saturating_add(value_14.saturating_mul(Offset16::RAW_BYTE_LEN))
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    KindsOfArraysOfOffsets::MIN_SIZE
); held }));

impl Default for KindsOfArraysOfOffsets<'_> {
    fn default() -> Self {
        let Ok(value_15) = FontData::default_table_data();

        Self {
            data: value_15,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for KindsOfArraysOfOffsets<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("KindsOfArraysOfOffsets")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "count", value: FieldType::from(self.count()) }),
            2u64 => Some(Field { name: "nonnullable_offsets", value: FieldType::from(self.nonnullables()) }),
            3u64 => Some(Field { name: "nullable_offsets", value: FieldType::from(self.nullables()) }),
            4u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility((1u16, 1u16));
                let Ok(versioned_nonnullables) = self.versioned_nonnullables();

                match compatibility {
                    Compatibility::Compatible => versioned_nonnullables.map(|versioned_nonnullables_2| Field { name: "versioned_nonnullable_offsets", value: FieldType::from(versioned_nonnullables_2) }),
                    Compatibility::Incompatible => None,
                }
            },
            5u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility((1u16, 1u16));
                let Ok(versioned_nullables) = self.versioned_nullables();

                match compatibility {
                    Compatibility::Compatible => versioned_nullables.map(|versioned_nullables_2| Field { name: "versioned_nullable_offsets", value: FieldType::from(versioned_nullables_2) }),
                    Compatibility::Incompatible => None,
                }
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for KindsOfArraysOfOffsets<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for KindsOfArrays<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(records_byte_range) = self.records_byte_range();

        Ok(0..records_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for KindsOfArrays<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_16) = data.len();

        match ::core::primitive::bool::then_some(value_16 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct KindsOfArrays<'a> {
    data: FontData<'a>,
}

impl<'a> KindsOfArrays<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<u16, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn count(&self) -> Result<u16, Never> {
        let Ok(range) = self.count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn scalars(&self) -> Result<&'a [BigEndian<u16>], Never> {
        let Ok(range) = self.scalars_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn records(&self) -> Result<&'a [Shmecord], Never> {
        let Ok(range) = self.records_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn versioned_scalars(&self) -> Result<Option<&'a [BigEndian<u16>]>, Never> {
        let Ok(range) = self.versioned_scalars_byte_range();

        self.data.read_array_if_present(range)
    }

    pub fn versioned_records(&self) -> Result<Option<&'a [Shmecord]>, Never> {
        let Ok(range) = self.versioned_records_byte_range();

        self.data.read_array_if_present(range)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn scalars_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(count) = self.count();
        let Ok(previous) = self.count_byte_range();
        let start = previous.end;
        let Ok(value_17) = transforms::to_u64(count);

        let end = start.saturating_add(value_17.saturating_mul(u16::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(count) = self.count();
        let Ok(previous) = self.scalars_byte_range();
        let start = previous.end;
        let Ok(value_18) = transforms::to_u64(count);

        let end = start.saturating_add(value_18.saturating_mul(Shmecord::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn versioned_scalars_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(count) = self.count();
        let Ok(previous) = self.records_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility(1u16);

        let end = match compatibility {
            Compatibility::Compatible => {
                let Ok(value_20) = transforms::to_u64(count);

            start.saturating_add(value_20.saturating_mul(u16::RAW_BYTE_LEN))
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }

    pub fn versioned_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(count) = self.count();
        let Ok(previous) = self.versioned_scalars_byte_range();
        let start = previous.end;
        let Ok(version) = self.version();
        let Ok(compatibility) = version.compatibility(1u16);

        let end = match compatibility {
            Compatibility::Compatible => {
                let Ok(value_22) = transforms::to_u64(count);

            start.saturating_add(value_22.saturating_mul(Shmecord::RAW_BYTE_LEN))
            }
            Compatibility::Incompatible => start,
        };
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(KindsOfArrays::MIN_SIZE); held }));

impl Default for KindsOfArrays<'_> {
    fn default() -> Self {
        let Ok(value_23) = FontData::default_table_data();

        Self {
            data: value_23,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for KindsOfArrays<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("KindsOfArrays")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "count", value: FieldType::from(self.count()) }),
            2u64 => Some(Field { name: "scalars", value: FieldType::from(self.scalars()) }),
            3u64 => {
                let Ok(value_24) = self.offset_data();

                Some(Field { name: "records", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(Shmecord),
                    { let Ok(explicit_value) = self.records(); explicit_value },
                    value_24,
                )) })
            },
            4u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility(1u16);
                let Ok(versioned_scalars) = self.versioned_scalars();

                match compatibility {
                    Compatibility::Compatible => versioned_scalars.map(|versioned_scalars_2| Field { name: "versioned_scalars", value: FieldType::from(versioned_scalars_2) }),
                    Compatibility::Incompatible => None,
                }
            },
            5u64 => {
                let Ok(version) = self.version();
                let Ok(compatibility) = version.compatibility(1u16);
                let Ok(value_25) = self.offset_data();
                let Ok(versioned_records) = self.versioned_records();

                match compatibility {
                    Compatibility::Compatible => versioned_records.map(|versioned_records_2| Field { name: "versioned_records", value: FieldType::from(traversal::FieldType::array_of_records(
                        stringify!(Shmecord),
                        versioned_records_2,
                        value_25,
                    )) }),
                    Compatibility::Incompatible => None,
                }
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for KindsOfArrays<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for VarLenHaver<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(other_field_byte_range) = self.other_field_byte_range();

        Ok(0..other_field_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl<'a> FontRead<'a> for VarLenHaver<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_26) = data.len();

        match ::core::primitive::bool::then_some(value_26 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct VarLenHaver<'a> {
    data: FontData<'a>,
}

impl<'a> VarLenHaver<'a> {
  pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn count(&self) -> Result<u16, Never> {
        let Ok(range) = self.count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn var_len(&self) -> Result<VarLenArray<'a, VarSizeDummy<'a>>, Never> {
        let Ok(range) = self.var_len_byte_range();
        let Ok(value_33) = self.data.slice(range);

        let records = match value_33 {
            Some(data) => match VarLenArray::read(data) {
                Ok(records) => records,
                Err(_unreadable_font_data) => VarLenArray::default(),
            },
            None => VarLenArray::default(),
        };
        Ok(records)
    }

    pub fn other_field(&self) -> Result<u32, Never> {
        let Ok(range) = self.other_field_byte_range();

        self.data.read_at_or_default(range.start)
    }

    pub fn count_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn var_len_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(count) = self.count();
        let Ok(previous) = self.count_byte_range();
        let start = previous.end;
        let end = start.saturating_add({ let Ok(explicit_value) = transforms::total_len_for_count::<VarSizeDummy, _>(self.data, start, count); explicit_value });
        Ok(start..end)
    }

    pub fn other_field_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.var_len_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(VarLenHaver::MIN_SIZE); held }));

impl Default for VarLenHaver<'_> {
    fn default() -> Self {
        let Ok(value_28) = FontData::default_table_data();

        Self {
            data: value_28,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for VarLenHaver<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("VarLenHaver")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "count", value: FieldType::from(self.count()) }),
            1u64 => Some(Field { name: "var_len", value: traversal::FieldType::Unknown }),
            2u64 => Some(Field { name: "other_field", value: FieldType::from(self.other_field()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for VarLenHaver<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for Dummy<'a> {
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

impl<'a> FontRead<'a> for Dummy<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_29) = data.len();

        match ::core::primitive::bool::then_some(value_29 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Dummy<'a> {
    data: FontData<'a>,
}

impl<'a> Dummy<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn value(&self) -> Result<u16, Never> {
        let Ok(range) = self.value_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn value_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn _reserved_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.value_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Dummy::MIN_SIZE); held }));

impl Default for Dummy<'_> {
    fn default() -> Self {
        let Ok(value_30) = FontData::default_table_data();

        Self {
            data: value_30,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Dummy<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Dummy")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "value", value: FieldType::from(self.value()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Dummy<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct Shmecord {
    pub length: BigEndian<u16>,
    pub breadth: BigEndian<u32>,
}

const _: () = {
  assert!(core::mem::size_of::<Shmecord>() == 6);
  assert!(core::mem::align_of::<Shmecord>() == 1);
  assert!(core::mem::offset_of!(Shmecord, length) == 0);
  assert!(core::mem::offset_of!(Shmecord, breadth) == 2);
};

impl Shmecord {
    pub fn length(&self) -> Result<u16, Never> {
        self.length.get()
    }

    pub fn breadth(&self) -> Result<u32, Never> {
        self.breadth.get()
    }
}

impl FixedSize for Shmecord {
    const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN + u32::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for Shmecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "Shmecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "length", value: FieldType::from(self.length()) }),
                1u64 => Some(Field { name: "breadth", value: FieldType::from(self.breadth()) }),
                _ => None,
            }),
            data,
        })
    }
}
