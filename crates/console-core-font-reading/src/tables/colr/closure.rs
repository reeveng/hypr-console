//! Computing closure for the COLR table.
//!
//! Closure callers request partial results. An unreadable COLR record is an absent dependency.
//! Each distinct paint starts inside the table and can schedule at most 255 children.
//! The work stack follows their table order, retaining the first-visit rule for shared paints.
use console_core_integer_sets::{Domain, Membership};
use console_core_never::Never;
use console_core_number_conversion::{fitted, wrapped};
use font_types::{GlyphId, GlyphId16};

use crate::{
  Presence, ReadError, ResolveOffset, collections::IntSet, tables::variations::NO_VARIATION_INDEX,
};

use super::{
  BaseGlyph, BaseGlyphPaint, Clip, ClipBox, ClipBoxFormat2, ClipList, ColorLine, ColorStop, Colr,
  Layer, Paint, PaintColrGlyph, PaintColrLayers, PaintComposite, PaintGlyph, PaintLinearGradient,
  PaintRadialGradient, PaintSolid, PaintSweepGradient, PaintVarLinearGradient,
  PaintVarRadialGradient, PaintVarSolid, PaintVarSweepGradient, PaintVarTransform, VarAffine2x3,
  VarColorLine, VarColorStop,
};

impl Colr<'_> {
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the public COLRv0 palette closure updates the caller's set in place"
    )
  )]
  pub fn v0_closure_palette_indices(
    &self,
    glyph_set: &IntSet<GlyphId>,
    palette_indices: &mut IntSet<u16>,
  ) {
    let Ok(()) = self.collect_layer_values(glyph_set, palette_indices, Layer::palette_index);
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the public COLRv0 glyph closure updates the caller's set in place"
    )
  )]
  pub fn v0_closure_glyphs(
    &self,
    glyph_set: &IntSet<GlyphId>,
    glyphset_colrv0: &mut IntSet<GlyphId>,
  ) {
    let Ok(()) = glyphset_colrv0.union(glyph_set);

    let Ok(()) = self.collect_layer_values(glyph_set, glyphset_colrv0, |layer| {
      let Ok(glyph) = layer.glyph_id();

      Ok(GlyphId::from(glyph))
    });
  }

  fn collect_layer_values<T: Domain>(
    &self,
    glyphs: &IntSet<GlyphId>,
    values: &mut IntSet<T>,
    layer_value: impl Fn(&Layer) -> Result<T, Never>,
  ) -> Result<(), Never> {
    let Ok(records) = readable_colr(self.base_glyph_records());
    let records_3 = match records.flatten() {
      Some(records_2) => records_2,
      None => return Ok(()),
    };
    let Ok(layers) = readable_colr(self.layer_records());
    let layers_3 = match layers.flatten() {
      Some(layers_2) => layers_2,
      None => return Ok(()),
    };

    visit_selected_records(
      records_3,
      glyphs,
      BaseGlyph::glyph_id,
      values,
      |record, output| {
        let Ok(first) = record.first_layer_index();
        let Ok(count) = record.num_layers();
        let end = u64::from(first).wrapping_add(u64::from(count));
        let Ok(range) = console_core_number_conversion::indices(u64::from(first)..end);

        for layer in layers_3.iter().skip(range.start).take(range.len()) {
          let Ok(value) = layer_value(layer);
          let Ok(_previous_presence) = output.insert(value);
        }

        Ok(())
      },
    )
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the public COLRv1 closure fills the caller's glyph, layer, palette, and variation sets in place"
    )
  )]
  pub fn v1_closure(
    &self,
    glyph_set: &mut IntSet<GlyphId>,
    layer_indices: &mut IntSet<u32>,
    palette_indices: &mut IntSet<u16>,
    variation_indices: &mut IntSet<u32>,
  ) {
    let Ok(version) = self.version();

    match version {
      0 => return,
      _with_paints => {}
    }

    let Ok(mut context) =
      Colrv1ClosureContext::new(layer_indices, palette_indices, variation_indices, self);
    let Ok(()) = context.collect_glyphs(glyph_set);
    let Ok(()) = context.collect_clips(glyph_set);
  }
}

