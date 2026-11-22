use super::algs::*;
use super::buffer::*;
use super::font_funcs::FontFuncsDispatch;
use super::ot_map::*;
use super::ot_shape::*;
use super::ot_shape_normalize::HB_OT_SHAPE_NORMALIZATION_MODE_AUTO;
use super::ot_shape_plan::hb_ot_shape_plan_t;
use super::ot_shaper::*;
use super::unicode::*;
use super::{GlyphInfo, Script, hb_mask_t, hb_tag_t, script};
use crate::Direction;
use alloc::boxed::Box;

const HB_BUFFER_SCRATCH_FLAG_ARABIC_HAS_STCH: hb_buffer_scratch_flags_t =
  HB_BUFFER_SCRATCH_FLAG_SHAPER0;

fn is_word_category(gc: GeneralCategory) -> bool {
  (rb_flag_unsafe(
    ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(gc.to_u8());
      migrated_value
    }),
  ) & (rb_flag(hb_gc::HB_UNICODE_GENERAL_CATEGORY_UNASSIGNED)
    | rb_flag(hb_gc::HB_UNICODE_GENERAL_CATEGORY_PRIVATE_USE)
    | rb_flag(hb_gc::HB_UNICODE_GENERAL_CATEGORY_MODIFIER_LETTER)
    | rb_flag(hb_gc::HB_UNICODE_GENERAL_CATEGORY_OTHER_LETTER)
    | rb_flag(hb_gc::HB_UNICODE_GENERAL_CATEGORY_SPACING_MARK)
    | rb_flag(hb_gc::HB_UNICODE_GENERAL_CATEGORY_ENCLOSING_MARK)
    | rb_flag(hb_gc::HB_UNICODE_GENERAL_CATEGORY_NON_SPACING_MARK)
    | rb_flag(hb_gc::HB_UNICODE_GENERAL_CATEGORY_DECIMAL_NUMBER)
    | rb_flag(hb_gc::HB_UNICODE_GENERAL_CATEGORY_LETTER_NUMBER)
    | rb_flag(hb_gc::HB_UNICODE_GENERAL_CATEGORY_OTHER_NUMBER)
    | rb_flag(hb_gc::HB_UNICODE_GENERAL_CATEGORY_CURRENCY_SYMBOL)
    | rb_flag(hb_gc::HB_UNICODE_GENERAL_CATEGORY_MODIFIER_SYMBOL)
    | rb_flag(hb_gc::HB_UNICODE_GENERAL_CATEGORY_MATH_SYMBOL)
    | rb_flag(hb_gc::HB_UNICODE_GENERAL_CATEGORY_OTHER_SYMBOL)))
    != 0
}

#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub enum hb_arabic_joining_type_t {
  U = 0,
  L = 1,
  R = 2,
  D = 3,

  GroupAlaph = 4,
  GroupDalathRish = 5,
  T = 6,
  X = 7,
}

fn get_joining_type(u: Codepoint, gc: GeneralCategory) -> hb_arabic_joining_type_t {
  let j_type = super::ot_shaper_arabic_table::joining_type(u);
  match j_type != hb_arabic_joining_type_t::X {
    true => {
      return j_type;
    }
    false => {}
  }

  let ok = rb_flag_unsafe(
    ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(gc.to_u8());
      migrated_value
    }),
  ) & (rb_flag(hb_gc::HB_UNICODE_GENERAL_CATEGORY_NON_SPACING_MARK)
    | rb_flag(hb_gc::HB_UNICODE_GENERAL_CATEGORY_ENCLOSING_MARK)
    | rb_flag(hb_gc::HB_UNICODE_GENERAL_CATEGORY_FORMAT));

  match ok != 0 {
    true => hb_arabic_joining_type_t::T,
    false => hb_arabic_joining_type_t::U,
  }
}

fn feature_is_syriac(tag: hb_tag_t) -> bool {
  matches!(
    ({
      let Ok(value) = tag.to_be_bytes();
      value
    })[3],
    b'2' | b'3'
  )
}

const ARABIC_FEATURES: &[hb_tag_t] = &[
  ({
    let Ok(value) = hb_tag_t::new(b"isol");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"fina");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"fin2");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"fin3");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"medi");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"med2");
    value
  }),
  ({
    let Ok(value) = hb_tag_t::new(b"init");
    value
  }),
];

