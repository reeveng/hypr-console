

use crate::codegen_prelude::*;

impl<'a> MinByteRange<'a> for Fvar<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(instance_size_byte_range) = self.instance_size_byte_range();

        Ok(0..instance_size_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl TopLevelTable for Fvar<'_> {
    const TAG: Tag = ({ let Ok(value) = Tag::new(b"fvar"); value });
}

impl<'a> FontRead<'a> for Fvar<'a> {
    fn read(data: FontData<'a>) -> Result<Self, ReadError> {
        let Ok(value_1) = data.len();

        match ::core::primitive::bool::then_some(value_1 < Self::MIN_SIZE, ()) { ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
        }, ::core::option::Option::None => {} }

        Ok(Self { data })
    }
}

#[derive(Clone)]
pub struct Fvar<'a> {
    data: FontData<'a>,
}

impl<'a> Fvar<'a> {
    pub const MIN_SIZE: u64 = (MajorMinor::RAW_BYTE_LEN
        + Offset16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN);
    basic_table_impls!(impl_the_methods);

    pub fn version(&self) -> Result<MajorMinor, Never> {
        let Ok(range) = self.version_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn axis_instance_arrays_offset(&self) -> Result<Offset16, Never> {
        let Ok(range) = self.axis_instance_arrays_offset_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn axis_instance_arrays(&self) -> Result<AxisInstanceArrays<'a>, ReadError> {
        let data = self.data;
        let Ok(axis_count) = self.axis_count();
        let Ok(instance_count) = self.instance_count();
        let Ok(instance_size) = self.instance_size();
        let args = (
            axis_count,
            instance_count,
            instance_size,
        );
        let Ok(axis_instance_arrays_offset) = self.axis_instance_arrays_offset();

        axis_instance_arrays_offset
            .resolve_with_args(data, &args)
    }

    pub fn axis_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.axis_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn axis_size(&self) -> Result<u16, Never> {
        let Ok(range) = self.axis_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn instance_count(&self) -> Result<u16, Never> {
        let Ok(range) = self.instance_count_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn instance_size(&self) -> Result<u16, Never> {
        let Ok(range) = self.instance_size_byte_range();

        self.data.read_at_or_zero(range.start)
    }

    pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
        let start = 0u64;
        let end = start.saturating_add(MajorMinor::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn axis_instance_arrays_offset_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.version_byte_range();
        let start = previous.end;
        let end = start.saturating_add(Offset16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn _reserved_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.axis_instance_arrays_offset_byte_range();
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

    pub fn axis_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.axis_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn instance_count_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.axis_size_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }

    pub fn instance_size_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(previous) = self.instance_count_byte_range();
        let start = previous.end;
        let end = start.saturating_add(u16::RAW_BYTE_LEN);
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(Fvar::MIN_SIZE); held }));

impl Default for Fvar<'_> {
    fn default() -> Self {
        let Ok(value_2) = FontData::default_table_data();

        Self {
            data: value_2,
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Fvar<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("Fvar")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => Some(Field { name: "version", value: FieldType::from(self.version()) }),
            1u64 => Some(Field { name: "axis_instance_arrays_offset", value: FieldType::from(FieldType::offset(
                    self.axis_instance_arrays_offset(),
                    (self.axis_instance_arrays()).map(Some),
                )) }),
            2u64 => Some(Field { name: "axis_count", value: FieldType::from(self.axis_count()) }),
            3u64 => Some(Field { name: "axis_size", value: FieldType::from(self.axis_size()) }),
            4u64 => Some(Field { name: "instance_count", value: FieldType::from(self.instance_count()) }),
            5u64 => Some(Field { name: "instance_size", value: FieldType::from(self.instance_size()) }),
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Fvar<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

impl<'a> MinByteRange<'a> for AxisInstanceArrays<'a> {
    fn min_byte_range(&self) -> std::result::Result<Range<u64>, console_core_never::Never> {
        let Ok(instances_byte_range) = self.instances_byte_range();

        Ok(0..instances_byte_range.end)
    }

    fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError> {
        let Ok(range) = self.min_byte_range();
        let Ok(data) = self.data.slice(range);
        let data_2 = data.ok_or(crate::ReadError::OutOfBounds)?;
        let Ok(bytes) = data_2.as_bytes();

        Ok(bytes)
    }
}

impl ReadArgs for AxisInstanceArrays<'_> {
    type Arguments = (u16, u16, u16);
}

impl<'a> FontReadWithArgs<'a> for AxisInstanceArrays<'a> {
    fn read_with_args(data: FontData<'a>, args: &(u16, u16, u16)) -> Result<Self, ReadError> {
        let (axis_count, instance_count, instance_size) = *args;

        Ok(Self {
            data,
            axis_count,
            instance_count,
            instance_size,
        })
    }
}

impl<'a> AxisInstanceArrays<'a> {
    #[cfg_attr(
      dylint_lib = "explicit024_no_swappable_params",
      expect(explicit024_no_swappable_params, reason = "the published `AxisInstanceArrays::read` method retains the positional argument order for `data`, `axis_count`, `instance_count`, `instance_size`")
    )]
    pub fn read(
        data: FontData<'a>,
        axis_count: u16,
        instance_count: u16,
        instance_size: u16,
    ) -> Result<Self, ReadError> {
        let args = (axis_count, instance_count, instance_size);
        Self::read_with_args(data, &args)
    }
}

