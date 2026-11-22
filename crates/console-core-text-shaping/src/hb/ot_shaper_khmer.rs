use super::buffer::*;
use super::font_funcs::FontFuncsDispatch;
use super::ot_map::*;
use super::ot_shape::*;
use super::ot_shape_normalize::*;
use super::ot_shape_plan::hb_ot_shape_plan_t;
use super::ot_shaper::*;
use super::ot_shaper_indic::ot_category_t;
use super::ot_shaper_syllabic::*;
use super::unicode::CharExt;
use super::unicode::Codepoint;
use super::{GlyphInfo, hb_mask_t, hb_tag_t};
use alloc::boxed::Box;

pub const KHMER_SHAPER: hb_ot_shaper_t = hb_ot_shaper_t {
  collect_features: Some(collect_features),
  override_features: Some(override_features),
  create_data: Some(|plan| Box::new(KhmerShapePlan::new(plan))),
  preprocess_text: None,
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
    KHMER_CATEGORY_VAR,
    khmer_category,
    set_khmer_category
  );
}

const KHMER_FEATURES: &[(hb_tag_t, hb_ot_map_feature_flags_t)] = &[
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
      let Ok(value) = hb_tag_t::new(b"pstf");
      value
    },
    F_MANUAL_JOINERS | F_PER_SYLLABLE,
  ),
  (
    {
      let Ok(value) = hb_tag_t::new(b"cfar");
      value
    },
    F_MANUAL_JOINERS | F_PER_SYLLABLE,
  ),
  (
    {
      let Ok(value) = hb_tag_t::new(b"pres");
      value
    },
    F_GLOBAL_MANUAL_JOINERS,
  ),
  (
    {
      let Ok(value) = hb_tag_t::new(b"abvs");
      value
    },
    F_GLOBAL_MANUAL_JOINERS,
  ),
  (
    {
      let Ok(value) = hb_tag_t::new(b"blws");
      value
    },
    F_GLOBAL_MANUAL_JOINERS,
  ),
  (
    {
      let Ok(value) = hb_tag_t::new(b"psts");
      value
    },
    F_GLOBAL_MANUAL_JOINERS,
  ),
];

mod khmer_feature {
  pub const PREF: usize = 0;
  pub const BLWF: usize = 1;
  pub const ABVF: usize = 2;
  pub const PSTF: usize = 3;
  pub const CFAR: usize = 4;
}

impl GlyphInfo {
  fn set_khmer_properties(&mut self) {
    let u = self.glyph_id;
    let (cat, _) = crate::hb::ot_shaper_indic_table::get_categories(u);

    self.set_khmer_category(cat);
  }
}

struct KhmerShapePlan {
  mask_array: [hb_mask_t; KHMER_FEATURES.len()],
}

impl KhmerShapePlan {
  fn new(plan: &hb_ot_shape_plan_t) -> Self {
    let mut mask_array = [0; KHMER_FEATURES.len()];
    for (i, feature) in KHMER_FEATURES.iter().enumerate() {
      mask_array[i] = match feature.1 & F_GLOBAL != 0 {
        true => 0,
        false => plan.ot_map.get_1_mask(feature.0),
      }
    }

    KhmerShapePlan { mask_array }
  }
}

fn collect_features(planner: &mut hb_ot_shape_planner_t) {
  planner.ot_map.add_gsub_pause(Some(setup_syllables));
  planner.ot_map.add_gsub_pause(Some(reorder_khmer));

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

  for feature in KHMER_FEATURES.iter().take(5) {
    planner.ot_map.add_feature(feature.0, feature.1, 1);
  }

  planner.ot_map.add_gsub_pause(Some(syllabic_clear_var));

  for feature in KHMER_FEATURES.iter().skip(5) {
    planner.ot_map.add_feature(feature.0, feature.1, 1);
  }
}

fn setup_syllables(
  _: &hb_ot_shape_plan_t,
  _: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) -> bool {
  buffer.allocate_var(GlyphInfo::SYLLABLE_VAR);

  super::ot_shaper_khmer_machine::find_syllables_khmer(buffer);

  let mut start = 0;
  let mut end = buffer.next_syllable(0);
  while start < buffer.len {
    buffer.unsafe_to_break(Some(start), Some(end));
    start = end;
    end = buffer.next_syllable(start);
  }

  false
}