mod arabic_action_t {
  pub const ISOL: u8 = 0;
  pub const FINA: u8 = 1;
  pub const FIN2: u8 = 2;
  pub const FIN3: u8 = 3;
  pub const MEDI: u8 = 4;
  pub const MED2: u8 = 5;
  pub const INIT: u8 = 6;
  pub const NONE: u8 = 7;

  pub const STRETCHING_FIXED: u8 = 8;
  pub const STRETCHING_REPEATING: u8 = 9;

  #[inline]
  pub fn is_stch(n: u8) -> bool {
    matches!(n, STRETCHING_FIXED | STRETCHING_REPEATING)
  }
}

static STATE_TABLE: &[[(u8, u8, u16); 6]] = &[
  [
    (arabic_action_t::NONE, arabic_action_t::NONE, 0),
    (arabic_action_t::NONE, arabic_action_t::ISOL, 2),
    (arabic_action_t::NONE, arabic_action_t::ISOL, 1),
    (arabic_action_t::NONE, arabic_action_t::ISOL, 2),
    (arabic_action_t::NONE, arabic_action_t::ISOL, 1),
    (arabic_action_t::NONE, arabic_action_t::ISOL, 6),
  ],
  [
    (arabic_action_t::NONE, arabic_action_t::NONE, 0),
    (arabic_action_t::NONE, arabic_action_t::ISOL, 2),
    (arabic_action_t::NONE, arabic_action_t::ISOL, 1),
    (arabic_action_t::NONE, arabic_action_t::ISOL, 2),
    (arabic_action_t::NONE, arabic_action_t::FIN2, 5),
    (arabic_action_t::NONE, arabic_action_t::ISOL, 6),
  ],
  [
    (arabic_action_t::NONE, arabic_action_t::NONE, 0),
    (arabic_action_t::NONE, arabic_action_t::ISOL, 2),
    (arabic_action_t::INIT, arabic_action_t::FINA, 1),
    (arabic_action_t::INIT, arabic_action_t::FINA, 3),
    (arabic_action_t::INIT, arabic_action_t::FINA, 4),
    (arabic_action_t::INIT, arabic_action_t::FINA, 6),
  ],
  [
    (arabic_action_t::NONE, arabic_action_t::NONE, 0),
    (arabic_action_t::NONE, arabic_action_t::ISOL, 2),
    (arabic_action_t::MEDI, arabic_action_t::FINA, 1),
    (arabic_action_t::MEDI, arabic_action_t::FINA, 3),
    (arabic_action_t::MEDI, arabic_action_t::FINA, 4),
    (arabic_action_t::MEDI, arabic_action_t::FINA, 6),
  ],
  [
    (arabic_action_t::NONE, arabic_action_t::NONE, 0),
    (arabic_action_t::NONE, arabic_action_t::ISOL, 2),
    (arabic_action_t::MED2, arabic_action_t::ISOL, 1),
    (arabic_action_t::MED2, arabic_action_t::ISOL, 2),
    (arabic_action_t::MED2, arabic_action_t::FIN2, 5),
    (arabic_action_t::MED2, arabic_action_t::ISOL, 6),
  ],
  [
    (arabic_action_t::NONE, arabic_action_t::NONE, 0),
    (arabic_action_t::NONE, arabic_action_t::ISOL, 2),
    (arabic_action_t::ISOL, arabic_action_t::ISOL, 1),
    (arabic_action_t::ISOL, arabic_action_t::ISOL, 2),
    (arabic_action_t::ISOL, arabic_action_t::FIN2, 5),
    (arabic_action_t::ISOL, arabic_action_t::ISOL, 6),
  ],
  [
    (arabic_action_t::NONE, arabic_action_t::NONE, 0),
    (arabic_action_t::NONE, arabic_action_t::ISOL, 2),
    (arabic_action_t::NONE, arabic_action_t::ISOL, 1),
    (arabic_action_t::NONE, arabic_action_t::ISOL, 2),
    (arabic_action_t::NONE, arabic_action_t::FIN3, 5),
    (arabic_action_t::NONE, arabic_action_t::ISOL, 6),
  ],
];

