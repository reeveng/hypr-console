//! Apple Advanced Typography common tables.
//!
//! See <https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6Tables.html>

include!("../../generated/generated_aat.rs");

pub mod class {
  pub const END_OF_TEXT: u8 = 0;
  pub const OUT_OF_BOUNDS: u8 = 1;
  pub const DELETED_GLYPH: u8 = 2;
}

impl Lookup0<'_> {
  pub fn values<T: LookupValue>(&self) -> Result<&[BigEndian<T>], ReadError> {
    let Ok(bytes) = self.values_data();
    let Ok(length) = console_core_number_conversion::fixed_index(bytes.len());
    let count = length.checked_div(T::RAW_BYTE_LEN).ok_or(ReadError::OutOfBounds)?;
    let extent = count.wrapping_mul(T::RAW_BYTE_LEN);
    let Ok(range) = console_core_number_conversion::indices_to(..extent);
    let records = bytes.get(range).ok_or(ReadError::OutOfBounds)?;
    let Ok(data) = FontData::new(records);
    let Ok(mut cursor) = data.cursor();

    cursor.read_array::<BigEndian<T>>(count)
  }

  pub fn value<T: LookupValue>(&self, index: u16) -> Result<T, ReadError> {
    {
      let Ok(value_32) = console_core_number_conversion::index({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(index);
        migrated_value
      });

      let values = self.values::<T>()?;

      values
        .get(value_32)
        .map(|val| {
          let Ok(value_41) = val.get();

          value_41
        })
        .ok_or(ReadError::OutOfBounds)
    }
  }
}

#[derive(Copy, Clone, bytemuck::AnyBitPattern)]
#[repr(C)]
pub struct LookupSegment2<T>
where
  T: LookupValue,
{
  pub last_glyph: BigEndian<u16>,
  pub first_glyph: BigEndian<u16>,
  pub value: BigEndian<T>,
}

const _: () = {
  assert!(core::mem::size_of::<LookupSegment2<u16>>() == 6);
  assert!(core::mem::align_of::<LookupSegment2<u16>>() == 1);
  assert!(core::mem::offset_of!(LookupSegment2<u16>, last_glyph) == 0);
  assert!(core::mem::offset_of!(LookupSegment2<u16>, first_glyph) == 2);
  assert!(core::mem::offset_of!(LookupSegment2<u16>, value) == 4);
  assert!(core::mem::size_of::<LookupSegment2<u32>>() == 8);
  assert!(core::mem::align_of::<LookupSegment2<u32>>() == 1);
  assert!(core::mem::offset_of!(LookupSegment2<u32>, last_glyph) == 0);
  assert!(core::mem::offset_of!(LookupSegment2<u32>, first_glyph) == 2);
  assert!(core::mem::offset_of!(LookupSegment2<u32>, value) == 4);
  assert!(core::mem::size_of::<LookupSegment2<GlyphId16>>() == 6);
  assert!(core::mem::align_of::<LookupSegment2<GlyphId16>>() == 1);
  assert!(core::mem::offset_of!(LookupSegment2<GlyphId16>, last_glyph) == 0);
  assert!(core::mem::offset_of!(LookupSegment2<GlyphId16>, first_glyph) == 2);
  assert!(core::mem::offset_of!(LookupSegment2<GlyphId16>, value) == 4);
};

impl<T: LookupValue> FixedSize for LookupSegment2<T> {
  const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN
    .wrapping_mul(2)
    .wrapping_add(T::RAW_BYTE_LEN);
}

impl Lookup2<'_> {
  pub fn value<T: LookupValue>(&self, index: u16) -> Result<T, ReadError> {
    let segments = self.segments::<T>()?;
    let ::core::result::Result::Ok(ix) = ::console_core_number_conversion::fixed_index(
      match segments.binary_search_by(|segment| {
        let Ok(value_3) = segment.first_glyph.get();

        value_3.cmp(&index)
      }) {
        Ok(ix) => ix,
        Err(ix) => ix.saturating_sub(1),
      },
    );
    let segment = segments
      .get(match ::console_core_number_conversion::index(ix) {
        ::core::result::Result::Ok(native_position) => native_position,
      })
      .ok_or(ReadError::OutOfBounds)?;
    let Ok(value_42) = segment.first_glyph.get();
    let Ok(value_43) = segment.last_glyph.get();

    let value_4 = value_42..=value_43;

    match ::core::primitive::bool::then_some(value_4.contains(&index), ()) {
      ::core::option::Option::Some(()) => {
        let value = segment.value;
        let Ok(value_5) = value.get();

        return Ok(value_5);
      }
      ::core::option::Option::None => {}
    }

    Err(ReadError::OutOfBounds)
  }

  pub fn segments<T: LookupValue>(&self) -> Result<&[LookupSegment2<T>], ReadError> {
    let Ok(segments_data) = self.segments_data();
    let Ok(value_6) = FontData::new(segments_data);

    let Ok(mut value_27) = value_6.cursor();

    value_27.read_array({
      let Ok(n_units) = self.n_units();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(n_units);
      migrated_value
    })
  }
}

