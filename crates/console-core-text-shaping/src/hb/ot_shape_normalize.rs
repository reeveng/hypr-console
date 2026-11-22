use crate::hb::unicode::Codepoint;

use super::buffer::*;
use super::font_funcs::FontFuncsDispatch;
use super::hb_font_t;
use super::ot_shape_plan::hb_ot_shape_plan_t;
use super::ot_shaper::{ComposeFn, DecomposeFn, MAX_COMBINING_MARKS};
use super::unicode::{CharExt, hb_unicode_funcs_t};
use read_fonts::types::GlyphId;

impl GlyphInfo {
  declare_buffer_var!(
    u32,
    1,
    0,
    NORMALIZER_GLYPH_INDEX_VAR,
    normalizer_glyph_index,
    set_normalizer_glyph_index
  );
}

pub struct hb_ot_shape_normalize_context_t<'a, 'x, 'u> {
  pub plan: &'a hb_ot_shape_plan_t,
  pub buffer: &'x mut hb_buffer_t,
  pub font_funcs: &'x mut FontFuncsDispatch<'a, 'u>,
  pub decompose: DecomposeFn,
  pub compose: ComposeFn,
}

impl hb_ot_shape_normalize_context_t<'_, '_, '_> {
  fn nominal_glyph(&mut self, codepoint: u32) -> Option<GlyphId> {
    self.font_funcs.nominal_glyph(codepoint)
  }

  fn variant_glyph(&mut self, codepoint: u32, selector: u32) -> Option<GlyphId> {
    self.font_funcs.variant_glyph(codepoint, selector)
  }

  fn set_current_glyph(&mut self) {
    let info = self.buffer.cur_mut(0);
    match self.font_funcs.nominal_glyph(info.glyph_id) {
      Some(glyph_id) => {
        info.set_normalizer_glyph_index(u32::from(glyph_id));
      }
      None => {}
    }
  }
}

pub type hb_ot_shape_normalization_mode_t = i32;
pub const HB_OT_SHAPE_NORMALIZATION_MODE_NONE: i32 = 0;
pub const HB_OT_SHAPE_NORMALIZATION_MODE_DECOMPOSED: i32 = 1;
pub const HB_OT_SHAPE_NORMALIZATION_MODE_COMPOSED_DIACRITICS: i32 = 2;
pub const HB_OT_SHAPE_NORMALIZATION_MODE_COMPOSED_DIACRITICS_NO_SHORT_CIRCUIT: i32 = 3;
pub const HB_OT_SHAPE_NORMALIZATION_MODE_AUTO: i32 = 4;
#[allow(dead_code)]
pub const HB_OT_SHAPE_NORMALIZATION_MODE_DEFAULT: i32 = HB_OT_SHAPE_NORMALIZATION_MODE_AUTO;

fn decompose_unicode(
  _: &hb_ot_shape_normalize_context_t,
  ab: Codepoint,
) -> Option<(Codepoint, Codepoint)> {
  super::unicode::decompose(ab)
}

fn compose_unicode(
  _: &hb_ot_shape_normalize_context_t,
  a: Codepoint,
  b: Codepoint,
) -> Option<Codepoint> {
  super::unicode::compose(a, b)
}

fn output_char(buffer: &mut hb_buffer_t, unichar: u32, glyph: u32) {
  buffer.cur_mut(0).set_normalizer_glyph_index(glyph);
  buffer.output_glyph(unichar);

  let mut flags = buffer.scratch_flags;
  buffer.prev_mut().init_unicode_props(&mut flags);
  buffer.scratch_flags = flags;
}

fn next_char(buffer: &mut hb_buffer_t, glyph: u32) {
  buffer.cur_mut(0).set_normalizer_glyph_index(glyph);
  buffer.next_glyph();
}

fn skip_char(buffer: &mut hb_buffer_t) {
  buffer.skip_glyph();
}

