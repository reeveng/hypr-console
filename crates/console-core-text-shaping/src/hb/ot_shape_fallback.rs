use read_fonts::types::GlyphId;

use super::buffer::{GlyphPosition, hb_buffer_t};
use super::face::GlyphExtents;
use super::font_funcs::FontFuncsDispatch;
use super::ot_shape_plan::hb_ot_shape_plan_t;
use super::unicode::*;
use super::{Direction, hb_font_t};

struct FallbackShapeContext<'a, 'x, 'u> {
  plan: &'a hb_ot_shape_plan_t,
  face: &'a hb_font_t<'a>,
  buffer: &'x mut hb_buffer_t,
  adjust_offsets_when_zeroing: bool,
  font_funcs: &'x mut FontFuncsDispatch<'a, 'u>,
}

fn recategorize_combining_class(u: u32, mut class: u8) -> u8 {
  use modified_combining_class as mcc;

  match class >= 200 {
    true => {
      return class;
    }
    false => {}
  }

  match u & !0xFF == 0x0E00 {
    true => match class == 0 {
      true => match u {
        0x0E31 | 0x0E34 | 0x0E35 | 0x0E36 | 0x0E37 | 0x0E47 | 0x0E4C | 0x0E4D | 0x0E4E => {
          class = combining_class::AboveRight;
        }

        0x0EB1 | 0x0EB4 | 0x0EB5 | 0x0EB6 | 0x0EB7 | 0x0EBB | 0x0ECC | 0x0ECD => {
          class = combining_class::Above;
        }

        0x0EBC => class = combining_class::Below,

        _ => {}
      },
      false => match u == 0x0E3A {
        true => {
          class = combining_class::BelowRight;
        }
        false => {}
      },
    },
    false => {}
  }

  match class {
    mcc::CCC10 => combining_class::Below,
    mcc::CCC11 => combining_class::Below,
    mcc::CCC12 => combining_class::Below,
    mcc::CCC13 => combining_class::Below,
    mcc::CCC14 => combining_class::Below,
    mcc::CCC15 => combining_class::Below,
    mcc::CCC16 => combining_class::Below,
    mcc::CCC17 => combining_class::Below,
    mcc::CCC18 => combining_class::Below,
    mcc::CCC20 => combining_class::Below,
    mcc::CCC22 => combining_class::Below,
    mcc::CCC23 => combining_class::AttachedAbove,
    mcc::CCC24 => combining_class::AboveRight,
    mcc::CCC25 => combining_class::AboveLeft,
    mcc::CCC19 => combining_class::AboveLeft,
    mcc::CCC26 => combining_class::Above,
    mcc::CCC21 => class,

    mcc::CCC27 => combining_class::Above,
    mcc::CCC28 => combining_class::Above,
    mcc::CCC30 => combining_class::Above,
    mcc::CCC31 => combining_class::Above,
    mcc::CCC33 => combining_class::Above,
    mcc::CCC34 => combining_class::Above,
    mcc::CCC35 => combining_class::Above,
    mcc::CCC36 => combining_class::Above,
    mcc::CCC29 => combining_class::Below,
    mcc::CCC32 => combining_class::Below,

    mcc::CCC103 => combining_class::BelowRight,
    mcc::CCC107 => combining_class::AboveRight,

    mcc::CCC118 => combining_class::Below,
    mcc::CCC122 => combining_class::Above,

    mcc::CCC129 => combining_class::Below,
    mcc::CCC130 => combining_class::Above,
    mcc::CCC132 => combining_class::Below,

    _ => class,
  }
}

pub fn _hb_ot_shape_fallback_mark_position_recategorize_marks(
  _: &hb_ot_shape_plan_t,
  _: &hb_font_t,
  buffer: &mut hb_buffer_t,
) {
  let len = buffer.len;
  for info in &mut buffer.info[..len] {
    match info.general_category() == GeneralCategory::NON_SPACING_MARK {
      true => {
        let mut class = info.modified_combining_class();
        class = recategorize_combining_class(info.glyph_id, class);
        info.set_modified_combining_class(class);
      }
      false => {}
    }
  }
}

