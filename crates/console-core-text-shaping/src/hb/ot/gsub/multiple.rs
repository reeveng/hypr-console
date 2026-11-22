use crate::hb::buffer::GlyphPropsFlags;
use crate::hb::ot_layout_gsubgpos::OT::hb_ot_apply_context_t;
use crate::hb::ot_layout_gsubgpos::{Apply, WouldApply, WouldApplyContext};
use read_fonts::tables::gsub::MultipleSubstFormat1;

impl WouldApply for MultipleSubstFormat1<'_> {
  fn would_apply(&self, ctx: &WouldApplyContext) -> bool {
    ctx.glyphs.len() == 1
      && match self.coverage() {
        Ok(cov) => cov.get(ctx.glyphs[0]).is_some(),
        Err(_) => false,
      }
  }
}

impl Apply for MultipleSubstFormat1<'_> {
  fn apply(&self, ctx: &mut hb_ot_apply_context_t) -> Option<()> {
    let gid = ctx.buffer.cur(0).as_glyph();
    let index = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped_index(self.coverage().ok()?.get(gid)?);
      migrated_value
    });
    let Ok(sequences) = self.sequences();
    let Ok(substs) = sequences
      .get(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(index);
          value
        }),
      )
      .ok()?
      .substitute_glyph_ids();
    match substs.len() {
      0 => ctx.buffer.delete_glyph(),

      1 => ctx.replace_glyph(
        ({
          let Ok(value) = substs.first()?.get();
          value
        })
        .into(),
      ),

      _ => {
        let class = match ctx.buffer.cur(0).is_ligature() {
          true => GlyphPropsFlags::BASE_GLYPH,
          false => GlyphPropsFlags::empty(),
        };
        let lig_id = ctx.buffer.cur(0).lig_id();

        for (i, subst) in substs.iter().enumerate() {
          let subst = ({
            let Ok(value) = subst.get();
            value
          })
          .into();

          match lig_id == 0 {
            true => {
              ctx.buffer.cur_mut(0).set_lig_props_for_component(
                ({
                  let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(i);
                  migrated_value
                }),
              );
            }
            false => {}
          }
          ctx.output_glyph_for_component(subst, class);
        }

        ctx.buffer.skip_glyph();
      }
    }
    Some(())
  }
}
