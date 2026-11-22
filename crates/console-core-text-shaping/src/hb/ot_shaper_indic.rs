use alloc::boxed::Box;
use core::cmp;
use core::ops::Range;

use read_fonts::types::GlyphId;

use super::algs::*;
use super::buffer::*;
use super::font_funcs::FontFuncsDispatch;
use super::ot_layout::*;
use super::ot_layout_gsubgpos::WouldApplyContext;
use super::ot_map::*;
use super::ot_shape::*;
use super::ot_shape_normalize::*;
use super::ot_shape_plan::hb_ot_shape_plan_t;
use super::ot_shaper::*;
use super::ot_shaper_syllabic::*;
use super::unicode::Codepoint;
use super::unicode::{CharExt, hb_gc};
use super::{GlyphInfo, Script, hb_font_t, hb_mask_t, hb_tag_t, script};

pub const INDIC_SHAPER: hb_ot_shaper_t = hb_ot_shaper_t {
  collect_features: Some(collect_features),
  override_features: Some(override_features),
  create_data: Some(|plan| Box::new(IndicShapePlan::new(plan))),
  preprocess_text: Some(preprocess_text),
  postprocess_glyphs: None,
  normalization_preference: HB_OT_SHAPE_NORMALIZATION_MODE_COMPOSED_DIACRITICS_NO_SHORT_CIRCUIT,
  decompose: Some(decompose),
  compose: Some(compose),
  setup_masks: Some(setup_masks),
  gpos_tag: None,
  reorder_marks: None,
  zero_width_marks: HB_OT_SHAPE_ZERO_WIDTH_MARKS_NONE,
  fallback_position: false,
};

impl GlyphInfo {
  declare_buffer_var_alias!(
    OT_SHAPER_VAR_U8_CATEGORY_VAR,
    u8,
    INDIC_CATEGORY_VAR,
    indic_category,
    set_indic_category
  );
  declare_buffer_var_alias!(
    OT_SHAPER_VAR_U8_AUXILIARY_VAR,
    u8,
    INDIC_POSITION_VAR,
    indic_position,
    set_indic_position
  );

  fn is_one_of(&self, flags: u32) -> bool {
    match self.ligated() {
      true => {
        return false;
      }
      false => {}
    }

    rb_flag_unsafe(
      ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u32>(self.indic_category());
        migrated_value
      }),
    ) & flags
      != 0
  }

  fn is_joiner(&self) -> bool {
    self.is_one_of(JOINER_FLAGS)
  }

  pub(crate) fn is_consonant(&self) -> bool {
    self.is_one_of(CONSONANT_FLAGS_INDIC)
  }

  fn is_halant(&self) -> bool {
    self.is_one_of(rb_flag(
      ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u32>(ot_category_t::OT_H);
        migrated_value
      }),
    ))
  }

  fn set_indic_properties(&mut self) {
    let u = self.glyph_id;
    let (cat, pos) = crate::hb::ot_shaper_indic_table::get_categories(u);

    self.set_indic_category(cat);
    self.set_indic_position(pos);
  }
}

pub type Category = u8;

#[allow(dead_code)]
pub mod ot_category_t {
  pub const OT_X: u8 = 0;
  pub const OT_C: u8 = 1;
  pub const OT_V: u8 = 2;
  pub const OT_N: u8 = 3;
  pub const OT_H: u8 = 4;
  pub const OT_ZWNJ: u8 = 5;
  pub const OT_ZWJ: u8 = 6;
  pub const OT_M: u8 = 7;
  pub const OT_SM: u8 = 8;
  pub const OT_A: u8 = 9;
  pub const OT_VD: u8 = OT_A;
  pub const OT_PLACEHOLDER: u8 = 10;
  pub const OT_GB: u8 = OT_PLACEHOLDER;
  pub const OT_DOTTEDCIRCLE: u8 = 11;
  pub const OT_RS: u8 = 12;
  pub const OT_MPst: u8 = 13;
  pub const OT_Repha: u8 = 14;
  pub const OT_Ra: u8 = 15;
  pub const OT_CM: u8 = 16;
  pub const OT_Symbol: u8 = 17;
  pub const OT_CS: u8 = 18;

  pub const OT_VAbv: u8 = 20;
  pub const OT_VBlw: u8 = 21;
  pub const OT_VPre: u8 = 22;
  pub const OT_VPst: u8 = 23;

  pub const OT_Robatic: u8 = 25;
  pub const OT_Xgroup: u8 = 26;
  pub const OT_Ygroup: u8 = 27;

  pub const OT_As: u8 = 32;
  pub const OT_MH: u8 = 35;
  pub const OT_MR: u8 = 36;
  pub const OT_MW: u8 = 37;
  pub const OT_MY: u8 = 38;
  pub const OT_PT: u8 = 39;
  pub const OT_VS: u8 = 40;
  pub const OT_ML: u8 = 41;

  pub const OT_SMPst: u8 = 57;

  pub const IV: u8 = 2;
}

pub mod ot_position_t {
  pub const POS_START: u8 = 0;

  pub const POS_RA_TO_BECOME_REPH: u8 = 1;
  pub const POS_PRE_M: u8 = 2;
  pub const POS_PRE_C: u8 = 3;

  pub const POS_BASE_C: u8 = 4;
  pub const POS_AFTER_MAIN: u8 = 5;

  pub const POS_ABOVE_C: u8 = 6;

  pub const POS_BEFORE_SUB: u8 = 7;
  pub const POS_BELOW_C: u8 = 8;
  pub const POS_AFTER_SUB: u8 = 9;

  pub const POS_BEFORE_POST: u8 = 10;
  pub const POS_POST_C: u8 = 11;
  pub const POS_AFTER_POST: u8 = 12;

  pub const POS_SMVD: u8 = 13;

  pub const POS_END: u8 = 14;
}

