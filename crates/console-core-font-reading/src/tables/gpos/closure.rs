//! support closure for GPOS

use super::{
  CursivePosFormat1, ExtensionPosFormat1, ExtensionSubtable, Gpos, MarkBasePosFormat1,
  MarkLigPosFormat1, MarkMarkPosFormat1, PairPos, PairPosFormat1, PairPosFormat2, PairSet,
  PositionLookup, PositionLookupList, PositionSubtables, SinglePos, SinglePosFormat1,
  SinglePosFormat2,
};
use crate::{FontRead, GlyphId, ReadError, Tag, collections::IntSet};

#[cfg(feature = "std")]
use crate::tables::layout::{Intersect, LayoutLookupList, LookupClosure, LookupClosureCtx};

impl Gpos<'_> {
  pub fn collect_features(
    &self,
    scripts: &IntSet<Tag>,
    languages: &IntSet<Tag>,
    features: &IntSet<Tag>,
  ) -> Result<IntSet<u16>, ReadError> {
    let Ok(script_list_offset) = self.script_list_offset();
    let Ok(value_1) = script_list_offset.nullness();
    let value_1_2 = matches!(value_1, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some({
      value_1_2 || {
        let Ok(feature_list_offset) = self.feature_list_offset();
        let Ok(value_26) = feature_list_offset.nullness();
        let value_26_2 = matches!(value_26, font_types::Nullness::Null);

        value_26_2
      }
    }, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(value_3) = IntSet::empty();

        return Ok(value_3);
      }
      ::core::option::Option::None => {}
    }

    let feature_list = self.feature_list()?;
    let script_list = self.script_list()?;
    let Ok(value_4) = self.offset_data();

    let Ok(head_ptr) = value_4.base_address();
    script_list.collect_features(head_ptr, &feature_list, scripts, languages, features)
  }

  pub fn collect_lookups(&self, feature_indices: &IntSet<u16>) -> Result<IntSet<u16>, ReadError> {
    let Ok(feature_list_offset) = self.feature_list_offset();
    let Ok(value_5) = feature_list_offset.nullness();
    let value_5_2 = matches!(value_5, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some(value_5_2, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(value_6) = IntSet::empty();

        return Ok(value_6);
      }
      ::core::option::Option::None => {}
    }

    let feature_list = self.feature_list()?;
    let mut lookup_indices = feature_list.collect_lookups(feature_indices)?;

    let own_feature_variations = self.feature_variations()?;

    match own_feature_variations {
      Some(feature_variations) => {
        let subs_lookup_indices = feature_variations.collect_lookups(feature_indices)?;
        let Ok(()) = lookup_indices.union(&subs_lookup_indices);
      }
      None => {}
    }

    Ok(lookup_indices)
  }

  pub fn closure_lookups(
    &self,
    glyphs: &IntSet<GlyphId>,
    lookup_indices: &mut IntSet<u16>,
  ) -> Result<(), ReadError> {
    let Ok(lookup_list_offset) = self.lookup_list_offset();
    let Ok(value_7) = lookup_list_offset.nullness();
    let value_7_2 = matches!(value_7, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some(value_7_2, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let lookup_list = self.lookup_list()?;
    lookup_list.closure_lookups(glyphs, lookup_indices)
  }
}

impl PositionLookupList<'_> {
  pub fn closure_lookups(
    &self,
    glyph_set: &IntSet<GlyphId>,
    lookup_indices: &mut IntSet<u16>,
  ) -> Result<(), ReadError> {
    let Ok(lookup_count) = self.lookup_count();
    let Ok(()) = lookup_indices.remove_range(lookup_count..=u16::MAX);
    let Ok(value_8) = lookup_indices.is_empty();

    match ::core::primitive::bool::then_some(value_8 == console_core_integer_sets::Emptiness::Empty, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let lookup_list = LayoutLookupList::Gpos(self);
    let Ok(mut c) = LookupClosureCtx::new(glyph_set, &lookup_list);

    let Ok(lookups) = self.lookups();
    let Ok(value_23) = lookup_indices.iter();

    for idx in value_23 {
      let lookup = match lookups.get({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(idx);
        migrated_value
      }) {
        Err(ReadError::NullOffset) => {
          let Ok(()) = c.set_lookup_inactive(idx);
          continue;
        }
        Ok(value) => Ok(value),
        Err(unreadable_layout_lookup) => Err(unreadable_layout_lookup),
      }?;
      lookup.closure_lookups(&mut c, idx)?;
    }

    let Ok(visited_lookups) = c.visited_lookups();
    let Ok(()) = lookup_indices.union(visited_lookups);
    let Ok(inactive_lookups) = c.inactive_lookups();
    let Ok(()) = lookup_indices.subtract(inactive_lookups);
    Ok(())
  }
}

impl LookupClosure for PositionLookup<'_> {
  fn closure_lookups(
    &self,
    c: &mut LookupClosureCtx,
    lookup_index: u16,
  ) -> Result<(), ReadError> {
    match ::core::primitive::bool::then_some(matches!(c.visit_lookup(lookup_index), Ok(crate::tables::layout::VisitDecision::Skip)), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let Ok(glyphs) = c.glyphs();
    let intersects = self.intersects(glyphs)?;

    match ::core::primitive::bool::then_some(!intersects, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = c.set_lookup_inactive(lookup_index);
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let subtables = self.subtables()?;

    subtables.closure_lookups(c, lookup_index)
  }
}

impl LookupClosure for PositionSubtables<'_> {
  fn closure_lookups(&self, c: &mut LookupClosureCtx, arg: u16) -> Result<(), ReadError> {
    match self {
      PositionSubtables::Contextual(subtables) => subtables.closure_lookups(c, arg),
      PositionSubtables::ChainContextual(subtables) => subtables.closure_lookups(c, arg),
      PositionSubtables::Single(_)
      | PositionSubtables::Pair(_)
      | PositionSubtables::Cursive(_)
      | PositionSubtables::MarkToBase(_)
      | PositionSubtables::MarkToLig(_)
      | PositionSubtables::MarkToMark(_)
      | PositionSubtables::EmptyExtension => Ok(()),
    }
  }
}

impl Intersect for PositionLookup<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    match self {
      PositionLookup::Single(inner) => {
        let Ok(subtables) = inner.subtables();

        subtables.intersects(glyph_set)
      }
      PositionLookup::Pair(inner) => {
        let Ok(subtables) = inner.subtables();

        subtables.intersects(glyph_set)
      }
      PositionLookup::Cursive(inner) => {
        let Ok(subtables) = inner.subtables();

        subtables.intersects(glyph_set)
      }
      PositionLookup::MarkToBase(inner) => {
        let Ok(subtables) = inner.subtables();

        subtables.intersects(glyph_set)
      }
      PositionLookup::MarkToLig(inner) => {
        let Ok(subtables) = inner.subtables();

        subtables.intersects(glyph_set)
      }
      PositionLookup::MarkToMark(inner) => {
        let Ok(subtables) = inner.subtables();

        subtables.intersects(glyph_set)
      }
      PositionLookup::Contextual(inner) => {
        let Ok(subtables) = inner.subtables();

        subtables.intersects(glyph_set)
      }
      PositionLookup::ChainContextual(inner) => {
        let Ok(subtables) = inner.subtables();

        subtables.intersects(glyph_set)
      }
      PositionLookup::Extension(inner) => {
        let Ok(subtables) = inner.subtables();

        subtables.intersects(glyph_set)
      }
    }
  }
}

