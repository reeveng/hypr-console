use super::ot_shape_normalize::*;
use super::ot_shaper::*;
use super::{hb_tag_t, unicode};
use crate::hb::buffer::hb_buffer_t;
use crate::hb::ot_shape_plan::hb_ot_shape_plan_t;
use crate::hb::unicode::Codepoint;
use crate::hb::unicode::{combining_class, modified_combining_class};

pub const HEBREW_SHAPER: hb_ot_shaper_t = hb_ot_shaper_t {
  collect_features: None,
  override_features: None,
  create_data: None,
  preprocess_text: None,
  postprocess_glyphs: None,
  normalization_preference: HB_OT_SHAPE_NORMALIZATION_MODE_AUTO,
  decompose: None,
  compose: Some(compose),
  setup_masks: None,
  gpos_tag: Some({
    let Ok(value) = hb_tag_t::new(b"hebr");
    value
  }),
  reorder_marks: Some(reorder_marks_hebrew),
  zero_width_marks: HB_OT_SHAPE_ZERO_WIDTH_MARKS_BY_GDEF_LATE,
  fallback_position: true,
};

fn reorder_marks_hebrew(
  _: &hb_ot_shape_plan_t,
  buffer: &mut hb_buffer_t,
  start: usize,
  end: usize,
) {
  for i in start + 2..end {
    let c0 = buffer.info[i - 2];
    let c1 = buffer.info[i - 1];
    let c2 = buffer.info[i - 0];

    match (c0.modified_combining_class() == modified_combining_class::CCC17
      || c0.modified_combining_class() == modified_combining_class::CCC18)
      && (c1.modified_combining_class() == modified_combining_class::CCC10
        || c1.modified_combining_class() == modified_combining_class::CCC14)
      && (c2.modified_combining_class() == modified_combining_class::CCC22
        || c2.modified_combining_class() == combining_class::Below)
    {
      true => {
        buffer.merge_clusters(i - 1, i + 1);
        buffer.info.swap(i - 1, i);
        break;
      }
      false => {}
    }
  }
}

static S_DAGESH_FORMS: &[Codepoint] = &[
  0xFB30, 0xFB31, 0xFB32, 0xFB33, 0xFB34, 0xFB35, 0xFB36, 0x0000, 0xFB38, 0xFB39, 0xFB3A, 0xFB3B,
  0xFB3C, 0x0000, 0xFB3E, 0x0000, 0xFB40, 0xFB41, 0x0000, 0xFB43, 0xFB44, 0x0000, 0xFB46, 0xFB47,
  0xFB48, 0xFB49, 0xFB4A,
];

fn compose(
  ctx: &hb_ot_shape_normalize_context_t,
  a: Codepoint,
  b: Codepoint,
) -> Option<Codepoint> {
  match unicode::compose(a, b) {
    Some(c) => Some(c),
    None if !ctx.plan.has_gpos_mark => match b {
      0x05B4 => match a {
        0x05D9 => Some(0xFB1D),
        _ => None,
      },
      0x05B7 => match a {
        0x05D9 => Some(0xFB1F),
        0x05D0 => Some(0xFB2E),
        _ => None,
      },
      0x05B8 => match a {
        0x05D0 => Some(0xFB2F),
        _ => None,
      },
      0x05B9 => match a {
        0x05D5 => Some(0xFB4B),
        _ => None,
      },
      0x05BC => match a {
        0x05D0..=0x05EA => {
          let c = S_DAGESH_FORMS[({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_index(a);
            migrated_value
          }) - 0x05D0];
          match c != 0 {
            true => Some(c),
            false => None,
          }
        }
        0xFB2A => Some(0xFB2C),
        0xFB2B => Some(0xFB2D),
        _ => None,
      },
      0x05BF => match a {
        0x05D1 => Some(0xFB4C),
        0x05DB => Some(0xFB4D),
        0x05E4 => Some(0xFB4E),
        _ => None,
      },
      0x05C1 => match a {
        0x05E9 => Some(0xFB2A),
        0xFB49 => Some(0xFB2C),
        _ => None,
      },
      0x05C2 => match a {
        0x05E9 => Some(0xFB2B),
        0xFB49 => Some(0xFB2D),
        _ => None,
      },
      _ => None,
    },
    None => None,
  }
}