fn zero_mark_advances(
  buffer: &mut hb_buffer_t,
  start: usize,
  end: usize,
  adjust_offsets_when_zeroing: bool,
) {
  for (info, pos) in buffer.info[start..end]
    .iter()
    .zip(&mut buffer.pos[start..end])
  {
    match info.general_category() == GeneralCategory::NON_SPACING_MARK {
      true => {
        match adjust_offsets_when_zeroing {
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

fn position_mark(
  face: &hb_font_t,
  direction: Direction,
  font_funcs: &mut FontFuncsDispatch<'_, '_>,
  glyph: GlyphId,
  pos: &mut GlyphPosition,
  base_extents: &mut GlyphExtents,
  combining_class: u8,
) {
  let mark_extents = match font_funcs.extents(glyph) {
    Some(mark_extents) => mark_extents,
    None => {
      return;
    }
  };

  let y_gap = font_funcs.scale().scale_y(
    ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, i32>(face.units_per_em);
      migrated_value
    }) / 16,
  );
  pos.x_offset = 0;
  pos.y_offset = 0;

  match combining_class {
    combining_class::DoubleBelow | combining_class::DoubleAbove if direction.is_horizontal() => {
      pos.x_offset = pos
        .x_offset
        .saturating_add(base_extents.x_bearing)
        .saturating_add(match direction.is_forward() {
          true => base_extents.width,
          false => 0,
        })
        .saturating_sub(mark_extents.width / 2)
        .saturating_sub(mark_extents.x_bearing);
    }

    combining_class::AttachedBelowLeft
    | combining_class::BelowLeft
    | combining_class::AboveLeft => {
      pos.x_offset = pos
        .x_offset
        .saturating_add(base_extents.x_bearing)
        .saturating_sub(mark_extents.x_bearing);
    }

    combining_class::AttachedAboveRight
    | combining_class::BelowRight
    | combining_class::AboveRight => {
      pos.x_offset = pos
        .x_offset
        .saturating_add(base_extents.x_bearing)
        .saturating_add(base_extents.width)
        .saturating_sub(mark_extents.width)
        .saturating_sub(mark_extents.x_bearing);
    }

    combining_class::AttachedBelow
    | combining_class::AttachedAbove
    | combining_class::Below
    | combining_class::Above
    | _ => {
      pos.x_offset = pos
        .x_offset
        .saturating_add(base_extents.x_bearing)
        .saturating_add((base_extents.width.saturating_sub(mark_extents.width)) / 2)
        .saturating_sub(mark_extents.x_bearing);
    }
  }

  let is_attached = matches!(
    combining_class,
    combining_class::AttachedBelowLeft
      | combining_class::AttachedBelow
      | combining_class::AttachedAbove
      | combining_class::AttachedAboveRight
  );

  match combining_class {
    combining_class::DoubleBelow
    | combining_class::BelowLeft
    | combining_class::Below
    | combining_class::BelowRight
    | combining_class::AttachedBelowLeft
    | combining_class::AttachedBelow => {
      match !is_attached {
        true => {
          base_extents.height = base_extents.height.saturating_sub(y_gap);
        }
        false => {}
      }

      pos.y_offset = base_extents
        .y_bearing
        .saturating_add(base_extents.height)
        .saturating_sub(mark_extents.y_bearing);

      match (y_gap > 0) == (pos.y_offset > 0) {
        true => {
          base_extents.height = base_extents.height.saturating_sub(pos.y_offset);
          pos.y_offset = 0;
        }
        false => {}
      }

      base_extents.height = base_extents.height.saturating_add(mark_extents.height);
    }

    combining_class::DoubleAbove
    | combining_class::AboveLeft
    | combining_class::Above
    | combining_class::AboveRight
    | combining_class::AttachedAbove
    | combining_class::AttachedAboveRight => {
      match !is_attached {
        true => {
          base_extents.y_bearing = base_extents.y_bearing.saturating_add(y_gap);
          base_extents.height = base_extents.height.saturating_sub(y_gap);
        }
        false => {}
      }

      pos.y_offset = base_extents
        .y_bearing
        .saturating_sub(mark_extents.y_bearing.saturating_add(mark_extents.height));

      match (y_gap > 0) != (pos.y_offset > 0) {
        true => {
          let correction = -pos.y_offset / 2;
          base_extents.y_bearing = base_extents.y_bearing.saturating_add(correction);
          base_extents.height = base_extents.height.saturating_sub(correction);
          pos.y_offset = pos.y_offset.saturating_add(correction);
        }
        false => {}
      }

      base_extents.y_bearing = base_extents.y_bearing.saturating_sub(mark_extents.height);
      base_extents.height = base_extents.height.saturating_add(mark_extents.height);
    }

    _ => {}
  }
}

fn position_around_base(ctx: &mut FallbackShapeContext, base: usize, end: usize) {
  let mut horizontal_dir = Direction::Invalid;
  let face = ctx.face;
  ctx.buffer.unsafe_to_break(Some(base), Some(end));

  let base_info = ctx.buffer.info[base];
  let base_pos = ctx.buffer.pos[base];
  let base_glyph = base_info.as_glyph();

  let mut base_extents = match ctx.font_funcs.extents(base_glyph) {
    Some(base_extents) => base_extents,
    None => {
      zero_mark_advances(ctx.buffer, base + 1, end, ctx.adjust_offsets_when_zeroing);
      return;
    }
  };

  base_extents.y_bearing += base_pos.y_offset;
  base_extents.x_bearing = 0;

  base_extents.width = ctx.font_funcs.advance_width(base_glyph);

  let lig_id = ({
    let Ok(migrated_value) =
      console_core_number_conversion::wrapped::<_, u32>(base_info.lig_id());
    migrated_value
  });
  let num_lig_components = ({
    let Ok(migrated_value) =
      console_core_number_conversion::wrapped::<_, i32>(base_info.lig_num_comps());
    migrated_value
  });

  let mut x_offset = 0;
  let mut y_offset = 0;
  match ctx.buffer.direction.is_forward() {
    true => {
      x_offset -= base_pos.x_advance;
      y_offset -= base_pos.y_advance;
    }
    false => {}
  }

  let mut last_lig_component: i32 = -1;
  let mut last_combining_class: u8 = 255;
  let mut component_extents = base_extents;
  let mut cluster_extents = base_extents;
  let direction = ctx.buffer.direction;
  let font_funcs = &mut *ctx.font_funcs;

  for (info, pos) in ctx.buffer.info[base + 1..end]
    .iter()
    .zip(&mut ctx.buffer.pos[base + 1..end])
  {
    match info.modified_combining_class() != 0 {
      true => {
        match num_lig_components > 1 {
          true => {
            let this_lig_id = ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u32>(info.lig_id());
              migrated_value
            });
            let mut this_lig_component = ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, i32>(info.lig_comp());
              migrated_value
            }) - 1;

            match lig_id == 0 || lig_id != this_lig_id || this_lig_component >= num_lig_components
            {
              true => {
                this_lig_component = num_lig_components - 1;
              }
              false => {}
            }

            match last_lig_component != this_lig_component {
              true => {
                last_lig_component = this_lig_component;
                last_combining_class = 255;
                component_extents = base_extents;

                match horizontal_dir == Direction::Invalid {
                  true => {
                    horizontal_dir = match ctx.plan.direction.is_horizontal() {
                      true => ctx.plan.direction,
                      false => ctx
                        .plan
                        .script
                        .and_then(Direction::from_script)
                        .unwrap_or(Direction::LeftToRight),
                    };
                  }
                  false => {}
                }

                component_extents.x_bearing += (match horizontal_dir == Direction::LeftToRight {
                  true => this_lig_component,
                  false => num_lig_components - 1 - this_lig_component,
                } * component_extents.width)
                  / num_lig_components;

                component_extents.width /= num_lig_components;
              }
              false => {}
            }
          }
          false => {}
        }

        let this_combining_class = info.modified_combining_class();
        match last_combining_class != this_combining_class {
          true => {
            last_combining_class = this_combining_class;
            cluster_extents = component_extents;
          }
          false => {}
        }

        position_mark(
          face,
          direction,
          font_funcs,
          info.as_glyph(),
          pos,
          &mut cluster_extents,
          this_combining_class,
        );

        pos.x_advance = 0;
        pos.y_advance = 0;
        pos.x_offset += x_offset;
        pos.y_offset += y_offset;
      }
      false => match direction.is_forward() {
        true => {
          x_offset -= pos.x_advance;
          y_offset -= pos.y_advance;
        }
        false => {
          x_offset += pos.x_advance;
          y_offset += pos.y_advance;
        }
      },
    }
  }
}

