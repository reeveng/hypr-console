use crate::hb::buffer::{HB_BUFFER_SCRATCH_FLAG_HAS_GPOS_ATTACHMENT, hb_buffer_t};
use crate::hb::ot_layout_common::lookup_flags;
use crate::hb::ot_layout_gpos_table::attach_type;
use crate::hb::ot_layout_gsubgpos::OT::hb_ot_apply_context_t;
use crate::hb::ot_layout_gsubgpos::{Apply, MatchSource, match_t, skipping_iterator_t};
use read_fonts::tables::gpos::{
  AnchorTable, MarkArray, MarkBasePosFormat1, MarkLigPosFormat1, MarkMarkPosFormat1,
};

trait MarkArrayExt {
  fn apply(
    &self,
    ctx: &mut hb_ot_apply_context_t,
    base_anchor: &AnchorTable,
    mark_anchor: &AnchorTable,
    glyph_pos: usize,
  ) -> Option<()>;
}

impl MarkArrayExt for MarkArray<'_> {
  fn apply(
    &self,
    ctx: &mut hb_ot_apply_context_t,
    base_anchor: &AnchorTable,
    mark_anchor: &AnchorTable,
    glyph_pos: usize,
  ) -> Option<()> {
    let (base_x, base_y) = ctx.face.ot_tables.resolve_anchor(base_anchor);
    let (mark_x, mark_y) = ctx.face.ot_tables.resolve_anchor(mark_anchor);
    let x_offset = ctx.scale_x(base_x - mark_x);
    let y_offset = ctx.scale_y(base_y - mark_y);

    ctx
      .buffer
      .unsafe_to_break(Some(glyph_pos), Some(ctx.buffer.idx + 1));

    let idx = ctx.buffer.idx;
    let pos = ctx.buffer.cur_pos_mut();
    pos.x_offset = x_offset;
    pos.y_offset = y_offset;
    pos.set_attach_type(attach_type::MARK);
    pos.set_attach_chain(
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i16>(
          (({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_offset(glyph_pos);
            migrated_value
          }) - ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_offset(idx);
            migrated_value
          })),
        );
        migrated_value
      }),
    );

    ctx.buffer.scratch_flags |= HB_BUFFER_SCRATCH_FLAG_HAS_GPOS_ATTACHMENT;
    ctx.buffer.idx += 1;

    Some(())
  }
}

impl Apply for MarkBasePosFormat1<'_> {
  fn apply(&self, ctx: &mut hb_ot_apply_context_t) -> Option<()> {
    let mark_glyph = ctx.buffer.cur(0).as_glyph();
    let mark_index = self.mark_coverage().ok()?.get(mark_glyph)?;

    let base_coverage = self.base_coverage().ok()?;
    let last_base_until = ctx.last_base_until;
    let mut last_base = ctx.last_base;

    match ctx.last_base_until
      > ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u32>(ctx.buffer.idx);
        migrated_value
      }) {
      true => {
        ctx.last_base_until = 0;
        ctx.last_base = -1;
      }
      false => {}
    }

    let mut iter = skipping_iterator_t::new(ctx, false);
    iter.set_lookup_props(u32::from(lookup_flags::IGNORE_MARKS));

    let mut j = iter.buffer.idx;
    while j
      > ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(last_base_until);
        migrated_value
      })
    {
      let mut _match = iter.match_at(j - 1, MatchSource::Info);
      match _match == match_t::MATCH {
        true => {
          match !accept(iter.buffer, j - 1)
            && base_coverage
              .get(iter.buffer.info[j - 1].as_glyph())
              .is_none()
          {
            true => {
              _match = match_t::SKIP;
            }
            false => {}
          }
        }
        false => {}
      }

      match _match == match_t::MATCH {
        true => {
          last_base = ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(j);
            migrated_value
          }) - 1;
          break;
        }
        false => {}
      }

      j -= 1;
    }
    ctx.last_base_until = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(ctx.buffer.idx);
      migrated_value
    });
    ctx.last_base = last_base;

    match ctx.last_base == -1 {
      true => {
        ctx
          .buffer
          .unsafe_to_concat_from_outbuffer(Some(0), Some(ctx.buffer.idx + 1));
        return None;
      }
      false => {}
    }

    let idx = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(ctx.last_base);
      migrated_value
    });

    let info = &ctx.buffer.info;

    let base_glyph = info[({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(idx);
      migrated_value
    })]
    .as_glyph();
    let base_index = match self.base_coverage().ok()?.get(base_glyph) {
      Some(base_index) => base_index,
      None => {
        ctx.buffer.unsafe_to_concat_from_outbuffer(
          Some(
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(idx);
              migrated_value
            }),
          ),
          Some(ctx.buffer.idx + 1),
        );
        return None;
      }
    };

    let mark_array = self.mark_array().ok()?;
    let Ok(mark_records) = mark_array.mark_records();
    let mark_record = mark_records.get(
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(mark_index);
        migrated_value
      }),
    )?;
    let mark_anchor = mark_record
      .mark_anchor({
        let Ok(value) = mark_array.offset_data();
        value
      })
      .ok()?;

    let base_array = self.base_array().ok()?;
    let Ok(base_records) = base_array.base_records();
    let base_record = base_records
      .get(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(base_index);
              migrated_value
            }),
          );
          value
        }),
      )
      .ok()?;
    let Ok(base_anchors) = base_record.base_anchors({
      let Ok(value) = base_array.offset_data();
      value
    });
    let base_anchor = base_anchors
      .get(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
            ({
              let Ok(mark_class) = mark_record.mark_class();
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(mark_class);
              migrated_value
            }),
          );
          value
        }),
      )
      .ok()??;

    mark_array.apply(
      ctx,
      &base_anchor,
      &mark_anchor,
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(idx);
        migrated_value
      }),
    )
  }
}

