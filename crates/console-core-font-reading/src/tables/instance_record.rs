//! An fvar InstanceRecord

#[cfg(feature = "experimental_traverse")]
use console_core_never::Never;
use types::{BigEndian, Fixed, FixedSize, NameId};

#[cfg(feature = "experimental_traverse")]
use crate::traversal::{Field, RecordResolver, SomeRecord};
use crate::{ComputeSize, FontData, FontReadWithArgs, ReadArgs, ReadError};

#[derive(Clone, Debug)]
pub struct InstanceRecord<'a> {
  pub subfamily_name_id: NameId,
  pub flags: u16,
  pub coordinates: &'a [BigEndian<Fixed>],
  pub post_script_name_id: Option<NameId>,
}

impl ReadArgs for InstanceRecord<'_> {
  type Arguments = (u16, u16);
}

impl<'a> InstanceRecord<'a> {
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the published `InstanceRecord::read` method retains the positional argument order for `data`, `axis_count`, `instance_size`"
    )
  )]
  pub fn read(
    data: FontData<'a>,
    axis_count: u16,
    instance_size: u16,
  ) -> Result<Self, ReadError> {
    let args = (axis_count, instance_size);
    Self::read_with_args(data, &args)
  }
}

impl<'a> FontReadWithArgs<'a> for InstanceRecord<'a> {
  fn read_with_args(data: FontData<'a>, args: &Self::Arguments) -> Result<Self, ReadError> {
    let axis_count = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(args.0);
      migrated_value
    };
    let instance_size = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(args.1);
      migrated_value
    };
    let Ok(mut cursor) = data.cursor();
    let subfamily_name_id = cursor.read()?;
    let flags = cursor.read()?;
    let coordinates = cursor.read_array(axis_count)?;

    let common_byte_len = ::core::primitive::u64::wrapping_add(
      ::core::primitive::u64::wrapping_mul(u16::RAW_BYTE_LEN, 2),
      ::core::primitive::u64::wrapping_mul(axis_count, Fixed::RAW_BYTE_LEN),
    );

    let has_post_script_name_id =
      instance_size >= ::core::primitive::u64::wrapping_add(common_byte_len, u16::RAW_BYTE_LEN);
    let post_script_name_id =
      match ::core::primitive::bool::then_some(has_post_script_name_id, ()) {
        ::core::option::Option::Some(()) => {
          let id: NameId = cursor.read()?;

          let Ok(value_1) = id.to_u16();

          (value_1 != 0xFFFF).then_some(id)
        }
        ::core::option::Option::None => None,
      };
    Ok(InstanceRecord {
      subfamily_name_id,
      flags,
      coordinates,
      post_script_name_id,
    })
  }
}

impl ComputeSize for InstanceRecord<'_> {
  #[inline]
  fn compute_size(args: &(u16, u16)) -> Result<u64, ReadError> {
    let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(args.1);

    Ok(migrated_value)
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> InstanceRecord<'a> {
  #[must_use]
  pub(crate) fn get_field(
    &self,
    idx: u64,
    _data: FontData<'a>,
  ) -> core::result::Result<Option<Field<'a>>, console_core_never::Never> {
    Ok(match idx {
      0 => {
        let Ok(field) = Field::new("subfamily_name_id", self.subfamily_name_id);

        Some(field)
      }
      1 => {
        let Ok(field) = Field::new("flags", self.flags);

        Some(field)
      }
      2 => {
        let Ok(field) = Field::new("coordinates", self.coordinates);

        Some(field)
      }
      3 => Some({
        let post_script_name_id = match self.post_script_name_id {
          Some(explicit_value) => explicit_value,
          None => return Ok(None),
        };
        let Ok(explicit_value) = Field::new("post_script_name_id", post_script_name_id);
        explicit_value
      }),
      _ => None,
    })
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for InstanceRecord<'a> {
  fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
    Ok(RecordResolver {
      name: "InstanceRecord",
      data,
      get_field: Box::new(move |idx, data_2| {
        let Ok(field) = self.get_field(idx, data_2);

        field
      }),
    })
  }
}