fn readable_colr<T>(record: Result<T, ReadError>) -> Result<Option<T>, Never> {
  Ok(match record {
    Ok(record_2) => Some(record_2),
    Err(_unreadable_colr_record) => None,
  })
}

fn lookup_glyph_record<R>(
  records: &[R],
  wanted: GlyphId16,
  glyph_id: impl Fn(&R) -> Result<GlyphId16, Never>,
) -> Result<Option<&R>, Never> {
  Ok(match records.binary_search_by(|record| {
    let Ok(glyph) = glyph_id(record);
    glyph.cmp(&wanted)
  }) {
    Ok(index) => records.get(index),
    Err(_insertion_position) => None,
  })
}

fn visit_selected_records<R, C>(
  records: &[R],
  glyphs: &IntSet<GlyphId>,
  glyph_id: impl Fn(&R) -> Result<GlyphId16, Never>,
  context: &mut C,
  mut visit: impl FnMut(&R, &mut C) -> Result<(), Never>,
) -> Result<(), Never> {
  let Ok(num_records) = wrapped::<_, u32>(records.len());
  let probe_cost = u32::BITS.wrapping_sub(num_records.leading_zeros());
  let Ok(selected_count) = glyphs.len();
  let Ok(selected_glyphs) = wrapped::<_, u32>(selected_count);
  let Ok(membership) = glyphs.membership();

  use core::cmp::Ordering;
  let relative_probe_cost = selected_glyphs.wrapping_mul(probe_cost).cmp(&num_records);

  match (membership, relative_probe_cost) {
    (Membership::Inclusive, Ordering::Less) => {
      let Ok(members) = glyphs.iter();

      for member in members {
        let wanted = match GlyphId16::try_from(member) {
          Ok(glyph) => glyph,
          Err(_outside_font_glyph_range) => continue,
        };
        let Ok(record) = lookup_glyph_record(records, wanted, &glyph_id);

        match record {
          Some(record_2) => {
            let Ok(()) = visit(record_2, context);
          }
          None => continue,
        }
      }

      return Ok(());
    }
    (Membership::Inclusive, Ordering::Equal | Ordering::Greater)
    | (Membership::Exclusive, _) => {}
  }

  for record in records {
    let Ok(glyph) = glyph_id(record);
    let Ok(presence) = glyphs.contains(GlyphId::from(glyph));

    match presence {
      Presence::Present => {
        let Ok(()) = visit(record, context);
      }
      Presence::Absent => continue,
    }
  }

  Ok(())
}

const MAX_PAINT_NESTING: u8 = 64;

struct PendingPaint<'a> {
  paint: Paint<'a>,
  remaining_depth: u8,
}

struct Colrv1ClosureContext<'a> {
  glyph_set: IntSet<GlyphId>,
  layer_indices: &'a mut IntSet<u32>,
  palette_indices: &'a mut IntSet<u16>,
  variation_indices: &'a mut IntSet<u32>,
  colr: &'a Colr<'a>,
  pending: Vec<PendingPaint<'a>>,
  children: Vec<Paint<'a>>,
  visit_limit: u64,
  visited_paints: IntSet<u32>,
  colr_head: u64,
}

impl<'a> Colrv1ClosureContext<'a> {
  fn new(
    layer_indices: &'a mut IntSet<u32>,
    palette_indices: &'a mut IntSet<u16>,
    variation_indices: &'a mut IntSet<u32>,
    colr: &'a Colr<'a>,
  ) -> Result<Self, Never> {
    let Ok(data) = colr.offset_data();
    let Ok(bytes) = data.as_bytes();
    let Ok(colr_head) = fitted::<_, u64>(bytes.as_ptr().addr());
    let Ok(table_length) = data.len();
    let Ok(glyph_set) = IntSet::empty();
    let Ok(visited_paints) = IntSet::empty();

    Ok(Self {
      glyph_set,
      layer_indices,
      palette_indices,
      variation_indices,
      colr,
      pending: Vec::new(),
      children: Vec::new(),
      visit_limit: table_length
        .saturating_mul(u64::from(u8::MAX))
        .saturating_add(1),
      visited_paints,
      colr_head,
    })
  }

