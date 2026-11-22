//! The [Extended Kerning (kerx)](https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6kerx.html) table.

use super::aat::{ExtendedStateTable, LookupU16, LookupU32, safe_read_array_to_end};

include!("../../generated/generated_kerx.rs");

impl VarSize for Subtable<'_> {
  type Size = u32;

  fn read_len_at(data: FontData, pos: u64) -> Result<u64, ReadError> {
    data.read_at::<u32>(pos).map(u64::from)
  }
}

impl<'a> Subtable<'a> {
  pub const HEADER_LEN: u64 = u32::RAW_BYTE_LEN * 3;

  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Subtable::is_vertical` method returns `bool` directly in the existing parser interface")
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(explicit007_no_bool_return, reason = "the published `Subtable::is_vertical` method retains the Boolean result used by existing predicate callers and implementations")
  )]
  pub fn is_vertical(&self) -> bool {
    let Ok(coverage) = self.coverage();

    coverage & 0x80000000 != 0
  }

  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Subtable::is_horizontal` method returns `bool` directly in the existing parser interface")
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(explicit007_no_bool_return, reason = "the published `Subtable::is_horizontal` method retains the Boolean result used by existing predicate callers and implementations")
  )]
  pub fn is_horizontal(&self) -> bool {
    !self.is_vertical()
  }

  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Subtable::is_cross_stream` method returns `bool` directly in the existing parser interface")
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(explicit007_no_bool_return, reason = "the published `Subtable::is_cross_stream` method retains the Boolean result used by existing predicate callers and implementations")
  )]
  pub fn is_cross_stream(&self) -> bool {
    let Ok(coverage) = self.coverage();

    coverage & 0x40000000 != 0
  }

  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Subtable::is_variable` method returns `bool` directly in the existing parser interface")
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(explicit007_no_bool_return, reason = "the published `Subtable::is_variable` method retains the Boolean result used by existing predicate callers and implementations")
  )]
  pub fn is_variable(&self) -> bool {
    let Ok(coverage) = self.coverage();

    coverage & 0x20000000 != 0
  }

  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Subtable::process_direction` method returns `bool` directly in the existing parser interface")
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(explicit007_no_bool_return, reason = "the published `Subtable::process_direction` method retains the Boolean result used by existing predicate callers and implementations")
  )]
  pub fn process_direction(&self) -> bool {
    let Ok(coverage) = self.coverage();

    coverage & 0x10000000 != 0
  }

  pub fn kind(&self) -> Result<SubtableKind<'a>, ReadError> {
    let Ok(data) = self.data();
    let Ok(value_1) = FontData::new(data);

    let Ok(coverage) = self.coverage();
    let Ok(tuple_count) = self.tuple_count();

    SubtableKind::read_with_args(value_1, &(coverage, tuple_count))
  }
}

#[derive(Clone)]
pub enum SubtableKind<'a> {
  Format0(Subtable0<'a>),
  Format1(Subtable1<'a>),
  Format2(Subtable2<'a>),
  Format4(Subtable4<'a>),
  Format6(Subtable6<'a>),
}

impl ReadArgs for SubtableKind<'_> {
  type Args = (u32, u32);
}

impl<'a> FontReadWithArgs<'a> for SubtableKind<'a> {
  fn read_with_args(data: FontData<'a>, args: &Self::Args) -> Result<Self, ReadError> {
    let format = args.0 & 0xFF;
    let tuple_count = args.1;

    match format {
      0 => {
        let read = Subtable0::read(data)?;

        Ok(Self::Format0(read))
      }
      1 => {
        let read = Subtable1::read(data)?;

        Ok(Self::Format1(read))
      }
      2 => {
        let read = Subtable2::read(data)?;

        Ok(Self::Format2(read))
      }

      4 => {
        let read = Subtable4::read(data)?;

        Ok(Self::Format4(read))
      }

      6 => {
        let with_args = Subtable6::read_with_args(data, &tuple_count)?;

        Ok(Self::Format6(with_args))
      }
      _ => Err(ReadError::InvalidFormat({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i64>(format);
        migrated_value
      })),
    }
  }
}

impl Subtable0<'_> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Subtable0::kerning` method returns the optional value directly to callers that match its presence")
  )]
  pub fn kerning(&self, left: GlyphId, right: GlyphId) -> Option<i32> {
    let Ok(pairs) = self.pairs();

    { let Ok(explicit_value) = pair_kerning(pairs, left, right); explicit_value }
  }
}

