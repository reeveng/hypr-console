//! The kerning table.

use super::aat::StateTable;
pub use super::kerx::Subtable0Pair;

include!("../../generated/generated_kern.rs");

#[derive(Clone)]
pub enum Kern<'a> {
  Ot(OtKern<'a>),
  Aat(AatKern<'a>),
}

impl TopLevelTable for Kern<'_> {
  const TAG: Tag = ({
    let Ok(value) = Tag::new(b"kern");
    value
  });
}

impl<'a> FontRead<'a> for Kern<'a> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    let at = data.read_at::<u16>(0)?;

    match ::core::primitive::bool::then_some(at == 0, ()) {
      ::core::option::Option::Some(()) => OtKern::read(data).map(Self::Ot),
      ::core::option::Option::None => AatKern::read(data).map(Self::Aat),
    }
  }
}

impl<'a> Kern<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Kern::subtables` method returns its iterator or trait value directly for existing callers to compose")
  )]
  pub fn subtables(
    &self,
  ) -> impl Iterator<Item = Result<Subtable<'a>, ReadError>> + 'a + Clone + use<'a> {
    let (data, is_aat, n_tables) = match self {
      Self::Ot(table) => {
        let Ok(data) = table.subtable_data();
        let Ok(n_tables) = table.n_tables();

        (data, false, u32::from(n_tables))
      }
      Self::Aat(table) => {
        let Ok(data) = table.subtable_data();
        let Ok(n_tables) = table.n_tables();

        (data, true, n_tables)
      }
    };
    let Ok(data_2) = FontData::new(data);
    Subtables {
      data: data_2,
      is_aat,
      n_tables,
    }
  }
}

#[derive(Clone)]
struct Subtables<'a> {
  data: FontData<'a>,
  is_aat: bool,
  n_tables: u32,
}

impl<'a> Iterator for Subtables<'a> {
  type Item = Result<Subtable<'a>, ReadError>;

  fn next(&mut self) -> Option<Self::Item> {
    let Ok(remaining) = self.data.is_empty();

    match (self.n_tables, remaining) {
      (0, _) | (_, crate::Emptiness::Empty) => return None,
      (_, crate::Emptiness::NonEmpty) => {}
    }

    match (1, &mut self.n_tables) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u32::wrapping_sub(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };

    let len = match ::core::primitive::bool::then_some(self.is_aat, ()) {
      ::core::option::Option::Some(()) => {
        let at = match self.data.read_at::<u32>(0) {
          ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
          ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
        }?;
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(at);
        migrated_value
      }
      ::core::option::Option::None => match ::core::primitive::bool::then_some(self.n_tables == 0, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(value_17) = self.data.len();

          value_17
        }
        ::core::option::Option::None => {
          let at = match self.data.read_at::<u16>(2) {
            ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
            ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
          }?;
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(at);
          migrated_value
        }
      },
    };

    match ::core::primitive::bool::then_some(len == 0, ()) {
      ::core::option::Option::Some(()) => {
        return None;
      }
      ::core::option::Option::None => {}
    }

    let Ok(data) = self.data.take_up_to(len);
    let data_2 = data?;
    Some(Subtable::read_with_args(data_2, &self.is_aat))
  }
}

impl OtSubtable<'_> {
  const HEADER_LEN: u64 = u16::RAW_BYTE_LEN * 3;
}

impl AatSubtable<'_> {
  const HEADER_LEN: u64 = u32::RAW_BYTE_LEN + u16::RAW_BYTE_LEN * 2;
}

#[derive(Clone)]
pub enum Subtable<'a> {
  Ot(OtSubtable<'a>),
  Aat(AatSubtable<'a>),
}

impl ReadArgs for Subtable<'_> {
  type Args = bool;
}

impl<'a> FontReadWithArgs<'a> for Subtable<'a> {
  fn read_with_args(data: FontData<'a>, args: &Self::Args) -> Result<Self, ReadError> {
    let is_aat = *args;

    match ::core::primitive::bool::then_some(is_aat, ()) {
      ::core::option::Option::Some(()) => {
        let read = AatSubtable::read(data)?;

        Ok(Self::Aat(read))
      }
      ::core::option::Option::None => {
        let read = OtSubtable::read(data)?;

        Ok(Self::Ot(read))
      }
    }
  }
}

