//! Computing the closure over a set of glyphs
//!
//! This means taking a set of glyphs and updating it to include any other glyphs
//! reachable from those glyphs via substitution, recursively.
use font_types::GlyphId;

use crate::{
  FontRead, ReadError, Tag,
  collections::IntSet,
  tables::layout::{ExtensionLookup, Subtables},
};

use super::{
  AlternateSubstFormat1, ChainedSequenceContext, ExtensionSubstFormat1, ExtensionSubtable, Gsub,
  Ligature, LigatureSet, LigatureSubstFormat1, MultipleSubstFormat1,
  ReverseChainSingleSubstFormat1, SequenceContext, SingleSubst, SingleSubstFormat1,
  SingleSubstFormat2, SubstitutionLookup, SubstitutionLookupList, SubstitutionSubtables,
};

#[cfg(feature = "std")]
use crate::tables::layout::{TraversalBudget, VisitDecision};

#[cfg(feature = "std")]
use crate::tables::layout::{
  ContextFormat1, ContextFormat2, ContextFormat3, Intersect, LayoutLookupList, LookupClosure,
  LookupClosureCtx,
};

mod ctx {
  use console_core_never::Never;
  use std::collections::HashMap;
  use types::GlyphId;

  use crate::{
    collections::IntSet,
    tables::gsub::{SubstitutionLookup, SubstitutionLookupList},
  };

  use super::GlyphClosure as _;
  use super::{ReadError, SubstitutionCardinality, TraversalBudget, VisitDecision};

  #[cfg(feature = "std")]
  use crate::tables::layout::{MAX_LOOKUP_VISIT_COUNT, MAX_NESTING_LEVEL};

  pub(super) struct ClosureCtx<'a> {
    glyphs: &'a mut IntSet<GlyphId>,
    active_glyphs_stack: Vec<IntSet<GlyphId>>,
    output: IntSet<GlyphId>,
    lookup_count: u16,
    nesting_level_left: u8,
    done_lookups_glyphs: HashMap<u16, (u64, IntSet<GlyphId>)>,
  }

  impl<'a> ClosureCtx<'a> {
    pub(super) fn new(glyphs: &'a mut IntSet<GlyphId>) -> core::result::Result<Self, console_core_never::Never> {
        let Ok(value_1) = IntSet::empty();

        Ok(Self {
          glyphs,
          active_glyphs_stack: Vec::new(),
          output: value_1,
          lookup_count: 0,
          nesting_level_left: MAX_NESTING_LEVEL,
          done_lookups_glyphs: Default::default(),
        })
    }

    #[must_use]
    pub(super) fn lookup_budget(&self) -> Result<TraversalBudget, Never> {
      Ok(match self.lookup_count.cmp(&MAX_LOOKUP_VISIT_COUNT) {
        core::cmp::Ordering::Greater => TraversalBudget::Exhausted,
        core::cmp::Ordering::Less | core::cmp::Ordering::Equal => TraversalBudget::Available,
      })
    }

    #[must_use]
    pub(super) fn parent_active_glyphs(&self) -> Result<&IntSet<GlyphId>, Never> {
      Ok(match self.active_glyphs_stack.last() {
        Some(glyphs) => glyphs,
        None => self.glyphs,
      })
    }

    pub(super) fn push_cur_active_glyphs(&mut self, glyphs: IntSet<GlyphId>) -> core::result::Result<(), console_core_never::Never> { {
      self.active_glyphs_stack.push(glyphs)
    }; Ok(()) }

    pub(super) fn pop_cur_done_glyphs(&mut self) -> core::result::Result<(), console_core_never::Never> { {
      self.active_glyphs_stack.pop();
    }; Ok(()) }

    pub(super) fn recurse(
      &mut self,
      lookup_list: &SubstitutionLookupList,
      lookup: &SubstitutionLookup,
      lookup_index: u16,
      glyphs: IntSet<GlyphId>,
      seen_seq_indices: &mut IntSet<u16>,
      sequences: core::ops::RangeInclusive<u16>,
    ) -> Result<(), ReadError> {
      match ::core::primitive::bool::then_some(self.nesting_level_left == 0, ()) {
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

      let Ok(()) = self.push_cur_active_glyphs(glyphs);

      match ::core::primitive::bool::then_some(matches!(self.visit_lookup(lookup_index), Ok(VisitDecision::Skip)), ()) {
        ::core::option::Option::Some(()) => {
          match (1, &mut self.nesting_level_left) {
            (explicit_arithmetic_amount, explicit_arithmetic_place) => {
              *explicit_arithmetic_place = ::core::primitive::u8::wrapping_add(
                *explicit_arithmetic_place,
                explicit_arithmetic_amount,
              )
            }
          };

          let Ok(()) = self.pop_cur_done_glyphs();
          return Ok(());
        }
        ::core::option::Option::None => {}
      }

      let cardinality = lookup.substitution_cardinality()?;

      match cardinality {
        SubstitutionCardinality::Variable => {
          let Ok(()) = seen_seq_indices.insert_range(sequences);
        }
        SubstitutionCardinality::OneToOne => {}
      }

      let subtables = lookup.subtables()?;
      subtables.closure_glyphs(self, lookup_list, lookup_index)?;

      match (1, &mut self.nesting_level_left) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::u8::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };

      let Ok(()) = self.pop_cur_done_glyphs();

      Ok(())
    }

    pub(super) fn reset_lookup_visit_count(&mut self) -> core::result::Result<(), console_core_never::Never> { {
      self.lookup_count = 0;
    }; Ok(()) }

    pub(super) fn visit_lookup(&mut self, lookup_index: u16) -> Result<VisitDecision, Never> {
      match self.lookup_budget() {
        Ok(TraversalBudget::Exhausted) => return Ok(VisitDecision::Skip),
        Ok(TraversalBudget::Available) => {}
      }

      self.lookup_count = self.lookup_count.wrapping_add(1);
      self.record_lookup_state(lookup_index)
    }

    fn record_lookup_state(&mut self, lookup_index: u16) -> Result<VisitDecision, Never> {
      let Ok(mut cur_active_glyphs) = IntSet::empty();
      let Ok(parent_glyphs) = self.parent_active_glyphs();

      let Ok(()) = cur_active_glyphs.union(parent_glyphs);

      let Ok(value_2) = IntSet::empty();

      let (count, covered) = self
        .done_lookups_glyphs
        .entry(lookup_index)
        .or_insert((0, value_2));

      let Ok(value_3) = self.glyphs.len();

      match ::core::primitive::bool::then_some(*count != value_3, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(value_4) = self.glyphs.len();

          *count = value_4;
          let ::core::result::Result::Ok(()) = covered.clear();
        }
        ::core::option::Option::None => {}
      }

      let Ok(mut value_5) = cur_active_glyphs.iter();

      match ::core::primitive::bool::then_some({
        value_5.all(|g| {
          let Ok(value_58) = covered.contains(g);

          value_58 == console_core_integer_sets::Presence::Present
        })
      }, ()) {
        ::core::option::Option::Some(()) => {
          return Ok(VisitDecision::Skip);
        }
        ::core::option::Option::None => {}
      }

      let value_7 = &cur_active_glyphs;

      let ::core::result::Result::Ok(()) = covered.union(value_7);
      Ok(VisitDecision::Visit)
    }

    #[must_use]
    pub(super) fn glyphs(&self) -> core::result::Result<&IntSet<GlyphId>, console_core_never::Never> {
        Ok(self.glyphs)
    }

    pub(super) fn add(&mut self, gid: GlyphId) -> core::result::Result<(), console_core_never::Never> { {
      let Ok(_presence_changed) = self.output.insert(gid);
    }; Ok(()) }

    pub(super) fn add_glyphs(&mut self, iter: impl IntoIterator<Item = GlyphId>) -> core::result::Result<(), console_core_never::Never> { {
      self.output.extend(iter)
    }; Ok(()) }

    pub(super) fn flush(&mut self) -> core::result::Result<(), console_core_never::Never> { {
      let ::core::result::Result::Ok(()) = self.glyphs.union(&self.output);
      let ::core::result::Result::Ok(()) = self.output.clear();
      self.active_glyphs_stack.clear();
    }; Ok(()) }
  }
}

use ctx::ClosureCtx;

#[cfg(feature = "std")]
enum SubstitutionCardinality {
  OneToOne,
  Variable,
}

trait GlyphClosure {
  fn closure_glyphs(
    &self,
    ctx: &mut ClosureCtx,
    lookup_list: &SubstitutionLookupList,
    lookup_index: u16,
  ) -> Result<(), ReadError>;