#[must_use]
pub(crate) fn pair_kerning(
  pairs: &[Subtable0Pair],
  left: GlyphId,
  right: GlyphId,
) -> core::result::Result<Option<i32>, console_core_never::Never> {
    let left_2: GlyphId16 = match match left.try_into() { Ok(value) => Some(value), Err(_left_glyph_outside_font_range) => None } { Some(explicit_value) => explicit_value, None => return Ok(None) };
    let right_2: GlyphId16 = match match right.try_into() { Ok(value) => Some(value), Err(_right_glyph_outside_font_range) => None } { Some(explicit_value) => explicit_value, None => return Ok(None) };

    #[must_use]
    fn make_key(left: GlyphId16, right: GlyphId16) -> core::result::Result<u32, console_core_never::Never> { Ok({
      let Ok(value_2) = left.to_u32();

      let Ok(value_11) = right.to_u32();

      ::core::primitive::u32::wrapping_shl(value_2, 16_u32) | value_11
    }) }

    let ::core::result::Result::Ok(idx) = ::console_core_number_conversion::fixed_index(match match pairs
      .binary_search_by_key(&({ let Ok(explicit_value) = make_key(left_2, right_2); explicit_value }), |pair| {
        let Ok(pair_left) = pair.left();
        let Ok(pair_right) = pair.right();

        { let Ok(explicit_value) = make_key(pair_left, pair_right); explicit_value }
      }) { ::core::result::Result::Ok(value___) => ::core::option::Option::Some(value___), ::core::result::Result::Err(_insertion_position) => ::core::option::Option::None } { Some(explicit_value) => explicit_value, None => return Ok(None) });

    Ok(pairs.get(match ::console_core_number_conversion::index(idx) { ::core::result::Result::Ok(native_position) => native_position }).map(|pair| {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>({
        let Ok(explicit_value) = pair.value();
        explicit_value
      });
      migrated_value
    }))
}

#[derive(Clone)]
pub struct Subtable1<'a> {
  pub state_table: ExtendedStateTable<'a, BigEndian<u16>>,
  pub values: &'a [BigEndian<i16>],
}

impl<'a> FontRead<'a> for Subtable1<'a> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    let state_table = ExtendedStateTable::read(data)?;
    let Ok(mut cursor) = data.cursor();
    let ::core::result::Result::Ok(()) = cursor.advance_by(ExtendedStateTable::<()>::HEADER_LEN);
    let values_offset = {
      let read = cursor.read::<u32>()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(read);
      migrated_value
    };
    let values = super::aat::safe_read_array_to_end(&data, values_offset)?;
    Ok(Self {
      state_table,
      values,
    })
  }
}

#[derive(Clone)]
pub struct Subtable2<'a> {
  pub data: FontData<'a>,
  pub left_offset_table: LookupU16<'a>,
  pub right_offset_table: LookupU16<'a>,
  pub array: &'a [BigEndian<i16>],
}

impl<'a> FontRead<'a> for Subtable2<'a> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    let Ok(mut cursor) = data.cursor();

    let ::core::result::Result::Ok(()) = cursor.advance_by(u32::RAW_BYTE_LEN);

    let left_offset = ({
      let read = cursor.read::<u32>()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(read);
      migrated_value
    })
    .checked_sub(Subtable::HEADER_LEN)
    .ok_or(ReadError::OutOfBounds)?;
    let right_offset = ({
      let read = cursor.read::<u32>()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(read);
      migrated_value
    })
    .checked_sub(Subtable::HEADER_LEN)
    .ok_or(ReadError::OutOfBounds)?;
    let array_offset = ({
      let read = cursor.read::<u32>()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(read);
      migrated_value
    })
    .checked_sub(Subtable::HEADER_LEN)
    .ok_or(ReadError::OutOfBounds)?;
    let Ok(value_3) = data.slice(left_offset..);

    let left_offset_table = LookupU16::read({
      let left_lookup_data = value_3.ok_or(ReadError::OutOfBounds)?;
      left_lookup_data
    })?;
    let Ok(value_4) = data.slice(right_offset..);

    let right_offset_table = LookupU16::read({
      let right_lookup_data = value_4.ok_or(ReadError::OutOfBounds)?;
      right_lookup_data
    })?;
    let array = safe_read_array_to_end(&data, array_offset)?;
    Ok(Self {
      data,
      left_offset_table,
      right_offset_table,
      array,
    })
  }
}

