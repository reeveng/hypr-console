//! Support Layout Closure

use types::{BigEndian, GlyphId16, Offset16};

use super::{
  ArrayOfOffsets, ChainedClassSequenceRule, ChainedClassSequenceRuleSet, ChainedSequenceContext,
  ChainedSequenceContextFormat1, ChainedSequenceContextFormat2, ChainedSequenceContextFormat3,
  ChainedSequenceRule, ChainedSequenceRuleSet, ClassDef, ClassDefFormat1, ClassDefFormat2,
  ClassSequenceRule, ClassSequenceRuleSet, CoverageTable, ExtensionLookup, Feature, FeatureList,
  FeatureVariations, FontRead, GlyphId, LangSys, ReadError, Script, ScriptList, SequenceContext,
  SequenceContextFormat1, SequenceContextFormat2, SequenceContextFormat3, SequenceLookupRecord,
  SequenceRule, SequenceRuleSet, Subtables, Tag,
};
use crate::{
  collections::IntSet,
  tables::{gpos::PositionLookupList, gsub::SubstitutionLookupList},
};

pub(crate) enum VisitDecision {
  Visit,
  Skip,
}

pub(crate) enum TraversalBudget {
  Available,
  Exhausted,
}

const MAX_SCRIPTS: u16 = 500;
const MAX_LANGSYS: u16 = 2000;
const MAX_FEATURE_INDICES: u16 = 1500;
pub(crate) const MAX_NESTING_LEVEL: u8 = 64;
pub(crate) const MAX_LOOKUP_VISIT_COUNT: u16 = 35000;

struct CollectFeaturesContext<'a> {
  script_count: u16,
  langsys_count: u16,
  feature_index_count: u16,
  visited_script: IntSet<u32>,
  visited_langsys: IntSet<u32>,
  feature_indices: &'a mut IntSet<u16>,
  feature_indices_filter: IntSet<u16>,
  table_head: u64,
}

impl<'a> CollectFeaturesContext<'a> {
  pub(crate) fn new(
    features: &IntSet<Tag>,
    table_head: u64,
    feature_list: &'a FeatureList<'a>,
    feature_indices: &'a mut IntSet<u16>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(value_1) = IntSet::empty();
    let Ok(value_2) = IntSet::empty();

    let Ok(feature_records) = feature_list.feature_records();

    Ok(Self {
      script_count: 0,
      langsys_count: 0,
      feature_index_count: 0,
      visited_script: value_1,
      visited_langsys: value_2,
      feature_indices,
      feature_indices_filter: feature_records
        .iter()
        .enumerate()
        .filter(|(_i, record)| {
          let Ok(feature_tag) = record.feature_tag();
          let Ok(value_58) = features.contains(feature_tag);

          value_58 == console_core_integer_sets::Presence::Present
        })
        .map(|(idx, _)| {
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(idx);
          migrated_value
        })
        .collect(),
      table_head,
    })
  }

  pub(crate) fn visit_script(
    &mut self,
    script: &Script,
  ) -> Result<VisitDecision, console_core_never::Never> {
    match self.script_count.cmp(&MAX_SCRIPTS) {
      core::cmp::Ordering::Greater => return Ok(VisitDecision::Skip),
      core::cmp::Ordering::Less | core::cmp::Ordering::Equal => {}
    }

    self.script_count = self.script_count.wrapping_add(1);
    let Ok(data) = script.offset_data();
    let Ok(address) = data.base_address();
    let Ok(offset) =
      console_core_number_conversion::wrapped::<_, u32>(address.wrapping_sub(self.table_head));
    Ok(match self.visited_script.insert(offset) {
      Ok(crate::Presence::Present) => VisitDecision::Skip,
      Ok(crate::Presence::Absent) => VisitDecision::Visit,
    })
  }

  pub(crate) fn visit_langsys(
    &mut self,
    langsys: &LangSys,
  ) -> Result<VisitDecision, console_core_never::Never> {
    match self.langsys_count.cmp(&MAX_LANGSYS) {
      core::cmp::Ordering::Greater => return Ok(VisitDecision::Skip),
      core::cmp::Ordering::Less | core::cmp::Ordering::Equal => {}
    }

    self.langsys_count = self.langsys_count.wrapping_add(1);
    let Ok(data) = langsys.offset_data();
    let Ok(address) = data.base_address();
    let Ok(offset) =
      console_core_number_conversion::wrapped::<_, u32>(address.wrapping_sub(self.table_head));
    Ok(match self.visited_langsys.insert(offset) {
      Ok(crate::Presence::Present) => VisitDecision::Skip,
      Ok(crate::Presence::Absent) => VisitDecision::Visit,
    })
  }

  pub(crate) fn charge_feature_indices(
    &mut self,
    count: u16,
  ) -> Result<TraversalBudget, console_core_never::Never> {
    self.feature_index_count = match self.feature_index_count.checked_add(count) {
      Some(new_count) => new_count,
      None => {
        self.feature_index_count = MAX_FEATURE_INDICES;
        return Ok(TraversalBudget::Exhausted);
      }
    };
    Ok(match self.feature_index_count.cmp(&MAX_FEATURE_INDICES) {
      core::cmp::Ordering::Greater => TraversalBudget::Exhausted,
      core::cmp::Ordering::Less | core::cmp::Ordering::Equal => TraversalBudget::Available,
    })
  }
}

impl ScriptList<'_> {
  pub(crate) fn collect_features(
    &self,
    layout_table_head: u64,
    feature_list: &FeatureList,
    scripts: &IntSet<Tag>,
    languages: &IntSet<Tag>,
    features: &IntSet<Tag>,
  ) -> Result<IntSet<u16>, ReadError> {
    let Ok(mut out) = IntSet::empty();
    let value_8 = &mut out;

    let Ok(mut c) =
      CollectFeaturesContext::new(features, layout_table_head, feature_list, value_8);
    let Ok(script_records) = self.script_records();
    let Ok(font_data) = self.offset_data();
    let Ok(value_9) = scripts
      .membership()
      .map(|membership| membership == console_core_integer_sets::Membership::Exclusive);

    match ::core::primitive::bool::then_some(value_9, ()) {
      ::core::option::Option::Some(()) => {
        for record in script_records {
          let Ok(tag) = record.script_tag();

          match scripts.contains(tag) {
            Ok(crate::Presence::Present) => {}
            Ok(crate::Presence::Absent) => continue,
          }

          let Ok(offset) = record.script_offset();

          match offset.nullness() {
            Ok(font_types::Nullness::NonNull) => {}
            Ok(font_types::Nullness::Null) => continue,
          }

          let script = record.script(font_data)?;
          script.collect_features(&mut c, languages)?;
        }
      }
      ::core::option::Option::None => {
        let Ok(value_12) = scripts.iter();

        for idx in value_12.filter_map(|tag| self.index_for_tag(tag)) {
          let Ok(value_20) = console_core_number_conversion::index({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(idx);
            migrated_value
          });

          let record = *match <[_]>::get(&*script_records, value_20) {
            ::core::option::Option::Some(indexed_value) => indexed_value,
            ::core::option::Option::None => {
              return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds);
            }
          };
          let Ok(script_offset) = record.script_offset();
          let Ok(value_13) = script_offset.nullness();
          let value_13_2 = matches!(value_13, font_types::Nullness::Null);

          match ::core::primitive::bool::then_some(value_13_2, ()) {
            ::core::option::Option::Some(()) => {
              continue;
            }
            ::core::option::Option::None => {}
          }

          let script = record.script(font_data)?;
          script.collect_features(&mut c, languages)?;
        }
      }
    }

    Ok(out)
  }
}