impl<'a> Subtable<'a> {
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
    match self {
      Self::Ot(subtable) => {
        let Ok(coverage) = subtable.coverage();

        coverage & ::core::primitive::u16::wrapping_shl(1, 0_u32) == 0
      }
      Self::Aat(subtable) => {
        let Ok(coverage) = subtable.coverage();

        coverage & 0x8000 != 0
      }
    }
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
    match self {
      Self::Ot(subtable) => {
        let Ok(coverage) = subtable.coverage();

        coverage & ::core::primitive::u16::wrapping_shl(1, 2_u32) != 0
      }
      Self::Aat(subtable) => {
        let Ok(coverage) = subtable.coverage();

        coverage & 0x4000 != 0
      }
    }
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
    match self {
      Self::Ot(_) => false,
      Self::Aat(subtable) => {
        let Ok(coverage) = subtable.coverage();

        coverage & 0x2000 != 0
      }
    }
  }

  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Subtable::is_state_machine` method returns `bool` directly in the existing parser interface")
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(explicit007_no_bool_return, reason = "the published `Subtable::is_state_machine` method retains the Boolean result used by existing predicate callers and implementations")
  )]
  pub fn is_state_machine(&self) -> bool {
    let Ok(data_and_format) = self.data_and_format();

    data_and_format.1 == 1
  }

  pub fn kind(&self) -> Result<SubtableKind<'a>, ReadError> {
    let Ok((data, format)) = self.data_and_format();
    let is_aat = matches!(self, Self::Aat(_));
    let Ok(value_2) = FontData::new(data);

    SubtableKind::read_with_args(value_2, &(format, is_aat))
  }

  #[must_use]
  fn data_and_format(&self) -> core::result::Result<(&'a [u8], u8), console_core_never::Never> {
      Ok(match self {
        Self::Ot(subtable) => {
          let Ok(data) = subtable.data();

          (
            data,
            ({
              let Ok(coverage) = subtable.coverage();
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(
                ::core::primitive::u16::wrapping_shr(coverage & 0xFF00, 8_u32),
              );
              migrated_value
            }),
          )
        }
        Self::Aat(subtable) => {
          let Ok(data) = subtable.data();

          (
            data,
            ({
              let Ok(coverage) = subtable.coverage();
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(coverage);
              migrated_value
            }),
          )
        }
      })
  }
}

#[derive(Clone)]
pub enum SubtableKind<'a> {
  Format0(Subtable0<'a>),
  Format1(StateTable<'a>),
  Format2(Subtable2<'a>),
  Format3(Subtable3<'a>),
}

impl ReadArgs for SubtableKind<'_> {
  type Args = (u8, bool);
}

impl<'a> FontReadWithArgs<'a> for SubtableKind<'a> {
  fn read_with_args(data: FontData<'a>, args: &Self::Args) -> Result<Self, ReadError> {
    let (format, is_aat) = *args;

    match format {
      0 => {
        let read = Subtable0::read(data)?;

        Ok(Self::Format0(read))
      }
      1 => {
        let read = StateTable::read(data)?;

        Ok(Self::Format1(read))
      }
      2 => {
        let header_len = match ::core::primitive::bool::then_some(is_aat, ()) {
          ::core::option::Option::Some(()) => AatSubtable::HEADER_LEN,
          ::core::option::Option::None => OtSubtable::HEADER_LEN,
        };
        let with_args = Subtable2::read_with_args(data, &header_len)?;

        Ok(Self::Format2(with_args))
      }
      3 => {
        let read = Subtable3::read(data)?;

        Ok(Self::Format3(read))
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

    let Ok(pair_kerning) = super::kerx::pair_kerning(pairs, left, right);

    pair_kerning
  }
}

#[derive(Clone)]
pub struct Subtable2<'a> {
  pub data: FontData<'a>,
  pub header_len: u64,
  pub left_offset_table: Subtable2ClassTable<'a>,
  pub right_offset_table: Subtable2ClassTable<'a>,
  pub array_offset: u64,
}

impl ReadArgs for Subtable2<'_> {
  type Args = u64;
}

impl<'a> FontReadWithArgs<'a> for Subtable2<'a> {
  fn read_with_args(data: FontData<'a>, args: &Self::Args) -> Result<Self, ReadError> {
    let Ok(mut cursor) = data.cursor();
    let header_len = *args;

    let ::core::result::Result::Ok(()) = cursor.advance_by(u16::RAW_BYTE_LEN);

    let left_offset = ({
      let read = cursor.read::<u16>()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(read);
      migrated_value
    })
    .checked_sub(header_len)
    .ok_or(ReadError::OutOfBounds)?;
    let right_offset = ({
      let read = cursor.read::<u16>()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(read);
      migrated_value
    })
    .checked_sub(header_len)
    .ok_or(ReadError::OutOfBounds)?;
    let array_offset = ({
      let read = cursor.read::<u16>()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(read);
      migrated_value
    })
    .checked_sub(header_len)
    .ok_or(ReadError::OutOfBounds)?;
    let Ok(value_3) = data.slice(left_offset..);

    let left_offset_table = Subtable2ClassTable::read({
      let left_class_data = value_3.ok_or(ReadError::OutOfBounds)?;
      left_class_data
    })?;
    let Ok(value_4) = data.slice(right_offset..);

    let right_offset_table = Subtable2ClassTable::read({
      let right_class_data = value_4.ok_or(ReadError::OutOfBounds)?;
      right_class_data
    })?;
    Ok(Self {
      data,
      header_len,
      left_offset_table,
      right_offset_table,
      array_offset,
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
    let left_offset = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(
        match { let Ok(explicit_value) = self.left_offset_table.value(left); explicit_value } {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => 0,
        },
      );
      migrated_value
    };
    let right_offset = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(
        match { let Ok(explicit_value) = self.right_offset_table.value(right); explicit_value } {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => 0,
        },
      );
      migrated_value
    };

    let left_offset_2 = left_offset.checked_sub(self.header_len)?;

    match ::core::primitive::bool::then_some(left_offset_2 < self.array_offset, ()) {
      ::core::option::Option::Some(()) => {
        return None;
      }
      ::core::option::Option::None => {}
    }

    let offset = left_offset_2.checked_add(right_offset)?;
    match self.data.read_at::<i16>(offset) {
      ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
      ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
    }
    .map(|value| {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(value);
      migrated_value
    })
  }
}

impl Subtable2ClassTable<'_> {
  #[must_use]
  fn value(&self, glyph_id: GlyphId) -> core::result::Result<Option<u16>, console_core_never::Never> {
      let Ok(value_10) = glyph_id.to_u32();

      let glyph_id_2: u16 = match match value_10.try_into() {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
      } { Some(explicit_value) => explicit_value, None => return Ok(None) };
      let Ok(first_glyph) = self.first_glyph();
      let Ok(value_5) = first_glyph.to_u16();

      let index = match glyph_id_2.checked_sub(value_5) { Some(explicit_value) => explicit_value, None => return Ok(None) };
      let Ok(value_12) = console_core_number_conversion::index({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(index);
        migrated_value
      });

      let Ok(offsets) = self.offsets();

      Ok(offsets.get(value_12).map(|offset| {
        let Ok(value_18) = offset.get();

        value_18
      }))
  }
}

impl Subtable3<'_> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Subtable3::kerning` method returns the optional value directly to callers that match its presence")
  )]
  pub fn kerning(&self, left: GlyphId, right: GlyphId) -> Option<i32> {
    let Ok(value_6) = left.to_u32();

    let Ok(value_13) = console_core_number_conversion::index({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_6);
      migrated_value
    });

    let left_class = {
      let own_left_class = ({
        let Ok(explicit_value) = self.left_class();
        explicit_value
      })
      .get(value_13)
      .copied()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(own_left_class);
      migrated_value
    };
    let Ok(value_7) = right.to_u32();

    let Ok(value_14) = console_core_number_conversion::index({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_7);
      migrated_value
    });

    let right_class = {
      let own_right_class = ({
        let Ok(explicit_value) = self.right_class();
        explicit_value
      })
      .get(value_14)
      .copied()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(own_right_class);
      migrated_value
    };
    let Ok(value_15) =
      console_core_number_conversion::index(::core::primitive::u64::wrapping_add(
        ::core::primitive::u64::wrapping_mul(left_class, {
          let Ok(right_class_count) = self.right_class_count();
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(right_class_count);
          migrated_value
        }),
        right_class,
      ));

    let index = {
      let Ok(own_kern_index) = self.kern_index();
      let kern_index = own_kern_index.get(value_15).copied()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(kern_index);
      migrated_value
    };
    let Ok(value_16) = console_core_number_conversion::index(index);

    let Ok(kern_value) = self.kern_value();

    kern_value.get(value_16).map(|value| {
      let Ok(value_8) = value.get();

      {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(value_8);
        migrated_value
      }
    })
  }
}