fn accept(buffer: &hb_buffer_t, idx: usize) -> bool {
  !buffer.info[idx].multiplied()
    || 0 == buffer.info[idx].lig_comp()
    || (idx == 0
      || buffer.info[idx - 1].is_mark()
      || !buffer.info[idx - 1].multiplied()
      || buffer.info[idx].lig_id() != buffer.info[idx - 1].lig_id()
      || buffer.info[idx].lig_comp() != buffer.info[idx - 1].lig_comp() + 1)
}

impl Apply for MarkMarkPosFormat1<'_> {
  fn apply(&self, ctx: &mut hb_ot_apply_context_t) -> Option<()> {
    let mark1_glyph = ctx.buffer.cur(0).as_glyph();
    let mark1_index = self.mark1_coverage().ok()?.get(mark1_glyph)?;
    let lookup_props = ctx.lookup_props;

    let mut iter = skipping_iterator_t::new(ctx, false);
    iter.reset_fast(iter.buffer.idx);
    iter.set_lookup_props(lookup_props & !u32::from(lookup_flags::IGNORE_FLAGS));

    let mut unsafe_from = 0;
    match !iter.prev(Some(&mut unsafe_from)) {
      true => {
        iter
          .buffer
          .unsafe_to_concat_from_outbuffer(Some(unsafe_from), Some(iter.buffer.idx + 1));
        return None;
      }
      false => {}
    }

    let iter_idx = iter.index();
    match !ctx.buffer.info[iter_idx].is_mark() {
      true => {
        ctx
          .buffer
          .unsafe_to_concat_from_outbuffer(Some(iter_idx), Some(ctx.buffer.idx + 1));
        return None;
      }
      false => {}
    }

    let id1 = ctx.buffer.cur(0).lig_id();
    let id2 = ctx.buffer.info[iter_idx].lig_id();
    let comp1 = ctx.buffer.cur(0).lig_comp();
    let comp2 = ctx.buffer.info[iter_idx].lig_comp();

    let matches = match id1 == id2 {
      true => id1 == 0 || comp1 == comp2,
      false => (id1 > 0 && comp1 == 0) || (id2 > 0 && comp2 == 0),
    };

    match !matches {
      true => {
        ctx
          .buffer
          .unsafe_to_concat_from_outbuffer(Some(iter_idx), Some(ctx.buffer.idx + 1));
        return None;
      }
      false => {}
    }

    let mark2_glyph = ctx.buffer.info[iter_idx].as_glyph();
    let mark2_index = self.mark2_coverage().ok()?.get(mark2_glyph)?;

    let mark1_array = self.mark1_array().ok()?;
    let Ok(mark_records) = mark1_array.mark_records();
    let mark1_record = mark_records.get(
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(mark1_index);
        migrated_value
      }),
    )?;
    let mark1_anchor = mark1_record
      .mark_anchor({
        let Ok(value) = mark1_array.offset_data();
        value
      })
      .ok()?;

    let base_array = self.mark2_array().ok()?;
    let Ok(mark2_records) = base_array.mark2_records();
    let base_record = mark2_records
      .get(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(mark2_index);
              migrated_value
            }),
          );
          value
        }),
      )
      .ok()?;
    let Ok(mark2_anchors) = base_record.mark2_anchors({
      let Ok(value) = base_array.offset_data();
      value
    });
    let base_anchor = mark2_anchors
      .get(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
            ({
              let Ok(mark_class) = mark1_record.mark_class();
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(mark_class);
              migrated_value
            }),
          );
          value
        }),
      )
      .ok()??;

    mark1_array.apply(ctx, &base_anchor, &mark1_anchor, iter_idx)
  }
}

