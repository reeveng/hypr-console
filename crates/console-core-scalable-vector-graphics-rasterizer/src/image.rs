//! Copyright 2018 the Resvg Authors
//! SPDX-License-Identifier: Apache-2.0 OR MIT

use console_core_never::Never;
use console_core_path_rasterizer::{Pixmap, PixmapMut, PixmapPaint, Transform};
use console_core_scalable_vector_graphics::{Image, ImageKind, IsVisibleAnswer, Tree};

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "An embedded tree inherits the bounded context already decremented by the image node"
  )
)]
pub(super) fn render(
  image: &Image,
  context: &crate::render::Context<'_>,
  transform: Transform,
  target: &mut PixmapMut<'_>,
) -> Result<(), Never> {
  let Ok(visibility) = image.is_visible();

  match visibility {
    IsVisibleAnswer::Yes => {}
    IsVisibleAnswer::No => return Ok(()),
  }

  let Ok(kind) = image.kind();

  match kind {
    ImageKind::SVG(tree) => {
      let Ok(()) = render_vector(tree, context, transform, target);
    }
    ImageKind::JPEG(_) | ImageKind::PNG(_) | ImageKind::GIF(_) | ImageKind::WEBP(_) => {
      #[cfg(feature = "raster-images")]
      {
        let Ok(mode) = image.rendering_mode();
        let Ok(()) = raster_images::render_raster(kind, transform, mode, target);
      }

      #[cfg(not(feature = "raster-images"))]
      console_core_logging::warn!("Images decoding was disabled by a build feature.");
    }
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Embedded SVG children inherit the current depth budget and each child consumes one unit"
  )
)]
fn render_vector(
  tree: &Tree,
  context: &crate::render::Context<'_>,
  transform: Transform,
  target: &mut PixmapMut<'_>,
) -> Result<(), Never> {
  let Ok(size) = crate::target_size(target);
  let size_2 = match size {
    Some(available) => available,
    None => return Ok(()),
  };
  let Ok(bounds) = crate::max_filter_bbox(size_2);
  let bounds_2 = match bounds {
    Some(available) => available,
    None => return Ok(()),
  };
  let Ok(context_2) = context.with_bounds(bounds_2);
  let Ok(layer) = Pixmap::with_size(size_2);
  let mut layer_2 = match layer {
    Some(available) => available,
    None => return Ok(()),
  };
  let Ok(mut view) = layer_2.as_mut();
  let Ok(children) = tree.root();
  let Ok(()) = crate::render::render_nodes(children, &context_2, transform, &mut view);
  let Ok(image) = layer_2.as_ref();
  let Ok(()) = target.draw_pixmap(
    0,
    0,
    image,
    &PixmapPaint::default(),
    Transform::default(),
    None,
  );

  Ok(())
}

#[cfg(feature = "raster-images")]
mod raster_images {
  use super::{ImageKind, Never, Pixmap, PixmapMut, Transform};
  use crate::OptionLog;
  use console_core_path_rasterizer::{FilterQuality, IntSize, Paint, Pattern, SpreadMode};
  use console_core_raster_image_decoding::{self as raster, DecodeMode, Format};
  use console_core_scalable_vector_graphics::ImageRendering;

  fn decode_raster(image: &ImageKind) -> Result<Option<Pixmap>, Never> {
    let (format, data) = match image {
      ImageKind::SVG(_) => return Ok(None),
      ImageKind::JPEG(data) => (Format::Jpeg, data),
      ImageKind::PNG(data) => (Format::Png, data),
      ImageKind::GIF(data) => (Format::Gif, data),
      ImageKind::WEBP(data) => (Format::Webp, data),
    };
    let decoded = raster::decoded(format, data, DecodeMode::Embedded).ok();
    let Ok(name) = format.says();
    let picture = match decoded
      .log_none(|| console_core_logging::warn!("Failed to decode a {} image.", name))
    {
      Some(picture) => picture,
      None => return Ok(None),
    };
    let Ok(dimensions) = IntSize::from_wh(picture.size.width, picture.size.height);
    let size = match dimensions {
      Some(size) => size,
      None => return Ok(None),
    };

    Pixmap::from_rgba8(picture.bytes, size)
  }