const INDIC_FEATURES: &[(hb_tag_t, hb_ot_map_feature_flags_t)] = &[
  (
    ({
      let Ok(value) = hb_tag_t::new(b"nukt");
      value
    }),
    F_GLOBAL_MANUAL_JOINERS | F_PER_SYLLABLE,
  ),
  (
    ({
      let Ok(value) = hb_tag_t::new(b"akhn");
      value
    }),
    F_GLOBAL_MANUAL_JOINERS | F_PER_SYLLABLE,
  ),
  (
    {
      let Ok(value) = hb_tag_t::new(b"rphf");
      value
    },
    F_MANUAL_JOINERS | F_PER_SYLLABLE,
  ),
  (
    ({
      let Ok(value) = hb_tag_t::new(b"rkrf");
      value
    }),
    F_GLOBAL_MANUAL_JOINERS | F_PER_SYLLABLE,
  ),
  (
    {
      let Ok(value) = hb_tag_t::new(b"pref");
      value
    },
    F_MANUAL_JOINERS | F_PER_SYLLABLE,
  ),
  (
    {
      let Ok(value) = hb_tag_t::new(b"blwf");
      value
    },
    F_MANUAL_JOINERS | F_PER_SYLLABLE,
  ),
  (
    {
      let Ok(value) = hb_tag_t::new(b"abvf");
      value
    },
    F_MANUAL_JOINERS | F_PER_SYLLABLE,
  ),
  (
    {
      let Ok(value) = hb_tag_t::new(b"half");
      value
    },
    F_MANUAL_JOINERS | F_PER_SYLLABLE,
  ),
  (
    {
      let Ok(value) = hb_tag_t::new(b"pstf");
      value
    },
    F_MANUAL_JOINERS | F_PER_SYLLABLE,
  ),
  (
    ({
      let Ok(value) = hb_tag_t::new(b"vatu");
      value
    }),
    F_GLOBAL_MANUAL_JOINERS | F_PER_SYLLABLE,
  ),
  (
    ({
      let Ok(value) = hb_tag_t::new(b"cjct");
      value
    }),
    F_GLOBAL_MANUAL_JOINERS | F_PER_SYLLABLE,
  ),
  (
    {
      let Ok(value) = hb_tag_t::new(b"init");
      value
    },
    F_MANUAL_JOINERS | F_PER_SYLLABLE,
  ),
  (
    ({
      let Ok(value) = hb_tag_t::new(b"pres");
      value
    }),
    F_GLOBAL_MANUAL_JOINERS | F_PER_SYLLABLE,
  ),
  (
    ({
      let Ok(value) = hb_tag_t::new(b"abvs");
      value
    }),
    F_GLOBAL_MANUAL_JOINERS | F_PER_SYLLABLE,
  ),
  (
    ({
      let Ok(value) = hb_tag_t::new(b"blws");
      value
    }),
    F_GLOBAL_MANUAL_JOINERS | F_PER_SYLLABLE,
  ),
  (
    ({
      let Ok(value) = hb_tag_t::new(b"psts");
      value
    }),
    F_GLOBAL_MANUAL_JOINERS | F_PER_SYLLABLE,
  ),
  (
    ({
      let Ok(value) = hb_tag_t::new(b"haln");
      value
    }),
    F_GLOBAL_MANUAL_JOINERS | F_PER_SYLLABLE,
  ),
];

#[allow(dead_code)]
mod indic_feature {
  pub const NUKT: usize = 0;
  pub const AKHN: usize = 1;
  pub const RPHF: usize = 2;
  pub const RKRF: usize = 3;
  pub const PREF: usize = 4;
  pub const BLWF: usize = 5;
  pub const ABVF: usize = 6;
  pub const HALF: usize = 7;
  pub const PSTF: usize = 8;
  pub const VATU: usize = 9;
  pub const CJCT: usize = 10;
  pub const INIT: usize = 11;
  pub const PRES: usize = 12;
  pub const ABVS: usize = 13;
  pub const BLWS: usize = 14;
  pub const PSTS: usize = 15;
  pub const HALN: usize = 16;
}

pub(crate) const fn category_flag(c: Category) -> u32 {
  rb_flag(c as u32)
}

const CONSONANT_FLAGS_INDIC: u32 = category_flag(ot_category_t::OT_C)
  | category_flag(ot_category_t::OT_CS)
  | category_flag(ot_category_t::OT_Ra)
  | category_flag(ot_category_t::OT_CM)
  | category_flag(ot_category_t::OT_V)
  | category_flag(ot_category_t::OT_PLACEHOLDER)
  | category_flag(ot_category_t::OT_DOTTEDCIRCLE);

const JOINER_FLAGS: u32 =
  category_flag(ot_category_t::OT_ZWJ) | category_flag(ot_category_t::OT_ZWNJ);

#[derive(Clone, Copy, PartialEq)]
enum RephPosition {
  AfterMain = ot_position_t::POS_AFTER_MAIN as isize,
  BeforeSub = ot_position_t::POS_BEFORE_SUB as isize,
  AfterSub = ot_position_t::POS_AFTER_SUB as isize,
  BeforePost = ot_position_t::POS_BEFORE_POST as isize,
  AfterPost = ot_position_t::POS_AFTER_POST as isize,
}

#[derive(Clone, Copy, PartialEq)]
enum RephMode {
  /// Reph formed out of initial Ra,H sequence.
  Implicit,
  /// Reph formed out of initial Ra,H,ZWJ sequence.
  Explicit,
  /// Encoded Repha character, needs reordering.
  LogRepha,
}

#[derive(Clone, Copy, PartialEq)]
enum BlwfMode {
  /// Below-forms feature applied to pre-base and post-base.
  PreAndPost,
  /// Below-forms feature applied to post-base only.
  PostOnly,
}

#[derive(Clone, Copy)]
struct IndicConfig {
  script: Option<Script>,
  has_old_spec: bool,
  virama: u32,
  reph_pos: RephPosition,
  reph_mode: RephMode,
  blwf_mode: BlwfMode,
}

impl IndicConfig {
  const fn new(
    script: Option<Script>,
    has_old_spec: bool,
    virama: u32,
    reph_pos: RephPosition,
    reph_mode: RephMode,
    blwf_mode: BlwfMode,
  ) -> Self {
    IndicConfig {
      script,
      has_old_spec,
      virama,
      reph_pos,
      reph_mode,
      blwf_mode,
    }
  }
}