  fn substitution_cardinality(&self) -> Result<SubstitutionCardinality, ReadError> {
    Ok(SubstitutionCardinality::OneToOne)
  }
}

const CLOSURE_MAX_STAGES: u8 = 12;

fn apply_closure_stage(
  ctx: &mut ClosureCtx,
  lookup_list: &SubstitutionLookupList,
  lookups: &IntSet<u16>,
) -> Result<(), ReadError> {
  use console_core_integer_sets::{Membership, Presence};

  let Ok(offsets) = lookup_list.lookups();

  match lookups.membership() {
    Ok(Membership::Exclusive) => {
      let Ok(count) = lookup_list.lookup_count();

      for position in (0..count).filter(|&position| matches!(lookups.contains(position), Ok(Presence::Present))) {
        let lookup = match offsets.get(u64::from(position)) {
          Ok(lookup) => lookup,
          Err(ReadError::NullOffset) => continue,
          Err(error) => return Err(error),
        };
        lookup.closure_glyphs(ctx, lookup_list, position)?;
        let Ok(()) = ctx.flush();
      }
    }
    Ok(Membership::Inclusive) => {
      let Ok(indices) = lookups.iter();

      for position in indices {
        let lookup = match offsets.get(u64::from(position)) {
          Ok(lookup) => lookup,
          Err(ReadError::NullOffset | ReadError::InvalidCollectionIndex(_)) => continue,
          Err(error) => return Err(error),
        };
        lookup.closure_glyphs(ctx, lookup_list, position)?;
        let Ok(()) = ctx.flush();
      }
    }
  }

  Ok(())
}

