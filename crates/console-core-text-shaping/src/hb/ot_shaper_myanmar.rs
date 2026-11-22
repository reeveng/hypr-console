use super::buffer::*;
use super::font_funcs::FontFuncsDispatch;
use super::ot_map::*;
use super::ot_shape::*;
use super::ot_shape_normalize::*;
use super::ot_shape_plan::hb_ot_shape_plan_t;
use super::ot_shaper::*;
use super::ot_shaper_indic::{ot_category_t, ot_position_t};
use super::ot_shaper_syllabic::*;
use super::{GlyphInfo, hb_tag_t};
use crate::hb::ot_shaper_indic::ot_category_t::OT_VPre;

pub const MYANMAR_SHAPER: hb_ot_shaper_t = hb_ot_shaper_t {
  collect_features: Some(collect_features),
  override_features: None,
  create_data: None,
  preprocess_text: None,
  postprocess_glyphs: None,
  normalization_preference: HB_OT_SHAPE_NORMALIZATION_MODE_COMPOSED_DIACRITICS_NO_SHORT_CIRCUIT,
  decompose: None,
  compose: None,
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
    MYANMAR_CATEGORY_VAR,
    myanmar_category,
    set_myanmar_category
  );
  declare_buffer_var_alias!(
    OT_SHAPER_VAR_U8_AUXILIARY_VAR,
    u8,
    MYANMAR_POSITION_VAR,
    myanmar_position,
    set_myanmar_position
  );
}

pub const MYANMAR_ZAWGYI_SHAPER: hb_ot_shaper_t = hb_ot_shaper_t {
  collect_features: None,
  override_features: None,
  create_data: None,
  preprocess_text: None,
  postprocess_glyphs: None,
  normalization_preference: HB_OT_SHAPE_NORMALIZATION_MODE_NONE,
  decompose: None,
  compose: None,
  setup_masks: None,
  gpos_tag: None,
  reorder_marks: None,
  zero_width_marks: HB_OT_SHAPE_ZERO_WIDTH_MARKS_NONE,
  fallback_position: false,
};

static MYANMAR_FEATURES: &[hb_tag_t] = &[
  ({
    let Ok(value) = hb_tag_t::new(b"rphf");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"pref");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"blwf");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"pstf");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"pres");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"abvs");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"blws");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"psts");
    value
  }),
];

