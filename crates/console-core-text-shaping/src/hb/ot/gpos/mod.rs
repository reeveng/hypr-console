//! OpenType GPOS lookups.

use crate::hb::ot_layout_gsubgpos::OT::hb_ot_apply_context_t;
use read_fonts::{
  FontData,
  tables::{gpos::ValueFormat, variations::DeltaSetIndex},
};

mod cursive;
mod mark;
mod pair;
mod single;

#[allow(unused_assignments)]
fn apply_value(
  ctx: &mut hb_ot_apply_context_t,
  idx: usize,
  data: &FontData,
  mut offset: usize,
  format: ValueFormat,
) -> Option<bool> {
  let scale = ctx.scale;
  let pos = &mut ctx.buffer.pos[idx];
  let is_horizontal = ctx.buffer.direction.is_horizontal();
  let mut worked = false;
  macro_rules! read_value {
    () => {{
      let value = data
        .read_at::<i16>(
          ({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(offset);
            value
          }),
        )
        .ok()? as i32;
      worked |= value != 0;
      offset += 2;
      value
    }};
  }
  let Ok(contains) = format.contains(ValueFormat::X_PLACEMENT);

  match (contains == read_fonts::Presence::Present) {
    true => {
      pos.x_offset += scale.scale_x(read_value!());
    }
    false => {}
  }
  let Ok(format_contains) = format.contains(ValueFormat::Y_PLACEMENT);

  match (format_contains == read_fonts::Presence::Present) {
    true => {
      pos.y_offset += scale.scale_y(read_value!());
    }
    false => {}
  }
  match (({
    let Ok(explicit_value) = format.contains(ValueFormat::X_ADVANCE);
    explicit_value
  }) == read_fonts::Presence::Present)
  {
    true => match is_horizontal {
      true => {
        pos.x_advance += scale.scale_x(read_value!());
      }
      false => {
        offset += 2;
      }
    },
    false => {}
  }
  match (({
    let Ok(explicit_value) = format.contains(ValueFormat::Y_ADVANCE);
    explicit_value
  }) == read_fonts::Presence::Present)
  {
    true => match !is_horizontal {
      true => {
        pos.y_advance -= scale.scale_y(read_value!());
      }
      false => {
        offset += 2;
      }
    },
    false => {}
  }
  let Ok(intersects) = format.intersects(ValueFormat::ANY_DEVICE_OR_VARIDX);

  match !(intersects == read_fonts::Overlap::Intersecting) {
    true => {
      return Some(worked);
    }
    false => {}
  }
  match &ctx.face.ot_tables.var_store {
    Some(vs) => {
      let coords = ctx.face.ot_tables.coords;
      macro_rules! read_delta {
        () => {{
          let rec_offset = data
            .read_at::<u16>(
              ({
                let Ok(value) = console_core_number_conversion::fitted::<_, u64>(offset);
                value
              }),
            )
            .ok()? as usize;
          offset += 2;

          let mut value = 0.0f32;

          if rec_offset != 0 {
            let format = data
              .read_at::<u16>(
                ({
                  let Ok(value) =
                    console_core_number_conversion::fitted::<_, u64>(rec_offset + 4);
                  value
                }),
              )
              .ok()?;

            const VARIATION_INDEX_FORMAT: u16 = 0x8000;
            if format == VARIATION_INDEX_FORMAT {
              let outer = data
                .read_at::<u16>(
                  ({
                    let Ok(value) = console_core_number_conversion::fitted::<_, u64>(rec_offset);
                    value
                  }),
                )
                .ok()?;
              let inner = data
                .read_at::<u16>(
                  ({
                    let Ok(value) =
                      console_core_number_conversion::fitted::<_, u64>(rec_offset + 2);
                    value
                  }),
                )
                .ok()?;
              value = vs
                .compute_float_delta(DeltaSetIndex { outer, inner }, coords)
                .unwrap_or_default()
                .to_f64() as f32;
              worked |= value != 0.0;
            }
          }
          value
        }};
      }
      match (({
        let Ok(explicit_value) = format.contains(ValueFormat::X_PLACEMENT_DEVICE);
        explicit_value
      }) == read_fonts::Presence::Present)
      {
        true => {
          pos.x_offset += scale.scale_x_f(read_delta!());
        }
        false => {}
      }
      match (({
        let Ok(explicit_value) = format.contains(ValueFormat::Y_PLACEMENT_DEVICE);
        explicit_value
      }) == read_fonts::Presence::Present)
      {
        true => {
          pos.y_offset += scale.scale_y_f(read_delta!());
        }
        false => {}
      }
      match (({
        let Ok(explicit_value) = format.contains(ValueFormat::X_ADVANCE_DEVICE);
        explicit_value
      }) == read_fonts::Presence::Present)
      {
        true => match is_horizontal {
          true => {
            pos.x_advance += scale.scale_x_f(read_delta!());
          }
          false => {
            offset += 2;
          }
        },
        false => {}
      }
      match (({
        let Ok(explicit_value) = format.contains(ValueFormat::Y_ADVANCE_DEVICE);
        explicit_value
      }) == read_fonts::Presence::Present)
      {
        true => match !is_horizontal {
          true => {
            pos.y_advance -= scale.scale_y_f(read_delta!());
          }
          false => {
            offset += 2;
          }
        },
        false => {}
      }
    }
    None => {}
  }
  Some(worked)
}