impl Script<'_> {
  fn collect_features(
    &self,
    c: &mut CollectFeaturesContext,
    languages: &IntSet<Tag>,
  ) -> Result<(), ReadError> {
    match ::core::primitive::bool::then_some(
      matches!(c.visit_script(self), Ok(VisitDecision::Skip)),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let Ok(lang_sys_records) = self.lang_sys_records();
    let Ok(font_data) = self.offset_data();

    let own_default_lang_sys = self.default_lang_sys()?;

    match own_default_lang_sys {
      Some(default_lang_sys) => {
        let Ok(()) = default_lang_sys.collect_features(c);
      }
      None => {}
    }

    let Ok(value_14) = languages
      .membership()
      .map(|membership| membership == console_core_integer_sets::Membership::Exclusive);

    match ::core::primitive::bool::then_some(value_14, ()) {
      ::core::option::Option::Some(()) => {
        for record in lang_sys_records {
          let Ok(tag) = record.lang_sys_tag();

          match languages.contains(tag) {
            Ok(crate::Presence::Present) => {}
            Ok(crate::Presence::Absent) => continue,
          }

          let Ok(offset) = record.lang_sys_offset();

          match offset.nullness() {
            Ok(font_types::Nullness::NonNull) => {}
            Ok(font_types::Nullness::Null) => continue,
          }

          let lang_sys = record.lang_sys(font_data)?;
          let Ok(()) = lang_sys.collect_features(c);
        }
      }
      ::core::option::Option::None => {
        let Ok(value_17) = languages.iter();

        for idx in value_17.filter_map(|tag| self.lang_sys_index_for_tag(tag)) {
          let Ok(value_35) = console_core_number_conversion::index({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(idx);
            migrated_value
          });

          let record = *match <[_]>::get(&*lang_sys_records, value_35) {
            ::core::option::Option::Some(indexed_value) => indexed_value,
            ::core::option::Option::None => {
              return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds);
            }
          };
          let Ok(lang_sys_offset) = record.lang_sys_offset();
          let Ok(value_18) = lang_sys_offset.nullness();
          let value_18_2 = matches!(value_18, font_types::Nullness::Null);

          match ::core::primitive::bool::then_some(value_18_2, ()) {
            ::core::option::Option::Some(()) => {
              continue;
            }
            ::core::option::Option::None => {}
          }

          let lang_sys = record.lang_sys(font_data)?;
          let Ok(()) = lang_sys.collect_features(c);
        }
      }
    }

    Ok(())
  }
}

impl LangSys<'_> {
  fn collect_features(
    &self,
    c: &mut CollectFeaturesContext,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      match ::core::primitive::bool::then_some(
        matches!(c.visit_langsys(self), Ok(VisitDecision::Skip)),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          return Ok(());
        }
        ::core::option::Option::None => {}
      }

      let Ok(value_19) = c.feature_indices_filter.is_empty();

      match ::core::primitive::bool::then_some(
        value_19 == console_core_integer_sets::Emptiness::Empty,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          return Ok(());
        }
        ::core::option::Option::None => {}
      }

      let Ok(required_feature_idx) = self.required_feature_index();

      match ::core::primitive::bool::then_some(
        {
          required_feature_idx != 0xFFFF
            && matches!(c.charge_feature_indices(1), Ok(TraversalBudget::Available))
            && {
              let Ok(value_63) = c.feature_indices_filter.contains(required_feature_idx);

              value_63 == console_core_integer_sets::Presence::Present
            }
        },
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let Ok(_presence_changed) = c.feature_indices.insert(required_feature_idx);
        }
        ::core::option::Option::None => {}
      }

      let Ok(feature_index_count) = self.feature_index_count();

      match ::core::primitive::bool::then_some(
        matches!(
          c.charge_feature_indices(feature_index_count),
          Ok(TraversalBudget::Exhausted)
        ),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          return Ok(());
        }
        ::core::option::Option::None => {}
      }

      let Ok(feature_indices) = self.feature_indices();

      for feature_index in feature_indices {
        let Ok(value_76) = feature_index.get();

        let idx = value_76;
        let value_21 = idx;

        let Ok(value_64) = c.feature_indices_filter.contains(value_21);

        match ::core::primitive::bool::then_some(
          value_64 == console_core_integer_sets::Presence::Absent,
          (),
        ) {
          ::core::option::Option::Some(()) => {
            continue;
          }
          ::core::option::Option::None => {}
        }

        let value_22 = idx;

        let Ok(_presence_changed) = c.feature_indices.insert(value_22);
        let value_23 = idx;

        let Ok(_presence_changed) = c.feature_indices_filter.remove(value_23);
      }
    };

    Ok(())
  }
}

impl Feature<'_> {
  #[must_use]
  pub(crate) fn collect_lookups(
    &self,
  ) -> core::result::Result<Vec<u16>, console_core_never::Never> {
    let Ok(lookup_list_indices) = self.lookup_list_indices();

    Ok(
      lookup_list_indices
        .iter()
        .map(|idx| {
          let Ok(value_65) = idx.get();

          value_65
        })
        .collect(),
    )
  }
}

impl FeatureList<'_> {
  pub(crate) fn collect_lookups(
    &self,
    feature_indices: &IntSet<u16>,
  ) -> Result<IntSet<u16>, ReadError> {
    let Ok(features_records) = self.feature_records();
    let Ok(num_features) = self.feature_count();
    let Ok(font_data) = self.offset_data();
    let Ok(mut lookup_idxes) = IntSet::empty();

    let Ok(value_24) = feature_indices
      .membership()
      .map(|membership| membership == console_core_integer_sets::Membership::Exclusive);

    match ::core::primitive::bool::then_some(value_24, ()) {
      ::core::option::Option::Some(()) => {
        for feature_rec in (0..num_features).filter_map(|i| {
          let Ok(value_41) = feature_indices.contains(i);

          (value_41 == console_core_integer_sets::Presence::Present)
            .then(|| {
              let Ok(value_49) = console_core_number_conversion::index({
                let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(i);
                migrated_value
              });

              features_records.get(value_49)
            })
            .flatten()
        }) {
          let Ok(feature_offset) = feature_rec.feature_offset();
          let Ok(value_25) = feature_offset.nullness();
          let value_25_2 = matches!(value_25, font_types::Nullness::Null);

          match ::core::primitive::bool::then_some(value_25_2, ()) {
            ::core::option::Option::Some(()) => {
              continue;
            }
            ::core::option::Option::None => {}
          }

          let feature = feature_rec.feature(font_data)?;
          let Ok(collect_lookups) = feature.collect_lookups();
          let Ok(()) = lookup_idxes.extend_unsorted(collect_lookups);
        }
      }
      ::core::option::Option::None => {
        let Ok(value_26) = feature_indices.iter();

        for feature_rec in value_26.filter_map(|i| {
          let Ok(value_75) = console_core_number_conversion::index({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(i);
            migrated_value
          });

          features_records.get(value_75)
        }) {
          let Ok(feature_offset) = feature_rec.feature_offset();
          let Ok(value_27) = feature_offset.nullness();
          let value_27_2 = matches!(value_27, font_types::Nullness::Null);

          match ::core::primitive::bool::then_some(value_27_2, ()) {
            ::core::option::Option::Some(()) => {
              continue;
            }
            ::core::option::Option::None => {}
          }

          let feature = feature_rec.feature(font_data)?;
          let Ok(collect_lookups) = feature.collect_lookups();
          let Ok(()) = lookup_idxes.extend_unsorted(collect_lookups);
        }
      }
    }

    Ok(lookup_idxes)
  }
}

