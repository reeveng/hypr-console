use crate::hb::ot_layout_gsubgpos::Apply;
use crate::hb::ot_layout_gsubgpos::OT::hb_ot_apply_context_t;
use read_fonts::tables::gpos::{SinglePosFormat1, SinglePosFormat2};

impl Apply for SinglePosFormat1<'_> {
  fn apply(&self, ctx: &mut hb_ot_apply_context_t) -> Option<()> {
    let glyph = ctx.buffer.cur(0).as_glyph();
    self.coverage().ok()?.get(glyph)?;
    let Ok(format) = self.value_format();
    let Ok(value_record_byte_range) = self.value_record_byte_range();
    let offset = value_record_byte_range.start;
    super::apply_value(
      ctx,
      ctx.buffer.idx,
      &({
        let Ok(data) = self.offset_data();
        data
      }),
      ({
        let Ok(value) = console_core_number_conversion::index(offset);
        value
      }),
      format,
    );
    ctx.buffer.idx += 1;
    Some(())
  }
}

impl Apply for SinglePosFormat2<'_> {
  fn apply(&self, ctx: &mut hb_ot_apply_context_t) -> Option<()> {
    let glyph = ctx.buffer.cur(0).as_glyph();
    let index = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped_index(self.coverage().ok()?.get(glyph)?);
      migrated_value
    });
    let Ok(format) = self.value_format();
    let Ok(value_records_byte_range) = self.value_records_byte_range();
    let offset = value_records_byte_range.start
      + (format.record_byte_len()
        * ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(index);
          value
        }));
    super::apply_value(
      ctx,
      ctx.buffer.idx,
      &({
        let Ok(data) = self.offset_data();
        data
      }),
      ({
        let Ok(value) = console_core_number_conversion::index(offset);
        value
      }),
      ({
        let explicit_value = format;
        explicit_value
      }),
    );
    ctx.buffer.idx += 1;
    Some(())
  }
}
