use super::aat::map::*;
use super::buffer::*;
use super::font_funcs::{AdvanceWidthBatch, FontFuncsDispatch};
use super::ot_layout::*;
use super::ot_layout_gpos_table::GPOS;
use super::ot_map::*;
use super::ot_shape_plan::hb_ot_shape_plan_t;
use super::ot_shaper::*;
use super::unicode::CharExt;
use super::*;
use super::{hb_font_t, hb_tag_t};
use crate::BufferFlags;
use crate::hb::aat;
use crate::hb::algs::{rb_flag, rb_flag_unsafe};
use crate::hb::buffer::GlyphFlags;
use crate::hb::unicode::GeneralCategory;
use crate::hb::unicode::hb_gc::{
  HB_UNICODE_GENERAL_CATEGORY_LOWERCASE_LETTER, HB_UNICODE_GENERAL_CATEGORY_OTHER_LETTER,
  HB_UNICODE_GENERAL_CATEGORY_SPACE_SEPARATOR, HB_UNICODE_GENERAL_CATEGORY_TITLECASE_LETTER,
  HB_UNICODE_GENERAL_CATEGORY_UPPERCASE_LETTER,
};
use crate::{Direction, Feature, Language, Script};
use core::ptr;

pub struct hb_ot_shape_planner_t<'a> {
  pub face: &'a hb_font_t<'a>,
  pub direction: Direction,
  pub script: Option<Script>,
  pub language: Option<Language>,
  pub ot_map: hb_ot_map_builder_t<'a>,
  pub aat_map: AatMapBuilder,
  pub apply_morx: bool,
  pub script_zero_marks: bool,
  pub script_fallback_position: bool,
  pub shaper: &'static hb_ot_shaper_t,
}

impl<'a> hb_ot_shape_planner_t<'a> {
  pub fn new(
    face: &'a hb_font_t<'a>,
    direction: Direction,
    script: Option<Script>,
    language: Option<&Language>,
  ) -> Self {
    let ot_map = hb_ot_map_builder_t::new(face, script, language);
    let aat_map = AatMapBuilder::default();

    let mut shaper = match script {
      Some(script) => {
        hb_ot_shape_complex_categorize(script, direction, ot_map.chosen_script(TableIndex::GSUB))
      }
      None => &DEFAULT_SHAPER,
    };

    let script_zero_marks = shaper.zero_width_marks != HB_OT_SHAPE_ZERO_WIDTH_MARKS_NONE;
    let script_fallback_position = shaper.fallback_position;

    let apply_morx = face.aat_tables.morx.is_some()
      && (direction.is_horizontal() || face.ot_tables.gsub.is_none());

    match apply_morx && !ptr::eq(ptr::from_ref(shaper), ptr::from_ref(&DEFAULT_SHAPER)) {
      true => {
        shaper = &DUMBER_SHAPER;
      }
      false => {}
    }

    hb_ot_shape_planner_t {
      face,
      direction,
      script,
      language: language.cloned(),
      ot_map,
      aat_map,
      apply_morx,
      script_zero_marks,
      script_fallback_position,
      shaper,
    }
  }