  pub(super) fn render_raster(
    image: &ImageKind,
    transform: Transform,
    mode: ImageRendering,
    target: &mut PixmapMut<'_>,
  ) -> Result<(), Never> {
    let Ok(decoded) = decode_raster(image);
    let raster = match decoded {
      Some(raster) => raster,
      None => return Ok(()),
    };
    let Ok(size) = raster.size();
    let Ok(size_in_path_units) = size.to_size();
    let Ok(rectangle) = size_in_path_units.to_rect(0.0, 0.0);
    let rect = match rectangle {
      Some(rect) => rect,
      None => return Ok(()),
    };

    let quality = match mode {
      ImageRendering::OptimizeQuality | ImageRendering::HighQuality => FilterQuality::Bicubic,
      ImageRendering::Smooth => FilterQuality::Bilinear,
      ImageRendering::OptimizeSpeed | ImageRendering::CrispEdges | ImageRendering::Pixelated => {
        FilterQuality::Nearest
      }
    };

    let Ok(pixels) = raster.as_ref();
    let Ok(shader) = Pattern::new(pixels, SpreadMode::Pad, quality, 1.0, Transform::default());
    let paint = Paint {
      shader,
      ..Paint::default()
    };

    target.fill_rect(rect, &paint, transform, None)
  }

  #[cfg(test)]
  mod tests {
    use super::{ImageKind, decode_raster};
    use std::sync::Arc;

    #[test]
    fn embedded_images_use_stored_dimensions_and_premultiplied_alpha() -> Result<(), &'static str>
    {
      let image = ImageKind::GIF(Arc::new(
        include_bytes!("../../console-core-gif-files/tests/pictures/placed.gif").to_vec(),
      ));
      let Ok(decoded_gif) = decode_raster(&image);
      let gif = decoded_gif.ok_or("GIF")?;
      assert_eq!(gif.width(), Ok(10));
      assert_eq!(gif.height(), Ok(6));

      let image = ImageKind::JPEG(Arc::new(
        include_bytes!("../../console-core-jpeg-files/tests/pictures/turned.jpg").to_vec(),
      ));
      let Ok(decoded_jpeg) = decode_raster(&image);
      let jpeg = decoded_jpeg.ok_or("JPEG")?;
      assert_eq!(jpeg.width(), Ok(75));
      assert_eq!(jpeg.height(), Ok(53));

      let image = ImageKind::WEBP(Arc::new(
        include_bytes!("../../console-core-webp-files/tests/pictures/alpha.webp").to_vec(),
      ));
      let Ok(decoded_webp) = decode_raster(&image);
      let webp = decoded_webp.ok_or("WEBP")?;
      let straight = include_bytes!("../../console-core-webp-files/tests/pictures/alpha.rgba");
      let Ok(pixels) = webp.data();
      assert_eq!(pixels.len(), straight.len());

      for (pixel, original) in pixels.chunks_exact(4).zip(straight.chunks_exact(4)) {
        let [red, green, blue, alpha] =
          <[u8; 4]>::try_from(original).map_err(|_| "source pixel")?;
        let pixel_2 = <[u8; 4]>::try_from(pixel).map_err(|_| "rendered pixel")?;
        let premultiplied = [red, green, blue].map(|color| {
          ::core::primitive::u16::wrapping_div(
            ::core::primitive::u16::wrapping_add(
              ::core::primitive::u16::wrapping_mul(u16::from(color), u16::from(alpha)),
              127,
            ),
            255,
          )
        });
        let [red_2, green_2, blue_2] = premultiplied;
        assert_eq!(
          pixel_2.map(u16::from),
          [red_2, green_2, blue_2, u16::from(alpha)]
        );
      }

      Ok(())
    }
  }
}