impl Lookup4<'_> {
  pub fn value<T: LookupValue>(&self, index: u16) -> Result<T, ReadError> {
    let Ok(segments) = self.segments();
    let ::core::result::Result::Ok(ix) = ::console_core_number_conversion::fixed_index(
      match segments.binary_search_by(|segment| {
        let Ok(value_7) = segment.first_glyph.get();

        value_7.cmp(&index)
      }) {
        Ok(ix) => ix,
        Err(ix) => ix.saturating_sub(1),
      },
    );
    let segment = segments
      .get(match ::console_core_number_conversion::index(ix) {
        ::core::result::Result::Ok(native_position) => native_position,
      })
      .ok_or(ReadError::OutOfBounds)?;
    let Ok(value_44) = segment.first_glyph.get();
    let Ok(value_45) = segment.last_glyph.get();

    let value_8 = value_44..=value_45;

    match ::core::primitive::bool::then_some(value_8.contains(&index), ()) {
      ::core::option::Option::Some(()) => {
        let base_offset = {
          let Ok(value_offset) = segment.value_offset();
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(value_offset);
          migrated_value
        };
        let offset = ::core::primitive::u64::wrapping_add(
          base_offset,
          ::core::primitive::u64::wrapping_mul(
            {
              let Ok(first_glyph) = segment.first_glyph();
              let checked_value = index
                .checked_sub(first_glyph)
                .ok_or(ReadError::OutOfBounds)?;
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u64>(checked_value);
              migrated_value
            },
            T::RAW_BYTE_LEN,
          ),
        );
        let Ok(value_9) = self.offset_data();

        return value_9.read_at(offset);
      }
      ::core::option::Option::None => {}
    }

    Err(ReadError::OutOfBounds)
  }

  pub fn segment_values<T: LookupValue>(
    &self,
    segment: u64,
  ) -> Result<&[BigEndian<T>], ReadError> {
    let Ok(value_33) = console_core_number_conversion::index(segment);

    let Ok(segments) = self.segments();
    let segment_2 = segments.get(value_33).ok_or(ReadError::OutOfBounds)?;
    let base_offset = {
      let Ok(value_offset) = segment_2.value_offset();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_offset);
      migrated_value
    };
    let Ok(value_10) = segment_2.last_glyph.get();
    let Ok(value_11) = segment_2.first_glyph.get();

    let n_elems = ::core::primitive::u64::wrapping_add(
      {
        let checked_value = value_10
          .checked_sub(value_11)
          .ok_or(ReadError::MalformedData(
            "invalid segment in format 4 AAT lookup table",
          ))?;
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(checked_value);
        migrated_value
      },
      1,
    );
    let Ok(value_12) = self.offset_data();

    value_12.read_array::<BigEndian<T>>(
      base_offset
        ..::core::primitive::u64::wrapping_add(
          base_offset,
          ::core::primitive::u64::wrapping_mul(n_elems, T::RAW_BYTE_LEN),
        ),
    )
  }
}

#[derive(Copy, Clone, bytemuck::AnyBitPattern)]
#[repr(C)]
pub struct LookupSingle<T>
where
  T: LookupValue,
{
  pub glyph: BigEndian<u16>,
  pub value: BigEndian<T>,
}

const _: () = {
  assert!(core::mem::size_of::<LookupSingle<u16>>() == 4);
  assert!(core::mem::align_of::<LookupSingle<u16>>() == 1);
  assert!(core::mem::offset_of!(LookupSingle<u16>, glyph) == 0);
  assert!(core::mem::offset_of!(LookupSingle<u16>, value) == 2);
  assert!(core::mem::size_of::<LookupSingle<u32>>() == 6);
  assert!(core::mem::align_of::<LookupSingle<u32>>() == 1);
  assert!(core::mem::offset_of!(LookupSingle<u32>, glyph) == 0);
  assert!(core::mem::offset_of!(LookupSingle<u32>, value) == 2);
  assert!(core::mem::size_of::<LookupSingle<GlyphId16>>() == 4);
  assert!(core::mem::align_of::<LookupSingle<GlyphId16>>() == 1);
  assert!(core::mem::offset_of!(LookupSingle<GlyphId16>, glyph) == 0);
  assert!(core::mem::offset_of!(LookupSingle<GlyphId16>, value) == 2);
};