impl Gsub<'_> {
  pub fn closure_glyphs(
    &self,
    lookups: &IntSet<u16>,
    glyphs: &mut IntSet<GlyphId>,
  ) -> Result<(), ReadError> {
    let Ok(lookup_list_offset) = self.lookup_list_offset();
    let Ok(value_8) = lookup_list_offset.nullness();
    let value_8_2 = matches!(value_8, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some(value_8_2, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let lookup_list = self.lookup_list()?;
    let Ok(mut ctx) = ClosureCtx::new(glyphs);
    let total_stages = u16::from(CLOSURE_MAX_STAGES).saturating_add(2);

    for _ in 0..total_stages {
      let Ok(()) = ctx.reset_lookup_visit_count();
      let Ok(ctx_glyphs) = ctx.glyphs();
      let Ok(glyphs_length) = ctx_glyphs.len();
      apply_closure_stage(&mut ctx, &lookup_list, lookups)?;

      let Ok(glyphs_after_stage) = ctx.glyphs();
      let Ok(glyph_count) = glyphs_after_stage.len();

      match ::core::primitive::bool::then_some(glyphs_length == glyph_count, ()) {
        ::core::option::Option::Some(()) => {
          break;
        }
        ::core::option::Option::None => {}
      }

    }

    Ok(())
  }

  pub fn collect_lookups(&self, feature_indices: &IntSet<u16>) -> Result<IntSet<u16>, ReadError> {
    let Ok(feature_list_offset) = self.feature_list_offset();
    let Ok(value_11) = feature_list_offset.nullness();
    let value_11_2 = matches!(value_11, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some(value_11_2, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(value_12) = IntSet::empty();

        return Ok(value_12);
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

  pub fn collect_features(
    &self,
    scripts: &IntSet<Tag>,
    languages: &IntSet<Tag>,
    features: &IntSet<Tag>,
  ) -> Result<IntSet<u16>, ReadError> {
    let Ok(script_list_offset) = self.script_list_offset();
    let Ok(value_13) = script_list_offset.nullness();
    let value_13_2 = matches!(value_13, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some({
      value_13_2 || {
        let Ok(feature_list_offset) = self.feature_list_offset();
        let Ok(value_59) = feature_list_offset.nullness();
        let value_59_2 = matches!(value_59, font_types::Nullness::Null);

        value_59_2
      }
    }, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(value_15) = IntSet::empty();

        return Ok(value_15);
      }
      ::core::option::Option::None => {}
    }

    let feature_list = self.feature_list()?;
    let script_list = self.script_list()?;
    let Ok(value_16) = self.offset_data();

    let Ok(head_ptr) = value_16.base_address();
    script_list.collect_features(head_ptr, &feature_list, scripts, languages, features)
  }

  pub fn closure_lookups(
    &self,
    glyphs: &IntSet<GlyphId>,
    lookup_indices: &mut IntSet<u16>,
  ) -> Result<(), ReadError> {
    let Ok(lookup_list_offset) = self.lookup_list_offset();
    let Ok(value_17) = lookup_list_offset.nullness();
    let value_17_2 = matches!(value_17, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some(value_17_2, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let lookup_list = self.lookup_list()?;
    lookup_list.closure_lookups(glyphs, lookup_indices)
  }
}

impl GlyphClosure for SubstitutionLookup<'_> {
  fn closure_glyphs(
    &self,
    ctx: &mut ClosureCtx,
    lookup_list: &SubstitutionLookupList,
    lookup_index: u16,
  ) -> Result<(), ReadError> {
    match ::core::primitive::bool::then_some(matches!(ctx.visit_lookup(lookup_index), Ok(VisitDecision::Skip)), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let subtables = self.subtables()?;

    subtables.closure_glyphs(ctx, lookup_list, lookup_index)
  }

  fn substitution_cardinality(&self) -> Result<SubstitutionCardinality, ReadError> {
    let subtables = self.subtables()?;

    subtables.substitution_cardinality()
  }
}

impl GlyphClosure for SubstitutionSubtables<'_> {
  fn closure_glyphs(
    &self,
    ctx: &mut ClosureCtx,
    lookup_list: &SubstitutionLookupList,
    lookup_index: u16,
  ) -> Result<(), ReadError> {
    match self {
      SubstitutionSubtables::Single(tables) => {
        tables.closure_glyphs(ctx, lookup_list, lookup_index)
      }
      SubstitutionSubtables::Multiple(tables) => {
        tables.closure_glyphs(ctx, lookup_list, lookup_index)
      }
      SubstitutionSubtables::Alternate(tables) => {
        tables.closure_glyphs(ctx, lookup_list, lookup_index)
      }
      SubstitutionSubtables::Ligature(tables) => {
        tables.closure_glyphs(ctx, lookup_list, lookup_index)
      }
      SubstitutionSubtables::Reverse(tables) => {
        tables.closure_glyphs(ctx, lookup_list, lookup_index)
      }
      SubstitutionSubtables::Contextual(tables) => {
        tables.closure_glyphs(ctx, lookup_list, lookup_index)
      }
      SubstitutionSubtables::ChainContextual(tables) => {
        tables.closure_glyphs(ctx, lookup_list, lookup_index)
      }
      SubstitutionSubtables::EmptyExtension => Ok(()),
    }
  }

  fn substitution_cardinality(&self) -> Result<SubstitutionCardinality, ReadError> {
    match self {
      SubstitutionSubtables::Single(_) => Ok(SubstitutionCardinality::OneToOne),
      SubstitutionSubtables::Multiple(_) => Ok(SubstitutionCardinality::Variable),
      SubstitutionSubtables::Alternate(_) => Ok(SubstitutionCardinality::OneToOne),
      SubstitutionSubtables::Ligature(_) => Ok(SubstitutionCardinality::Variable),
      SubstitutionSubtables::Reverse(_) => Ok(SubstitutionCardinality::OneToOne),
      SubstitutionSubtables::Contextual(_) => Ok(SubstitutionCardinality::Variable),
      SubstitutionSubtables::ChainContextual(_) => Ok(SubstitutionCardinality::Variable),
      SubstitutionSubtables::EmptyExtension => Ok(SubstitutionCardinality::OneToOne),
    }
  }
}

impl<'a, T: FontRead<'a> + GlyphClosure + 'a, Ext: ExtensionLookup<'a, T> + 'a> GlyphClosure
  for Subtables<'a, T, Ext>
{
  fn closure_glyphs(
    &self,
    ctx: &mut ClosureCtx,
    lookup_list: &SubstitutionLookupList,
    lookup_index: u16,
  ) -> Result<(), ReadError> {
    for t in self.iter().filter_map(|table| match table {
      Err(ReadError::NullOffset) => None,
      Ok(value) => Some(Ok(value)),
      Err(unreadable_layout_record) => Some(Err(unreadable_layout_record)),
    }) {
      let subtable = t?;
      subtable.closure_glyphs(ctx, lookup_list, lookup_index)?;
    }

    Ok(())
  }
}

impl GlyphClosure for SingleSubst<'_> {
  fn closure_glyphs(
    &self,
    ctx: &mut ClosureCtx,
    lookup_list: &SubstitutionLookupList,
    lookup_index: u16,
  ) -> Result<(), ReadError> {
    match self {
      SingleSubst::Format1(t) => t.closure_glyphs(ctx, lookup_list, lookup_index),
      SingleSubst::Format2(t) => t.closure_glyphs(ctx, lookup_list, lookup_index),
    }
  }
}

impl GlyphClosure for SingleSubstFormat1<'_> {
  fn closure_glyphs(
    &self,
    ctx: &mut ClosureCtx,
    _lookup_list: &SubstitutionLookupList,
    _lookup_index: u16,
  ) -> Result<(), ReadError> {
    let Ok(offset) = self.coverage_offset();

    match offset.nullness() {
      Ok(font_types::Nullness::Null) => return Ok(()),
      Ok(font_types::Nullness::NonNull) => {}
    }

    let coverage = self.coverage()?;
    let mask = u16::MAX;

    match (coverage.population() >= u64::from(mask)).then_some(()) {
      Some(()) => return Ok(()),
      None => {}
    }

    let Ok(parent_glyphs) = ctx.parent_active_glyphs();
    let intersection = coverage.intersect_set(parent_glyphs);
    let Ok(first) = intersection.first();
    let Ok(last) = intersection.last();
    let (first_glyph, last_glyph) = match first.zip(last) {
      Some(bounds) => bounds,
      None => return Ok(()),
    };

    let Ok(glyph_delta) = self.delta_glyph_id();
    let delta = i32::from(glyph_delta);
    let mask_2 = i32::from(mask);
    let Ok(first_number) = first_glyph.to_u32();
    let Ok(last_number) = last_glyph.to_u32();
    let Ok(min_before) = console_core_number_conversion::wrapped::<_, i32>(first_number);
    let Ok(max_before) = console_core_number_conversion::wrapped::<_, i32>(last_number);
    let min_after = min_before.wrapping_add(delta) & mask_2;
    let max_after = max_before.wrapping_add(delta) & mask_2;
    let Ok(range_count) = console_core_number_conversion::wrapped::<_, u64>(
      max_before.wrapping_sub(min_before).wrapping_add(1),
    );
    let Ok(input_count) = intersection.len();
    let bounds = min_before..=max_before;
    let endpoint_covered = bounds.contains(&min_after) || bounds.contains(&max_after);

    match (input_count == range_count && endpoint_covered).then_some(()) {
      Some(()) => return Ok(()),
      None => {}
    }

    let Ok(selected) = intersection.iter();

    for glyph in selected {
      let Ok(number) = glyph.to_u32();
      let Ok(signed) = console_core_number_conversion::wrapped::<_, i32>(number);
      let mapped = signed.wrapping_add(delta) & mask_2;
      let Ok(output) = console_core_number_conversion::wrapped::<_, u32>(mapped);
      let Ok(()) = ctx.add(GlyphId::from(output));
    }

    Ok(())
  }
}

impl GlyphClosure for SingleSubstFormat2<'_> {
  fn closure_glyphs(
    &self,
    ctx: &mut ClosureCtx,
    _lookup_list: &SubstitutionLookupList,
    _lookup_index: u16,
  ) -> Result<(), ReadError> {
    let Ok(coverage_offset) = self.coverage_offset();
    let Ok(value_20) = coverage_offset.nullness();
    let value_20_2 = matches!(value_20, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some(value_20_2
      || ({
        let Ok(explicit_value) = self.glyph_count();
        explicit_value
      }) == 0, ())
    {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let coverage = self.coverage()?;
    let Ok(glyph_set) = ctx.parent_active_glyphs();
    let Ok(subs_glyphs) = self.substitute_glyph_ids();

    let Ok(value_21) = glyph_set.len();

    let new_glyphs: Vec<GlyphId> = match ::core::primitive::bool::then_some(({
      let Ok(glyph_count) = self.glyph_count();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(glyph_count);
      migrated_value
    }) > ::core::primitive::u64::wrapping_mul(value_21, {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(coverage.cost());
      migrated_value
    }), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(value_22) = glyph_set.iter();

        value_22
          .filter_map(|g| coverage.get(g))
          .filter_map(|idx| {
            let Ok(value_56) = console_core_number_conversion::index({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(idx);
              migrated_value
            });

            subs_glyphs.get(value_56).map(|new_g| {
              let Ok(value_78) = new_g.get();

              GlyphId::from(value_78)
            })
          })
          .collect()
      }
      ::core::option::Option::None => coverage
        .iter()
        .zip(subs_glyphs)
        .filter(|&(g, _)| {
          let Ok(value_61) = glyph_set.contains(GlyphId::from(g));

          value_61 == console_core_integer_sets::Presence::Present
        })
        .map(|(_, &new_g)| {
          let Ok(value_80) = new_g.get();

          GlyphId::from(value_80)
        })
        .collect(),
    };
    let Ok(()) = ctx.add_glyphs(new_glyphs);
    Ok(())
  }
}

impl GlyphClosure for MultipleSubstFormat1<'_> {
  fn closure_glyphs(
    &self,
    ctx: &mut ClosureCtx,
    _lookup_list: &SubstitutionLookupList,
    _lookup_index: u16,
  ) -> Result<(), ReadError> {
    let Ok(coverage_offset) = self.coverage_offset();
    let Ok(value_24) = coverage_offset.nullness();
    let value_24_2 = matches!(value_24, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some(value_24_2
      || ({
        let Ok(explicit_value) = self.sequence_count();
        explicit_value
      }) == 0, ())
    {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let coverage = self.coverage()?;
    let Ok(glyph_set) = ctx.parent_active_glyphs();
    let Ok(sequences) = self.sequences();

    let Ok(value_25) = glyph_set.len();

    let new_glyphs: Vec<GlyphId> = match ::core::primitive::bool::then_some(({
      let Ok(sequence_count) = self.sequence_count();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(sequence_count);
      migrated_value
    }) > ::core::primitive::u64::wrapping_mul(value_25, {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(coverage.cost());
      migrated_value
    }), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(value_26) = glyph_set.iter();

        value_26
          .filter_map(|g| coverage.get(g))
          .filter_map(|idx| {
            match sequences.get({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(idx);
              migrated_value
            }) {
              ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
              ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
            }
          })
          .flat_map(|seq| {
            let Ok(substitute_glyph_ids) = seq.substitute_glyph_ids();

            substitute_glyph_ids.iter().map(|new_g| {
              let Ok(value_81) = new_g.get();

              GlyphId::from(value_81)
            })
          })
          .collect()
      }
      ::core::option::Option::None => {
        let Ok(value_82) = sequences.iter_as_nullable();

        coverage
          .iter()
          .zip(value_82)
          .filter_map(|(g, seq)| {
            let Ok(value_83) = glyph_set.contains(GlyphId::from(g));

            (value_83 == console_core_integer_sets::Presence::Present)
              .then(|| {
                match seq {
                  ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
                  ::core::result::Result::Err(_unreadable_font_data) => {
                    ::core::option::Option::None
                  }
                }
                .flatten()
              })
              .flatten()
          })
          .flat_map(|seq| {
            let Ok(substitute_glyph_ids) = seq.substitute_glyph_ids();

            substitute_glyph_ids.iter().map(|new_g| {
              let Ok(value_84) = new_g.get();

              GlyphId::from(value_84)
            })
          })
          .collect()
      }
    };

    let Ok(()) = ctx.add_glyphs(new_glyphs);
    Ok(())
  }
}

impl GlyphClosure for AlternateSubstFormat1<'_> {
  fn closure_glyphs(
    &self,
    ctx: &mut ClosureCtx,
    _lookup_list: &SubstitutionLookupList,
    _lookup_index: u16,
  ) -> Result<(), ReadError> {
    let Ok(coverage_offset) = self.coverage_offset();
    let Ok(value_27) = coverage_offset.nullness();
    let value_27_2 = matches!(value_27, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some(value_27_2
      || ({
        let Ok(explicit_value) = self.alternate_set_count();
        explicit_value
      }) == 0, ())
    {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let coverage = self.coverage()?;
    let Ok(glyph_set) = ctx.parent_active_glyphs();
    let Ok(alts) = self.alternate_sets();

    let Ok(value_28) = glyph_set.len();

    let new_glyphs: Vec<GlyphId> = match ::core::primitive::bool::then_some(({
      let Ok(alternate_set_count) = self.alternate_set_count();
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u64>(alternate_set_count);
      migrated_value
    }) > ::core::primitive::u64::wrapping_mul(value_28, {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(coverage.cost());
      migrated_value
    }), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(value_29) = glyph_set.iter();

        value_29
          .filter_map(|g| coverage.get(g))
          .filter_map(|idx| {
            match alts.get({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(idx);
              migrated_value
            }) {
              ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
              ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
            }
          })
          .flat_map(|alt_set| {
            let Ok(alternate_glyph_ids) = alt_set.alternate_glyph_ids();

            alternate_glyph_ids.iter().map(|new_g| {
              let Ok(value_85) = new_g.get();

              GlyphId::from(value_85)
            })
          })
          .collect()
      }
      ::core::option::Option::None => {
        let Ok(value_86) = alts.iter_as_nullable();

        coverage
          .iter()
          .zip(value_86)
          .filter_map(|(g, alt_set)| {
            let Ok(value_87) = glyph_set.contains(GlyphId::from(g));

            (value_87 == console_core_integer_sets::Presence::Present)
              .then(|| {
                match alt_set {
                  ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
                  ::core::result::Result::Err(_unreadable_font_data) => {
                    ::core::option::Option::None
                  }
                }
                .flatten()
              })
              .flatten()
          })
          .flat_map(|alt_set| {
            let Ok(alternate_glyph_ids) = alt_set.alternate_glyph_ids();

            alternate_glyph_ids.iter().map(|new_g| {
              let Ok(value_88) = new_g.get();

              GlyphId::from(value_88)
            })
          })
          .collect()
      }
    };

    let Ok(()) = ctx.add_glyphs(new_glyphs);
    Ok(())
  }
}

impl GlyphClosure for LigatureSubstFormat1<'_> {
  fn closure_glyphs(
    &self,
    ctx: &mut ClosureCtx,
    _lookup_list: &SubstitutionLookupList,
    _lookup_index: u16,
  ) -> Result<(), ReadError> {
    let Ok(coverage_offset) = self.coverage_offset();
    let Ok(value_30) = coverage_offset.nullness();
    let value_30_2 = matches!(value_30, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some(value_30_2
      || ({
        let Ok(explicit_value) = self.ligature_set_count();
        explicit_value
      }) == 0, ())
    {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let coverage = self.coverage()?;
    let Ok(ligs) = self.ligature_sets();
    let Ok(parent_glyphs) = ctx.parent_active_glyphs();
    let Ok(value_31) = parent_glyphs.len();

    let lig_set_idxes: Vec<u64> = match ::core::primitive::bool::then_some(({
      let Ok(ligature_set_count) = self.ligature_set_count();
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u64>(ligature_set_count);
      migrated_value
    }) > value_31, ())
    {
      ::core::option::Option::Some(()) => {
        let Ok(value_32) = parent_glyphs.iter();

        value_32
          .filter_map(|g| coverage.get(g))
          .map(|idx| {
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(idx);
            migrated_value
          })
          .collect()
      }
      ::core::option::Option::None => coverage
        .iter()
        .enumerate()
        .filter(|&(_idx, g)| {
          let Ok(value_62) = parent_glyphs.contains(GlyphId::from(g));

          value_62 == console_core_integer_sets::Presence::Present
        })
        .map(|(idx, _)| {
          let Ok(index) = console_core_number_conversion::fitted::<_, u64>(idx);
          index
        })
        .collect(),
    };

    for idx in lig_set_idxes {
      let lig_set = match ligs.get(idx) {
        Err(ReadError::NullOffset) => continue,
        Ok(value) => Ok(value),
        Err(unreadable_ligature_set) => Err(unreadable_ligature_set),
      }?;
      let Ok(ligatures) = lig_set.ligatures();
      let Ok(value_90) = ligatures.iter_as_nullable();

      'over_lig_2: for lig in value_90 {
        let ligature = lig?;
        let lig_3 = match ligature {
          Some(lig_2) => lig_2,
          None => {
            continue 'over_lig_2;
          }
        };

        let Ok(glyphs) = ctx.glyphs();
        let intersects = lig_3.intersects(glyphs)?;

        match ::core::primitive::bool::then_some(intersects, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(ligature_glyph) = lig_3.ligature_glyph();
            let Ok(()) = ctx.add(GlyphId::from(ligature_glyph));
          }
          ::core::option::Option::None => {}
        }
      }
    }

    Ok(())
  }
}

impl GlyphClosure for ReverseChainSingleSubstFormat1<'_> {
  fn closure_glyphs(
    &self,
    ctx: &mut ClosureCtx,
    _lookup_list: &SubstitutionLookupList,
    _lookup_index: u16,
  ) -> Result<(), ReadError> {
    let Ok(glyphs) = ctx.glyphs();
    let intersects = self.intersects(glyphs)?;

    match ::core::primitive::bool::then_some(!intersects, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let coverage = self.coverage()?;
    let Ok(glyph_set) = ctx.parent_active_glyphs();
    let Ok(value_34) = glyph_set.len();

    let idxes: Vec<u64> = match ::core::primitive::bool::then_some(({
      let Ok(glyph_count) = self.glyph_count();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(glyph_count);
      migrated_value
    }) > value_34, ())
    {
      ::core::option::Option::Some(()) => {
        let Ok(value_35) = glyph_set.iter();

        value_35
          .filter_map(|g| coverage.get(g))
          .map(|idx| {
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(idx);
            migrated_value
          })
          .collect()
      }
      ::core::option::Option::None => coverage
        .iter()
        .enumerate()
        .filter(|&(_idx, g)| {
          let Ok(value_63) = glyph_set.contains(GlyphId::from(g));

          value_63 == console_core_integer_sets::Presence::Present
        })
        .map(|(idx, _)| {
          let Ok(index) = console_core_number_conversion::fitted::<_, u64>(idx);
          index
        })
        .collect(),
    };

    let Ok(sub_glyphs) = self.substitute_glyph_ids();

    for i in idxes {
      let Ok(value_70) = console_core_number_conversion::index(i);

      let g = match sub_glyphs.get(value_70) {
        Some(g) => g,
        None => {
          continue;
        }
      };
      let Ok(value_92) = g.get();

      let Ok(()) = ctx.add(GlyphId::from(value_92));
    }

    Ok(())
  }
}

impl GlyphClosure for SequenceContext<'_> {
  fn closure_glyphs(
    &self,
    ctx: &mut ClosureCtx,
    lookup_list: &SubstitutionLookupList,
    lookup_index: u16,
  ) -> Result<(), ReadError> {
    match self {
      Self::Format1(table) => {
        ContextFormat1::Plain(table.clone()).closure_glyphs(ctx, lookup_list, lookup_index)
      }
      Self::Format2(table) => {
        ContextFormat2::Plain(table.clone()).closure_glyphs(ctx, lookup_list, lookup_index)
      }
      Self::Format3(table) => {
        ContextFormat3::Plain(table.clone()).closure_glyphs(ctx, lookup_list, lookup_index)
      }
    }
  }
}

impl GlyphClosure for ChainedSequenceContext<'_> {
  fn closure_glyphs(
    &self,
    ctx: &mut ClosureCtx,
    lookup_list: &SubstitutionLookupList,
    lookup_index: u16,
  ) -> Result<(), ReadError> {
    match self {
      Self::Format1(table) => {
        ContextFormat1::Chain(table.clone()).closure_glyphs(ctx, lookup_list, lookup_index)
      }
      Self::Format2(table) => {
        ContextFormat2::Chain(table.clone()).closure_glyphs(ctx, lookup_list, lookup_index)
      }
      Self::Format3(table) => {
        ContextFormat3::Chain(table.clone()).closure_glyphs(ctx, lookup_list, lookup_index)
      }
    }
  }
}