  fn collect_glyphs(&mut self, glyphs: &mut IntSet<GlyphId>) -> Result<(), Never> {
    let Ok(base_glyph_list) = readable_colr(self.colr.base_glyph_list());
    let base_glyph_list_2 = match base_glyph_list.flatten() {
      Some(list) => list,
      None => return Ok(()),
    };
    let Ok(records) = base_glyph_list_2.base_glyph_paint_records();
    let Ok(data) = base_glyph_list_2.offset_data();

    let Ok(()) = visit_selected_records(
      records,
      glyphs,
      BaseGlyphPaint::glyph_id,
      self,
      |record, context| {
        let Ok(paint) = readable_colr(record.paint(data));

        match paint {
          Some(paint_2) => context.dispatch(paint_2),
          None => Ok(()),
        }
      },
    );

    glyphs.union(&self.glyph_set)
  }

  fn collect_clips(&mut self, glyphs: &IntSet<GlyphId>) -> Result<(), Never> {
    let Ok(clip_list) = readable_colr(self.colr.clip_list());
    let clip_list_2 = match clip_list.flatten() {
      Some(list) => list,
      None => return Ok(()),
    };
    let Ok(()) = self.glyph_set.union(glyphs);
    let Ok(clips) = clip_list_2.clips();

    for clip in clips {
      let Ok(()) = clip.v1_closure(self, &clip_list_2);
    }

    Ok(())
  }

  fn dispatch(&mut self, root: Paint<'a>) -> Result<(), Never> {
    self.pending.push(PendingPaint {
      paint: root,
      remaining_depth: MAX_PAINT_NESTING,
    });

    for _visit in 0..self.visit_limit {
      let PendingPaint {
        paint,
        remaining_depth,
      } = match self.pending.pop() {
        Some(next) => next,
        None => break,
      };
      let child_depth = match remaining_depth.checked_sub(1) {
        Some(depth) => depth,
        None => continue,
      };
      let Ok(presence) = self.record_paint(&paint);

      match presence {
        Presence::Present => continue,
        Presence::Absent => {}
      }

      let Ok(()) = paint.v1_closure(self);

      self
        .pending
        .extend(self.children.drain(..).rev().map(|paint_2| PendingPaint {
          paint: paint_2,
          remaining_depth: child_depth,
        }));
    }

    Ok(())
  }

  fn schedule(&mut self, paint: Paint<'a>) -> Result<(), Never> {
    self.children.push(paint);

    Ok(())
  }

  fn schedule_child(&mut self, result: Result<Paint<'a>, ReadError>) -> Result<(), Never> {
    let Ok(child) = readable_colr(result);

    match child {
      Some(paint) => self.schedule(paint),
      None => Ok(()),
    }
  }

  fn schedule_variable_child(
    &mut self,
    result: Result<Paint<'a>, ReadError>,
    var_index_base: impl FnOnce() -> Result<u32, Never>,
    fields: u8,
  ) -> Result<(), Never> {
    let Ok(child) = readable_colr(result);

    match child {
      Some(paint) => {
        let Ok(()) = self.schedule(paint);
        let Ok(base) = var_index_base();

        self.add_variation_indices(base, fields)
      }
      None => Ok(()),
    }
  }

  fn record_paint(&mut self, paint: &Paint<'a>) -> Result<Presence, Never> {
    let Ok(data) = paint.offset_data();
    let Ok(bytes) = data.as_bytes();
    let Ok(paint_head) = fitted::<_, u64>(bytes.as_ptr().addr());
    let Ok(offset) = wrapped::<_, u32>(paint_head.wrapping_sub(self.colr_head));

    self.visited_paints.insert(offset)
  }

  fn add_layer_index(&mut self, layer_index: u32) -> Result<(), Never> {
    let Ok(_presence_changed) = self.layer_indices.insert(layer_index);

    Ok(())
  }

