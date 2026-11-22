//! Copyright 2018 the Resvg Authors
//! SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::OptionLog;
use console_core_never::Never;
use console_core_number_conversion::{nearest_f32_from_i64, toward_zero_i32, toward_zero_u32};
use console_core_path_rasterizer::{
  BlendMode, FilterQuality, IntRect, Pixmap, PixmapMut, PixmapPaint, Transform,
};
use console_core_paths::{NonZeroRect, Point};
use console_core_scalable_vector_graphics::console_core_paths::StrictNum;
use console_core_scalable_vector_graphics::{Group, Node, ShouldIsolateAnswer};

const MAX_NESTED_NODES: u16 = 256;

#[derive(Clone, Copy)]
pub(super) struct Context {
  max_bbox: IntRect,
  depth_left: u16,
}

impl Context {
  pub(super) fn root(max_bbox: IntRect) -> Result<Self, Never> {
    Ok(Self {
      max_bbox,
      depth_left: MAX_NESTED_NODES,
    })
  }

  pub(super) fn with_bounds(self, max_bbox: IntRect) -> Result<Self, Never> {
    Ok(Self { max_bbox, ..self })
  }

  pub(super) fn descend(self) -> Result<Option<Self>, Never> {
    Ok(
      self
        .depth_left
        .checked_sub(1)
        .map(|depth_left| Self { depth_left, ..self }),
    )
  }
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Each child enters render_node, which consumes one unit of the inherited render depth before visiting another SVG group"
  )
)]
pub(super) fn render_nodes(
  group: &Group,
  context: &Context,
  transform: Transform,
  target: &mut PixmapMut<'_>,
) -> Result<(), Never> {
  let Ok(children) = group.children();

  for child in children {
    let Ok(()) = render_node(child, context, transform, target);
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Every group, path pattern, embedded SVG, or text render inherits a decremented depth before dispatch"
  )
)]
pub(super) fn render_node(
  node: &Node,
  context: &Context,
  transform: Transform,
  target: &mut PixmapMut<'_>,
) -> Result<(), Never> {
  let Ok(descendant) = context.descend();
  let context_3 = match descendant {
    Some(context_2) => context_2,
    None => return Ok(()),
  };

  match node {
    Node::Group(group) => {
      let Ok(_layer) = render_group(group, &context_3, transform, target);
    }
    Node::Path(path) => {
      let Ok(()) =
        crate::path::render(path, BlendMode::SourceOver, &context_3, transform, target);
    }
    Node::Image(image) => {
      let Ok(()) = crate::image::render(image, &context_3, transform, target);
    }
    Node::Text(text) => {
      let Ok(outlines) = text.flattened();
      let Ok(_layer) = render_group(outlines, &context_3, transform, target);
    }
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "The group inherits a reduced node depth, and its children, filter image content, clip, and mask reuse that same context"
  )
)]
fn render_group(
  group: &Group,
  context: &Context,
  transform: Transform,
  target: &mut PixmapMut<'_>,
) -> Result<Option<()>, Never> {
  let Ok(group_transform) = group.transform();
  let Ok(mapped) = transform.pre_concat(group_transform);
  let Ok(isolation) = group.should_isolate();

  match isolation {
    ShouldIsolateAnswer::No => {
      let Ok(()) = render_nodes(group, context, mapped, target);
      Ok(Some(()))
    }
    ShouldIsolateAnswer::Yes => {
      let Ok(()) = isolated_group(group, context, mapped, target);
      Ok(Some(()))
    }
  }
}

enum LayerKind {
  Paint,
  Filter,
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Children and filter image inserts consume inherited render depth, and clip and mask processing consume depth independently along each chain"
  )
)]
fn isolated_group(
  group: &Group,
  context: &Context,
  transform: Transform,
  target: &mut PixmapMut<'_>,
) -> Result<(), Never> {
  let Ok(layer) = group.layer_bounding_box();
  let Ok(mapped) = layer.transform(transform);
  let placed_layer = match mapped {
    Some(layer_2) => layer_2,
    None => return Ok(()),
  };
  let Ok(filters) = group.filters();
  let kind = match filters.is_empty().then_some(()) {
    Some(()) => LayerKind::Paint,
    None => LayerKind::Filter,
  };
  let Ok(geometry) = pixel_bounds(placed_layer, kind);
  let rectangle = match geometry {
    Some(rectangle) => rectangle,
    None => return Ok(()),
  };
  let Ok(clipped) = crate::geom::fit_to_rect(rectangle, context.max_bbox);
  let bounds = match clipped {
    Some(bounds) => bounds,
    None => return Ok(()),
  };
  let Ok(shift) = layer_translation(placed_layer, bounds);
  let Ok(local) = shift.pre_concat(transform);
  let Ok(width) = bounds.width();
  let Ok(height) = bounds.height();
  let Ok(allocated) = Pixmap::new(width, height);
  let mut bitmap = match allocated.log_none(|| {
    console_core_logging::warn!("Failed to allocate a group layer for {:?}.", bounds)
  }) {
    Some(bitmap) => bitmap,
    None => return Ok(()),
  };
  let Ok(mut view) = bitmap.as_mut();
  let Ok(()) = render_nodes(group, context, local, &mut view);

  for filter in filters {
    let Ok(()) = crate::filter::apply(filter, context, local, &mut bitmap);
  }

  let Ok(clip) = group.clip_path();

  match clip {
    Some(clip_2) => {
      let Ok(()) = crate::clip::apply(clip_2, context, local, &mut bitmap);
    }
    None => {}
  }

  let Ok(mask) = group.mask();

  match mask {
    Some(mask_2) => {
      let Ok(()) = crate::mask::apply(mask_2, context, local, &mut bitmap);
    }
    None => {}
  }

  let Ok(opacity) = group.opacity();
  let Ok(blend) = group.blend_mode();
  let Ok(blend_mode) = convert_blend_mode(blend);
  let paint = PixmapPaint {
    opacity: opacity.get(),
    blend_mode,
    quality: FilterQuality::Nearest,
  };
  let Ok(x) = bounds.x();
  let Ok(y) = bounds.y();
  let Ok(pixels) = bitmap.as_ref();
  let Ok(identity) = Transform::identity();
  let Ok(()) = target.draw_pixmap(x, y, pixels, &paint, identity, None);

  Ok(())
}