impl Intersect for ExtensionSubtable<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    match self {
      ExtensionSubtable::Single(inner) => inner.intersects(glyph_set),
      ExtensionSubtable::Pair(inner) => inner.intersects(glyph_set),
      ExtensionSubtable::Cursive(inner) => inner.intersects(glyph_set),
      ExtensionSubtable::MarkToBase(inner) => inner.intersects(glyph_set),
      ExtensionSubtable::MarkToLig(inner) => inner.intersects(glyph_set),
      ExtensionSubtable::MarkToMark(inner) => inner.intersects(glyph_set),
      ExtensionSubtable::Contextual(inner) => inner.intersects(glyph_set),
      ExtensionSubtable::ChainContextual(inner) => inner.intersects(glyph_set),
    }
  }
}

impl<'a, T> Intersect for ExtensionPosFormat1<'a, T>
where
  T: Intersect + FontRead<'a>,
{
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(extension_offset) = self.extension_offset();
    let Ok(value_9) = extension_offset.nullness();
    let value_9_2 = matches!(value_9, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some(value_9_2, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(false);
      }
      ::core::option::Option::None => {}
    }

    let extension = self.extension()?;

    extension.intersects(glyph_set)
  }
}

impl Intersect for SinglePos<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    match self {
      Self::Format1(item) => item.intersects(glyph_set),
      Self::Format2(item) => item.intersects(glyph_set),
    }
  }
}

