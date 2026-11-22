//! OpenType layout.

use core::ops::{Index, IndexMut};

use super::buffer::*;
use super::font_funcs::FontFuncsDispatch;
use super::ot::lookup::LookupInfo;
use super::ot_layout_gsubgpos::OT;
use super::ot_shape_plan::hb_ot_shape_plan_t;
use super::unicode::hb_unicode_funcs_t;
use super::{GlyphInfo, hb_font_t};
use crate::hb::ot_layout_gsubgpos::OT::check_glyph_property;
use crate::hb::unicode::GeneralCategory;

impl GlyphInfo {
  declare_buffer_var!(u16, 1, 0, GLYPH_PROPS_VAR, glyph_props, set_glyph_props);
  declare_buffer_var!(u8, 1, 2, LIG_PROPS_VAR, lig_props, set_lig_props);
  declare_buffer_var!(u8, 1, 3, SYLLABLE_VAR, syllable, set_syllable);
  declare_buffer_var!(
    u16,
    2,
    0,
    UNICODE_PROPS_VAR,
    unicode_props,
    set_unicode_props
  );
}

impl hb_buffer_t {
  pub(crate) fn allocate_unicode_vars(&mut self) {
    self.allocate_var(GlyphInfo::UNICODE_PROPS_VAR);
  }
  pub(crate) fn deallocate_unicode_vars(&mut self) {
    self.deallocate_var(GlyphInfo::UNICODE_PROPS_VAR);
  }
  pub(crate) fn assert_unicode_vars(&mut self) {
    self.assert_var(GlyphInfo::UNICODE_PROPS_VAR);
  }

  pub(crate) fn allocate_gsubgpos_vars(&mut self) {
    self.allocate_var(GlyphInfo::LIG_PROPS_VAR);
    self.allocate_var(GlyphInfo::GLYPH_PROPS_VAR);
  }
  pub(crate) fn deallocate_gsubgpos_vars(&mut self) {
    self.deallocate_var(GlyphInfo::LIG_PROPS_VAR);
    self.deallocate_var(GlyphInfo::GLYPH_PROPS_VAR);
  }
  pub(crate) fn assert_gsubgpos_vars(&mut self) {
    self.assert_var(GlyphInfo::LIG_PROPS_VAR);
    self.assert_var(GlyphInfo::GLYPH_PROPS_VAR);
  }
}

pub const MAX_NESTING_LEVEL: usize = 64;
pub const MAX_CONTEXT_LENGTH: usize = 64;

pub fn hb_ot_layout_has_kerning(face: &hb_font_t) -> bool {
  face.aat_tables.kern.is_some()
}

pub fn hb_ot_layout_has_machine_kerning(face: &hb_font_t) -> bool {
  match face.aat_tables.kern {
    Some(ref kern) => kern
      .0
      .subtables()
      .filter_map(Result::ok)
      .any(|s| s.is_state_machine()),
    None => false,
  }
}

pub fn hb_ot_layout_has_cross_kerning(face: &hb_font_t) -> bool {
  match face.aat_tables.kern {
    Some(ref kern) => kern
      .0
      .subtables()
      .filter_map(Result::ok)
      .any(|s| s.is_cross_stream()),
    None => false,
  }
}

pub fn _hb_ot_layout_set_glyph_props(face: &hb_font_t, buffer: &mut hb_buffer_t) {
  buffer.assert_gsubgpos_vars();

  let len = buffer.len;
  for info in &mut buffer.info[..len] {
    info.set_glyph_props(face.ot_tables.glyph_props(info.as_glyph()));
    info.set_lig_props(0);
  }
}