fn reorder_khmer(
  plan: &hb_ot_shape_plan_t,
  face: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) -> bool {
  use super::ot_shaper_khmer_machine::SyllableType;

  let mut ret = false;

  match insert_dotted_circles(
    face,
    buffer,
    SyllableType::BrokenCluster as u8,
    ot_category_t::OT_DOTTEDCIRCLE,
    Some(ot_category_t::OT_Repha),
    None,
  ) {
    true => {
      ret = true;
    }
    false => {}
  }

  let khmer_plan = plan.data::<KhmerShapePlan>();

  let mut start = 0;
  let mut end = buffer.next_syllable(0);
  while start < buffer.len {
    reorder_syllable_khmer(khmer_plan, start, end, buffer);
    start = end;
    end = buffer.next_syllable(start);
  }

  buffer.deallocate_var(GlyphInfo::KHMER_CATEGORY_VAR);

  ret
}

fn reorder_syllable_khmer(
  khmer_plan: &KhmerShapePlan,
  start: usize,
  end: usize,
  buffer: &mut hb_buffer_t,
) {
  use super::ot_shaper_khmer_machine::SyllableType;

  let syllable_type = match buffer.info[start].syllable() & 0x0F {
    0 => SyllableType::ConsonantSyllable,
    1 => SyllableType::BrokenCluster,
    2 => SyllableType::NonKhmerCluster,
    _ => unreachable!(),
  };

  match syllable_type {
    SyllableType::ConsonantSyllable | SyllableType::BrokenCluster => {
      reorder_consonant_syllable(khmer_plan, start, end, buffer);
    }
    SyllableType::NonKhmerCluster => {}
  }
}

fn reorder_consonant_syllable(
  plan: &KhmerShapePlan,
  start: usize,
  end: usize,
  buffer: &mut hb_buffer_t,
) {
  {
    let mask = plan.mask_array[khmer_feature::BLWF]
      | plan.mask_array[khmer_feature::ABVF]
      | plan.mask_array[khmer_feature::PSTF];
    for info in &mut buffer.info[start + 1..end] {
      info.mask |= mask;
    }
  }

  let mut num_coengs = 0;
  for i in start + 1..end {
    match buffer.info[i].indic_category() == ot_category_t::OT_H && num_coengs <= 2 && i + 1 < end
    {
      true => {
        num_coengs += 1;

        match buffer.info[i + 1].indic_category() == ot_category_t::OT_Ra {
          true => {
            for j in 0..2 {
              buffer.info[i + j].mask |= plan.mask_array[khmer_feature::PREF];
            }

            buffer.merge_clusters(start, i + 2);
            let t0 = buffer.info[i];
            let t1 = buffer.info[i + 1];
            for k in (0..i - start).rev() {
              buffer.info[k + start + 2] = buffer.info[k + start];
            }

            buffer.info[start] = t0;
            buffer.info[start + 1] = t1;

            match plan.mask_array[khmer_feature::CFAR] != 0 {
              true => {
                for j in i + 2..end {
                  buffer.info[j].mask |= plan.mask_array[khmer_feature::CFAR];
                }
              }
              false => {}
            }

            num_coengs = 2;
          }
          false => {}
        }
      }
      false => match buffer.info[i].indic_category() == ot_category_t::OT_VPre {
        true => {
          buffer.merge_clusters(start, i + 1);
          let t = buffer.info[i];
          for k in (0..i - start).rev() {
            buffer.info[k + start + 1] = buffer.info[k + start];
          }
          buffer.info[start] = t;
        }
        false => {}
      },
    }
  }
}

fn override_features(planner: &mut hb_ot_shape_planner_t) {
  planner.ot_map.enable_feature(
    {
      let Ok(value) = hb_tag_t::new(b"clig");
      value
    },
    F_NONE,
    1,
  );

  planner.ot_map.disable_feature({
    let Ok(value) = hb_tag_t::new(b"liga");
    value
  });
}

fn decompose(
  _: &hb_ot_shape_normalize_context_t,
  ab: Codepoint,
) -> Option<(Codepoint, Codepoint)> {
  match ab {
    0x17BE | 0x17BF | 0x17C0 | 0x17C4 | 0x17C5 => Some((0x17C1, ab)),
    _ => crate::hb::unicode::decompose(ab),
  }
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

fn setup_masks(_: &hb_ot_shape_plan_t, _: &mut FontFuncsDispatch, buffer: &mut hb_buffer_t) {
  buffer.allocate_var(GlyphInfo::KHMER_CATEGORY_VAR);

  for info in buffer.info_slice_mut() {
    info.set_khmer_properties();
  }
}
