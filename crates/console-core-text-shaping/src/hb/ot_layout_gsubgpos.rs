//! Matching of glyph patterns.

use super::buffer::GlyphInfo;
use super::buffer::{GlyphPropsFlags, hb_buffer_t};
use super::cache::hb_cache_t;
use super::face::Scale;
use super::hb_font_t;
use super::hb_mask_t;
use super::ot_layout::*;
use super::ot_layout_common::*;
use super::set_digest::hb_set_digest_t;
use crate::hb::ot::{ClassDefInfo, CoverageInfo};
use crate::hb::ot_layout_gsubgpos::OT::check_glyph_property;
use crate::hb::unicode::GeneralCategory;
use alloc::boxed::Box;
use alloc::vec::Vec;
use read_fonts::tables::layout::SequenceLookupRecord;
use read_fonts::types::GlyphId;

pub(crate) type MatchPositions = Vec<u32>;

/// Value represents glyph id.
pub fn match_glyph(info: &mut GlyphInfo, value: u32) -> bool {
  info.glyph_id == value
}

pub fn match_always(_info: &mut GlyphInfo, _value: u32) -> bool {
  true
}

pub fn match_input(
  ctx: &mut hb_ot_apply_context_t,
  input_len: u16,
  match_func: impl Fn(&mut GlyphInfo, u32) -> bool,
  end_position: &mut usize,
  p_total_component_count: Option<&mut u8>,
) -> bool {
  #[derive(PartialEq)]
  enum Ligbase {
    NotChecked,
    MayNotSkip,
    MaySkip,
  }

  let count = usize::from(input_len) + 1;

  match count == 1 {
    true => {
      *end_position = ctx.buffer.idx + 1;
      ctx.match_positions_len = 1;
      ctx.match_positions[0] = ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u32>(ctx.buffer.idx);
        migrated_value
      });
      match p_total_component_count {
        Some(p_total_component_count) => {
          *p_total_component_count = ctx.buffer.cur(0).lig_num_comps();
        }
        None => {}
      }
      return true;
    }
    false => {}
  }

  match count > MAX_CONTEXT_LENGTH {
    true => {
      return false;
    }
    false => {}
  }
  ctx.match_positions_len = count;

  let mut iter = skipping_iterator_t::with_match_fn(ctx, false, Some(match_func));
  iter.reset(iter.buffer.idx);
  iter.set_glyph_data(0);

  let first = *iter.buffer.cur(0);
  let first_lig_id = first.lig_id();
  let first_lig_comp = first.lig_comp();
  let mut total_component_count = 0;
  let mut ligbase = Ligbase::NotChecked;

  for i in 1..count {
    let mut unsafe_to = 0;
    match !iter.next(Some(&mut unsafe_to)) {
      true => {
        *end_position = unsafe_to;
        return false;
      }
      false => {}
    }

    iter.set_match_position(i, iter.index());

    let this = iter.buffer.info[iter.index()];
    let this_lig_id = this.lig_id();
    let this_lig_comp = this.lig_comp();

    match first_lig_id != 0 && first_lig_comp != 0 {
      true => match first_lig_id != this_lig_id || first_lig_comp != this_lig_comp {
        true => {
          match ligbase == Ligbase::NotChecked {
            true => {
              let out = iter.buffer.out_info();
              let mut j = iter.buffer.out_len;
              let mut found = false;
              while j > 0 && out[j - 1].lig_id() == first_lig_id {
                match out[j - 1].lig_comp() == 0 {
                  true => {
                    j -= 1;
                    found = true;
                    break;
                  }
                  false => {}
                }
                j -= 1;
              }

              ligbase = match found && iter.may_skip(&out[j]) == may_skip_t::SKIP_YES {
                true => Ligbase::MaySkip,
                false => Ligbase::MayNotSkip,
              };
            }
            false => {}
          }

          match ligbase == Ligbase::MayNotSkip {
            true => {
              return false;
            }
            false => {}
          }
        }
        false => {}
      },
      false => match this_lig_id != 0 && this_lig_comp != 0 && (this_lig_id != first_lig_id) {
        true => {
          return false;
        }
        false => {}
      },
    }

    total_component_count += this.lig_num_comps();
  }

  *end_position = iter.index() + 1;

  match p_total_component_count {
    Some(p_total_component_count) => {
      total_component_count += first.lig_num_comps();
      *p_total_component_count = total_component_count;
    }
    None => {}
  }

  ctx.match_positions[0] = ({
    let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(iter.buffer.idx);
    migrated_value
  });

  true
}

