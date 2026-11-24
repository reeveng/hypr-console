//! OpenType GPOS lookups.

use crate::hb::ot_layout_gsubgpos::OT::hb_ot_apply_context_t;
use console_core_number_conversion::fitted;
use read_fonts::{
  FontData, Overlap, Presence,
  tables::{
    gpos::ValueFormat,
    variations::{DeltaSetIndex, ItemVariationStore},
  },
  types::F2Dot14,
};

mod cursive;
mod mark;
mod pair;
mod single;

fn read_value(data: &FontData<'_>, offset: &mut usize) -> Option<i32> {
  let Ok(position) = fitted::<_, u64>(*offset);
  let value = data.read_at::<i16>(position).ok()?;
  *offset += 2;
  Some(i32::from(value))
}

fn read_delta(
  data: &FontData<'_>,
  offset: &mut usize,
  variation_store: &ItemVariationStore<'_>,
  coords: &[F2Dot14],
) -> Option<f32> {
  let Ok(position) = fitted::<_, u64>(*offset);
  let relative_offset = data.read_at::<u16>(position).ok()?;
  *offset += 2;
  let record_offset = match relative_offset {
    0 => return Some(0.0),
    offset => u64::from(offset),
  };

  const VARIATION_INDEX_FORMAT: u16 = 0x8000;
  let format = data.read_at::<u16>(record_offset + 4).ok()?;
  match format {
    VARIATION_INDEX_FORMAT => {}
    _ => return Some(0.0),
  }

  let outer = data.read_at::<u16>(record_offset).ok()?;
  let inner = data.read_at::<u16>(record_offset + 2).ok()?;
  let value = variation_store
    .compute_float_delta(DeltaSetIndex { outer, inner }, coords)
    .unwrap_or_default()
    .to_f64() as f32;
  Some(value)
}

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

  let Ok(x_placement) = format.contains(ValueFormat::X_PLACEMENT);
  match x_placement {
    Presence::Present => {
      let value = read_value(data, &mut offset)?;
      worked |= value != 0;
      pos.x_offset += scale.scale_x(value);
    }
    Presence::Absent => {}
  }

  let Ok(y_placement) = format.contains(ValueFormat::Y_PLACEMENT);
  match y_placement {
    Presence::Present => {
      let value = read_value(data, &mut offset)?;
      worked |= value != 0;
      pos.y_offset += scale.scale_y(value);
    }
    Presence::Absent => {}
  }

  let Ok(x_advance) = format.contains(ValueFormat::X_ADVANCE);
  match (x_advance, is_horizontal) {
    (Presence::Present, true) => {
      let value = read_value(data, &mut offset)?;
      worked |= value != 0;
      pos.x_advance += scale.scale_x(value);
    }
    (Presence::Present, false) => offset += 2,
    (Presence::Absent, _) => {}
  }

  let Ok(y_advance) = format.contains(ValueFormat::Y_ADVANCE);
  match (y_advance, is_horizontal) {
    (Presence::Present, false) => {
      let value = read_value(data, &mut offset)?;
      worked |= value != 0;
      pos.y_advance -= scale.scale_y(value);
    }
    (Presence::Present, true) => offset += 2,
    (Presence::Absent, _) => {}
  }

  let Ok(device_fields) = format.intersects(ValueFormat::ANY_DEVICE_OR_VARIDX);
  let variation_store = match (device_fields, &ctx.face.ot_tables.var_store) {
    (Overlap::Intersecting, Some(variation_store)) => variation_store,
    _ => return Some(worked),
  };
  let coords = ctx.face.ot_tables.coords;

  let Ok(x_placement_device) = format.contains(ValueFormat::X_PLACEMENT_DEVICE);
  match x_placement_device {
    Presence::Present => {
      let delta = read_delta(data, &mut offset, variation_store, coords)?;
      worked |= delta != 0.0;
      pos.x_offset += scale.scale_x_f(delta);
    }
    Presence::Absent => {}
  }

  let Ok(y_placement_device) = format.contains(ValueFormat::Y_PLACEMENT_DEVICE);
  match y_placement_device {
    Presence::Present => {
      let delta = read_delta(data, &mut offset, variation_store, coords)?;
      worked |= delta != 0.0;
      pos.y_offset += scale.scale_y_f(delta);
    }
    Presence::Absent => {}
  }

  let Ok(x_advance_device) = format.contains(ValueFormat::X_ADVANCE_DEVICE);
  match (x_advance_device, is_horizontal) {
    (Presence::Present, true) => {
      let delta = read_delta(data, &mut offset, variation_store, coords)?;
      worked |= delta != 0.0;
      pos.x_advance += scale.scale_x_f(delta);
    }
    (Presence::Present, false) => offset += 2,
    (Presence::Absent, _) => {}
  }

  let Ok(y_advance_device) = format.contains(ValueFormat::Y_ADVANCE_DEVICE);
  match (y_advance_device, is_horizontal) {
    (Presence::Present, false) => {
      let delta = read_delta(data, &mut offset, variation_store, coords)?;
      worked |= delta != 0.0;
      pos.y_advance -= scale.scale_y_f(delta);
    }
    (Presence::Present, true) | (Presence::Absent, _) => {}
  }

  Some(worked)
}