pub fn hb_ot_layout_has_glyph_classes(face: &hb_font_t) -> bool {
  face.ot_tables.has_glyph_classes()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableIndex {
  GSUB = 0,
  GPOS = 1,
}

impl TableIndex {
  pub fn iter() -> impl Iterator<Item = TableIndex> {
    [Self::GSUB, Self::GPOS].iter().copied()
  }
}

impl<T> Index<TableIndex> for [T] {
  type Output = T;

  fn index(&self, table_index: TableIndex) -> &Self::Output {
    &self[table_index as usize]
  }
}

impl<T> IndexMut<TableIndex> for [T] {
  fn index_mut(&mut self, table_index: TableIndex) -> &mut Self::Output {
    &mut self[table_index as usize]
  }
}

/// A lookup-based layout table (GSUB or GPOS).
pub trait LayoutTable {
  /// The index of this table.
  const INDEX: TableIndex;

  /// Whether lookups in this table can be applied to the buffer in-place.
  const IN_PLACE: bool;

  /// Get the lookup at the specified index.
  fn get_lookup(&self, index: u16) -> Option<&LookupInfo>;
}

/// Called before substitution lookups are performed, to ensure that glyph
/// class and other properties are set on the glyphs in the buffer.
pub fn hb_ot_layout_substitute_start(face: &hb_font_t, buffer: &mut hb_buffer_t) {
  _hb_ot_layout_set_glyph_props(face, buffer);
}

/// Applies the lookups in the given GSUB or GPOS table.
pub fn apply_layout_table<T: LayoutTable>(
  plan: &hb_ot_shape_plan_t,
  face: &hb_font_t,
  font_funcs: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
  table: Option<&T>,
) {
  let mut ctx = OT::hb_ot_apply_context_t::new(T::INDEX, face, *font_funcs.scale(), buffer);

  for (stage_index, stage) in plan.ot_map.stages(T::INDEX).iter().enumerate() {
    match table {
      Some(table) => {
        for lookup_map in plan.ot_map.stage_lookups(T::INDEX, stage_index) {
          let lookup = match table.get_lookup(lookup_map.index) {
            Some(lookup) => lookup,
            None => {
              continue;
            }
          };

          match lookup.digest().may_intersect(&ctx.buffer.digest) {
            true => {
              ctx.lookup_index = lookup_map.index;
              ctx.set_lookup_mask(lookup_map.mask);
              ctx.auto_zwj = lookup_map.auto_zwj;
              ctx.auto_zwnj = lookup_map.auto_zwnj;

              ctx.random = lookup_map.random;
              ctx.per_syllable = lookup_map.per_syllable;

              apply_string::<T>(&mut ctx, lookup);
            }
            false => {}
          }
        }
      }
      None => {}
    }

    match stage.pause_func {
      Some(func) => match func(plan, font_funcs, ctx.buffer) {
        true => {
          ctx.buffer.update_digest();
        }
        false => {}
      },
      None => {}
    }
  }
}

fn apply_string<T: LayoutTable>(ctx: &mut OT::hb_ot_apply_context_t, lookup: &LookupInfo) {
  match ctx.buffer.is_empty() || ctx.lookup_mask() == 0 {
    true => {
      return;
    }
    false => {}
  }

  ctx.lookup_props = lookup.props();
  ctx.update_matchers();

  match !lookup.is_reverse() {
    true => {
      match !T::IN_PLACE {
        true => {
          ctx.buffer.clear_output();
        }
        false => {}
      }
      ctx.buffer.idx = 0;
      apply_forward(ctx, lookup);

      match !T::IN_PLACE {
        true => {
          ctx.buffer.sync();
        }
        false => {}
      }
    }
    false => {
      debug_assert!(!ctx.buffer.have_output);

      ctx.buffer.idx = ctx.buffer.len - 1;
      apply_backward(ctx, lookup);
    }
  }
}

fn apply_forward(ctx: &mut OT::hb_ot_apply_context_t, lookup: &LookupInfo) -> bool {
  let mut ret = false;
  let table_data = match ctx.face.ot_tables.table_data(ctx.table_index) {
    Some(table_data) => table_data,
    None => {
      return false;
    }
  };

  let use_hot_subtable_cache = lookup.cache_enter(ctx);

  while ctx.buffer.successful {
    let mut j = ctx.buffer.idx;
    while j < ctx.buffer.len && {
      let info = &ctx.buffer.info[j];
      !(lookup.digest.may_have(info.glyph_id)
        && (info.mask & ctx.lookup_mask()) != 0
        && check_glyph_property(ctx.face, info, ctx.lookup_props))
    } {
      j += 1;
    }
    match j > ctx.buffer.idx {
      true => {
        ctx.buffer.next_glyphs(j - ctx.buffer.idx);
      }
      false => {}
    }
    match ctx.buffer.idx >= ctx.buffer.len {
      true => {
        break;
      }
      false => {}
    }

    match lookup
      .apply(ctx, table_data, use_hot_subtable_cache)
      .is_some()
    {
      true => {
        ret = true;
      }
      false => {
        ctx.buffer.next_glyph();
      }
    }
  }

  match use_hot_subtable_cache {
    true => {
      lookup.cache_leave(ctx);
    }
    false => {}
  }

  ret
}

fn apply_backward(ctx: &mut OT::hb_ot_apply_context_t, lookup: &LookupInfo) -> bool {
  let mut ret = false;
  let table_data = match ctx.face.ot_tables.table_data(ctx.table_index) {
    Some(table_data) => table_data,
    None => {
      return false;
    }
  };
  loop {
    let cur = ctx.buffer.cur(0);
    ret |= lookup.digest.may_have(cur.glyph_id)
      && (cur.mask & ctx.lookup_mask()) != 0
      && check_glyph_property(ctx.face, cur, ctx.lookup_props)
      && lookup.apply(ctx, table_data, false).is_some();

    match ctx.buffer.idx == 0 {
      true => {
        break;
      }
      false => {}
    }

    ctx.buffer.idx -= 1;
  }
  ret
}

impl GlyphInfo {
  /// HB: _hb_glyph_info_set_general_category
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L260>
  #[doc(alias = "_hb_glyph_info_set_general_category")]
  #[inline]
  pub(crate) fn set_general_category(&mut self, gen_cat: GeneralCategory) {
    let gen_cat = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(gen_cat.0);
      migrated_value
    });
    let n = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(gen_cat);
      migrated_value
    }) | (self.unicode_props() & (0xFF & !UnicodeProps::GENERAL_CATEGORY.bits()));
    self.set_unicode_props(n);
  }

  /// HB: _hb_glyph_info_get_general_category
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L268>
  #[doc(alias = "_hb_glyph_info_get_general_category")]
  #[inline]
  pub(crate) fn general_category(&self) -> GeneralCategory {
    let n = self.unicode_props() & UnicodeProps::GENERAL_CATEGORY.bits();
    GeneralCategory(
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(n);
        migrated_value
      }),
    )
  }

  /// HB: _hb_glyph_info_is_unicode_mark
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L274>
  #[doc(alias = "_hb_glyph_info_is_unicode_mark")]
  #[inline]
  pub(crate) fn is_unicode_mark(&self) -> bool {
    self.general_category().is_mark()
  }

  /// HB: _hb_glyph_info_set_modified_combining_class
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L279>
  #[doc(alias = "_hb_glyph_info_set_modified_combining_class")]
  #[inline]
  pub(crate) fn set_modified_combining_class(&mut self, modified_class: u8) {
    match !self.is_unicode_mark() {
      true => {
        return;
      }
      false => {}
    }
    let n = (({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(modified_class);
      migrated_value
    }) << 8)
      | (self.unicode_props() & 0xFF);
    self.set_unicode_props(n);
  }

  /// HB: _hb_glyph_info_get_modified_combining_class
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L287>
  #[doc(alias = "_hb_glyph_info_get_modified_combining_class")]
  #[inline]
  pub(crate) fn modified_combining_class(&self) -> u8 {
    match self.is_unicode_mark() {
      true => {
        ({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u8>((self.unicode_props() >> 8));
          migrated_value
        })
      }
      false => 0,
    }
  }

  /// HB: _hb_glyph_info_is_unicode_space
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L294>
  #[doc(alias = "_hb_glyph_info_is_unicode_space")]
  #[inline]
  pub(crate) fn is_unicode_space(&self) -> bool {
    self.general_category() == GeneralCategory::SPACE_SEPARATOR
  }

  /// HB: _hb_glyph_info_set_unicode_space_fallback_type
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L300>
  #[doc(alias = "_hb_glyph_info_set_unicode_space_fallback_type")]
  #[inline]
  pub(crate) fn set_unicode_space_fallback_type(&mut self, s: hb_unicode_funcs_t::space_t) {
    match !self.is_unicode_space() {
      true => {
        return;
      }
      false => {}
    }
    let n = (({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(s);
      migrated_value
    }) << 8)
      | (self.unicode_props() & 0xFF);
    self.set_unicode_props(n);
  }

  /// HB: _hb_glyph_info_get_unicode_space_fallback_type
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L307>
  #[doc(alias = "_hb_glyph_info_get_unicode_space_fallback_type")]
  #[inline]
  pub(crate) fn unicode_space_fallback_type(&self) -> hb_unicode_funcs_t::space_t {
    match self.is_unicode_space() {
      true => {
        ({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u8>((self.unicode_props() >> 8));
          migrated_value
        })
      }
      false => hb_unicode_funcs_t::NOT_SPACE,
    }
  }

  /// HB: _hb_glyph_info_is_variation_selector
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L314>
  #[doc(alias = "_hb_glyph_info_is_variation_selector")]
  #[inline]
  pub(crate) fn is_variation_selector(&self) -> bool {
    let a = self.general_category() == GeneralCategory::FORMAT;
    let b = (self.unicode_props() & UnicodeProps::CF_VS.bits()) != 0;
    a && b
  }

  /// HB: _hb_glyph_info_set_variation_selector
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L321>
  #[doc(alias = "_hb_glyph_info_set_variation_selector")]
  #[inline]
  pub(crate) fn set_variation_selector(&mut self, customize: bool) {
    match customize {
      true => {
        self.set_general_category(GeneralCategory::FORMAT);
        self.set_unicode_props(self.unicode_props() | UnicodeProps::CF_VS.bits());
      }
      false => {
        self.set_general_category(GeneralCategory::NON_SPACING_MARK);
      }
    }
  }

  /// HB: _hb_glyph_info_is_default_ignorable
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L338>
  #[doc(alias = "_hb_glyph_info_is_default_ignorable")]
  #[inline]
  pub(crate) fn is_default_ignorable(&self) -> bool {
    let n = self.unicode_props() & UnicodeProps::IGNORABLE.bits();
    n != 0 && !self.substituted()
  }

  /// HB: _hb_glyph_info_set_default_ignorable
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L344>
  #[doc(alias = "_hb_glyph_info_set_default_ignorable")]
  #[inline]
  pub(crate) fn _set_default_ignorable(&mut self) {
    self.set_unicode_props(self.unicode_props() | UnicodeProps::IGNORABLE.bits());
  }

  /// HB: _hb_glyph_info_clear_default_ignorable
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L349>
  #[doc(alias = "_hb_glyph_info_clear_default_ignorable")]
  #[inline]
  pub(crate) fn clear_default_ignorable(&mut self) {
    let mut n = self.unicode_props();
    n &= !UnicodeProps::IGNORABLE.bits();
    self.set_unicode_props(n);
  }

  /// HB: _hb_glyph_info_is_hidden
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L354>
  #[doc(alias = "_hb_glyph_info_is_hidden")]
  #[inline]
  pub(crate) fn is_hidden(&self) -> bool {
    (self.unicode_props() & UnicodeProps::HIDDEN.bits()) != 0
  }

  /// HB: _hb_glyph_info_set_continuation
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L365>
  #[doc(alias = "_hb_glyph_info_set_continuation")]
  #[inline]
  pub(crate) fn set_continuation(&mut self, scratch_flags: &mut hb_buffer_scratch_flags_t) {
    *scratch_flags |= HB_BUFFER_SCRATCH_FLAG_HAS_CONTINUATIONS;
    let mut n = self.unicode_props();
    n |= UnicodeProps::CONTINUATION.bits();
    self.set_unicode_props(n);
  }

  /// HB: _hb_glyph_info_clear_continuation
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L370>
  #[doc(alias = "_hb_glyph_info_clear_continuation")]
  #[inline]
  pub(crate) fn clear_continuation(&mut self) {
    let mut n = self.unicode_props();
    n &= !UnicodeProps::CONTINUATION.bits();
    self.set_unicode_props(n);
  }

  /// HB: _hb_glyph_info_is_continuation
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L375>
  #[doc(alias = "_hb_glyph_info_is_continuation")]
  #[inline]
  pub(crate) fn is_continuation(&self) -> bool {
    self.unicode_props() & UnicodeProps::CONTINUATION.bits() != 0
  }
}