pub fn match_backtrack(
  ctx: &mut hb_ot_apply_context_t,
  backtrack_len: u16,
  match_func: impl Fn(&mut GlyphInfo, u32) -> bool,
  match_start: &mut usize,
) -> bool {
  match backtrack_len == 0 {
    true => {
      *match_start = ctx.buffer.backtrack_len();
      return true;
    }
    false => {}
  }

  let mut iter = skipping_iterator_t::with_match_fn(ctx, true, Some(match_func));
  iter.reset_back(iter.buffer.backtrack_len());
  iter.set_glyph_data(0);

  for _ in 0..backtrack_len {
    let mut unsafe_from = 0;
    match !iter.prev(Some(&mut unsafe_from)) {
      true => {
        *match_start = unsafe_from;
        return false;
      }
      false => {}
    }
  }

  *match_start = iter.index();
  true
}

pub fn match_lookahead(
  ctx: &mut hb_ot_apply_context_t,
  lookahead_len: u16,
  match_func: impl Fn(&mut GlyphInfo, u32) -> bool,
  start_index: usize,
  end_index: &mut usize,
) -> bool {
  match lookahead_len == 0 {
    true => {
      *end_index = start_index;
      return true;
    }
    false => {}
  }

  debug_assert!(start_index >= 1);
  let mut iter = skipping_iterator_t::with_match_fn(ctx, true, Some(match_func));
  iter.reset(start_index - 1);
  iter.set_glyph_data(0);

  for _ in 0..lookahead_len {
    let mut unsafe_to = 0;
    match !iter.next(Some(&mut unsafe_to)) {
      true => {
        *end_index = unsafe_to;
        return false;
      }
      false => {}
    }
  }

  *end_index = iter.index() + 1;
  true
}

#[derive(PartialEq, Eq, Copy, Clone)]
pub enum match_t {
  MATCH,
  NOT_MATCH,
  SKIP,
}

#[derive(PartialEq, Eq, Copy, Clone)]
enum may_match_t {
  MATCH_NO,
  MATCH_YES,
  MATCH_MAYBE,
}

#[derive(PartialEq, Eq, Copy, Clone)]
pub enum may_skip_t {
  SKIP_NO,
  SKIP_YES,
  SKIP_MAYBE,
}

#[derive(Default)]
pub struct matcher_t {
  lookup_props: u32,
  mask: hb_mask_t,
  ignore_zwnj: bool,
  ignore_zwj: bool,
  ignore_hidden: bool,
  per_syllable: bool,
}

impl matcher_t {
  fn new(ctx: &hb_ot_apply_context_t, context_match: bool) -> Self {
    matcher_t {
      lookup_props: ctx.lookup_props,

      ignore_zwnj: ctx.table_index == TableIndex::GPOS || (context_match && ctx.auto_zwnj),

      ignore_zwj: context_match || ctx.auto_zwj,

      ignore_hidden: ctx.table_index == TableIndex::GPOS,
      mask: match context_match {
        true => u32::MAX,
        false => ctx.lookup_mask(),
      },

      per_syllable: ctx.table_index == TableIndex::GSUB && ctx.per_syllable,
    }
  }

  fn may_match(
    &self,
    info: &mut GlyphInfo,
    glyph_data: u32,
    match_func: Option<&impl Fn(&mut GlyphInfo, u32) -> bool>,
    syllable: u8,
  ) -> may_match_t {
    match (info.mask & self.mask) == 0
      || (self.per_syllable && syllable != 0 && syllable != info.syllable())
    {
      true => {
        return may_match_t::MATCH_NO;
      }
      false => {}
    }

    match match_func {
      Some(match_func) => {
        return match match_func(info, glyph_data) {
          true => may_match_t::MATCH_YES,
          false => may_match_t::MATCH_NO,
        };
      }
      None => {}
    }

    may_match_t::MATCH_MAYBE
  }

