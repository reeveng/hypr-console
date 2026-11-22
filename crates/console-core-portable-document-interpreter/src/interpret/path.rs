//! Painting the path the content stream built: filled, stroked or both, and the paint each of
//! those reads from the graphics state.
//!
//! A path with no width or no height fills nothing a person could see, though a reader shows it,
//! so it is drawn as a hairline from corner to corner instead (pdf.js issue 4260). A path that is
//! an upright rectangle reaches the device as one, which most devices fill faster than a path.

use crate::color::{Color, ColorSpace};
use crate::context::{Context, path_as_rect};
use crate::device::Device;
use crate::util::{Nearly, Nearness, fast_bounding_box};
use crate::{
  FillRule, Paint, PaintOperation, PathDrawMode, PatternSpace, StrokeProps, Visibility,
};
use console_core_never::Never;
use kurbo::{Affine, BezPath, Cap, Join, PathEl};

pub(crate) fn fill_path<'a>(
  context: &mut Context<'a>,
  device: &mut impl Device<'a>,
  fill_rule: FillRule,
) -> Result<(), Never> {
  let Ok(()) = fill_path_impl(context, device, fill_rule, None);

  context.path.truncate(0);

  Ok(())
}

pub(crate) fn stroke_path<'a>(
  context: &mut Context<'a>,
  device: &mut impl Device<'a>,
) -> Result<(), Never> {
  let Ok(()) = stroke_path_impl(context, device, None);

  context.path.truncate(0);

  Ok(())
}

pub(crate) fn fill_stroke_path<'a>(
  context: &mut Context<'a>,
  device: &mut impl Device<'a>,
  fill_rule: FillRule,
) -> Result<(), Never> {
  let Ok(()) = fill_path_impl(context, device, fill_rule, None);
  let Ok(()) = stroke_path_impl(context, device, None);

  context.path.truncate(0);

  Ok(())
}

pub(crate) fn close_path(context: &mut Context<'_>) -> Result<(), Never> {
  match context.path.elements().last() {
    Some(PathEl::ClosePath) | None => {}
    Some(
      PathEl::MoveTo(_) | PathEl::LineTo(_) | PathEl::QuadTo(_, _) | PathEl::CurveTo(_, _, _),
    ) => {
      context.path.close_path();
      context.last_point = context.sub_path_start;
    }
  }

  Ok(())
}

pub(crate) fn fill_path_impl<'a>(
  context: &mut Context<'a>,
  device: &mut impl Device<'a>,
  fill_rule: FillRule,
  path: Option<&BezPath>,
) -> Result<(), Never> {
  let Ok(visibility) = context.ocg_state.is_visible();

  match visibility {
    Visibility::Visible => {}
    Visibility::Hidden => return Ok(()),
  }

  let Ok(paint) = get_paint(context, PaintOperation::Fill);
  let Ok(state) = context.get();
  let transform = state.ctm;

  device.set_soft_mask(state.graphics_state.soft_mask.clone());
  device.set_blend_mode(state.graphics_state.blend_mode);

  let path_3 = match path {
    Some(path_2) => path_2,
    None => &context.path,
  };
  let Ok(bbox) = fast_bounding_box(path_3);
  let Ok(wide) = bbox.width().nearly(0.0);
  let Ok(high) = bbox.height().nearly(0.0);

  match (wide, high) {
    (Nearly::Equal, _) | (_, Nearly::Equal) => {
      let mut hairline = BezPath::new();

      hairline.move_to((bbox.x0, bbox.y0));
      hairline.line_to((bbox.x1, bbox.y1));

      let stroke_props = StrokeProps {
        line_width: 0.001,
        line_join: Join::Bevel,
        line_cap: Cap::Butt,
        ..Default::default()
      };

      device.draw_path(
        &hairline,
        transform,
        &paint,
        &PathDrawMode::Stroke(stroke_props),
      );

      Ok(())
    }
    (
      Nearly::Below | Nearly::Above | Nearly::Unordered,
      Nearly::Below | Nearly::Above | Nearly::Unordered,
    ) => draw(
      device,
      path_3,
      transform,
      &paint,
      &PathDrawMode::Fill(fill_rule),
    ),
  }
}

pub(crate) fn stroke_path_impl<'a>(
  context: &mut Context<'a>,
  device: &mut impl Device<'a>,
  path: Option<&BezPath>,
) -> Result<(), Never> {
  let Ok(visibility) = context.ocg_state.is_visible();

  match visibility {
    Visibility::Visible => {}
    Visibility::Hidden => return Ok(()),
  }

  let Ok(stroke_props) = context.stroke_props();
  let Ok(paint) = get_paint(context, PaintOperation::Stroke);
  let Ok(state) = context.get();
  let transform = state.ctm;

  device.set_soft_mask(state.graphics_state.soft_mask.clone());
  device.set_blend_mode(state.graphics_state.blend_mode);

  let path_3 = match path {
    Some(path_2) => path_2,
    None => &context.path,
  };

  draw(
    device,
    path_3,
    transform,
    &paint,
    &PathDrawMode::Stroke(stroke_props),
  )
}

fn draw<'a>(
  device: &mut impl Device<'a>,
  path: &BezPath,
  transform: Affine,
  paint: &Paint<'a>,
  draw_mode: &PathDrawMode,
) -> Result<(), Never> {
  let Ok(rect) = path_as_rect(path);

  match rect {
    Some(rect_2) => device.draw_rect(&rect_2, transform, paint, draw_mode),
    None => {
      device.draw_path(path, transform, paint, draw_mode);

      Ok(())
    }
  }
}

pub(crate) fn get_paint<'a>(
  context: &Context<'a>,
  operation: PaintOperation,
) -> Result<Paint<'a>, Never> {
  let Ok(state) = context.get();
  let Ok(data) = match operation {
    PaintOperation::Stroke => state.stroke_data(),
    PaintOperation::Fill => state.non_stroke_data(),
  };
  let Ok(space) = data.color_space.is_pattern();

  Ok(match (space, data.pattern) {
    (_, Some(mut pattern)) => {
      match &data.transfer_function {
        Some(transfer) => {
          let Ok(()) = pattern.set_transfer_function(transfer.clone());
        }
        None => {}
      }

      let Ok(root) = context.root_transform();
      let Ok(()) = pattern.pre_concat_transform(root);

      Paint::Pattern(Box::new(pattern))
    }
    (PatternSpace::Pattern, None) => {
      let Ok(gray) = ColorSpace::device_gray();
      let Ok(clear) = Color::new(gray, vec![0.0], crate::interpret::state::Opacity(0.0));

      Paint::Color(clear)
    }
    (PatternSpace::NotPattern, None) => {
      let Ok(color) = Color::new(data.color_space, data.color, data.alpha);

      match &data.transfer_function {
        Some(transfer) => {
          let Ok(rgba) = color.to_rgba();
          let Ok(transferred) = transfer.apply(&rgba);
          let Ok(color_2) = Color::from_rgba(transferred);

          Paint::Color(color_2)
        }
        None => Paint::Color(color),
      }
    }
  })
}
