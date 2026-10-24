// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub fn render(
    image: &console_core_scalable_vector_graphics::Image,
    transform: console_core_path_rasterizer::Transform,
    pixmap: &mut console_core_path_rasterizer::PixmapMut,
) {
    if !image.is_visible() {
        return;
    }

    render_inner(image.kind(), transform, image.rendering_mode(), pixmap);
}

pub fn render_inner(
    image_kind: &console_core_scalable_vector_graphics::ImageKind,
    transform: console_core_path_rasterizer::Transform,
    #[allow(unused_variables)] rendering_mode: console_core_scalable_vector_graphics::ImageRendering,
    pixmap: &mut console_core_path_rasterizer::PixmapMut,
) {
    match image_kind {
        console_core_scalable_vector_graphics::ImageKind::SVG(tree) => {
            render_vector(tree, transform, pixmap);
        }
        #[cfg(feature = "raster-images")]
        _ => {
            raster_images::render_raster(image_kind, transform, rendering_mode, pixmap);
        }
        #[cfg(not(feature = "raster-images"))]
        _ => {
            log::warn!("Images decoding was disabled by a build feature.");
        }
    }
}

fn render_vector(
    tree: &console_core_scalable_vector_graphics::Tree,
    transform: console_core_path_rasterizer::Transform,
    pixmap: &mut console_core_path_rasterizer::PixmapMut,
) -> Option<()> {
    let mut sub_pixmap = console_core_path_rasterizer::Pixmap::new(pixmap.width(), pixmap.height()).unwrap();
    crate::render(tree, transform, &mut sub_pixmap.as_mut());
    pixmap.draw_pixmap(
        0,
        0,
        sub_pixmap.as_ref(),
        &console_core_path_rasterizer::PixmapPaint::default(),
        console_core_path_rasterizer::Transform::default(),
        None,
    );

    Some(())
}

#[cfg(feature = "raster-images")]
mod raster_images {
    use crate::OptionLog;
    use std::io::Cursor;
    use console_core_scalable_vector_graphics::ImageRendering;

    fn decode_raster(image: &console_core_scalable_vector_graphics::ImageKind) -> Option<console_core_path_rasterizer::Pixmap> {
        match image {
            console_core_scalable_vector_graphics::ImageKind::SVG(_) => None,
            console_core_scalable_vector_graphics::ImageKind::JPEG(data) => {
                decode_jpeg(data).log_none(|| log::warn!("Failed to decode a JPEG image."))
            }
            console_core_scalable_vector_graphics::ImageKind::PNG(data) => {
                decode_png(data).log_none(|| log::warn!("Failed to decode a PNG image."))
            }
            console_core_scalable_vector_graphics::ImageKind::GIF(data) => {
                decode_gif(data).log_none(|| log::warn!("Failed to decode a GIF image."))
            }
            console_core_scalable_vector_graphics::ImageKind::WEBP(data) => {
                decode_webp(data).log_none(|| log::warn!("Failed to decode a WebP image."))
            }
        }
    }

    fn decode_png(data: &[u8]) -> Option<console_core_path_rasterizer::Pixmap> {
        console_core_path_rasterizer::Pixmap::decode_png(data).ok()
    }

    fn decode_jpeg(data: &[u8]) -> Option<console_core_path_rasterizer::Pixmap> {
        use zune_jpeg::zune_core::colorspace::ColorSpace;
        use zune_jpeg::zune_core::options::DecoderOptions;

        let cursor = Cursor::new(data);
        let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
        let mut decoder = zune_jpeg::JpegDecoder::new_with_options(cursor, options);
        decoder.decode_headers().ok()?;
        let output_cs = decoder.output_colorspace()?;

        let img_data = {
            let data = decoder.decode().ok()?;
            match output_cs {
                ColorSpace::RGBA => data,
                _ => return None,
            }
        };

        let info = decoder.info()?;

        let size = console_core_path_rasterizer::IntSize::from_wh(info.width as u32, info.height as u32)?;
        console_core_path_rasterizer::Pixmap::from_vec(img_data, size)
    }