  #[inline(always)]
  fn may_skip(&self, info: &GlyphInfo, face: &hb_font_t, lookup_props: u32) -> may_skip_t {
    match !check_glyph_property(face, info, lookup_props) {
      true => {
        return may_skip_t::SKIP_YES;
      }
      false => {}
    }

    match info.is_default_ignorable()
      && (self.ignore_zwnj || !info.is_zwnj())
      && (self.ignore_zwj || !info.is_zwj())
      && (self.ignore_hidden || !info.is_hidden())
    {
      true => {
        return may_skip_t::SKIP_MAYBE;
      }
      false => {}
    }

    may_skip_t::SKIP_NO
  }
}

pub struct skipping_iterator_t<'f, 'c, F> {
  pub(crate) buffer: &'c mut hb_buffer_t,
  face: &'c hb_font_t<'f>,
  matcher: &'c matcher_t,
  match_positions: &'c mut MatchPositions,
  buf_len: usize,
  glyph_data: u32,
  pub(crate) buf_idx: usize,
  match_func: Option<F>,
  lookup_props: u32,
  syllable: u8,
}

impl<'f, 'c> skipping_iterator_t<'f, 'c, fn(&mut GlyphInfo, u32) -> bool> {
  pub fn new(ctx: &'c mut hb_ot_apply_context_t<'f>, context_match: bool) -> Self {
    Self::with_match_fn(ctx, context_match, None)
  }
}

pub(crate) enum MatchSource {
  Info,
  OutInfo,
}

impl<'f, 'c, F> skipping_iterator_t<'f, 'c, F>
where
  F: Fn(&mut GlyphInfo, u32) -> bool,
{
  pub fn with_match_fn(
    ctx: &'c mut hb_ot_apply_context_t<'f>,
    context_match: bool,
    match_fn: Option<F>,
  ) -> Self {
    let matcher = match context_match {
      true => &ctx.context_matcher,
      false => &ctx.matcher,
    };
    let buf_len = ctx.buffer.len;
    skipping_iterator_t {
      buffer: ctx.buffer,
      face: ctx.face,
      glyph_data: 0,
      buf_len,
      buf_idx: 0,
      matcher,
      match_func: match_fn,
      match_positions: &mut ctx.match_positions,
      lookup_props: matcher.lookup_props,
      syllable: 0,
    }
  }

  pub fn set_match_position(&mut self, idx: usize, position: usize) {
    let count = idx + 1;
    match count > self.match_positions.len() {
      true => {
        self.match_positions.resize(count, 0);
      }
      false => {}
    }

    self.match_positions[idx] = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(position);
      migrated_value
    });
  }

  pub fn set_glyph_data(&mut self, glyph_data: u32) {
    self.glyph_data = glyph_data;
  }

  fn advance_glyph_data(&mut self) {
    self.glyph_data += 1;
  }

  pub fn set_lookup_props(&mut self, lookup_props: u32) {
    self.lookup_props = lookup_props;
  }

  pub fn index(&self) -> usize {
    self.buf_idx
  }

  #[inline]
  pub fn next(&mut self, unsafe_to: Option<&mut usize>) -> bool {
    let stop = self.buf_len.saturating_sub(1);

    while self.buf_idx < stop {
      self.buf_idx += 1;

      match self.match_at(self.buf_idx, MatchSource::Info) {
        match_t::MATCH => {
          self.advance_glyph_data();
          return true;
        }
        match_t::NOT_MATCH => {
          match unsafe_to {
            Some(unsafe_to) => {
              *unsafe_to = self.buf_idx + 1;
            }
            None => {}
          }

          return false;
        }
        match_t::SKIP => continue,
      }
    }

    match unsafe_to {
      Some(unsafe_to) => {
        *unsafe_to = self.buf_idx + 1;
      }
      None => {}
    }

    false
  }

  #[inline]
  pub fn prev(&mut self, unsafe_from: Option<&mut usize>) -> bool {
    let stop: usize = 0;

    while self.buf_idx > stop {
      self.buf_idx -= 1;

      match self.match_at(self.buf_idx, MatchSource::OutInfo) {
        match_t::MATCH => {
          self.advance_glyph_data();
          return true;
        }
        match_t::NOT_MATCH => {
          match unsafe_from {
            Some(unsafe_from) => {
              *unsafe_from = self.buf_idx.max(1) - 1;
            }
            None => {}
          }

          return false;
        }
        match_t::SKIP => {
          continue;
        }
      }
    }

    match unsafe_from {
      Some(unsafe_from) => {
        *unsafe_from = 0;
      }
      None => {}
    }

    false
  }

  pub fn reset(&mut self, start_index: usize) {
    self.buf_idx = start_index;
    self.buf_len = self.buffer.len;
    self.syllable = self.buffer.cur(0).syllable();
  }

  pub fn reset_back(&mut self, start_index: usize) {
    self.buf_idx = start_index;
    self.syllable = self.buffer.cur(0).syllable();
  }

  pub fn reset_fast(&mut self, start_index: usize) {
    self.buf_idx = start_index;
  }

  pub fn may_skip(&self, info: &GlyphInfo) -> may_skip_t {
    self.matcher.may_skip(info, self.face, self.lookup_props)
  }

  #[inline]
  pub fn match_at(&mut self, idx: usize, source: MatchSource) -> match_t {
    let info = match source {
      MatchSource::Info => &mut self.buffer.info[idx],
      MatchSource::OutInfo => &mut self.buffer.out_info_mut()[idx],
    };
    let skip = self.matcher.may_skip(info, self.face, self.lookup_props);

    match skip == may_skip_t::SKIP_YES {
      true => {
        return match_t::SKIP;
      }
      false => {}
    }

    let _match = self.matcher.may_match(
      info,
      self.glyph_data,
      self.match_func.as_ref(),
      self.syllable,
    );

    match _match == may_match_t::MATCH_YES
      || (_match == may_match_t::MATCH_MAYBE && skip == may_skip_t::SKIP_NO)
    {
      true => {
        return match_t::MATCH;
      }
      false => {}
    }

    match skip == may_skip_t::SKIP_NO {
      true => {
        return match_t::NOT_MATCH;
      }
      false => {}
    }

    match_t::SKIP
  }
}