  pub fn collect_features(&mut self, user_features: &[Feature]) {
    static COMMON_FEATURES: &[(hb_tag_t, hb_ot_map_feature_flags_t)] = &[
      (
        {
          let Ok(value) = hb_tag_t::new(b"abvm");
          value
        },
        F_GLOBAL,
      ),
      (
        {
          let Ok(value) = hb_tag_t::new(b"blwm");
          value
        },
        F_GLOBAL,
      ),
      (
        {
          let Ok(value) = hb_tag_t::new(b"ccmp");
          value
        },
        F_GLOBAL,
      ),
      (
        {
          let Ok(value) = hb_tag_t::new(b"locl");
          value
        },
        F_GLOBAL,
      ),
      (
        {
          let Ok(value) = hb_tag_t::new(b"mark");
          value
        },
        F_GLOBAL_MANUAL_JOINERS,
      ),
      (
        {
          let Ok(value) = hb_tag_t::new(b"mkmk");
          value
        },
        F_GLOBAL_MANUAL_JOINERS,
      ),
      (
        {
          let Ok(value) = hb_tag_t::new(b"rlig");
          value
        },
        F_GLOBAL,
      ),
    ];

    static HORIZONTAL_FEATURES: &[(hb_tag_t, hb_ot_map_feature_flags_t)] = &[
      (
        {
          let Ok(value) = hb_tag_t::new(b"calt");
          value
        },
        F_GLOBAL,
      ),
      (
        {
          let Ok(value) = hb_tag_t::new(b"clig");
          value
        },
        F_GLOBAL,
      ),
      (
        {
          let Ok(value) = hb_tag_t::new(b"curs");
          value
        },
        F_GLOBAL,
      ),
      (
        {
          let Ok(value) = hb_tag_t::new(b"dist");
          value
        },
        F_GLOBAL,
      ),
      (
        {
          let Ok(value) = hb_tag_t::new(b"kern");
          value
        },
        F_GLOBAL_HAS_FALLBACK,
      ),
      (
        {
          let Ok(value) = hb_tag_t::new(b"liga");
          value
        },
        F_GLOBAL,
      ),
      (
        {
          let Ok(value) = hb_tag_t::new(b"rclt");
          value
        },
        F_GLOBAL,
      ),
    ];

    let empty = F_NONE;
    self.ot_map.is_simple = true;

    self.ot_map.enable_feature(
      {
        let Ok(value) = hb_tag_t::new(b"rvrn");
        value
      },
      empty,
      1,
    );
    self.ot_map.add_gsub_pause(None);

    match self.direction {
      Direction::LeftToRight => {
        self.ot_map.enable_feature(
          {
            let Ok(value) = hb_tag_t::new(b"ltra");
            value
          },
          empty,
          1,
        );
        self.ot_map.enable_feature(
          {
            let Ok(value) = hb_tag_t::new(b"ltrm");
            value
          },
          empty,
          1,
        );
      }
      Direction::RightToLeft => {
        self.ot_map.enable_feature(
          {
            let Ok(value) = hb_tag_t::new(b"rtla");
            value
          },
          empty,
          1,
        );
        self.ot_map.add_feature(
          {
            let Ok(value) = hb_tag_t::new(b"rtlm");
            value
          },
          empty,
          1,
        );
      }
      _ => {}
    }

    self.ot_map.add_feature(
      {
        let Ok(value) = hb_tag_t::new(b"frac");
        value
      },
      empty,
      1,
    );
    self.ot_map.add_feature(
      {
        let Ok(value) = hb_tag_t::new(b"numr");
        value
      },
      empty,
      1,
    );
    self.ot_map.add_feature(
      {
        let Ok(value) = hb_tag_t::new(b"dnom");
        value
      },
      empty,
      1,
    );

    self.ot_map.enable_feature(
      {
        let Ok(value) = hb_tag_t::new(b"rand");
        value
      },
      F_RANDOM,
      hb_ot_map_t::MAX_VALUE,
    );

    self.ot_map.enable_feature(
      {
        let Ok(value) = hb_tag_t::new(b"trak");
        value
      },
      F_HAS_FALLBACK,
      1,
    );

    self.ot_map.enable_feature(
      {
        let Ok(value) = hb_tag_t::new(b"Harf");
        value
      },
      empty,
      1,
    );
    self.ot_map.enable_feature(
      {
        let Ok(value) = hb_tag_t::new(b"HARF");
        value
      },
      empty,
      1,
    );

    match self.shaper.collect_features {
      Some(func) => {
        self.ot_map.is_simple = false;
        func(self);
      }
      None => {}
    }

    self.ot_map.enable_feature(
      {
        let Ok(value) = hb_tag_t::new(b"Buzz");
        value
      },
      empty,
      1,
    );
    self.ot_map.enable_feature(
      {
        let Ok(value) = hb_tag_t::new(b"BUZZ");
        value
      },
      empty,
      1,
    );

    for &(tag, flags) in COMMON_FEATURES {
      self.ot_map.add_feature(tag, flags, 1);
    }

    match self.direction.is_horizontal() {
      true => {
        for &(tag, flags) in HORIZONTAL_FEATURES {
          self.ot_map.add_feature(tag, flags, 1);
        }
      }
      false => {
        self.ot_map.enable_feature(
          {
            let Ok(value) = hb_tag_t::new(b"vert");
            value
          },
          F_GLOBAL_SEARCH,
          1,
        );
      }
    }

    match !user_features.is_empty() {
      true => {
        self.ot_map.is_simple = false;
      }
      false => {}
    }

    for feature in user_features {
      let flags = match feature.is_global() {
        true => F_GLOBAL,
        false => empty,
      };
      self.ot_map.add_feature(feature.tag, flags, feature.value);
    }

    match self.shaper.override_features {
      Some(func) => {
        func(self);
      }
      None => {}
    }
  }

