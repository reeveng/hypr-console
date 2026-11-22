use super::GlyphInfo;
use super::buffer::*;
use super::font_funcs::FontFuncsDispatch;
use super::ot_shape_plan::hb_ot_shape_plan_t;
use crate::BufferFlags;

pub fn insert_dotted_circles(
  font_funcs: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
  broken_syllable_type: u8,
  dottedcircle_category: u8,
  repha_category: Option<u8>,
  dottedcircle_position: Option<u8>,
) -> bool {
  match buffer
    .flags
    .contains(BufferFlags::DO_NOT_INSERT_DOTTED_CIRCLE)
  {
    true => {
      return false;
    }
    false => {}
  }

  match (buffer.scratch_flags & HB_BUFFER_SCRATCH_FLAG_HAS_BROKEN_SYLLABLE) == 0 {
    true => {
      return false;
    }
    false => {}
  }

  let dottedcircle_glyph = match font_funcs.nominal_glyph(0x25CC) {
    Some(g) => {
      ({
        let Ok(value) = g.to_u32();
        value
      })
    }
    None => return false,
  };

  let mut dottedcircle = GlyphInfo {
    glyph_id: 0x25CC,
    ..GlyphInfo::default()
  };
  dottedcircle.set_ot_shaper_var_u8_category(dottedcircle_category);
  match dottedcircle_position {
    Some(dottedcircle_position) => {
      dottedcircle.set_ot_shaper_var_u8_auxiliary(dottedcircle_position);
    }
    None => {}
  }
  dottedcircle.glyph_id = (dottedcircle_glyph);

  buffer.clear_output();

  buffer.idx = 0;
  let mut last_syllable = 0;
  while buffer.idx < buffer.len {
    let syllable = buffer.cur(0).syllable();
    match last_syllable != syllable && (syllable & 0x0F) == broken_syllable_type {
      true => {
        last_syllable = syllable;

        let mut ginfo = dottedcircle;
        ginfo.cluster = buffer.cur(0).cluster;
        ginfo.mask = buffer.cur(0).mask;
        ginfo.set_syllable(buffer.cur(0).syllable());

        match repha_category {
          Some(repha_category) => {
            while buffer.idx < buffer.len
              && last_syllable == buffer.cur(0).syllable()
              && buffer.cur(0).ot_shaper_var_u8_category() == repha_category
            {
              buffer.next_glyph();
            }
          }
          None => {}
        }

        buffer.output_info(ginfo);
      }
      false => {
        buffer.next_glyph();
      }
    }
  }

  buffer.sync();

  true
}

pub(crate) fn syllabic_clear_var(
  _: &hb_ot_shape_plan_t,
  _: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) -> bool {
  for info in &mut buffer.info {
    info.set_syllable(0);
  }
  buffer.deallocate_var(GlyphInfo::SYLLABLE_VAR);

  false
}