impl FeatureVariations<'_> {
  pub(crate) fn collect_lookups(
    &self,
    feature_indices: &IntSet<u16>,
  ) -> Result<IntSet<u16>, ReadError> {
    let Ok(mut out) = IntSet::empty();

    let Ok(feature_variation_records) = self.feature_variation_records();

    for variation_rec in feature_variation_records {
      let Ok(value_28) = self.offset_data();

      let feature_table_substitution = variation_rec.feature_table_substitution(value_28)?;
      let subs = match feature_table_substitution {
        Some(subs) => subs,
        None => {
          continue;
        }
      };

      let Ok(substitutions) = subs.substitutions();

      'over_sub_record_2: for sub_record in substitutions.iter().filter(|sub_rec| {
        let Ok(feature_index) = sub_rec.feature_index();
        let Ok(value_66) = feature_indices.contains(feature_index);

        value_66 == console_core_integer_sets::Presence::Present
      }) {
        let Ok(alternate_feature_offset) = sub_record.alternate_feature_offset();
        let Ok(value_30) = alternate_feature_offset.nullness();
        let value_30_2 = matches!(value_30, font_types::Nullness::Null);

        match ::core::primitive::bool::then_some(value_30_2, ()) {
          ::core::option::Option::Some(()) => {
            continue 'over_sub_record_2;
          }
          ::core::option::Option::None => {}
        }

        let Ok(value_31) = subs.offset_data();

        let sub_f = sub_record.alternate_feature(value_31)?;
        ({
          let Ok(lookup_list_indices) = sub_f.lookup_list_indices();
          let Ok(value) = out.extend_unsorted(lookup_list_indices.iter().map(|i| {
            let Ok(value_72) = i.get();

            value_72
          }));
          value
        });
      }
    }

    Ok(out)
  }
}

pub(crate) enum LayoutLookupList<'a> {
  Gsub(&'a SubstitutionLookupList<'a>),
  Gpos(&'a PositionLookupList<'a>),
}

pub(crate) struct LookupClosureCtx<'a> {
  visited_lookups: IntSet<u16>,
  inactive_lookups: IntSet<u16>,
  glyph_set: &'a IntSet<GlyphId>,
  lookup_count: u16,
  nesting_level_left: u8,
  lookup_list: &'a LayoutLookupList<'a>,
}

impl<'a> LookupClosureCtx<'a> {
  #[must_use]
  pub(crate) fn new(
    glyph_set: &'a IntSet<GlyphId>,
    lookup_list: &'a LayoutLookupList,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(value_32) = IntSet::empty();
    let Ok(value_33) = IntSet::empty();

    Ok(Self {
      visited_lookups: value_32,
      inactive_lookups: value_33,
      glyph_set,
      lookup_count: 0,
      nesting_level_left: MAX_NESTING_LEVEL,
      lookup_list,
    })
  }

  #[must_use]
  pub(crate) fn visited_lookups(
    &self,
  ) -> core::result::Result<&IntSet<u16>, console_core_never::Never> {
    Ok(&self.visited_lookups)
  }

  #[must_use]
  pub(crate) fn inactive_lookups(
    &self,
  ) -> core::result::Result<&IntSet<u16>, console_core_never::Never> {
    Ok(&self.inactive_lookups)
  }

  #[must_use]
  pub(crate) fn glyphs(
    &self,
  ) -> core::result::Result<&IntSet<GlyphId>, console_core_never::Never> {
    Ok(self.glyph_set)
  }

  pub(crate) fn set_lookup_inactive(
    &mut self,
    lookup_index: u16,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(_presence_changed) = self.inactive_lookups.insert(lookup_index);
    };

    Ok(())
  }

  pub(crate) fn lookup_budget(&self) -> Result<TraversalBudget, console_core_never::Never> {
    Ok(match self.lookup_count.cmp(&MAX_LOOKUP_VISIT_COUNT) {
      core::cmp::Ordering::Greater => TraversalBudget::Exhausted,
      core::cmp::Ordering::Less | core::cmp::Ordering::Equal => TraversalBudget::Available,
    })
  }

  pub(crate) fn visit_lookup(
    &mut self,
    lookup_index: u16,
  ) -> Result<VisitDecision, console_core_never::Never> {
    match self.lookup_budget() {
      Ok(TraversalBudget::Exhausted) => return Ok(VisitDecision::Skip),
      Ok(TraversalBudget::Available) => {}
    }

    self.lookup_count = self.lookup_count.wrapping_add(1);
    Ok(match self.visited_lookups.insert(lookup_index) {
      Ok(crate::Presence::Present) => VisitDecision::Skip,
      Ok(crate::Presence::Absent) => VisitDecision::Visit,
    })
  }

  pub(crate) fn recurse(&mut self, lookup_index: u16) -> Result<(), ReadError> {
    match ::core::primitive::bool::then_some(self.nesting_level_left == 0, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(
      {
        matches!(self.lookup_budget(), Ok(TraversalBudget::Exhausted)) || {
          let Ok(value_73) = self.visited_lookups.contains(lookup_index);

          value_73 == console_core_integer_sets::Presence::Present
        }
      },
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    match (1, &mut self.nesting_level_left) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u8::wrapping_sub(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };

    let position = u64::from(lookup_index);

    match self.lookup_list {
      LayoutLookupList::Gpos(lookuplist) => {
        let Ok(lookups) = lookuplist.lookups();
        self.apply_nested_lookup(lookups.get(position), lookup_index)?;
      }
      LayoutLookupList::Gsub(lookuplist) => {
        let Ok(lookups) = lookuplist.lookups();
        self.apply_nested_lookup(lookups.get(position), lookup_index)?;
      }
    }

    match (1, &mut self.nesting_level_left) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u8::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };

    Ok(())
  }

  fn apply_nested_lookup(
    &mut self,
    lookup: Result<impl LookupClosure, ReadError>,
    index: u16,
  ) -> Result<(), ReadError> {
    match lookup {
      Ok(lookup_2) => lookup_2.closure_lookups(self, index),
      Err(ReadError::NullOffset | ReadError::InvalidCollectionIndex(_)) => Ok(()),
      Err(unreadable_lookup) => Err(unreadable_lookup),
    }
  }
}

