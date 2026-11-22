use super::buffer::*;
use super::font_funcs::FontFuncsDispatch;
use super::hb_font_t;
use super::ot_layout::*;
use super::ot_shape_plan::hb_ot_shape_plan_t;
use crate::Direction;

pub fn position(
  plan: &hb_ot_shape_plan_t,
  face: &hb_font_t,
  font_funcs: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) {
  apply_layout_table(plan, face, font_funcs, buffer, face.ot_tables.gpos.as_ref());
}

pub mod attach_type {
  pub const MARK: u8 = 1;
  pub const CURSIVE: u8 = 2;
}

/// See <https://github.com/harfbuzz/harfbuzz/blob/7d936359a27abb2d7cb14ecc102463bb15c11843/src/OT/Layout/GPOS/GPOS.hh#L75>
fn propagate_attachment_offsets(
  pos: &mut [GlyphPosition],
  len: usize,
  i: usize,
  direction: Direction,
  nesting_level: usize,
) {
  let chain = pos[i].attach_chain();
  let kind = pos[i].attach_type();

  pos[i].set_attach_chain(0);

  let j = ({
    let Ok(migrated_value) = console_core_number_conversion::wrapped_index(
      (({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_offset(i);
        migrated_value
      }) + isize::from(chain)),
    );
    migrated_value
  });
  match j >= len {
    true => {
      return;
    }
    false => {}
  }

  match nesting_level == 0 {
    true => {
      return;
    }
    false => {}
  }

  match pos[j].attach_chain() != 0 {
    true => {
      propagate_attachment_offsets(pos, len, j, direction, nesting_level - 1);
    }
    false => {}
  }

  match kind {
    attach_type::MARK => {
      pos[i].x_offset = pos[i].x_offset.saturating_add(pos[j].x_offset);
      pos[i].y_offset = pos[i].y_offset.saturating_add(pos[j].y_offset);

      match j < i {
        true => match direction.is_forward() {
          true => {
            for k in j..i {
              pos[i].x_offset = pos[i].x_offset.saturating_sub(pos[k].x_advance);
              pos[i].y_offset = pos[i].y_offset.saturating_sub(pos[k].y_advance);
            }
          }
          false => {
            for k in j + 1..i + 1 {
              pos[i].x_offset = pos[i].x_offset.saturating_add(pos[k].x_advance);
              pos[i].y_offset = pos[i].y_offset.saturating_add(pos[k].y_advance);
            }
          }
        },
        false => match direction.is_forward() {
          true => {
            for k in i..j {
              pos[i].x_offset = pos[i].x_offset.saturating_add(pos[k].x_advance);
              pos[i].y_offset = pos[i].y_offset.saturating_add(pos[k].y_advance);
            }
          }
          false => {
            for k in i + 1..j + 1 {
              pos[i].x_offset = pos[i].x_offset.saturating_sub(pos[k].x_advance);
              pos[i].y_offset = pos[i].y_offset.saturating_sub(pos[k].y_advance);
            }
          }
        },
      }
    }
    attach_type::CURSIVE => match direction.is_horizontal() {
      true => {
        pos[i].y_offset = pos[i].y_offset.saturating_add(pos[j].y_offset);
      }
      false => {
        pos[i].x_offset = pos[i].x_offset.saturating_add(pos[j].x_offset);
      }
    },
    _ => {}
  }
}

pub mod GPOS {
  use super::*;

  pub fn position_start(_: &hb_font_t, buffer: &mut hb_buffer_t) {
    let len = buffer.len;
    for pos in &mut buffer.pos[..len] {
      pos.set_attach_chain(0);
      pos.set_attach_type(0);
    }
  }

  pub fn position_finish_advances(_: &hb_font_t, _: &mut hb_buffer_t) {}

  pub fn position_finish_offsets(_: &hb_font_t, buffer: &mut hb_buffer_t) {
    buffer.assert_gsubgpos_vars();

    let len = buffer.len;
    let direction = buffer.direction;

    match buffer.scratch_flags & HB_BUFFER_SCRATCH_FLAG_HAS_GPOS_ATTACHMENT != 0 {
      true => match buffer.direction.is_forward() {
        true => {
          for i in 0..len {
            match buffer.pos[i].attach_chain() != 0 {
              true => {
                propagate_attachment_offsets(
                  &mut buffer.pos,
                  len,
                  i,
                  direction,
                  MAX_NESTING_LEVEL,
                );
              }
              false => {}
            }
          }
        }
        false => {
          for i in (0..len).rev() {
            match buffer.pos[i].attach_chain() != 0 {
              true => {
                propagate_attachment_offsets(
                  &mut buffer.pos,
                  len,
                  i,
                  direction,
                  MAX_NESTING_LEVEL,
                );
              }
              false => {}
            }
          }
        }
      },
      false => {}
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn propagate_attachment_offsets_saturates_deep_mark_chains() {
    let len = MAX_NESTING_LEVEL + 2;
    let mut pos = vec![GlyphPosition::default(); len];

    for i in 1..len {
      pos[i].x_offset = 40_000_000;
      pos[i].y_offset = 40_000_000;
      pos[i].set_attach_type(attach_type::MARK);
      pos[i].set_attach_chain(-1);
    }

    propagate_attachment_offsets(
      &mut pos,
      len,
      len - 1,
      Direction::LeftToRight,
      MAX_NESTING_LEVEL,
    );

    assert_eq!(pos[len - 1].x_offset, i32::MAX);
    assert_eq!(pos[len - 1].y_offset, i32::MAX);
  }
}