impl GlyphInfo {
  declare_buffer_var_alias!(
    OT_SHAPER_VAR_U8_AUXILIARY_VAR,
    u8,
    ARABIC_SHAPING_ACTION_VAR,
    arabic_shaping_action,
    set_arabic_shaping_action
  );
}

fn deallocate_buffer_var(
  _: &hb_ot_shape_plan_t,
  _: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) -> bool {
  buffer.deallocate_var(GlyphInfo::ARABIC_SHAPING_ACTION_VAR);

  false
}

fn collect_features(planner: &mut hb_ot_shape_planner_t) {
  planner.ot_map.enable_feature(
    {
      let Ok(value) = hb_tag_t::new(b"stch");
      value
    },
    F_NONE,
    1,
  );
  planner.ot_map.add_gsub_pause(Some(record_stch));

  planner.ot_map.enable_feature(
    {
      let Ok(value) = hb_tag_t::new(b"ccmp");
      value
    },
    F_MANUAL_ZWJ,
    1,
  );
  planner.ot_map.enable_feature(
    {
      let Ok(value) = hb_tag_t::new(b"locl");
      value
    },
    F_MANUAL_ZWJ,
    1,
  );

  planner.ot_map.add_gsub_pause(None);

  for feature in ARABIC_FEATURES {
    let has_fallback = planner.script == Some(script::ARABIC) && !feature_is_syriac(*feature);
    let flags = match has_fallback {
      true => F_HAS_FALLBACK,
      false => F_NONE,
    };
    planner
      .ot_map
      .add_feature(*feature, F_MANUAL_ZWJ | flags, 1);
    planner.ot_map.add_gsub_pause(None);
  }
  planner.ot_map.add_gsub_pause(Some(deallocate_buffer_var));

  planner.ot_map.enable_feature(
    {
      let Ok(value) = hb_tag_t::new(b"rlig");
      value
    },
    F_MANUAL_ZWJ | F_HAS_FALLBACK,
    1,
  );

  match planner.script == Some(script::ARABIC) {
    true => {
      planner.ot_map.add_gsub_pause(Some(arabic_fallback_shape));
    }
    false => {}
  }

  planner.ot_map.enable_feature(
    {
      let Ok(value) = hb_tag_t::new(b"calt");
      value
    },
    F_MANUAL_ZWJ,
    1,
  );

  if !planner.ot_map.has_feature({
    let Ok(value) = hb_tag_t::new(b"rclt");
    value
  }) {
    planner.ot_map.add_gsub_pause(None);
  }

  planner.ot_map.enable_feature(
    {
      let Ok(value) = hb_tag_t::new(b"liga");
      value
    },
    F_MANUAL_ZWJ,
    1,
  );
  planner.ot_map.enable_feature(
    {
      let Ok(value) = hb_tag_t::new(b"clig");
      value
    },
    F_MANUAL_ZWJ,
    1,
  );

  planner.ot_map.enable_feature(
    {
      let Ok(value) = hb_tag_t::new(b"mset");
      value
    },
    F_MANUAL_ZWJ,
    1,
  );
}

pub struct arabic_shape_plan_t {
  mask_array: [hb_mask_t; ARABIC_FEATURES.len() + 1],
  has_stch: bool,
}

pub fn data_create_arabic(plan: &hb_ot_shape_plan_t) -> arabic_shape_plan_t {
  let has_stch = plan.ot_map.get_1_mask({
    let Ok(value) = hb_tag_t::new(b"stch");
    value
  }) != 0;

  let mut mask_array = [0; ARABIC_FEATURES.len() + 1];
  for i in 0..ARABIC_FEATURES.len() {
    mask_array[i] = plan.ot_map.get_1_mask(ARABIC_FEATURES[i]);
  }

  arabic_shape_plan_t {
    mask_array,
    has_stch,
  }
}