    fn decode_gif(data: &[u8]) -> Option<console_core_path_rasterizer::Pixmap> {
        let mut decoder = gif::DecodeOptions::new();
        decoder.set_color_output(gif::ColorOutput::RGBA);
        let mut decoder = decoder.read_info(data).ok()?;
        let first_frame = decoder.read_next_frame().ok()??;

        let size = console_core_path_rasterizer::IntSize::from_wh(
            u32::from(first_frame.width),
            u32::from(first_frame.height),
        )?;

        let (w, h) = size.dimensions();
        let mut pixmap = console_core_path_rasterizer::Pixmap::new(w, h)?;
        rgba_to_pixmap(&first_frame.buffer, &mut pixmap);
        Some(pixmap)
    }

    fn decode_webp(data: &[u8]) -> Option<console_core_path_rasterizer::Pixmap> {
        let mut decoder = image_webp::WebPDecoder::new(std::io::Cursor::new(data)).ok()?;
        let mut first_frame = vec![0; decoder.output_buffer_size()?];
        decoder.read_image(&mut first_frame).ok()?;

        let (w, h) = decoder.dimensions();
        let mut pixmap = console_core_path_rasterizer::Pixmap::new(w, h)?;

        if decoder.has_alpha() {
            rgba_to_pixmap(&first_frame, &mut pixmap);
        } else {
            rgb_to_pixmap(&first_frame, &mut pixmap);
        }

        Some(pixmap)
    }

    fn rgb_to_pixmap(data: &[u8], pixmap: &mut console_core_path_rasterizer::Pixmap) {
        use rgb::FromSlice;

        let mut i = 0;
        let dst = pixmap.data_mut();
        for p in data.as_rgb() {
            dst[i + 0] = p.r;
            dst[i + 1] = p.g;
            dst[i + 2] = p.b;
            dst[i + 3] = 255;

            i += console_core_path_rasterizer::BYTES_PER_PIXEL;
        }
    }

    fn rgba_to_pixmap(data: &[u8], pixmap: &mut console_core_path_rasterizer::Pixmap) {
        use rgb::FromSlice;

        let mut i = 0;
        let dst = pixmap.data_mut();
        for p in data.as_rgba() {
            let a = p.a as f64 / 255.0;
            dst[i + 0] = (p.r as f64 * a + 0.5) as u8;
            dst[i + 1] = (p.g as f64 * a + 0.5) as u8;
            dst[i + 2] = (p.b as f64 * a + 0.5) as u8;
            dst[i + 3] = p.a;

            i += console_core_path_rasterizer::BYTES_PER_PIXEL;
        }
    }

    pub(crate) fn render_raster(
        image: &console_core_scalable_vector_graphics::ImageKind,
        transform: console_core_path_rasterizer::Transform,
        rendering_mode: console_core_scalable_vector_graphics::ImageRendering,
        pixmap: &mut console_core_path_rasterizer::PixmapMut,
    ) -> Option<()> {
        let raster = decode_raster(image)?;

        let rect = console_core_path_rasterizer::Size::from_wh(raster.width() as f32, raster.height() as f32)?
            .to_rect(0.0, 0.0)?;

        let quality = match rendering_mode {
            ImageRendering::OptimizeQuality => console_core_path_rasterizer::FilterQuality::Bicubic,
            ImageRendering::OptimizeSpeed => console_core_path_rasterizer::FilterQuality::Nearest,
            ImageRendering::Smooth => console_core_path_rasterizer::FilterQuality::Bilinear,
            ImageRendering::HighQuality => console_core_path_rasterizer::FilterQuality::Bicubic,
            ImageRendering::CrispEdges => console_core_path_rasterizer::FilterQuality::Nearest,
            ImageRendering::Pixelated => console_core_path_rasterizer::FilterQuality::Nearest,
        };

        let pattern = console_core_path_rasterizer::Pattern::new(
            raster.as_ref(),
            console_core_path_rasterizer::SpreadMode::Pad,
            quality,
            1.0,
            console_core_path_rasterizer::Transform::default(),
        );
        let mut paint = console_core_path_rasterizer::Paint::default();
        paint.shader = pattern;

        pixmap.fill_rect(rect, &paint, transform, None);

        Some(())
    }
}