  fn add_palette_index(&mut self, palette_index: u16) -> Result<(), Never> {
    let Ok(_presence_changed) = self.palette_indices.insert(palette_index);

    Ok(())
  }

  fn add_variation_indices(&mut self, var_index_base: u32, num_vars: u8) -> Result<(), Never> {
    match ::core::primitive::bool::then_some(
      num_vars == 0 || var_index_base == NO_VARIATION_INDEX,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let last_var_index = ::core::primitive::u32::wrapping_sub(
      ::core::primitive::u32::wrapping_add(var_index_base, {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(num_vars);
        migrated_value
      }),
      1,
    );
    let ::core::result::Result::Ok(()) = self
      .variation_indices
      .insert_range(var_index_base..=last_var_index);

    Ok(())
  }

  fn add_glyph_id(&mut self, gid: GlyphId16) -> Result<(), Never> {
    let Ok(_presence_changed) = self.glyph_set.insert(GlyphId::from(gid));

    Ok(())
  }
}

impl ColorStop {
  fn v1_closure(&self, c: &mut Colrv1ClosureContext) -> Result<(), Never> {
    let Ok(palette_index) = self.palette_index();
    let Ok(()) = c.add_palette_index(palette_index);

    Ok(())
  }
}

impl VarColorStop {
  fn v1_closure(&self, c: &mut Colrv1ClosureContext) -> Result<(), Never> {
    let Ok(palette_index) = self.palette_index();
    let Ok(()) = c.add_palette_index(palette_index);
    let Ok(var_index_base) = self.var_index_base();
    let Ok(()) = c.add_variation_indices(var_index_base, 2);

    Ok(())
  }
}

impl ColorLine<'_> {
  fn v1_closure(&self, c: &mut Colrv1ClosureContext) -> Result<(), Never> {
    let Ok(color_stops) = self.color_stops();

    for colorstop in color_stops {
      let Ok(()) = colorstop.v1_closure(c);
    }

    Ok(())
  }
}

impl VarColorLine<'_> {
  fn v1_closure(&self, c: &mut Colrv1ClosureContext) -> Result<(), Never> {
    let Ok(color_stops) = self.color_stops();

    for var_colorstop in color_stops {
      let Ok(()) = var_colorstop.v1_closure(c);
    }

    Ok(())
  }
}

impl<'a> Paint<'a> {
  fn v1_closure(&self, context: &mut Colrv1ClosureContext<'a>) -> Result<(), Never> {
    match self {
      Self::ColrLayers(item) => item.v1_closure(context),
      Self::Solid(item) => item.v1_closure(context),
      Self::VarSolid(item) => item.v1_closure(context),
      Self::LinearGradient(item) => item.v1_closure(context),
      Self::VarLinearGradient(item) => item.v1_closure(context),
      Self::RadialGradient(item) => item.v1_closure(context),
      Self::VarRadialGradient(item) => item.v1_closure(context),
      Self::SweepGradient(item) => item.v1_closure(context),
      Self::VarSweepGradient(item) => item.v1_closure(context),
      Self::Glyph(item) => item.v1_closure(context),
      Self::ColrGlyph(item) => item.v1_closure(context),
      Self::Transform(item) => context.schedule_child(item.paint()),
      Self::VarTransform(item) => item.v1_closure(context),
      Self::Translate(item) => context.schedule_child(item.paint()),
      Self::VarTranslate(item) => {
        context.schedule_variable_child(item.paint(), || item.var_index_base(), 2)
      }
      Self::Scale(item) => context.schedule_child(item.paint()),
      Self::VarScale(item) => {
        context.schedule_variable_child(item.paint(), || item.var_index_base(), 2)
      }
      Self::ScaleAroundCenter(item) => context.schedule_child(item.paint()),
      Self::VarScaleAroundCenter(item) => {
        context.schedule_variable_child(item.paint(), || item.var_index_base(), 4)
      }
      Self::ScaleUniform(item) => context.schedule_child(item.paint()),
      Self::VarScaleUniform(item) => {
        context.schedule_variable_child(item.paint(), || item.var_index_base(), 1)
      }
      Self::ScaleUniformAroundCenter(item) => context.schedule_child(item.paint()),
      Self::VarScaleUniformAroundCenter(item) => {
        context.schedule_variable_child(item.paint(), || item.var_index_base(), 3)
      }
      Self::Rotate(item) => context.schedule_child(item.paint()),
      Self::VarRotate(item) => {
        context.schedule_variable_child(item.paint(), || item.var_index_base(), 1)
      }
      Self::RotateAroundCenter(item) => context.schedule_child(item.paint()),
      Self::VarRotateAroundCenter(item) => {
        context.schedule_variable_child(item.paint(), || item.var_index_base(), 3)
      }
      Self::Skew(item) => context.schedule_child(item.paint()),
      Self::VarSkew(item) => {
        context.schedule_variable_child(item.paint(), || item.var_index_base(), 2)
      }
      Self::SkewAroundCenter(item) => context.schedule_child(item.paint()),
      Self::VarSkewAroundCenter(item) => {
        context.schedule_variable_child(item.paint(), || item.var_index_base(), 4)
      }
      Self::Composite(item) => item.v1_closure(context),
    }
  }
}