pub(crate) fn apply_lookup(
  ctx: &mut hb_ot_apply_context_t,
  input_len: usize,
  match_end: usize,
  lookups: &[SequenceLookupRecord],
) {
  let mut count = input_len + 1;

  debug_assert!(count <= ctx.match_positions.len(), "");

  let mut end: isize = {
    let backtrack_len = ctx.buffer.backtrack_len();
    let delta = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_offset(backtrack_len);
      migrated_value
    }) - ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_offset(ctx.buffer.idx);
      migrated_value
    });

    for j in 0..count {
      ctx.match_positions[j] = ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
          (({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped_offset(ctx.match_positions[j]);
            migrated_value
          }) + delta),
        );
        migrated_value
      });
    }

    ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_offset(backtrack_len);
      migrated_value
    }) + ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_offset(match_end);
      migrated_value
    }) - ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_offset(ctx.buffer.idx);
      migrated_value
    })
  };

  for record in lookups {
    match !ctx.buffer.successful {
      true => {
        break;
      }
      false => {}
    }

    let idx = usize::from(
      ({
        let Ok(value) = record.sequence_index.get();
        value
      }),
    );
    match idx >= count {
      true => {
        continue;
      }
      false => {}
    }

    let orig_len = ctx.buffer.backtrack_len() + ctx.buffer.lookahead_len();

    match ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped_index(ctx.match_positions[idx]);
      migrated_value
    }) >= orig_len
    {
      true => {
        continue;
      }
      false => {}
    }

    match !ctx.buffer.move_to(
      ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped_index(ctx.match_positions[idx]);
        migrated_value
      }),
    ) {
      true => {
        break;
      }
      false => {}
    }

    match ctx.buffer.max_ops <= 0 {
      true => {
        break;
      }
      false => {}
    }

    if ctx
      .recurse({
        let Ok(value) = record.lookup_list_index.get();
        value
      })
      .is_none()
    {
      continue;
    }

    let new_len = ctx.buffer.backtrack_len() + ctx.buffer.lookahead_len();
    let mut delta = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_offset(new_len);
      migrated_value
    }) - ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_offset(orig_len);
      migrated_value
    });
    match delta == 0 {
      true => {
        continue;
      }
      false => {}
    }

    end += delta;
    match end
      < ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped_offset(ctx.match_positions[idx]);
        migrated_value
      }) {
      true => {
        delta += ({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped_offset(ctx.match_positions[idx]);
          migrated_value
        }) - end;
        end = ({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped_offset(ctx.match_positions[idx]);
          migrated_value
        });
      }
      false => {}
    }

    let mut next = idx + 1;

    match delta > 0 {
      true => {
        match ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(delta);
          migrated_value
        }) + count
          > MAX_CONTEXT_LENGTH
        {
          true => {
            break;
          }
          false => {}
        }

        match ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(delta);
          migrated_value
        }) + count
          > ctx.match_positions.len()
        {
          true => {
            ctx.match_positions.resize(
              ({
                let Ok(migrated_value) = console_core_number_conversion::wrapped_index(delta);
                migrated_value
              }) + count,
              0,
            );
          }
          false => {}
        }
      }
      false => {
        delta = delta.max(
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_offset(next);
            migrated_value
          }) - ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_offset(count);
            migrated_value
          }),
        );
        next = ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(
            (({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_offset(next);
              migrated_value
            }) - delta),
          );
          migrated_value
        });
      }
    }

    ctx.match_positions.copy_within(
      next..count,
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(
          (({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_offset(next);
            migrated_value
          }) + delta),
        );
        migrated_value
      }),
    );
    next = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(
        (({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_offset(next);
          migrated_value
        }) + delta),
      );
      migrated_value
    });
    count = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(
        (({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_offset(count);
          migrated_value
        }) + delta),
      );
      migrated_value
    });
    ctx.match_positions_len = count;

    for j in idx + 1..next {
      ctx.match_positions[j] = ctx.match_positions[j - 1] + 1;
    }

    while next < count {
      ctx.match_positions[next] = ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
          (({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped_offset(ctx.match_positions[next]);
            migrated_value
          }) + delta),
        );
        migrated_value
      });
      next += 1;
    }
  }

  ctx.buffer.move_to(end.try_into().unwrap());
}