pub(crate) fn _hb_grapheme_group_func(_: &GlyphInfo, b: &GlyphInfo) -> bool {
  b.is_continuation()
}

pub fn _hb_ot_layout_reverse_graphemes(buffer: &mut hb_buffer_t) {
  buffer.reverse_groups(
    _hb_grapheme_group_func,
    buffer.cluster_level == HB_BUFFER_CLUSTER_LEVEL_MONOTONE_CHARACTERS,
  );
}

impl GlyphInfo {
  /// HB: _hb_glyph_info_is_unicode_format
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L398>
  #[doc(alias = "_hb_glyph_info_is_unicode_format")]
  #[inline]
  pub(crate) fn is_unicode_format(&self) -> bool {
    self.general_category() == GeneralCategory::FORMAT
  }

  /// HB: _hb_glyph_info_is_zwnj
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L404>
  #[doc(alias = "_hb_glyph_info_is_zwnj")]
  #[inline]
  pub(crate) fn is_zwnj(&self) -> bool {
    self.is_unicode_format() && (self.unicode_props() & UnicodeProps::CF_ZWNJ.bits() != 0)
  }

  /// HB: _hb_glyph_info_is_zwj
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L409>
  #[doc(alias = "_hb_glyph_info_is_zwnj")]
  #[inline]
  pub(crate) fn is_zwj(&self) -> bool {
    self.is_unicode_format() && (self.unicode_props() & UnicodeProps::CF_ZWJ.bits() != 0)
  }