impl<T: LookupValue> FixedSize for LookupSingle<T> {
  const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN.wrapping_add(T::RAW_BYTE_LEN);
}

impl Lookup6<'_> {
  pub fn value<T: LookupValue>(&self, index: u16) -> Result<T, ReadError> {
    let entries = self.entries::<T>()?;

    let entry = entries.binary_search_by_key(&index, |entry| {
      let Ok(glyph) = entry.glyph.get();
      glyph
    }).map_err(|_insertion_position| ReadError::OutOfBounds)
      .and_then(|position| entries.get(position).ok_or(ReadError::OutOfBounds))?;
    let Ok(value) = entry.value.get();
    Ok(value)
  }

  pub fn entries<T: LookupValue>(&self) -> Result<&[LookupSingle<T>], ReadError> {
    let Ok(entries_data) = self.entries_data();
    let Ok(value_15) = FontData::new(entries_data);

    let Ok(mut value_29) = value_15.cursor();

    value_29.read_array({
      let Ok(n_units) = self.n_units();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(n_units);
      migrated_value
    })
  }
}

impl Lookup8<'_> {
  pub fn value<T: LookupValue>(&self, index: u16) -> Result<T, ReadError> {
    let Ok(first_glyph) = self.first_glyph();

    index
      .checked_sub(first_glyph)
      .and_then(|ix| {
        let Ok(value_35) = console_core_number_conversion::index({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(ix);
          migrated_value
        });

        let Ok(value_array) = self.value_array();

        value_array.get(value_35).map(|val| {
          let Ok(value_16) = val.get();

          T::from_u16(value_16)
        })
      })
      .ok_or(ReadError::OutOfBounds)
  }
}

impl Lookup10<'_> {
  pub fn value<T: LookupValue>(&self, index: u16) -> Result<T, ReadError> {
    let ix = {
      let Ok(first_glyph) = self.first_glyph();
      let checked_value = index
        .checked_sub(first_glyph)
        .ok_or(ReadError::OutOfBounds)?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(checked_value);
      migrated_value
    };
    let unit_size = {
      let Ok(own_unit_size) = self.unit_size();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(own_unit_size);
      migrated_value
    };
    let offset = ::core::primitive::u64::wrapping_mul(ix, unit_size);
    let Ok(values_data) = self.values_data();
    let Ok(value_17) = FontData::new(values_data);

    let Ok(mut cursor) = value_17.cursor();
    let ::core::result::Result::Ok(()) = cursor.advance_by(offset);
    let val = match unit_size {
      1 => {
        let read = cursor.read::<u8>()?;
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(read);
        migrated_value
      }
      2 => {
        let read = cursor.read::<u16>()?;
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(read);
        migrated_value
      }
      4 => cursor.read::<u32>()?,
      _ => {
        return Err(ReadError::MalformedData(
          "invalid unit_size in format 10 AAT lookup table",
        ));
      }
    };
    Ok(T::from_u32(val))
  }
}

impl Lookup<'_> {
  pub fn value<T: LookupValue>(&self, index: u16) -> Result<T, ReadError> {
    match self {
      Lookup::Format0(lookup) => lookup.value::<T>(index),
      Lookup::Format2(lookup) => lookup.value::<T>(index),
      Lookup::Format4(lookup) => lookup.value::<T>(index),
      Lookup::Format6(lookup) => lookup.value::<T>(index),
      Lookup::Format8(lookup) => lookup.value::<T>(index),
      Lookup::Format10(lookup) => lookup.value::<T>(index),
    }
  }
}

#[derive(Clone)]
pub struct TypedLookup<'a, T> {
  pub lookup: Lookup<'a>,
  _marker: std::marker::PhantomData<fn() -> T>,
}

impl<T: LookupValue> TypedLookup<'_, T> {
  pub fn value(&self, index: u16) -> Result<T, ReadError> {
    self.lookup.value::<T>(index)
  }
}