impl<'a> PaintColrLayers<'a> {
  fn v1_closure(&self, context: &mut Colrv1ClosureContext<'a>) -> Result<(), Never> {
    let Ok(num_layers) = self.num_layers();

    match num_layers {
      0 => return Ok(()),
      _has_layers => {}
    }

    let Ok(layer_list) = readable_colr(context.colr.layer_list());
    let layer_list_2 = match layer_list.flatten() {
      Some(list) => list,
      None => return Ok(()),
    };
    let Ok(first) = self.first_layer_index();
    let last = first.wrapping_add(u32::from(num_layers)).wrapping_sub(1);
    let Ok(data) = layer_list_2.offset_data();
    let Ok(offsets) = layer_list_2.paint_offsets();

    for layer_index in first..=last {
      let Ok(index) = console_core_number_conversion::index(u64::from(layer_index));
      let offset = match offsets.get(index) {
        Some(offset) => offset,
        None => break,
      };
      let Ok(offset_2) = offset.get();
      let Ok(paint) = readable_colr(offset_2.resolve::<Paint>(data));

      match paint {
        Some(paint_2) => {
          let Ok(()) = context.add_layer_index(layer_index);
          let Ok(()) = context.schedule(paint_2);
        }
        None => continue,
      }
    }

    Ok(())
  }
}

impl PaintSolid<'_> {
  fn v1_closure(&self, c: &mut Colrv1ClosureContext) -> Result<(), Never> {
    let Ok(palette_index) = self.palette_index();
    let Ok(()) = c.add_palette_index(palette_index);

    Ok(())
  }
}

impl PaintVarSolid<'_> {
  fn v1_closure(&self, c: &mut Colrv1ClosureContext) -> Result<(), Never> {
    let Ok(palette_index) = self.palette_index();
    let Ok(()) = c.add_palette_index(palette_index);
    let Ok(var_index_base) = self.var_index_base();
    let Ok(()) = c.add_variation_indices(var_index_base, 1);

    Ok(())
  }
}

impl PaintLinearGradient<'_> {
  fn v1_closure(&self, c: &mut Colrv1ClosureContext) -> Result<(), Never> {
    let Ok(readable) = readable_colr(self.color_line());

    match readable {
      Some(colorline) => {
        let Ok(()) = colorline.v1_closure(c);
      }
      None => {}
    }

    Ok(())
  }
}

impl PaintVarLinearGradient<'_> {
  fn v1_closure(&self, c: &mut Colrv1ClosureContext) -> Result<(), Never> {
    let Ok(readable) = readable_colr(self.color_line());

    match readable {
      Some(var_colorline) => {
        let Ok(()) = var_colorline.v1_closure(c);
        let Ok(var_index_base) = self.var_index_base();
        let Ok(()) = c.add_variation_indices(var_index_base, 6);
      }
      None => {}
    }

    Ok(())
  }
}

