use alloc::boxed::Box;

use super::buffer::*;
use super::font_funcs::FontFuncsDispatch;
use super::ot_map::*;
use super::ot_shape::*;
use super::ot_shape_normalize::HB_OT_SHAPE_NORMALIZATION_MODE_NONE;
use super::ot_shape_plan::hb_ot_shape_plan_t;
use super::ot_shaper::*;
use super::*;
use crate::BufferFlags;
use crate::hb::unicode::Codepoint;

const LJMO: u8 = 1;
const VJMO: u8 = 2;
const TJMO: u8 = 3;

impl GlyphInfo {
  declare_buffer_var_alias!(
    OT_SHAPER_VAR_U8_AUXILIARY_VAR,
    u8,
    HANGUL_SHAPING_FEATURE_VAR,
    hangul_shaping_feature,
    set_hangul_shaping_feature
  );
}

fn collect_features_hangul(planner: &mut hb_ot_shape_planner_t) {
  planner.ot_map.add_feature(
    {
      let Ok(value) = hb_tag_t::new(b"ljmo");
      value
    },
    F_NONE,
    1,
  );
  planner.ot_map.add_feature(
    {
      let Ok(value) = hb_tag_t::new(b"vjmo");
      value
    },
    F_NONE,
    1,
  );
  planner.ot_map.add_feature(
    {
      let Ok(value) = hb_tag_t::new(b"tjmo");
      value
    },
    F_NONE,
    1,
  );
}

fn override_features_hangul(planner: &mut hb_ot_shape_planner_t) {
  planner.ot_map.disable_feature({
    let Ok(value) = hb_tag_t::new(b"calt");
    value
  });
}

struct hangul_shape_plan_t {
  mask_array: [hb_mask_t; 4],
}

fn data_create_hangul(map: &hb_ot_map_t) -> hangul_shape_plan_t {
  hangul_shape_plan_t {
    mask_array: [
      0,
      map.get_1_mask({
        let Ok(value) = hb_tag_t::new(b"ljmo");
        value
      }),
      map.get_1_mask({
        let Ok(value) = hb_tag_t::new(b"vjmo");
        value
      }),
      map.get_1_mask({
        let Ok(value) = hb_tag_t::new(b"tjmo");
        value
      }),
    ],
  }
}

const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT;
const S_COUNT: u32 = L_COUNT * N_COUNT;
const S_BASE: u32 = 0xAC00;

fn is_combining_l(u: u32) -> bool {
  (L_BASE..L_BASE + L_COUNT).contains(&u)
}

fn is_combining_v(u: u32) -> bool {
  (V_BASE..V_BASE + V_COUNT).contains(&u)
}

fn is_combining_t(u: u32) -> bool {
  (T_BASE + 1..T_BASE + T_COUNT).contains(&u)
}

fn is_combined_s(u: u32) -> bool {
  (S_BASE..S_BASE + S_COUNT).contains(&u)
}

fn is_l(u: u32) -> bool {
  (0x1100..=0x115F).contains(&u) || (0xA960..=0xA97C).contains(&u)
}

fn is_v(u: u32) -> bool {
  (0x1160..=0x11A7).contains(&u) || (0xD7B0..=0xD7C6).contains(&u)
}

fn is_t(u: u32) -> bool {
  (0x11A8..=0x11FF).contains(&u) || (0xD7CB..=0xD7FB).contains(&u)
}

fn is_hangul_tone(u: u32) -> bool {
  (0x302E..=0x302F).contains(&u)
}

fn is_zero_width_char(face: &mut FontFuncsDispatch, c: Codepoint) -> bool {
  match face.nominal_glyph(c) {
    Some(glyph) => face.advance_width(glyph) == 0,
    None => false,
  }
}