/// Find out whether a lookup would be applied.
pub trait WouldApply {
  /// Whether the lookup would be applied.
  fn would_apply(&self, ctx: &WouldApplyContext) -> bool;
}

pub(crate) type MappingCache = hb_cache_t<16, 8, 256, 16>;

pub(crate) type BinaryCache = hb_cache_t<15, 1, 256, 8>;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum SubtableExternalCacheMode {
  #[allow(unused)]
  None,
  Small,
  Full,
}

pub(crate) struct LigatureSubstFormat1Cache {
  pub seconds: hb_set_digest_t,
  pub coverage: MappingCache,
}

impl LigatureSubstFormat1Cache {
  pub fn new(seconds: hb_set_digest_t) -> Self {
    LigatureSubstFormat1Cache {
      coverage: MappingCache::new(),
      seconds,
    }
  }
}

pub(crate) struct LigatureSubstFormat1SmallCache {
  pub coverage: CoverageInfo,
  pub seconds: hb_set_digest_t,
}

pub(crate) struct PairPosFormat1Cache {
  pub coverage: MappingCache,
}

impl PairPosFormat1Cache {
  pub fn new() -> Self {
    PairPosFormat1Cache {
      coverage: MappingCache::new(),
    }
  }
}

pub(crate) struct PairPosFormat1SmallCache {
  pub coverage: CoverageInfo,
}

pub(crate) struct PairPosFormat2Cache {
  pub coverage: MappingCache,
  pub first: MappingCache,
  pub second: MappingCache,
}

impl PairPosFormat2Cache {
  pub fn new() -> Self {
    PairPosFormat2Cache {
      coverage: MappingCache::new(),
      first: MappingCache::new(),
      second: MappingCache::new(),
    }
  }
}

pub(crate) struct PairPosFormat2SmallCache {
  pub coverage: CoverageInfo,
  pub first: ClassDefInfo,
  pub second: ClassDefInfo,
}

pub(crate) struct ContextFormat2Cache {
  pub coverage: CoverageInfo,
  pub input: ClassDefInfo,
  pub coverage_cache: BinaryCache,
}

pub(crate) struct ChainContextFormat2Cache {
  pub coverage: CoverageInfo,
  pub backtrack: ClassDefInfo,
  pub input: ClassDefInfo,
  pub lookahead: ClassDefInfo,
  pub coverage_cache: BinaryCache,
}

pub(crate) enum SubtableExternalCache {
  None,
  LigatureSubstFormat1Cache(Box<LigatureSubstFormat1Cache>),
  LigatureSubstFormat1SmallCache(LigatureSubstFormat1SmallCache),
  PairPosFormat1Cache(Box<PairPosFormat1Cache>),
  PairPosFormat1SmallCache(PairPosFormat1SmallCache),
  PairPosFormat2Cache(Box<PairPosFormat2Cache>),
  PairPosFormat2SmallCache(PairPosFormat2SmallCache),
  ContextFormat2Cache(ContextFormat2Cache),
  ChainContextFormat2Cache(ChainContextFormat2Cache),
}