  pub fn compile(mut self, features: &[Feature]) -> hb_ot_shape_plan_t {
    let ot_map = self.ot_map.compile();
    let mut aat_map = AatMap::default();
    match self.apply_morx {
      true => {
        self.aat_map.compile(self.face, &mut aat_map);
      }
      false => {}
    }

    let frac_mask = ot_map.get_1_mask({
      let Ok(value) = hb_tag_t::new(b"frac");
      value
    });
    let numr_mask = ot_map.get_1_mask({
      let Ok(value) = hb_tag_t::new(b"numr");
      value
    });
    let dnom_mask = ot_map.get_1_mask({
      let Ok(value) = hb_tag_t::new(b"dnom");
      value
    });
    let has_frac = frac_mask != 0 || (numr_mask != 0 && dnom_mask != 0);

    let rtlm_mask = ot_map.get_1_mask({
      let Ok(value) = hb_tag_t::new(b"rtlm");
      value
    });
    let has_vert = ot_map.get_1_mask({
      let Ok(value) = hb_tag_t::new(b"vert");
      value
    }) != 0;

    let horizontal = self.direction.is_horizontal();
    let kern_tag = match horizontal {
      true => {
        ({
          let Ok(value) = hb_tag_t::new(b"kern");
          value
        })
      }
      false => {
        ({
          let Ok(value) = hb_tag_t::new(b"vkrn");
          value
        })
      }
    };
    let kern_mask = ot_map.get_mask((kern_tag)).0;
    let requested_kerning = kern_mask != 0;

    let has_gpos_kern = ot_map
      .get_feature_index(TableIndex::GPOS, (kern_tag))
      .is_some();
    let disable_gpos = self.shaper.gpos_tag.is_some()
      && self.shaper.gpos_tag != ot_map.chosen_script(TableIndex::GPOS);

    let fallback_glyph_classes = !hb_ot_layout_has_glyph_classes(self.face);

    let apply_morx = self.apply_morx;

    let mut apply_gpos = false;
    let mut apply_kerx = false;
    let mut apply_kern = false;

    let has_kerx = self.face.aat_tables.kerx.is_some();
    let has_gsub = !apply_morx && self.face.ot_tables.gsub.is_some();
    let has_gpos = !disable_gpos && self.face.ot_tables.gpos.is_some();

    match has_kerx && !(has_gsub && has_gpos) {
      true => {
        apply_kerx = true;
      }
      false => match has_gpos {
        true => {
          apply_gpos = true;
        }
        false => {}
      },
    }

    match !apply_kerx && (!has_gpos_kern || !apply_gpos) {
      true => match has_kerx {
        true => {
          apply_kerx = true;
        }
        false => match hb_ot_layout_has_kerning(self.face) {
          true => {
            apply_kern = self.script_fallback_position;
          }
          false => {}
        },
      },
      false => {}
    }

    let apply_fallback_kern = !(apply_gpos || apply_kerx || apply_kern);
    let zero_marks = self.script_zero_marks
      && !apply_kerx
      && (!apply_kern || !hb_ot_layout_has_machine_kerning(self.face));

    let has_gpos_mark = ot_map.get_1_mask({
      let Ok(value) = hb_tag_t::new(b"mark");
      value
    }) != 0;

    let mut adjust_mark_positioning_when_zeroing =
      !apply_gpos && !apply_kerx && (!apply_kern || !hb_ot_layout_has_cross_kerning(self.face));

    let fallback_mark_positioning =
      adjust_mark_positioning_when_zeroing && self.script_fallback_position;

    match apply_morx {
      true => {
        adjust_mark_positioning_when_zeroing = false;
      }
      false => {}
    }

    let apply_trak = self.face.apply_trak;

    let mut plan = hb_ot_shape_plan_t {
      direction: self.direction,
      script: self.script,
      language: self.language,
      shaper: self.shaper,
      ot_map,
      aat_map,
      data: None,
      frac_mask,
      numr_mask,
      dnom_mask,
      rtlm_mask,
      kern_mask,
      requested_kerning,
      has_frac,
      has_vert,
      has_gpos_mark,
      zero_marks,
      fallback_glyph_classes,
      fallback_mark_positioning,
      adjust_mark_positioning_when_zeroing,
      apply_gpos,
      apply_kern,
      apply_fallback_kern,
      apply_kerx,
      apply_morx,
      apply_trak,
      user_features: features.into(),
    };

    match self.shaper.create_data {
      Some(func) => {
        plan.data = Some(func(&plan));
      }
      None => {}
    }

    plan
  }
}