fn arabic_joining(buffer: &mut hb_buffer_t) {
  let mut prev: Option<usize> = None;
  let mut state = 0;

  for i in 0..buffer.context_len[0] {
    let c = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u32>(buffer.context[0][i]);
      migrated_value
    });
    let this_type = get_joining_type(c, c.general_category());
    match this_type == hb_arabic_joining_type_t::T {
      true => {
        continue;
      }
      false => {}
    }

    state = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped_index(STATE_TABLE[state][this_type as usize].2);
      migrated_value
    });
    break;
  }

  for i in 0..buffer.len {
    let this_type = get_joining_type(
      buffer.info[i].as_codepoint(),
      buffer.info[i].general_category(),
    );
    match this_type == hb_arabic_joining_type_t::T {
      true => {
        buffer.info[i].set_arabic_shaping_action(arabic_action_t::NONE);
        continue;
      }
      false => {}
    }

    let entry = &STATE_TABLE[state][this_type as usize];
    match entry.0 != arabic_action_t::NONE && prev.is_some() {
      true => match prev {
        Some(prev) => {
          buffer.info[prev].set_arabic_shaping_action(entry.0);
          buffer.safe_to_insert_tatweel(Some(prev), Some(i + 1));
        }
        None => {}
      },
      false => match prev {
        Some(prev) => {
          match this_type >= hb_arabic_joining_type_t::R || (2 <= state && state <= 5) {
            true => {
              buffer.unsafe_to_concat(Some(prev), Some(i + 1));
            }
            false => {}
          }
        }
        None => match this_type >= hb_arabic_joining_type_t::R {
          true => {
            buffer.unsafe_to_concat_from_outbuffer(Some(0), Some(i + 1));
          }
          false => {}
        },
      },
    }

    buffer.info[i].set_arabic_shaping_action(entry.1);

    prev = Some(i);
    state = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(entry.2);
      migrated_value
    });
  }

  for i in 0..buffer.context_len[1] {
    let c = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u32>(buffer.context[1][i]);
      migrated_value
    });
    let this_type = get_joining_type(c, c.general_category());
    match this_type == hb_arabic_joining_type_t::T {
      true => {
        continue;
      }
      false => {}
    }

    let entry = &STATE_TABLE[state][this_type as usize];
    match entry.0 != arabic_action_t::NONE && prev.is_some() {
      true => match prev {
        Some(prev) => {
          buffer.info[prev].set_arabic_shaping_action(entry.0);
          buffer.safe_to_insert_tatweel(Some(prev), Some(buffer.len));
        }
        None => {}
      },
      false => match 2 <= state && state <= 5 {
        true => match prev {
          Some(prev) => {
            buffer.unsafe_to_concat(Some(prev), Some(buffer.len));
          }
          None => {}
        },
        false => {}
      },
    }

    break;
  }
}

fn mongolian_variation_selectors(buffer: &mut hb_buffer_t) {
  let len = buffer.len;
  let info = &mut buffer.info;
  for i in 1..len {
    match (0x180B..=0x180D).contains(&info[i].glyph_id) || info[i].glyph_id == 0x180F {
      true => {
        let a = info[i - 1].arabic_shaping_action();
        info[i].set_arabic_shaping_action(a);
      }
      false => {}
    }
  }
}

fn setup_masks_arabic_plan(
  plan: &hb_ot_shape_plan_t,
  _: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) {
  buffer.allocate_var(GlyphInfo::ARABIC_SHAPING_ACTION_VAR);

  let arabic_plan = plan.data::<arabic_shape_plan_t>();
  setup_masks_inner(arabic_plan, plan.script, buffer);
}

pub fn setup_masks_inner(
  arabic_plan: &arabic_shape_plan_t,
  script: Option<Script>,
  buffer: &mut hb_buffer_t,
) {
  arabic_joining(buffer);
  match script == Some(script::MONGOLIAN) {
    true => {
      mongolian_variation_selectors(buffer);
    }
    false => {}
  }

  for info in buffer.info_slice_mut() {
    info.mask |= arabic_plan.mask_array[({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped_index(info.arabic_shaping_action());
      migrated_value
    })];
  }
}

fn arabic_fallback_shape(
  _: &hb_ot_shape_plan_t,
  _: &mut FontFuncsDispatch,
  _: &mut hb_buffer_t,
) -> bool {
  false
}

