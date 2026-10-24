// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::render::Context;

pub fn render(
    path: &console_core_scalable_vector_graphics::Path,
    blend_mode: console_core_path_rasterizer::BlendMode,
    ctx: &Context,
    transform: console_core_path_rasterizer::Transform,
    pixmap: &mut console_core_path_rasterizer::PixmapMut,
) {
    if !path.is_visible() {
        return;
    }

    if path.paint_order() == console_core_scalable_vector_graphics::PaintOrder::FillAndStroke {
        fill_path(path, blend_mode, ctx, transform, pixmap);
        stroke_path(path, blend_mode, ctx, transform, pixmap);
    } else {
        stroke_path(path, blend_mode, ctx, transform, pixmap);
        fill_path(path, blend_mode, ctx, transform, pixmap);
    }
}

pub fn fill_path(
    path: &console_core_scalable_vector_graphics::Path,
    blend_mode: console_core_path_rasterizer::BlendMode,
    ctx: &Context,
    transform: console_core_path_rasterizer::Transform,
    pixmap: &mut console_core_path_rasterizer::PixmapMut,
) -> Option<()> {
    let fill = path.fill()?;

    // Horizontal and vertical lines cannot be filled. Skip.
    if path.data().bounds().width() == 0.0 || path.data().bounds().height() == 0.0 {
        return None;
    }

    let rule = match fill.rule() {
        console_core_scalable_vector_graphics::FillRule::NonZero => console_core_path_rasterizer::FillRule::Winding,
        console_core_scalable_vector_graphics::FillRule::EvenOdd => console_core_path_rasterizer::FillRule::EvenOdd,
    };

    let pattern_pixmap;
    let mut paint = console_core_path_rasterizer::Paint::default();
    match fill.paint() {
        console_core_scalable_vector_graphics::Paint::Color(c) => {
            paint.set_color_rgba8(c.red, c.green, c.blue, fill.opacity().to_u8());
        }
        console_core_scalable_vector_graphics::Paint::LinearGradient(lg) => {
            paint.shader = convert_linear_gradient(lg, fill.opacity())?;
        }
        console_core_scalable_vector_graphics::Paint::RadialGradient(rg) => {
            paint.shader = convert_radial_gradient(rg, fill.opacity())?;
        }
        console_core_scalable_vector_graphics::Paint::Pattern(pattern) => {
            let (patt_pix, patt_ts) = render_pattern_pixmap(pattern, ctx, transform)?;

            pattern_pixmap = patt_pix;
            paint.shader = console_core_path_rasterizer::Pattern::new(
                pattern_pixmap.as_ref(),
                console_core_path_rasterizer::SpreadMode::Repeat,
                console_core_path_rasterizer::FilterQuality::Bicubic,
                fill.opacity().get(),
                patt_ts,
            );
        }
    }
    paint.anti_alias = path.rendering_mode().use_shape_antialiasing();
    paint.blend_mode = blend_mode;

    pixmap.fill_path(path.data(), &paint, rule, transform, None);
    Some(())
}

fn stroke_path(
    path: &console_core_scalable_vector_graphics::Path,
    blend_mode: console_core_path_rasterizer::BlendMode,
    ctx: &Context,
    transform: console_core_path_rasterizer::Transform,
    pixmap: &mut console_core_path_rasterizer::PixmapMut,
) -> Option<()> {
    let stroke = path.stroke()?;
    let pattern_pixmap;
    let mut paint = console_core_path_rasterizer::Paint::default();
    match stroke.paint() {
        console_core_scalable_vector_graphics::Paint::Color(c) => {
            paint.set_color_rgba8(c.red, c.green, c.blue, stroke.opacity().to_u8());
        }
        console_core_scalable_vector_graphics::Paint::LinearGradient(lg) => {
            paint.shader = convert_linear_gradient(lg, stroke.opacity())?;
        }
        console_core_scalable_vector_graphics::Paint::RadialGradient(rg) => {
            paint.shader = convert_radial_gradient(rg, stroke.opacity())?;
        }
        console_core_scalable_vector_graphics::Paint::Pattern(pattern) => {
            let (patt_pix, patt_ts) = render_pattern_pixmap(pattern, ctx, transform)?;

            pattern_pixmap = patt_pix;
            paint.shader = console_core_path_rasterizer::Pattern::new(
                pattern_pixmap.as_ref(),
                console_core_path_rasterizer::SpreadMode::Repeat,
                console_core_path_rasterizer::FilterQuality::Bicubic,
                stroke.opacity().get(),
                patt_ts,
            );
        }
    }
    paint.anti_alias = path.rendering_mode().use_shape_antialiasing();
    paint.blend_mode = blend_mode;

    pixmap.stroke_path(path.data(), &paint, &stroke.to_tiny_skia(), transform, None);

    Some(())
}

