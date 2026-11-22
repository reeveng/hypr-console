//! Rasterizes SVG trees and nodes into sRGB pixmaps.
//! Copyright 2017 the Resvg Authors.
//! SPDX-License-Identifier: Apache-2.0 OR MIT

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub use console_core_path_rasterizer;
pub use console_core_scalable_vector_graphics;

use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_path_rasterizer::{IntRect, IntSize, PixmapMut, Transform};
use console_core_paths::Point;
use console_core_scalable_vector_graphics::{Node, Tree};

mod clip;
mod filter;
mod geom;
mod image;
mod mask;
mod path;
mod render;

#[doc = "Draws the SVG tree in sRGB with the supplied root transform."]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "This is the established public draw entry point, which reports no value"
  )
)]
pub fn render(tree: &Tree, transform: Transform, pixmap: &mut PixmapMut<'_>) {
  let Ok(size) = target_size(pixmap);
  let size_3 = match size {
    Some(size_2) => size_2,
    None => return,
  };
  let Ok(bounds) = max_filter_bbox(size_3);
  let bounds_3 = match bounds {
    Some(bounds_2) => bounds_2,
    None => return,
  };
  let Ok(context) = render::Context::root(bounds_3);
  let Ok(root) = tree.root();
  let Ok(()) = render::render_nodes(root, &context, transform, pixmap);
}

#[doc = "Draws one node in sRGB. Returns None when its layer has no size."]
#[doc = "Use the node's absolute layer bounding box to determine the expected pixmap size."]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "The established public node renderer reports an empty node through Option"
  )
)]
pub fn render_node(node: &Node, transform: Transform, pixmap: &mut PixmapMut<'_>) -> Option<()> {
  let Ok(layer) = node.abs_layer_bounding_box();
  let bbox = layer?;
  let Ok(size) = target_size(pixmap);
  let size_2 = size?;
  let Ok(max_bounds) = max_filter_bbox(size_2);
  let max_bounds_2 = max_bounds?;
  let Ok(context) = render::Context::root(max_bounds_2);
  let Ok(x) = bbox.x();
  let Ok(y) = bbox.y();
  let Ok(shifted) = transform.pre_translate_by(Point { x: -x, y: -y });
  let Ok(()) = render::render_node(node, &context, shifted, pixmap);

  Some(())
}

pub(crate) trait OptionLog {
  fn log_none<F: FnOnce()>(self, f: F) -> Self;
}

impl<T> OptionLog for Option<T> {
  #[inline]
  fn log_none<F: FnOnce()>(self, f: F) -> Self {
    self.or_else(|| {
      f();
      None
    })
  }
}

fn target_size(pixmap: &PixmapMut<'_>) -> Result<Option<IntSize>, Never> {
  let Ok(width) = pixmap.width();
  let Ok(height) = pixmap.height();

  IntSize::from_wh(width, height)
}

fn max_filter_bbox(size: IntSize) -> Result<Option<IntRect>, Never> {
  let Ok((width, height)) = size.dimensions();
  let Ok(signed_width) = fitted::<u32, i32>(width);
  let Ok(signed_height) = fitted::<u32, i32>(height);
  let Ok(bounds) = IntRect::from_xywh(
    signed_width.saturating_mul(-2),
    signed_height.saturating_mul(-2),
    width.saturating_mul(5),
    height.saturating_mul(5),
  );

  match bounds {
    Some(bounds_2) => Ok(Some(bounds_2)),
    None => IntRect::from_ltrb(::core::primitive::i32::wrapping_div(i32::MIN, 2), ::core::primitive::i32::wrapping_div(i32::MIN, 2), ::core::primitive::i32::wrapping_div(i32::MAX, 2), ::core::primitive::i32::wrapping_div(i32::MAX, 2)),
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn max_filter_bbox_is_clamped() -> Result<(), &'static str> {
    let Ok(size) = super::IntSize::from_wh(u32::MAX, u32::MAX);
    let size_2 = size.ok_or("construct size")?;
    let Ok(bounds) = super::max_filter_bbox(size_2);
    let bbox = bounds.ok_or("construct fallback bounds")?;
    assert_eq!(bbox.left(), Ok(::core::primitive::i32::wrapping_div(i32::MIN, 2)));
    assert_eq!(bbox.top(), Ok(::core::primitive::i32::wrapping_div(i32::MIN, 2)));
    assert_eq!(bbox.right(), Ok(::core::primitive::i32::wrapping_div(i32::MAX, 2)));
    assert_eq!(bbox.bottom(), Ok(::core::primitive::i32::wrapping_div(i32::MAX, 2)));

    Ok(())
  }
}