static INDIC_CONFIGS: &[IndicConfig] = &[
  IndicConfig::new(
    None,
    false,
    0,
    RephPosition::BeforePost,
    RephMode::Implicit,
    BlwfMode::PreAndPost,
  ),
  IndicConfig::new(
    Some(script::DEVANAGARI),
    true,
    0x094D,
    RephPosition::BeforePost,
    RephMode::Implicit,
    BlwfMode::PreAndPost,
  ),
  IndicConfig::new(
    Some(script::BENGALI),
    true,
    0x09CD,
    RephPosition::AfterSub,
    RephMode::Implicit,
    BlwfMode::PreAndPost,
  ),
  IndicConfig::new(
    Some(script::GURMUKHI),
    true,
    0x0A4D,
    RephPosition::BeforeSub,
    RephMode::Implicit,
    BlwfMode::PreAndPost,
  ),
  IndicConfig::new(
    Some(script::GUJARATI),
    true,
    0x0ACD,
    RephPosition::BeforePost,
    RephMode::Implicit,
    BlwfMode::PreAndPost,
  ),
  IndicConfig::new(
    Some(script::ORIYA),
    true,
    0x0B4D,
    RephPosition::AfterMain,
    RephMode::Implicit,
    BlwfMode::PreAndPost,
  ),
  IndicConfig::new(
    Some(script::TAMIL),
    true,
    0x0BCD,
    RephPosition::AfterPost,
    RephMode::Implicit,
    BlwfMode::PreAndPost,
  ),
  IndicConfig::new(
    Some(script::TELUGU),
    true,
    0x0C4D,
    RephPosition::AfterPost,
    RephMode::Explicit,
    BlwfMode::PostOnly,
  ),
  IndicConfig::new(
    Some(script::KANNADA),
    true,
    0x0CCD,
    RephPosition::AfterPost,
    RephMode::Implicit,
    BlwfMode::PostOnly,
  ),
  IndicConfig::new(
    Some(script::MALAYALAM),
    true,
    0x0D4D,
    RephPosition::AfterMain,
    RephMode::LogRepha,
    BlwfMode::PreAndPost,
  ),
  IndicConfig::new(
    Some(script::SINHALA),
    false,
    0x0DCA,
    RephPosition::AfterPost,
    RephMode::Explicit,
    BlwfMode::PreAndPost,
  ),
];

struct IndicWouldSubstituteFeature {
  lookups: Range<usize>,
  zero_context: bool,
}

impl IndicWouldSubstituteFeature {
  pub fn new(map: &hb_ot_map_t, feature_tag: hb_tag_t, zero_context: bool) -> Self {
    IndicWouldSubstituteFeature {
      lookups: match map.get_feature_stage(TableIndex::GSUB, feature_tag) {
        Some(stage) => map.stage_lookup_range(TableIndex::GSUB, stage),
        None => 0..0,
      },
      zero_context,
    }
  }

  pub fn would_substitute(
    &self,
    map: &hb_ot_map_t,
    face: &hb_font_t,
    glyphs: &[GlyphId],
  ) -> bool {
    for index in self.lookups.clone() {
      let lookup = map.lookup(TableIndex::GSUB, index);
      let ctx = WouldApplyContext {
        glyphs,
        zero_context: self.zero_context,
      };
      match face
        .ot_tables
        .gsub
        .as_ref()
        .and_then(|table| table.get_lookup(lookup.index))
        .is_some_and(|lookup| lookup.would_apply(face, &ctx) == Some(true))
      {
        true => {
          return true;
        }
        false => {}
      }
    }

    false
  }
}

struct IndicShapePlan {
  config: IndicConfig,
  is_old_spec: bool,

  rphf: IndicWouldSubstituteFeature,
  pref: IndicWouldSubstituteFeature,
  blwf: IndicWouldSubstituteFeature,
  pstf: IndicWouldSubstituteFeature,
  vatu: IndicWouldSubstituteFeature,
  mask_array: [hb_mask_t; INDIC_FEATURES.len()],
}

impl IndicShapePlan {
  fn new(plan: &hb_ot_shape_plan_t) -> Self {
    let script = plan.script;
    let config = match INDIC_CONFIGS.iter().skip(1).find(|c| c.script == script) {
      Some(c) => *c,
      None => INDIC_CONFIGS[0],
    };

    let is_old_spec = config.has_old_spec
      && plan
        .ot_map
        .chosen_script(TableIndex::GSUB)
        .is_none_or(|tag| {
          ({
            let Ok(value) = tag.to_be_bytes();
            value
          })[3]
            != b'2'
        });

    let zero_context = is_old_spec && script != Some(script::MALAYALAM);

    let mut mask_array = [0; INDIC_FEATURES.len()];
    for (i, feature) in INDIC_FEATURES.iter().enumerate() {
      mask_array[i] = match feature.1 & F_GLOBAL != 0 {
        true => 0,
        false => plan.ot_map.get_1_mask(feature.0),
      }
    }

    IndicShapePlan {
      config,
      is_old_spec,

      rphf: IndicWouldSubstituteFeature::new(
        &plan.ot_map,
        ({
          let Ok(value) = hb_tag_t::new(b"rphf");
          value
        }),
        zero_context,
      ),
      pref: IndicWouldSubstituteFeature::new(
        &plan.ot_map,
        ({
          let Ok(value) = hb_tag_t::new(b"pref");
          value
        }),
        zero_context,
      ),
      blwf: IndicWouldSubstituteFeature::new(
        &plan.ot_map,
        ({
          let Ok(value) = hb_tag_t::new(b"blwf");
          value
        }),
        zero_context,
      ),
      pstf: IndicWouldSubstituteFeature::new(
        &plan.ot_map,
        ({
          let Ok(value) = hb_tag_t::new(b"pstf");
          value
        }),
        zero_context,
      ),
      vatu: IndicWouldSubstituteFeature::new(
        &plan.ot_map,
        ({
          let Ok(value) = hb_tag_t::new(b"vatu");
          value
        }),
        zero_context,
      ),
      mask_array,
    }
  }
}

fn collect_features(planner: &mut hb_ot_shape_planner_t) {
  planner.ot_map.add_gsub_pause(Some(setup_syllables));

  planner.ot_map.enable_feature(
    {
      let Ok(value) = hb_tag_t::new(b"locl");
      value
    },
    F_PER_SYLLABLE,
    1,
  );

  planner.ot_map.enable_feature(
    {
      let Ok(value) = hb_tag_t::new(b"ccmp");
      value
    },
    F_PER_SYLLABLE,
    1,
  );

  planner.ot_map.add_gsub_pause(Some(initial_reordering));

  for feature in INDIC_FEATURES.iter().take(11) {
    planner.ot_map.add_feature(feature.0, feature.1, 1);
    planner.ot_map.add_gsub_pause(None);
  }

  planner.ot_map.add_gsub_pause(Some(final_reordering));

  for feature in INDIC_FEATURES.iter().skip(11) {
    planner.ot_map.add_feature(feature.0, feature.1, 1);
  }
}

fn override_features(planner: &mut hb_ot_shape_planner_t) {
  planner.ot_map.disable_feature({
    let Ok(value) = hb_tag_t::new(b"liga");
    value
  });
  planner.ot_map.add_gsub_pause(Some(syllabic_clear_var));
}

fn preprocess_text(_: &hb_ot_shape_plan_t, _: &mut FontFuncsDispatch, buffer: &mut hb_buffer_t) {
  super::ot_shaper_vowel_constraints::preprocess_text_vowel_constraints(buffer);
}

