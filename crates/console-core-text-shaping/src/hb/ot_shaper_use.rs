use super::algs::*;
use super::buffer::*;
use super::font_funcs::FontFuncsDispatch;
use super::ot_layout::*;
use super::ot_map::*;
use super::ot_shape::*;
use super::ot_shape_normalize::*;
use super::ot_shape_plan::hb_ot_shape_plan_t;
use super::ot_shaper::*;
use super::ot_shaper_arabic::arabic_shape_plan_t;
use super::ot_shaper_syllabic::*;
use super::unicode::{CharExt, Codepoint};
use super::{GlyphInfo, Script, hb_mask_t, hb_tag_t, script};
use alloc::boxed::Box;

pub const UNIVERSAL_SHAPER: hb_ot_shaper_t = hb_ot_shaper_t {
  collect_features: Some(collect_features),
  override_features: None,
  create_data: Some(|plan| Box::new(UniversalShapePlan::new(plan))),
  preprocess_text: Some(preprocess_text),
  postprocess_glyphs: None,
  normalization_preference: HB_OT_SHAPE_NORMALIZATION_MODE_COMPOSED_DIACRITICS_NO_SHORT_CIRCUIT,
  decompose: None,
  compose: Some(compose),
  setup_masks: Some(setup_masks),
  gpos_tag: None,
  reorder_marks: None,
  zero_width_marks: HB_OT_SHAPE_ZERO_WIDTH_MARKS_BY_GDEF_EARLY,
  fallback_position: false,
};

impl GlyphInfo {
  declare_buffer_var_alias!(
    OT_SHAPER_VAR_U8_CATEGORY_VAR,
    u8,
    USE_CATEGORY_VAR,
    use_category,
    set_use_category
  );

  fn is_halant_use(&self) -> bool {
    matches!(
      self.use_category(),
      category::H | category::HVM | category::IS
    ) && !self.ligated()
  }
}

pub type Category = u8;
#[allow(dead_code)]
pub mod category {
  pub const O: u8 = 0;

  pub const B: u8 = 1;

  pub const N: u8 = 4;
  pub const GB: u8 = 5;
  pub const CGJ: u8 = 6;

  pub const SUB: u8 = 11;
  pub const H: u8 = 12;

  pub const HN: u8 = 13;
  pub const ZWNJ: u8 = 14;

  pub const WJ: u8 = 16;

  pub const RSV: u8 = 17;
  pub const R: u8 = 18;
  pub const S: u8 = 19;

  pub const CS: u8 = 43;

  pub const IS: u8 = 44;

  pub const Sk: u8 = 48;

  pub const FAbv: u8 = 24;
  pub const FBlw: u8 = 25;
  pub const FPst: u8 = 26;
  pub const MAbv: u8 = 27;
  pub const MBlw: u8 = 28;
  pub const MPst: u8 = 29;
  pub const MPre: u8 = 30;
  pub const CMAbv: u8 = 31;
  pub const CMBlw: u8 = 32;
  pub const VAbv: u8 = 33;
  pub const VBlw: u8 = 34;
  pub const VPst: u8 = 35;
  pub const VPre: u8 = 22;
  pub const VMAbv: u8 = 37;
  pub const VMBlw: u8 = 38;
  pub const VMPst: u8 = 39;
  pub const VMPre: u8 = 23;
  pub const SMAbv: u8 = 41;
  pub const SMBlw: u8 = 42;
  pub const FMAbv: u8 = 45;
  pub const FMBlw: u8 = 46;
  pub const FMPst: u8 = 47;
  pub const G: u8 = 49;
  pub const J: u8 = 50;
  pub const SB: u8 = 51;
  pub const SE: u8 = 52;
  pub const HVM: u8 = 53;
  pub const HM: u8 = 54;
  pub const HR: u8 = 55;
  pub const RK: u8 = 56;
}

static BASIC_FEATURES: &[hb_tag_t] = &[
  ({
    let Ok(value) = hb_tag_t::new(b"rkrf");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"abvf");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"blwf");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"half");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"pstf");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"vatu");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"cjct");
    value
  }),
];

static TOPOGRAPHICAL_FEATURES: &[hb_tag_t] = &[
  ({
    let Ok(value) = hb_tag_t::new(b"isol");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"init");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"medi");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"fina");
    value
  }),
];