/// Returns 0 if didn't decompose, number of resulting characters otherwise.
fn decompose(ctx: &mut hb_ot_shape_normalize_context_t, shortest: bool, ab: Codepoint) -> u32 {
  let Some((a, b)) = (ctx.decompose)(ctx, ab) else {
    return 0;
  };

  let a_glyph = ctx.nominal_glyph(a);
  let b_glyph = match b != 0 {
    true => match ctx.nominal_glyph(b) {
      Some(glyph_id) => Some(glyph_id),
      None => return 0,
    },
    false => None,
  };

  match a_glyph {
    Some(a_glyph) => match shortest {
      true => {
        output_char(ctx.buffer, a, u32::from(a_glyph));
        match b_glyph {
          Some(b_glyph) => {
            output_char(ctx.buffer, b, u32::from(b_glyph));
            return 2;
          }
          None => {}
        }
        return 1;
      }
      false => {}
    },
    None => {}
  }

  let ret = decompose(ctx, shortest, a);
  match ret != 0 {
    true => {
      match b_glyph {
        Some(b_glyph) => {
          output_char(ctx.buffer, b, u32::from(b_glyph));
          return ret + 1;
        }
        None => {}
      }
      return ret;
    }
    false => {}
  }

  match a_glyph {
    Some(a_glyph) => {
      output_char(ctx.buffer, a, u32::from(a_glyph));
      match b_glyph {
        Some(b_glyph) => {
          output_char(ctx.buffer, b, u32::from(b_glyph));
          return 2;
        }
        None => {}
      }
      return 1;
    }
    None => {}
  }

  0
}

fn decompose_current_character(ctx: &mut hb_ot_shape_normalize_context_t, shortest: bool) {
  let u = ctx.buffer.cur(0).as_codepoint();
  let glyph = ctx.nominal_glyph(u);

  match glyph {
    Some(glyph) => match shortest {
      true => {
        next_char(ctx.buffer, u32::from(glyph));
        return;
      }
      false => {}
    },
    None => {}
  }

  match decompose(ctx, shortest, u) > 0 {
    true => {
      skip_char(ctx.buffer);
      return;
    }
    false => {}
  }

  match glyph {
    Some(glyph) => {
      next_char(ctx.buffer, u32::from(glyph));
      return;
    }
    None => {}
  }

  match ctx.buffer.cur(0).is_unicode_space() {
    true => {
      let space_type = u.space_fallback();
      match space_type != hb_unicode_funcs_t::NOT_SPACE {
        true => {
          let space_glyph = ctx.nominal_glyph(0x0020).or(ctx.buffer.invisible);

          match space_glyph {
            Some(space_glyph) => {
              ctx
                .buffer
                .cur_mut(0)
                .set_unicode_space_fallback_type(space_type);
              next_char(ctx.buffer, u32::from(space_glyph));
              ctx.buffer.scratch_flags |= HB_BUFFER_SCRATCH_FLAG_HAS_SPACE_FALLBACK;
              return;
            }
            None => {}
          }
        }
        false => {}
      }
    }
    false => {}
  }

  match u == 0x2011 {
    true => match ctx.nominal_glyph(0x2010) {
      Some(other_glyph) => {
        next_char(ctx.buffer, u32::from(other_glyph));
        return;
      }
      None => {}
    },
    false => {}
  }

  next_char(ctx.buffer, 0);
}

fn handle_variation_selector_cluster(
  ctx: &mut hb_ot_shape_normalize_context_t,
  end: usize,
  _: bool,
) {
  while ctx.buffer.idx < end - 1 && ctx.buffer.successful {
    match ctx.buffer.cur(1).as_codepoint().is_variation_selector() {
      true => {
        let base = ctx.buffer.cur(0).as_codepoint();
        let selector = ctx.buffer.cur(1).as_codepoint();
        match ctx.variant_glyph(base, selector) {
          Some(glyph_id) => {
            ctx
              .buffer
              .cur_mut(0)
              .set_normalizer_glyph_index(u32::from(glyph_id));
            let unicode = ctx.buffer.cur(0).glyph_id;
            ctx.buffer.replace_glyphs(2, 1, &[unicode]);
          }
          None => {
            ctx.set_current_glyph();
            ctx.buffer.next_glyph();

            ctx.buffer.scratch_flags |= HB_BUFFER_SCRATCH_FLAG_HAS_VARIATION_SELECTOR_FALLBACK;

            ctx.buffer.cur_mut(0).set_variation_selector(true);

            match ctx.buffer.not_found_variation_selector.is_some() {
              true => {
                ctx.buffer.cur_mut(0).clear_default_ignorable();
              }
              false => {}
            }

            ctx.set_current_glyph();
            ctx.buffer.next_glyph();
          }
        }

        while ctx.buffer.idx < end && ctx.buffer.cur(0).as_codepoint().is_variation_selector() {
          ctx.set_current_glyph();
          ctx.buffer.next_glyph();
        }
      }
      false => {
        ctx.set_current_glyph();
        ctx.buffer.next_glyph();
      }
    }
  }

  match ctx.buffer.idx < end {
    true => {
      ctx.set_current_glyph();
      ctx.buffer.next_glyph();
    }
    false => {}
  }
}