impl<'a, T> FontRead<'a> for TypedLookup<'a, T> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    let lookup = Lookup::read(data)?;

    Ok(Self {
      lookup,
      _marker: std::marker::PhantomData,
    })
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a, T> SomeTable<'a> for TypedLookup<'a, T> {
  fn type_name(&self) -> Result<&str, Never> {
    Ok("TypedLookup")
  }

  fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
    let Ok(explicit_value) = self.lookup.get_field(idx);

    Ok(explicit_value)
  }
}

pub trait LookupValue: Copy + Scalar + bytemuck::AnyBitPattern {
  fn from_u16(v: u16) -> Self;

  fn from_u32(v: u32) -> Self;
}

impl LookupValue for u16 {
  fn from_u16(v: u16) -> Self {
    v
  }

  fn from_u32(v: u32) -> Self {
    {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(v);
      migrated_value
    }
  }
}

impl LookupValue for u32 {
  fn from_u16(v: u16) -> Self {
    {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(v);
      migrated_value
    }
  }

  fn from_u32(v: u32) -> Self {
    v
  }
}

impl LookupValue for GlyphId16 {
  fn from_u16(v: u16) -> Self {
    GlyphId16::from(v)
  }

  fn from_u32(v: u32) -> Self {
    GlyphId16::from({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(v);
      migrated_value
    })
  }
}

pub type LookupU16<'a> = TypedLookup<'a, u16>;
pub type LookupU32<'a> = TypedLookup<'a, u32>;
pub type LookupGlyphId<'a> = TypedLookup<'a, GlyphId16>;

#[derive(Copy, Clone, bytemuck::AnyBitPattern, Debug)]
pub struct NoPayload(());

impl FixedSize for NoPayload {
  const RAW_BYTE_LEN: u64 = 0;
}

#[derive(Clone, Debug)]
pub struct StateEntry<T = NoPayload> {
  pub new_state: u16,
  pub flags: u16,
  pub payload: T,
}

impl<'a, T: bytemuck::AnyBitPattern + FixedSize> FontRead<'a> for StateEntry<T> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    let Ok(mut cursor) = data.cursor();
    let new_state = cursor.read()?;
    let flags = cursor.read()?;
    let Ok(value_18) = cursor.remaining();

    let remaining = value_18.ok_or(ReadError::OutOfBounds)?;
    let payload = ::core::result::Result::<&_, _>::copied(remaining.read_ref_at(0))?;
    Ok(Self {
      new_state,
      flags,
      payload,
    })
  }
}

impl<T> FixedSize for StateEntry<T>
where
  T: FixedSize,
{
  const RAW_BYTE_LEN: u64 = u16::RAW_BYTE_LEN + u16::RAW_BYTE_LEN + T::RAW_BYTE_LEN;
}

#[derive(Clone)]
pub struct StateTable<'a> {
  pub header: StateHeader<'a>,
  n_classes: u64,
  class_first_glyph: u16,
  class_array: &'a [u8],
  state_array: &'a [u8],
  entry_table: &'a [u8],
}

impl StateTable<'_> {
  pub const HEADER_LEN: u64 = u16::RAW_BYTE_LEN * 4;

  pub fn class(&self, glyph_id: GlyphId16) -> Result<u8, ReadError> {
    let Ok(glyph_id_2) = glyph_id.to_u16();

    match ::core::primitive::bool::then_some(glyph_id_2 == 0xFFFF, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(class::DELETED_GLYPH);
      }
      ::core::option::Option::None => {}
    }

    let value_19 = glyph_id_2;

    value_19
      .checked_sub(self.class_first_glyph)
      .and_then(|ix| {
        let Ok(value_36) = console_core_number_conversion::index({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(ix);
          migrated_value
        });

        self.class_array.get(value_36).copied()
      })
      .ok_or(ReadError::OutOfBounds)
  }

  #[inline(always)]
  pub fn entry(&self, state: u16, class: u8) -> Result<StateEntry, ReadError> {
    let mut class_2 = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(class);
      migrated_value
    };

    match ::core::primitive::bool::then_some(class_2 >= self.n_classes, ()) {
      ::core::option::Option::Some(()) => {
        class_2 = {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(class::OUT_OF_BOUNDS);
          migrated_value
        };
      }
      ::core::option::Option::None => {}
    }

    let Ok(value_37) =
      console_core_number_conversion::index(::core::primitive::u64::wrapping_add(
        ::core::primitive::u64::wrapping_mul(
          {
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(state);
            migrated_value
          },
          self.n_classes,
        ),
        class_2,
      ));

    let entry_ix = {
      let state_array = self
        .state_array
        .get(value_37)
        .copied()
        .ok_or(ReadError::OutOfBounds)?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(state_array);
      migrated_value
    };
    let entry_offset = ::core::primitive::u64::wrapping_mul(entry_ix, 4);
    let Ok(value_38) = console_core_number_conversion::indices_from(entry_offset..);

    let entry_data = self
      .entry_table
      .get(value_38)
      .ok_or(ReadError::OutOfBounds)?;
    let Ok(value_20) = FontData::new(entry_data);

    let mut entry = StateEntry::read(value_20)?;

    let Ok(state_array_offset) = self.header.state_array_offset();
    let Ok(value_21) = state_array_offset.to_u32();

    let checked_value = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(entry.new_state);
      migrated_value
    })
    .checked_sub({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(value_21);
      migrated_value
    })
    .ok_or(ReadError::OutOfBounds)?;
    let Ok(n_classes) = console_core_number_conversion::wrapped::<_, i32>(self.n_classes);
    let new_state = checked_value.checked_div(n_classes).ok_or(ReadError::OutOfBounds)?;
    let map_err = new_state.try_into().map_err(|_| ReadError::OutOfBounds)?;
    entry.new_state = map_err;
    Ok(entry)
  }

  pub fn read_value<T: Scalar>(&self, offset: u64) -> Result<T, ReadError> {
    let Ok(value_22) = self.header.offset_data();

    value_22.read_at::<T>(offset)
  }
}