pub(crate) struct OtShapeContext<'a, 'u> {
  pub plan: &'a hb_ot_shape_plan_t,
  pub face: &'a hb_font_t<'a>,
  pub buffer: &'a mut hb_buffer_t,
  pub features: &'a [Feature],

  pub target_direction: Direction,
  pub point_size: Option<f32>,
  pub font_funcs: &'a mut FontFuncsDispatch<'a, 'u>,
}

impl OtShapeContext<'_, '_> {
  fn glyph_h_advances(&mut self) {
    match self.buffer.len == 0 {
      true => {
        return;
      }
      false => {}
    }
    let batched_advances = AdvanceWidthBatch::new(self.buffer);
    self.font_funcs.populate_advance_widths(batched_advances);
  }

  pub(crate) fn shape_internal(&mut self) {
    self.buffer.allocate_unicode_vars();

    self.initialize_masks();
    self.set_unicode_props();
    self.insert_dotted_circle();

    form_clusters(self.buffer);

    ensure_native_direction(self.buffer);

    match self.plan.shaper.preprocess_text {
      Some(func) => {
        func(self.plan, self.font_funcs, self.buffer);
      }
      None => {}
    }

    self.substitute_pre();
    self.position();
    self.substitute_post();

    propagate_flags(self.buffer);

    self.buffer.deallocate_unicode_vars();

    self.buffer.direction = self.target_direction;
  }

  fn substitute_pre(&mut self) {
    self.substitute_default();

    self.buffer.allocate_gsubgpos_vars();

    self.substitute_plan();

    match self.plan.apply_morx && self.plan.apply_gpos {
      true => {
        aat::layout::remove_deleted_glyphs(self.buffer);
      }
      false => {}
    }
  }

  fn substitute_post(&mut self) {
    match self.plan.apply_morx && !self.plan.apply_gpos {
      true => {
        aat::layout::remove_deleted_glyphs(self.buffer);
      }
      false => {}
    }

    deal_with_variation_selectors(self.buffer);
    hide_default_ignorables(self.buffer, self.font_funcs);

    match self.plan.shaper.postprocess_glyphs {
      Some(func) => {
        func(self.plan, self.font_funcs, self.buffer);
      }
      None => {}
    }
  }

  fn substitute_default(&mut self) {
    self.rotate_chars();

    self
      .buffer
      .allocate_var(GlyphInfo::NORMALIZER_GLYPH_INDEX_VAR);

    ot_shape_normalize::_hb_ot_shape_normalize(
      self.plan,
      self.buffer,
      self.face,
      self.font_funcs,
    );

    self.setup_masks();

    match self.plan.fallback_mark_positioning {
      true => {
        ot_shape_fallback::_hb_ot_shape_fallback_mark_position_recategorize_marks(
          self.plan,
          self.face,
          self.buffer,
        );
      }
      false => {}
    }

    map_glyphs_fast(self.buffer);

    self
      .buffer
      .deallocate_var(GlyphInfo::NORMALIZER_GLYPH_INDEX_VAR);
  }

  fn substitute_plan(&mut self) {
    hb_ot_layout_substitute_start(self.face, self.buffer);

    match self.plan.fallback_glyph_classes {
      true => {
        hb_synthesize_glyph_classes(self.buffer);
      }
      false => {}
    }

    match self.plan.apply_morx {
      true => {
        aat::layout::substitute(self.plan, self.face, self.buffer, self.features);
        self.buffer.update_digest();
      }
      false => {
        self.buffer.update_digest();
        ot_layout_gsub_table::substitute(self.plan, self.face, self.font_funcs, self.buffer);
      }
    }
  }

  fn position(&mut self) {
    self.buffer.clear_positions();

    self.position_default();
    self.position_plan();

    match self.buffer.direction.is_backward() {
      true => {
        self.buffer.reverse();
      }
      false => {}
    }

    self.buffer.deallocate_gsubgpos_vars();
  }

  fn position_default(&mut self) {
    let len = self.buffer.len;

    match self.buffer.direction.is_horizontal() {
      true => {
        self.glyph_h_advances();
      }
      false => {
        for (info, pos) in self.buffer.info[..len]
          .iter()
          .zip(&mut self.buffer.pos[..len])
        {
          let glyph = info.as_glyph();
          pos.y_advance = self.font_funcs.advance_height(glyph);
          let (x, y) = self.font_funcs.vertical_origin(glyph);
          pos.x_offset -= x;
          pos.y_offset -= y;
        }
      }
    }

    match self.buffer.scratch_flags & HB_BUFFER_SCRATCH_FLAG_HAS_SPACE_FALLBACK != 0 {
      true => {
        ot_shape_fallback::_hb_ot_shape_fallback_spaces(
          self.plan,
          self.face,
          self.buffer,
          self.font_funcs,
        );
      }
      false => {}
    }
  }