#[derive(Clone)]
pub struct AxisInstanceArrays<'a> {
    data: FontData<'a>,
    axis_count: u16,
    instance_count: u16,
    instance_size: u16,
}

impl<'a> AxisInstanceArrays<'a> {
    pub const MIN_SIZE: u64 = 0;
    basic_table_impls!(impl_the_methods);

    pub fn axes(&self) -> Result<&'a [VariationAxisRecord], Never> {
        let Ok(range) = self.axes_byte_range();

        self.data.read_array_or_empty(range)
    }

    pub fn instances(&self) -> Result<ComputedArray<'a, InstanceRecord<'a>>, Never> {
        let Ok(range) = self.instances_byte_range();

        self.data.read_with_args_or_default(range, &(({ let Ok(explicit_value) = self.axis_count(); explicit_value }), ({ let Ok(explicit_value) = self.instance_size(); explicit_value })))
    }

    pub(crate) fn axis_count(&self) -> Result<u16, Never> {
        Ok(self.axis_count)
    }

    pub(crate) fn instance_count(&self) -> Result<u16, Never> {
        Ok(self.instance_count)
    }

    pub(crate) fn instance_size(&self) -> Result<u16, Never> {
        Ok(self.instance_size)
    }

    pub fn axes_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(axis_count) = self.axis_count();
        let start = 0u64;
        let Ok(value_4) = transforms::to_u64(axis_count);

        let end = start.saturating_add(value_4.saturating_mul(VariationAxisRecord::RAW_BYTE_LEN));
        Ok(start..end)
    }

    pub fn instances_byte_range(&self) -> Result<Range<u64>, Never> {
        let Ok(instance_count) = self.instance_count();
        let Ok(previous) = self.axes_byte_range();
        let start = previous.end;
        let Ok(value_5) = transforms::to_u64(instance_count);

        let Ok(axis_count) = self.axis_count();
        let Ok(instance_size) = self.instance_size();
        let Ok(size_or_zero) = transforms::size_or_zero(<InstanceRecord as ComputeSize>::compute_size(&(
            axis_count,
            instance_size,
        )));
        let end = start.saturating_add(value_5.saturating_mul(
                size_or_zero,
            ));
        Ok(start..end)
    }
}

const _: () = assert!(({ let Ok(held) = FontData::default_data_long_enough(
    AxisInstanceArrays::MIN_SIZE
); held }));