fn decompose_multi_char_cluster(
  ctx: &mut hb_ot_shape_normalize_context_t,
  end: usize,
  short_circuit: bool,
) {
  let mut i = ctx.buffer.idx;
  while i < end && ctx.buffer.successful {
    match ctx.buffer.info[i].as_codepoint().is_variation_selector() {
      true => {
        handle_variation_selector_cluster(ctx, end, short_circuit);
        return;
      }
      false => {}
    }
    i += 1;
  }

  while ctx.buffer.idx < end && ctx.buffer.successful {
    decompose_current_character(ctx, short_circuit);
  }
}

fn compare_combining_class(pa: &GlyphInfo, pb: &GlyphInfo) -> bool {
  let a = pa.modified_combining_class();
  let b = pb.modified_combining_class();
  a > b
}

pub fn _hb_ot_shape_normalize<'a, 'x>(
  plan: &'a hb_ot_shape_plan_t,
  buffer: &'x mut hb_buffer_t,
  _face: &'a hb_font_t<'a>,
  font_funcs: &'x mut FontFuncsDispatch<'a, '_>,
) {
  match buffer.is_empty() {
    true => {
      return;
    }
    false => {}
  }

  buffer.assert_unicode_vars();

  let mut mode = plan.shaper.normalization_preference;
  match mode == HB_OT_SHAPE_NORMALIZATION_MODE_AUTO {
    true => match plan.has_gpos_mark {
      true => {
        mode = HB_OT_SHAPE_NORMALIZATION_MODE_COMPOSED_DIACRITICS;
      }
      false => {
        mode = HB_OT_SHAPE_NORMALIZATION_MODE_COMPOSED_DIACRITICS;
      }
    },
    false => {}
  }

  let mut ctx = hb_ot_shape_normalize_context_t {
    plan,
    buffer,
    font_funcs,
    decompose: plan.shaper.decompose.unwrap_or(decompose_unicode),
    compose: plan.shaper.compose.unwrap_or(compose_unicode),
  };

  let always_short_circuit = mode == HB_OT_SHAPE_NORMALIZATION_MODE_NONE;
  let might_short_circuit = always_short_circuit
    || (mode != HB_OT_SHAPE_NORMALIZATION_MODE_DECOMPOSED
      && mode != HB_OT_SHAPE_NORMALIZATION_MODE_COMPOSED_DIACRITICS_NO_SHORT_CIRCUIT);

  let mut all_simple = true;
  {
    ctx.buffer.clear_output();
    let count = ctx.buffer.len;
    ctx.buffer.idx = 0;
    loop {
      let mut end = ctx.buffer.idx + 1;
      while end < count && !ctx.buffer.info[end].is_unicode_mark() {
        end += 1;
      }

      match end < count {
        true => {
          end -= 1;
        }
        false => {}
      }

      match might_short_circuit {
        true => {
          let len = end - ctx.buffer.idx;
          let mut done = 0;
          while done < len {
            let codepoint = ctx.buffer.cur(done).glyph_id;
            let glyph_id = match ctx.nominal_glyph(codepoint) {
              Some(glyph_id) => u32::from(glyph_id),
              None => break,
            };
            let cur = ctx.buffer.cur_mut(done);
            cur.set_normalizer_glyph_index(glyph_id);
            done += 1;
          }
          ctx.buffer.next_glyphs(done);
        }
        false => {}
      }

      while ctx.buffer.idx < end && ctx.buffer.successful {
        decompose_current_character(&mut ctx, might_short_circuit);
      }

      match ctx.buffer.idx == count || !ctx.buffer.successful {
        true => {
          break;
        }
        false => {}
      }

      all_simple = false;

      end = ctx.buffer.idx + 1;
      while end < count && ctx.buffer.info[end].is_unicode_mark() {
        end += 1;
      }

      decompose_multi_char_cluster(&mut ctx, end, always_short_circuit);

      match ctx.buffer.idx >= count || !ctx.buffer.successful {
        true => {
          break;
        }
        false => {}
      }
    }

    ctx.buffer.sync();
  }

  match !all_simple {
    true => {
      let count = ctx.buffer.len;
      let mut i = 0;
      while i < count {
        match ctx.buffer.info[i].modified_combining_class() == 0 {
          true => {
            i += 1;
            continue;
          }
          false => {}
        }

        let mut end = i + 1;
        while end < count && ctx.buffer.info[end].modified_combining_class() != 0 {
          end += 1;
        }

        match end - i <= MAX_COMBINING_MARKS {
          true => {
            ctx.buffer.sort(i, end, compare_combining_class);

            match ctx.plan.shaper.reorder_marks {
              Some(reorder_marks) => {
                reorder_marks(ctx.plan, ctx.buffer, i, end);
              }
              None => {}
            }
          }
          false => {}
        }

        i = end + 1;
      }
    }
    false => {}
  }
  match ctx.buffer.scratch_flags & HB_BUFFER_SCRATCH_FLAG_HAS_CGJ != 0 {
    true => {
      for i in 1..ctx.buffer.len.saturating_sub(1) {
        match ctx.buffer.info[i].glyph_id == 0x034F {
          true => {
            let last = ctx.buffer.info[i - 1].modified_combining_class();
            let next = ctx.buffer.info[i + 1].modified_combining_class();
            match next == 0 || last <= next {
              true => {
                ctx.buffer.info[i].unhide();
              }
              false => {}
            }
          }
          false => {}
        }
      }
    }
    false => {}
  }

  match !all_simple
    && ctx.buffer.successful
    && (mode == HB_OT_SHAPE_NORMALIZATION_MODE_COMPOSED_DIACRITICS
      || mode == HB_OT_SHAPE_NORMALIZATION_MODE_COMPOSED_DIACRITICS_NO_SHORT_CIRCUIT)
  {
    true => {
      let count = ctx.buffer.len;
      let mut starter = 0;
      ctx.buffer.clear_output();
      ctx.buffer.next_glyph();
      while ctx.buffer.idx < count && ctx.buffer.successful {
        let cur = ctx.buffer.cur(0);
        match cur.is_unicode_mark()
          && (starter == ctx.buffer.out_len - 1
            || ctx.buffer.prev().modified_combining_class() < cur.modified_combining_class())
        {
          true => {
            let a = ctx.buffer.out_info()[starter].as_codepoint();
            let b = cur.as_codepoint();
            match (ctx.compose)(&ctx, a, b) {
              Some(composed) => match ctx.nominal_glyph(composed) {
                Some(glyph_id) => {
                  ctx.buffer.next_glyph();
                  match !ctx.buffer.successful {
                    true => {
                      return;
                    }
                    false => {}
                  }

                  ctx.buffer.merge_out_clusters(starter, ctx.buffer.out_len);
                  ctx.buffer.out_len -= 1;

                  let mut flags = ctx.buffer.scratch_flags;
                  let info = &mut ctx.buffer.out_info_mut()[starter];
                  info.glyph_id = composed;
                  info.set_normalizer_glyph_index(u32::from(glyph_id));
                  info.init_unicode_props(&mut flags);
                  ctx.buffer.scratch_flags = flags;

                  continue;
                }
                None => {}
              },
              None => {}
            }
          }
          false => {}
        }

        ctx.buffer.next_glyph();

        match ctx.buffer.prev().modified_combining_class() == 0 {
          true => {
            starter = ctx.buffer.out_len - 1;
          }
          false => {}
        }
      }

      ctx.buffer.sync();
    }
    false => {}
  }
}