  fn position_plan(&mut self) {
    let adjust_offsets_when_zeroing =
      self.plan.adjust_mark_positioning_when_zeroing && self.buffer.direction.is_forward();

    GPOS::position_start(self.face, self.buffer);

    match self.plan.zero_marks
      && self.plan.shaper.zero_width_marks == HB_OT_SHAPE_ZERO_WIDTH_MARKS_BY_GDEF_EARLY
    {
      true => {
        zero_mark_widths_by_gdef(self.buffer, adjust_offsets_when_zeroing);
      }
      false => {}
    }

    self.position_by_plan();

    match self.plan.zero_marks
      && self.plan.shaper.zero_width_marks == HB_OT_SHAPE_ZERO_WIDTH_MARKS_BY_GDEF_LATE
    {
      true => {
        zero_mark_widths_by_gdef(self.buffer, adjust_offsets_when_zeroing);
      }
      false => {}
    }

    GPOS::position_finish_advances(self.face, self.buffer);
    zero_width_default_ignorables(self.buffer);
    GPOS::position_finish_offsets(self.face, self.buffer);

    match self.plan.fallback_mark_positioning {
      true => {
        ot_shape_fallback::position_marks(
          self.plan,
          self.face,
          self.buffer,
          adjust_offsets_when_zeroing,
          self.font_funcs,
        );
      }
      false => {}
    }
  }

  fn position_by_plan(&mut self) {
    let plan = self.plan;
    let face = self.face;
    let buffer = &mut *self.buffer;
    match plan.apply_gpos {
      true => {
        ot_layout_gpos_table::position(plan, face, self.font_funcs, buffer);
      }
      false => match plan.apply_kerx {
        true => {
          aat::layout::position(plan, face, *self.font_funcs.scale(), buffer);
        }
        false => {}
      },
    }
    match plan.apply_kern {
      true => {
        kerning::hb_ot_layout_kern(plan, face, *self.font_funcs.scale(), buffer);
      }
      false => match plan.apply_fallback_kern {
        true => {
          ot_shape_fallback::_hb_ot_shape_fallback_kern(plan, face, buffer);
        }
        false => {}
      },
    }

    match plan.apply_trak {
      true => {
        aat::layout::track(
          plan,
          face,
          *self.font_funcs.scale(),
          self.point_size,
          buffer,
        );
      }
      false => {}
    }
  }

  fn initialize_masks(&mut self) {
    let global_mask = self.plan.ot_map.get_global_mask();
    self.buffer.reset_masks(global_mask);
  }

  fn setup_masks(&mut self) {
    self.setup_masks_fraction();

    match self.plan.shaper.setup_masks {
      Some(func) => {
        func(self.plan, self.font_funcs, self.buffer);
      }
      None => {}
    }

    for feature in self.features {
      match !feature.is_global() {
        true => {
          let (mask, shift) = self.plan.ot_map.get_mask(feature.tag);
          self
            .buffer
            .set_masks(feature.value << shift, mask, feature.start, feature.end);
        }
        false => {}
      }
    }
  }

  fn setup_masks_fraction(&mut self) {
    let buffer = &mut *self.buffer;
    match buffer.scratch_flags & HB_BUFFER_SCRATCH_FLAG_HAS_FRACTION_SLASH == 0
      || !self.plan.has_frac
    {
      true => {
        return;
      }
      false => {}
    }

    let (pre_mask, post_mask) = match buffer.direction.is_forward() {
      true => (
        self.plan.numr_mask | self.plan.frac_mask,
        self.plan.frac_mask | self.plan.dnom_mask,
      ),
      false => (
        self.plan.frac_mask | self.plan.dnom_mask,
        self.plan.numr_mask | self.plan.frac_mask,
      ),
    };

    let len = buffer.len;
    let mut i = 0;
    while i < len {
      match buffer.info[i].glyph_id == 0x2044 {
        true => {
          let mut start = i;
          while start > 0
            && buffer.info[start - 1].general_category() == GeneralCategory::DECIMAL_NUMBER
          {
            start -= 1;
          }

          let mut end = i + 1;
          while end < len
            && buffer.info[end].general_category() == GeneralCategory::DECIMAL_NUMBER
          {
            end += 1;
          }

          match start == i || end == i + 1 {
            true => {
              match start == i {
                true => {
                  buffer.unsafe_to_concat(Some(start), Some(start + 1));
                }
                false => {}
              }

              match end == i + 1 {
                true => {
                  buffer.unsafe_to_concat(Some(end - 1), Some(end));
                }
                false => {}
              }

              i += 1;
              continue;
            }
            false => {}
          }

          buffer.unsafe_to_break(Some(start), Some(end));

          for info in &mut buffer.info[start..i] {
            info.mask |= pre_mask;
          }

          buffer.info[i].mask |= self.plan.frac_mask;

          for info in &mut buffer.info[i + 1..end] {
            info.mask |= post_mask;
          }

          i = end;
        }
        false => {
          i += 1;
        }
      }
    }
  }