#[derive(Clone, Copy, PartialEq)]
enum JoiningForm {
  Isolated = 0,
  Initial,
  Medial,
  Terminal,
}

static OTHER_FEATURES: &[hb_tag_t] = &[
  ({
    let Ok(value) = hb_tag_t::new(b"abvs");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"blws");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"haln");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"pres");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"psts");
    value
  }),
];

struct UniversalShapePlan {
  rphf_mask: hb_mask_t,
  arabic_plan: Option<arabic_shape_plan_t>,
}

impl UniversalShapePlan {
  fn new(plan: &hb_ot_shape_plan_t) -> UniversalShapePlan {
    let mut arabic_plan = None;

    match plan.script.is_some_and(has_arabic_joining) {
      true => {
        arabic_plan = Some(crate::hb::ot_shaper_arabic::data_create_arabic(plan));
      }
      false => {}
    }

    UniversalShapePlan {
      rphf_mask: plan.ot_map.get_1_mask({
        let Ok(value) = hb_tag_t::new(b"rphf");
        value
      }),
      arabic_plan,
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
  planner.ot_map.enable_feature(
    {
      let Ok(value) = hb_tag_t::new(b"nukt");
      value
    },
    F_PER_SYLLABLE,
    1,
  );
  planner.ot_map.enable_feature(
    {
      let Ok(value) = hb_tag_t::new(b"akhn");
      value
    },
    F_MANUAL_ZWJ | F_PER_SYLLABLE,
    1,
  );

  planner
    .ot_map
    .add_gsub_pause(Some(_hb_clear_substitution_flags));
  planner.ot_map.add_feature(
    {
      let Ok(value) = hb_tag_t::new(b"rphf");
      value
    },
    F_MANUAL_ZWJ | F_PER_SYLLABLE,
    1,
  );
  planner.ot_map.add_gsub_pause(Some(record_rphf));
  planner
    .ot_map
    .add_gsub_pause(Some(_hb_clear_substitution_flags));
  planner.ot_map.enable_feature(
    {
      let Ok(value) = hb_tag_t::new(b"pref");
      value
    },
    F_MANUAL_ZWJ | F_PER_SYLLABLE,
    1,
  );
  planner.ot_map.add_gsub_pause(Some(record_pref));

  for feature in BASIC_FEATURES {
    planner
      .ot_map
      .enable_feature(*feature, F_MANUAL_ZWJ | F_PER_SYLLABLE, 1);
  }

  planner.ot_map.add_gsub_pause(Some(reorder_use));
  planner.ot_map.add_gsub_pause(Some(syllabic_clear_var));

  for feature in TOPOGRAPHICAL_FEATURES {
    planner.ot_map.add_feature(*feature, F_NONE, 1);
  }
  planner.ot_map.add_gsub_pause(None);

  for feature in OTHER_FEATURES {
    planner.ot_map.enable_feature(*feature, F_MANUAL_ZWJ, 1);
  }
}

fn setup_syllables(
  plan: &hb_ot_shape_plan_t,
  _: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) -> bool {
  buffer.allocate_var(GlyphInfo::SYLLABLE_VAR);

  super::ot_shaper_use_machine::find_syllables(buffer);

  foreach_syllable!(buffer, start, end, {
    buffer.unsafe_to_break(Some(start), Some(end));
  });

  setup_rphf_mask(plan, buffer);
  setup_topographical_masks(plan, buffer);

  false
}

fn setup_rphf_mask(plan: &hb_ot_shape_plan_t, buffer: &mut hb_buffer_t) -> bool {
  let universal_plan = plan.data::<UniversalShapePlan>();

  let mask = universal_plan.rphf_mask;
  match mask == 0 {
    true => {
      return false;
    }
    false => {}
  }

  let mut start = 0;
  let mut end = buffer.next_syllable(0);
  while start < buffer.len {
    let limit = match buffer.info[start].use_category() == category::R {
      true => 1,
      false => core::cmp::min(3, end - start),
    };

    for i in start..start + limit {
      buffer.info[i].mask |= mask;
    }

    start = end;
    end = buffer.next_syllable(start);
  }

  false
}

fn setup_topographical_masks(plan: &hb_ot_shape_plan_t, buffer: &mut hb_buffer_t) {
  use super::ot_shaper_use_machine::SyllableType;

  match plan.data::<UniversalShapePlan>().arabic_plan.is_some() {
    true => {
      return;
    }
    false => {}
  }

  let mut masks = [0; 4];
  let mut all_masks = 0;
  for i in 0..4 {
    masks[i] = plan.ot_map.get_1_mask(TOPOGRAPHICAL_FEATURES[i]);
    match masks[i] == plan.ot_map.get_global_mask() {
      true => {
        masks[i] = 0;
      }
      false => {}
    }

    all_masks |= masks[i];
  }

  match all_masks == 0 {
    true => {
      return;
    }
    false => {}
  }

  let other_masks = !all_masks;

  let mut last_start = 0;
  let mut last_form = None;
  let mut start = 0;
  let mut end = buffer.next_syllable(0);
  while start < buffer.len {
    let syllable = buffer.info[start].syllable() & 0x0F;
    match syllable == SyllableType::HieroglyphCluster as u8
      || syllable == SyllableType::NonCluster as u8
    {
      true => {
        last_form = None;
      }
      false => {
        let join =
          last_form == Some(JoiningForm::Terminal) || last_form == Some(JoiningForm::Isolated);

        match join {
          true => {
            let form = match last_form == Some(JoiningForm::Terminal) {
              true => JoiningForm::Medial,
              false => JoiningForm::Initial,
            };

            for i in last_start..start {
              buffer.info[i].mask = (buffer.info[i].mask & other_masks) | masks[form as usize];
            }
          }
          false => {}
        }

        let form = match join {
          true => JoiningForm::Terminal,
          false => JoiningForm::Isolated,
        };
        last_form = Some(form);
        for i in start..end {
          buffer.info[i].mask = (buffer.info[i].mask & other_masks) | masks[form as usize];
        }
      }
    }

    last_start = start;
    start = end;
    end = buffer.next_syllable(start);
  }
}

fn record_rphf(
  plan: &hb_ot_shape_plan_t,
  _: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) -> bool {
  let universal_plan = plan.data::<UniversalShapePlan>();

  let mask = universal_plan.rphf_mask;
  match mask == 0 {
    true => {
      return false;
    }
    false => {}
  }

  let mut start = 0;
  let mut end = buffer.next_syllable(0);
  while start < buffer.len {
    for i in start..end {
      match buffer.info[i].mask & mask == 0 {
        true => {
          break;
        }
        false => {}
      }

      match buffer.info[i].substituted() {
        true => {
          buffer.info[i].set_use_category(category::R);
          break;
        }
        false => {}
      }
    }

    start = end;
    end = buffer.next_syllable(start);
  }

  false
}

fn reorder_use(
  _: &hb_ot_shape_plan_t,
  font: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) -> bool {
  use super::ot_shaper_use_machine::SyllableType;

  let mut ret = false;

  match insert_dotted_circles(
    font,
    buffer,
    SyllableType::BrokenCluster as u8,
    category::B,
    Some(category::R),
    None,
  ) {
    true => {
      ret = true;
    }
    false => {}
  }

  let mut start = 0;
  let mut end = buffer.next_syllable(0);
  while start < buffer.len {
    reorder_syllable_use(start, end, buffer);
    start = end;
    end = buffer.next_syllable(start);
  }

  buffer.deallocate_var(GlyphInfo::USE_CATEGORY_VAR);

  ret
}

const fn category_flag(c: Category) -> u32 {
  rb_flag(c as u32)
}

const fn category_flag64(c: Category) -> u64 {
  rb_flag64(c as u32)
}

const POST_BASE_FLAGS: u64 = category_flag64(category::FAbv)
  | category_flag64(category::FBlw)
  | category_flag64(category::FPst)
  | category_flag64(category::FMAbv)
  | category_flag64(category::FMBlw)
  | category_flag64(category::FMPst)
  | category_flag64(category::MAbv)
  | category_flag64(category::MBlw)
  | category_flag64(category::MPst)
  | category_flag64(category::MPre)
  | category_flag64(category::VAbv)
  | category_flag64(category::VBlw)
  | category_flag64(category::VPst)
  | category_flag64(category::VPre)
  | category_flag64(category::VMAbv)
  | category_flag64(category::VMBlw)
  | category_flag64(category::VMPst)
  | category_flag64(category::VMPre);

fn reorder_syllable_use(start: usize, end: usize, buffer: &mut hb_buffer_t) {
  use super::ot_shaper_use_machine::SyllableType;

  let syllable_type = ({
    let Ok(migrated_value) =
      console_core_number_conversion::wrapped::<_, u32>((buffer.info[start].syllable() & 0x0F));
    migrated_value
  });

  match (rb_flag_unsafe(syllable_type)
    & (rb_flag(SyllableType::ViramaTerminatedCluster as u32)
      | rb_flag(SyllableType::SakotTerminatedCluster as u32)
      | rb_flag(SyllableType::StandardCluster as u32)
      | rb_flag(SyllableType::BrokenCluster as u32)
      | 0))
    == 0
  {
    true => {
      return;
    }
    false => {}
  }

  match buffer.info[start].use_category() == category::R && end - start > 1 {
    true => {
      for i in start + 1..end {
        let is_post_base_glyph = (rb_flag64_unsafe(
          ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u32>(buffer.info[i].use_category());
            migrated_value
          }),
        ) & POST_BASE_FLAGS)
          != 0
          || buffer.info[i].is_halant_use();

        match is_post_base_glyph || i == end - 1 {
          true => {
            let mut i = i;
            match is_post_base_glyph {
              true => {
                i -= 1;
              }
              false => {}
            }

            buffer.merge_clusters(start, i + 1);
            let t = buffer.info[start];
            for k in 0..i - start {
              buffer.info[k + start] = buffer.info[k + start + 1];
            }
            buffer.info[i] = t;

            break;
          }
          false => {}
        }
      }
    }
    false => {}
  }

