//! Copyright 2019 the Resvg Authors.
//! SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::render::Context;
use console_core_never::Never;
use console_core_path_rasterizer::{
  Color, CoverageMode, FillRule, Mask, MaskType, PathBuilder, Pixmap, Transform,
};
use console_core_scalable_vector_graphics::{Mask as SvgMask, MaskType as SvgMaskType};

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Each nested mask and its rendered children reuse the depth budget decremented at this entry"
  )
)]
pub(super) fn apply(
  mask: &SvgMask,
  context: &Context,
  transform: Transform,
  target: &mut Pixmap,
) -> Result<(), Never> {
  let Ok(descendant) = context.descend();
  let context_3 = match descendant {
    Some(context_2) => context_2,
    None => return Ok(()),
  };

  let Ok(root) = mask.root();
  let Ok(children) = root.children();

  match children.is_empty().then_some(()) {
    Some(()) => return target.fill(Color::TRANSPARENT),
    None => {}
  }

  let Ok(size) = target.size();
  let Ok(allocated) = Pixmap::with_size(size);
  let mut bitmap = match allocated {
    Some(bitmap) => bitmap,
    None => return Ok(()),
  };

  let Ok(mut bounds_mask) = Mask::with_size(size);
  let Ok(bounds) = mask.rect();
  let Ok(rectangle) = bounds.to_rect();
  let Ok(path) = PathBuilder::from_rect(rectangle);
  let Ok(()) = bounds_mask.fill_path(
    &path,
    FillRule::Winding,
    CoverageMode::Antialiased,
    transform,
  );
  let Ok(mut view) = bitmap.as_mut();
  let Ok(()) = crate::render::render_nodes(root, &context_3, transform, &mut view);
  let Ok(()) = bitmap.apply_mask(&bounds_mask);
  let Ok(referenced) = mask.mask();

  match referenced {
    Some(nested) => {
      let Ok(()) = apply(nested, &context_3, transform, target);
    }
    None => {}
  }

  let Ok(meaning) = mask.kind();
  let coverage = match meaning {
    SvgMaskType::Luminance => MaskType::Luminance,
    SvgMaskType::Alpha => MaskType::Alpha,
  };
  let Ok(view_2) = bitmap.as_ref();
  let Ok(alpha) = Mask::from_pixmap(view_2, coverage);

  target.apply_mask(&alpha)
}