pub(crate) trait LookupClosure {
  fn closure_lookups(&self, _c: &mut LookupClosureCtx, _arg: u16) -> Result<(), ReadError> {
    Ok(())
  }
}

pub trait Intersect {
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(
      explicit007_no_bool_return,
      reason = "the published `Intersect::intersects` trait method retains the Boolean result used by existing predicate callers and implementations"
    )
  )]
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError>;
}

impl Intersect for ClassDef<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    match self {
      ClassDef::Format1(table) => table.intersects(glyph_set),
      ClassDef::Format2(table) => table.intersects(glyph_set),
    }
  }
}

impl Intersect for ClassDefFormat1<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(glyph_count) = self.glyph_count();

    match ::core::primitive::bool::then_some(glyph_count == 0, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(false);
      }
      ::core::option::Option::None => {}
    }

    let Ok(start_glyph_id) = self.start_glyph_id();
    let Ok(start) = start_glyph_id.to_u32();
    let end = ::core::primitive::u32::wrapping_add(start, {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(glyph_count);
      migrated_value
    });

    let start_glyph = GlyphId::from(start);
    let Ok(class_values) = self.class_value_array();
    let Ok(value_36) = glyph_set.contains(start_glyph);

    match ::core::primitive::bool::then_some(
      (value_36 == console_core_integer_sets::Presence::Present)
        && *match <[_]>::get(&*class_values, 0) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds);
          }
        } != 0,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(true);
      }
      ::core::option::Option::None => {}
    }

    let Ok(following_glyphs) = glyph_set.iter_after(start_glyph);

    for g in following_glyphs {
      let Ok(g_2) = g.to_u32();

      match ::core::primitive::bool::then_some(g_2 >= end, ()) {
        ::core::option::Option::Some(()) => {
          break;
        }
        ::core::option::Option::None => {}
      }

      let Ok(value_78) = console_core_number_conversion::index({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(
          ::core::primitive::u32::wrapping_sub(g_2, start),
        );
        migrated_value
      });

      let class = match class_values.get(value_78) {
        Some(class) => class,
        None => {
          break;
        }
      };
      let Ok(value_47) = class.get();

      match ::core::primitive::bool::then_some(value_47 != 0, ()) {
        ::core::option::Option::Some(()) => {
          return Ok(true);
        }
        ::core::option::Option::None => {}
      }
    }

    Ok(false)
  }
}

impl Intersect for ClassDefFormat2<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(num_ranges) = self.class_range_count();
    let num_bits = ::core::primitive::u32::wrapping_sub(16, num_ranges.leading_zeros());
    let Ok(value_38) = glyph_set.len();

    match ::core::primitive::bool::then_some(
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(num_ranges);
        migrated_value
      }) > ::core::primitive::u64::wrapping_mul(value_38, {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(num_bits);
        migrated_value
      }),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let Ok(value_95) = glyph_set.iter();

        for g in value_95.map(|g| {
          let Ok(value_48) = g.to_u32();

          GlyphId16::from({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(value_48);
            migrated_value
          })
        }) {
          match ::core::primitive::bool::then_some(self.get(g) != 0, ()) {
            ::core::option::Option::Some(()) => {
              return Ok(true);
            }
            ::core::option::Option::None => {}
          }
        }
      }
      ::core::option::Option::None => {
        let Ok(class_range_records) = self.class_range_records();

        for record in class_range_records {
          let Ok(start_glyph_id) = record.start_glyph_id();
          let first = GlyphId::from(start_glyph_id);
          let Ok(end_glyph_id) = record.end_glyph_id();
          let last = GlyphId::from(end_glyph_id);
          let Ok(value_39) = glyph_set.intersects_range(first..=last);

          match ::core::primitive::bool::then_some(
            (value_39 == console_core_integer_sets::Overlap::Intersecting)
              && ({
                let Ok(explicit_value) = record.class();
                explicit_value
              }) != 0,
            (),
          ) {
            ::core::option::Option::Some(()) => {
              return Ok(true);
            }
            ::core::option::Option::None => {}
          }
        }
      }
    }

    Ok(false)
  }
}

impl<'a, T, Ext> LookupClosure for Subtables<'a, T, Ext>
where
  T: LookupClosure + Intersect + FontRead<'a> + 'a,
  Ext: ExtensionLookup<'a, T> + 'a,
{
  fn closure_lookups(&self, c: &mut LookupClosureCtx, arg: u16) -> Result<(), ReadError> {
    for t in self.iter().filter_map(|table| match table {
      Err(ReadError::NullOffset) => None,
      Ok(value) => Some(Ok(value)),
      Err(unreadable_layout_record) => Some(Err(unreadable_layout_record)),
    }) {
      let lookup_subtable = t?;
      lookup_subtable.closure_lookups(c, arg)?;
    }

    Ok(())
  }
}

impl<'a, T> Intersect for ArrayOfOffsets<'a, T, Offset16>
where
  T: Intersect + FontRead<'a>,
{
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(value_40) = self.iter();

    for t in value_40.filter_map(|table| match table {
      Err(ReadError::NullOffset) => None,
      Ok(value) => Some(Ok(value)),
      Err(unreadable_layout_record) => Some(Err(unreadable_layout_record)),
    }) {
      let positioning_subtable = t?;
      let intersects = positioning_subtable.intersects(glyph_set)?;

      match ::core::primitive::bool::then_some(intersects, ()) {
        ::core::option::Option::Some(()) => {
          return Ok(true);
        }
        ::core::option::Option::None => {}
      }
    }

    Ok(false)
  }
}