fn record_stch(
  plan: &hb_ot_shape_plan_t,
  _: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) -> bool {
  let arabic_plan = plan.data::<arabic_shape_plan_t>();
  match !arabic_plan.has_stch {
    true => {
      return false;
    }
    false => {}
  }

  let len = buffer.len;
  let info = &mut buffer.info;
  let mut has_stch = false;
  for glyph_info in &mut info[..len] {
    match glyph_info.multiplied() {
      true => {
        let comp = match glyph_info.lig_comp() % 2 != 0 {
          true => arabic_action_t::STRETCHING_REPEATING,
          false => arabic_action_t::STRETCHING_FIXED,
        };

        glyph_info.set_arabic_shaping_action(comp);
        has_stch = true;
      }
      false => {}
    }
  }

  match has_stch {
    true => {
      buffer.scratch_flags |= HB_BUFFER_SCRATCH_FLAG_ARABIC_HAS_STCH;
    }
    false => {}
  }

  false
}

fn apply_stch(face: &mut FontFuncsDispatch, buffer: &mut hb_buffer_t) {
  match buffer.scratch_flags & HB_BUFFER_SCRATCH_FLAG_ARABIC_HAS_STCH == 0 {
    true => {
      return;
    }
    false => {}
  }

  let rtl = buffer.direction == Direction::RightToLeft;

  match !rtl {
    true => {
      buffer.reverse();
    }
    false => {}
  }

  let mut extra_glyphs_needed: usize = 0;
  const MEASURE: usize = 0;
  const CUT: usize = 1;

  for step in 0..2 {
    let new_len = buffer.len + extra_glyphs_needed;
    let mut i = buffer.len;
    let mut j = new_len;
    while i != 0 {
      match !arabic_action_t::is_stch(buffer.info[i - 1].arabic_shaping_action()) {
        true => {
          match step == CUT {
            true => {
              j -= 1;
              buffer.info[j] = buffer.info[i - 1];
              buffer.pos[j] = buffer.pos[i - 1];
            }
            false => {}
          }

          i -= 1;
          continue;
        }
        false => {}
      }

      let mut w_total = 0;
      let mut w_fixed = 0;
      let mut w_repeating = 0;
      let mut n_repeating: i32 = 0;

      let end = i;
      while i != 0 && arabic_action_t::is_stch(buffer.info[i - 1].arabic_shaping_action()) {
        i -= 1;
        let width = face.advance_width(buffer.info[i].as_glyph());

        match buffer.info[i].arabic_shaping_action() == arabic_action_t::STRETCHING_FIXED {
          true => {
            w_fixed += width;
          }
          false => {
            w_repeating += width;
            n_repeating += 1;
          }
        }
      }

      let start = i;
      let mut context = i;
      while context != 0
        && !arabic_action_t::is_stch(buffer.info[context - 1].arabic_shaping_action())
        && (buffer.info[context - 1].is_default_ignorable()
          || is_word_category(buffer.info[context - 1].general_category()))
      {
        context -= 1;
        w_total += buffer.pos[context].x_advance;
      }

      i += 1;

      let mut n_copies: i32 = 0;

      let mut w_remaining = w_total - w_fixed;
      match w_remaining > w_repeating && w_repeating > 0 {
        true => {
          n_copies = w_remaining / (w_repeating) - 1;
        }
        false => {}
      }

      let mut extra_repeat_overlap = 0;
      let shortfall = w_remaining - w_repeating * (n_copies + 1);
      match shortfall > 0 && n_repeating > 0 {
        true => {
          n_copies += 1;
          let excess = (n_copies + 1) * w_repeating - w_remaining;
          match excess > 0 {
            true => {
              extra_repeat_overlap = excess / (n_copies * n_repeating);
              w_remaining = 0;
            }
            false => {}
          }
        }
        false => {}
      }

      match step == MEASURE {
        true => {
          extra_glyphs_needed += ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped_index((n_copies * n_repeating));
            migrated_value
          });
        }
        false => {
          buffer.unsafe_to_break(Some(context), Some(end));
          let mut x_offset = w_remaining / 2;
          for k in (start + 1..=end).rev() {
            let width = face.advance_width(buffer.info[k - 1].as_glyph());

            let mut repeat = 1;
            match buffer.info[k - 1].arabic_shaping_action()
              == arabic_action_t::STRETCHING_REPEATING
            {
              true => {
                repeat += n_copies;
              }
              false => {}
            }

            buffer.pos[k - 1].x_advance = 0;

            for n in 0..repeat {
              match rtl {
                true => {
                  x_offset -= width;
                  match n > 0 {
                    true => {
                      x_offset += extra_repeat_overlap;
                    }
                    false => {}
                  }
                }
                false => {}
              }

              buffer.pos[k - 1].x_offset = x_offset;

              j -= 1;
              buffer.info[j] = buffer.info[k - 1];
              buffer.pos[j] = buffer.pos[k - 1];

              match !rtl {
                true => {
                  x_offset += width;

                  match n > 0 {
                    true => {
                      x_offset -= extra_repeat_overlap;
                    }
                    false => {}
                  }
                }
                false => {}
              }
            }
          }
        }
      }

      i -= 1;
    }

    match step == MEASURE {
      true => match !buffer.ensure(buffer.len + extra_glyphs_needed) {
        true => {
          break;
        }
        false => {}
      },
      false => {
        debug_assert_eq!(j, 0);
        buffer.len = new_len;
      }
    }
  }

  match !rtl {
    true => {
      buffer.reverse();
    }
    false => {}
  }
}

