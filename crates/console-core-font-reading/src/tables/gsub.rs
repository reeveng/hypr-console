//! the [GSUB] table
//!
//! [GSUB]: https://docs.microsoft.com/en-us/typography/opentype/spec/gsub

pub use super::layout::{
  ChainedSequenceContext, ClassDef, CoverageTable, Device, FeatureList, FeatureVariations,
  Lookup, LookupList, ScriptList, SequenceContext,
};
use super::layout::{ExtensionLookup, LookupFlag, Subtables};

#[cfg(feature = "std")]
mod closure;
include!("../../generated/generated_gsub.rs");

pub type SubstitutionLookupList<'a> = LookupList<'a, SubstitutionLookup<'a>>;

pub type SubstitutionSequenceContext<'a> = super::layout::SequenceContext<'a>;

pub type SubstitutionChainContext<'a> = super::layout::ChainedSequenceContext<'a>;

impl<'a, T: FontRead<'a>> ExtensionLookup<'a, T> for ExtensionSubstFormat1<'a, T> {
  fn extension(&self) -> Result<T, ReadError> {
    self.extension()
  }
}

type SubSubtables<'a, T> = Subtables<'a, T, ExtensionSubstFormat1<'a, T>>;

pub enum SubstitutionSubtables<'a> {
  Single(SubSubtables<'a, SingleSubst<'a>>),
  Multiple(SubSubtables<'a, MultipleSubstFormat1<'a>>),
  Alternate(SubSubtables<'a, AlternateSubstFormat1<'a>>),
  Ligature(SubSubtables<'a, LigatureSubstFormat1<'a>>),
  Contextual(SubSubtables<'a, SubstitutionSequenceContext<'a>>),
  ChainContextual(SubSubtables<'a, SubstitutionChainContext<'a>>),
  Reverse(SubSubtables<'a, ReverseChainSingleSubstFormat1<'a>>),
  EmptyExtension,
}

impl<'a> SubstitutionLookup<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `SubstitutionLookup::lookup_flag` method returns `LookupFlag` directly in the existing parser interface")
  )]
  pub fn lookup_flag(&self) -> LookupFlag {
    let Ok(of_unit_type) = self.of_unit_type();
    let Ok(lookup_flag) = of_unit_type.lookup_flag();

    lookup_flag
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `SubstitutionLookup::lookup_type` method returns `u16` directly in the existing parser interface")
  )]
  pub fn lookup_type(&self) -> u16 {
    let Ok(of_unit_type) = self.of_unit_type();
    let Ok(lookup_type) = of_unit_type.lookup_type();

    lookup_type
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `SubstitutionLookup::mark_filtering_set` method returns the optional value directly to callers that match its presence")
  )]
  pub fn mark_filtering_set(&self) -> Option<u16> {
    let Ok(of_unit_type) = self.of_unit_type();
    let Ok(mark_filtering_set) = of_unit_type.mark_filtering_set();

    mark_filtering_set
  }

  pub fn subtables(&self) -> Result<SubstitutionSubtables<'a>, ReadError> {
    let Ok(raw_lookup) = self.of_unit_type();
    let Ok(offsets) = raw_lookup.subtable_offsets();
    let Ok(data) = raw_lookup.offset_data();
    let Ok(read_lookup_type) = raw_lookup.lookup_type();

    match read_lookup_type {
      1 => {
        let Ok(subtables) = Subtables::new(offsets, data);

        Ok(SubstitutionSubtables::Single(subtables))
      }
      2 => {
        let Ok(subtables) = Subtables::new(offsets, data);

        Ok(SubstitutionSubtables::Multiple(subtables))
      }
      3 => {
        let Ok(subtables) = Subtables::new(offsets, data);

        Ok(SubstitutionSubtables::Alternate(subtables))
      }
      4 => {
        let Ok(subtables) = Subtables::new(offsets, data);

        Ok(SubstitutionSubtables::Ligature(subtables))
      }
      5 => {
        let Ok(subtables) = Subtables::new(offsets, data);

        Ok(SubstitutionSubtables::Contextual(subtables))
      }
      6 => {
        let Ok(subtables) = Subtables::new(offsets, data);

        Ok(SubstitutionSubtables::ChainContextual(subtables))
      }
      8 => {
        let Ok(subtables) = Subtables::new(offsets, data);

        Ok(SubstitutionSubtables::Reverse(subtables))
      }
      7 => {
        let extension_type = offsets.iter().find_map(|off| {
          let Ok(value_1) = off.get();

          match value_1.resolve::<ExtensionSubstFormat1<()>>(data) {
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
            return Ok(SubstitutionSubtables::EmptyExtension);
          }
        };

        match lookup_type {
          1 => {
            let Ok(subtables) = Subtables::new_ext(offsets, data);

            Ok(SubstitutionSubtables::Single(subtables))
          }
          2 => {
            let Ok(subtables) = Subtables::new_ext(offsets, data);

            Ok(SubstitutionSubtables::Multiple(subtables))
          }
          3 => {
            let Ok(subtables) = Subtables::new_ext(offsets, data);

            Ok(SubstitutionSubtables::Alternate(subtables))
          }
          4 => {
            let Ok(subtables) = Subtables::new_ext(offsets, data);

            Ok(SubstitutionSubtables::Ligature(subtables))
          }
          5 => {
            let Ok(subtables) = Subtables::new_ext(offsets, data);

            Ok(SubstitutionSubtables::Contextual(subtables))
          }
          6 => {
            let Ok(subtables) = Subtables::new_ext(offsets, data);

            Ok(SubstitutionSubtables::ChainContextual(subtables))
          }
          8 => {
            let Ok(subtables) = Subtables::new_ext(offsets, data);

            Ok(SubstitutionSubtables::Reverse(subtables))
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