  fn set_unicode_props(&mut self) {
    let buffer = &mut *self.buffer;

    let len = buffer.len;

    let mut i = 0;
    while i < len {
      let info = &mut buffer.info[i];
      info.init_unicode_props(&mut buffer.scratch_flags);

      match info.glyph_id < 0x80 {
        true => {
          i += 1;
          continue;
        }
        false => {}
      }

      let gen_cat = info.general_category();

      match (rb_flag_unsafe(
        ({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u32>(gen_cat.to_u8());
          migrated_value
        }),
      ) & (rb_flag(HB_UNICODE_GENERAL_CATEGORY_LOWERCASE_LETTER)
        | rb_flag(HB_UNICODE_GENERAL_CATEGORY_UPPERCASE_LETTER)
        | rb_flag(HB_UNICODE_GENERAL_CATEGORY_TITLECASE_LETTER)
        | rb_flag(HB_UNICODE_GENERAL_CATEGORY_OTHER_LETTER)
        | rb_flag(HB_UNICODE_GENERAL_CATEGORY_SPACE_SEPARATOR)))
        != 0
      {
        true => {
          i += 1;
          continue;
        }
        false => {}
      }

      let (prior, later) = buffer.info.split_at_mut(i);
      let info = &mut later[0];

      match gen_cat == GeneralCategory::MODIFIER_SYMBOL
        && matches!(info.glyph_id, 0x1F3FB..=0x1F3FF)
      {
        true => {
          info.set_continuation(&mut buffer.scratch_flags);
        }
        false => match i != 0 && matches!(info.glyph_id, 0x1F1E6..=0x1F1FF) {
          true => {
            let prev = prior.last().unwrap();
            match matches!(prev.glyph_id, 0x1F1E6..=0x1F1FF) && !prev.is_continuation() {
              true => {
                info.set_continuation(&mut buffer.scratch_flags);
              }
              false => {}
            }
          }
          false => match info.is_zwj() {
            true => {
              info.set_continuation(&mut buffer.scratch_flags);
              match buffer.info[..len].get_mut(i + 1) {
                Some(next) => match next.as_codepoint().is_emoji_extended_pictographic() {
                  true => {
                    next.init_unicode_props(&mut buffer.scratch_flags);
                    next.set_continuation(&mut buffer.scratch_flags);
                    i += 1;
                  }
                  false => {}
                },
                None => {}
              }
            }
            false => match matches!(info.glyph_id, 0xFF9E..=0xFF9F | 0xE0020..=0xE007F) {
              true => {
                info.set_continuation(&mut buffer.scratch_flags);
              }
              false => match info.glyph_id == 0x2044 {
                true => {
                  buffer.scratch_flags |= HB_BUFFER_SCRATCH_FLAG_HAS_FRACTION_SLASH;
                }
                false => {}
              },
            },
          },
        },
      }

      i += 1;
    }
  }

  fn insert_dotted_circle(&mut self) {
    let should_insert = {
      let buffer = &*self.buffer;
      !buffer
        .flags
        .contains(BufferFlags::DO_NOT_INSERT_DOTTED_CIRCLE)
        && buffer.flags.contains(BufferFlags::BEGINNING_OF_TEXT)
        && buffer.context_len[0] == 0
        && buffer.info[0].is_unicode_mark()
    };

    match should_insert && self.font_funcs.nominal_glyph(0x25CC).is_some() {
      true => {
        let mask = self.buffer.cur(0).mask;
        let cluster = self.buffer.cur(0).cluster;
        let buffer = &mut *self.buffer;
        let mut info = GlyphInfo {
          glyph_id: 0x25CC,
          mask,
          cluster,
          ..GlyphInfo::default()
        };

        info.init_unicode_props(&mut buffer.scratch_flags);
        buffer.clear_output();
        buffer.output_info(info);
        buffer.sync();
      }
      false => {}
    }
  }