pub(crate) enum ContextFormat1<'a> {
  Plain(SequenceContextFormat1<'a>),
  Chain(ChainedSequenceContextFormat1<'a>),
}

pub(crate) enum Format1RuleSet<'a> {
  Plain(SequenceRuleSet<'a>),
  Chain(ChainedSequenceRuleSet<'a>),
}

pub(crate) enum Format1Rule<'a> {
  Plain(SequenceRule<'a>),
  Chain(ChainedSequenceRule<'a>),
}

impl ContextFormat1<'_> {
  pub(crate) fn coverage(&self) -> Result<Option<CoverageTable<'_>>, ReadError> {
    match self {
      ContextFormat1::Plain(table) => {
        let Ok(offset) = table.coverage_offset();
        let Ok(nullness) = offset.nullness();

        match nullness {
          font_types::Nullness::NonNull => table.coverage().map(Some),
          font_types::Nullness::Null => Ok(None),
        }
      }
      ContextFormat1::Chain(table) => {
        let Ok(offset) = table.coverage_offset();
        let Ok(nullness) = offset.nullness();

        match nullness {
          font_types::Nullness::NonNull => table.coverage().map(Some),
          font_types::Nullness::Null => Ok(None),
        }
      }
    }
  }

  #[must_use]
  pub(crate) fn rule_sets(
    &self,
  ) -> core::result::Result<
    impl Iterator<Item = Result<Option<Format1RuleSet<'_>>, ReadError>>,
    console_core_never::Never,
  > {
    let (left, right) = match self {
      ContextFormat1::Plain(table) => {
        let Ok(seq_rule_sets) = table.seq_rule_sets();
        let Ok(value_77) = seq_rule_sets.iter();

        (
          Some(value_77.map(|rs| rs.map(|rs_2| rs_2.map(Format1RuleSet::Plain)))),
          None,
        )
      }
      ContextFormat1::Chain(table) => {
        let Ok(chained_seq_rule_sets) = table.chained_seq_rule_sets();
        let Ok(value_79) = chained_seq_rule_sets.iter();

        (
          None,
          Some(value_79.map(|rs| rs.map(|rs_3| rs_3.map(Format1RuleSet::Chain)))),
        )
      }
    };

    Ok(
      left
        .into_iter()
        .flatten()
        .chain(right.into_iter().flatten()),
    )
  }
}

impl Format1RuleSet<'_> {
  #[must_use]
  pub(crate) fn rules(
    &self,
  ) -> core::result::Result<
    impl Iterator<Item = Result<Option<Format1Rule<'_>>, ReadError>>,
    console_core_never::Never,
  > {
    let (left, right) = match self {
      Self::Plain(table) => {
        let Ok(seq_rules) = table.seq_rules();
        let Ok(value_82) = seq_rules.iter_as_nullable();

        (
          Some(value_82.map(|rule| rule.map(|r| r.map(Format1Rule::Plain)))),
          None,
        )
      }
      Self::Chain(table) => {
        let Ok(chained_seq_rules) = table.chained_seq_rules();
        let Ok(value_83) = chained_seq_rules.iter_as_nullable();

        (
          None,
          Some(value_83.map(|rule| rule.map(|r| r.map(Format1Rule::Chain)))),
        )
      }
    };

    Ok(
      left
        .into_iter()
        .flatten()
        .chain(right.into_iter().flatten()),
    )
  }
}

impl Format1Rule<'_> {
  #[must_use]
  pub(crate) fn input_sequence(
    &self,
  ) -> core::result::Result<&[BigEndian<GlyphId16>], console_core_never::Never> {
    Ok(match self {
      Self::Plain(table) => {
        let Ok(input_sequence) = table.input_sequence();

        input_sequence
      }
      Self::Chain(table) => {
        let Ok(input_sequence) = table.input_sequence();

        input_sequence
      }
    })
  }

  #[must_use]
  pub(crate) fn lookup_records(
    &self,
  ) -> core::result::Result<&[SequenceLookupRecord], console_core_never::Never> {
    Ok(match self {
      Self::Plain(table) => {
        let Ok(seq_lookup_records) = table.seq_lookup_records();

        seq_lookup_records
      }
      Self::Chain(table) => {
        let Ok(seq_lookup_records) = table.seq_lookup_records();

        seq_lookup_records
      }
    })
  }
}

impl Intersect for &[BigEndian<GlyphId16>] {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    Ok(self.iter().all(|g| {
      let Ok(value_84) = g.get();

      let Ok(value_74) = glyph_set.contains(GlyphId::from(value_84));

      value_74 == console_core_integer_sets::Presence::Present
    }))
  }
}

impl Intersect for Format1Rule<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    match self {
      Self::Plain(table) => {
        let Ok(input_sequence) = table.input_sequence();

        input_sequence.intersects(glyph_set)
      }
      Self::Chain(table) => {
        let Ok(backtrack_sequence) = table.backtrack_sequence();
        let intersects = backtrack_sequence.intersects(glyph_set)?;

        Ok(
          intersects
            && {
              let checked_value = ({
                let Ok(explicit_value) = table.input_sequence();
                explicit_value
              })
              .intersects(glyph_set)?;
              checked_value
            }
            && {
              let checked_value_2 = ({
                let Ok(explicit_value) = table.lookahead_sequence();
                explicit_value
              })
              .intersects(glyph_set)?;
              checked_value_2
            },
        )
      }
    }
  }
}

impl LookupClosure for Format1Rule<'_> {
  fn closure_lookups(&self, c: &mut LookupClosureCtx, _arg: u16) -> Result<(), ReadError> {
    match ::core::primitive::bool::then_some(
      matches!(c.lookup_budget(), Ok(TraversalBudget::Exhausted))
        || !{
          let glyph_overlap = self.intersects({
            let Ok(explicit_value) = c.glyphs();
            explicit_value
          })?;
          glyph_overlap
        },
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let Ok(lookup_records) = self.lookup_records();

    for lookup_record in lookup_records {
      let Ok(index) = lookup_record.lookup_list_index();
      c.recurse(index)?;
    }

    Ok(())
  }
}

pub(crate) enum ContextFormat2<'a> {
  Plain(SequenceContextFormat2<'a>),
  Chain(ChainedSequenceContextFormat2<'a>),
}

pub(crate) enum Format2RuleSet<'a> {
  Plain(ClassSequenceRuleSet<'a>),
  Chain(ChainedClassSequenceRuleSet<'a>),
}

pub(crate) enum Format2Rule<'a> {
  Plain(ClassSequenceRule<'a>),
  Chain(ChainedClassSequenceRule<'a>),
}

impl ContextFormat2<'_> {
  pub(crate) fn coverage(&self) -> Result<Option<CoverageTable<'_>>, ReadError> {
    match self {
      ContextFormat2::Plain(table) => {
        let Ok(offset) = table.coverage_offset();
        let Ok(nullness) = offset.nullness();

        match nullness {
          font_types::Nullness::NonNull => table.coverage().map(Some),
          font_types::Nullness::Null => Ok(None),
        }
      }
      ContextFormat2::Chain(table) => {
        let Ok(offset) = table.coverage_offset();
        let Ok(nullness) = offset.nullness();

        match nullness {
          font_types::Nullness::NonNull => table.coverage().map(Some),
          font_types::Nullness::Null => Ok(None),
        }
      }
    }
  }

  pub(crate) fn input_class_def(&self) -> Result<Option<ClassDef<'_>>, ReadError> {
    match self {
      ContextFormat2::Plain(table_ref) => {
        let Ok(offset) = table_ref.class_def_offset();
        let Ok(nullness) = offset.nullness();

        match nullness {
          font_types::Nullness::NonNull => table_ref.class_def().map(Some),
          font_types::Nullness::Null => Ok(None),
        }
      }
      ContextFormat2::Chain(table_ref) => {
        let Ok(offset) = table_ref.input_class_def_offset();
        let Ok(nullness) = offset.nullness();

        match nullness {
          font_types::Nullness::NonNull => table_ref.input_class_def().map(Some),
          font_types::Nullness::Null => Ok(None),
        }
      }
    }
  }

  #[must_use]
  pub(crate) fn rule_sets(
    &self,
  ) -> core::result::Result<
    impl Iterator<Item = Result<Option<Format2RuleSet<'_>>, ReadError>>,
    console_core_never::Never,
  > {
    let (left, right) = match self {
      ContextFormat2::Plain(table) => {
        let Ok(class_seq_rule_sets) = table.class_seq_rule_sets();
        let Ok(value_80) = class_seq_rule_sets.iter();

        (
          Some(value_80.map(|rs| rs.map(|rs_2| rs_2.map(Format2RuleSet::Plain)))),
          None,
        )
      }
      ContextFormat2::Chain(table) => {
        let Ok(chained_class_seq_rule_sets) = table.chained_class_seq_rule_sets();
        let Ok(value_81) = chained_class_seq_rule_sets.iter();

        (
          None,
          Some(value_81.map(|rs| rs.map(|rs_3| rs_3.map(Format2RuleSet::Chain)))),
        )
      }
    };

    Ok(
      left
        .into_iter()
        .flatten()
        .chain(right.into_iter().flatten()),
    )
  }
}

