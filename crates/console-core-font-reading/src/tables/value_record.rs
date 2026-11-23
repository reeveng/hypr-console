//! A GPOS ValueRecord

use console_core_integer_sets::Presence;
use console_core_never::Never;
use font_types::Nullable;
use types::{BigEndian, F2Dot14, FixedSize, Offset16};

use super::ValueFormat;
use crate::{
  ResolveNullableOffset,
  tables::{
    layout::DeviceOrVariationIndex,
    variations::{DeltaSetIndex, ItemVariationStore},
  },
};

#[cfg(feature = "experimental_traverse")]
use crate::traversal::{Field, FieldType, RecordResolver, SomeRecord};
use crate::{ComputeSize, FontData, FontReadWithArgs, ReadArgs, ReadError};

fn adjustment_or_zero(adjustment: Option<BigEndian<i16>>) -> Result<i16, Never> {
  match adjustment {
    Some(value) => value.get(),
    None => Ok(0),
  }
}

impl ValueFormat {
  pub const ANY_DEVICE_OR_VARIDX: Self = ValueFormat {
    bits: 0x0010 | 0x0020 | 0x0040 | 0x0080,
  };

  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ValueFormat::record_byte_len` method returns `u64` directly in the existing parser interface"
    )
  )]
  pub fn record_byte_len(self) -> u64 {
    ::core::primitive::u64::wrapping_mul(
      {
        let Ok(bits) = self.bits();
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u64>(bits.count_ones());
        migrated_value
      },
      u16::RAW_BYTE_LEN,
    )
  }
}

#[derive(Clone, Default)]
pub struct ValueContext<'a> {
  coords: &'a [F2Dot14],
  var_store: Option<ItemVariationStore<'a>>,
}

impl<'a> ValueContext<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ValueContext::new` method supports direct construction or chaining with its existing Self return"
    )
  )]
  pub fn new() -> Self {
    Self::default()
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ValueContext::with_coords` method supports direct construction or chaining with its existing Self return"
    )
  )]
  pub fn with_coords(mut self, coords: &'a [F2Dot14]) -> Self {
    self.coords = coords;
    self
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ValueContext::with_var_store` method supports direct construction or chaining with its existing Self return"
    )
  )]
  pub fn with_var_store(mut self, var_store: Option<ItemVariationStore<'a>>) -> Self {
    self.var_store = var_store;
    self
  }

  #[must_use]
  fn var_store_and_coords(
    &self,
  ) -> core::result::Result<
    Option<(&ItemVariationStore<'a>, &'a [F2Dot14])>,
    console_core_never::Never,
  > {
    let var_store = match self.var_store.as_ref() {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    Ok(Some((var_store, self.coords)))
  }
}

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct Value {
  pub format: ValueFormat,
  pub x_placement: i16,
  pub y_placement: i16,
  pub x_advance: i16,
  pub y_advance: i16,
  pub x_placement_delta: i32,
  pub y_placement_delta: i32,
  pub x_advance_delta: i32,
  pub y_advance_delta: i32,
}