impl PaintRadialGradient<'_> {
  fn v1_closure(&self, c: &mut Colrv1ClosureContext) -> Result<(), Never> {
    let Ok(readable) = readable_colr(self.color_line());

    match readable {
      Some(colorline) => {
        let Ok(()) = colorline.v1_closure(c);
      }
      None => {}
    }

    Ok(())
  }
}

impl PaintVarRadialGradient<'_> {
  fn v1_closure(&self, c: &mut Colrv1ClosureContext) -> Result<(), Never> {
    let Ok(readable) = readable_colr(self.color_line());

    match readable {
      Some(var_colorline) => {
        let Ok(()) = var_colorline.v1_closure(c);
        let Ok(var_index_base) = self.var_index_base();
        let Ok(()) = c.add_variation_indices(var_index_base, 6);
      }
      None => {}
    }

    Ok(())
  }
}

impl PaintSweepGradient<'_> {
  fn v1_closure(&self, c: &mut Colrv1ClosureContext) -> Result<(), Never> {
    let Ok(readable) = readable_colr(self.color_line());

    match readable {
      Some(colorline) => {
        let Ok(()) = colorline.v1_closure(c);
      }
      None => {}
    }

    Ok(())
  }
}

impl PaintVarSweepGradient<'_> {
  fn v1_closure(&self, c: &mut Colrv1ClosureContext) -> Result<(), Never> {
    let Ok(readable) = readable_colr(self.color_line());

    match readable {
      Some(var_colorline) => {
        let Ok(()) = var_colorline.v1_closure(c);
        let Ok(var_index_base) = self.var_index_base();
        let Ok(()) = c.add_variation_indices(var_index_base, 4);
      }
      None => {}
    }

    Ok(())
  }
}

impl<'a> PaintGlyph<'a> {
  fn v1_closure(&self, c: &mut Colrv1ClosureContext<'a>) -> Result<(), Never> {
    let Ok(readable) = readable_colr(self.paint());

    match readable {
      Some(paint) => {
        let Ok(glyph_id) = self.glyph_id();
        let Ok(()) = c.add_glyph_id(glyph_id);
        let Ok(()) = c.schedule(paint);
      }
      None => {}
    }

    Ok(())
  }
}

impl<'a> PaintColrGlyph<'a> {
  fn v1_closure(&self, context: &mut Colrv1ClosureContext<'a>) -> Result<(), Never> {
    let Ok(glyph_id) = self.glyph_id();
    let Ok(base_glyph_list) = readable_colr(context.colr.base_glyph_list());
    let list = match base_glyph_list.flatten() {
      Some(list) => list,
      None => return Ok(()),
    };
    let Ok(records) = list.base_glyph_paint_records();
    let Ok(record) = lookup_glyph_record(records, glyph_id, BaseGlyphPaint::glyph_id);
    let record_3 = match record {
      Some(record_2) => record_2,
      None => return Ok(()),
    };
    let Ok(data) = list.offset_data();
    let Ok(paint) = readable_colr(record_3.paint(data));

    match paint {
      Some(paint_2) => {
        let Ok(()) = context.add_glyph_id(glyph_id);

        context.schedule(paint_2)
      }
      None => Ok(()),
    }
  }
}

impl VarAffine2x3<'_> {
  fn v1_closure(&self, c: &mut Colrv1ClosureContext) -> Result<(), Never> {
    let Ok(var_index_base) = self.var_index_base();
    let Ok(()) = c.add_variation_indices(var_index_base, 6);

    Ok(())
  }
}

impl<'a> PaintVarTransform<'a> {
  fn v1_closure(&self, context: &mut Colrv1ClosureContext<'a>) -> Result<(), Never> {
    let parts = self
      .paint()
      .and_then(|paint| self.transform().map(|transform| (paint, transform)));
    let Ok(parts_2) = readable_colr(parts);

    match parts_2 {
      Some((paint, transform)) => {
        let Ok(()) = context.schedule(paint);

        transform.v1_closure(context)
      }
      None => Ok(()),
    }
  }
}