fn decompose(
  _: &hb_ot_shape_normalize_context_t,
  ab: Codepoint,
) -> Option<(Codepoint, Codepoint)> {
  match ab {
    0x0931 | 0x09DC | 0x09DD | 0x0B94 => return None,
    _ => {}
  }

  crate::hb::unicode::decompose(ab)
}

fn compose(_: &hb_ot_shape_normalize_context_t, a: Codepoint, b: Codepoint) -> Option<Codepoint> {
  match a.general_category().is_mark() {
    true => {
      return None;
    }
    false => {}
  }

  match a == 0x09AF && b == 0x09BC {
    true => {
      return Some(0x09DF);
    }
    false => {}
  }

  crate::hb::unicode::compose(a, b)
}

fn setup_masks(_: &hb_ot_shape_plan_t, _: &mut FontFuncsDispatch, buffer: &mut hb_buffer_t) {
  buffer.allocate_var(GlyphInfo::INDIC_CATEGORY_VAR);
  buffer.allocate_var(GlyphInfo::INDIC_POSITION_VAR);

  for info in buffer.info_slice_mut() {
    info.set_indic_properties();
  }
}

fn setup_syllables(
  _: &hb_ot_shape_plan_t,
  _: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) -> bool {
  buffer.allocate_var(GlyphInfo::SYLLABLE_VAR);

  super::ot_shaper_indic_machine::find_syllables_indic(buffer);

  let mut start = 0;
  let mut end = buffer.next_syllable(0);
  while start < buffer.len {
    buffer.unsafe_to_break(Some(start), Some(end));
    start = end;
    end = buffer.next_syllable(start);
  }

  false
}

fn initial_reordering(
  plan: &hb_ot_shape_plan_t,
  font_funcs: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) -> bool {
  use super::ot_shaper_indic_machine::SyllableType;

  let mut ret = false;

  let indic_plan = plan.data::<IndicShapePlan>();

  update_consonant_positions(plan, indic_plan, font_funcs, buffer);
  match insert_dotted_circles(
    font_funcs,
    buffer,
    SyllableType::BrokenCluster as u8,
    ot_category_t::OT_DOTTEDCIRCLE,
    Some(ot_category_t::OT_Repha),
    Some(ot_position_t::POS_END),
  ) {
    true => {
      ret = true;
    }
    false => {}
  }

  let mut start = 0;
  let mut end = buffer.next_syllable(0);
  while start < buffer.len {
    initial_reordering_syllable(plan, indic_plan, font_funcs, start, end, buffer);
    start = end;
    end = buffer.next_syllable(start);
  }

  ret
}

fn update_consonant_positions(
  plan: &hb_ot_shape_plan_t,
  indic_plan: &IndicShapePlan,
  font_funcs: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) {
  let mut virama_glyph = None;
  match indic_plan.config.virama != 0 {
    true => {
      virama_glyph = font_funcs.nominal_glyph(indic_plan.config.virama);
    }
    false => {}
  }

  match virama_glyph {
    Some(virama) => {
      let face = font_funcs.font();
      for info in buffer.info_slice_mut() {
        match info.indic_position() == ot_position_t::POS_BASE_C {
          true => {
            let consonant = info.as_glyph();
            info.set_indic_position(consonant_position_from_face(
              plan, indic_plan, face, consonant, virama,
            ));
          }
          false => {}
        }
      }
    }
    None => {}
  }
}

fn consonant_position_from_face(
  plan: &hb_ot_shape_plan_t,
  indic_plan: &IndicShapePlan,
  face: &hb_font_t,
  consonant: GlyphId,
  virama: GlyphId,
) -> u8 {
  match indic_plan
    .blwf
    .would_substitute(&plan.ot_map, face, &[virama, consonant])
    || indic_plan
      .blwf
      .would_substitute(&plan.ot_map, face, &[consonant, virama])
    || indic_plan
      .vatu
      .would_substitute(&plan.ot_map, face, &[virama, consonant])
    || indic_plan
      .vatu
      .would_substitute(&plan.ot_map, face, &[consonant, virama])
  {
    true => {
      return ot_position_t::POS_BELOW_C;
    }
    false => {}
  }

  match indic_plan
    .pstf
    .would_substitute(&plan.ot_map, face, &[virama, consonant])
    || indic_plan
      .pstf
      .would_substitute(&plan.ot_map, face, &[consonant, virama])
  {
    true => {
      return ot_position_t::POS_POST_C;
    }
    false => {}
  }

  match indic_plan
    .pref
    .would_substitute(&plan.ot_map, face, &[virama, consonant])
    || indic_plan
      .pref
      .would_substitute(&plan.ot_map, face, &[consonant, virama])
  {
    true => {
      return ot_position_t::POS_POST_C;
    }
    false => {}
  }

  ot_position_t::POS_BASE_C
}

fn initial_reordering_syllable(
  plan: &hb_ot_shape_plan_t,
  indic_plan: &IndicShapePlan,
  font_funcs: &mut FontFuncsDispatch,
  start: usize,
  end: usize,
  buffer: &mut hb_buffer_t,
) {
  use super::ot_shaper_indic_machine::SyllableType;

  let syllable_type = match buffer.info[start].syllable() & 0x0F {
    0 => SyllableType::ConsonantSyllable,
    1 => SyllableType::VowelSyllable,
    2 => SyllableType::StandaloneCluster,
    3 => SyllableType::SymbolCluster,
    4 => SyllableType::BrokenCluster,
    5 => SyllableType::NonIndicCluster,
    _ => unreachable!(),
  };

  match syllable_type {
    SyllableType::VowelSyllable | SyllableType::ConsonantSyllable => {
      initial_reordering_consonant_syllable(plan, indic_plan, font_funcs, start, end, buffer);
    }

    SyllableType::BrokenCluster | SyllableType::StandaloneCluster => {
      initial_reordering_standalone_cluster(plan, indic_plan, font_funcs, start, end, buffer);
    }
    SyllableType::SymbolCluster | SyllableType::NonIndicCluster => {}
  }
}

