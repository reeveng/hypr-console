//! Copyright 2019 the Resvg Authors.
//! SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::render::Context;
use console_core_never::Never;
use console_core_path_rasterizer::{
  BlendMode, Color, IntRect, Mask, MaskType, Pixmap, PixmapMut, PixmapPaint, Transform,
};
use console_core_scalable_vector_graphics::{ClipPath, Group, IsVisibleAnswer, Node};

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Each nested clip or clip group receives a decremented context before further traversal"
  )
)]
pub(super) fn apply(
  clip: &ClipPath,
  context: &Context,
  transform: Transform,
  target: &mut Pixmap,
) -> Result<(), Never> {
  let Ok(descendant) = context.descend();
  let context_3 = match descendant {
    Some(context_2) => context_2,
    None => return Ok(()),
  };

  let Ok(size) = target.size();
  let Ok(allocated) = Pixmap::with_size(size);
  let mut bitmap = match allocated {
    Some(bitmap) => bitmap,
    None => return Ok(()),
  };
  let Ok(()) = bitmap.fill(Color::BLACK);
  let Ok(root) = clip.root();
  let Ok(local) = clip.transform();
  let Ok(mapped) = transform.pre_concat(local);
  let Ok(mut view) = bitmap.as_mut();
  let Ok(()) = draw_children(root, BlendMode::Clear, &context_3, mapped, &mut view);
  let Ok(nested) = clip.clip_path();

  match nested {
    Some(nested_clip) => {
      let Ok(()) = apply(nested_clip, &context_3, transform, target);
    }
    None => {}
  }

  let Ok(image) = bitmap.as_ref();
  let Ok(mut alpha) = Mask::from_pixmap(image, MaskType::Alpha);
  let Ok(()) = alpha.invert();
  target.apply_mask(&alpha)
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "The inherited clip depth decreases before paths, groups, and flattened text are drawn"
  )
)]
fn draw_children(
  parent: &Group,
  blend: BlendMode,
  context: &Context,
  transform: Transform,
  target: &mut PixmapMut<'_>,
) -> Result<(), Never> {
  let Ok(descendant) = context.descend();
  let context_3 = match descendant {
    Some(context_2) => context_2,
    None => return Ok(()),
  };

  let Ok(unit) = IntRect::from_xywh(0, 0, 1, 1);
  let bounds = match unit {
    Some(bounds) => bounds,
    None => return Ok(()),
  };

  let Ok(path_context) = context_3.with_bounds(bounds);
  let Ok(children) = parent.children();

  for child in children {
    match child {
      Node::Path(path) => {
        let Ok(visibility) = path.is_visible();

        match visibility {
          IsVisibleAnswer::Yes => {
            let Ok(()) = crate::path::fill_path(path, blend, &path_context, transform, target);
          }
          IsVisibleAnswer::No => {}
        }
      }
      Node::Text(text) => {
        let Ok(outlines) = text.flattened();
        let Ok(()) = draw_children(outlines, blend, &context_3, transform, target);
      }
      Node::Group(group) => {
        let Ok(()) = draw_group(group, blend, &context_3, transform, target);
      }
      Node::Image(_) => {}
    }
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Nested clip groups and clip paths consume the supplied context before drawing further descendants"
  )
)]
fn draw_group(
  group: &Group,
  blend: BlendMode,
  context: &Context,
  transform: Transform,
  target: &mut PixmapMut<'_>,
) -> Result<(), Never> {
  let Ok(own_transform) = group.transform();
  let Ok(mapped) = transform.pre_concat(own_transform);
  let Ok(nested) = group.clip_path();

  match nested {
    Some(clip) => clip_group(group, clip, context, mapped, target),
    None => draw_children(group, blend, context, mapped, target),
  }
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Drawing clip content and applying nested clip paths both consume the inherited depth"
  )
)]
fn clip_group(
  group: &Group,
  clip: &ClipPath,
  context: &Context,
  transform: Transform,
  target: &mut PixmapMut<'_>,
) -> Result<(), Never> {
  let Ok(size) = crate::target_size(target);
  let size_2 = match size {
    Some(size_3) => size_3,
    None => return Ok(()),
  };
  let Ok(allocated) = Pixmap::with_size(size_2);
  let mut bitmap = match allocated {
    Some(bitmap) => bitmap,
    None => return Ok(()),
  };
  let Ok(mut view) = bitmap.as_mut();
  let Ok(()) = draw_children(group, BlendMode::SourceOver, context, transform, &mut view);
  let Ok(()) = apply(clip, context, transform, &mut bitmap);
  let paint = PixmapPaint {
    blend_mode: BlendMode::Xor,
    ..PixmapPaint::default()
  };
  let Ok(image) = bitmap.as_ref();
  let Ok(identity) = Transform::identity();
  let Ok(()) = target.draw_pixmap(0, 0, image, &paint, identity, None);

  Ok(())
}