impl Format2RuleSet<'_> {
  #[must_use]
  pub(crate) fn rules(
    &self,
  ) -> core::result::Result<
    impl Iterator<Item = Result<Option<Format2Rule<'_>>, ReadError>>,
    console_core_never::Never,
  > {
    let (left, right) = match self {
      Format2RuleSet::Plain(table) => {
        let Ok(class_seq_rules) = table.class_seq_rules();
        let Ok(value_85) = class_seq_rules.iter_as_nullable();

        (
          Some(value_85.map(|rule| rule.map(|r| r.map(Format2Rule::Plain)))),
          None,
        )
      }
      Format2RuleSet::Chain(table) => {
        let Ok(chained_class_seq_rules) = table.chained_class_seq_rules();
        let Ok(value_86) = chained_class_seq_rules.iter_as_nullable();

        (
          None,
          Some(value_86.map(|rule| rule.map(|r| r.map(Format2Rule::Chain)))),
        )
      }
    };

    Ok(
      left
        .into_iter()
        .flatten()
        .chain(right.into_iter().flatten()),
    )
  }
}

impl Format2Rule<'_> {
  #[must_use]
  pub(crate) fn input_sequence(
    &self,
  ) -> core::result::Result<&[BigEndian<u16>], console_core_never::Never> {
    Ok(match self {
      Self::Plain(table) => {
        let Ok(input_sequence) = table.input_sequence();

        input_sequence
      }
      Self::Chain(table) => {
        let Ok(input_sequence) = table.input_sequence();

        input_sequence
      }
    })
  }

  #[must_use]
  pub(crate) fn lookup_records(
    &self,
  ) -> core::result::Result<&[SequenceLookupRecord], console_core_never::Never> {
    Ok(match self {
      Self::Plain(table) => {
        let Ok(seq_lookup_records) = table.seq_lookup_records();

        seq_lookup_records
      }
      Self::Chain(table) => {
        let Ok(seq_lookup_records) = table.seq_lookup_records();

        seq_lookup_records
      }
    })
  }

  #[must_use]
  pub(crate) fn intersects(
    &self,
    input_classes: &IntSet<u16>,
    backtrack_classes: &IntSet<u16>,
    lookahead_classes: &IntSet<u16>,
  ) -> Result<crate::Overlap, console_core_never::Never> {
    match self {
      Self::Plain(table) => table.intersects(input_classes),
      Self::Chain(table) => table.intersects(input_classes, backtrack_classes, lookahead_classes),
    }
  }
}

impl ClassSequenceRule<'_> {
  #[must_use]
  fn intersects(
    &self,
    input_classes: &IntSet<u16>,
  ) -> Result<crate::Overlap, console_core_never::Never> {
    let Ok(input_sequence) = self.input_sequence();

    Ok(crate::Overlap::from(input_sequence.iter().all(|c| {
      let Ok(value_42) = c.get();

      let Ok(value_68) = input_classes.contains(value_42);

      value_68 == console_core_integer_sets::Presence::Present
    })))
  }
}

impl ChainedClassSequenceRule<'_> {
  #[must_use]
  fn intersects(
    &self,
    input_classes: &IntSet<u16>,
    backtrack_classes: &IntSet<u16>,
    lookahead_classes: &IntSet<u16>,
  ) -> Result<crate::Overlap, console_core_never::Never> {
    let Ok(input_sequence) = self.input_sequence();

    Ok(crate::Overlap::from(
      input_sequence.iter().all(|c| {
        let Ok(value_43) = c.get();

        let Ok(value_69) = input_classes.contains(value_43);

        value_69 == console_core_integer_sets::Presence::Present
      }) && ({
        let Ok(explicit_value) = self.backtrack_sequence();
        explicit_value
      })
      .iter()
      .all(|c| {
        let Ok(value_44) = c.get();

        let Ok(value_70) = backtrack_classes.contains(value_44);

        value_70 == console_core_integer_sets::Presence::Present
      }) && ({
        let Ok(explicit_value) = self.lookahead_sequence();
        explicit_value
      })
      .iter()
      .all(|c| {
        let Ok(value_45) = c.get();

        let Ok(value_71) = lookahead_classes.contains(value_45);

        value_71 == console_core_integer_sets::Presence::Present
      }),
    ))
  }
}

pub(crate) enum ContextFormat3<'a> {
  Plain(SequenceContextFormat3<'a>),
  Chain(ChainedSequenceContextFormat3<'a>),
}

impl ContextFormat3<'_> {
  #[must_use]
  pub(crate) fn coverages(
    &self,
  ) -> core::result::Result<ArrayOfOffsets<'_, CoverageTable<'_>>, console_core_never::Never> {
    Ok(match self {
      ContextFormat3::Plain(table) => {
        let Ok(coverages) = table.coverages();

        coverages
      }
      ContextFormat3::Chain(table) => {
        let Ok(input_coverages) = table.input_coverages();

        input_coverages
      }
    })
  }

  #[must_use]
  pub(crate) fn lookup_records(
    &self,
  ) -> core::result::Result<&[SequenceLookupRecord], console_core_never::Never> {
    Ok(match self {
      ContextFormat3::Plain(table) => {
        let Ok(seq_lookup_records) = table.seq_lookup_records();

        seq_lookup_records
      }
      ContextFormat3::Chain(table) => {
        let Ok(seq_lookup_records) = table.seq_lookup_records();

        seq_lookup_records
      }
    })
  }

  pub(crate) fn matches_glyphs(
    &self,
    glyphs: &IntSet<GlyphId>,
  ) -> Result<crate::Overlap, ReadError> {
    let (backtrack, lookahead) = match self {
      Self::Plain(_) => (None, None),
      Self::Chain(table) => (
        Some(table.backtrack_coverages()),
        Some(table.lookahead_coverages()),
      ),
    };

    let Ok(coverages) = self.coverages();
    let Ok(value_46) = coverages.iter_as_nullable();

    for coverage in value_46
      .chain(backtrack.into_iter().flat_map(|x| {
        let Ok(value_88) = ({
          let Ok(explicit_value) = x;
          explicit_value
        })
        .iter_as_nullable();

        value_88
      }))
      .chain(lookahead.into_iter().flat_map(|x| {
        let Ok(value_89) = ({
          let Ok(explicit_value) = x;
          explicit_value
        })
        .iter_as_nullable();

        value_89
      }))
    {
      let context_coverage = coverage?;
      let coverage_3 = match context_coverage {
        Some(coverage_2) => coverage_2,
        None => {
          return Ok(crate::Overlap::Disjoint);
        }
      };

      match ::core::primitive::bool::then_some(!coverage_3.intersects(glyphs), ()) {
        ::core::option::Option::Some(()) => {
          return Ok(crate::Overlap::Disjoint);
        }
        ::core::option::Option::None => {}
      }
    }

    Ok(crate::Overlap::Intersecting)
  }
}

