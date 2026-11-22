use crate::hb::ot_layout_gsubgpos::OT::hb_ot_apply_context_t;
use crate::hb::ot_layout_gsubgpos::{Apply, WouldApply, WouldApplyContext};
use read_fonts::tables::gsub::{SingleSubstFormat1, SingleSubstFormat2};

impl WouldApply for SingleSubstFormat1<'_> {
  fn would_apply(&self, ctx: &WouldApplyContext) -> bool {
    let gid = ctx.glyphs[0];
    ctx.glyphs.len() == 1
      && match self.coverage() {
        Ok(cov) => cov.get(gid).is_some(),
        Err(_) => false,
      }
  }
}

impl Apply for SingleSubstFormat1<'_> {
  fn apply(&self, ctx: &mut hb_ot_apply_context_t) -> Option<()> {
    let glyph = ctx.buffer.cur(0).as_glyph();
    self.coverage().ok()?.get(glyph)?;
    let subst = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
        (({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>({
            let Ok(value) = glyph.to_u32();
            value
          });
          migrated_value
        }) + ({
          let Ok(delta_glyph_id) = self.delta_glyph_id();
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, i32>(delta_glyph_id);
          migrated_value
        })),
      );
      migrated_value
    });
    ctx.replace_glyph(subst.into());
    Some(())
  }
}

impl WouldApply for SingleSubstFormat2<'_> {
  fn would_apply(&self, ctx: &WouldApplyContext) -> bool {
    ctx.glyphs.len() == 1
      && match self.coverage() {
        Ok(cov) => cov.get(ctx.glyphs[0]).is_some(),
        Err(_) => false,
      }
  }
}

impl Apply for SingleSubstFormat2<'_> {
  fn apply(&self, ctx: &mut hb_ot_apply_context_t) -> Option<()> {
    let glyph = ctx.buffer.cur(0).as_glyph();
    let index = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped_index(self.coverage().ok()?.get(glyph)?);
      migrated_value
    });
    let subst = ({
      let Ok(value) = ({
        let Ok(substitute_glyph_ids) = self.substitute_glyph_ids();
        let Ok(value) = substitute_glyph_ids.get(index)?.get();
        value
      })
      .to_u16();
      value
    });
    ctx.replace_glyph(subst.into());
    Some(())
  }
}