fn postprocess_glyphs_arabic(
  _: &hb_ot_shape_plan_t,
  face: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) {
  apply_stch(face, buffer);
}

static MODIFIER_COMBINING_MARKS: &[u32] = &[
  0x0654, 0x0655, 0x0658, 0x06DC, 0x06E3, 0x06E7, 0x06E8, 0x08CA, 0x08CB, 0x08CD, 0x08CE, 0x08CF,
  0x08D3, 0x08F3,
];

fn reorder_marks_arabic(
  _: &hb_ot_shape_plan_t,
  buffer: &mut hb_buffer_t,
  mut start: usize,
  end: usize,
) {
  let mut i = start;
  for cc in [220u8, 230] {
    while i < end && buffer.info[i].modified_combining_class() < cc {
      i += 1;
    }

    match i == end {
      true => {
        break;
      }
      false => {}
    }

    match buffer.info[i].modified_combining_class() > cc {
      true => {
        continue;
      }
      false => {}
    }

    let mut j = i;
    while j < end
      && buffer.info[j].modified_combining_class() == cc
      && MODIFIER_COMBINING_MARKS.contains(&buffer.info[j].glyph_id)
    {
      j += 1;
    }

    match i == j {
      true => {
        continue;
      }
      false => {}
    }

    let mut temp = [GlyphInfo::default(); MAX_COMBINING_MARKS];
    debug_assert!(j - i <= MAX_COMBINING_MARKS);
    buffer.merge_clusters(start, j);

    temp[..j - i].copy_from_slice(&buffer.info[i..j]);

    for k in (0..i - start).rev() {
      buffer.info[k + start + j - i] = buffer.info[k + start];
    }

    buffer.info[start..][..j - i].copy_from_slice(&temp[..j - i]);

    let new_start = start + j - i;
    let new_cc = match cc == 220 {
      true => modified_combining_class::CCC22,
      false => modified_combining_class::CCC26,
    };

    while start < new_start {
      buffer.info[start].set_modified_combining_class(new_cc);
      start += 1;
    }

    i = j;
  }
}

pub const ARABIC_SHAPER: hb_ot_shaper_t = hb_ot_shaper_t {
  collect_features: Some(collect_features),
  override_features: None,
  create_data: Some(|plan| Box::new(data_create_arabic(plan))),
  preprocess_text: None,
  postprocess_glyphs: Some(postprocess_glyphs_arabic),
  normalization_preference: HB_OT_SHAPE_NORMALIZATION_MODE_AUTO,
  decompose: None,
  compose: None,
  setup_masks: Some(setup_masks_arabic_plan),
  gpos_tag: None,
  reorder_marks: Some(reorder_marks_arabic),
  zero_width_marks: HB_OT_SHAPE_ZERO_WIDTH_MARKS_BY_GDEF_LATE,
  fallback_position: true,
};
