//! the [GPOS] table
//!
//! [GPOS]: https://docs.microsoft.com/en-us/typography/opentype/spec/gpos

#[path = "./value_record.rs"]
mod value_record;

#[cfg(feature = "std")]
mod closure;

use crate::array::ComputedArray;

pub use super::layout::{
  ClassDef, CoverageTable, Device, DeviceOrVariationIndex, FeatureList, FeatureVariations,
  Lookup, ScriptList,
};
use super::layout::{ExtensionLookup, LookupFlag, Subtables};
pub use value_record::{Value, ValueContext, ValueRecord};

include!("../../generated/generated_gpos.rs");

pub type PositionLookupList<'a> = super::layout::LookupList<'a, PositionLookup<'a>>;

pub type PositionSequenceContext<'a> = super::layout::SequenceContext<'a>;

pub type PositionChainContext<'a> = super::layout::ChainedSequenceContext<'a>;

impl<'a> AnchorTable<'a> {
  pub fn x_device(&self) -> Result<Option<DeviceOrVariationIndex<'a>>, ReadError> {
    match self {
      AnchorTable::Format3(inner) => inner.x_device(),
      AnchorTable::Format1(_) | AnchorTable::Format2(_) => Ok(None),
    }
  }

  pub fn y_device(&self) -> Result<Option<DeviceOrVariationIndex<'a>>, ReadError> {
    match self {
      AnchorTable::Format3(inner) => inner.y_device(),
      AnchorTable::Format1(_) | AnchorTable::Format2(_) => Ok(None),
    }
  }
}

impl<'a, T: FontRead<'a>> ExtensionLookup<'a, T> for ExtensionPosFormat1<'a, T> {
  fn extension(&self) -> Result<T, ReadError> {
    self.extension()
  }
}

type PosSubtables<'a, T> = Subtables<'a, T, ExtensionPosFormat1<'a, T>>;

pub enum PositionSubtables<'a> {
  Single(PosSubtables<'a, SinglePos<'a>>),
  Pair(PosSubtables<'a, PairPos<'a>>),
  Cursive(PosSubtables<'a, CursivePosFormat1<'a>>),
  MarkToBase(PosSubtables<'a, MarkBasePosFormat1<'a>>),
  MarkToLig(PosSubtables<'a, MarkLigPosFormat1<'a>>),
  MarkToMark(PosSubtables<'a, MarkMarkPosFormat1<'a>>),
  Contextual(PosSubtables<'a, PositionSequenceContext<'a>>),
  ChainContextual(PosSubtables<'a, PositionChainContext<'a>>),
  EmptyExtension,
}

impl<'a> PositionLookup<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `PositionLookup::lookup_flag` method returns `LookupFlag` directly in the existing parser interface")
  )]
  pub fn lookup_flag(&self) -> LookupFlag {
    let Ok(of_unit_type) = self.of_unit_type();
    let Ok(lookup_flag) = of_unit_type.lookup_flag();

    lookup_flag
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `PositionLookup::lookup_type` method returns `u16` directly in the existing parser interface")
  )]
  pub fn lookup_type(&self) -> u16 {
    let Ok(of_unit_type) = self.of_unit_type();
    let Ok(lookup_type) = of_unit_type.lookup_type();

    lookup_type
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `PositionLookup::mark_filtering_set` method returns the optional value directly to callers that match its presence")
  )]
  pub fn mark_filtering_set(&self) -> Option<u16> {
    let Ok(of_unit_type) = self.of_unit_type();
    let Ok(mark_filtering_set) = of_unit_type.mark_filtering_set();

    mark_filtering_set
  }

  pub fn subtables(&self) -> Result<PositionSubtables<'a>, ReadError> {
    let Ok(raw_lookup) = self.of_unit_type();
    let Ok(offsets) = raw_lookup.subtable_offsets();
    let Ok(data) = raw_lookup.offset_data();
    let Ok(read_lookup_type) = raw_lookup.lookup_type();

    match read_lookup_type {
      1 => {
          let Ok(subtables) = Subtables::new(offsets, data);

          Ok(PositionSubtables::Single(subtables))
      }
      2 => {
          let Ok(subtables) = Subtables::new(offsets, data);

          Ok(PositionSubtables::Pair(subtables))
      }
      3 => {
          let Ok(subtables) = Subtables::new(offsets, data);

          Ok(PositionSubtables::Cursive(subtables))
      }
      4 => {
          let Ok(subtables) = Subtables::new(offsets, data);

          Ok(PositionSubtables::MarkToBase(subtables))
      }
      5 => {
          let Ok(subtables) = Subtables::new(offsets, data);

          Ok(PositionSubtables::MarkToLig(subtables))
      }
      6 => {
          let Ok(subtables) = Subtables::new(offsets, data);

          Ok(PositionSubtables::MarkToMark(subtables))
      }
      7 => {
          let Ok(subtables) = Subtables::new(offsets, data);

          Ok(PositionSubtables::Contextual(subtables))
      }
      8 => {
          let Ok(subtables) = Subtables::new(
            offsets, data,
          );

          Ok(PositionSubtables::ChainContextual(subtables))
      }
      9 => {
        let extension_type = offsets.iter().find_map(|off| {
          let Ok(value_1) = off.get();

          match value_1.resolve::<ExtensionPosFormat1<()>>(data) {
            ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
            ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
          }
          .map(|ext| {
            let Ok(lookup_type) = ext.extension_lookup_type();

            lookup_type
          })
        });

        let lookup_type = match extension_type {
          Some(lookup_type) => lookup_type,
          None => {
            return Ok(PositionSubtables::EmptyExtension);
          }
        };

        match lookup_type {
          1 => {
              let Ok(subtables) = Subtables::new_ext(offsets, data);

              Ok(PositionSubtables::Single(subtables))
          }
          2 => {
              let Ok(subtables) = Subtables::new_ext(offsets, data);

              Ok(PositionSubtables::Pair(subtables))
          }
          3 => {
              let Ok(subtables) = Subtables::new_ext(
                offsets, data,
              );

              Ok(PositionSubtables::Cursive(subtables))
          }
          4 => {
              let Ok(subtables) = Subtables::new_ext(
                offsets, data,
              );

              Ok(PositionSubtables::MarkToBase(subtables))
          }
          5 => {
              let Ok(subtables) = Subtables::new_ext(
                offsets, data,
              );

              Ok(PositionSubtables::MarkToLig(subtables))
          }
          6 => {
              let Ok(subtables) = Subtables::new_ext(
                offsets, data,
              );

              Ok(PositionSubtables::MarkToMark(subtables))
          }
          7 => {
              let Ok(subtables) = Subtables::new_ext(
                offsets, data,
              );

              Ok(PositionSubtables::Contextual(subtables))
          }
          8 => {
              let Ok(subtables) = Subtables::new_ext(
                offsets, data,
              );

              Ok(PositionSubtables::ChainContextual(subtables))
          }
          other => Err(ReadError::InvalidFormat({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i64>(other);
            migrated_value
          })),
        }
      }
      other => Err(ReadError::InvalidFormat({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i64>(other);
        migrated_value
      })),
    }
  }
}

