

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Hdmx<'a> {
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

impl TopLevelTable for Hdmx<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"hdmx"); value });
}

impl ReadArgs for Hdmx<'_> {
    type Arguments = u16;
}

impl<'a> FontReadWithArgs<'a> for Hdmx<'a> {
    fn read_with_args(data: FontData<'a>, args: &u16) -> Result<Self, ReadError> {
        let num_glyphs = *args;

        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data, num_glyphs })
    }
}

impl<'a> Hdmx<'a> {
    pub fn read(data: FontData<'a>, num_glyphs: u16) -> Result<Self, ReadError> {
        let args = num_glyphs;
        Self::read_with_args(data, &args)
    }
}

#[derive(Clone)]
pub struct Hdmx<'a> {
    data: FontData<'a>,
    num_glyphs: u16,
}

impl<'a> Hdmx<'a> {
    pub const MIN_SIZE: u64 = (u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + u32::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<u16, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn num_records(&self) -> Result<u16, Never> {
        let Ok(range) = self.num_records_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn size_device_record(&self) -> Result<u32, Never> {
        let Ok(range) = self.size_device_record_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn records(&self) -> Result<ComputedArray<'a, DeviceRecord<'a>>, Never> {
        let Ok(range) = self.records_byte_range();

        self.data.read_with_args_or_default(range, &(({ let Ok(explicit_value) = self.num_glyphs(); explicit_value }), ({ let Ok(explicit_value) = self.size_device_record(); explicit_value })))
    }

    pub(crate) fn num_glyphs(&self) -> Result<u16, Never> {
        Ok(self.num_glyphs)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn num_records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn size_device_record_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.num_records_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u32::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn records_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(num_records) = self.num_records();
        let Ok(previous) = self.size_device_record_byte_range();
        let start = previous.end;
        let Ok(value_2) = transforms::to_u64(num_records);

        let Ok(num_glyphs) = self.num_glyphs();
        let Ok(size_device_record) = self.size_device_record();
        let Ok(size_or_zero) = transforms::size_or_zero(<DeviceRecord as ComputeSize>::compute_size(&(
            num_glyphs,
            size_device_record,
        )));
        let end = start.saturating_add(value_2.saturating_mul(
                size_or_zero,
            ));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Hdmx::MIN_SIZE); held }));

impl Default for Hdmx<'_> {
    fn default() -> Self {
        let Ok(value_3) = FontData::default_table_data();

        Self {
            data: value_3,
            num_glyphs: Default::default(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Hdmx<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Hdmx")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "num_records", value: FieldType::from(self.num_records()) }),
            2u64 => Some(Field { name: "size_device_record", value: FieldType::from(self.size_device_record()) }),
            3u64 => {
                let Ok(value_4) = self.offset_data();

                let Ok(records) = self.records();

                Some(Field { name: "records", value: FieldType::from(traversal::FieldType::computed_array(
                    "DeviceRecord",
                    records,
                    value_4,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Hdmx<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}