impl GlyphClosure for ContextFormat1<'_> {
  fn closure_glyphs(
    &self,
    ctx: &mut ClosureCtx,
    lookup_list: &SubstitutionLookupList,
    _lookup_index: u16,
  ) -> Result<(), ReadError> {
    let own_coverage = self.coverage()?;
    let coverage = match own_coverage {
      Some(coverage) => coverage,
      None => {
        return Ok(());
      }
    };
    let Ok(parent_glyphs) = ctx.parent_active_glyphs();
    let cov_active_glyphs = coverage.intersect_set(parent_glyphs);
    let Ok(value_37) = cov_active_glyphs.is_empty();

    match ::core::primitive::bool::then_some(value_37 == console_core_integer_sets::Emptiness::Empty, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let Ok(lookups) = lookup_list.lookups();

    for (gid, rule_set) in coverage
      .iter()
      .zip({ let Ok(explicit_value) = self.rule_sets(); explicit_value })
      .filter_map(|(g, rule_set)| {
        rule_set
          .transpose()
          .filter(|_| {
            let Ok(value_64) = cov_active_glyphs.contains(GlyphId::from(g));

            value_64 == console_core_integer_sets::Presence::Present
          })
          .map(|rs| (g, rs))
      })
    {
      match ctx.lookup_budget() {
        Ok(TraversalBudget::Exhausted) => return Ok(()),
        Ok(TraversalBudget::Available) => {}
      }

      let rule_set_2 = rule_set?;
      let Ok(rules) = rule_set_2.rules();

      'over_rule_2: for rule in rules {
        match ctx.lookup_budget() {
          Ok(TraversalBudget::Exhausted) => return Ok(()),
          Ok(TraversalBudget::Available) => {}
        }

        let substitution_rule = rule?;
        let rule_3 = match substitution_rule {
          Some(rule_2) => rule_2,
          None => {
            continue 'over_rule_2;
          }
        };

        let Ok(glyphs) = ctx.glyphs();
        let intersects = rule_3.intersects(glyphs)?;

        match ::core::primitive::bool::then_some(!intersects, ()) {
          ::core::option::Option::Some(()) => {
            continue 'over_rule_2;
          }
          ::core::option::Option::None => {}
        }

        let Ok(input_seq) = rule_3.input_sequence();
        let Ok(input_count) = console_core_number_conversion::fitted::<_, u64>(input_seq.len().wrapping_add(1));

        let Ok(mut seen_sequence_indices) = IntSet::new();

        let Ok(lookup_records) = rule_3.lookup_records();

        'over_lookup_record_3: for lookup_record in lookup_records {
          let Ok(lookup_index) = lookup_record.lookup_list_index();
          let lookup = match lookups.get(u64::from(lookup_index)) {
            Ok(lookup) => lookup,
            Err(ReadError::NullOffset | ReadError::InvalidCollectionIndex(_)) => continue 'over_lookup_record_3,
            Err(error) => return Err(error),
          };
          let Ok(sequence_idx) = lookup_record.sequence_index();

          match (u64::from(sequence_idx) >= input_count).then_some(()) {
            Some(()) => continue 'over_lookup_record_3,
            None => {}
          }

          let Ok(mut active_glyphs) = IntSet::empty();
          let Ok(prior) = seen_sequence_indices.insert(sequence_idx);

          match (prior, sequence_idx) {
            (crate::Presence::Present, _) => {
              let Ok(current) = ctx.glyphs();
              let Ok(glyphs_2) = current.iter();
              active_glyphs.extend(glyphs_2);
            }
            (crate::Presence::Absent, 0) => {
              let Ok(_previous_presence) = active_glyphs.insert(GlyphId::from(gid));
            }
            (crate::Presence::Absent, position) => {
              let Ok(index) = console_core_number_conversion::index(position.wrapping_sub(1));
              let entry = input_seq.get(index).ok_or(ReadError::OutOfBounds)?;
              let Ok(glyph) = entry.get();
              let Ok(_previous_presence) = active_glyphs.insert(GlyphId::from(glyph));
            }
          }

          let Ok(sequence_end) = console_core_number_conversion::wrapped::<_, u16>(input_count);
          ctx.recurse(
            lookup_list,
            &lookup,
            lookup_index,
            active_glyphs,
            &mut seen_sequence_indices,
            sequence_idx..=sequence_end,
          )?;
        }
      }
    }

    Ok(())
  }
}