impl Intersect for SinglePosFormat1<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(coverage_offset) = self.coverage_offset();
    let Ok(value_10) = coverage_offset.nullness();
    let value_10_2 = matches!(value_10, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some(value_10_2, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(false);
      }
      ::core::option::Option::None => {}
    }

    let coverage = self.coverage()?;

    Ok(coverage.intersects(glyph_set))
  }
}

impl Intersect for SinglePosFormat2<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(coverage_offset) = self.coverage_offset();
    let Ok(value_11) = coverage_offset.nullness();
    let value_11_2 = matches!(value_11, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some(value_11_2, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(false);
      }
      ::core::option::Option::None => {}
    }

    let coverage = self.coverage()?;

    Ok(coverage.intersects(glyph_set))
  }
}

impl Intersect for PairPos<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    match self {
      Self::Format1(item) => item.intersects(glyph_set),
      Self::Format2(item) => item.intersects(glyph_set),
    }
  }
}

impl Intersect for PairPosFormat1<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(coverage_offset) = self.coverage_offset();
    let Ok(value_12) = coverage_offset.nullness();
    let value_12_2 = matches!(value_12, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some(value_12_2, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(false);
      }
      ::core::option::Option::None => {}
    }

    let coverage = self.coverage()?;
    let Ok(pair_sets) = self.pair_sets();

    let Ok(num_pair_sets) = self.pair_set_count();
    let num_bits = ::core::primitive::u32::wrapping_sub(16, num_pair_sets.leading_zeros());
    let Ok(value_13) = glyph_set.len();

    match ::core::primitive::bool::then_some(({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(num_pair_sets);
      migrated_value
    }) > ::core::primitive::u64::wrapping_mul(value_13, {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(num_bits);
      migrated_value
    }), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(value_34) = glyph_set.iter();

        for g in value_34 {
          let i = match coverage.get(g) {
            Some(i) => i,
            None => {
              continue;
            }
          };
          let pair_set = match pair_sets.get({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(i);
            migrated_value
          }) {
            Err(ReadError::NullOffset) => continue,
            Ok(value) => Ok(value),
            Err(unreadable_layout_record) => Err(unreadable_layout_record),
          }?;

          let intersects = pair_set.intersects(glyph_set)?;

          match ::core::primitive::bool::then_some(intersects, ()) {
            ::core::option::Option::Some(()) => {
              return Ok(true);
            }
            ::core::option::Option::None => {}
          }
        }
      }
      ::core::option::Option::None => {
        let Ok(value_35) = pair_sets.iter_as_nullable();

        for (g, pair_set) in coverage.iter().zip(value_35) {
          let Ok(value_14) = glyph_set.contains(GlyphId::from(g));

          match ::core::primitive::bool::then_some(value_14 == console_core_integer_sets::Presence::Absent, ()) {
            ::core::option::Option::Some(()) => {
              continue;
            }
            ::core::option::Option::None => {}
          }

          let maybe_pair_set = pair_set?;
          let pair_set_3 = match maybe_pair_set {
            Some(pair_set_2) => pair_set_2,
            None => {
              continue;
            }
          };

          let intersects = pair_set_3.intersects(glyph_set)?;

          match ::core::primitive::bool::then_some(intersects, ()) {
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

impl Intersect for PairSet<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(pair_value_records) = self.pair_value_records();
    let Ok(value_36) = pair_value_records.iter();

    for record in value_36 {
      let pair_record = record?;
      let Ok(second_glyph) = pair_record.second_glyph();
      let Ok(value_15) = glyph_set.contains(GlyphId::from(second_glyph));

      match ::core::primitive::bool::then_some(value_15 == console_core_integer_sets::Presence::Present, ()) {
        ::core::option::Option::Some(()) => {
          return Ok(true);
        }
        ::core::option::Option::None => {}
      }
    }

    Ok(false)
  }
}

impl Intersect for PairPosFormat2<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(coverage_offset) = self.coverage_offset();
    let Ok(value_16) = coverage_offset.nullness();
    let value_16_2 = matches!(value_16, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some({
      value_16_2
        || {
          let Ok(class_def1_offset) = self.class_def1_offset();
          let Ok(value_28) = class_def1_offset.nullness();
          let value_28_2 = matches!(value_28, font_types::Nullness::Null);

          value_28_2
        }
        || {
          let Ok(class_def2_offset) = self.class_def2_offset();
          let Ok(value_29) = class_def2_offset.nullness();
          let value_29_2 = matches!(value_29, font_types::Nullness::Null);

          value_29_2
        }
    }, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(false);
      }
      ::core::option::Option::None => {}
    }

    let coverage = self.coverage()?;

    Ok(
      coverage.intersects(glyph_set) && {
        let class_def2 = self.class_def2()?;
        let checked_value = class_def2.intersects(glyph_set)?;
        checked_value
      },
    )
  }
}