impl<'a> FontRead<'a> for StateTable<'a> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    let header = StateHeader::read(data)?;

    let n_classes = {
      let Ok(state_size) = header.state_size();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(state_size);
      migrated_value
    };

    match ::core::primitive::bool::then_some(n_classes == 0, ()) {
      ::core::option::Option::Some(()) => {
        return Err(ReadError::MalformedData("empty AAT state table"));
      }
      ::core::option::Option::None => {}
    }

    let class_table = header.class_table()?;
    let Ok(class_first_glyph) = class_table.first_glyph();
    let Ok(class_array) = class_table.class_array();
    let state_array = {
      let header_state_array = header.state_array()?;
      let Ok(explicit_value) = header_state_array.data();
      explicit_value
    };
    let entry_table = {
      let header_entry_table = header.entry_table()?;
      let Ok(explicit_value) = header_entry_table.data();
      explicit_value
    };
    let read = StateHeader::read(data)?;

    Ok(Self {
      header: read,
      n_classes,
      class_first_glyph,
      class_array,
      state_array,
      entry_table,
    })
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for StateTable<'a> {
  fn type_name(&self) -> Result<&str, Never> {
    Ok("StateTable")
  }

  fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
    let Ok(explicit_value) = self.header.get_field(idx);

    Ok(explicit_value)
  }
}

#[derive(Clone)]
pub struct ExtendedStateTable<'a, T = NoPayload> {
  pub n_classes: u64,
  pub class_table: LookupU16<'a>,
  state_array: &'a [BigEndian<u16>],
  entry_table: &'a [u8],
  _marker: std::marker::PhantomData<fn() -> T>,
}

impl<T> ExtendedStateTable<'_, T> {
  pub const HEADER_LEN: u64 = u32::RAW_BYTE_LEN * 4;
}

impl<T> ExtendedStateTable<'_, T>
where
  T: FixedSize + bytemuck::AnyBitPattern,
{
  pub fn class(&self, glyph_id: GlyphId) -> Result<u16, ReadError> {
    let Ok(value_30) = glyph_id.to_u32();

    let glyph_id_2: u16 = value_30.try_into().map_err(|_| ReadError::OutOfBounds)?;

    match ::core::primitive::bool::then_some(glyph_id_2 == 0xFFFF, ()) {
      ::core::option::Option::Some(()) => {
        return Ok({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u16>(class::DELETED_GLYPH);
          migrated_value
        });
      }
      ::core::option::Option::None => {}
    }

    self.class_table.value(glyph_id_2)
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(explicit024_no_swappable_params, reason = "the published `ExtendedStateTable::entry` method retains the positional argument order for `state`, `class`")
  )]
  pub fn entry(&self, state: u16, class: u16) -> Result<StateEntry<T>, ReadError> {
    let mut class_2 = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(class);
      migrated_value
    };

    match ::core::primitive::bool::then_some(class_2 >= self.n_classes, ()) {
      ::core::option::Option::Some(()) => {
        class_2 = {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(class::OUT_OF_BOUNDS);
          migrated_value
        };
      }
      ::core::option::Option::None => {}
    }

    let state_ix = ::core::primitive::u64::wrapping_add(
      ::core::primitive::u64::wrapping_mul(
        {
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(state);
          migrated_value
        },
        self.n_classes,
      ),
      class_2,
    );
    let Ok(value_39) = console_core_number_conversion::index(state_ix);

    let state_array = self
      .state_array
      .get(value_39)
      .copied()
      .ok_or(ReadError::OutOfBounds)?;
    let Ok(value_23) = state_array.get();

    let entry_ix = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_23);
      migrated_value
    };
    let entry_offset =
      ::core::primitive::u64::wrapping_mul(entry_ix, StateEntry::<T>::RAW_BYTE_LEN);
    let Ok(value_40) = console_core_number_conversion::indices_from(entry_offset..);

    let entry_data = self
      .entry_table
      .get(value_40)
      .ok_or(ReadError::OutOfBounds)?;
    let Ok(value_24) = FontData::new(entry_data);

    StateEntry::read(value_24)
  }
}