fn initial_reordering_consonant_syllable(
  plan: &hb_ot_shape_plan_t,
  indic_plan: &IndicShapePlan,
  font_funcs: &mut FontFuncsDispatch,
  start: usize,
  end: usize,
  buffer: &mut hb_buffer_t,
) {
  let face = font_funcs.font();

  match buffer.script == Some(script::KANNADA)
    && start + 3 <= end
    && buffer.info[start].is_one_of(category_flag(ot_category_t::OT_Ra))
    && buffer.info[start + 1].is_one_of(category_flag(ot_category_t::OT_H))
    && buffer.info[start + 2].is_one_of(category_flag(ot_category_t::OT_ZWJ))
  {
    true => {
      buffer.merge_clusters(start + 1, start + 3);
      buffer.info.swap(start + 1, start + 2);
    }
    false => {}
  }

  let mut base = end;
  let mut has_reph = false;

  {
    let mut limit = start;
    match indic_plan.mask_array[indic_feature::RPHF] != 0
      && start + 3 <= end
      && ((indic_plan.config.reph_mode == RephMode::Implicit
        && !buffer.info[start + 2].is_joiner())
        || (indic_plan.config.reph_mode == RephMode::Explicit
          && buffer.info[start + 2].indic_category() == ot_category_t::OT_ZWJ))
    {
      true => {
        let glyphs = &[
          buffer.info[start].as_glyph(),
          buffer.info[start + 1].as_glyph(),
          match indic_plan.config.reph_mode == RephMode::Explicit {
            true => buffer.info[start + 2].as_glyph(),
            false => GlyphId::NOTDEF,
          },
        ];
        match indic_plan
          .rphf
          .would_substitute(&plan.ot_map, face, &glyphs[0..2])
          || (indic_plan.config.reph_mode == RephMode::Explicit
            && indic_plan.rphf.would_substitute(&plan.ot_map, face, glyphs))
        {
          true => {
            limit += 2;
            while limit < end && buffer.info[limit].is_joiner() {
              limit += 1;
            }
            base = start;
            has_reph = true;
          }
          false => {}
        }
      }
      false => match indic_plan.config.reph_mode == RephMode::LogRepha
        && buffer.info[start].indic_category() == ot_category_t::OT_Repha
      {
        true => {
          limit += 1;
          while limit < end && buffer.info[limit].is_joiner() {
            limit += 1;
          }
          base = start;
          has_reph = true;
        }
        false => {}
      },
    }

    {
      let mut i = end;
      let mut seen_below = false;
      loop {
        i -= 1;

        match buffer.info[i].is_consonant() {
          true => {
            match buffer.info[i].indic_position() != ot_position_t::POS_BELOW_C
              && (buffer.info[i].indic_position() != ot_position_t::POS_POST_C || seen_below)
            {
              true => {
                base = i;
                break;
              }
              false => {}
            }
            match buffer.info[i].indic_position() == ot_position_t::POS_BELOW_C {
              true => {
                seen_below = true;
              }
              false => {}
            }

            base = i;
          }
          false => {
            match start < i
              && buffer.info[i].indic_category() == ot_category_t::OT_ZWJ
              && buffer.info[i - 1].indic_category() == ot_category_t::OT_H
            {
              true => {
                break;
              }
              false => {}
            }
          }
        }

        match i <= limit {
          true => {
            break;
          }
          false => {}
        }
      }
    }

    match has_reph && base == start && limit - base <= 2 {
      true => {
        has_reph = false;
      }
      false => {}
    }
  }

  for i in start..base {
    let pos = buffer.info[i].indic_position();
    buffer.info[i].set_indic_position(cmp::min(ot_position_t::POS_PRE_C, pos));
  }

  match base < end {
    true => {
      buffer.info[base].set_indic_position(ot_position_t::POS_BASE_C);
    }
    false => {}
  }

  match has_reph {
    true => {
      buffer.info[start].set_indic_position(ot_position_t::POS_RA_TO_BECOME_REPH);
    }
    false => {}
  }

  match indic_plan.is_old_spec {
    true => {
      let disallow_double_halants = buffer.script == Some(script::KANNADA);
      for i in base + 1..end {
        match buffer.info[i].indic_category() == ot_category_t::OT_H {
          true => {
            let mut j = end - 1;
            while j > i {
              match buffer.info[j].is_consonant()
                || (disallow_double_halants
                  && buffer.info[j].indic_category() == ot_category_t::OT_H)
              {
                true => {
                  break;
                }
                false => {}
              }

              j -= 1;
            }

            match buffer.info[j].indic_category() != ot_category_t::OT_H && j > i {
              true => {
                let t = buffer.info[i];
                for k in 0..j - i {
                  buffer.info[k + i] = buffer.info[k + i + 1];
                }
                buffer.info[j] = t;
              }
              false => {}
            }

            break;
          }
          false => {}
        }
      }
    }
    false => {}
  }

  {
    let mut last_pos = ot_position_t::POS_START;
    for i in start..end {
      let ok = rb_flag_unsafe(
        ({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u32>(buffer.info[i].indic_category());
          migrated_value
        }),
      ) & (category_flag(ot_category_t::OT_ZWJ)
        | category_flag(ot_category_t::OT_ZWNJ)
        | category_flag(ot_category_t::OT_N)
        | category_flag(ot_category_t::OT_RS)
        | category_flag(ot_category_t::OT_CM)
        | category_flag(ot_category_t::OT_H))
        != 0;
      match ok {
        true => {
          buffer.info[i].set_indic_position(last_pos);

          match buffer.info[i].indic_category() == ot_category_t::OT_H
            && buffer.info[i].indic_position() == ot_position_t::POS_PRE_M
          {
            true => {
              for j in (start + 1..=i).rev() {
                match buffer.info[j - 1].indic_position() != ot_position_t::POS_PRE_M {
                  true => {
                    let pos = buffer.info[j - 1].indic_position();
                    buffer.info[i].set_indic_position(pos);
                    break;
                  }
                  false => {}
                }
              }
            }
            false => {}
          }
        }
        false => match buffer.info[i].indic_position() != ot_position_t::POS_SMVD {
          true => {
            match buffer.info[i].indic_category() == ot_category_t::OT_MPst
              && i > start
              && buffer.info[i - 1].indic_category() == ot_category_t::OT_SM
            {
              true => {
                let val = buffer.info[i].indic_position();
                buffer.info[i - 1].set_indic_position(val);
              }
              false => {}
            }

            last_pos = buffer.info[i].indic_position();
          }
          false => {}
        },
      }
    }
  }

  {
    let mut last = base;
    for i in base + 1..end {
      match buffer.info[i].is_consonant() {
        true => {
          for j in last + 1..i {
            match buffer.info[j].indic_position() < ot_position_t::POS_SMVD {
              true => {
                let pos = buffer.info[i].indic_position();
                buffer.info[j].set_indic_position(pos);
              }
              false => {}
            }
          }

          last = i;
        }
        false => match (rb_flag_unsafe(
          ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u32>(buffer.info[i].indic_category());
            migrated_value
          }),
        ) & (rb_flag(
          ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u32>(ot_category_t::OT_M);
            migrated_value
          }),
        ) | rb_flag(
          ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u32>(ot_category_t::OT_MPst);
            migrated_value
          }),
        )))
          != 0
        {
          true => {
            last = i;
          }
          false => {}
        },
      }
    }
  }

  {
    let syllable = buffer.info[start].syllable();
    for i in start..end {
      buffer.info[i].set_syllable(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>((i - start));
          migrated_value
        }),
      );
    }

    buffer.info[start..end].sort_by_key(|a| a.indic_position());

    let mut first_left_mantra = end;
    let mut last_left_mantra = end;
    base = end;

    for i in start..end {
      match buffer.info[i].indic_position() == ot_position_t::POS_BASE_C {
        true => {
          base = i;
          break;
        }
        false => match buffer.info[i].indic_position() == ot_position_t::POS_PRE_M {
          true => {
            match first_left_mantra == end {
              true => {
                first_left_mantra = i;
              }
              false => {}
            }

            last_left_mantra = i;
          }
          false => {}
        },
      }
    }

    match first_left_mantra < last_left_mantra {
      true => {
        buffer.reverse_range(first_left_mantra, last_left_mantra + 1);

        let mut i = first_left_mantra;

        for j in i..=last_left_mantra {
          match (rb_flag_unsafe(
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
                buffer.info[j].indic_category(),
              );
              migrated_value
            }),
          ) & (rb_flag(
            ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u32>(ot_category_t::OT_M);
              migrated_value
            }),
          ) | rb_flag(
            ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u32>(ot_category_t::OT_MPst);
              migrated_value
            }),
          )))
            != 0
          {
            true => {
              buffer.reverse_range(i, j + 1);
              i = j + 1;
            }
            false => {}
          }
        }
      }
      false => {}
    }

    match indic_plan.is_old_spec || end - start > 127 {
      true => {
        buffer.merge_clusters(base, end);
      }
      false => {
        for i in base..end {
          match buffer.info[i].syllable() != 255 {
            true => {
              let mut min = i;
              let mut max = i;
              let mut j = start
                + ({
                  let Ok(migrated_value) =
                    console_core_number_conversion::wrapped_index(buffer.info[i].syllable());
                  migrated_value
                });
              while j != i {
                min = cmp::min(min, j);
                max = cmp::max(max, j);
                let next = start
                  + ({
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped_index(buffer.info[j].syllable());
                    migrated_value
                  });
                buffer.info[j].set_syllable(255);
                j = next;
              }

              buffer.merge_clusters(cmp::max(base, min), max + 1);
            }
            false => {}
          }
        }
      }
    }

    for info in &mut buffer.info[start..end] {
      info.set_syllable(syllable);
    }
  }

  {
    for info in &mut buffer.info[start..end] {
      match info.indic_position() != ot_position_t::POS_RA_TO_BECOME_REPH {
        true => {
          break;
        }
        false => {}
      }

      info.mask |= indic_plan.mask_array[indic_feature::RPHF];
    }

    let mut mask = indic_plan.mask_array[indic_feature::HALF];
    match !indic_plan.is_old_spec && indic_plan.config.blwf_mode == BlwfMode::PreAndPost {
      true => {
        mask |= indic_plan.mask_array[indic_feature::BLWF];
      }
      false => {}
    }

    for info in &mut buffer.info[start..base] {
      info.mask |= mask;
    }

    mask = 0;
    match base < end {
      true => {
        buffer.info[base].mask |= mask;
      }
      false => {}
    }

    mask = indic_plan.mask_array[indic_feature::BLWF]
      | indic_plan.mask_array[indic_feature::ABVF]
      | indic_plan.mask_array[indic_feature::PSTF];
    for i in base + 1..end {
      buffer.info[i].mask |= mask;
    }
  }

  match indic_plan.is_old_spec && buffer.script == Some(script::DEVANAGARI) {
    true => {
      for i in start..base.saturating_sub(1) {
        match buffer.info[i].indic_category() == ot_category_t::OT_Ra
          && buffer.info[i + 1].indic_category() == ot_category_t::OT_H
          && (i + 2 == base || buffer.info[i + 2].indic_category() != ot_category_t::OT_ZWJ)
        {
          true => {
            buffer.info[i].mask |= indic_plan.mask_array[indic_feature::BLWF];
            buffer.info[i + 1].mask |= indic_plan.mask_array[indic_feature::BLWF];
          }
          false => {}
        }
      }
    }
    false => {}
  }

  let pref_len = 2;
  match indic_plan.mask_array[indic_feature::PREF] != 0 && base + pref_len < end {
    true => {
      for i in base + 1..end - pref_len + 1 {
        let glyphs = &[buffer.info[i + 0].as_glyph(), buffer.info[i + 1].as_glyph()];
        match indic_plan.pref.would_substitute(&plan.ot_map, face, glyphs) {
          true => {
            buffer.info[i + 0].mask |= indic_plan.mask_array[indic_feature::PREF];
            buffer.info[i + 1].mask |= indic_plan.mask_array[indic_feature::PREF];
            break;
          }
          false => {}
        }
      }
    }
    false => {}
  }

  for i in start + 1..end {
    match buffer.info[i].is_joiner() {
      true => {
        let non_joiner = buffer.info[i].indic_category() == ot_category_t::OT_ZWNJ;
        let mut j = i;

        loop {
          j -= 1;

          match non_joiner {
            true => {
              buffer.info[j].mask &= !indic_plan.mask_array[indic_feature::HALF];
            }
            false => {}
          }

          match j <= start || buffer.info[j].is_consonant() {
            true => {
              break;
            }
            false => {}
          }
        }
      }
      false => {}
    }
  }
}