impl Intersect for CursivePosFormat1<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(coverage_offset) = self.coverage_offset();
    let Ok(value_19) = coverage_offset.nullness();
    let value_19_2 = matches!(value_19, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some(value_19_2, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(false);
      }
      ::core::option::Option::None => {}
    }

    let coverage = self.coverage()?;

    Ok(coverage.intersects(glyph_set))
  }
}

impl Intersect for MarkBasePosFormat1<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(mark_coverage_offset) = self.mark_coverage_offset();
    let Ok(value_20) = mark_coverage_offset.nullness();
    let value_20_2 = matches!(value_20, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some({
      value_20_2 || {
        let Ok(base_coverage_offset) = self.base_coverage_offset();
        let Ok(value_30) = base_coverage_offset.nullness();
        let value_30_2 = matches!(value_30, font_types::Nullness::Null);

        value_30_2
      }
    }, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(false);
      }
      ::core::option::Option::None => {}
    }

    let mark_coverage = self.mark_coverage()?;

    Ok(mark_coverage.intersects(glyph_set) && {
      let base_coverage = self.base_coverage()?;
      base_coverage
    }.intersects(glyph_set))
  }
}

impl Intersect for MarkLigPosFormat1<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(mark_coverage_offset) = self.mark_coverage_offset();
    let Ok(value_22) = mark_coverage_offset.nullness();
    let value_22_2 = matches!(value_22, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some({
      value_22_2 || {
        let Ok(ligature_coverage_offset) = self.ligature_coverage_offset();
        let Ok(value_31) = ligature_coverage_offset.nullness();
        let value_31_2 = matches!(value_31, font_types::Nullness::Null);

        value_31_2
      }
    }, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(false);
      }
      ::core::option::Option::None => {}
    }

    let mark_coverage = self.mark_coverage()?;

    Ok(mark_coverage.intersects(glyph_set) && {
      let ligature_coverage = self.ligature_coverage()?;
      ligature_coverage
    }.intersects(glyph_set))
  }
}

impl Intersect for MarkMarkPosFormat1<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(mark1_coverage_offset) = self.mark1_coverage_offset();
    let Ok(value_24) = mark1_coverage_offset.nullness();
    let value_24_2 = matches!(value_24, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some({
      value_24_2 || {
        let Ok(mark2_coverage_offset) = self.mark2_coverage_offset();
        let Ok(value_32) = mark2_coverage_offset.nullness();
        let value_32_2 = matches!(value_32, font_types::Nullness::Null);

        value_32_2
      }
    }, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(false);
      }
      ::core::option::Option::None => {}
    }

    let mark1_coverage = self.mark1_coverage()?;

    Ok(mark1_coverage.intersects(glyph_set) && {
      let secondary_mark_coverage = self.mark2_coverage()?;
      secondary_mark_coverage
    }.intersects(glyph_set))
  }
}