impl<'a, T> FontRead<'a> for ExtendedStateTable<'a, T> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    let header = StxHeader::read(data)?;
    let n_classes = {
      let Ok(header_n_classes) = header.n_classes();
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u64>(header_n_classes);
      migrated_value
    };
    let class_table = header.class_table()?;
    let state_array = {
      let header_state_array = header.state_array()?;
      let Ok(explicit_value) = header_state_array.data();
      explicit_value
    };
    let entry_table = {
      let header_entry_table = header.entry_table()?;
      let Ok(explicit_value) = header_entry_table.data();
      explicit_value
    };
    Ok(Self {
      n_classes,
      class_table,
      state_array,
      entry_table,
      _marker: std::marker::PhantomData,
    })
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a, T> SomeTable<'a> for ExtendedStateTable<'a, T> {
  fn type_name(&self) -> Result<&str, Never> {
    Ok("ExtendedStateTable")
  }

  fn get_field(&self, _idx: u64) -> Result<Option<Field<'a>>, Never> {
    Ok(None)
  }
}

pub(crate) fn safe_read_array_to_end<'a, T: bytemuck::AnyBitPattern + FixedSize>(
  data: &FontData<'a>,
  offset: u64,
) -> Result<&'a [T], ReadError> {
  let Ok(value_25) = data.len();

  let len = value_25.checked_sub(offset).ok_or(ReadError::OutOfBounds)?;
  let count = len.checked_div(T::RAW_BYTE_LEN).ok_or(ReadError::OutOfBounds)?;
  let end = offset.wrapping_add(count.wrapping_mul(T::RAW_BYTE_LEN));
  data.read_array(offset..end)
}

#[cfg(test)]
mod tests {

  use super::*;