impl<'a> PaintComposite<'a> {
  fn v1_closure(&self, context: &mut Colrv1ClosureContext<'a>) -> Result<(), Never> {
    let Ok(()) = context.schedule_child(self.source_paint());

    context.schedule_child(self.backdrop_paint())
  }
}

impl Clip {
  fn v1_closure(&self, c: &mut Colrv1ClosureContext, clip_list: &ClipList) -> Result<(), Never> {
    let Ok(value_18) = clip_list.offset_data();

    let Ok(clip_box) = readable_colr(self.clip_box(value_18));
    let clip_box_3 = match clip_box {
      Some(clip_box_2) => clip_box_2,
      None => return Ok(()),
    };
    let Ok(start_glyph_id) = self.start_glyph_id();
    let start_id = GlyphId::from(start_glyph_id);
    let Ok(end_glyph_id) = self.end_glyph_id();
    let end_id = GlyphId::from(end_glyph_id);
    let Ok(value_19) = c.glyph_set.intersects_range(start_id..=end_id);

    match ::core::primitive::bool::then_some(
      value_19 == console_core_integer_sets::Overlap::Intersecting,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = clip_box_3.v1_closure(c);
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }
}

impl ClipBox<'_> {
  fn v1_closure(&self, c: &mut Colrv1ClosureContext) -> Result<(), Never> {
    match self {
      Self::Format2(item) => {
        let Ok(v1_closure) = item.v1_closure(c);

        v1_closure
      }
      Self::Format1(_) => {}
    }

    Ok(())
  }
}

