//! Copyright 2019 the Resvg Authors.
//! SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::render::Context;
use console_core_never::Never;
use console_core_number_conversion::toward_zero_u32;
use console_core_path_rasterizer::{
  BlendMode, Color, CoverageMode, FillRule, FilterQuality, GradientStop, IntSize, LinearGradient,
  Paint, Pattern, Pixmap, PixmapMut, RadialGradient, Shader, SpreadMode, Transform,
};
use console_core_paths::{Point, ScaleFactors};
use console_core_scalable_vector_graphics::console_core_paths::{StrictNormalized, StrictNum};
use console_core_scalable_vector_graphics::{
  BaseGradient, FillRule as SvgFillRule, IsVisibleAnswer, LinearGradient as SvgLinearGradient,
  Opacity, Paint as SvgPaint, PaintOrder, Path, Pattern as SvgPattern,
  RadialGradient as SvgRadialGradient, SpreadMethod, UseShapeAntialiasingAnswer,
};

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Fill and stroke retain the current node’s bounded context when they descend into pattern paint"
  )
)]
pub(super) fn render(
  path: &Path,
  blend: BlendMode,
  context: &Context<'_>,
  transform: Transform,
  target: &mut PixmapMut<'_>,
) -> Result<(), Never> {
  let Ok(visibility) = path.is_visible();

  match visibility {
    IsVisibleAnswer::Yes => {}
    IsVisibleAnswer::No => return Ok(()),
  }

  let Ok(order) = path.paint_order();

  match order {
    PaintOrder::FillAndStroke => {
      let Ok(()) = fill_path(path, blend, context, transform, target);
      let Ok(()) = stroke_path(path, blend, context, transform, target);
    }
    PaintOrder::StrokeAndFill => {
      let Ok(()) = stroke_path(path, blend, context, transform, target);
      let Ok(()) = fill_path(path, blend, context, transform, target);
    }
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "The inherited context is passed to the bounded pattern renderer"
  )
)]
pub(super) fn fill_path(
  path: &Path,
  blend: BlendMode,
  context: &Context<'_>,
  transform: Transform,
  target: &mut PixmapMut<'_>,
) -> Result<(), Never> {
  let Ok(fill) = path.fill();
  let fill_3 = match fill {
    Some(fill_2) => fill_2,
    None => return Ok(()),
  };
  let Ok(data) = path.data();
  let Ok(bounds) = data.bounds();
  let Ok(width) = bounds.width();
  let Ok(height) = bounds.height();

  use core::num::FpCategory::Zero;

  match (width.classify(), height.classify()) {
    (Zero, _) | (_, Zero) => return Ok(()),
    _ => {}
  }

  let Ok(fill_rule) = fill_3.rule();
  let rule = match fill_rule {
    SvgFillRule::NonZero => FillRule::Winding,
    SvgFillRule::EvenOdd => FillRule::EvenOdd,
  };
  let Ok(opacity) = fill_3.opacity();
  let Ok(source) = fill_3.paint();
  let Ok(coverage) = coverage_for(path);

  with_paint(
    source,
    PaintSetup {
      blend,
      coverage,
      context,
      opacity,
      transform,
    },
    ShapePaint::Fill { path: data, rule },
    target,
  )
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "The inherited context is passed to the bounded pattern renderer"
  )
)]
fn stroke_path(
  path: &Path,
  blend: BlendMode,
  context: &Context<'_>,
  transform: Transform,
  target: &mut PixmapMut<'_>,
) -> Result<(), Never> {
  let Ok(stroke) = path.stroke();
  let stroke_3 = match stroke {
    Some(stroke_2) => stroke_2,
    None => return Ok(()),
  };

  let Ok(data) = path.data();
  let Ok(outline) = stroke_3.to_tiny_skia();
  let Ok(opacity) = stroke_3.opacity();
  let Ok(source) = stroke_3.paint();
  let Ok(coverage) = coverage_for(path);

  with_paint(
    source,
    PaintSetup {
      blend,
      coverage,
      context,
      opacity,
      transform,
    },
    ShapePaint::Stroke {
      path: data,
      style: &outline,
    },
    target,
  )
}

fn coverage_for(path: &Path) -> Result<CoverageMode, Never> {
  let Ok(mode) = path.rendering_mode();
  let Ok(antialiasing) = mode.use_shape_antialiasing();

  Ok(match antialiasing {
    UseShapeAntialiasingAnswer::Yes => CoverageMode::Antialiased,
    UseShapeAntialiasingAnswer::No => CoverageMode::Binary,
  })
}

enum ShapePaint<'a> {
  Fill {
    path: &'a console_core_path_rasterizer::Path,
    rule: FillRule,
  },
  Stroke {
    path: &'a console_core_path_rasterizer::Path,
    style: &'a console_core_path_rasterizer::Stroke,
  },
}