fn convert_linear_gradient(
    gradient: &console_core_scalable_vector_graphics::LinearGradient,
    opacity: console_core_scalable_vector_graphics::Opacity,
) -> Option<console_core_path_rasterizer::Shader<'_>> {
    let (mode, points) = convert_base_gradient(gradient, opacity)?;

    let shader = console_core_path_rasterizer::LinearGradient::new(
        (gradient.x1(), gradient.y1()).into(),
        (gradient.x2(), gradient.y2()).into(),
        points,
        mode,
        gradient.transform(),
    )?;

    Some(shader)
}

fn convert_radial_gradient(
    gradient: &console_core_scalable_vector_graphics::RadialGradient,
    opacity: console_core_scalable_vector_graphics::Opacity,
) -> Option<console_core_path_rasterizer::Shader<'_>> {
    let (mode, points) = convert_base_gradient(gradient, opacity)?;

    let shader = console_core_path_rasterizer::RadialGradient::new(
        (gradient.fx(), gradient.fy()).into(),
        gradient.fr().get(),
        (gradient.cx(), gradient.cy()).into(),
        gradient.r().get(),
        points,
        mode,
        gradient.transform(),
    )?;

    Some(shader)
}

fn convert_base_gradient(
    gradient: &console_core_scalable_vector_graphics::BaseGradient,
    opacity: console_core_scalable_vector_graphics::Opacity,
) -> Option<(console_core_path_rasterizer::SpreadMode, Vec<console_core_path_rasterizer::GradientStop>)> {
    let mode = match gradient.spread_method() {
        console_core_scalable_vector_graphics::SpreadMethod::Pad => console_core_path_rasterizer::SpreadMode::Pad,
        console_core_scalable_vector_graphics::SpreadMethod::Reflect => console_core_path_rasterizer::SpreadMode::Reflect,
        console_core_scalable_vector_graphics::SpreadMethod::Repeat => console_core_path_rasterizer::SpreadMode::Repeat,
    };

    let mut points = Vec::with_capacity(gradient.stops().len());
    for stop in gradient.stops() {
        let alpha = stop.opacity() * opacity;
        let color = console_core_path_rasterizer::Color::from_rgba8(
            stop.color().red,
            stop.color().green,
            stop.color().blue,
            alpha.to_u8(),
        );
        points.push(console_core_path_rasterizer::GradientStop::new(stop.offset().get(), color));
    }

    Some((mode, points))
}

fn render_pattern_pixmap(
    pattern: &console_core_scalable_vector_graphics::Pattern,
    ctx: &Context,
    transform: console_core_path_rasterizer::Transform,
) -> Option<(console_core_path_rasterizer::Pixmap, console_core_path_rasterizer::Transform)> {
    let (sx, sy) = {
        let ts2 = transform.pre_concat(pattern.transform());
        ts2.get_scale()
    };

    let rect = pattern.rect();
    let img_size = console_core_path_rasterizer::IntSize::from_wh(
        (rect.width() * sx).round() as u32,
        (rect.height() * sy).round() as u32,
    )?;
    let mut pixmap = console_core_path_rasterizer::Pixmap::new(img_size.width(), img_size.height())?;

    let transform = console_core_path_rasterizer::Transform::from_scale(sx, sy);
    crate::render::render_nodes(pattern.root(), ctx, transform, &mut pixmap.as_mut());

    let mut ts = console_core_path_rasterizer::Transform::default();
    ts = ts.pre_concat(pattern.transform());
    ts = ts.pre_translate(rect.x(), rect.y());
    ts = ts.pre_scale(1.0 / sx, 1.0 / sy);

    Some((pixmap, ts))
}
