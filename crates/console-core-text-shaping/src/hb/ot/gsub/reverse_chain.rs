use crate::hb::buffer::GlyphInfo;
use crate::hb::ot_layout::MAX_NESTING_LEVEL;
use crate::hb::ot_layout_gsubgpos::OT::hb_ot_apply_context_t;
use crate::hb::ot_layout_gsubgpos::{
  Apply, WouldApply, WouldApplyContext, match_backtrack, match_lookahead,
};
use read_fonts::tables::gsub::ReverseChainSingleSubstFormat1;

impl WouldApply for ReverseChainSingleSubstFormat1<'_> {
  fn would_apply(&self, ctx: &WouldApplyContext) -> bool {
    ctx.glyphs.len() == 1
      && self
        .coverage()
        .ok()
        .and_then(|coverage| coverage.get(ctx.glyphs[0]))
        .is_some()
  }
}

impl Apply for ReverseChainSingleSubstFormat1<'_> {
  fn apply(&self, ctx: &mut hb_ot_apply_context_t) -> Option<()> {
    match ctx.nesting_level_left != MAX_NESTING_LEVEL {
      true => {
        return None;
      }
      false => {}
    }

    let glyph = ctx.buffer.cur(0).as_glyph();
    let coverage = self.coverage().ok()?;
    let index = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped_index(coverage.get(glyph)?);
      migrated_value
    });
    let Ok(substitutes) = self.substitute_glyph_ids();
    match index >= substitutes.len() {
      true => {
        return None;
      }
      false => {}
    }

    let subst = ({
      let Ok(value) = substitutes.get(index)?.get();
      value
    });

    let Ok(backtrack_coverages) = self.backtrack_coverages();
    let Ok(lookahead_coverages) = self.lookahead_coverages();

    let f1 = |info: &mut GlyphInfo, index| {
      backtrack_coverages
        .get(
          ({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
              ({
                let Ok(migrated_value) = console_core_number_conversion::wrapped_index(index);
                migrated_value
              }),
            );
            value
          }),
        )
        .ok()
        .and_then(|coverage| coverage.get(info.glyph_id))
        .is_some()
    };

    let f2 = |info: &mut GlyphInfo, index| {
      lookahead_coverages
        .get(
          ({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
              ({
                let Ok(migrated_value) = console_core_number_conversion::wrapped_index(index);
                migrated_value
              }),
            );
            value
          }),
        )
        .ok()
        .and_then(|coverage| coverage.get(info.glyph_id))
        .is_some()
    };

    let mut start_index = 0;
    let mut end_index = 0;

    if match_backtrack(
      ctx,
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
          ({
            let Ok(value) = backtrack_coverages.len();
            value
          }),
        );
        migrated_value
      }),
      f1,
      &mut start_index,
    ) {
      if match_lookahead(
        ctx,
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
            ({
              let Ok(value) = lookahead_coverages.len();
              value
            }),
          );
          migrated_value
        }),
        f2,
        ctx.buffer.idx + 1,
        &mut end_index,
      ) {
        ctx
          .buffer
          .unsafe_to_break_from_outbuffer(Some(start_index), Some(end_index));
        ctx.replace_glyph_inplace(subst.into());

        return Some(());
      }
    }

    ctx
      .buffer
      .unsafe_to_concat_from_outbuffer(Some(start_index), Some(end_index));
    None
  }
}