struct ContextClasses<'a> {
  input_class_def: crate::tables::layout::ClassDef<'a>,
  coverage: IntSet<u16>,
  input: IntSet<u16>,
  backtrack: IntSet<u16>,
  lookahead: IntSet<u16>,
}

fn selected_context_classes<'a>(
  contextual: &'a ContextFormat2<'_>,
  ctx: &ClosureCtx,
) -> Result<Option<ContextClasses<'a>>, ReadError> {
  use console_core_integer_sets::Emptiness;
  use font_types::Nullness;

  let coverage_table = contextual.coverage()?;
  let coverage = match coverage_table {
    Some(coverage) => coverage,
    None => return Ok(None),
  };
  let Ok(parent) = ctx.parent_active_glyphs();
  let active = coverage.intersect_set(parent);

  match active.is_empty() {
    Ok(Emptiness::Empty) => return Ok(None),
    Ok(Emptiness::NonEmpty) => {}
  }

  let class_table = contextual.input_class_def()?;
  let input_class_def = match class_table {
    Some(class_def) => class_def,
    None => return Ok(None),
  };
  let coverage_classes = input_class_def.intersect_classes(&active);

  match coverage_classes.is_empty() {
    Ok(Emptiness::Empty) => return Ok(None),
    Ok(Emptiness::NonEmpty) => {}
  }

  let Ok(glyphs) = ctx.glyphs();
  let input = input_class_def.intersect_classes(glyphs);
  let Ok(empty_backtrack) = IntSet::empty();
  let Ok(empty_lookahead) = IntSet::empty();
  let (backtrack, lookahead) = match contextual {
    ContextFormat2::Plain(_) => (empty_backtrack, empty_lookahead),
    ContextFormat2::Chain(table) => {
      let Ok(backtrack_offset) = table.backtrack_class_def_offset();
      let backtrack = match backtrack_offset.nullness() {
        Ok(Nullness::Null) => empty_backtrack,
        Ok(Nullness::NonNull) => {
          let classes = table.backtrack_class_def()?;
          classes.intersect_classes(glyphs)
        }
      };
      let Ok(lookahead_offset) = table.lookahead_class_def_offset();
      let lookahead = match lookahead_offset.nullness() {
        Ok(Nullness::Null) => empty_lookahead,
        Ok(Nullness::NonNull) => {
          let classes = table.lookahead_class_def()?;
          classes.intersect_classes(glyphs)
        }
      };
      (backtrack, lookahead)
    }
  };

  Ok(Some(ContextClasses { input_class_def, coverage: coverage_classes, input, backtrack, lookahead }))
}

impl GlyphClosure for ContextFormat2<'_> {
  fn closure_glyphs(
    &self,
    ctx: &mut ClosureCtx,
    lookup_list: &SubstitutionLookupList,
    _lookup_index: u16,
  ) -> Result<(), ReadError> {
    let available = selected_context_classes(self, ctx)?;
    let ContextClasses {
      input_class_def,
      coverage: coverage_glyph_classes,
      input: input_glyph_classes,
      backtrack: backtrack_classes,
      lookahead: lookahead_classes,
    } = match available {
      Some(classes) => classes,
      None => return Ok(()),
    };
    let Ok(lookups) = lookup_list.lookups();

    let Ok(rule_sets) = self.rule_sets();
    let reachable = rule_sets.enumerate().filter_map(|(ordinal, rule_set)| {
      let Ok(class) = console_core_number_conversion::wrapped::<_, u16>(ordinal);

      match coverage_glyph_classes.contains(class) {
        Ok(crate::Presence::Present) => rule_set.transpose().map(|rules| (class, rules)),
        Ok(crate::Presence::Absent) => None,
      }
    });

    for (i, rule_set) in reachable {
      match ctx.lookup_budget() {
        Ok(TraversalBudget::Exhausted) => return Ok(()),
        Ok(TraversalBudget::Available) => {}
      }

      let rule_set_2 = rule_set?;
      let Ok(rules) = rule_set_2.rules();

      'over_rule_2: for rule in rules {
        match ctx.lookup_budget() {
          Ok(TraversalBudget::Exhausted) => return Ok(()),
          Ok(TraversalBudget::Available) => {}
        }

        let chain_rule = rule?;
        let rule_3 = match chain_rule {
          Some(rule_2) => rule_2,
          None => {
            continue 'over_rule_2;
          }
        };

        match ::core::primitive::bool::then_some(matches!(rule_3.intersects(&input_glyph_classes, &backtrack_classes, &lookahead_classes), Ok(crate::Overlap::Disjoint)), ()) {
          ::core::option::Option::Some(()) => {
            continue 'over_rule_2;
          }
          ::core::option::Option::None => {}
        }

        let Ok(input_seq) = rule_3.input_sequence();
        let Ok(input_count) = console_core_number_conversion::fitted::<_, u64>(input_seq.len().wrapping_add(1));

        let Ok(mut seen_sequence_indices) = IntSet::new();

        let Ok(lookup_records) = rule_3.lookup_records();

        'over_lookup_record_3: for lookup_record in lookup_records {
          let Ok(lookup_index) = lookup_record.lookup_list_index();
          let lookup = match lookups.get(u64::from(lookup_index)) {
            Ok(lookup) => lookup,
            Err(ReadError::NullOffset | ReadError::InvalidCollectionIndex(_)) => continue 'over_lookup_record_3,
            Err(error) => return Err(error),
          };
          let Ok(sequence_idx) = lookup_record.sequence_index();

          match (u64::from(sequence_idx) >= input_count).then_some(()) {
            Some(()) => continue 'over_lookup_record_3,
            None => {}
          }

          let Ok(prior) = seen_sequence_indices.insert(sequence_idx);
          let active_glyphs = match (prior, sequence_idx) {
            (crate::Presence::Present, _) => {
              let Ok(current) = ctx.glyphs();
              current.clone()
            }
            (crate::Presence::Absent, 0) => {
              let Ok(parent) = ctx.parent_active_glyphs();
              input_class_def.intersected_class_glyphs(parent, i)
            }
            (crate::Presence::Absent, position) => {
              let Ok(index) = console_core_number_conversion::index(position.wrapping_sub(1));
              let entry = input_seq.get(index).ok_or(ReadError::OutOfBounds)?;
              let Ok(class) = entry.get();
              let Ok(current) = ctx.glyphs();
              input_class_def.intersected_class_glyphs(current, class)
            }
          };

          let Ok(sequence_end) = console_core_number_conversion::wrapped::<_, u16>(input_count);
          ctx.recurse(
            lookup_list,
            &lookup,
            lookup_index,
            active_glyphs,
            &mut seen_sequence_indices,
            sequence_idx..=sequence_end,
          )?;
        }
      }
    }

    Ok(())
  }
}