fn initial_reordering_standalone_cluster(
  plan: &hb_ot_shape_plan_t,
  indic_plan: &IndicShapePlan,
  face: &mut FontFuncsDispatch,
  start: usize,
  end: usize,
  buffer: &mut hb_buffer_t,
) {
  initial_reordering_consonant_syllable(plan, indic_plan, face, start, end, buffer);
}

fn final_reordering(
  plan: &hb_ot_shape_plan_t,
  face: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) -> bool {
  match buffer.is_empty() {
    true => {
      return false;
    }
    false => {}
  }

  foreach_syllable!(buffer, start, end, {
    final_reordering_impl(plan, face, start, end, buffer);
  });

  buffer.deallocate_var(GlyphInfo::INDIC_CATEGORY_VAR);
  buffer.deallocate_var(GlyphInfo::INDIC_POSITION_VAR);

  false
}

fn final_reordering_impl(
  plan: &hb_ot_shape_plan_t,
  face: &mut FontFuncsDispatch,
  start: usize,
  end: usize,
  buffer: &mut hb_buffer_t,
) {
  let indic_plan = plan.data::<IndicShapePlan>();

  let mut virama_glyph = None;
  match indic_plan.config.virama != 0 {
    true => match face.nominal_glyph(indic_plan.config.virama) {
      Some(g) => {
        virama_glyph = Some(
          ({
            let Ok(value) = g.to_u32();
            value
          }),
        );
      }
      None => {}
    },
    false => {}
  }

  match virama_glyph {
    Some(virama_glyph) => {
      for info in &mut buffer.info[start..end] {
        match info.glyph_id == (virama_glyph) && info.ligated() && info.multiplied() {
          true => {
            info.set_indic_category(ot_category_t::OT_H);
            info.clear_ligated_and_multiplied();
          }
          false => {}
        }
      }
    }
    None => {}
  }

  let mut try_pref = indic_plan.mask_array[indic_feature::PREF] != 0;

  let mut base = start;
  while base < end {
    match ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u32>(buffer.info[base].indic_position());
      migrated_value
    }) >= ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u32>(ot_position_t::POS_BASE_C);
      migrated_value
    }) {
      true => {
        match try_pref && base + 1 < end {
          true => {
            for i in base + 1..end {
              match (buffer.info[i].mask & indic_plan.mask_array[indic_feature::PREF]) != 0 {
                true => {
                  match !(buffer.info[i].substituted()
                    && buffer.info[i].ligated_and_didnt_multiply())
                  {
                    true => {
                      base = i;
                      while base < end && buffer.info[base].is_halant() {
                        base += 1;
                      }

                      match base < end {
                        true => {
                          buffer.info[base].set_indic_position(ot_position_t::POS_BASE_C);
                        }
                        false => {}
                      }

                      try_pref = false;
                    }
                    false => {}
                  }

                  break;
                }
                false => {}
              }

              match base == end {
                true => {
                  break;
                }
                false => {}
              }
            }
          }
          false => {}
        }

        match buffer.script == Some(script::MALAYALAM) {
          true => {
            let mut i = base + 1;
            while i < end {
              while i < end && buffer.info[i].is_joiner() {
                i += 1;
              }

              match i == end || !buffer.info[i].is_halant() {
                true => {
                  break;
                }
                false => {}
              }

              i += 1;

              while i < end && buffer.info[i].is_joiner() {
                i += 1;
              }

              match i < end
                && buffer.info[i].is_consonant()
                && buffer.info[i].indic_position() == ot_position_t::POS_BELOW_C
              {
                true => {
                  base = i;
                  buffer.info[base].set_indic_position(ot_position_t::POS_BASE_C);
                }
                false => {}
              }

              i += 1;
            }
          }
          false => {}
        }

        match start < base
          && ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
              buffer.info[base].indic_position(),
            );
            migrated_value
          }) > ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u32>(ot_position_t::POS_BASE_C);
            migrated_value
          }) {
          true => {
            base -= 1;
          }
          false => {}
        }

        break;
      }
      false => {}
    }

    base += 1;
  }

  match base == end
    && start < base
    && buffer.info[base - 1].is_one_of(rb_flag(
      ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u32>(ot_category_t::OT_ZWJ);
        migrated_value
      }),
    )) {
    true => {
      base -= 1;
    }
    false => {}
  }

  match base < end {
    true => {
      while start < base
        && buffer.info[base].is_one_of(
          rb_flag(
            ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u32>(ot_category_t::OT_N);
              migrated_value
            }),
          ) | rb_flag(
            ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u32>(ot_category_t::OT_H);
              migrated_value
            }),
          ),
        )
      {
        base -= 1;
      }
    }
    false => {}
  }

  match start + 1 < end && start < base {
    true => {
      let mut new_pos = match base == end {
        true => base - 2,
        false => base - 1,
      };

      match buffer.script != Some(script::MALAYALAM) && buffer.script != Some(script::TAMIL) {
        true => loop {
          while new_pos > start
            && !buffer.info[new_pos].is_one_of(
              rb_flag(
                ({
                  let Ok(migrated_value) =
                    console_core_number_conversion::wrapped::<_, u32>(ot_category_t::OT_M);
                  migrated_value
                }),
              ) | rb_flag(
                ({
                  let Ok(migrated_value) =
                    console_core_number_conversion::wrapped::<_, u32>(ot_category_t::OT_MPst);
                  migrated_value
                }),
              ) | rb_flag(
                ({
                  let Ok(migrated_value) =
                    console_core_number_conversion::wrapped::<_, u32>(ot_category_t::OT_H);
                  migrated_value
                }),
              ),
            )
          {
            new_pos -= 1;
          }

          match buffer.info[new_pos].is_halant()
            && buffer.info[new_pos].indic_position() != ot_position_t::POS_PRE_M
          {
            true => match new_pos + 1 < end {
              true => match buffer.info[new_pos + 1].indic_category() == ot_category_t::OT_ZWJ {
                true => match new_pos > start {
                  true => {
                    new_pos -= 1;
                    continue;
                  }
                  false => {}
                },
                false => {}
              },
              false => {}
            },
            false => {
              new_pos = start;
            }
          }

          break;
        },
        false => {}
      }

      match start < new_pos && buffer.info[new_pos].indic_position() != ot_position_t::POS_PRE_M {
        true => {
          for i in (start + 1..=new_pos).rev() {
            match buffer.info[i - 1].indic_position() == ot_position_t::POS_PRE_M {
              true => {
                let old_pos = i - 1;

                match old_pos < base && base <= new_pos {
                  true => {
                    base -= 1;
                  }
                  false => {}
                }

                let tmp = buffer.info[old_pos];
                for i in 0..new_pos - old_pos {
                  buffer.info[i + old_pos] = buffer.info[i + old_pos + 1];
                }
                buffer.info[new_pos] = tmp;

                buffer.merge_clusters(new_pos, cmp::min(end, base + 1));

                new_pos -= 1;
              }
              false => {}
            }
          }
        }
        false => {
          for i in start..base {
            match buffer.info[i].indic_position() == ot_position_t::POS_PRE_M {
              true => {
                buffer.merge_clusters(i, cmp::min(end, base + 1));
                break;
              }
              false => {}
            }
          }
        }
      }
    }
    false => {}
  }

  match start + 1 < end
    && buffer.info[start].indic_position() == ot_position_t::POS_RA_TO_BECOME_REPH
    && (buffer.info[start].indic_category() == ot_category_t::OT_Repha)
      ^ buffer.info[start].ligated_and_didnt_multiply()
  {
    true => {
      let mut new_reph_pos;
      'reph: {
        let reph_pos = indic_plan.config.reph_pos;

        match reph_pos != RephPosition::AfterPost {
          true => {
            {
              new_reph_pos = start + 1;
              while new_reph_pos < base && !buffer.info[new_reph_pos].is_halant() {
                new_reph_pos += 1;
              }

              match new_reph_pos < base && buffer.info[new_reph_pos].is_halant() {
                true => {
                  match new_reph_pos + 1 < base && buffer.info[new_reph_pos + 1].is_joiner() {
                    true => {
                      new_reph_pos += 1;
                    }
                    false => {}
                  }

                  break 'reph;
                }
                false => {}
              }
            }

            match reph_pos == RephPosition::AfterMain {
              true => {
                new_reph_pos = base;
                while new_reph_pos + 1 < end
                  && buffer.info[new_reph_pos + 1].indic_position()
                    <= ot_position_t::POS_AFTER_MAIN
                {
                  new_reph_pos += 1;
                }

                match new_reph_pos < end {
                  true => {
                    break 'reph;
                  }
                  false => {}
                }
              }
              false => {}
            }

            match reph_pos == RephPosition::AfterSub {
              true => {
                new_reph_pos = base;
                while new_reph_pos + 1 < end
                  && (rb_flag_unsafe(
                    ({
                      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
                        buffer.info[new_reph_pos + 1].indic_position(),
                      );
                      migrated_value
                    }),
                  ) & (rb_flag(
                    ({
                      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
                        ot_position_t::POS_POST_C,
                      );
                      migrated_value
                    }),
                  ) | rb_flag(
                    ({
                      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
                        ot_position_t::POS_AFTER_POST,
                      );
                      migrated_value
                    }),
                  ) | rb_flag(
                    ({
                      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
                        ot_position_t::POS_SMVD,
                      );
                      migrated_value
                    }),
                  )))
                    == 0
                {
                  new_reph_pos += 1;
                }

                match new_reph_pos < end {
                  true => {
                    break 'reph;
                  }
                  false => {}
                }
              }
              false => {}
            }
          }
          false => {}
        }

        new_reph_pos = start + 1;
        while new_reph_pos < base && !buffer.info[new_reph_pos].is_halant() {
          new_reph_pos += 1;
        }

        match new_reph_pos < base && buffer.info[new_reph_pos].is_halant() {
          true => {
            match new_reph_pos + 1 < base && buffer.info[new_reph_pos + 1].is_joiner() {
              true => {
                new_reph_pos += 1;
              }
              false => {}
            }

            break 'reph;
          }
          false => {}
        }

        {
          new_reph_pos = end - 1;
          while new_reph_pos > start
            && buffer.info[new_reph_pos].indic_position() == ot_position_t::POS_SMVD
          {
            new_reph_pos -= 1;
          }

          match buffer.info[new_reph_pos].is_halant() {
            true => {
              for info in &buffer.info[base + 1..new_reph_pos] {
                match (rb_flag_unsafe(
                  ({
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped::<_, u32>(info.indic_category());
                    migrated_value
                  }),
                ) & (rb_flag(
                  ({
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped::<_, u32>(ot_category_t::OT_M);
                    migrated_value
                  }),
                ) | rb_flag(
                  ({
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped::<_, u32>(ot_category_t::OT_MPst);
                    migrated_value
                  }),
                )))
                  != 0
                {
                  true => {
                    new_reph_pos -= 1;
                  }
                  false => {}
                }
              }
            }
            false => {}
          }
        }

        break 'reph;
      }

      buffer.merge_clusters(start, new_reph_pos + 1);

      let reph = buffer.info[start];
      for i in 0..new_reph_pos - start {
        buffer.info[i + start] = buffer.info[i + start + 1];
      }
      buffer.info[new_reph_pos] = reph;

      match start < base && base <= new_reph_pos {
        true => {
          base -= 1;
        }
        false => {}
      }
    }
    false => {}
  }

  match try_pref && base + 1 < end {
    true => {
      for i in base + 1..end {
        match (buffer.info[i].mask & indic_plan.mask_array[indic_feature::PREF]) != 0 {
          true => {
            match buffer.info[i].ligated_and_didnt_multiply() {
              true => {
                let mut new_pos = base;

                match buffer.script != Some(script::MALAYALAM)
                  && buffer.script != Some(script::TAMIL)
                {
                  true => {
                    while new_pos > start
                      && !buffer.info[new_pos - 1].is_one_of(
                        rb_flag(
                          ({
                            let Ok(migrated_value) =
                              console_core_number_conversion::wrapped::<_, u32>(
                                ot_category_t::OT_M,
                              );
                            migrated_value
                          }),
                        ) | rb_flag(
                          ({
                            let Ok(migrated_value) =
                              console_core_number_conversion::wrapped::<_, u32>(
                                ot_category_t::OT_MPst,
                              );
                            migrated_value
                          }),
                        ) | rb_flag(
                          ({
                            let Ok(migrated_value) =
                              console_core_number_conversion::wrapped::<_, u32>(
                                ot_category_t::OT_H,
                              );
                            migrated_value
                          }),
                        ),
                      )
                    {
                      new_pos -= 1;
                    }
                  }
                  false => {}
                }

                match new_pos > start && buffer.info[new_pos - 1].is_halant() {
                  true => match new_pos < end && buffer.info[new_pos].is_joiner() {
                    true => {
                      new_pos += 1;
                    }
                    false => {}
                  },
                  false => {}
                }

                {
                  let old_pos = i;

                  buffer.merge_clusters(new_pos, old_pos + 1);
                  let tmp = buffer.info[old_pos];
                  for i in (0..old_pos - new_pos).rev() {
                    buffer.info[i + new_pos + 1] = buffer.info[i + new_pos];
                  }
                  buffer.info[new_pos] = tmp;

                  match new_pos <= base && base < old_pos {
                    true => {
                      #[allow(unused_assignments)]
                      {
                        base += 1;
                      }
                    }
                    false => {}
                  }
                }
              }
              false => {}
            }

            break;
          }
          false => {}
        }
      }
    }
    false => {}
  }

  match buffer.info[start].indic_position() == ot_position_t::POS_PRE_M {
    true => {
      match start == 0
        || (rb_flag_unsafe(
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
              buffer.info[start - 1].general_category().to_u8(),
            );
            migrated_value
          }),
        ) & rb_flag_range(
          hb_gc::HB_UNICODE_GENERAL_CATEGORY_FORMAT,
          hb_gc::HB_UNICODE_GENERAL_CATEGORY_NON_SPACING_MARK,
        )) == 0
      {
        true => {
          buffer.info[start].mask |= indic_plan.mask_array[indic_feature::INIT];
        }
        false => {
          buffer.unsafe_to_break(Some(start - 1), Some(start + 1));
        }
      }
    }
    false => {}
  }
}