fn pixel_bounds(float_bounds: NonZeroRect, kind: LayerKind) -> Result<Option<IntRect>, Never> {
  let Ok(x) = float_bounds.x();
  let Ok(y) = float_bounds.y();
  let Ok(width) = float_bounds.width();
  let Ok(height) = float_bounds.height();
  let Ok(left) = toward_zero_i32(f64::from(x.floor()));
  let Ok(top) = toward_zero_i32(f64::from(y.floor()));
  let Ok(w) = toward_zero_u32(f64::from(width.ceil()));
  let Ok(h) = toward_zero_u32(f64::from(height.ceil()));

  let (left_2, top_2, width_3, height_3) = match kind {
    LayerKind::Paint => {
      match (
        left.checked_sub(2),
        top.checked_sub(2),
        w.checked_add(4),
        h.checked_add(4),
      ) {
        (Some(x_2), Some(y_2), Some(width_2), Some(height_2)) => (x_2, y_2, width_2, height_2),
        _ => return Ok(None),
      }
    }
    LayerKind::Filter => (left, top, w.max(1), h.max(1)),
  };
  IntRect::from_xywh(left_2, top_2, width_3, height_3)
}

fn layer_translation(original: NonZeroRect, pixels: IntRect) -> Result<Transform, Never> {
  let Ok(x) = original.x();
  let Ok(y) = original.y();
  let Ok(canvas_x) = pixels.x();
  let Ok(canvas_y) = pixels.y();
  let Ok(left) = nearest_f32_from_i64(i64::from(canvas_x));
  let Ok(top) = nearest_f32_from_i64(i64::from(canvas_y));

  Transform::from_translation(Point {
    x: -(x - (x - left)),
    y: -(y - (y - top)),
  })
}

pub(super) fn convert_blend_mode(
  mode: console_core_scalable_vector_graphics::BlendMode,
) -> Result<BlendMode, Never> {
  Ok(match mode {
    console_core_scalable_vector_graphics::BlendMode::Normal => {
      console_core_path_rasterizer::BlendMode::SourceOver
    }
    console_core_scalable_vector_graphics::BlendMode::Multiply => {
      console_core_path_rasterizer::BlendMode::Multiply
    }
    console_core_scalable_vector_graphics::BlendMode::Screen => {
      console_core_path_rasterizer::BlendMode::Screen
    }
    console_core_scalable_vector_graphics::BlendMode::Overlay => {
      console_core_path_rasterizer::BlendMode::Overlay
    }
    console_core_scalable_vector_graphics::BlendMode::Darken => {
      console_core_path_rasterizer::BlendMode::Darken
    }
    console_core_scalable_vector_graphics::BlendMode::Lighten => {
      console_core_path_rasterizer::BlendMode::Lighten
    }
    console_core_scalable_vector_graphics::BlendMode::ColorDodge => {
      console_core_path_rasterizer::BlendMode::ColorDodge
    }
    console_core_scalable_vector_graphics::BlendMode::ColorBurn => {
      console_core_path_rasterizer::BlendMode::ColorBurn
    }
    console_core_scalable_vector_graphics::BlendMode::HardLight => {
      console_core_path_rasterizer::BlendMode::HardLight
    }
    console_core_scalable_vector_graphics::BlendMode::SoftLight => {
      console_core_path_rasterizer::BlendMode::SoftLight
    }
    console_core_scalable_vector_graphics::BlendMode::Difference => {
      console_core_path_rasterizer::BlendMode::Difference
    }
    console_core_scalable_vector_graphics::BlendMode::Exclusion => {
      console_core_path_rasterizer::BlendMode::Exclusion
    }
    console_core_scalable_vector_graphics::BlendMode::Hue => {
      console_core_path_rasterizer::BlendMode::Hue
    }
    console_core_scalable_vector_graphics::BlendMode::Saturation => {
      console_core_path_rasterizer::BlendMode::Saturation
    }
    console_core_scalable_vector_graphics::BlendMode::Color => {
      console_core_path_rasterizer::BlendMode::Color
    }
    console_core_scalable_vector_graphics::BlendMode::Luminosity => {
      console_core_path_rasterizer::BlendMode::Luminosity
    }
  })
}

#[cfg(test)]
mod tests {

  #[test]
  fn filter_bbox_outside_int_rect() -> Result<(), &'static str> {
    let svg = r#"<svg filter="url(#f)"><filter id="f" x="2em"><feFlood/></filter><path d="M0 0H1e8V1"/></svg>"#;
    let tree = console_core_scalable_vector_graphics::Tree::from_str(
      svg,
      &console_core_scalable_vector_graphics::Options::default(),
    )
    .map_err(|_| "parse SVG")?;
    let Ok(allocated) = console_core_path_rasterizer::Pixmap::new(1, 1);
    let mut pixmap = allocated.ok_or("allocate canvas")?;

    let Ok(identity) = console_core_path_rasterizer::Transform::identity();

    crate::render(
      &tree,
      identity,
      &mut ({
        let Ok(view) = pixmap.as_mut();
        view
      }),
    );

    Ok(())
  }
}