impl Apply for MarkLigPosFormat1<'_> {
  fn apply(&self, ctx: &mut hb_ot_apply_context_t) -> Option<()> {
    let mark_glyph = ctx.buffer.cur(0).as_glyph();
    let mark_index = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(
        self.mark_coverage().ok()?.get(mark_glyph)?,
      );
      migrated_value
    });

    match ctx.last_base_until
      > ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u32>(ctx.buffer.idx);
        migrated_value
      }) {
      true => {
        ctx.last_base_until = 0;
        ctx.last_base = -1;
      }
      false => {}
    }

    let last_base_until = ctx.last_base_until;
    let mut last_base = ctx.last_base;

    let mut iter = skipping_iterator_t::new(ctx, false);
    iter.set_lookup_props(u32::from(lookup_flags::IGNORE_MARKS));

    let mut j = iter.buffer.idx;
    while j
      > ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(last_base_until);
        migrated_value
      })
    {
      let mut _match = iter.match_at(j - 1, MatchSource::Info);
      match _match == match_t::MATCH {
        true => {
          last_base = ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(j);
            migrated_value
          }) - 1;
          break;
        }
        false => {}
      }
      j -= 1;
    }

    ctx.last_base_until = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(ctx.buffer.idx);
      migrated_value
    });
    ctx.last_base = last_base;

    match ctx.last_base == -1 {
      true => {
        ctx
          .buffer
          .unsafe_to_concat_from_outbuffer(Some(0), Some(ctx.buffer.idx + 1));
        return None;
      }
      false => {}
    }

    let idx = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(ctx.last_base);
      migrated_value
    });

    let lig_glyph = ctx.buffer.info[idx].as_glyph();
    let lig_index = match self.ligature_coverage().ok()?.get(lig_glyph) {
      Some(lig_index) => lig_index,
      None => {
        ctx
          .buffer
          .unsafe_to_concat_from_outbuffer(Some(idx), Some(ctx.buffer.idx + 1));
        return None;
      }
    };
    let Ok(ligature_attaches) = self.ligature_array().ok()?.ligature_attaches();
    let lig_attach = ligature_attaches
      .get(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(lig_index);
              migrated_value
            }),
          );
          value
        }),
      )
      .ok()?;

    let Ok(comp_count) = lig_attach.component_count();
    match comp_count == 0 {
      true => {
        ctx
          .buffer
          .unsafe_to_concat_from_outbuffer(Some(idx), Some(ctx.buffer.idx + 1));
        return None;
      }
      false => {}
    }

    let lig_id = ctx.buffer.info[idx].lig_id();
    let mark_id = ctx.buffer.cur(0).lig_id();
    let mark_comp = u16::from(ctx.buffer.cur(0).lig_comp());
    let matches = lig_id != 0 && lig_id == mark_id && mark_comp > 0;
    let comp_index = match matches {
      true => mark_comp.min(comp_count),
      false => comp_count,
    } - 1;

    let mark_array = self.mark_array().ok()?;
    let Ok(mark_records) = mark_array.mark_records();
    let mark_record = mark_records.get(mark_index)?;
    let mark_anchor = mark_record
      .mark_anchor({
        let Ok(value) = mark_array.offset_data();
        value
      })
      .ok()?;

    let Ok(component_records) = lig_attach.component_records();
    let base_record = component_records
      .get(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(comp_index);
              migrated_value
            }),
          );
          value
        }),
      )
      .ok()?;
    let Ok(ligature_anchors) = base_record.ligature_anchors({
      let Ok(value) = lig_attach.offset_data();
      value
    });
    let base_anchor = ligature_anchors
      .get(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
            ({
              let Ok(mark_class) = mark_record.mark_class();
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(mark_class);
              migrated_value
            }),
          );
          value
        }),
      )
      .ok()??;

    mark_array.apply(ctx, &base_anchor, &mark_anchor, idx)
  }
}