/// Apply a lookup.
pub trait Apply {
  fn apply(&self, ctx: &mut hb_ot_apply_context_t) -> Option<()> {
    self.apply_with_external_cache(ctx, &SubtableExternalCache::None)
  }

  fn apply_with_external_cache(
    &self,
    ctx: &mut hb_ot_apply_context_t,
    _external_cache: &SubtableExternalCache,
  ) -> Option<()> {
    self.apply(ctx)
  }

  fn apply_cached(
    &self,
    ctx: &mut hb_ot_apply_context_t,
    external_cache: &SubtableExternalCache,
  ) -> Option<()> {
    self.apply_with_external_cache(ctx, external_cache)
  }

  fn cache_cost(&self) -> u32 {
    0
  }

  fn external_cache_create(&self, mode: SubtableExternalCacheMode) -> SubtableExternalCache {
    let _ = mode;
    SubtableExternalCache::None
  }
}

pub struct WouldApplyContext<'a> {
  pub glyphs: &'a [GlyphId],
  pub zero_context: bool,
}

pub mod OT {
  use super::*;

  fn match_properties_mark(
    face: &hb_font_t,
    info: &GlyphInfo,
    glyph_props: u16,
    match_props: u32,
  ) -> bool {
    match ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(match_props);
      migrated_value
    }) & lookup_flags::USE_MARK_FILTERING_SET
      != 0
    {
      true => {
        let set_index = ({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u16>((match_props >> 16));
          migrated_value
        });
        return face.ot_tables.is_mark_glyph(
          {
            let Ok(value) = info.as_glyph().to_u32();
            value
          },
          set_index,
        );
      }
      false => {}
    }

    match ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(match_props);
      migrated_value
    }) & lookup_flags::MARK_ATTACHMENT_TYPE_MASK
      != 0
    {
      true => {
        return (({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(match_props);
          migrated_value
        }) & lookup_flags::MARK_ATTACHMENT_TYPE_MASK)
          == (glyph_props & lookup_flags::MARK_ATTACHMENT_TYPE_MASK);
      }
      false => {}
    }

    true
  }

  #[inline(always)]
  pub fn check_glyph_property(face: &hb_font_t, info: &GlyphInfo, match_props: u32) -> bool {
    let glyph_props = info.glyph_props();

    match glyph_props
      & ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(match_props);
        migrated_value
      })
      & lookup_flags::IGNORE_FLAGS
      != 0
    {
      true => {
        return false;
      }
      false => {}
    }

    match glyph_props & GlyphPropsFlags::MARK.bits() != 0 {
      true => {
        return match_properties_mark(face, info, glyph_props, match_props);
      }
      false => {}
    }

    true
  }

  pub struct hb_ot_apply_context_t<'a> {
    pub table_index: TableIndex,
    pub face: &'a hb_font_t<'a>,
    pub scale: Scale,
    pub buffer: &'a mut hb_buffer_t,
    lookup_mask: hb_mask_t,
    pub per_syllable: bool,
    pub lookup_index: u16,
    pub lookup_props: u32,
    pub nesting_level_left: usize,
    pub auto_zwnj: bool,
    pub auto_zwj: bool,
    pub random: bool,
    pub random_state: u32,
    pub new_syllables: Option<u8>,
    pub last_base: i32,
    pub last_base_until: u32,
    pub(crate) matcher: matcher_t,
    pub(crate) context_matcher: matcher_t,
    pub(crate) match_positions_len: usize,
    pub(crate) match_positions: MatchPositions,
  }

  impl<'a> hb_ot_apply_context_t<'a> {
    pub fn new(
      table_index: TableIndex,
      face: &'a hb_font_t<'a>,
      scale: Scale,
      buffer: &'a mut hb_buffer_t,
    ) -> Self {
      Self {
        table_index,
        face,
        scale,
        buffer,
        lookup_mask: 1,
        per_syllable: false,
        lookup_index: u16::MAX,
        lookup_props: u32::MAX,
        nesting_level_left: MAX_NESTING_LEVEL,
        auto_zwnj: true,
        auto_zwj: true,
        random: false,
        random_state: 1,
        new_syllables: None,
        last_base: -1,
        last_base_until: 0,
        matcher: matcher_t::default(),
        context_matcher: matcher_t::default(),
        match_positions_len: 0,
        match_positions: alloc::vec![0; 1],
      }
    }

    #[inline(always)]
    pub fn scale_x(&self, value: f32) -> i32 {
      self.scale.scale_x_f(value)
    }

    #[inline(always)]
    pub fn scale_y(&self, value: f32) -> i32 {
      self.scale.scale_y_f(value)
    }

    pub fn random_number(&mut self) -> u32 {
      self.random_state = self.random_state.wrapping_mul(48271)
        % ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(i32::MAX);
          migrated_value
        });
      self.random_state
    }

    pub fn set_lookup_mask(&mut self, mask: hb_mask_t) {
      self.lookup_mask = mask;
      self.last_base = -1;
      self.last_base_until = 0;
    }

    pub fn lookup_mask(&self) -> hb_mask_t {
      self.lookup_mask
    }

    pub fn update_matchers(&mut self) {
      self.matcher = matcher_t::new(self, false);
      self.context_matcher = matcher_t::new(self, true);
    }

    pub fn recurse(&mut self, sub_lookup_index: u16) -> Option<()> {
      match self.nesting_level_left == 0 {
        true => {
          self.buffer.successful = false;
          return None;
        }
        false => {}
      }

      self.buffer.max_ops -= 1;
      match self.buffer.max_ops < 0 {
        true => {
          self.buffer.successful = false;
          return None;
        }
        false => {}
      }

      self.nesting_level_left -= 1;
      let saved_props = self.lookup_props;
      let saved_index = self.lookup_index;

      self.match_positions.resize(self.match_positions_len, 0);
      let saved_match_positions = self.match_positions.clone();
      let saved_match_positions_len = self.match_positions_len;

      self.lookup_index = sub_lookup_index;
      let applied = self
        .face
        .ot_tables
        .table_data_and_lookup(self.table_index, sub_lookup_index)
        .and_then(|(table_data, lookup)| {
          self.lookup_props = lookup.props();
          self.update_matchers();
          lookup.apply(self, table_data, false)
        });
      self.lookup_props = saved_props;
      self.lookup_index = saved_index;
      self.update_matchers();
      self.match_positions = saved_match_positions;
      self.match_positions_len = saved_match_positions_len;
      self.nesting_level_left += 1;
      applied
    }

    fn set_glyph_class(
      &mut self,
      glyph_id: GlyphId,
      class_guess: GlyphPropsFlags,
      ligature: bool,
      component: bool,
    ) {
      self.buffer.digest.add(glyph_id.into());

      match self.new_syllables {
        Some(syllable) => {
          self.buffer.cur_mut(0).set_syllable(syllable);
        }
        None => {}
      }

      let cur = self.buffer.cur_mut(0);
      let mut props = cur.glyph_props();

      props |= GlyphPropsFlags::SUBSTITUTED.bits();

      match ligature {
        true => {
          props |= GlyphPropsFlags::LIGATED.bits();

          props &= !GlyphPropsFlags::MULTIPLIED.bits();
        }
        false => {}
      }

      match component {
        true => {
          props |= GlyphPropsFlags::MULTIPLIED.bits();
        }
        false => {}
      }

      let has_glyph_classes = self.face.ot_tables.has_glyph_classes();

      match has_glyph_classes {
        true => {
          props &= GlyphPropsFlags::PRESERVE.bits();
          cur.set_glyph_props(props | self.face.ot_tables.glyph_props(glyph_id));
        }
        false => match !class_guess.is_empty() {
          true => {
            props &= GlyphPropsFlags::PRESERVE.bits();
            cur.set_glyph_props(props | class_guess.bits());
          }
          false => {
            cur.set_glyph_props(props);
          }
        },
      }
    }

    pub fn replace_glyph(&mut self, glyph_id: GlyphId) {
      self.set_glyph_class(glyph_id, GlyphPropsFlags::empty(), false, false);
      self.buffer.replace_glyph(u32::from(glyph_id));
    }

    pub fn replace_glyph_inplace(&mut self, glyph_id: GlyphId) {
      self.set_glyph_class(glyph_id, GlyphPropsFlags::empty(), false, false);
      self.buffer.cur_mut(0).glyph_id = u32::from(glyph_id);
    }

    pub fn replace_glyph_with_ligature(
      &mut self,
      glyph_id: GlyphId,
      class_guess: GlyphPropsFlags,
    ) {
      self.set_glyph_class(glyph_id, class_guess, true, false);
      self.buffer.replace_glyph(u32::from(glyph_id));
    }

    pub fn output_glyph_for_component(
      &mut self,
      glyph_id: GlyphId,
      class_guess: GlyphPropsFlags,
    ) {
      self.set_glyph_class(glyph_id, class_guess, false, true);
      self.buffer.output_glyph(u32::from(glyph_id));
    }
  }
}