struct PaintSetup<'a> {
  blend: BlendMode,
  coverage: CoverageMode,
  context: &'a Context<'a>,
  opacity: Opacity,
  transform: Transform,
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Pattern rendering reuses the node’s depth budget and consumes it for each child"
  )
)]
fn with_paint(
  paint: &SvgPaint,
  setup: PaintSetup<'_>,
  shape: ShapePaint<'_>,
  target: &mut PixmapMut<'_>,
) -> Result<(), Never> {
  let tile;
  let shader = match paint {
    SvgPaint::Color(rgb) => {
      let Ok(color) = Color::from_rgba8(rgb.red, rgb.green, rgb.blue, setup.opacity.to_u8());
      Some(Shader::SolidColor(color))
    }
    SvgPaint::LinearGradient(gradient) => {
      let Ok(converted) = convert_linear_gradient(gradient, setup.opacity);
      converted
    }
    SvgPaint::RadialGradient(gradient) => {
      let Ok(converted) = convert_radial_gradient(gradient, setup.opacity);
      converted
    }
    SvgPaint::Pattern(pattern) => {
      let Ok(rendered) = render_pattern_pixmap(pattern, setup.context, setup.transform);
      let (canvas, placement) = match rendered {
        Some(tile_2) => tile_2,
        None => return Ok(()),
      };
      tile = canvas;
      let Ok(pixels) = tile.as_ref();
      let Ok(shader) = Pattern::new(
        pixels,
        SpreadMode::Repeat,
        FilterQuality::Bicubic,
        setup.opacity.get(),
        placement,
      );

      Some(shader)
    }
  };

  let shader_3 = match shader {
    Some(shader_2) => shader_2,
    None => return Ok(()),
  };
  let paint_2 = Paint {
    shader: shader_3,
    blend_mode: setup.blend,
    coverage: setup.coverage,
    ..Paint::default()
  };

  match shape {
    ShapePaint::Fill { path, rule } => {
      target.fill_path(path, &paint_2, rule, setup.transform, None)
    }
    ShapePaint::Stroke { path, style } => {
      target.stroke_path(path, &paint_2, style, setup.transform, None)
    }
  }
}

fn convert_linear_gradient(
  gradient: &SvgLinearGradient,
  opacity: Opacity,
) -> Result<Option<Shader<'static>>, Never> {
  let Ok((mode, stops)) = convert_base_gradient(gradient, opacity);
  let Ok(x1) = gradient.x1();
  let Ok(y1) = gradient.y1();
  let Ok(x2) = gradient.x2();
  let Ok(y2) = gradient.y2();
  let Ok(transform) = gradient.transform();

  LinearGradient::new(
    Point { x: x1, y: y1 },
    Point { x: x2, y: y2 },
    stops,
    mode,
    transform,
  )
}

fn convert_radial_gradient(
  gradient: &SvgRadialGradient,
  opacity: Opacity,
) -> Result<Option<Shader<'static>>, Never> {
  let Ok((mode, stops)) = convert_base_gradient(gradient, opacity);
  let Ok(fx) = gradient.fx();
  let Ok(fy) = gradient.fy();
  let Ok(fr) = gradient.fr();
  let Ok(cx) = gradient.cx();
  let Ok(cy) = gradient.cy();
  let Ok(radius) = gradient.r();
  let Ok(transform) = gradient.transform();

  RadialGradient::new(
    Point { x: fx, y: fy },
    fr.get(),
    Point { x: cx, y: cy },
    radius.get(),
    stops,
    mode,
    transform,
  )
}

fn convert_base_gradient(
  gradient: &BaseGradient,
  opacity: Opacity,
) -> Result<(SpreadMode, Vec<GradientStop>), Never> {
  let Ok(spread) = gradient.spread_method();
  let mode = match spread {
    SpreadMethod::Pad => SpreadMode::Pad,
    SpreadMethod::Reflect => SpreadMode::Reflect,
    SpreadMethod::Repeat => SpreadMode::Repeat,
  };
  let Ok(stops) = gradient.stops();
  let mut points = Vec::with_capacity(stops.len());

  for stop in stops {
    let Ok(stop_opacity) = stop.opacity();
    let alpha = stop_opacity * opacity;
    let Ok(rgb) = stop.color();
    let Ok(offset) = stop.offset();
    let Ok(color) = Color::from_rgba8(rgb.red, rgb.green, rgb.blue, alpha.to_u8());
    let Ok(point) = GradientStop::new(offset.get(), color);
    points.push(point);
  }

  Ok((mode, points))
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Each pattern child enters the renderer with the remaining depth of its ancestor node"
  )
)]
fn render_pattern_pixmap(
  pattern: &SvgPattern,
  context: &Context<'_>,
  transform: Transform,
) -> Result<Option<(Pixmap, Transform)>, Never> {
  let Ok(pattern_transform) = pattern.transform();
  let Ok(combined) = transform.pre_concat(pattern_transform);
  let Ok((sx, sy)) = combined.get_scale();
  let Ok(rect) = pattern.rect();
  let Ok(width) = rect.width();
  let Ok(height) = rect.height();
  let Ok(w) = toward_zero_u32(f64::from((width * sx).round()));
  let Ok(h) = toward_zero_u32(f64::from((height * sy).round()));
  let Ok(dimensions) = IntSize::from_wh(w, h);
  let dimensions_3 = match dimensions {
    Some(dimensions_2) => dimensions_2,
    None => return Ok(None),
  };
  let Ok(allocated) = Pixmap::with_size(dimensions_3);
  let mut pixmap = match allocated {
    Some(pixmap) => pixmap,
    None => return Ok(None),
  };
  let Ok(local) = Transform::from_scale_factors(ScaleFactors { x: sx, y: sy });
  let Ok(root) = pattern.root();
  let Ok(mut view) = pixmap.as_mut();
  let Ok(()) = crate::render::render_nodes(root, context, local, &mut view);

  let Ok(x) = rect.x();
  let Ok(y) = rect.y();
  let Ok(translated) = pattern_transform.pre_translate_by(Point { x, y });
  let Ok(placement) = translated.pre_scale_by(ScaleFactors {
    x: 1.0 / sx,
    y: 1.0 / sy,
  });

  Ok(Some((pixmap, placement)))
}
