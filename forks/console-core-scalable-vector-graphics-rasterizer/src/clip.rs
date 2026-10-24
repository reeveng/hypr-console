// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::render::Context;

pub fn apply(
    clip: &console_core_scalable_vector_graphics::ClipPath,
    transform: console_core_path_rasterizer::Transform,
    pixmap: &mut console_core_path_rasterizer::Pixmap,
) {
    let mut clip_pixmap = console_core_path_rasterizer::Pixmap::new(pixmap.width(), pixmap.height()).unwrap();
    clip_pixmap.fill(console_core_path_rasterizer::Color::BLACK);

    draw_children(
        clip.root(),
        console_core_path_rasterizer::BlendMode::Clear,
        transform.pre_concat(clip.transform()),
        &mut clip_pixmap.as_mut(),
    );

    if let Some(clip) = clip.clip_path() {
        apply(clip, transform, pixmap);
    }

    let mut mask = console_core_path_rasterizer::Mask::from_pixmap(clip_pixmap.as_ref(), console_core_path_rasterizer::MaskType::Alpha);
    mask.invert();
    pixmap.apply_mask(&mask);
}

fn draw_children(
    parent: &console_core_scalable_vector_graphics::Group,
    mode: console_core_path_rasterizer::BlendMode,
    transform: console_core_path_rasterizer::Transform,
    pixmap: &mut console_core_path_rasterizer::PixmapMut,
) {
    for child in parent.children() {
        match child {
            console_core_scalable_vector_graphics::Node::Path(path) => {
                if !path.is_visible() {
                    continue;
                }

                // We could use any values here. They will not be used anyway.
                let ctx = Context {
                    max_bbox: console_core_path_rasterizer::IntRect::from_xywh(0, 0, 1, 1).unwrap(),
                };

                crate::path::fill_path(path, mode, &ctx, transform, pixmap);
            }
            console_core_scalable_vector_graphics::Node::Text(text) => {
                draw_children(text.flattened(), mode, transform, pixmap);
            }
            console_core_scalable_vector_graphics::Node::Group(group) => {
                let transform = transform.pre_concat(group.transform());

                if let Some(clip) = group.clip_path() {
                    // If a `clipPath` child also has a `clip-path`
                    // then we should render this child on a new canvas,
                    // clip it, and only then draw it to the `clipPath`.
                    clip_group(group, clip, transform, pixmap);
                } else {
                    draw_children(group, mode, transform, pixmap);
                }
            }
            _ => {}
        }
    }
}

fn clip_group(
    children: &console_core_scalable_vector_graphics::Group,
    clip: &console_core_scalable_vector_graphics::ClipPath,
    transform: console_core_path_rasterizer::Transform,
    pixmap: &mut console_core_path_rasterizer::PixmapMut,
) -> Option<()> {
    let mut clip_pixmap = console_core_path_rasterizer::Pixmap::new(pixmap.width(), pixmap.height()).unwrap();

    draw_children(
        children,
        console_core_path_rasterizer::BlendMode::SourceOver,
        transform,
        &mut clip_pixmap.as_mut(),
    );
    apply(clip, transform, &mut clip_pixmap);

    let mut paint = console_core_path_rasterizer::PixmapPaint::default();
    paint.blend_mode = console_core_path_rasterizer::BlendMode::Xor;
    pixmap.draw_pixmap(
        0,
        0,
        clip_pixmap.as_ref(),
        &paint,
        console_core_path_rasterizer::Transform::identity(),
        None,
    );

    Some(())
}
