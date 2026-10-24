// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::render::Context;

pub fn apply(
    mask: &console_core_scalable_vector_graphics::Mask,
    ctx: &Context,
    transform: console_core_path_rasterizer::Transform,
    pixmap: &mut console_core_path_rasterizer::Pixmap,
) {
    if mask.root().children().is_empty() {
        pixmap.fill(console_core_path_rasterizer::Color::TRANSPARENT);
        return;
    }

    let mut mask_pixmap = console_core_path_rasterizer::Pixmap::new(pixmap.width(), pixmap.height()).unwrap();

    {
        // TODO: only when needed
        // Mask has to be clipped by mask.region
        let mut alpha_mask = console_core_path_rasterizer::Mask::new(pixmap.width(), pixmap.height()).unwrap();
        alpha_mask.fill_path(
            &console_core_path_rasterizer::PathBuilder::from_rect(mask.rect().to_rect()),
            console_core_path_rasterizer::FillRule::Winding,
            true,
            transform,
        );

        crate::render::render_nodes(mask.root(), ctx, transform, &mut mask_pixmap.as_mut());

        mask_pixmap.apply_mask(&alpha_mask);
    }

    if let Some(mask) = mask.mask() {
        self::apply(mask, ctx, transform, pixmap);
    }

    let mask_type = match mask.kind() {
        console_core_scalable_vector_graphics::MaskType::Luminance => console_core_path_rasterizer::MaskType::Luminance,
        console_core_scalable_vector_graphics::MaskType::Alpha => console_core_path_rasterizer::MaskType::Alpha,
    };

    let mask = console_core_path_rasterizer::Mask::from_pixmap(mask_pixmap.as_ref(), mask_type);
    pixmap.apply_mask(&mask);
}