  /// HB: _hb_glyph_info_is_aat_deleted
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L426>
  #[doc(alias = "_hb_glyph_info_is_aat_deleted")]
  #[inline]
  pub(crate) fn is_aat_deleted(&self) -> bool {
    self.is_unicode_format() && (self.unicode_props() & UnicodeProps::CF_AAT_DELETED.bits() != 0)
  }

  /// HB: _hb_glyph_info_set_aat_deleted
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L431>
  #[doc(alias = "_hb_glyph_info_set_aat_deleted")]
  #[inline]
  pub(crate) fn set_aat_deleted(&mut self) {
    self.set_general_category(GeneralCategory::FORMAT);
    self.set_unicode_props(
      self.unicode_props() | UnicodeProps::CF_AAT_DELETED.bits() | UnicodeProps::HIDDEN.bits(),
    );
  }

  const IS_LIG_BASE: u8 = 0x10;

  /// HB: _hb_glyph_info_set_lig_props_for_ligature
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L472>
  #[doc(alias = "_hb_glyph_info_set_lig_props_for_ligature")]
  #[inline]
  pub(crate) fn set_lig_props_for_ligature(&mut self, lig_id: u8, lig_num_comps: u8) {
    self.set_lig_props((lig_id << 5) | Self::IS_LIG_BASE | (lig_num_comps & 0x0F));
  }

