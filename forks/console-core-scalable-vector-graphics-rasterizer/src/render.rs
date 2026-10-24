// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::OptionLog;

pub struct Context {
    pub max_bbox: console_core_path_rasterizer::IntRect,
}

pub fn render_nodes(
    parent: &console_core_scalable_vector_graphics::Group,
    ctx: &Context,
    transform: console_core_path_rasterizer::Transform,
    pixmap: &mut console_core_path_rasterizer::PixmapMut,
) {
    for node in parent.children() {
        render_node(node, ctx, transform, pixmap);
    }
}

pub fn render_node(
    node: &console_core_scalable_vector_graphics::Node,
    ctx: &Context,
    transform: console_core_path_rasterizer::Transform,
    pixmap: &mut console_core_path_rasterizer::PixmapMut,
) {
    match node {
        console_core_scalable_vector_graphics::Node::Group(group) => {
            render_group(group, ctx, transform, pixmap);
        }
        console_core_scalable_vector_graphics::Node::Path(path) => {
            crate::path::render(
                path,
                console_core_path_rasterizer::BlendMode::SourceOver,
                ctx,
                transform,
                pixmap,
            );
        }
        console_core_scalable_vector_graphics::Node::Image(image) => {
            crate::image::render(image, transform, pixmap);
        }
        console_core_scalable_vector_graphics::Node::Text(text) => {
            render_group(text.flattened(), ctx, transform, pixmap);
        }
    }
}

fn render_group(
    group: &console_core_scalable_vector_graphics::Group,
    ctx: &Context,
    transform: console_core_path_rasterizer::Transform,
    pixmap: &mut console_core_path_rasterizer::PixmapMut,
) -> Option<()> {
    let transform = transform.pre_concat(group.transform());

    if !group.should_isolate() {
        render_nodes(group, ctx, transform, pixmap);
        return Some(());
    }

    let bbox = group.layer_bounding_box().transform(transform)?;

    let mut ibbox = if group.filters().is_empty() {
        // Convert group bbox into an integer one, expanding each side outwards by 2px
        // to make sure that anti-aliased pixels would not be clipped.
        console_core_path_rasterizer::IntRect::from_xywh(
            (bbox.x().floor() as i32).checked_sub(2)?,
            (bbox.y().floor() as i32).checked_sub(2)?,
            (bbox.width().ceil() as u32).checked_add(4)?,
            (bbox.height().ceil() as u32).checked_add(4)?,
        )?
    } else {
        // The bounding box for groups with filters is special and should not be expanded by 2px,
        // because it's already acting as a clipping region.
        let bbox = console_core_path_rasterizer::IntRect::from_xywh(
            bbox.x().floor() as i32,
            bbox.y().floor() as i32,
            bbox.width().ceil().max(1.0) as u32,
            bbox.height().ceil().max(1.0) as u32,
        )?;
        // Make sure our filter region is not bigger than 4x the canvas size.
        // This is required mainly to prevent huge filter regions that would tank the performance.
        // It should not affect the final result in any way.
        crate::geom::fit_to_rect(bbox, ctx.max_bbox)?
    };

    // Make sure our layer is not bigger than 4x the canvas size.
    // This is required to prevent huge layers.
    if group.filters().is_empty() {
        ibbox = crate::geom::fit_to_rect(ibbox, ctx.max_bbox)?;
    }

    let shift_ts = {
        // Original shift.
        let mut dx = bbox.x();
        let mut dy = bbox.y();

        // Account for subpixel positioned layers.
        dx -= bbox.x() - ibbox.x() as f32;
        dy -= bbox.y() - ibbox.y() as f32;

        console_core_path_rasterizer::Transform::from_translate(-dx, -dy)
    };

    let transform = shift_ts.pre_concat(transform);

    let mut sub_pixmap = console_core_path_rasterizer::Pixmap::new(ibbox.width(), ibbox.height())
        .log_none(|| log::warn!("Failed to allocate a group layer for: {:?}.", ibbox))?;

    render_nodes(group, ctx, transform, &mut sub_pixmap.as_mut());

    if !group.filters().is_empty() {
        for filter in group.filters() {
            crate::filter::apply(filter, transform, &mut sub_pixmap);
        }
    }

    if let Some(clip_path) = group.clip_path() {
        crate::clip::apply(clip_path, transform, &mut sub_pixmap);
    }

    if let Some(mask) = group.mask() {
        crate::mask::apply(mask, ctx, transform, &mut sub_pixmap);
    }

    let paint = console_core_path_rasterizer::PixmapPaint {
        opacity: group.opacity().get(),
        blend_mode: convert_blend_mode(group.blend_mode()),
        quality: console_core_path_rasterizer::FilterQuality::Nearest,
    };

    pixmap.draw_pixmap(
        ibbox.x(),
        ibbox.y(),
        sub_pixmap.as_ref(),
        &paint,
        console_core_path_rasterizer::Transform::identity(),
        None,
    );

    Some(())
}