#[cfg(test)]
mod tests {

  use super::*;

  #[test]
  fn ot_format_0() -> ::core::result::Result<(), crate::test_support::Failure> {
    #[rustfmt::skip]
        const KERN_VER_0_FMT_0_DATA: &[u8] = &[
            0x00, 0x00,
            0x00, 0x01,
            0x00, 0x00,
            0x00, 0x20,
            0x00,
            0x01,
            0x00, 0x03,
            0x00, 0x0C,
            0x00, 0x01,
            0x00, 0x06,
            0x00, 0x04, 0x00, 0x0C, 0xFF, 0xD8,
            0x00, 0x04, 0x00, 0x1C, 0x00, 0x28,
            0x00, 0x05, 0x00, 0x28, 0xFF, 0xCE,
        ];
    let kern = crate::test_support::fixture_value(Kern::read({
      let Ok(explicit_value) = FontData::new(KERN_VER_0_FMT_0_DATA);
      explicit_value
    }), "fixture value")?;

    let ot_kern = match &kern {
        Kern::Ot(matched_ot_kern) => matched_ot_kern,
        crate::tables::kern::Kern::Aat(..) => return Err(crate::test_support::Failure::from(alloc::format!("Should be an OpenType kerning table"))),
    };

    assert_eq!(ot_kern.version(), Ok(0));
    assert_eq!(ot_kern.n_tables(), Ok(1));
    let subtables = kern.subtables().collect::<Vec<_>>();
    assert_eq!(subtables.len(), 1);
    let fixture_value = crate::test_support::fixture_value(subtables.first(), "fixture value")?;
    let subtable = crate::test_support::fixture_value(fixture_value.as_ref(), "fixture value")?;
    assert!(subtable.is_horizontal());

    let ot_subtable = match subtable {
        Subtable::Ot(matched_ot_subtable) => matched_ot_subtable,
        crate::tables::kern::Subtable::Aat(..) => return Err(crate::test_support::Failure::from(alloc::format!("Should be an OpenType subtable"))),
    };

    assert_eq!(ot_subtable.coverage(), Ok(1));
    assert_eq!(ot_subtable.length(), Ok(32));
    check_format_0(subtable)?;

    Ok(())
  }