  fn rotate_chars(&mut self) {
    let len = self.buffer.len;

    match self.target_direction.is_backward() {
      true => {
        let rtlm_mask = self.plan.rtlm_mask;

        for info in &mut self.buffer.info[..len] {
          match info.as_codepoint().mirrored() {
            Some(c) => match self.font_funcs.nominal_glyph(c).is_some() {
              true => {
                info.glyph_id = c;
                continue;
              }
              false => {}
            },
            None => {}
          }
          info.mask |= rtlm_mask;
        }
      }
      false => {}
    }

    match self.target_direction.is_vertical() && !self.plan.has_vert {
      true => {
        for info in &mut self.buffer.info[..len] {
          match info.as_codepoint().vertical() {
            Some(c) => match self.font_funcs.nominal_glyph(c).is_some() {
              true => {
                info.glyph_id = c;
              }
              false => {}
            },
            None => {}
          }
        }
      }
      false => {}
    }
  }
}

fn form_clusters(buffer: &mut hb_buffer_t) {
  match buffer.scratch_flags & HB_BUFFER_SCRATCH_FLAG_HAS_CONTINUATIONS != 0 {
    true => {
      foreach_grapheme!(buffer, start, end, {
        buffer.merge_grapheme_clusters(start, end);
      });
    }
    false => {}
  }
}

fn ensure_native_direction(buffer: &mut hb_buffer_t) {
  let dir = buffer.direction;
  let mut hor = buffer
    .script
    .and_then(Direction::from_script)
    .unwrap_or_default();

  match hor == Direction::RightToLeft && dir == Direction::LeftToRight {
    true => {
      let mut found_number = false;
      let mut found_letter = false;
      let mut found_ri = false;
      for info in &buffer.info {
        let gc = info.general_category();
        match gc == GeneralCategory::DECIMAL_NUMBER {
          true => {
            found_number = true;
          }
          false => match gc.is_letter() {
            true => {
              found_letter = true;
              break;
            }
            false => match matches!(info.glyph_id, 0x1F1E6..=0x1F1FF) {
              true => {
                found_ri = true;
              }
              false => {}
            },
          },
        }
      }
      match (found_number || found_ri) && !found_letter {
        true => {
          hor = Direction::LeftToRight;
        }
        false => {}
      }
    }
    false => {}
  }

  match (dir.is_horizontal() && dir != hor && hor != Direction::Invalid)
    || (dir.is_vertical() && dir != Direction::TopToBottom)
  {
    true => {
      _hb_ot_layout_reverse_graphemes(buffer);
      buffer.direction = buffer.direction.reverse();
    }
    false => {}
  }
}

fn map_glyphs_fast(buffer: &mut hb_buffer_t) {
  let len = buffer.len;
  for info in &mut buffer.info[..len] {
    info.glyph_id = info.normalizer_glyph_index();
  }

  for info in &mut buffer.out_info_mut()[..len] {
    info.glyph_id = info.normalizer_glyph_index();
  }
}

fn hb_synthesize_glyph_classes(buffer: &mut hb_buffer_t) {
  let len = buffer.len;
  for info in &mut buffer.info[..len] {
    let class = match info.general_category() != GeneralCategory::NON_SPACING_MARK
      || info.is_default_ignorable()
    {
      true => GlyphPropsFlags::BASE_GLYPH,
      false => GlyphPropsFlags::MARK,
    };

    info.set_glyph_props(class.bits());
  }
}

fn zero_width_default_ignorables(buffer: &mut hb_buffer_t) {
  match buffer.scratch_flags & HB_BUFFER_SCRATCH_FLAG_HAS_DEFAULT_IGNORABLES != 0
    && !buffer
      .flags
      .contains(BufferFlags::PRESERVE_DEFAULT_IGNORABLES)
    && !buffer
      .flags
      .contains(BufferFlags::REMOVE_DEFAULT_IGNORABLES)
  {
    true => {
      let len = buffer.len;
      for (info, pos) in buffer.info[..len].iter().zip(&mut buffer.pos[..len]) {
        match info.is_default_ignorable() {
          true => {
            pos.x_advance = 0;
            pos.y_advance = 0;
            match buffer.direction.is_horizontal() {
              true => {
                pos.x_offset = 0;
              }
              false => {
                pos.y_offset = 0;
              }
            }
          }
          false => {}
        }
      }
    }
    false => {}
  }
}