impl ClipBoxFormat2<'_> {
  fn v1_closure(&self, c: &mut Colrv1ClosureContext) -> Result<(), Never> {
    let Ok(var_index_base) = self.var_index_base();
    let Ok(()) = c.add_variation_indices(var_index_base, 4);

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{FontRef, GlyphId, TableProvider};

  #[test]
  fn test_colr_v0_closure() -> Result<(), ReadError> {
    let font = FontRef::new(font_test_data::COLRV0V1_VARIABLE)?;
    let colr = font.colr()?;
    let Ok(mut input) = IntSet::empty();
    let Ok(base) = GlyphId::new(168);
    let Ok(_previous_presence) = input.insert(base);
    let Ok(mut output) = IntSet::empty();
    colr.v0_closure_glyphs(&input, &mut output);
    let Ok(()) = verify_glyphs(&output, &[5, 168, 170, 171, 172, 173, 174, 175, 176]);

    let Ok(mut palette) = IntSet::empty();
    colr.v0_closure_palette_indices(&output, &mut palette);
    let Ok(()) = verify_members(&palette, &[0, 1, 2, 3, 4, 5, 6, 10]);
    Ok(())
  }

  #[test]
  fn test_colr_v0_closure_not_found() -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::COLRV0V1_VARIABLE)?;
    let colr = font.colr()?;

    let Ok(mut input_glyph_set) = IntSet::empty();
    let Ok(_previous_presence) = (input_glyph_set).insert({
      let Ok(value) = GlyphId::new(8);
      value
    });

    let Ok(mut glyph_set_colred) = IntSet::empty();

    colr.v0_closure_glyphs(&input_glyph_set, &mut glyph_set_colred);
    let Ok(value_56) = glyph_set_colred.len();

    assert_eq!(value_56, 1);
    let Ok(value_57) = glyph_set_colred.contains({
      let Ok(value) = GlyphId::new(8);
      value
    });

    assert_eq!(value_57, console_core_integer_sets::Presence::Present);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn test_colr_v1_closure_no_var() -> Result<(), ReadError> {
    let font = FontRef::new(font_test_data::COLRV0V1_VARIABLE)?;
    let colr = font.colr()?;
    let Ok(mut glyphs) = IntSet::empty();

    for number in [220, 120] {
      let Ok(glyph) = GlyphId::new(number);
      let Ok(_previous_presence) = glyphs.insert(glyph);
    }

    let Ok(mut layers) = IntSet::empty();
    let Ok(mut palette) = IntSet::empty();
    let Ok(mut variations) = IntSet::empty();
    colr.v1_closure(&mut glyphs, &mut layers, &mut palette, &mut variations);
    let Ok(()) = verify_glyphs(&glyphs, &[6, 7, 220, 3, 2, 120]);
    let Ok(()) = verify_members(&palette, &[0, 4, 10, 11, 12]);
    let Ok(()) = verify_members(&layers, &[0, 1]);
    assert_eq!(variations.is_empty(), Ok(console_core_integer_sets::Emptiness::Empty));
    Ok(())
  }

  fn verify_glyphs(glyphs: &IntSet<GlyphId>, expected: &[u32]) -> Result<(), Never> {
    let Ok(count) = fitted::<_, u64>(expected.len());
    assert_eq!(glyphs.len(), Ok(count));

    for &number in expected {
      let Ok(glyph) = GlyphId::new(number);
      assert_eq!(glyphs.contains(glyph), Ok(Presence::Present));
    }

    Ok(())
  }

  fn verify_members<T: Domain + Copy>(set: &IntSet<T>, expected: &[T]) -> Result<(), Never> {
    let Ok(count) = fitted::<_, u64>(expected.len());
    assert_eq!(set.len(), Ok(count));

    for &member in expected {
      assert_eq!(set.contains(member), Ok(Presence::Present));
    }

    Ok(())
  }

  #[test]
  fn test_colr_v1_closure_w_var() -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::COLRV0V1_VARIABLE)?;
    let colr = font.colr()?;

    let Ok(mut glyph_set) = IntSet::empty();
    let Ok(_previous_presence) = glyph_set.insert({
      let Ok(value) = GlyphId::new(109);
      value
    });

    let Ok(mut layer_indices) = IntSet::empty();
    let Ok(mut palette_indices) = IntSet::empty();
    let Ok(mut variation_indices) = IntSet::empty();

    colr.v1_closure(
      &mut glyph_set,
      &mut layer_indices,
      &mut palette_indices,
      &mut variation_indices,
    );

    let Ok(value_78) = (glyph_set).len();

    assert_eq!(value_78, 2);
    let Ok(value_79) = (glyph_set).contains({
      let Ok(value) = GlyphId::new(3);
      value
    });

    assert_eq!(value_79, console_core_integer_sets::Presence::Present);
    let Ok(value_80) = (glyph_set).contains({
      let Ok(value) = GlyphId::new(109);
      value
    });

    assert_eq!(value_80, console_core_integer_sets::Presence::Present);

    let Ok(value_81) = palette_indices.len();

    assert_eq!(value_81, 2);
    let Ok(value_82) = palette_indices.contains(1);

    assert_eq!(value_82, console_core_integer_sets::Presence::Present);
    let Ok(value_83) = palette_indices.contains(4);

    assert_eq!(value_83, console_core_integer_sets::Presence::Present);

    let Ok(value_84) = layer_indices.is_empty();

    assert_eq!(value_84, console_core_integer_sets::Emptiness::Empty);

    let Ok(value_85) = variation_indices.len();

    assert_eq!(value_85, 6);
    let Ok(value_86) = variation_indices.contains(51);

    assert_eq!(value_86, console_core_integer_sets::Presence::Present);
    let Ok(value_87) = variation_indices.contains(52);

    assert_eq!(value_87, console_core_integer_sets::Presence::Present);
    let Ok(value_88) = variation_indices.contains(53);

    assert_eq!(value_88, console_core_integer_sets::Presence::Present);
    let Ok(value_89) = variation_indices.contains(54);

    assert_eq!(value_89, console_core_integer_sets::Presence::Present);
    let Ok(value_90) = variation_indices.contains(55);

    assert_eq!(value_90, console_core_integer_sets::Presence::Present);
    let Ok(value_91) = variation_indices.contains(56);

    assert_eq!(value_91, console_core_integer_sets::Presence::Present);
    ::core::result::Result::Ok(())
  }
}