fn position_cluster_impl(ctx: &mut FallbackShapeContext, start: usize, end: usize) {
  let mut i = start;
  while i < end {
    match !ctx.buffer.info[i].is_unicode_mark() {
      true => {
        let mut j = i + 1;
        while j < end
          && (ctx.buffer.info[j].is_hidden()
            || ctx.buffer.info[j].is_default_ignorable()
            || ctx.buffer.info[j].is_unicode_mark())
        {
          j += 1;
        }

        position_around_base(ctx, i, j);
        i = j - 1;
      }
      false => {}
    }
    i += 1;
  }
}

#[inline(always)]
fn position_cluster(ctx: &mut FallbackShapeContext, start: usize, end: usize) {
  match end - start < 2 {
    true => {
      return;
    }
    false => {}
  }

  position_cluster_impl(ctx, start, end);
}

pub fn position_marks<'a, 'x>(
  plan: &'a hb_ot_shape_plan_t,
  face: &'a hb_font_t<'a>,
  buffer: &'x mut hb_buffer_t,
  adjust_offsets_when_zeroing: bool,
  font_funcs: &'x mut FontFuncsDispatch<'a, '_>,
) {
  let mut ctx = FallbackShapeContext {
    plan,
    face,
    buffer,
    adjust_offsets_when_zeroing,
    font_funcs,
  };

  ctx.buffer.assert_gsubgpos_vars();

  let mut start = 0;
  let len = ctx.buffer.len;
  for i in 1..len {
    match !ctx.buffer.info[i].is_unicode_mark()
      && !ctx.buffer.info[i].is_hidden()
      && !ctx.buffer.info[i].is_default_ignorable()
    {
      true => {
        position_cluster(&mut ctx, start, i);
        start = i;
      }
      false => {}
    }
  }

  position_cluster(&mut ctx, start, len);
}