use OT::hb_ot_apply_context_t;

pub fn ligate_input(
  ctx: &mut hb_ot_apply_context_t,

  count: usize,

  match_end: usize,
  total_component_count: u8,
  lig_glyph: GlyphId,
) {
  ctx.buffer.merge_clusters(ctx.buffer.idx, match_end);

  let mut is_base_ligature = ctx.buffer.info[({
    let Ok(migrated_value) =
      console_core_number_conversion::wrapped_index(ctx.match_positions[0]);
    migrated_value
  })]
  .is_base_glyph();
  let mut is_mark_ligature = ctx.buffer.info[({
    let Ok(migrated_value) =
      console_core_number_conversion::wrapped_index(ctx.match_positions[0]);
    migrated_value
  })]
  .is_mark();
  for i in 1..count {
    match !ctx.buffer.info[({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped_index(ctx.match_positions[i]);
      migrated_value
    })]
    .is_mark()
    {
      true => {
        is_base_ligature = false;
        is_mark_ligature = false;
      }
      false => {}
    }
  }

  let is_ligature = !is_base_ligature && !is_mark_ligature;
  let class = match is_ligature {
    true => GlyphPropsFlags::LIGATURE,
    false => GlyphPropsFlags::empty(),
  };
  let lig_id = match is_ligature {
    true => ctx.buffer.allocate_lig_id(),
    false => 0,
  };
  let first = ctx.buffer.cur_mut(0);
  let mut last_lig_id = first.lig_id();
  let mut last_num_comps = first.lig_num_comps();
  let mut comps_so_far = last_num_comps;

  match is_ligature {
    true => {
      first.set_lig_props_for_ligature(lig_id, total_component_count);
      match first.general_category() == GeneralCategory::NON_SPACING_MARK {
        true => {
          first.set_general_category(GeneralCategory::OTHER_LETTER);
        }
        false => {}
      }
    }
    false => {}
  }

  ctx.replace_glyph_with_ligature(lig_glyph, class);

  for i in 1..count {
    while ctx.buffer.idx
      < ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped_index(ctx.match_positions[i]);
        migrated_value
      })
      && ctx.buffer.successful
    {
      match is_ligature {
        true => {
          let cur = ctx.buffer.cur_mut(0);
          let mut this_comp = cur.lig_comp();
          match this_comp == 0 {
            true => {
              this_comp = last_num_comps;
            }
            false => {}
          }

          debug_assert!(comps_so_far >= last_num_comps);
          let new_lig_comp = comps_so_far - last_num_comps + this_comp.min(last_num_comps);
          cur.set_lig_props_for_mark(lig_id, new_lig_comp);
        }
        false => {}
      }
      ctx.buffer.next_glyph();
    }

    let cur = ctx.buffer.cur(0);
    last_lig_id = cur.lig_id();
    last_num_comps = cur.lig_num_comps();
    comps_so_far += last_num_comps;

    ctx.buffer.idx += 1;
  }

  match !is_mark_ligature && last_lig_id != 0 {
    true => {
      for i in ctx.buffer.idx..ctx.buffer.len {
        let info = &mut ctx.buffer.info[i];
        match last_lig_id != info.lig_id() {
          true => {
            break;
          }
          false => {}
        }

        let this_comp = info.lig_comp();
        match this_comp == 0 {
          true => {
            break;
          }
          false => {}
        }

        debug_assert!(comps_so_far >= last_num_comps);
        let new_lig_comp = comps_so_far - last_num_comps + this_comp.min(last_num_comps);
        info.set_lig_props_for_mark(lig_id, new_lig_comp);
      }
    }
    false => {}
  }
}
