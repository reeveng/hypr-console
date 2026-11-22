use crate::hb::ot_layout_gsubgpos::OT::hb_ot_apply_context_t;
use crate::hb::ot_layout_gsubgpos::{Apply, WouldApply, WouldApplyContext};
use read_fonts::tables::gsub::{AlternateSet, AlternateSubstFormat1};

impl Apply for AlternateSet<'_> {
  fn apply(&self, ctx: &mut hb_ot_apply_context_t) -> Option<()> {
    let Ok(alternates) = self.alternate_glyph_ids();
    let len = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u16>(alternates.len());
      migrated_value
    });
    match len == 0 {
      true => {
        return None;
      }
      false => {}
    }

    let glyph_mask = ctx.buffer.cur(0).mask;

    let shift = ctx.lookup_mask().trailing_zeros();
    let mut alt_index = (ctx.lookup_mask() & glyph_mask) >> shift;

    match alt_index == crate::hb::ot_map::hb_ot_map_t::MAX_VALUE && ctx.random {
      true => {
        ctx.buffer.unsafe_to_break(Some(0), Some(ctx.buffer.len));
        alt_index = ctx.random_number() % u32::from(len) + 1;
      }
      false => {}
    }

    let idx = u16::try_from(alt_index).ok()?.checked_sub(1)?;
    ctx.replace_glyph(
      ({
        let Ok(value) = alternates
          .get(
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(idx);
              migrated_value
            }),
          )?
          .get();
        value
      })
      .into(),
    );

    Some(())
  }
}

impl WouldApply for AlternateSubstFormat1<'_> {
  fn would_apply(&self, ctx: &WouldApplyContext) -> bool {
    ctx.glyphs.len() == 1
      && match self.coverage() {
        Ok(cov) => cov.get(ctx.glyphs[0]).is_some(),
        Err(_) => false,
      }
  }
}

impl Apply for AlternateSubstFormat1<'_> {
  fn apply(&self, ctx: &mut hb_ot_apply_context_t) -> Option<()> {
    let glyph = ctx.buffer.cur(0).as_glyph();
    let index = self.coverage().ok()?.get(glyph)?;
    let Ok(alternate_sets) = self.alternate_sets();
    let set = alternate_sets
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
      .ok()?;
    set.apply(ctx)
  }
}