impl Value {
  #[inline]
  pub fn read(
    offset_data: FontData,
    offset: u64,
    format: ValueFormat,
    context: &ValueContext,
  ) -> Result<Self, ReadError> {
    let mut value = Self {
      format,
      ..Default::default()
    };
    let Ok(mut cursor) = offset_data.cursor();
    let ::core::result::Result::Ok(()) = cursor.advance_by(offset);
    let Ok(x_placement_flag) = format.contains(ValueFormat::X_PLACEMENT);

    match x_placement_flag {
      Presence::Present => {
        let read = cursor.read()?;
        value.x_placement = read;
      }
      Presence::Absent => {}
    }

    let Ok(y_placement_flag) = format.contains(ValueFormat::Y_PLACEMENT);

    match y_placement_flag {
      Presence::Present => {
        let read = cursor.read()?;
        value.y_placement = read;
      }
      Presence::Absent => {}
    }

    let Ok(x_advance_flag) = format.contains(ValueFormat::X_ADVANCE);

    match x_advance_flag {
      Presence::Present => {
        let read = cursor.read()?;
        value.x_advance = read;
      }
      Presence::Absent => {}
    }

    let Ok(y_advance_flag) = format.contains(ValueFormat::Y_ADVANCE);

    match y_advance_flag {
      Presence::Present => {
        let read = cursor.read()?;
        value.y_advance = read;
      }
      Presence::Absent => {}
    }

    let Ok(any_device_or_varidx) = format.contains(ValueFormat::ANY_DEVICE_OR_VARIDX);

    match any_device_or_varidx {
      Presence::Present => {
        return Ok(value);
      }
      Presence::Absent => {}
    }

    let Ok(var_store_and_coords) = context.var_store_and_coords();

    match var_store_and_coords {
      Some((ivs, coords)) => {
        let compute_delta = |offset_2: u16| {
          let rec_offset = {
            let at = match offset_data.read_at::<u16>({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u64>(offset_2);
              migrated_value
            }) {
              ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
              ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
            }?;
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(at);
            migrated_value
          };
          let format_2 = match offset_data
            .read_at::<u16>(::core::primitive::u64::wrapping_add(rec_offset, 4))
          {
            ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
            ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
          }?;

          const VARIATION_INDEX_FORMAT: u16 = 0x8000;

          match ::core::primitive::bool::then_some(format_2 != VARIATION_INDEX_FORMAT, ()) {
            ::core::option::Option::Some(()) => {
              return Some(0);
            }
            ::core::option::Option::None => {}
          }

          let outer = match offset_data.read_at::<u16>(rec_offset) {
            ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
            ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
          }?;
          let inner = match offset_data
            .read_at::<u16>(::core::primitive::u64::wrapping_add(rec_offset, 2))
          {
            ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
            ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
          }?;

          match ivs.compute_delta(DeltaSetIndex { outer, inner }, coords) {
            ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
            ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
          }
        };
        let Ok(x_placement_device_flag) = format.contains(ValueFormat::X_PLACEMENT_DEVICE);

        match x_placement_device_flag {
          Presence::Present => {
            let read = cursor.read()?;
            value.x_placement_delta = match compute_delta(read) {
              ::core::option::Option::Some(present) => present,
              ::core::option::Option::None => 0,
            };
          }
          Presence::Absent => {}
        }

        let Ok(y_placement_device_flag) = format.contains(ValueFormat::Y_PLACEMENT_DEVICE);

        match y_placement_device_flag {
          Presence::Present => {
            let read = cursor.read()?;
            value.y_placement_delta = match compute_delta(read) {
              ::core::option::Option::Some(present) => present,
              ::core::option::Option::None => 0,
            };
          }
          Presence::Absent => {}
        }

        let Ok(x_advance_device_flag) = format.contains(ValueFormat::X_ADVANCE_DEVICE);

        match x_advance_device_flag {
          Presence::Present => {
            let read = cursor.read()?;
            value.x_advance_delta = match compute_delta(read) {
              ::core::option::Option::Some(present) => present,
              ::core::option::Option::None => 0,
            };
          }
          Presence::Absent => {}
        }

        let Ok(y_advance_device_flag) = format.contains(ValueFormat::Y_ADVANCE_DEVICE);

        match y_advance_device_flag {
          Presence::Present => {
            let read = cursor.read()?;
            value.y_advance_delta = match compute_delta(read) {
              ::core::option::Option::Some(present) => present,
              ::core::option::Option::None => 0,
            };
          }
          Presence::Absent => {}
        }
      }
      ::core::option::Option::None => {}
    }

    Ok(value)
  }
}

#[derive(Clone, Default, Eq)]
pub struct ValueRecord {
  pub x_placement: Option<BigEndian<i16>>,
  pub y_placement: Option<BigEndian<i16>>,
  pub x_advance: Option<BigEndian<i16>>,
  pub y_advance: Option<BigEndian<i16>>,
  pub x_placement_device: BigEndian<Nullable<Offset16>>,
  pub y_placement_device: BigEndian<Nullable<Offset16>>,
  pub x_advance_device: BigEndian<Nullable<Offset16>>,
  pub y_advance_device: BigEndian<Nullable<Offset16>>,
  #[doc(hidden)]
  pub format: ValueFormat,
}

