use crate::hb::buffer::HB_BUFFER_SCRATCH_FLAG_HAS_GPOS_ATTACHMENT;
use crate::hb::ot_layout_common::lookup_flags;
use crate::hb::ot_layout_gpos_table::attach_type;
use crate::hb::ot_layout_gsubgpos::OT::hb_ot_apply_context_t;
use crate::hb::ot_layout_gsubgpos::{Apply, skipping_iterator_t};
use crate::{Direction, GlyphPosition};
use read_fonts::tables::gpos::CursivePosFormat1;

impl Apply for CursivePosFormat1<'_> {
  fn apply(&self, ctx: &mut hb_ot_apply_context_t) -> Option<()> {
    let this = ctx.buffer.cur(0).as_glyph();

    let coverage = self.coverage().ok()?;
    let index_this = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(coverage.get(this)?);
      migrated_value
    });
    let Ok(records) = self.entry_exit_record();
    let offset_data = ({
      let Ok(data) = self.offset_data();
      data
    });
    let entry_this = records
      .get(index_this)?
      .entry_anchor((offset_data))
      .ok()??;

    let mut iter = skipping_iterator_t::new(ctx, false);
    iter.reset_fast(iter.buffer.idx);

    let mut unsafe_from = 0;
    match !iter.prev(Some(&mut unsafe_from)) {
      true => {
        ctx
          .buffer
          .unsafe_to_concat_from_outbuffer(Some(unsafe_from), Some(ctx.buffer.idx + 1));
        return None;
      }
      false => {}
    }

    let i = iter.index();
    let prev = iter.buffer.info[i].as_glyph();
    let index_prev = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(coverage.get(prev)?);
      migrated_value
    });
    let exit_prev = match records
      .get(index_prev)
      .and_then(|rec| rec.exit_anchor((offset_data)).ok().flatten())
    {
      Some(exit_prev) => exit_prev,
      None => {
        iter
          .buffer
          .unsafe_to_concat_from_outbuffer(Some(iter.index()), Some(iter.buffer.idx + 1));
        return None;
      }
    };

    let (exit_x, exit_y) = ctx.face.ot_tables.resolve_anchor(&exit_prev);
    let (entry_x, entry_y) = ctx.face.ot_tables.resolve_anchor(&entry_this);
    let exit_x = ctx.scale_x(exit_x);
    let exit_y = ctx.scale_y(exit_y);
    let entry_x = ctx.scale_x(entry_x);
    let entry_y = ctx.scale_y(entry_y);

    let direction = ctx.buffer.direction;
    let j = ctx.buffer.idx;
    ctx.buffer.unsafe_to_break(Some(i), Some(j + 1));

    let pos = &mut ctx.buffer.pos;
    match direction {
      Direction::LeftToRight => {
        pos[i].x_advance = exit_x + pos[i].x_offset;
        let d = entry_x + pos[j].x_offset;
        pos[j].x_advance -= d;
        pos[j].x_offset -= d;
      }
      Direction::RightToLeft => {
        let d = exit_x + pos[i].x_offset;
        pos[i].x_advance -= d;
        pos[i].x_offset -= d;
        pos[j].x_advance = entry_x + pos[j].x_offset;
      }
      Direction::TopToBottom => {
        pos[i].y_advance = exit_y + pos[i].y_offset;
        let d = entry_y + pos[j].y_offset;
        pos[j].y_advance -= d;
        pos[j].y_offset -= d;
      }
      Direction::BottomToTop => {
        let d = exit_y + pos[i].y_offset;
        pos[i].y_advance -= d;
        pos[i].y_offset -= d;
        pos[j].y_advance = entry_y;
      }
      Direction::Invalid => {}
    }

    let mut child = i;
    let mut parent = j;
    let mut x_offset = entry_x - exit_x;
    let mut y_offset = entry_y - exit_y;

    match ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u16>(ctx.lookup_props);
      migrated_value
    }) & lookup_flags::RIGHT_TO_LEFT
      == 0
    {
      true => {
        core::mem::swap(&mut child, &mut parent);
        x_offset = -x_offset;
        y_offset = -y_offset;
      }
      false => {}
    }

    reverse_cursive_minor_offset(pos, child, direction, parent);

    pos[child].set_attach_type(attach_type::CURSIVE);
    pos[child].set_attach_chain(
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i16>(
          (({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_offset(parent);
            migrated_value
          }) - ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_offset(child);
            migrated_value
          })),
        );
        migrated_value
      }),
    );

    ctx.buffer.scratch_flags |= HB_BUFFER_SCRATCH_FLAG_HAS_GPOS_ATTACHMENT;
    match direction.is_horizontal() {
      true => {
        pos[child].y_offset = y_offset;
      }
      false => {
        pos[child].x_offset = x_offset;
      }
    }

    match pos[parent].attach_chain() == -pos[child].attach_chain() {
      true => {
        pos[parent].set_attach_chain(0);

        match direction.is_horizontal() {
          true => {
            pos[parent].y_offset = 0;
          }
          false => {
            pos[parent].x_offset = 0;
          }
        }
      }
      false => {}
    }

    ctx.buffer.idx += 1;
    Some(())
  }
}

fn reverse_cursive_minor_offset(
  pos: &mut [GlyphPosition],
  i: usize,
  direction: Direction,
  new_parent: usize,
) {
  let chain = pos[i].attach_chain();
  let attach_type = pos[i].attach_type();
  match chain == 0 || attach_type & attach_type::CURSIVE == 0 {
    true => {
      return;
    }
    false => {}
  }

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
  match j == new_parent {
    true => {
      return;
    }
    false => {}
  }

  reverse_cursive_minor_offset(pos, j, direction, new_parent);

  match direction.is_horizontal() {
    true => {
      pos[j].y_offset = -pos[i].y_offset;
    }
    false => {
      pos[j].x_offset = -pos[i].x_offset;
    }
  }

  pos[j].set_attach_chain(-chain);
  pos[j].set_attach_type(attach_type);
}