impl GlyphInfo {
  fn set_myanmar_properties(&mut self) {
    let u = self.glyph_id;
    let (cat, _) = crate::hb::ot_shaper_indic_table::get_categories(u);

    self.set_myanmar_category(cat);
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

  planner.ot_map.add_gsub_pause(Some(reorder_myanmar));

  for feature in MYANMAR_FEATURES.iter().take(4) {
    planner
      .ot_map
      .enable_feature(*feature, F_MANUAL_ZWJ | F_PER_SYLLABLE, 1);
    planner.ot_map.add_gsub_pause(None);
  }

  planner.ot_map.add_gsub_pause(Some(syllabic_clear_var));

  for feature in MYANMAR_FEATURES.iter().skip(4) {
    planner.ot_map.enable_feature(*feature, F_MANUAL_ZWJ, 1);
  }
}

fn setup_syllables(
  _: &hb_ot_shape_plan_t,
  _: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) -> bool {
  buffer.allocate_var(GlyphInfo::SYLLABLE_VAR);

  super::ot_shaper_myanmar_machine::find_syllables_myanmar(buffer);

  let mut start = 0;
  let mut end = buffer.next_syllable(0);
  while start < buffer.len {
    buffer.unsafe_to_break(Some(start), Some(end));
    start = end;
    end = buffer.next_syllable(start);
  }

  false
}

fn reorder_myanmar(
  _: &hb_ot_shape_plan_t,
  face: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) -> bool {
  use super::ot_shaper_myanmar_machine::SyllableType;

  let mut ret = false;

  match insert_dotted_circles(
    face,
    buffer,
    SyllableType::BrokenCluster as u8,
    ot_category_t::OT_DOTTEDCIRCLE,
    None,
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
    reorder_syllable_myanmar(start, end, buffer);
    start = end;
    end = buffer.next_syllable(start);
  }

  buffer.deallocate_var(GlyphInfo::MYANMAR_CATEGORY_VAR);
  buffer.deallocate_var(GlyphInfo::MYANMAR_POSITION_VAR);

  ret
}

fn reorder_syllable_myanmar(start: usize, end: usize, buffer: &mut hb_buffer_t) {
  use super::ot_shaper_myanmar_machine::SyllableType;

  let syllable_type = match buffer.info[start].syllable() & 0x0F {
    0 => SyllableType::ConsonantSyllable,
    1 => SyllableType::PunctuationCluster,
    2 => SyllableType::BrokenCluster,
    3 => SyllableType::NonMyanmarCluster,
    _ => unreachable!(),
  };

  match syllable_type {
    SyllableType::ConsonantSyllable | SyllableType::BrokenCluster => {
      initial_reordering_consonant_syllable(start, end, buffer);
    }
    SyllableType::PunctuationCluster | SyllableType::NonMyanmarCluster => {}
  }
}

fn initial_reordering_consonant_syllable(start: usize, end: usize, buffer: &mut hb_buffer_t) {
  let mut base = end;
  let mut has_reph = false;

  {
    let mut limit = start;
    match start + 3 <= end
      && buffer.info[start + 0].myanmar_category() == ot_category_t::OT_Ra
      && buffer.info[start + 1].myanmar_category() == ot_category_t::OT_As
      && buffer.info[start + 2].myanmar_category() == ot_category_t::OT_H
    {
      true => {
        limit += 3;
        base = start;
        has_reph = true;
      }
      false => {}
    }

    {
      match !has_reph {
        true => {
          base = limit;
        }
        false => {}
      }

      for i in limit..end {
        match buffer.info[i].is_consonant() {
          true => {
            base = i;
            break;
          }
          false => {}
        }
      }
    }
  }

  {
    let mut i = start;
    while i
      < start
        + match has_reph {
          true => 3,
          false => 0,
        }
    {
      buffer.info[i].set_myanmar_position(ot_position_t::POS_AFTER_MAIN);
      i += 1;
    }

    while i < base {
      buffer.info[i].set_myanmar_position(ot_position_t::POS_PRE_C);
      i += 1;
    }

    match i < end {
      true => {
        buffer.info[i].set_myanmar_position(ot_position_t::POS_BASE_C);
        i += 1;
      }
      false => {}
    }

    let mut pos = ot_position_t::POS_AFTER_MAIN;

    for i in i..end {
      match buffer.info[i].myanmar_category() == ot_category_t::OT_MR {
        true => {
          buffer.info[i].set_myanmar_position(ot_position_t::POS_PRE_C);
          continue;
        }
        false => {}
      }

      match buffer.info[i].myanmar_category() == OT_VPre {
        true => {
          buffer.info[i].set_myanmar_position(ot_position_t::POS_PRE_M);
          continue;
        }
        false => {}
      }

      match buffer.info[i].myanmar_category() == ot_category_t::OT_VS {
        true => {
          let t = buffer.info[i - 1].myanmar_position();
          buffer.info[i].set_myanmar_position(t);
          continue;
        }
        false => {}
      }

      match pos == ot_position_t::POS_AFTER_MAIN
        && buffer.info[i].myanmar_category() == ot_category_t::OT_VBlw
      {
        true => {
          pos = ot_position_t::POS_BELOW_C;
          buffer.info[i].set_myanmar_position(pos);
          continue;
        }
        false => {}
      }

      match pos == ot_position_t::POS_BELOW_C
        && buffer.info[i].myanmar_category() == ot_category_t::OT_A
      {
        true => {
          buffer.info[i].set_myanmar_position(ot_position_t::POS_BEFORE_SUB);
          continue;
        }
        false => {}
      }

      match pos == ot_position_t::POS_BELOW_C
        && buffer.info[i].myanmar_category() == ot_category_t::OT_VBlw
      {
        true => {
          buffer.info[i].set_myanmar_position(pos);
          continue;
        }
        false => {}
      }

      match pos == ot_position_t::POS_BELOW_C
        && buffer.info[i].myanmar_category() != ot_category_t::OT_A
      {
        true => {
          pos = ot_position_t::POS_AFTER_SUB;
          buffer.info[i].set_myanmar_position(pos);
          continue;
        }
        false => {}
      }

      buffer.info[i].set_myanmar_position(pos);
    }
  }

  buffer.sort(start, end, |a, b| {
    a.myanmar_position().cmp(&b.myanmar_position()) == core::cmp::Ordering::Greater
  });

  let mut first_left_matra = end;
  let mut last_left_matra = end;

  for i in start..end {
    match buffer.info[i].myanmar_position() == ot_position_t::POS_PRE_M {
      true => {
        match first_left_matra == end {
          true => {
            first_left_matra = i;
          }
          false => {}
        }

        last_left_matra = i;
      }
      false => {}
    }
  }

  match first_left_matra < last_left_matra {
    true => {
      buffer.reverse_range(first_left_matra, last_left_matra + 1);

      let mut i = first_left_matra;

      for j in i..=last_left_matra {
        match buffer.info[j].myanmar_category() == OT_VPre {
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
}

fn setup_masks(_: &hb_ot_shape_plan_t, _: &mut FontFuncsDispatch, buffer: &mut hb_buffer_t) {
  buffer.allocate_var(GlyphInfo::MYANMAR_CATEGORY_VAR);
  buffer.allocate_var(GlyphInfo::MYANMAR_POSITION_VAR);

  for info in buffer.info_slice_mut() {
    info.set_myanmar_properties();
  }
}