impl GlyphClosure for ContextFormat3<'_> {
  fn closure_glyphs(
    &self,
    ctx: &mut ClosureCtx,
    lookup_list: &SubstitutionLookupList,
    _lookup_index: u16,
  ) -> Result<(), ReadError> {
    let Ok(glyphs) = ctx.glyphs();
    let intersects = self.intersects(glyphs)?;

    match ::core::primitive::bool::then_some(!intersects, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let Ok(value_89) = IntSet::new();

    let mut seen_sequence_indices = value_89;
    let Ok(input_coverages) = self.coverages();
    let Ok(input_count) = input_coverages.len();
    let Ok(lookups) = lookup_list.lookups();

    let Ok(lookup_records) = self.lookup_records();

    for record in lookup_records {
      let Ok(lookup_index) = record.lookup_list_index();
      let lookup = match lookups.get({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(lookup_index);
        migrated_value
      }) {
        Err(ReadError::NullOffset) | Err(ReadError::InvalidCollectionIndex(_)) => continue,
        Ok(value) => Ok(value),
        Err(unreadable_layout_lookup) => Err(unreadable_layout_lookup),
      }?;

      let Ok(seq_idx) = record.sequence_index();

      match ::core::primitive::bool::then_some(({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(seq_idx);
        migrated_value
      }) >= input_count, ())
      {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      let Ok(value_102) = seen_sequence_indices.insert(seq_idx);

      let active_glyphs = match ::core::primitive::bool::then_some(value_102 == console_core_integer_sets::Presence::Present, ()) {
        ::core::option::Option::Some(()) => {
            let Ok(ctx_glyphs) = ctx.glyphs();

            ctx_glyphs.clone()
        }
        ::core::option::Option::None => match ::core::primitive::bool::then_some(seq_idx == 0, ()) {
          ::core::option::Option::Some(()) => {
            let cov = input_coverages.get(0)?;
            let Ok(parent_glyphs) = ctx.parent_active_glyphs();

            cov.intersect_set(parent_glyphs)
          }
          ::core::option::Option::None => {
            let cov = input_coverages.get({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(seq_idx);
              migrated_value
            })?;
            let Ok(ctx_glyphs) = ctx.glyphs();

            cov.intersect_set(ctx_glyphs)
          }
        },
      };

      let value_46 = &mut seen_sequence_indices;

      ctx.recurse(
        lookup_list,
        &lookup,
        lookup_index,
        active_glyphs,
        value_46,
        seq_idx
          ..=::core::primitive::u16::wrapping_add(
            {
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u16>(input_count);
              migrated_value
            },
            1,
          ),
      )?;
    }

    Ok(())
  }
}

impl SubstitutionLookupList<'_> {
  pub fn closure_lookups(
    &self,
    glyph_set: &IntSet<GlyphId>,
    lookup_indices: &mut IntSet<u16>,
  ) -> Result<(), ReadError> {
    let Ok(lookup_count) = self.lookup_count();
    let Ok(()) = lookup_indices.remove_range(lookup_count..=u16::MAX);
    let Ok(value_47) = lookup_indices.is_empty();

    match ::core::primitive::bool::then_some(value_47 == console_core_integer_sets::Emptiness::Empty, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let lookup_list = LayoutLookupList::Gsub(self);
    let Ok(mut c) = LookupClosureCtx::new(glyph_set, &lookup_list);

    let Ok(lookups) = self.lookups();
    let Ok(value_104) = lookup_indices.iter();

    for idx in value_104 {
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

impl LookupClosure for SubstitutionLookup<'_> {
  fn closure_lookups(
    &self,
    c: &mut LookupClosureCtx,
    lookup_index: u16,
  ) -> Result<(), ReadError> {
    match ::core::primitive::bool::then_some(matches!(c.visit_lookup(lookup_index), Ok(VisitDecision::Skip)), ()) {
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

impl Intersect for SubstitutionLookup<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    match self {
      SubstitutionLookup::Single(inner) => {
        let Ok(subtables) = inner.subtables();

        subtables.intersects(glyph_set)
      }
      SubstitutionLookup::Multiple(inner) => {
        let Ok(subtables) = inner.subtables();

        subtables.intersects(glyph_set)
      }
      SubstitutionLookup::Alternate(inner) => {
        let Ok(subtables) = inner.subtables();

        subtables.intersects(glyph_set)
      }
      SubstitutionLookup::Ligature(inner) => {
        let Ok(subtables) = inner.subtables();

        subtables.intersects(glyph_set)
      }
      SubstitutionLookup::Contextual(inner) => {
        let Ok(subtables) = inner.subtables();

        subtables.intersects(glyph_set)
      }
      SubstitutionLookup::ChainContextual(inner) => {
        let Ok(subtables) = inner.subtables();

        subtables.intersects(glyph_set)
      }
      SubstitutionLookup::Extension(inner) => {
        let Ok(subtables) = inner.subtables();

        subtables.intersects(glyph_set)
      }
      SubstitutionLookup::Reverse(inner) => {
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
      ExtensionSubtable::Multiple(inner) => inner.intersects(glyph_set),
      ExtensionSubtable::Alternate(inner) => inner.intersects(glyph_set),
      ExtensionSubtable::Ligature(inner) => inner.intersects(glyph_set),
      ExtensionSubtable::Contextual(inner) => inner.intersects(glyph_set),
      ExtensionSubtable::ChainContextual(inner) => inner.intersects(glyph_set),
      ExtensionSubtable::Reverse(inner) => inner.intersects(glyph_set),
    }
  }
}

impl<'a, T> Intersect for ExtensionSubstFormat1<'a, T>
where
  T: Intersect + FontRead<'a>,
{
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(extension_offset) = self.extension_offset();
    let Ok(value_48) = extension_offset.nullness();
    let value_48_2 = matches!(value_48, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some(value_48_2, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(false);
      }
      ::core::option::Option::None => {}
    }

    let extension = self.extension()?;

    extension.intersects(glyph_set)
  }
}

impl LookupClosure for SubstitutionSubtables<'_> {
  fn closure_lookups(&self, c: &mut LookupClosureCtx, arg: u16) -> Result<(), ReadError> {
    match self {
      SubstitutionSubtables::ChainContextual(subtables) => subtables.closure_lookups(c, arg),
      SubstitutionSubtables::Contextual(subtables) => subtables.closure_lookups(c, arg),
      SubstitutionSubtables::Single(_)
      | SubstitutionSubtables::Multiple(_)
      | SubstitutionSubtables::Alternate(_)
      | SubstitutionSubtables::Ligature(_)
      | SubstitutionSubtables::Reverse(_)
      | SubstitutionSubtables::EmptyExtension => Ok(()),
    }
  }
}

impl Intersect for SingleSubst<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    match self {
      Self::Format1(item) => item.intersects(glyph_set),
      Self::Format2(item) => item.intersects(glyph_set),
    }
  }
}

impl Intersect for SingleSubstFormat1<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(coverage_offset) = self.coverage_offset();
    let Ok(value_49) = coverage_offset.nullness();
    let value_49_2 = matches!(value_49, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some(value_49_2, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(false);
      }
      ::core::option::Option::None => {}
    }

    let coverage = self.coverage()?;

    Ok(coverage.intersects(glyph_set))
  }
}