impl Subtable2<'_> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Subtable2::kerning` method returns the optional value directly to callers that match its presence")
  )]
  pub fn kerning(&self, left: GlyphId, right: GlyphId) -> Option<i32> {
    let Ok(value_12) = left.to_u32();

    let left_2: u16 = match value_12.try_into() {
      ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
      ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
    }?;
    let Ok(value_13) = right.to_u32();

    let right_2: u16 = match value_13.try_into() {
      ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
      ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
    }?;
    let left_idx = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(
        match self.left_offset_table.value(left_2) { Ok(value) => value, Err(_missing_left_kerning_offset) => 0 },
      );
      migrated_value
    };
    let right_idx = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(
        match self.right_offset_table.value(right_2) { Ok(value) => value, Err(_missing_right_kerning_offset) => 0 },
      );
      migrated_value
    };
    let Ok(value_16) = console_core_number_conversion::index(
      ::core::primitive::u64::wrapping_add(left_idx, right_idx),
    );

    self.array.get(value_16).map(|value| {
      let Ok(value_5) = value.get();

      {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(value_5);
        migrated_value
      }
    })
  }
}

#[derive(Clone)]
pub struct Subtable4<'a> {
  pub state_table: ExtendedStateTable<'a, BigEndian<u16>>,
  pub flags: u32,
  pub actions: Subtable4Actions<'a>,
}

impl<'a> FontRead<'a> for Subtable4<'a> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    let state_table = ExtendedStateTable::read(data)?;
    let Ok(mut cursor) = data.cursor();
    let ::core::result::Result::Ok(()) = cursor.advance_by(ExtendedStateTable::<()>::HEADER_LEN);
    let flags = cursor.read::<u32>()?;
    let action_type = ::core::primitive::u32::wrapping_shr(flags & 0xC0000000, 30_u32);
    let offset = {
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u64>(flags & 0x00FFFFFF);
      migrated_value
    };
    let actions = match action_type {
      0 => Subtable4Actions::ControlPoints({
        let short_values = safe_read_array_to_end(&data, offset)?;
        short_values
      }),
      1 => Subtable4Actions::AnchorPoints({
        let long_values = safe_read_array_to_end(&data, offset)?;
        long_values
      }),
      2 => Subtable4Actions::ControlPointCoords({
        let control_points = safe_read_array_to_end(&data, offset)?;
        control_points
      }),
      _ => {
        return Err(ReadError::MalformedData(
          "invalid action type in kerx subtable 4",
        ));
      }
    };
    Ok(Self {
      state_table,
      flags,
      actions,
    })
  }
}

#[derive(Clone)]
pub enum Subtable4Actions<'a> {
  ControlPoints(&'a [BigEndian<u16>]),
  AnchorPoints(&'a [BigEndian<u16>]),
  ControlPointCoords(&'a [BigEndian<i16>]),
}

#[derive(Clone)]
pub enum Subtable6<'a> {
  ShortValues(
    LookupU16<'a>,
    LookupU16<'a>,
    &'a [BigEndian<i16>],
    Option<&'a [BigEndian<i16>]>,
  ),
  LongValues(
    LookupU32<'a>,
    LookupU32<'a>,
    &'a [BigEndian<i32>],
    Option<&'a [BigEndian<i16>]>,
  ),
}

impl ReadArgs for Subtable6<'_> {
  type Args = u32;
}