impl PartialEq for ValueRecord {
  fn eq(&self, other: &Self) -> bool {
    self.x_placement == other.x_placement
      && self.y_placement == other.y_placement
      && self.x_advance == other.x_advance
      && self.y_advance == other.y_advance
      && self.x_placement_device == other.x_placement_device
      && self.y_placement_device == other.y_placement_device
      && self.x_advance_device == other.x_advance_device
      && self.y_advance_device == other.y_advance_device
  }
}

impl ValueRecord {
  pub fn read(data: FontData, format: ValueFormat) -> Result<Self, ReadError> {
    let mut this = ValueRecord {
      format,
      ..Default::default()
    };
    let Ok(mut cursor) = data.cursor();

    let Ok(x_placement_flag) = format.contains(ValueFormat::X_PLACEMENT);

    match x_placement_flag {
      Presence::Present => {
        let be = cursor.read_be()?;
        this.x_placement = Some(be);
      }
      Presence::Absent => {}
    }

    let Ok(y_placement_flag) = format.contains(ValueFormat::Y_PLACEMENT);

    match y_placement_flag {
      Presence::Present => {
        let be = cursor.read_be()?;
        this.y_placement = Some(be);
      }
      Presence::Absent => {}
    }

    let Ok(x_advance_flag) = format.contains(ValueFormat::X_ADVANCE);

    match x_advance_flag {
      Presence::Present => {
        let be = cursor.read_be()?;
        this.x_advance = Some(be);
      }
      Presence::Absent => {}
    }

    let Ok(y_advance_flag) = format.contains(ValueFormat::Y_ADVANCE);

    match y_advance_flag {
      Presence::Present => {
        let be = cursor.read_be()?;
        this.y_advance = Some(be);
      }
      Presence::Absent => {}
    }

    let Ok(x_placement_device_flag) = format.contains(ValueFormat::X_PLACEMENT_DEVICE);

    match x_placement_device_flag {
      Presence::Present => {
        let be = cursor.read_be()?;
        this.x_placement_device = be;
      }
      Presence::Absent => {}
    }

    let Ok(y_placement_device_flag) = format.contains(ValueFormat::Y_PLACEMENT_DEVICE);

    match y_placement_device_flag {
      Presence::Present => {
        let be = cursor.read_be()?;
        this.y_placement_device = be;
      }
      Presence::Absent => {}
    }

    let Ok(x_advance_device_flag) = format.contains(ValueFormat::X_ADVANCE_DEVICE);

    match x_advance_device_flag {
      Presence::Present => {
        let be = cursor.read_be()?;
        this.x_advance_device = be;
      }
      Presence::Absent => {}
    }

    let Ok(y_advance_device_flag) = format.contains(ValueFormat::Y_ADVANCE_DEVICE);

    match y_advance_device_flag {
      Presence::Present => {
        let be = cursor.read_be()?;
        this.y_advance_device = be;
      }
      Presence::Absent => {}
    }

    Ok(this)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ValueRecord::x_placement` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn x_placement(&self) -> Option<i16> {
    self.x_placement.map(|val| {
      let Ok(value_33) = val.get();

      value_33
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ValueRecord::y_placement` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn y_placement(&self) -> Option<i16> {
    self.y_placement.map(|val| {
      let Ok(value_34) = val.get();

      value_34
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ValueRecord::x_advance` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn x_advance(&self) -> Option<i16> {
    self.x_advance.map(|val| {
      let Ok(value_35) = val.get();

      value_35
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `ValueRecord::y_advance` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn y_advance(&self) -> Option<i16> {
    self.y_advance.map(|val| {
      let Ok(value_36) = val.get();

      value_36
    })
  }

  pub fn x_placement_device<'a>(
    &self,
    data: FontData<'a>,
  ) -> Result<Option<DeviceOrVariationIndex<'a>>, ReadError> {
    let Ok(value_1) = self.x_placement_device.get();

    value_1.resolve(data)
  }

  pub fn y_placement_device<'a>(
    &self,
    data: FontData<'a>,
  ) -> Result<Option<DeviceOrVariationIndex<'a>>, ReadError> {
    let Ok(value_2) = self.y_placement_device.get();

    value_2.resolve(data)
  }

  pub fn x_advance_device<'a>(
    &self,
    data: FontData<'a>,
  ) -> Result<Option<DeviceOrVariationIndex<'a>>, ReadError> {
    let Ok(value_3) = self.x_advance_device.get();

    value_3.resolve(data)
  }

  pub fn y_advance_device<'a>(
    &self,
    data: FontData<'a>,
  ) -> Result<Option<DeviceOrVariationIndex<'a>>, ReadError> {
    let Ok(value_4) = self.y_advance_device.get();

    value_4.resolve(data)
  }

  pub fn value(&self, offset_data: FontData, context: &ValueContext) -> Result<Value, ReadError> {
    let Ok(x_placement) = adjustment_or_zero(self.x_placement);
    let Ok(y_placement) = adjustment_or_zero(self.y_placement);
    let Ok(x_advance) = adjustment_or_zero(self.x_advance);
    let Ok(y_advance) = adjustment_or_zero(self.y_advance);

    let mut value = Value {
      format: self.format,
      x_placement,
      y_placement,
      x_advance,
      y_advance,
      ..Default::default()
    };

    let Ok(var_store_and_coords) = context.var_store_and_coords();

    match var_store_and_coords {
      Some((ivs, coords)) => {
        let compute_delta = |value_2: DeviceOrVariationIndex| match value_2 {
          DeviceOrVariationIndex::VariationIndex(var_idx) => {
            let Ok(outer) = var_idx.delta_set_outer_index();
            let Ok(inner) = var_idx.delta_set_inner_index();

            match ivs.compute_delta(DeltaSetIndex { outer, inner }, coords) {
              ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
              ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
            }
          }
          DeviceOrVariationIndex::Device(_) => None,
        };

        let x_placement_device = self.x_placement_device(offset_data)?;

        match x_placement_device {
          Some(device) => {
            value.x_placement_delta = match compute_delta(device) {
              ::core::option::Option::Some(present) => present,
              ::core::option::Option::None => 0,
            };
          }
          None => {}
        }

        let y_placement_device = self.y_placement_device(offset_data)?;

        match y_placement_device {
          Some(device) => {
            value.y_placement_delta = match compute_delta(device) {
              ::core::option::Option::Some(present) => present,
              ::core::option::Option::None => 0,
            };
          }
          None => {}
        }

        let x_advance_device = self.x_advance_device(offset_data)?;

        match x_advance_device {
          Some(device) => {
            value.x_advance_delta = match compute_delta(device) {
              ::core::option::Option::Some(present) => present,
              ::core::option::Option::None => 0,
            };
          }
          None => {}
        }

        let y_advance_device = self.y_advance_device(offset_data)?;

        match y_advance_device {
          Some(device) => {
            value.y_advance_delta = match compute_delta(device) {
              ::core::option::Option::Some(present) => present,
              ::core::option::Option::None => 0,
            };
          }
          None => {}
        }
      }
      ::core::option::Option::None => {}
    }

    Ok(value)
  }
}

impl ReadArgs for ValueRecord {
  type Arguments = ValueFormat;
}

impl<'a> FontReadWithArgs<'a> for ValueRecord {
  fn read_with_args(data: FontData<'a>, args: &Self::Arguments) -> Result<Self, ReadError> {
    ValueRecord::read(data, *args)
  }
}

impl std::fmt::Debug for ValueRecord {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    let mut record = f.debug_struct("ValueRecord");

    let adjustments = [
      ("x_placement", self.x_placement),
      ("y_placement", self.y_placement),
      ("x_advance", self.x_advance),
      ("y_advance", self.y_advance),
    ];
    for (name, adjustment) in adjustments {
      match adjustment {
        Some(value) => {
          let _updated_fields = record.field(name, &value);
        }
        None => {}
      }
    }

    let device_offsets = [
      ("x_placement_device", self.x_placement_device),
      ("y_placement_device", self.y_placement_device),
      ("x_advance_device", self.x_advance_device),
      ("y_advance_device", self.y_advance_device),
    ];
    for (name, offset) in device_offsets {
      let Ok(value) = offset.get();

      match value.nullness() {
        Ok(font_types::Nullness::NonNull) => {
          let _updated_fields = record.field(name, &value);
        }
        Ok(font_types::Nullness::Null) => {}
      }
    }

    record.finish()
  }
}

impl ComputeSize for ValueRecord {
  #[inline]
  fn compute_size(args: &ValueFormat) -> Result<u64, ReadError> {
    Ok(args.record_byte_len())
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> ValueRecord {
  #[must_use]
  pub(crate) fn traversal_type(
    &self,
    data: FontData<'a>,
  ) -> core::result::Result<FieldType<'a>, console_core_never::Never> {
    let Ok(traverse) = self.clone().traverse(data);

    Ok(FieldType::Record(traverse))
  }

  fn get_field(&self, idx: u64, data: FontData<'a>) -> Result<Option<Field<'a>>, Never> {
    enum Value {
      Scalar(i16),
      Device(Nullable<Offset16>),
    }

    let Ok(x_placement_device) = self.x_placement_device.get();
    let Ok(y_placement_device) = self.y_placement_device.get();
    let Ok(x_advance_device) = self.x_advance_device.get();
    let Ok(y_advance_device) = self.y_advance_device.get();

    let device = |name, offset: Nullable<Offset16>| {
      let Ok(nullness) = offset.nullness();

      match nullness {
        font_types::Nullness::Null => None,
        font_types::Nullness::NonNull => Some((name, Value::Device(offset))),
      }
    };
    let fields = [
      self
        .x_placement()
        .map(|value| ("x_placement", Value::Scalar(value))),
      self
        .y_placement()
        .map(|value| ("y_placement", Value::Scalar(value))),
      self
        .x_advance()
        .map(|value| ("x_advance", Value::Scalar(value))),
      self
        .y_advance()
        .map(|value| ("y_advance", Value::Scalar(value))),
      device("x_placement_device", x_placement_device),
      device("y_placement_device", y_placement_device),
      device("x_advance_device", x_advance_device),
      device("y_advance_device", y_advance_device),
    ];

    let Ok(index) = console_core_number_conversion::index(idx);
    let field = fields.into_iter().flatten().nth(index);

    Ok(field.map(|(name, value)| {
      let typ = match value {
        Value::Scalar(value_2) => FieldType::from(value_2),
        Value::Device(offset) => {
          let target =
            ResolveNullableOffset::resolve::<DeviceOrVariationIndex<'a>>(&offset, data);
          let Ok(field_type) = FieldType::offset(offset, target);

          field_type
        }
      };
      let Ok(field_2) = Field::new(name, typ);

      field_2
    }))
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for ValueRecord {
  fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
    Ok(RecordResolver {
      name: "ValueRecord",
      data,
      get_field: Box::new(move |idx, data_2| {
        let Ok(field) = self.get_field(idx, data_2);

        field
      }),
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn sanity_check_format_const() {
    let format = ValueFormat::X_ADVANCE_DEVICE
      | ValueFormat::Y_ADVANCE_DEVICE
      | ValueFormat::Y_PLACEMENT_DEVICE
      | ValueFormat::X_PLACEMENT_DEVICE;
    assert_eq!(format, ValueFormat::ANY_DEVICE_OR_VARIDX);
    assert_eq!(
      format.record_byte_len(),
      ::core::primitive::u64::wrapping_mul(4, 2)
    );
  }
}