  #[test]
  fn aat_format_0() -> ::core::result::Result<(), crate::test_support::Failure> {
    #[rustfmt::skip]
        const KERN_VER_1_FMT_0_DATA: &[u8] = &[
            0x00, 0x01, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x22,
            0x00,
            0x00,
            0x00, 0x00,
            0x00, 0x03,
            0x00, 0x0C,
            0x00, 0x01,
            0x00, 0x06,
            0x00, 0x04, 0x00, 0x0C, 0xFF, 0xD8,
            0x00, 0x04, 0x00, 0x1C, 0x00, 0x28,
            0x00, 0x05, 0x00, 0x28, 0xFF, 0xCE,
        ];
    let kern = crate::test_support::fixture_value(Kern::read({
      let Ok(explicit_value) = FontData::new(KERN_VER_1_FMT_0_DATA);
      explicit_value
    }), "fixture value")?;

    let aat_kern = match &kern {
        Kern::Aat(matched_aat_kern) => matched_aat_kern,
        crate::tables::kern::Kern::Ot(..) => return Err(crate::test_support::Failure::from(alloc::format!("Should be an AAT kerning table"))),
    };

    assert_eq!(aat_kern.version(), Ok(MajorMinor::VERSION_1_0));
    assert_eq!(aat_kern.n_tables(), Ok(1));
    let subtables = kern.subtables().collect::<Vec<_>>();
    assert_eq!(subtables.len(), 1);
    let fixture_value = crate::test_support::fixture_value(subtables.first(), "fixture value")?;
    let subtable = crate::test_support::fixture_value(fixture_value.as_ref(), "fixture value")?;
    assert!(subtable.is_horizontal());

    let aat_subtable = match subtable {
        Subtable::Aat(matched_aat_subtable) => matched_aat_subtable,
        crate::tables::kern::Subtable::Ot(..) => return Err(crate::test_support::Failure::from(alloc::format!("Should be an AAT subtable"))),
    };

    assert_eq!(aat_subtable.coverage(), Ok(0));
    assert_eq!(aat_subtable.length(), Ok(34));
    check_format_0(subtable)?;

    Ok(())
  }