impl Intersect for SingleSubstFormat2<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(coverage_offset) = self.coverage_offset();
    let Ok(value_50) = coverage_offset.nullness();
    let value_50_2 = matches!(value_50, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some(value_50_2, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(false);
      }
      ::core::option::Option::None => {}
    }

    let coverage = self.coverage()?;

    Ok(coverage.intersects(glyph_set))
  }
}

impl Intersect for MultipleSubstFormat1<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(coverage_offset) = self.coverage_offset();
    let Ok(value_51) = coverage_offset.nullness();
    let value_51_2 = matches!(value_51, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some(value_51_2, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(false);
      }
      ::core::option::Option::None => {}
    }

    let coverage = self.coverage()?;

    Ok(coverage.intersects(glyph_set))
  }
}

impl Intersect for AlternateSubstFormat1<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(coverage_offset) = self.coverage_offset();
    let Ok(value_52) = coverage_offset.nullness();
    let value_52_2 = matches!(value_52, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some(value_52_2, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(false);
      }
      ::core::option::Option::None => {}
    }

    let coverage = self.coverage()?;

    Ok(coverage.intersects(glyph_set))
  }
}

impl Intersect for LigatureSubstFormat1<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(coverage_offset) = self.coverage_offset();
    let Ok(value_53) = coverage_offset.nullness();
    let value_53_2 = matches!(value_53, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some(value_53_2, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(false);
      }
      ::core::option::Option::None => {}
    }

    let coverage = self.coverage()?;
    let Ok(lig_sets) = self.ligature_sets();
    let Ok(value_107) = lig_sets.iter_as_nullable();

    for lig_set in coverage.iter().zip(value_107).filter_map(|(g, lig_set)| {
      let Ok(value_54) = glyph_set.contains(GlyphId::from(g));

      (value_54 == console_core_integer_sets::Presence::Present).then_some(lig_set)
    }) {
      let ligature_set = lig_set?;
      let lig_set_3 = match ligature_set {
        Some(lig_set_2) => lig_set_2,
        None => {
          continue;
        }
      };

      let intersects = lig_set_3.intersects(glyph_set)?;

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

impl Intersect for LigatureSet<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(ligs) = self.ligatures();
    let Ok(value_55) = ligs.iter_as_nullable();

    for lig in value_55 {
      let lig_2 = lig?;
      let lig_4 = match lig_2 {
        Some(lig_3) => lig_3,
        None => continue,
      };
      let intersects = lig_4.intersects(glyph_set)?;

      match ::core::primitive::bool::then_some(intersects, ()) {
        ::core::option::Option::Some(()) => return Ok(true),
        ::core::option::Option::None => {}
      }
    }

    Ok(false)
  }
}

impl Intersect for Ligature<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(component_glyph_ids) = self.component_glyph_ids();
    let ret = component_glyph_ids.iter().all(|g| {
      let Ok(value_108) = g.get();

      let Ok(value_119) = glyph_set.contains(GlyphId::from(value_108));

      value_119 == console_core_integer_sets::Presence::Present
    });
    Ok(ret)
  }
}

impl Intersect for ReverseChainSingleSubstFormat1<'_> {
  fn intersects(&self, glyph_set: &IntSet<GlyphId>) -> Result<bool, ReadError> {
    let Ok(coverage_offset) = self.coverage_offset();
    let Ok(value_57) = coverage_offset.nullness();
    let value_57_2 = matches!(value_57, font_types::Nullness::Null);

    match ::core::primitive::bool::then_some(value_57_2, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(false);
      }
      ::core::option::Option::None => {}
    }

    let own_coverage = self.coverage()?;

    match ::core::primitive::bool::then_some(!own_coverage.intersects(glyph_set), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(false);
      }
      ::core::option::Option::None => {}
    }

    let Ok(backtrack_coverages) = self.backtrack_coverages();
    let Ok(value_109) = backtrack_coverages.iter_as_nullable();

    for coverage in value_109 {
      let context_coverage = coverage?;
      let coverage_3 = match context_coverage {
        Some(coverage_2) => coverage_2,
        None => {
          return Ok(false);
        }
      };

      match ::core::primitive::bool::then_some(!coverage_3.intersects(glyph_set), ()) {
        ::core::option::Option::Some(()) => {
          return Ok(false);
        }
        ::core::option::Option::None => {}
      }
    }

    let Ok(lookahead_coverages) = self.lookahead_coverages();
    let Ok(value_110) = lookahead_coverages.iter_as_nullable();

    for coverage in value_110 {
      let chain_coverage = coverage?;
      let coverage_5 = match chain_coverage {
        Some(coverage_4) => coverage_4,
        None => {
          return Ok(false);
        }
      };

      match ::core::primitive::bool::then_some(!coverage_5.intersects(glyph_set), ()) {
        ::core::option::Option::Some(()) => {
          return Ok(false);
        }
        ::core::option::Option::None => {}
      }
    }

    Ok(true)
  }
}

#[cfg(test)]
mod tests {
  use std::collections::{HashMap, HashSet};

  use crate::{FontRef, TableProvider, test_support::{Failure, fixture_value}};

  use super::*;
  use font_test_data::closure as test_data;

  struct GlyphMap {
    to_gid: HashMap<&'static str, GlyphId>,
    from_gid: HashMap<GlyphId, &'static str>,
  }

  impl GlyphMap {
    fn new(raw_order: &'static str) -> Result<Self, Failure> {
      let mut to_gid = HashMap::new();
      let names = raw_order
        .lines()
        .map(str::trim)
        .filter(|line| !(line.starts_with('#') || line.is_empty()));

      for (index, name) in names.enumerate() {
        let id = u32::try_from(index)?;
        let Ok(glyph) = GlyphId::new(id);
        to_gid.insert(name, glyph);
      }

      let from_gid = to_gid.iter().map(|(name, gid)| (*gid, *name)).collect();
      Ok(Self { from_gid, to_gid })
    }
  }

