//! The [morx (Extended Glyph Metamorphosis)](https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6morx.html) table.

use super::aat::{ExtendedStateTable, LookupU16, safe_read_array_to_end};

include!("../../generated/generated_morx.rs");

impl VarSize for Chain<'_> {
  type Size = u32;

  fn read_len_at(data: FontData, pos: u64) -> Result<u64, ReadError> {
    let position = pos
      .checked_add(u32::RAW_BYTE_LEN)
      .ok_or(ReadError::OutOfBounds)?;

    data.read_at::<u32>(position).map(u64::from)
  }
}

impl VarSize for Subtable<'_> {
  type Size = u32;

  fn read_len_at(data: FontData, pos: u64) -> Result<u64, ReadError> {
    data.read_at::<u32>(pos).map(u64::from)
  }
}

impl<'a> Subtable<'a> {
  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Subtable::is_logical` method returns `bool` directly in the existing parser interface")
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(explicit007_no_bool_return, reason = "the published `Subtable::is_logical` method retains the Boolean result used by existing predicate callers and implementations")
  )]
  pub fn is_logical(&self) -> bool {
    let Ok(coverage) = self.coverage();

    coverage & 0x10000000 != 0
  }

  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Subtable::is_all_directions` method returns `bool` directly in the existing parser interface")
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(explicit007_no_bool_return, reason = "the published `Subtable::is_all_directions` method retains the Boolean result used by existing predicate callers and implementations")
  )]
  pub fn is_all_directions(&self) -> bool {
    let Ok(coverage) = self.coverage();

    coverage & 0x20000000 != 0
  }

  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Subtable::is_backwards` method returns `bool` directly in the existing parser interface")
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(explicit007_no_bool_return, reason = "the published `Subtable::is_backwards` method retains the Boolean result used by existing predicate callers and implementations")
  )]
  pub fn is_backwards(&self) -> bool {
    let Ok(coverage) = self.coverage();

    coverage & 0x40000000 != 0
  }

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

  pub fn kind(&self) -> Result<SubtableKind<'a>, ReadError> {
    let Ok(data) = self.data();
    let Ok(value_1) = FontData::new(data);

    let Ok(coverage) = self.coverage();

    SubtableKind::read_with_args(value_1, &coverage)
  }
}

#[derive(Clone)]
pub enum SubtableKind<'a> {
  Rearrangement(ExtendedStateTable<'a>),
  Contextual(ContextualSubtable<'a>),
  Ligature(LigatureSubtable<'a>),
  NonContextual(LookupU16<'a>),
  Insertion(InsertionSubtable<'a>),
}

impl ReadArgs for SubtableKind<'_> {
  type Args = u32;
}

impl<'a> FontReadWithArgs<'a> for SubtableKind<'a> {
  fn read_with_args(data: FontData<'a>, args: &Self::Args) -> Result<Self, ReadError> {
    let format = *args & 0xFF;

    match format {
      0 => {
        let read = ExtendedStateTable::read(data)?;

        Ok(Self::Rearrangement(read))
      }
      1 => {
        let read = ContextualSubtable::read(data)?;

        Ok(Self::Contextual(read))
      }
      2 => {
        let read = LigatureSubtable::read(data)?;

        Ok(Self::Ligature(read))
      }

      4 => {
        let read = LookupU16::read(data)?;

        Ok(Self::NonContextual(read))
      }
      5 => {
        let read = InsertionSubtable::read(data)?;

        Ok(Self::Insertion(read))
      }
      _ => Err(ReadError::InvalidFormat({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i64>(format);
        migrated_value
      })),
    }
  }
}

#[derive(Clone)]
pub struct ContextualSubtable<'a> {
  pub state_table: ExtendedStateTable<'a, ContextualEntryData>,
  pub lookups: ArrayOfOffsets<'a, LookupU16<'a>, Offset32>,
}

impl<'a> FontRead<'a> for ContextualSubtable<'a> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    let state_table = ExtendedStateTable::read(data)?;
    let Ok(mut cursor) = data.cursor();
    let ::core::result::Result::Ok(()) = cursor.advance_by(ExtendedStateTable::<()>::HEADER_LEN);
    let offset = {
      let read = cursor.read::<u32>()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(read);
      migrated_value
    };
    let Ok(end) = data.len();
    let array = data.read_array(offset..end)?;
    let Ok(offsets_data) = FontData::new(array);
    let value_2 = &offsets_data;

    let raw_offsets: &[BigEndian<Offset32>] = safe_read_array_to_end(value_2, 0)?;
    let Ok(value_9) = ArrayOfOffsets::new(raw_offsets, offsets_data, ());

    let lookups = value_9;
    Ok(Self {
      state_table,
      lookups,
    })
  }
}

#[derive(Clone)]
pub struct LigatureSubtable<'a> {
  pub state_table: ExtendedStateTable<'a, BigEndian<u16>>,
  pub ligature_actions: &'a [BigEndian<u32>],
  pub components: &'a [BigEndian<u16>],
  pub ligatures: &'a [BigEndian<GlyphId16>],
}

impl<'a> FontRead<'a> for LigatureSubtable<'a> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    let state_table = ExtendedStateTable::read(data)?;
    let Ok(mut cursor) = data.cursor();
    let ::core::result::Result::Ok(()) = cursor.advance_by(ExtendedStateTable::<()>::HEADER_LEN);

    let lig_action_offset = {
      let read = cursor.read::<u32>()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(read);
      migrated_value
    };
    let component_offset = {
      let read = cursor.read::<u32>()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(read);
      migrated_value
    };
    let ligature_offset = {
      let read = cursor.read::<u32>()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(read);
      migrated_value
    };
    let ligature_actions = safe_read_array_to_end(&data, lig_action_offset)?;
    let components = safe_read_array_to_end(&data, component_offset)?;
    let ligatures = safe_read_array_to_end(&data, ligature_offset)?;
    Ok(Self {
      state_table,
      ligature_actions,
      components,
      ligatures,
    })
  }
}

#[derive(Clone)]
pub struct InsertionSubtable<'a> {
  pub state_table: ExtendedStateTable<'a, InsertionEntryData>,
  pub glyphs: &'a [BigEndian<GlyphId16>],
}

impl<'a> FontRead<'a> for InsertionSubtable<'a> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    let state_table = ExtendedStateTable::read(data)?;
    let Ok(mut cursor) = data.cursor();
    let ::core::result::Result::Ok(()) = cursor.advance_by(ExtendedStateTable::<()>::HEADER_LEN);
    let glyphs_offset = {
      let read = cursor.read::<u32>()?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(read);
      migrated_value
    };
    let glyphs = safe_read_array_to_end(&data, glyphs_offset)?;
    Ok(Self {
      state_table,
      glyphs,
    })
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for Chain<'a> {
  fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
    Ok(RecordResolver {
      name: "Chain",
      get_field: Box::new(move |idx, _data| match idx {
        0u64 => {
          let Ok(field) = Field::new("default_flags", self.default_flags());

          Some(field)
        }
        _ => None,
      }),
      data,
    })
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
          let Ok(field) = Field::new("sub_feature_flags", self.sub_feature_flags());

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
  use crate::{FontRef, TableProvider};

  #[test]
  fn parse_chain_flags_features() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::morx::FOUR)?;
    let morx = font.morx()?;
    let Ok(chains) = morx.chains();
    let Ok(mut value_3) = chains.iter();

    let fixture_value = crate::test_support::fixture_value(value_3.next(), "fixture value")?;
    let chain = fixture_value?;
    assert_eq!(chain.default_flags(), Ok(1));
    let Ok(features) = chain.features();
    let feature = *match <[_]>::get(&*features, 0) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::test_support::Failure::from(crate::read::ReadError::OutOfBounds));
      }
    };
    assert_eq!(feature.feature_type(), Ok(4));
    assert_eq!(feature.feature_settings(), Ok(0));
    assert_eq!(feature.enable_flags(), Ok(1));
    assert_eq!(feature.disable_flags(), Ok(0xFFFFFFFF));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_rearrangement() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::morx::FOUR)?;
    let morx = font.morx()?;
    let Ok(chains) = morx.chains();
    let Ok(mut value_4) = chains.iter();

    let fixture_value = crate::test_support::fixture_value(value_4.next(), "fixture value")?;
    let chain = fixture_value?;
    let subtable = {
      let rearrangement_subtable = crate::test_support::fixture_value(
      ({
        let Ok(subtables) = chain.subtables();
        let Ok(explicit_value) = subtables.iter();
        explicit_value
      })
      .next(),
      "fixture value",
    )?;
      rearrangement_subtable
    }?;
    assert_eq!(subtable.coverage(), Ok(0x2000_0000));

    let kind = subtable.kind()?;

    match kind {
      SubtableKind::Rearrangement(_) => Ok(()),
      SubtableKind::Contextual(..)
      | SubtableKind::Ligature(..)
      | SubtableKind::NonContextual(..)
      | SubtableKind::Insertion(..) => Err(crate::test_support::Failure::from("expected a rearrangement subtable")),
    }
  }

  #[test]
  fn parse_contextual() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::morx::EIGHTEEN)?;
    let morx = font.morx()?;
    let Ok(chains) = morx.chains();
    let Ok(mut value_5) = chains.iter();

    let fixture_value = crate::test_support::fixture_value(value_5.next(), "fixture value")?;
    let chain = fixture_value?;
    let subtable = {
      let contextual_subtable = crate::test_support::fixture_value(
      ({
        let Ok(subtables) = chain.subtables();
        let Ok(explicit_value) = subtables.iter();
        explicit_value
      })
      .next(),
      "fixture value",
    )?;
      contextual_subtable
    }?;
    assert_eq!(subtable.coverage(), Ok(0x2000_0001));

    let kind = {
        let outcome_kind = subtable.kind()?;

        match outcome_kind {
            SubtableKind::Contextual(matched_kind) => matched_kind,
            crate::tables::morx::SubtableKind::Rearrangement(..) | crate::tables::morx::SubtableKind::Ligature(..) | crate::tables::morx::SubtableKind::NonContextual(..) | crate::tables::morx::SubtableKind::Insertion(..) => return Err(crate::test_support::Failure::from(alloc::format!("expected contextual subtable!"))),
        }
    };

    let lookup = kind.lookups.get(0)?;
    let expected = [None, None, Some(7u16), Some(8), Some(9), Some(10), Some(11)];
    let values = (0..7)
      .map(|gid| match lookup.value(gid) {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
      })
      .collect::<Vec<_>>();
    assert_eq!(values, &expected);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_ligature() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::morx::FORTY_ONE)?;
    let morx = font.morx()?;
    let Ok(chains) = morx.chains();
    let Ok(mut value_6) = chains.iter();

    let fixture_value = crate::test_support::fixture_value(value_6.next(), "fixture value")?;
    let chain = fixture_value?;
    let subtable = {
      let ligature_subtable = crate::test_support::fixture_value(
      ({
        let Ok(subtables) = chain.subtables();
        let Ok(explicit_value) = subtables.iter();
        explicit_value
      })
      .next(),
      "fixture value",
    )?;
      ligature_subtable
    }?;
    assert_eq!(subtable.coverage(), Ok(0x2000_0002));

    let kind = {
        let outcome_kind = subtable.kind()?;

        match outcome_kind {
            SubtableKind::Ligature(matched_kind) => matched_kind,
            crate::tables::morx::SubtableKind::Rearrangement(..) | crate::tables::morx::SubtableKind::Contextual(..) | crate::tables::morx::SubtableKind::NonContextual(..) | crate::tables::morx::SubtableKind::Insertion(..) => return Err(crate::test_support::Failure::from(alloc::format!("expected ligature subtable!"))),
        }
    };

    let expected_actions = [0x3FFFFFFE, 0xBFFFFFFE];

    let actions = kind
      .ligature_actions
      .iter()
      .take(2)
      .map(|action| {
        let Ok(value) = action.get();
        value
      })
      .collect::<Vec<_>>();
    assert_eq!(actions, &expected_actions);
    let expected_components = [0u16, 1, 0, 0];

    let components = kind
      .components
      .iter()
      .take(4)
      .map(|comp| {
        let Ok(value) = comp.get();
        value
      })
      .collect::<Vec<_>>();
    assert_eq!(components, &expected_components);
    let expected_ligatures = [
      {
        let Ok(value) = GlyphId16::new(5);
        value
      },
      {
        let Ok(value) = GlyphId16::new(6);
        value
      },
    ];
    let ligatures = kind
      .ligatures
      .iter()
      .map(|gid| {
        let Ok(value) = gid.get();
        value
      })
      .collect::<Vec<_>>();
    assert_eq!(ligatures, &expected_ligatures);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_non_contextual() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::morx::ONE)?;
    let morx = font.morx()?;
    let Ok(chains) = morx.chains();
    let Ok(mut value_7) = chains.iter();

    let fixture_value = crate::test_support::fixture_value(value_7.next(), "fixture value")?;
    let chain = fixture_value?;
    let subtable = {
      let noncontextual_subtable = crate::test_support::fixture_value(
      ({
        let Ok(subtables) = chain.subtables();
        let Ok(explicit_value) = subtables.iter();
        explicit_value
      })
      .next(),
      "fixture value",
    )?;
      noncontextual_subtable
    }?;
    assert_eq!(subtable.coverage(), Ok(0x2000_0004));

    let kind = {
        let outcome_kind = subtable.kind()?;

        match outcome_kind {
            SubtableKind::NonContextual(matched_kind) => matched_kind,
            crate::tables::morx::SubtableKind::Rearrangement(..) | crate::tables::morx::SubtableKind::Contextual(..) | crate::tables::morx::SubtableKind::Ligature(..) | crate::tables::morx::SubtableKind::Insertion(..) => return Err(crate::test_support::Failure::from(alloc::format!("expected non-contextual subtable!"))),
        }
    };

    let expected_values = [None, None, Some(5u16), None, Some(7)];
    let values = (0..5)
      .map(|gid| match kind.value(gid) {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
      })
      .collect::<Vec<_>>();
    assert_eq!(values, &expected_values);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_insertion() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::morx::THIRTY_FOUR)?;
    let morx = font.morx()?;
    let Ok(chains) = morx.chains();
    let Ok(mut value_8) = chains.iter();

    let fixture_value = crate::test_support::fixture_value(value_8.next(), "fixture value")?;
    let chain = fixture_value?;
    let subtable = {
      let insertion_subtable = crate::test_support::fixture_value(
      ({
        let Ok(subtables) = chain.subtables();
        let Ok(explicit_value) = subtables.iter();
        explicit_value
      })
      .next(),
      "fixture value",
    )?;
      insertion_subtable
    }?;
    assert_eq!(subtable.coverage(), Ok(0x2000_0005));

    let kind = {
        let outcome_kind = subtable.kind()?;

        match outcome_kind {
            SubtableKind::Insertion(matched_kind) => matched_kind,
            crate::tables::morx::SubtableKind::Rearrangement(..) | crate::tables::morx::SubtableKind::Contextual(..) | crate::tables::morx::SubtableKind::Ligature(..) | crate::tables::morx::SubtableKind::NonContextual(..) => return Err(crate::test_support::Failure::from(alloc::format!("expected insertion subtable!"))),
        }
    };

    let mut expected_glyphs = vec![];

    for _ in 0..9 {
      for gid in [3, 2] {
        expected_glyphs.push({
          let Ok(value) = GlyphId16::new(gid);
          value
        });
      }
    }

    assert_eq!(kind.glyphs, &expected_glyphs);
    ::core::result::Result::Ok(())
  }
}