pub fn convert_blend_mode(mode: console_core_scalable_vector_graphics::BlendMode) -> console_core_path_rasterizer::BlendMode {
    match mode {
        console_core_scalable_vector_graphics::BlendMode::Normal => console_core_path_rasterizer::BlendMode::SourceOver,
        console_core_scalable_vector_graphics::BlendMode::Multiply => console_core_path_rasterizer::BlendMode::Multiply,
        console_core_scalable_vector_graphics::BlendMode::Screen => console_core_path_rasterizer::BlendMode::Screen,
        console_core_scalable_vector_graphics::BlendMode::Overlay => console_core_path_rasterizer::BlendMode::Overlay,
        console_core_scalable_vector_graphics::BlendMode::Darken => console_core_path_rasterizer::BlendMode::Darken,
        console_core_scalable_vector_graphics::BlendMode::Lighten => console_core_path_rasterizer::BlendMode::Lighten,
        console_core_scalable_vector_graphics::BlendMode::ColorDodge => console_core_path_rasterizer::BlendMode::ColorDodge,
        console_core_scalable_vector_graphics::BlendMode::ColorBurn => console_core_path_rasterizer::BlendMode::ColorBurn,
        console_core_scalable_vector_graphics::BlendMode::HardLight => console_core_path_rasterizer::BlendMode::HardLight,
        console_core_scalable_vector_graphics::BlendMode::SoftLight => console_core_path_rasterizer::BlendMode::SoftLight,
        console_core_scalable_vector_graphics::BlendMode::Difference => console_core_path_rasterizer::BlendMode::Difference,
        console_core_scalable_vector_graphics::BlendMode::Exclusion => console_core_path_rasterizer::BlendMode::Exclusion,
        console_core_scalable_vector_graphics::BlendMode::Hue => console_core_path_rasterizer::BlendMode::Hue,
        console_core_scalable_vector_graphics::BlendMode::Saturation => console_core_path_rasterizer::BlendMode::Saturation,
        console_core_scalable_vector_graphics::BlendMode::Color => console_core_path_rasterizer::BlendMode::Color,
        console_core_scalable_vector_graphics::BlendMode::Luminosity => console_core_path_rasterizer::BlendMode::Luminosity,
    }
}

#[cfg(test)]
mod tests {

    // Derived from https://github.com/servo/servo/issues/42258.
    #[test]
    fn filter_bbox_outside_int_rect() {
        let svg = r#"<svg filter="url(#f)"><filter id="f" x="2em"><feFlood/></filter><path d="M0 0H1e8V1"/></svg>"#;
        let tree = console_core_scalable_vector_graphics::Tree::from_str(svg, &console_core_scalable_vector_graphics::Options::default()).unwrap();
        let mut pixmap = console_core_path_rasterizer::Pixmap::new(1, 1).unwrap();

        // Just make sure we don't panic.
        crate::render(
            &tree,
            console_core_path_rasterizer::Transform::identity(),
            &mut pixmap.as_mut(),
        );
    }
}