impl<'a> FontReadWithArgs<'a> for Subtable6<'a> {
  fn read_with_args(data: FontData<'a>, args: &Self::Args) -> Result<Self, ReadError> {
    let tuple_count = *args;
    let Ok(mut cursor) = data.cursor();
    let flags = cursor.read::<u32>()?;

    let ::core::result::Result::Ok(()) =
      cursor.advance_by(::core::primitive::u64::wrapping_mul(u16::RAW_BYTE_LEN, 2));

    let row_index_table_offset = ({
      let read = cursor.read::<u32>()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(read);
      migrated_value
    })
    .checked_sub(Subtable::HEADER_LEN)
    .ok_or(ReadError::OutOfBounds)?;
    let column_index_table_offset = ({
      let read = cursor.read::<u32>()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(read);
      migrated_value
    })
    .checked_sub(Subtable::HEADER_LEN)
    .ok_or(ReadError::OutOfBounds)?;
    let kerning_array_offset = ({
      let read = cursor.read::<u32>()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(read);
      migrated_value
    })
    .checked_sub(Subtable::HEADER_LEN)
    .ok_or(ReadError::OutOfBounds)?;
    let kerning_vector = match ::core::primitive::bool::then_some(tuple_count != 0, ()) {
      ::core::option::Option::Some(()) => {
        let kerning_vector_offset = ({
          let read = cursor.read::<u32>()?;
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(read);
          migrated_value
        })
        .checked_sub(Subtable::HEADER_LEN)
        .ok_or(ReadError::OutOfBounds)?;
        Some({
          let kerning_vector = safe_read_array_to_end(&data, kerning_vector_offset)?;
          kerning_vector
        })
      }
      ::core::option::Option::None => None,
    };
    let Ok(value_6) = data.slice(row_index_table_offset..);

    let row_data = value_6.ok_or(ReadError::OutOfBounds)?;
    let Ok(value_7) = data.slice(column_index_table_offset..);

    let column_data = value_7.ok_or(ReadError::OutOfBounds)?;

    match ::core::primitive::bool::then_some(flags & 1 == 0, ()) {
      ::core::option::Option::Some(()) => {
        let rows = LookupU16::read(row_data)?;
        let columns = LookupU16::read(column_data)?;
        let kerning_array = safe_read_array_to_end(&data, kerning_array_offset)?;
        Ok(Self::ShortValues(
          rows,
          columns,
          kerning_array,
          kerning_vector,
        ))
      }
      ::core::option::Option::None => {
        let rows = LookupU32::read(row_data)?;
        let columns = LookupU32::read(column_data)?;
        let kerning_array = safe_read_array_to_end(&data, kerning_array_offset)?;
        Ok(Self::LongValues(
          rows,
          columns,
          kerning_array,
          kerning_vector,
        ))
      }
    }
  }
}