  fn check_format_0(subtable: &Subtable) -> Result<(), ReadError> {
    let format0 = {
        let outcome_format0 = subtable.kind()?;

        match outcome_format0 {
            SubtableKind::Format0(matched_format0) => matched_format0,
            crate::tables::kern::SubtableKind::Format1(..) | crate::tables::kern::SubtableKind::Format2(..) | crate::tables::kern::SubtableKind::Format3(..) => return Err(ReadError::MalformedData("expected a format 0 kern subtable")),
        }
    };

    const EXPECTED: &[(u32, u32, i32)] = &[(4, 12, -40), (4, 28, 40), (5, 40, -50)];
    let Ok(format0_pairs) = format0.pairs();
    let pairs = format0_pairs
      .iter()
      .map(|pair| {
        (
          ({
            let Ok(pair_left) = pair.left();
            let Ok(value) = pair_left.to_u32();
            value
          }),
          ({
            let Ok(pair_right) = pair.right();
            let Ok(value) = pair_right.to_u32();
            value
          }),
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>({
              let Ok(explicit_value) = pair.value();
              explicit_value
            });
            migrated_value
          }),
        )
      })
      .collect::<Vec<_>>();
    assert_eq!(pairs, EXPECTED);

    for (left, right, value) in EXPECTED.iter().copied() {
      assert_eq!(
        format0.kerning(
          <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(left),
          <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(right)
        ),
        Some(value),
        "left = {left}, right = {right}"
      );
    }

    Ok(())
  }

  #[test]
  fn format_2() -> ::core::result::Result<(), crate::read::ReadError> {
    let kern = Kern::read({
      let Ok(explicit_value) = FontData::new(KERN_FORMAT_2);
      explicit_value
    })?;
    let subtables = kern
      .subtables()
      .filter_map(|t| match t {
        ::core::result::Result::Ok(value___) => ::core::option::Option::Some(value___),
        ::core::result::Result::Err(_unreadable_font_data_) => ::core::option::Option::None,
      })
      .collect::<Vec<_>>();
    assert_eq!(subtables.len(), 3);

    check_format_2(
      &*match <[_]>::get(&subtables, 1) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None => {
          return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds)?;
        }
      },
      &[
        (68, 60, -100),
        (68, 61, -20),
        (68, 88, -20),
        (69, 67, -30),
        (69, 69, -30),
        (69, 70, -30),
        (69, 71, -30),
        (69, 73, -30),
        (69, 81, -30),
        (69, 83, -30),
        (72, 67, -20),
        (72, 69, -20),
        (72, 70, -20),
        (72, 71, -20),
        (72, 73, -20),
        (72, 81, -20),
        (72, 83, -20),
        (81, 60, -100),
        (81, 61, -20),
        (81, 88, -20),
        (82, 60, -100),
        (82, 61, -20),
        (82, 88, -20),
        (84, 67, -50),
        (84, 69, -50),
        (84, 70, -50),
        (84, 71, -50),
        (84, 73, -50),
        (84, 81, -50),
        (84, 83, -50),
        (88, 67, -20),
        (88, 69, -20),
        (88, 70, -20),
        (88, 71, -20),
        (88, 73, -20),
        (88, 81, -20),
        (88, 83, -20),
      ],
    )?;
    check_format_2(
      &*match <[_]>::get(&subtables, 2) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None => {
          return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds)?;
        }
      },
      &[
        (60, 67, -100),
        (60, 69, -100),
        (60, 70, -100),
        (60, 71, -100),
        (60, 73, -100),
        (60, 81, -100),
        (60, 83, -100),
      ],
    )?;
    ::core::result::Result::Ok(())
  }

  fn check_format_2(subtable: &Subtable, expected: &[(u32, u32, i32)]) -> Result<(), ReadError> {
    let format2 = {
        let outcome_format2 = subtable.kind()?;

        match outcome_format2 {
            SubtableKind::Format2(matched_format2) => matched_format2,
            crate::tables::kern::SubtableKind::Format0(..) | crate::tables::kern::SubtableKind::Format1(..) | crate::tables::kern::SubtableKind::Format3(..) => return Err(ReadError::MalformedData("expected a format 2 kern subtable")),
        }
    };

    for (left, right, value) in expected.iter().copied() {
      assert_eq!(
        format2.kerning(
          <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(left),
          <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(right)
        ),
        Some(value),
        "left = {left}, right = {right}"
      );
    }

    Ok(())
  }

  const KERN_FORMAT_2: &[u8] = &[
    0x0, 0x1, 0x0, 0x0, 0x0, 0x0, 0x0, 0x3, 0x0, 0x0, 0x0, 0x16, 0x0, 0x0, 0x0, 0x0, 0x0, 0x1,
    0x0, 0x6, 0x0, 0x0, 0x0, 0x0, 0x0, 0x24, 0x0, 0x3C, 0xFF, 0x7E, 0x0, 0x0, 0x1, 0x74, 0x0,
    0x2, 0x0, 0x0, 0x0, 0x12, 0x0, 0x7C, 0x0, 0xB0, 0x0, 0x10, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xFF, 0xEC, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xFF, 0xEC, 0xFF,
    0xEC, 0xFF, 0xBA, 0xFF, 0x9C, 0xFF, 0xD8, 0xFF, 0xE2, 0xFF, 0x7E, 0x0, 0x0, 0xFF, 0xEC, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xFF, 0xE2, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xFF, 0xCE, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x44, 0x0, 0x18, 0x0,
    0x34, 0x0, 0x58, 0x0, 0x10, 0x0, 0x10, 0x0, 0x22, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10,
    0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x34, 0x0, 0x34, 0x0, 0x10, 0x0, 0x6A, 0x0,
    0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x46, 0x0, 0x10, 0x0, 0x10, 0x0, 0x46, 0x0, 0x37, 0x0, 0x60,
    0x0, 0x10, 0x0, 0x0, 0x0, 0x8, 0x0, 0xE, 0x0, 0xC, 0x0, 0xA, 0x0, 0x6, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x2, 0x0, 0x0, 0x0, 0x2, 0x0, 0x2, 0x0, 0x2, 0x0, 0x0,
    0x0, 0x2, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x2,
    0x0, 0x0, 0x0, 0x2, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x4, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x4, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x2, 0x0, 0x2, 0x0, 0x2, 0x0, 0x2, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x2, 0x0, 0x0, 0x3, 0x84, 0x0, 0x2,
    0x0, 0x0, 0x0, 0x16, 0x1, 0x44, 0x2, 0x64, 0x0, 0x10, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xFF, 0xC4,
    0xFF, 0xCE, 0xFF, 0xB0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0xFF, 0x9C, 0xFF, 0xC4, 0xFF, 0x7E, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xFF, 0xD8, 0xFF, 0xD8, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xFF, 0xE2, 0xFF, 0xD8, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xFF, 0x7E, 0x0,
    0x0, 0x0, 0x0, 0xFF, 0x60, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0xFF, 0xD8, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xFF, 0xE2, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0xFF, 0xD8, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xFF, 0xD8, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xFF, 0xD8, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0xFF, 0x6A, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xFF,
    0x7E, 0xFF, 0xE2, 0xFF, 0x9C, 0xFF, 0x56, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xFF, 0xCE, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xFF, 0xCE,
    0xFF, 0xD8, 0xFF, 0xD8, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xFF, 0xCE, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0xFF, 0x9C, 0xFF, 0xB0, 0x0, 0x0, 0x0, 0x0, 0xFF, 0xEC, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0xFF, 0xEC, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x24,
    0x0, 0x8E, 0x1, 0x18, 0x0, 0x10, 0x0, 0x10, 0x1, 0x2, 0x0, 0x10, 0x0, 0x94, 0x0, 0x10, 0x0,
    0x10, 0x0, 0x10, 0x1, 0x2E, 0x0, 0x10, 0x0, 0xD6, 0x0, 0x10, 0x0, 0x10, 0x1, 0x2, 0x0, 0xAA,
    0x0, 0x10, 0x0, 0xC0, 0x0, 0x10, 0x0, 0xEC, 0x1, 0x2E, 0x0, 0x26, 0x0, 0x68, 0x0, 0x52, 0x0,
    0x3C, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10,
    0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0,
    0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10,
    0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0,
    0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x7E, 0x0, 0x10, 0x1, 0x2, 0x0, 0x10,
    0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0,
    0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10,
    0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0,
    0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10,
    0x0, 0x10, 0x0, 0x7E, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0,
    0x10, 0x1, 0x18, 0x0, 0x10, 0x0, 0x10, 0x0, 0x7E, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10,
    0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0,
    0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x7E, 0x0, 0x10, 0x0, 0x7E,
    0x0, 0x7E, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x0, 0x10, 0x1, 0x2, 0x0, 0x24, 0x0,
    0x8E, 0x0, 0x6, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x8, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x4, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0xC, 0x0, 0x14, 0x0, 0xE, 0x0, 0x10, 0x0, 0x12, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x2, 0x0, 0x0, 0x0, 0x2, 0x0, 0x2, 0x0,
    0x2, 0x0, 0x0, 0x0, 0x2, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xA, 0x0,
    0xA, 0x0, 0x2, 0x0, 0x0, 0x0, 0x2, 0x0, 0xA, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x4, 0x0, 0x0, 0x0, 0x0, 0x0, 0x4, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x2, 0x0, 0x2, 0x0, 0x2, 0x0, 0x2, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x6, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x6, 0x0, 0x0, 0x0, 0x0, 0x0, 0x4, 0x0, 0x2, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x4,
  ];

  #[test]
  fn format_3() -> ::core::result::Result<(), crate::test_support::Failure> {
    let mut buf = font_test_data::bebuffer::BeBuffer::default();
    buf.extend([5u16]);
    buf.extend([4u8]);
    buf.extend([3u8]);
    buf.extend([2u8]);
    buf.extend([0u8]);
    buf.extend([0i16, -10, -20, 12]);
    buf.extend([0u8, 2, 1, 1, 2]);
    buf.extend([0u8, 1, 1, 0, 1]);
    buf.extend([0u8, 1, 2, 3, 2, 1]);
    let format3 = crate::test_support::fixture_value(Subtable3::read({
      let Ok(explicit_value) = FontData::new(&buf);
      explicit_value
    }), "fixture value")?;

    const EXPECTED: [(u32, u32, i32); 25] = [
      (0, 0, 0),
      (0, 1, -10),
      (0, 2, -10),
      (0, 3, 0),
      (0, 4, -10),
      (1, 0, -20),
      (1, 1, -10),
      (1, 2, -10),
      (1, 3, -20),
      (1, 4, -10),
      (2, 0, -20),
      (2, 1, 12),
      (2, 2, 12),
      (2, 3, -20),
      (2, 4, 12),
      (3, 0, -20),
      (3, 1, 12),
      (3, 2, 12),
      (3, 3, -20),
      (3, 4, 12),
      (4, 0, -20),
      (4, 1, -10),
      (4, 2, -10),
      (4, 3, -20),
      (4, 4, -10),
    ];
    for (left, right, value) in EXPECTED {
      assert_eq!(
        format3.kerning(
          <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(left),
          <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(right)
        ),
        Some(value),
        "left = {left}, right = {right}"
      );
    }
  ;

    Ok(())
  }
}