impl Intersect for ContextFormat1<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let own_coverage = self.coverage()?;
    let coverage = match own_coverage {
      Some(coverage) => coverage,
      None => {
        return Ok(false);
      }
    };

    for rule_set in coverage
      .iter()
      .zip({
        let Ok(explicit_value) = self.rule_sets();
        explicit_value
      })
      .filter_map(|(g, rule_set)| {
        rule_set.transpose().filter(|_| {
          let Ok(value_90) = glyph_set.contains(GlyphId::from(g));

          value_90 == console_core_integer_sets::Presence::Present
        })
      })
    {
      let rule_set_2 = rule_set?;
      let Ok(rules) = rule_set_2.rules();

      'over_rule_2: for rule in rules {
        let sequence_rule = rule?;
        let rule_3 = match sequence_rule {
          Some(rule_2) => rule_2,
          None => {
            continue 'over_rule_2;
          }
        };

        let intersects = rule_3.intersects(glyph_set)?;

        match ::core::primitive::bool::then_some(intersects, ()) {
          ::core::option::Option::Some(()) => {
            return Ok(true);
          }
          ::core::option::Option::None => {}
        }
      }
    }

    Ok(false)
  }
}

impl LookupClosure for ContextFormat1<'_> {
  fn closure_lookups(&self, c: &mut LookupClosureCtx, arg: u16) -> Result<(), ReadError> {
    let own_coverage = self.coverage()?;
    let coverage = match own_coverage {
      Some(coverage) => coverage,
      None => {
        return Ok(());
      }
    };
    let Ok(glyph_set) = c.glyphs();

    let intersected_idxes: IntSet<u16> = coverage
      .iter()
      .enumerate()
      .filter(|&(_, g)| {
        let Ok(value_91) = glyph_set.contains(GlyphId::from(g));

        value_91 == console_core_integer_sets::Presence::Present
      })
      .map(|(idx, _)| {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(idx);
        migrated_value
      })
      .collect();

    let Ok(rule_sets) = self.rule_sets();

    for rule_set in rule_sets.enumerate().filter_map(|(idx, rule_set)| {
      rule_set.transpose().filter(|_| {
        let Ok(value_92) = intersected_idxes.contains({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(idx);
          migrated_value
        });

        value_92 == console_core_integer_sets::Presence::Present
      })
    }) {
      match ::core::primitive::bool::then_some(
        matches!(c.lookup_budget(), Ok(TraversalBudget::Exhausted)),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          return Ok(());
        }
        ::core::option::Option::None => {}
      }

      let rule_set_2 = rule_set?;
      let Ok(rules) = rule_set_2.rules();

      'over_rule_2: for rule in rules {
        let chain_rule = rule?;
        let rule_3 = match chain_rule {
          Some(rule_2) => rule_2,
          None => {
            continue 'over_rule_2;
          }
        };
        rule_3.closure_lookups(c, arg)?;
      }
    }

    Ok(())
  }
}

impl Intersect for ContextFormat2<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let own_coverage = self.coverage()?;
    let coverage = match own_coverage {
      Some(coverage) => coverage,
      None => {
        return Ok(false);
      }
    };
    let retained_coverage_glyphs = coverage.intersect_set(glyph_set);
    let Ok(value_50) = retained_coverage_glyphs.is_empty();

    match ::core::primitive::bool::then_some(
      value_50 == console_core_integer_sets::Emptiness::Empty,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(false);
      }
      ::core::option::Option::None => {}
    }

    let own_input_class_def = self.input_class_def()?;
    let input_class_def = match own_input_class_def {
      Some(input_class_def) => input_class_def,
      None => {
        return Ok(false);
      }
    };
    let coverage_glyph_classes = input_class_def.intersect_classes(&retained_coverage_glyphs);
    let input_glyph_classes = input_class_def.intersect_classes(glyph_set);

    let backtrack_classes = match self {
      Self::Plain(_) => {
        let Ok(value_107) = IntSet::empty();

        value_107
      }
      Self::Chain(table) => {
        let Ok(backtrack_class_def_offset) = table.backtrack_class_def_offset();
        let Ok(value_51) = backtrack_class_def_offset.nullness();
        let value_51_2 = matches!(value_51, font_types::Nullness::Null);

        match ::core::primitive::bool::then_some(value_51_2, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(value_100) = IntSet::empty();

            value_100
          }
          ::core::option::Option::None => {
            let backtrack_class_def = table.backtrack_class_def()?;

            backtrack_class_def.intersect_classes(glyph_set)
          }
        }
      }
    };
    let lookahead_classes = match self {
      Self::Plain(_) => {
        let Ok(value_108) = IntSet::empty();

        value_108
      }
      Self::Chain(table) => {
        let Ok(lookahead_class_def_offset) = table.lookahead_class_def_offset();
        let Ok(value_52) = lookahead_class_def_offset.nullness();
        let value_52_2 = matches!(value_52, font_types::Nullness::Null);

        match ::core::primitive::bool::then_some(value_52_2, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(value_102) = IntSet::empty();

            value_102
          }
          ::core::option::Option::None => {
            let lookahead_class_def = table.lookahead_class_def()?;

            lookahead_class_def.intersect_classes(glyph_set)
          }
        }
      }
    };

    let Ok(rule_sets) = self.rule_sets();

    for rule_set in rule_sets.enumerate().filter_map(|(c, rule_set)| {
      let Ok(value_53) = coverage_glyph_classes.contains({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(c);
        migrated_value
      });

      (value_53 == console_core_integer_sets::Presence::Present)
        .then_some(rule_set.transpose())
        .flatten()
    }) {
      let rule_set_2 = rule_set?;
      let Ok(rules) = rule_set_2.rules();

      'over_rule_2: for rule in rules {
        let class_rule = rule?;
        let rule_3 = match class_rule {
          Some(rule_2) => rule_2,
          None => {
            continue 'over_rule_2;
          }
        };

        match ::core::primitive::bool::then_some(
          matches!(
            rule_3.intersects(&input_glyph_classes, &backtrack_classes, &lookahead_classes),
            Ok(crate::Overlap::Intersecting)
          ),
          (),
        ) {
          ::core::option::Option::Some(()) => {
            return Ok(true);
          }
          ::core::option::Option::None => {}
        }
      }
    }

    Ok(false)
  }
}