  /// HB: _hb_glyph_info_set_lig_props_for_mark
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L480>
  #[doc(alias = "_hb_glyph_info_set_lig_props_for_mark")]
  #[inline]
  pub(crate) fn set_lig_props_for_mark(&mut self, lig_id: u8, lig_comp: u8) {
    self.set_lig_props((lig_id << 5) | (lig_comp & 0x0F));
  }

  /// HB: _hb_glyph_info_set_lig_props_for_component
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L488>
  #[doc(alias = "_hb_glyph_info_set_lig_props_for_component")]
  #[inline]
  pub(crate) fn set_lig_props_for_component(&mut self, comp: u8) {
    self.set_lig_props_for_mark(0, comp);
  }

  /// HB: _hb_glyph_info_get_lig_id
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L494>
  #[doc(alias = "_hb_glyph_info_get_lig_id")]
  #[inline]
  pub(crate) fn lig_id(&self) -> u8 {
    self.lig_props() >> 5
  }

  /// HB: _hb_glyph_info_ligated_internal
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L500>
  #[doc(alias = "_hb_glyph_info_ligated_internal")]
  #[inline]
  pub(crate) fn ligated_internal(&self) -> bool {
    self.lig_props() & Self::IS_LIG_BASE != 0
  }

  /// HB: _hb_glyph_info_get_lig_comp
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L506>
  #[doc(alias = "_hb_glyph_info_get_lig_comp")]
  #[inline]
  pub(crate) fn lig_comp(&self) -> u8 {
    match self.ligated_internal() {
      true => 0,
      false => self.lig_props() & 0x0F,
    }
  }