  let mut j = start;
  for i in start..end {
    let flag = rb_flag_unsafe(
      ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u32>(buffer.info[i].use_category());
        migrated_value
      }),
    );
    match buffer.info[i].is_halant_use() {
      true => {
        j = i + 1;
      }
      false => {
        match (flag & (category_flag(category::VPre) | category_flag(category::VMPre))) != 0
          && buffer.info[i].lig_comp() == 0
          && j < i
        {
          true => {
            buffer.merge_clusters(j, i + 1);
            let t = buffer.info[i];
            for k in (0..i - j).rev() {
              buffer.info[k + j + 1] = buffer.info[k + j];
            }
            buffer.info[j] = t;
          }
          false => {}
        }
      }
    }
  }
}

fn record_pref(
  _: &hb_ot_shape_plan_t,
  _: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) -> bool {
  let mut start = 0;
  let mut end = buffer.next_syllable(0);
  while start < buffer.len {
    for i in start..end {
      match buffer.info[i].substituted() {
        true => {
          buffer.info[i].set_use_category(category::VPre);
          break;
        }
        false => {}
      }
    }

    start = end;
    end = buffer.next_syllable(start);
  }

  false
}

fn has_arabic_joining(script: Script) -> bool {
  matches!(
    script,
    script::ADLAM
      | script::ARABIC
      | script::CHORASMIAN
      | script::HANIFI_ROHINGYA
      | script::MANDAIC
      | script::MANICHAEAN
      | script::MONGOLIAN
      | script::NKO
      | script::OLD_UYGHUR
      | script::PHAGS_PA
      | script::PSALTER_PAHLAVI
      | script::SOGDIAN
      | script::SYRIAC
  )
}

fn preprocess_text(_: &hb_ot_shape_plan_t, _: &mut FontFuncsDispatch, buffer: &mut hb_buffer_t) {
  super::ot_shaper_vowel_constraints::preprocess_text_vowel_constraints(buffer);
}

fn compose(_: &hb_ot_shape_normalize_context_t, a: Codepoint, b: Codepoint) -> Option<Codepoint> {
  match a.general_category().is_mark() {
    true => {
      return None;
    }
    false => {}
  }

  crate::hb::unicode::compose(a, b)
}

fn setup_masks(plan: &hb_ot_shape_plan_t, _: &mut FontFuncsDispatch, buffer: &mut hb_buffer_t) {
  let universal_plan = plan.data::<UniversalShapePlan>();

  if let Some(ref arabic_plan) = universal_plan.arabic_plan {
    crate::hb::ot_shaper_arabic::setup_masks_inner(arabic_plan, plan.script, buffer);
  }

  buffer.allocate_var(GlyphInfo::USE_CATEGORY_VAR);

  for info in buffer.info_slice_mut() {
    info.set_use_category(super::ot_shaper_use_table::hb_use_get_category(
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(info.glyph_id);
        migrated_value
      }),
    ));
  }
}