impl Default for AxisInstanceArrays<'_> {
    fn default() -> Self {
        let Ok(value_6) = FontData::default_table_data();

        Self {
            data: value_6,
            axis_count: Default::default(),
            instance_count: Default::default(),
            instance_size: Default::default(),
        }
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for AxisInstanceArrays<'a> {
    fn type_name(&self) -> Result<&str, Never> {
        Ok("AxisInstanceArrays")
    }

    fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
        Ok(match idx {
            0u64 => {
                let Ok(value_7) = self.offset_data();

                Some(Field { name: "axes", value: FieldType::from(traversal::FieldType::array_of_records(
                    stringify!(VariationAxisRecord),
                    { let Ok(explicit_value) = self.axes(); explicit_value },
                    value_7,
                )) })
            },
            1u64 => {
                let Ok(value_8) = self.offset_data();

                let Ok(instances) = self.instances();

                Some(Field { name: "instances", value: FieldType::from(traversal::FieldType::computed_array(
                    "InstanceRecord",
                    instances,
                    value_8,
                )) })
            },
            _ => None,
        })
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for AxisInstanceArrays<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct VariationAxisRecord {
    pub axis_tag: BigEndian<Tag>,
    pub min_value: BigEndian<Fixed>,
    pub default_value: BigEndian<Fixed>,
    pub max_value: BigEndian<Fixed>,
    pub flags: BigEndian<u16>,
    pub axis_name_id: BigEndian<NameId>,
}

const _: () = {
  assert!(core::mem::size_of::<VariationAxisRecord>() == 20);
  assert!(core::mem::align_of::<VariationAxisRecord>() == 1);
  assert!(core::mem::offset_of!(VariationAxisRecord, axis_tag) == 0);
  assert!(core::mem::offset_of!(VariationAxisRecord, min_value) == 4);
  assert!(core::mem::offset_of!(VariationAxisRecord, default_value) == 8);
  assert!(core::mem::offset_of!(VariationAxisRecord, max_value) == 12);
  assert!(core::mem::offset_of!(VariationAxisRecord, flags) == 16);
  assert!(core::mem::offset_of!(VariationAxisRecord, axis_name_id) == 18);
};

impl VariationAxisRecord {
    pub fn axis_tag(&self) -> Result<Tag, Never> {
        self.axis_tag.get()
    }

    pub fn min_value(&self) -> Result<Fixed, Never> {
        self.min_value.get()
    }

    pub fn default_value(&self) -> Result<Fixed, Never> {
        self.default_value.get()
    }

    pub fn max_value(&self) -> Result<Fixed, Never> {
        self.max_value.get()
    }

    pub fn flags(&self) -> Result<u16, Never> {
        self.flags.get()
    }

    pub fn axis_name_id(&self) -> Result<NameId, Never> {
        self.axis_name_id.get()
    }
}

impl FixedSize for VariationAxisRecord {
    const RAW_BYTE_LEN: u64 = Tag::RAW_BYTE_LEN
        + Fixed::RAW_BYTE_LEN
        + Fixed::RAW_BYTE_LEN
        + Fixed::RAW_BYTE_LEN
        + u16::RAW_BYTE_LEN
        + NameId::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for VariationAxisRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "VariationAxisRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "axis_tag", value: FieldType::from(self.axis_tag()) }),
                1u64 => Some(Field { name: "min_value", value: FieldType::from(self.min_value()) }),
                2u64 => Some(Field { name: "default_value", value: FieldType::from(self.default_value()) }),
                3u64 => Some(Field { name: "max_value", value: FieldType::from(self.max_value()) }),
                4u64 => Some(Field { name: "flags", value: FieldType::from(self.flags()) }),
                5u64 => Some(Field { name: "axis_name_id", value: FieldType::from(self.axis_name_id()) }),
                _ => None,
            }),
            data,
        })
    }
}