  /// HB: _hb_glyph_info_get_lig_num_comps
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L515>
  #[doc(alias = "_hb_glyph_info_get_lig_num_comps")]
  #[inline]
  pub(crate) fn lig_num_comps(&self) -> u8 {
    match self.glyph_props() & GlyphPropsFlags::LIGATURE.bits() != 0 && self.ligated_internal() {
      true => self.lig_props() & 0x0F,
      false => 1,
    }
  }

  /// HB: _hb_glyph_info_is_base_glyph
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L548>
  #[doc(alias = "_hb_glyph_info_is_base_glyph")]
  #[inline]
  pub(crate) fn is_base_glyph(&self) -> bool {
    self.glyph_props() & GlyphPropsFlags::BASE_GLYPH.bits() != 0
  }

  /// HB: _hb_glyph_info_is_ligature
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L554>
  #[doc(alias = "_hb_glyph_info_is_ligature")]
  #[inline]
  pub(crate) fn is_ligature(&self) -> bool {
    self.glyph_props() & GlyphPropsFlags::LIGATURE.bits() != 0
  }

  /// HB: _hb_glyph_info_is_mark
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L560>
  #[doc(alias = "_hb_glyph_info_is_mark")]
  #[inline]
  pub(crate) fn is_mark(&self) -> bool {
    self.glyph_props() & GlyphPropsFlags::MARK.bits() != 0
  }

  /// HB: _hb_glyph_info_substituted
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L566>
  #[doc(alias = "_hb_glyph_info_substituted")]
  #[inline]
  pub(crate) fn substituted(&self) -> bool {
    self.glyph_props() & GlyphPropsFlags::SUBSTITUTED.bits() != 0
  }

  /// HB: _hb_glyph_info_ligated
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L572>
  #[doc(alias = "_hb_glyph_info_ligated")]
  #[inline]
  pub(crate) fn ligated(&self) -> bool {
    self.glyph_props() & GlyphPropsFlags::LIGATED.bits() != 0
  }

  /// HB: _hb_glyph_info_multiplied
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L578>
  #[doc(alias = "_hb_glyph_info_multiplied")]
  #[inline]
  pub(crate) fn multiplied(&self) -> bool {
    self.glyph_props() & GlyphPropsFlags::MULTIPLIED.bits() != 0
  }

  /// HB: _hb_glyph_info_ligated_and_didnt_multiply
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L584>
  #[doc(alias = "_hb_glyph_info_ligated_and_didnt_multiply")]
  #[inline]
  pub(crate) fn ligated_and_didnt_multiply(&self) -> bool {
    self.ligated() && !self.multiplied()
  }

  /// HB: _hb_glyph_info_clear_ligated_and_multiplied
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L590>
  #[doc(alias = "_hb_glyph_info_clear_ligated_and_multiplied")]
  #[inline]
  pub(crate) fn clear_ligated_and_multiplied(&mut self) {
    let mut n = self.glyph_props();
    n &= !(GlyphPropsFlags::LIGATED | GlyphPropsFlags::MULTIPLIED).bits();
    self.set_glyph_props(n);
  }

  /// HB: _hb_glyph_info_clear_substituted
  ///
  /// See <https://github.com/harfbuzz/harfbuzz/blob/368598b5bd9c37a15cb0fd5438b8e617e254609b/src/hb-ot-layout.hh#L597>
  #[doc(alias = "_hb_glyph_info_clear_substituted")]
  #[inline]
  pub(crate) fn clear_substituted(&mut self) {
    let mut n = self.glyph_props();
    n &= !GlyphPropsFlags::SUBSTITUTED.bits();
    self.set_glyph_props(n);
  }
}

pub fn _hb_clear_substitution_flags(
  _: &hb_ot_shape_plan_t,
  _: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) -> bool {
  let len = buffer.len;
  for info in &mut buffer.info[..len] {
    info.clear_substituted();
  }

  false
}