impl LookupClosure for ContextFormat2<'_> {
  fn closure_lookups(&self, c: &mut LookupClosureCtx, _arg: u16) -> Result<(), ReadError> {
    let own_coverage = self.coverage()?;
    let coverage = match own_coverage {
      Some(coverage) => coverage,
      None => {
        return Ok(());
      }
    };
    let Ok(glyph_set) = c.glyphs();
    let retained_coverage_glyphs = coverage.intersect_set(glyph_set);
    let Ok(value_54) = retained_coverage_glyphs.is_empty();

    match ::core::primitive::bool::then_some(
      value_54 == console_core_integer_sets::Emptiness::Empty,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let own_input_class_def = self.input_class_def()?;
    let input_class_def = match own_input_class_def {
      Some(input_class_def) => input_class_def,
      None => {
        return Ok(());
      }
    };
    let coverage_glyph_classes = input_class_def.intersect_classes(&retained_coverage_glyphs);
    let input_glyph_classes = input_class_def.intersect_classes(glyph_set);

    let backtrack_classes = match self {
      Self::Plain(_) => {
        let Ok(value_109) = IntSet::empty();

        value_109
      }
      Self::Chain(table) => {
        let Ok(backtrack_class_def_offset) = table.backtrack_class_def_offset();
        let Ok(value_55) = backtrack_class_def_offset.nullness();
        let value_55_2 = matches!(value_55, font_types::Nullness::Null);

        match ::core::primitive::bool::then_some(value_55_2, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(value_104) = IntSet::empty();

            value_104
          }
          ::core::option::Option::None => {
            let backtrack_class_def = table.backtrack_class_def()?;

            backtrack_class_def.intersect_classes(glyph_set)
          }
        }
      }
    };
    let lookahead_classes = match self {
      Self::Plain(_) => {
        let Ok(value_110) = IntSet::empty();

        value_110
      }
      Self::Chain(table) => {
        let Ok(lookahead_class_def_offset) = table.lookahead_class_def_offset();
        let Ok(value_56) = lookahead_class_def_offset.nullness();
        let value_56_2 = matches!(value_56, font_types::Nullness::Null);

        match ::core::primitive::bool::then_some(value_56_2, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(value_106) = IntSet::empty();

            value_106
          }
          ::core::option::Option::None => {
            let lookahead_class_def = table.lookahead_class_def()?;

            lookahead_class_def.intersect_classes(glyph_set)
          }
        }
      }
    };

    let Ok(rule_sets) = self.rule_sets();

    for rule_set in rule_sets.enumerate().filter_map(|(c_2, rule_set)| {
      let Ok(value_57) = coverage_glyph_classes.contains({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(c_2);
        migrated_value
      });

      (value_57 == console_core_integer_sets::Presence::Present)
        .then_some(rule_set.transpose())
        .flatten()
    }) {
      match ::core::primitive::bool::then_some(
        matches!(c.lookup_budget(), Ok(TraversalBudget::Exhausted)),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          return Ok(());
        }
        ::core::option::Option::None => {}
      }

      let rule_set_2 = rule_set?;
      let Ok(rules) = rule_set_2.rules();

      'over_rule_2: for rule in rules {
        match ::core::primitive::bool::then_some(
          matches!(c.lookup_budget(), Ok(TraversalBudget::Exhausted)),
          (),
        ) {
          ::core::option::Option::Some(()) => {
            return Ok(());
          }
          ::core::option::Option::None => {}
        }

        let chained_class_rule = rule?;
        let rule_3 = match chained_class_rule {
          Some(rule_2) => rule_2,
          None => {
            continue 'over_rule_2;
          }
        };

        match ::core::primitive::bool::then_some(
          !matches!(
            rule_3.intersects(&input_glyph_classes, &backtrack_classes, &lookahead_classes),
            Ok(crate::Overlap::Intersecting)
          ),
          (),
        ) {
          ::core::option::Option::Some(()) => {
            continue 'over_rule_2;
          }
          ::core::option::Option::None => {}
        }

        let Ok(lookup_records) = rule_3.lookup_records();

        for lookup_record in lookup_records {
          let Ok(index) = lookup_record.lookup_list_index();
          c.recurse(index)?;
        }
      }
    }

    Ok(())
  }
}

impl Intersect for ContextFormat3<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    self
      .matches_glyphs(glyph_set)
      .map(|overlap| overlap == crate::Overlap::Intersecting)
  }
}

impl LookupClosure for ContextFormat3<'_> {
  fn closure_lookups(&self, c: &mut LookupClosureCtx, _arg: u16) -> Result<(), ReadError> {
    let Ok(glyphs) = c.glyphs();
    let intersects = self.intersects(glyphs)?;

    match ::core::primitive::bool::then_some(!intersects, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let Ok(lookup_records) = self.lookup_records();

    for lookup_record in lookup_records {
      let Ok(index) = lookup_record.lookup_list_index();
      c.recurse(index)?;
    }

    Ok(())
  }
}

impl Intersect for SequenceContext<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    match self {
      Self::Format1(table) => ContextFormat1::Plain(table.clone()).intersects(glyph_set),
      Self::Format2(table) => ContextFormat2::Plain(table.clone()).intersects(glyph_set),
      Self::Format3(table) => ContextFormat3::Plain(table.clone()).intersects(glyph_set),
    }
  }
}

impl LookupClosure for SequenceContext<'_> {
  fn closure_lookups(&self, c: &mut LookupClosureCtx, arg: u16) -> Result<(), ReadError> {
    match self {
      Self::Format1(table) => ContextFormat1::Plain(table.clone()).closure_lookups(c, arg),
      Self::Format2(table) => ContextFormat2::Plain(table.clone()).closure_lookups(c, arg),
      Self::Format3(table) => ContextFormat3::Plain(table.clone()).closure_lookups(c, arg),
    }
  }
}

impl Intersect for ChainedSequenceContext<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    match self {
      Self::Format1(table) => ContextFormat1::Chain(table.clone()).intersects(glyph_set),
      Self::Format2(table) => ContextFormat2::Chain(table.clone()).intersects(glyph_set),
      Self::Format3(table) => ContextFormat3::Chain(table.clone()).intersects(glyph_set),
    }
  }
}

impl LookupClosure for ChainedSequenceContext<'_> {
  fn closure_lookups(&self, c: &mut LookupClosureCtx, arg: u16) -> Result<(), ReadError> {
    match self {
      Self::Format1(table) => ContextFormat1::Chain(table.clone()).closure_lookups(c, arg),
      Self::Format2(table) => ContextFormat2::Chain(table.clone()).closure_lookups(c, arg),
      Self::Format3(table) => ContextFormat3::Chain(table.clone()).closure_lookups(c, arg),
    }
  }
}