impl Subtable6<'_> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Subtable6::kerning` method returns the optional value directly to callers that match its presence")
  )]
  pub fn kerning(&self, left: GlyphId, right: GlyphId) -> Option<i32> {
    let Ok(value_14) = left.to_u32();

    let left_2: u16 = match value_14.try_into() {
      ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
      ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
    }?;
    let Ok(value_15) = right.to_u32();

    let right_2: u16 = match value_15.try_into() {
      ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
      ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
    }?;

    #[must_use]
    fn tuple_kern(value: i32, vector: &Option<&[BigEndian<i16>]>) -> core::result::Result<Option<i32>, console_core_never::Never> {
        Ok(match vector {
          Some(values) => {
            let Ok(value_17) =
              console_core_number_conversion::index(::core::primitive::u64::wrapping_shr(
                {
                  let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value);
                  migrated_value
                },
                1_u32,
              ));

            values.get(value_17).map(|adjustment| {
              let Ok(value_8) = adjustment.get();

              {
                let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(value_8);
                migrated_value
              }
            })
          }
          None => Some(value),
        })
    }

    match self {
      Self::ShortValues(rows, columns, array, vector) => {
        let left_idx = match rows.value(left_2) { Ok(value) => value, Err(_missing_left_kerning_class) => Default::default() };
        let right_idx = match columns.value(right_2) { Ok(value) => value, Err(_missing_right_kerning_class) => Default::default() };
        let idx = ::core::primitive::u64::wrapping_add(
          {
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(left_idx);
            migrated_value
          },
          {
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(right_idx);
            migrated_value
          },
        );
        let Ok(value_18) = console_core_number_conversion::index(idx);

        let value = array.get(value_18).map(|value| {
          let Ok(value_9) = value.get();

          {
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(value_9);
            migrated_value
          }
        })?;

        { let Ok(explicit_value) = tuple_kern(value, vector); explicit_value }
      }
      Self::LongValues(rows, columns, array, vector) => {
        let left_idx = match rows.value(left_2) { Ok(value) => value, Err(_missing_left_kerning_class) => Default::default() };
        let right_idx = match columns.value(right_2) { Ok(value) => value, Err(_missing_right_kerning_class) => Default::default() };
        let idx = ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(left_idx);
          migrated_value
        })
        .checked_add({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(right_idx);
          migrated_value
        })?;
        let Ok(value_19) = console_core_number_conversion::index(idx);

        let value = array.get(value_19).map(|value| {
          let Ok(value_21) = value.get();

          value_21
        })?;
        let value_10 = value;

        { let Ok(explicit_value) = tuple_kern(value_10, vector); explicit_value }
      }
    }
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for Subtable<'a> {
  fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
    Ok(RecordResolver {
      name: "Subtable",
      get_field: Box::new(move |idx, _data| match idx {
        0u64 => {
          let Ok(field) = Field::new("coverage", self.coverage());

          Some(field)
        }
        1u64 => {
          let Ok(field) = Field::new("tuple_count", self.tuple_count());

          Some(field)
        }
        _ => None,
      }),
      data,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_subtable0() -> ::core::result::Result<(), crate::test_support::Failure> {
    let mut buf = font_test_data::bebuffer::BeBuffer::default();

    buf.extend([6u32, 0, 0, 0]);

    let mut pairs = [
      (0u32, 1u32, -10i32),
      (2, 4, 22),
      (0, 3, -6),
      (8, 2, 500),
      (10, 1, 42),
      (9, 12, -1000),
    ];

    pairs.sort_by_key(|pair| ::core::primitive::u32::wrapping_shl(pair.0, 16_u32) | pair.1);

    for pair in &pairs {
      {
        buf.extend([({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(pair.0);
          migrated_value
        })]);
        buf.extend([({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(pair.1);
          migrated_value
        })]);
        buf.extend([({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i16>(pair.2);
          migrated_value
        })]);
      };
    }

    let data = buf.to_vec();
    let subtable0 = crate::test_support::fixture_value(Subtable0::read({
      let Ok(explicit_value) = FontData::new(&data);
      explicit_value
    }), "fixture value")?;

    for pair in pairs {
      assert_eq!(
        subtable0.kerning(
          <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(pair.0),
          <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(pair.1)
        ),
        Some(pair.2)
      );
    }
  ;

    Ok(())
  }

  #[test]
  fn parse_subtable1() -> ::core::result::Result<(), crate::read::ReadError> {
    let Ok(data) = FormatOneFour::One.build_subtable();
    let subtable1 = Subtable1::read({
      let Ok(explicit_value) = FontData::new(&data);
      explicit_value
    })?;
    let values = subtable1
      .values
      .iter()
      .take(ONE_EXPECTED.len())
      .map(|value| {
        let Ok(value_2) = value.get();
        value_2
      })
      .collect::<Vec<_>>();
    assert_eq!(values, &ONE_EXPECTED);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_subtable2() -> ::core::result::Result<(), crate::test_support::Failure> {
    let Ok(data) = FormatTwoSix::Two.build_subtable();
    let subtable = Subtable2::read({
      let Ok(explicit_value) = FontData::new(&data);
      explicit_value
    })?;
    let mut values = vec![];

    for left in 0u32..4 {
      for right in 0u32..4 {
        let kerning = match subtable.kerning(
          <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(left),
          <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(right),
        ) {
          Some(kerning) => kerning,
          None => {
            return Err(crate::test_support::Failure::from(alloc::format!("expected kerning value for {left} and {right}")));
          }
        };
        values.push(kerning);
      }
    }

    assert_eq!(values, &TWO_SIX_EXPECTED);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_subtable4_control_points() -> ::core::result::Result<(), crate::test_support::Failure> {
    let Ok(data) = FormatOneFour::FourControlPoints.build_subtable();
    let subtable4 = Subtable4::read({
      let Ok(explicit_value) = FontData::new(&data);
      explicit_value
    })?;

    let action = match &subtable4.actions {
        Subtable4Actions::ControlPoints(matched_action) => matched_action,
        crate::tables::kerx::Subtable4Actions::AnchorPoints(..) | crate::tables::kerx::Subtable4Actions::ControlPointCoords(..) => return Err(crate::test_support::Failure::from(alloc::format!("expected subtable 4 control points action"))),
    };

    let values = action
      .as_chunks::<2>()
      .0
      .iter()
      .take(FOUR_OUTLINE_ANKR_EXPECTED.len())
      .map(|values| {
        (
          ({
            let Ok(value) = (*match &*values {
              [element, ..] => element,
            })
            .get();
            value
          }),
          ({
            let Ok(value) = (*match &*values {
              [_, element, ..] => element,
            })
            .get();
            value
          }),
        )
      })
      .collect::<Vec<_>>();
    assert_eq!(values, &FOUR_OUTLINE_ANKR_EXPECTED);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_subtable4_anchor_points() -> ::core::result::Result<(), crate::test_support::Failure> {
    let Ok(data) = FormatOneFour::FourAnchorPoints.build_subtable();
    let subtable4 = Subtable4::read({
      let Ok(explicit_value) = FontData::new(&data);
      explicit_value
    })?;

    let action = match &subtable4.actions {
        Subtable4Actions::AnchorPoints(matched_action) => matched_action,
        crate::tables::kerx::Subtable4Actions::ControlPoints(..) | crate::tables::kerx::Subtable4Actions::ControlPointCoords(..) => return Err(crate::test_support::Failure::from(alloc::format!("expected subtable 4 anchor points action"))),
    };

    let values = action
      .as_chunks::<2>()
      .0
      .iter()
      .take(FOUR_OUTLINE_ANKR_EXPECTED.len())
      .map(|values| {
        (
          ({
            let Ok(value) = (*match &*values {
              [element, ..] => element,
            })
            .get();
            value
          }),
          ({
            let Ok(value) = (*match &*values {
              [_, element, ..] => element,
            })
            .get();
            value
          }),
        )
      })
      .collect::<Vec<_>>();
    assert_eq!(values, &FOUR_OUTLINE_ANKR_EXPECTED);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_subtable4_coords() -> ::core::result::Result<(), crate::test_support::Failure> {
    let Ok(data) = FormatOneFour::FourCoords.build_subtable();
    let subtable4 = Subtable4::read({
      let Ok(explicit_value) = FontData::new(&data);
      explicit_value
    })?;

    let action = match &subtable4.actions {
        Subtable4Actions::ControlPointCoords(matched_action) => matched_action,
        crate::tables::kerx::Subtable4Actions::ControlPoints(..) | crate::tables::kerx::Subtable4Actions::AnchorPoints(..) => return Err(crate::test_support::Failure::from(alloc::format!("expected subtable 4 coords action"))),
    };

    let values = action
      .as_chunks::<4>()
      .0
      .iter()
      .take(FOUR_COORDS_EXPECTED.len())
      .map(|values| {
        [
          ({
            let Ok(value) = (*match &*values {
              [element, ..] => element,
            })
            .get();
            value
          }),
          ({
            let Ok(value) = (*match &*values {
              [_, element, ..] => element,
            })
            .get();
            value
          }),
          ({
            let Ok(value) = (*match &*values {
              [_, _, element, ..] => element,
            })
            .get();
            value
          }),
          ({
            let Ok(value) = (*match &*values {
              [_, _, _, element, ..] => element,
            })
            .get();
            value
          }),
        ]
      })
      .collect::<Vec<_>>();
    assert_eq!(values, &FOUR_COORDS_EXPECTED);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_subtable6_short() -> ::core::result::Result<(), crate::test_support::Failure> {
    let Ok(data) = FormatTwoSix::SixShort.build_subtable();
    let subtable = Subtable6::read_with_args(
      {
        let Ok(explicit_value) = FontData::new(&data);
        explicit_value
      },
      &0,
    )?;

    match &subtable {
        Subtable6::ShortValues(..) => (),
        crate::tables::kerx::Subtable6::LongValues(..) => return Err(crate::test_support::Failure::from(alloc::format!("expected short values in subtable 6"))),
    };

    check_subtable6(subtable)?;
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_subtable6_long() -> ::core::result::Result<(), crate::test_support::Failure> {
    let Ok(data) = FormatTwoSix::SixLong.build_subtable();
    let subtable = Subtable6::read_with_args(
      {
        let Ok(explicit_value) = FontData::new(&data);
        explicit_value
      },
      &0,
    )?;

    match &subtable {
        Subtable6::LongValues(..) => (),
        crate::tables::kerx::Subtable6::ShortValues(..) => return Err(crate::test_support::Failure::from(alloc::format!("expected long values in subtable 6"))),
    };

    check_subtable6(subtable)?;
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_subtable6_long_vector() -> ::core::result::Result<(), crate::test_support::Failure> {
    let Ok(data) = FormatTwoSix::SixLongVector.build_subtable();
    let subtable = Subtable6::read_with_args(
      {
        let Ok(explicit_value) = FontData::new(&data);
        explicit_value
      },
      &1,
    )?;

    match &subtable {
        Subtable6::LongValues(..) => (),
        crate::tables::kerx::Subtable6::ShortValues(..) => return Err(crate::test_support::Failure::from(alloc::format!("expected long values in subtable 6"))),
    };

    check_subtable6(subtable)?;
    ::core::result::Result::Ok(())
  }

  fn check_subtable6(subtable: Subtable6) -> Result<(), crate::test_support::Failure> {
    let mut values = vec![];

    for left in 0u32..4 {
      for right in 0u32..4 {
        let kerning = match subtable.kerning(
          <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(left),
          <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(right),
        ) {
          Some(kerning) => kerning,
          None => {
            return Err(crate::test_support::Failure::from(format!("expected kerning value for {left} and {right}")));
          }
        };
        values.push(kerning);
      }
    }

    assert_eq!(values, &TWO_SIX_EXPECTED);
    ::core::result::Result::Ok(())
  }

  const ONE_EXPECTED: [i16; 8] = [-40, -20, -10, 0, 10, 20, 40, 80];

  const FOUR_OUTLINE_ANKR_EXPECTED: [(u16, u16); 4] = [(0, 2), (2, 4), (4, 8), (8, 16)];

  const FOUR_COORDS_EXPECTED: [[i16; 4]; 4] = [
    [-10, 10, -20, 20],
    [1, 2, 3, 4],
    [-1, -2, -3, -4],
    [10, -10, 20, -20],
  ];

  enum FormatOneFour {
    One,
    FourControlPoints,
    FourAnchorPoints,
    FourCoords,
  }

  impl FormatOneFour {
    #[must_use]
    fn build_subtable(&self) -> core::result::Result<Vec<u8>, console_core_never::Never> {
        let mut flags_offset = ::core::primitive::u64::wrapping_add(
          ExtendedStateTable::<()>::HEADER_LEN,
          u32::RAW_BYTE_LEN,
        );

        match self {
          Self::FourAnchorPoints => {
            flags_offset |= ::core::primitive::u64::wrapping_shl(1, 30_u32);
          }
          Self::FourCoords => {
            flags_offset |= ::core::primitive::u64::wrapping_shl(2, 30_u32);
          }
          Self::One | Self::FourControlPoints => {}
        }

        let mut buf = font_test_data::bebuffer::BeBuffer::default();
        buf.extend([({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(flags_offset);
          migrated_value
        })]);

        match self {
          Self::One => {
            buf.extend(ONE_EXPECTED);
          }
          Self::FourControlPoints | Self::FourAnchorPoints => {
            for indices in FOUR_OUTLINE_ANKR_EXPECTED {
              {
                buf.extend([indices.0]);
                buf.extend([indices.1]);
              };
            }
          }
          Self::FourCoords => {
            for coords in FOUR_COORDS_EXPECTED {
              buf.extend(coords);
            }
          }
        }

        let payload = buf.to_vec();
        let payload_len = {
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(payload.len());
          migrated_value
        };
        #[rustfmt::skip]
              let header = [
                  6_u32,
                  ::core::primitive::u32::wrapping_add(payload_len, 16),
                  ::core::primitive::u32::wrapping_add(payload_len, 52),
                  ::core::primitive::u32::wrapping_add(payload_len, 88),
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
              let entry_table: [u16; 9] = [
                  0, 0, 1,
                  2, 0, 2,
                  0, 0, 3,
              ];

        Ok(font_test_data::be_buffer! { (..(header)), (..(payload)), (..(class_table)), (..(state_array)), (..(entry_table)) }
                  .to_vec())
    }
  }

  const TWO_SIX_EXPECTED: [i32; 16] =
    [0i32, 10, 20, 0, 8, 4, -2, 8, 30, -10, -20, 30, 8, 4, -2, 8];

  enum FormatTwoSix {
    Two,
    SixShort,
    SixLong,
    SixLongVector,
  }

  #[derive(Clone, Copy)]
  enum KerningValueWidth {
    Short,
    Long,
  }

  impl FormatTwoSix {
    #[must_use]
    fn build_subtable(&self) -> core::result::Result<Vec<u8>, console_core_never::Never> {
        let mut buf = font_test_data::bebuffer::BeBuffer::default();
        let row_count = 3u32;
        let column_count = 3u32;
        let value_width = match self {
          Self::Two | Self::SixShort => KerningValueWidth::Short,
          Self::SixLong | Self::SixLongVector => KerningValueWidth::Long,
        };
        let has_kerning_vector = matches!(self, Self::SixLongVector);

        match ::core::primitive::bool::then_some(matches!(self, Self::SixShort | Self::SixLong | Self::SixLongVector), ()) {
          ::core::option::Option::Some(()) => {
            {
              buf.extend([match value_width {
                KerningValueWidth::Long => 1u32,
                KerningValueWidth::Short => 0u32,
              }]);
              buf.extend([({
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped::<_, u16>(row_count);
                migrated_value
              })]);
              buf.extend([({
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped::<_, u16>(column_count);
                migrated_value
              })]);
            };
          }
          ::core::option::Option::None => {
            buf.extend([row_count]);
          }
        }

        let ::core::result::Result::Ok(row_table) = build_lookup(
          &[
            0,
            ::core::primitive::u32::wrapping_mul(2, column_count),
            column_count,
            ::core::primitive::u32::wrapping_mul(2, column_count),
          ],
          value_width,
        );
        let ::core::result::Result::Ok(column_table) = build_lookup(&[0, 1, 2, 0], value_width);

        let kerning_array = [0i32, 10, 20, 30, -10, -20, 8, 4, -2];
        let mut offset = ::core::primitive::u64::wrapping_add(
          Subtable::HEADER_LEN,
          ::core::primitive::u64::wrapping_mul(
            u32::RAW_BYTE_LEN,
            match ::core::primitive::bool::then_some(matches!(self, Self::SixShort | Self::SixLong | Self::SixLongVector), ()) {
              ::core::option::Option::Some(()) => 5,
              ::core::option::Option::None => 4,
            },
          ),
        );

        match ::core::primitive::bool::then_some(has_kerning_vector, ()) {
          ::core::option::Option::Some(()) => {
            offset = ::core::primitive::u64::wrapping_add(offset, 4);
          }
          ::core::option::Option::None => {}
        }

        buf.extend([({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(offset);
          migrated_value
        })]);
        match (
          {
            let Ok(explicit_index) =
              console_core_number_conversion::fitted::<_, u64>(row_table.len());
            explicit_index
          },
          &mut offset,
        ) {
          (explicit_arithmetic_amount, explicit_arithmetic_place) => {
            *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
              *explicit_arithmetic_place,
              explicit_arithmetic_amount,
            )
          }
        };

        buf.extend([({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(offset);
          migrated_value
        })]);
        match (
          {
            let Ok(explicit_index) =
              console_core_number_conversion::fitted::<_, u64>(column_table.len());
            explicit_index
          },
          &mut offset,
        ) {
          (explicit_arithmetic_amount, explicit_arithmetic_place) => {
            *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
              *explicit_arithmetic_place,
              explicit_arithmetic_amount,
            )
          }
        };

        buf.extend([({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(offset);
          migrated_value
        })]);
        match ::core::primitive::bool::then_some(has_kerning_vector, ()) {
          ::core::option::Option::Some(()) => {
            match (::core::primitive::u64::wrapping_mul(9, 4), &mut offset) {
              (explicit_arithmetic_amount, explicit_arithmetic_place) => {
                *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
                  *explicit_arithmetic_place,
                  explicit_arithmetic_amount,
                )
              }
            };

            buf.extend([({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(offset);
              migrated_value
            })]);
            buf.extend(row_table);
            buf.extend(column_table);

            let offsets: [u32; 9] = core::array::from_fn(|idx| {
              ::core::primitive::u32::wrapping_mul(
                {
                  let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(idx);
                  migrated_value
                },
                2,
              )
            });
            buf.extend(offsets);

            for value in &kerning_array {
              buf.extend([({
                let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i16>(*value);
                migrated_value
              })]);
            }
          }
          ::core::option::Option::None => {
            buf.extend(row_table);
            buf.extend(column_table);

            match value_width {
              KerningValueWidth::Long => {
                buf.extend(kerning_array);
              }
              KerningValueWidth::Short => {
                for &value in &kerning_array {
                  let Ok(encoded) = console_core_number_conversion::wrapped::<_, i16>(value);
                  buf.extend([encoded]);
                }
              }
            }
          }
        }

        Ok(buf.to_vec())
    }
  }

  fn build_lookup(
    values: &[u32],
    value_width: KerningValueWidth,
  ) -> ::core::result::Result<Vec<u8>, ::console_core_never::Never> {
    let mut buf = font_test_data::bebuffer::BeBuffer::default();

    buf.extend([0u16]);

    for value in values {
      match value_width {
        KerningValueWidth::Long => {
          buf.extend([*value]);
        }
        KerningValueWidth::Short => {
          buf.extend([({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(*value);
            migrated_value
          })]);
        }
      }
    }

    ::core::result::Result::Ok(buf.to_vec())
  }
}