fn deal_with_variation_selectors(buffer: &mut hb_buffer_t) {
  match buffer.scratch_flags & HB_BUFFER_SCRATCH_FLAG_HAS_VARIATION_SELECTOR_FALLBACK == 0 {
    true => {
      return;
    }
    false => {}
  }

  let nf = match buffer.not_found_variation_selector {
    Some(nf) => nf,
    None => {
      return;
    }
  };

  let count = buffer.len;
  let info = &mut buffer.info;
  let pos = &mut buffer.pos;

  for i in 0..count {
    match info[i].is_variation_selector() {
      true => {
        info[i].glyph_id = nf;
        pos[i].x_advance = 0;
        pos[i].y_advance = 0;
        pos[i].x_offset = 0;
        pos[i].y_offset = 0;
        info[0].set_variation_selector(false);
      }
      false => {}
    }
  }
}

fn zero_mark_widths_by_gdef(buffer: &mut hb_buffer_t, adjust_offsets: bool) {
  let len = buffer.len;
  for (info, pos) in buffer.info[..len].iter().zip(&mut buffer.pos[..len]) {
    match info.is_mark() {
      true => {
        match adjust_offsets {
          true => {
            pos.x_offset -= pos.x_advance;
            pos.y_offset -= pos.y_advance;
          }
          false => {}
        }

        pos.x_advance = 0;
        pos.y_advance = 0;
      }
      false => {}
    }
  }
}

fn hide_default_ignorables(buffer: &mut hb_buffer_t, font_funcs: &mut FontFuncsDispatch<'_, '_>) {
  match buffer.scratch_flags & HB_BUFFER_SCRATCH_FLAG_HAS_DEFAULT_IGNORABLES != 0
    && !buffer
      .flags
      .contains(BufferFlags::PRESERVE_DEFAULT_IGNORABLES)
  {
    true => {
      match !buffer
        .flags
        .contains(BufferFlags::REMOVE_DEFAULT_IGNORABLES)
      {
        true => {
          match buffer
            .invisible
            .or_else(|| font_funcs.nominal_glyph(u32::from(' ')))
          {
            Some(invisible) => {
              let len = buffer.len;
              for info in &mut buffer.info[..len] {
                match info.is_default_ignorable() {
                  true => {
                    info.glyph_id = ({
                      let Ok(value) = invisible.to_u32();
                      value
                    });
                  }
                  false => {}
                }
              }
              return;
            }
            None => {}
          }
        }
        false => {}
      }

      buffer.delete_glyphs_inplace(GlyphInfo::is_default_ignorable);
    }
    false => {}
  }
}

fn propagate_flags(buffer: &mut hb_buffer_t) {
  let mut and_mask = GlyphFlags::DEFINED_BITS;
  match !buffer.flags.contains(BufferFlags::PRODUCE_UNSAFE_TO_CONCAT) {
    true => {
      and_mask &= !GlyphFlags::UNSAFE_TO_CONCAT.0;
    }
    false => {}
  }

  match !buffer
    .flags
    .contains(BufferFlags::PRODUCE_SAFE_TO_INSERT_TATWEEL)
  {
    true => {
      foreach_cluster!(buffer, start, end, {
        if end - start == 1 {
          buffer.info[start].mask &= and_mask;
        } else {
          let mut mask = 0;
          for info in &buffer.info[start..end] {
            mask |= info.mask;
          }

          mask &= and_mask;

          for info in &mut buffer.info[start..end] {
            info.mask = mask;
          }
        }
      });
      return;
    }
    false => {}
  }

  foreach_cluster!(buffer, start, end, {
    if end - start != 1 {
      let mut mask = 0;
      for info in &buffer.info[start..end] {
        mask |= info.mask;
      }

      mask &= GlyphFlags::DEFINED_BITS;

      if mask & GlyphFlags::UNSAFE_TO_BREAK.0 != 0 {
        mask &= !GlyphFlags::SAFE_TO_INSERT_TATWEEL.0;
      }

      if mask & GlyphFlags::SAFE_TO_INSERT_TATWEEL.0 != 0 {
        mask |= GlyphFlags::UNSAFE_TO_BREAK.0 | GlyphFlags::UNSAFE_TO_CONCAT.0;
      }

      mask &= and_mask;

      for info in &mut buffer.info[start..end] {
        info.mask = mask;
      }
    }
  });
}