  fn get_gsub(test_data: &'static [u8]) -> Result<Gsub<'static>, ReadError> {
    let font = FontRef::new(test_data)?;
    font.gsub()
  }

  fn compute_closure(
    gsub: &Gsub,
    glyph_map: &GlyphMap,
    input: &[&str],
  ) -> Result<IntSet<GlyphId>, Failure> {
    let Ok(all_glyphs) = IntSet::all();
    let lookup_indices = gsub.collect_lookups(&all_glyphs)?;
    let mut input_glyphs = input
      .iter()
      .map(|name| fixture_value(glyph_map.to_gid.get(name).copied(), "closure input glyph"))
      .collect::<Result<_, _>>()?;
    gsub.closure_glyphs(&lookup_indices, &mut input_glyphs)?;
    Ok(input_glyphs)
  }

  fn check_closure(
    glyph_map: &GlyphMap,
    result: &IntSet<GlyphId>,
    expected_names: &[&str],
  ) -> Result<(), Failure> {
    let Ok(glyphs) = result.iter();
    let actual = glyphs
      .map(|glyph| fixture_value(glyph_map.from_gid.get(&glyph).copied(), "closure output glyph"))
      .collect::<Result<HashSet<_>, _>>()?;
    let expected = expected_names.iter().copied().collect::<HashSet<_>>();

    match ::core::primitive::bool::then_some(actual == expected, ()) {
      ::core::option::Option::Some(()) => Ok(()),
      ::core::option::Option::None => {
        let missing: Vec<_> = expected.difference(&actual).collect();
        let unexpected: Vec<_> = actual.difference(&expected).collect();
        Err(Failure::from(format!("Closure output does not match\nmissing {missing:?}\nunexpected {unexpected:?}")))
      }
    }
  }

  #[test]
  fn smoke_test() -> Result<(), Failure> {
    let gsub = get_gsub(test_data::SIMPLE)?;
    let glyph_map = GlyphMap::new(test_data::SIMPLE_GLYPHS)?;
    let result = compute_closure(&gsub, &glyph_map, &["a"])?;

    check_closure(
      &glyph_map,
      &result,
      &["a", "A", "b", "c", "d", "a_a", "a.1", "a.2", "a.3"]
    )?;
    Ok(())
  }

  #[test]
  fn recursive() -> Result<(), Failure> {
    let gsub = get_gsub(test_data::RECURSIVE)?;
    let glyph_map = GlyphMap::new(test_data::RECURSIVE_GLYPHS)?;
    let result = compute_closure(&gsub, &glyph_map, &["a"])?;
    check_closure(&glyph_map, &result, &["a", "b", "c", "d"])?;
    Ok(())
  }

  #[test]
  fn contextual_lookups_nop() -> Result<(), Failure> {
    let gsub = get_gsub(test_data::CONTEXTUAL)?;
    let glyph_map = GlyphMap::new(test_data::CONTEXTUAL_GLYPHS)?;

    let nop = compute_closure(&gsub, &glyph_map, &["three", "four", "e", "f"])?;
    check_closure(&glyph_map, &nop, &["three", "four", "e", "f"])?;
    Ok(())
  }

  #[test]
  fn contextual_lookups_chained_f1() -> Result<(), Failure> {
    let gsub = get_gsub(test_data::CONTEXTUAL)?;
    let glyph_map = GlyphMap::new(test_data::CONTEXTUAL_GLYPHS)?;
    let gsub6f1 = compute_closure(
      &gsub,
      &glyph_map,
      &["one", "two", "three", "four", "five", "six", "seven"],
    )?;
    check_closure(
      &glyph_map,
      &gsub6f1,
      &[
        "one", "two", "three", "four", "five", "six", "seven", "X", "Y"
      ]
    )?;
    Ok(())
  }

  #[test]
  fn contextual_lookups_chained_f3() -> Result<(), Failure> {
    let gsub = get_gsub(test_data::CONTEXTUAL)?;
    let glyph_map = GlyphMap::new(test_data::CONTEXTUAL_GLYPHS)?;
    let gsub6f3 = compute_closure(&gsub, &glyph_map, &["space", "e"])?;
    check_closure(&glyph_map, &gsub6f3, &["space", "e", "e.2"])?;

    let gsub5f3 = compute_closure(&gsub, &glyph_map, &["f", "g"])?;
    check_closure(&glyph_map, &gsub5f3, &["f", "g", "f.2"])?;
    Ok(())
  }

  #[test]
  fn contextual_plain_f1() -> Result<(), Failure> {
    let gsub = get_gsub(test_data::CONTEXTUAL)?;
    let glyph_map = GlyphMap::new(test_data::CONTEXTUAL_GLYPHS)?;
    let gsub5f1 = compute_closure(&gsub, &glyph_map, &["a", "b"])?;
    check_closure(&glyph_map, &gsub5f1, &["a", "b", "a_b"])?;
    Ok(())
  }

  #[test]
  fn contextual_plain_f3() -> Result<(), Failure> {
    let gsub = get_gsub(test_data::CONTEXTUAL)?;
    let glyph_map = GlyphMap::new(test_data::CONTEXTUAL_GLYPHS)?;
    let gsub5f3 = compute_closure(&gsub, &glyph_map, &["f", "g"])?;
    check_closure(&glyph_map, &gsub5f3, &["f", "g", "f.2"])?;
    Ok(())
  }

  #[test]
  fn recursive_context() -> Result<(), Failure> {
    let gsub = get_gsub(test_data::RECURSIVE_CONTEXTUAL)?;
    let glyph_map = GlyphMap::new(test_data::RECURSIVE_CONTEXTUAL_GLYPHS)?;

    let nop = compute_closure(&gsub, &glyph_map, &["b", "B"])?;
    check_closure(&glyph_map, &nop, &["b", "B"])?;

    let full = compute_closure(&gsub, &glyph_map, &["a", "b", "c"])?;
    check_closure(&glyph_map, &full, &["a", "b", "c", "B", "B.2", "B.3"])?;

    let intermediate = compute_closure(&gsub, &glyph_map, &["a", "B.2"])?;
    check_closure(&glyph_map, &intermediate, &["a", "B.2", "B.3"])?;
    Ok(())
  }

  #[test]
  fn feature_variations() -> Result<(), Failure> {
    let gsub = get_gsub(test_data::VARIATIONS_CLOSURE)?;
    let glyph_map = GlyphMap::new(test_data::VARIATIONS_GLYPHS)?;

    let input = compute_closure(&gsub, &glyph_map, &["a"])?;
    check_closure(&glyph_map, &input, &["a", "b", "c"])?;
    Ok(())
  }

  #[test]
  fn chain_context_format3() -> Result<(), Failure> {
    let gsub = get_gsub(test_data::CHAIN_CONTEXT_FORMAT3_BITS)?;
    let glyph_map = GlyphMap::new(test_data::CHAIN_CONTEXT_FORMAT3_BITS_GLYPHS)?;

    let nop = compute_closure(&gsub, &glyph_map, &["c", "z"])?;
    check_closure(&glyph_map, &nop, &["c", "z"])?;

    let full = compute_closure(&gsub, &glyph_map, &["a", "b", "c", "z"])?;
    check_closure(&glyph_map, &full, &["a", "b", "c", "z", "A", "B"])?;
    Ok(())
  }

  #[test]
  fn closure_ignore_unreachable_glyphs() -> Result<(), Failure> {
    let font = FontRef::new(font_test_data::closure::CONTEXT_ONLY_REACHABLE)?;
    let gsub = font.gsub()?;
    let glyph_map = GlyphMap::new(test_data::CONTEXT_ONLY_REACHABLE_GLYPHS)?;
    let result = compute_closure(&gsub, &glyph_map, &["a", "b", "c", "d", "e", "f", "period"])?;
    check_closure(
      &glyph_map,
      &result,
      &["a", "b", "c", "d", "e", "f", "period", "A", "B", "C"]
    )?;
    Ok(())
  }

  #[test]
  fn cyclical_context() -> Result<(), Failure> {
    let gsub = get_gsub(test_data::CYCLIC_CONTEXTUAL)?;
    let glyph_map = GlyphMap::new(test_data::RECURSIVE_CONTEXTUAL_GLYPHS)?;

    let nop = compute_closure(&gsub, &glyph_map, &["a", "b", "c"])?;
    check_closure(&glyph_map, &nop, &["a", "b", "c"])?;
    Ok(())
  }

  #[test]
  fn collect_all_features() -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::closure::CONTEXTUAL)?;
    let gsub = font.gsub()?;
    let ret = gsub.collect_features(
      &{
        let Ok(all) = IntSet::all();
        all
      },
      &{
        let Ok(all) = IntSet::all();
        all
      },
      &{
        let Ok(all) = IntSet::all();
        all
      },
    )?;
    let Ok(value_111) = ret.len();

    assert_eq!(value_111, 2);
    let Ok(value_112) = ret.contains(0);

    assert_eq!(value_112, console_core_integer_sets::Presence::Present);
    let Ok(value_113) = ret.contains(1);

    assert_eq!(value_113, console_core_integer_sets::Presence::Present);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn collect_all_features_with_feature_filter()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::closure::CONTEXTUAL)?;
    let gsub = font.gsub()?;

    let Ok(mut feature_tags) = IntSet::empty();
    let Ok(_previous_presence) = (feature_tags).insert({
      let Ok(value) = Tag::new(b"SUB5");
      value
    });

    let ret = gsub.collect_features(
      &{
        let Ok(all) = IntSet::all();
        all
      },
      &{
        let Ok(all) = IntSet::all();
        all
      },
      &feature_tags,
    )?;
    let Ok(value_115) = ret.len();

    assert_eq!(value_115, 1);
    let Ok(value_116) = ret.contains(0);

    assert_eq!(value_116, console_core_integer_sets::Presence::Present);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn collect_all_features_with_script_filter()
  -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::closure::CONTEXTUAL)?;
    let gsub = font.gsub()?;

    let Ok(mut script_tags) = IntSet::empty();
    let Ok(_previous_presence) = (script_tags).insert({
      let Ok(value) = Tag::new(b"LATN");
      value
    });

    let ret = gsub.collect_features(
      &script_tags,
      &{
        let Ok(all) = IntSet::all();
        all
      },
      &{
        let Ok(all) = IntSet::all();
        all
      },
    )?;
    let Ok(value_118) = ret.is_empty();

    assert_eq!(value_118, console_core_integer_sets::Emptiness::Empty);
    ::core::result::Result::Ok(())
  }
}