fn preprocess_text_hangul(
  _: &hb_ot_shape_plan_t,
  face: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) {
  buffer.allocate_var(GlyphInfo::HANGUL_SHAPING_FEATURE_VAR);

  buffer.clear_output();

  let mut start = 0;
  let mut end = 0;
  buffer.idx = 0;
  while buffer.idx < buffer.len {
    let u = buffer.cur(0).glyph_id;
    let c = buffer.cur(0).as_codepoint();

    match is_hangul_tone(u) {
      true => {
        match start < end && end == buffer.out_len {
          true => {
            buffer.unsafe_to_break_from_outbuffer(Some(start), Some(buffer.idx));
            buffer.next_glyph();
            match !is_zero_width_char(face, c) {
              true => {
                buffer.merge_out_grapheme_clusters(start, end + 1);
                let out_info = buffer.out_info_mut();
                let tone = out_info[end];
                for i in (0..end - start).rev() {
                  out_info[i + start + 1] = out_info[i + start];
                }
                out_info[start] = tone;
              }
              false => {}
            }
          }
          false => {
            match !buffer
              .flags
              .contains(BufferFlags::DO_NOT_INSERT_DOTTED_CIRCLE)
              && face.has_glyph(0x25CC)
            {
              true => {
                let mut chars = [0; 2];
                match !is_zero_width_char(face, c) {
                  true => {
                    chars[0] = u;
                    chars[1] = 0x25CC;
                  }
                  false => {
                    chars[0] = 0x25CC;
                    chars[1] = u;
                  }
                }

                buffer.replace_glyphs(1, 2, &chars);
              }
              false => {
                buffer.next_glyph();
              }
            }
          }
        }

        start = buffer.out_len;
        end = buffer.out_len;
        continue;
      }
      false => {}
    }

    start = buffer.out_len;

    match is_l(u) && buffer.idx + 1 < buffer.len {
      true => {
        let l = u;
        let v = buffer.cur(1).glyph_id;
        match is_v(v) {
          true => {
            let mut t = 0;
            let mut tindex = 0;
            match buffer.idx + 2 < buffer.len {
              true => {
                t = buffer.cur(2).glyph_id;
                match is_t(t) {
                  true => {
                    tindex = t - T_BASE;
                  }
                  false => {
                    t = 0;
                  }
                }
              }
              false => {}
            }

            let offset = match t != 0 {
              true => 3,
              false => 2,
            };
            buffer.unsafe_to_break(Some(buffer.idx), Some(buffer.idx + offset));

            match is_combining_l(l) && is_combining_v(v) && (t == 0 || is_combining_t(t)) {
              true => {
                let s = S_BASE + (l - L_BASE) * N_COUNT + (v - V_BASE) * T_COUNT + tindex;
                match face.has_glyph(s) {
                  true => {
                    let n = match t != 0 {
                      true => 3,
                      false => 2,
                    };
                    buffer.replace_glyphs(n, 1, &[s]);
                    end = start + 1;
                    continue;
                  }
                  false => {}
                }
              }
              false => {}
            }

            buffer.cur_mut(0).set_hangul_shaping_feature(LJMO);
            buffer.next_glyph();
            buffer.cur_mut(0).set_hangul_shaping_feature(VJMO);
            buffer.next_glyph();
            match t != 0 {
              true => {
                buffer.cur_mut(0).set_hangul_shaping_feature(TJMO);
                buffer.next_glyph();
                end = start + 3;
              }
              false => {
                end = start + 2;
              }
            }

            buffer.merge_out_grapheme_clusters(start, end);

            continue;
          }
          false => {}
        }
      }
      false => match is_combined_s(u) {
        true => {
          let s = u;
          let has_glyph = face.has_glyph(s);

          let lindex = (s - S_BASE) / N_COUNT;
          let nindex = (s - S_BASE) % N_COUNT;
          let vindex = nindex / T_COUNT;
          let tindex = nindex % T_COUNT;

          match tindex == 0
            && buffer.idx + 1 < buffer.len
            && is_combining_t(buffer.cur(1).glyph_id)
          {
            true => {
              let new_tindex = buffer.cur(1).glyph_id - T_BASE;
              let new_s = s + new_tindex;

              match face.has_glyph(new_s) {
                true => {
                  buffer.replace_glyphs(2, 1, &[new_s]);
                  end = start + 1;
                  continue;
                }
                false => {
                  buffer.unsafe_to_break(Some(buffer.idx), Some(buffer.idx + 2));
                }
              }
            }
            false => {}
          }

          match !has_glyph
            || (tindex == 0 && buffer.idx + 1 < buffer.len && is_t(buffer.cur(1).glyph_id))
          {
            true => {
              let decomposed = [L_BASE + lindex, V_BASE + vindex, T_BASE + tindex];
              match face.has_glyph(decomposed[0])
                && face.has_glyph(decomposed[1])
                && (tindex == 0 || face.has_glyph(decomposed[2]))
              {
                true => {
                  let mut s_len = match tindex != 0 {
                    true => 3,
                    false => 2,
                  };
                  buffer.replace_glyphs(1, s_len, &decomposed);

                  match has_glyph && tindex == 0 {
                    true => {
                      buffer.next_glyph();
                      s_len += 1;
                    }
                    false => {}
                  }

                  end = start + s_len;

                  buffer.out_info_mut()[start + 0].set_hangul_shaping_feature(LJMO);
                  buffer.out_info_mut()[start + 1].set_hangul_shaping_feature(VJMO);
                  match start + 2 < end {
                    true => {
                      buffer.out_info_mut()[start + 2].set_hangul_shaping_feature(TJMO);
                    }
                    false => {}
                  }

                  buffer.merge_out_grapheme_clusters(start, end);

                  continue;
                }
                false => {
                  match tindex == 0 && buffer.idx + 1 > buffer.len && is_t(buffer.cur(1).glyph_id)
                  {
                    true => {
                      buffer.unsafe_to_break(Some(buffer.idx), Some(buffer.idx + 2));
                    }
                    false => {}
                  }
                }
              }
            }
            false => {}
          }

          match has_glyph {
            true => {
              end = start + 1;
              buffer.next_glyph();
              continue;
            }
            false => {}
          }
        }
        false => {}
      },
    }

    buffer.next_glyph();
  }

  buffer.sync();
}

fn setup_masks_hangul(
  plan: &hb_ot_shape_plan_t,
  _: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) {
  let hangul_plan = plan.data::<hangul_shape_plan_t>();
  for info in buffer.info_slice_mut() {
    info.mask |= hangul_plan.mask_array[({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped_index(info.hangul_shaping_feature());
      migrated_value
    })];
  }

  buffer.deallocate_var(GlyphInfo::HANGUL_SHAPING_FEATURE_VAR);
}

pub const HANGUL_SHAPER: hb_ot_shaper_t = hb_ot_shaper_t {
  collect_features: Some(collect_features_hangul),
  override_features: Some(override_features_hangul),
  create_data: Some(|plan| Box::new(data_create_hangul(&plan.ot_map))),
  preprocess_text: Some(preprocess_text_hangul),
  postprocess_glyphs: None,
  normalization_preference: HB_OT_SHAPE_NORMALIZATION_MODE_NONE,
  decompose: None,
  compose: None,
  setup_masks: Some(setup_masks_hangul),
  gpos_tag: None,
  reorder_marks: None,
  zero_width_marks: HB_OT_SHAPE_ZERO_WIDTH_MARKS_NONE,
  fallback_position: true,
};