  #[test]
  fn lookup_format_0() -> ::core::result::Result<(), crate::read::ReadError> {
    #[rustfmt::skip]
        let words = [
            0_u16,
            0, 2, 4, 6, 8, 10, 12, 14, 16,
        ];
    let mut buf = font_test_data::bebuffer::BeBuffer::default();
    buf.extend(words);
    let lookup = LookupU16::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf))?;

    for gid in 0..=8 {
      assert_eq!(
        match lookup.value(gid) { ::core::result::Result::Ok(read_value) => read_value, ::core::result::Result::Err(read_error) => return ::core::result::Result::Err(read_error) },
        ::core::primitive::u16::wrapping_mul(gid, 2)
      );
    }

    assert!((match &(lookup.value(9)) { Ok(_validated_input) => false, Err(_invalid_fixture_input) => true }));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn lookup_format_2() -> ::core::result::Result<(), crate::test_support::Failure> {
    #[rustfmt::skip]
        let words = [
            2_u16,
            6,
            3,
            12,
            1,
            6,
            22, 20, 4,
            24, 23, 5,
            28, 25, 6,
        ];
    let mut buf = font_test_data::bebuffer::BeBuffer::default();
    buf.extend(words);
    let lookup = crate::test_support::fixture_value(LookupU16::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf)), "fixture value")?;
    let expected = [(20..=22, 4), (23..=24, 5), (25..=28, 6)];

    for (range, class) in expected {
      for gid in range {
        assert_eq!(lookup.value(gid), ::core::result::Result::Ok(class));
      }
    }

    for fail in [0, 10, 19, 29, 0xFFFF] {
      assert!((match &(lookup.value(fail)) { Ok(_validated_input) => false, Err(_invalid_fixture_input) => true }));
    }
  ;

    Ok(())
  }

  #[test]
  fn lookup_format_4() -> ::core::result::Result<(), crate::test_support::Failure> {
    #[rustfmt::skip]
        let words = [
            4_u16,
            6,
            3,
            12,
            1,
            6,
            22, 20, 30,
            24, 23, 36,
            28, 25, 40,

            3, 2, 1,
            100, 150,
            8, 6, 7, 9
        ];
    let mut buf = font_test_data::bebuffer::BeBuffer::default();
    buf.extend(words);
    let lookup = crate::test_support::fixture_value(LookupU16::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf)), "fixture value")?;

    let expected = [
      (20, 3),
      (21, 2),
      (22, 1),
      (23, 100),
      (24, 150),
      (25, 8),
      (26, 6),
      (27, 7),
      (28, 9),
    ];
    for (in_glyph, out_glyph) in expected {
      assert_eq!(lookup.value(in_glyph), ::core::result::Result::Ok(out_glyph));
    }

    for fail in [0, 10, 19, 29, 0xFFFF] {
      assert!((match &(lookup.value(fail)) { Ok(_validated_input) => false, Err(_invalid_fixture_input) => true }));
    }
  ;

    Ok(())
  }

  #[test]
  fn lookup_format_6() -> ::core::result::Result<(), crate::test_support::Failure> {
    #[rustfmt::skip]
        let words = [
            6_u16,
            4,
            4,
            16,
            2,
            0,
            50, 600,
            51, 601,
            201, 602,
            202, 900,
        ];
    let mut buf = font_test_data::bebuffer::BeBuffer::default();
    buf.extend(words);
    let lookup = crate::test_support::fixture_value(LookupU16::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf)), "fixture value")?;
    let expected = [(50, 600), (51, 601), (201, 602), (202, 900)];

    for (in_glyph, out_glyph) in expected {
      assert_eq!(lookup.value(in_glyph), ::core::result::Result::Ok(out_glyph));
    }

    for fail in [0, 10, 49, 52, 203, 0xFFFF] {
      assert!((match &(lookup.value(fail)) { Ok(_validated_input) => false, Err(_invalid_fixture_input) => true }));
    }
  ;

    Ok(())
  }

  #[test]
  fn lookup_format_8() -> ::core::result::Result<(), crate::test_support::Failure> {
    #[rustfmt::skip]
        let words = [
            8_u16,
            201,
            7,
            3, 8, 2, 9, 1, 200, 60,
        ];
    let mut buf = font_test_data::bebuffer::BeBuffer::default();
    buf.extend(words);
    let lookup = crate::test_support::fixture_value(LookupU16::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf)), "fixture value")?;
    let expected = {
      let [_, _, _, _explicit_part @ ..] = &words;
      let _explicit_slice: &[_] = _explicit_part;
      _explicit_slice
    };

    for (gid, expected_2) in (201..209).zip(expected) {
      let fixture_value = crate::test_support::fixture_value(lookup.value(gid), "fixture value")?;
      assert_eq!(fixture_value, *expected_2);
    }

    for fail in [0, 10, 200, 210, 0xFFFF] {
      assert!((match &(lookup.value(fail)) { Ok(_validated_input) => false, Err(_invalid_fixture_input) => true }));
    }
  ;

    Ok(())
  }

  #[test]
  fn lookup_format_10() -> ::core::result::Result<(), crate::test_support::Failure> {
    #[rustfmt::skip]
        let words = [
            10_u16,
            4,
            201,
            7,
        ];

    let mapped = [3_u32, 8, 2902384, 9, 1, u32::MAX, 60];
    let mut buf = font_test_data::bebuffer::BeBuffer::default();

    {
      buf.extend(words);
      buf.extend(mapped);
    };

    let lookup = crate::test_support::fixture_value(LookupU32::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf)), "fixture value")?;

    for (gid, expected) in (201..209).zip(mapped) {
      assert_eq!(lookup.value(gid), ::core::result::Result::Ok(expected));
    }

    for fail in [0, 10, 200, 210, 0xFFFF] {
      assert!((match &(lookup.value(fail)) { Ok(_validated_input) => false, Err(_invalid_fixture_input) => true }));
    }
  ;

    Ok(())
  }

  #[test]
  fn extended_state_table() -> ::core::result::Result<(), crate::read::ReadError> {
    #[rustfmt::skip]
        let header = [
            6_u32,
            20,
            56,
            92,
            0,
        ];
    #[rustfmt::skip]
        let class_table = [
            6_u16,
            4,
            5,
            16,
            2,
            0,
            50, 4,
            51, 4,
            80, 5,
            201, 4,
            202, 4,
            !0, !0
        ];
    #[rustfmt::skip]
        let state_array: [u16; 18] = [
            0, 0, 0, 0, 0, 1,
            0, 0, 0, 0, 0, 1,
            0, 0, 0, 0, 2, 1,
        ];
    #[rustfmt::skip]
        let entry_table: [u16; 12] = [
            0, 0, u16::MAX, u16::MAX,
            2, 0, u16::MAX, u16::MAX,
            0, 0, u16::MAX, 0,
        ];
    let buf = font_test_data::be_buffer! { (..(header)), (..(class_table)), (..(state_array)), (..(entry_table)) };
    let table = ExtendedStateTable::<ContextualData>::read(
      <crate::font_data::FontData<'_> as ::core::convert::From<&[::core::primitive::u8]>>::from(
        &*buf,
      ),
    )?;

    for (id, expected_class) in [(50, 4), (80, 5), (201, 4)] {
      let Ok(glyph) = GlyphId::new(id);
      let class = table.class(glyph)?;
      assert_eq!(class, expected_class);
    }

    let entry = table.entry(0, 4)?;
    assert_eq!(entry.new_state, 0);
    assert_eq!(entry.payload.current_index, !0);

    let entry_2 = table.entry(0, 5)?;
    assert_eq!(entry_2.new_state, 2);

    let entry_3 = table.entry(2, 4)?;
    assert_eq!(entry_3.new_state, 0);
    assert_eq!(entry_3.payload.current_index, 0);
    ::core::result::Result::Ok(())
  }

  #[derive(Copy, Clone, Debug, bytemuck::AnyBitPattern)]
  #[repr(C)]
  struct ContextualData {
    _mark_index: BigEndian<u16>,
    current_index: BigEndian<u16>,
  }

  const _: () = {
    assert!(core::mem::size_of::<ContextualData>() == 4);
    assert!(core::mem::align_of::<ContextualData>() == 1);
    assert!(core::mem::offset_of!(ContextualData, _mark_index) == 0);
    assert!(core::mem::offset_of!(ContextualData, current_index) == 2);
  };

  impl FixedSize for ContextualData {
    const RAW_BYTE_LEN: u64 = 4;
  }

  #[test]
  fn state_table() -> ::core::result::Result<(), crate::test_support::Failure> {
    #[rustfmt::skip]
        let header = [
            7_u16,
            10,
            18,
            40,
            64,
        ];
    #[rustfmt::skip]
        let class_table = [
            3_u16,
            4,
        ];
    let classes = [1u8, 2, 3, 4];
    #[rustfmt::skip]
        let state_array: [u8; 22] = [
            2, 0, 0, 2, 1, 0, 0,
            2, 0, 0, 2, 1, 0, 0,
            2, 3, 3, 2, 3, 4, 5,
            0,
        ];
    #[rustfmt::skip]
        let entry_table: [u16; 10] = [


            18, 0x8112,
            32, 0x8112,
            18, 0x0000,
            32, 0x8114,
            18, 0x8116,
        ];
    let buf = font_test_data::be_buffer! { (..(header)), (..(class_table)), (..(classes)), (..(state_array)), (..(entry_table)) };
    let table = crate::test_support::fixture_value(StateTable::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf)), "fixture value")?;

    for i in 0..4u8 {
      let fixture_value = crate::test_support::fixture_value(table
        .class(GlyphId16::from(::core::primitive::u16::wrapping_add(
          {
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(i);
            migrated_value
          },
          3
        ))), "fixture value")?;
      assert_eq!(
        fixture_value,
        ::core::primitive::u8::wrapping_add(i, 1)
      );
    }

    let cases = [
      ((0, 4), (2, 0x8112)),
      ((2, 1), (2, 0x8114)),
      ((1, 3), (0, 0x0000)),
      ((2, 5), (0, 0x8116)),
    ];
    for ((state, class), (new_state, flags)) in cases {
      let entry = crate::test_support::fixture_value(table.entry(state, class), "fixture value")?;
      assert_eq!(
        entry.new_state, new_state,
        "state {state}, class {class} should map to new state {new_state} (got {})",
        entry.new_state
      );
      assert_eq!(
        entry.flags, flags,
        "state {state}, class {class} should map to flags 0x{flags:X} (got 0x{:X})",
        entry.flags
      );
    }
  ;

    Ok(())
  }
}