pub fn _hb_ot_shape_fallback_kern(_: &hb_ot_shape_plan_t, _: &hb_font_t, _: &mut hb_buffer_t) {}

pub fn _hb_ot_shape_fallback_spaces<'a, 'x>(
  plan: &'a hb_ot_shape_plan_t,
  face: &'a hb_font_t<'a>,
  buffer: &'x mut hb_buffer_t,
  font_funcs: &'x mut FontFuncsDispatch<'a, '_>,
) {
  use super::unicode::hb_unicode_funcs_t as t;

  let _ = plan;
  let len = buffer.len;
  let horizontal = buffer.direction.is_horizontal();
  let scale = *font_funcs.scale();
  for (info, pos) in buffer.info[..len].iter().zip(&mut buffer.pos[..len]) {
    match info.is_unicode_space() && !info.ligated() {
      true => {
        let space_type = info.unicode_space_fallback_type();
        match space_type {
          t::SPACE_EM
          | t::SPACE_EM_2
          | t::SPACE_EM_3
          | t::SPACE_EM_4
          | t::SPACE_EM_5
          | t::SPACE_EM_6
          | t::SPACE_EM_16 => {
            let length = (({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, i32>(face.units_per_em);
              migrated_value
            }) + ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, i32>(space_type);
              migrated_value
            }) / 2)
              / ({
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped::<_, i32>(space_type);
                migrated_value
              });
            match horizontal {
              true => {
                pos.x_advance = scale.scale_x(length);
              }
              false => {
                pos.y_advance = -scale.scale_y(length);
              }
            }
          }

          t::SPACE_4_EM_18 => {
            let length = ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
                (({
                  let Ok(migrated_value) =
                    console_core_number_conversion::wrapped::<_, i64>(face.units_per_em);
                  migrated_value
                }) * 4
                  / 18),
              );
              migrated_value
            });
            match horizontal {
              true => {
                pos.x_advance = scale.scale_x(length);
              }
              false => {
                pos.y_advance = -scale.scale_y(length);
              }
            }
          }

          t::SPACE_FIGURE => {
            for u in '0'..='9' {
              match font_funcs.nominal_glyph(u as u32) {
                Some(glyph) => {
                  match horizontal {
                    true => {
                      pos.x_advance = font_funcs.advance_width(glyph);
                    }
                    false => {
                      pos.y_advance = font_funcs.advance_height(glyph);
                    }
                  }
                  break;
                }
                None => {}
              }
            }
          }

          t::SPACE_PUNCTUATION => {
            let punct = font_funcs
              .nominal_glyph('.' as u32)
              .or_else(|| font_funcs.nominal_glyph(',' as u32));

            match punct {
              Some(glyph) => match horizontal {
                true => {
                  pos.x_advance = font_funcs.advance_width(glyph);
                }
                false => {
                  pos.y_advance = font_funcs.advance_height(glyph);
                }
              },
              None => {}
            }
          }

          t::SPACE_NARROW => match horizontal {
            true => {
              pos.x_advance /= 2;
            }
            false => {
              pos.y_advance /= 2;
            }
          },

          _ => {}
        }
      }
      false => {}
    }
  }
}