impl PairPosFormat2<'_> {
  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(explicit024_no_swappable_params, reason = "the published `PairPosFormat2::values` method retains the positional argument order for `class1`, `class2`, `context`")
  )]
  pub fn values(
    &self,
    class1: u16,
    class2: u16,
    context: &ValueContext,
  ) -> Result<[Value; 2], ReadError> {
    let Ok(format1) = self.value_format1();
    let format1_len = format1.record_byte_len();
    let Ok(format2) = self.value_format2();
    let record_size =
      ::core::primitive::u64::wrapping_add(format1_len, format2.record_byte_len());
    let Ok(data) = self.offset_data();

    let record_offset = ::core::primitive::u64::wrapping_add(
      ::core::primitive::u64::wrapping_add(
        ::core::primitive::u64::wrapping_mul(
          ::core::primitive::u64::wrapping_mul(
            {
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(class1);
              migrated_value
            },
            record_size,
          ),
          {
            let Ok(class2_count) = self.class2_count();
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u64>(class2_count);
            migrated_value
          },
        ),
        ::core::primitive::u64::wrapping_mul(
          {
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(class2);
            migrated_value
          },
          record_size,
        ),
      ),
      ({
        let Ok(explicit_value) = self.class1_records_byte_range();
        explicit_value
      })
      .start,
    );
    let read = Value::read(data, record_offset, format1, context)?;
    let value_read = Value::read(
      data,
      ::core::primitive::u64::wrapping_add(record_offset, format1_len),
      format2,
      context,
    )?;

    Ok([read, value_read])
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn pair_pos2_values_match_value_records() -> ::core::result::Result<(), crate::test_support::Failure> {
    let Ok(data) = FontData::new(font_test_data::gpos::PAIRPOSFORMAT2);
    let table = crate::test_support::fixture_value(PairPosFormat2::read(data), "fixture value")?;
    let Ok(class1_count) = table.class1_count();
    let Ok(class2_count) = table.class2_count();
    let Ok(records) = table.class1_records();
    let context = ValueContext::default();

    for class1 in 0..class1_count {
      let class1_record = crate::test_support::fixture_value(records
        .get({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(class1);
          migrated_value
        }), "fixture value")?;
      let Ok(class2_records) = class1_record.class2_records();

      for class2 in 0..class2_count {
        let record = crate::test_support::fixture_value(class2_records
          .get({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(class2);
            migrated_value
          }), "fixture value")?;
        let first = record.value_record1.value(data, &context)?;
        let second = record.value_record2.value(data, &context)?;
        let value_records = [first, second];
        let values = crate::test_support::fixture_value(table.values(class1, class2, &context), "fixture value")?;
        assert_eq!(value_records, values);
      }
    }
  ;

    Ok(())
  }

  #[test]
  fn default_for_generics() -> ::core::result::Result<(), crate::test_support::Failure> {
    let inner = {
        let outcome_inner = ExtensionSubtable::default();

        match outcome_inner {
            ExtensionSubtable::Single(matched_inner) => matched_inner,
            crate::tables::gpos::ExtensionSubtable::Pair(..) | crate::tables::gpos::ExtensionSubtable::Cursive(..) | crate::tables::gpos::ExtensionSubtable::MarkToBase(..) | crate::tables::gpos::ExtensionSubtable::MarkToLig(..) | crate::tables::gpos::ExtensionSubtable::MarkToMark(..) | crate::tables::gpos::ExtensionSubtable::Contextual(..) | crate::tables::gpos::ExtensionSubtable::ChainContextual(..) => return Err(crate::test_support::Failure::from(alloc::format!("this is quite bad"))),
        }
    };

    assert_eq!(inner.extension_lookup_type(), Ok(0));

    let extension = match inner.extension() {
      Ok(value) => value,
      Err(_unreadable_extension_subtable) => Default::default(),
    };

    match extension {
      SinglePos::Format1(_) => Ok(()),
      SinglePos::Format2(_) => Err(crate::test_support::Failure::from("expected a format 1 extension")),
    }
  }
}
