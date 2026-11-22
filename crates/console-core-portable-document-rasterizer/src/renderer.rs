use crate::{ScaleFactor, derive_settings};
use console_core_never::Never;
use console_core_number_conversion::{
  fitted, index, nearest_f32_from_f64, nearest_f32_from_u64, toward_zero_u8, toward_zero_u16,
  toward_zero_u32, whole_u16,
};
use console_core_picture_scaling::{Channels, catmull_rom};
use console_core_portable_document_interpreter::encode::EncodedShadingPattern;
use console_core_portable_document_interpreter::font::Glyph;
use console_core_portable_document_interpreter::pattern::{
  Pattern, ShadingPattern, TilingPattern,
};
use console_core_portable_document_interpreter::{
  BlendMode, CacheKey, ClipPath, Device, FillRule, GlyphDrawMode, ImageData, Interpolation,
  LumaData, MaskType, Paint, PaintOperation, PathDrawMode, RgbData, SoftMask, StrokeProps,
};
use console_core_size::Size;
use kurbo::{Affine, BezPath, Point, Rect, Shape, Vec2};
use std::collections::HashMap;
use std::sync::Arc;
use vello_cpu::color::palette::css::{BLACK, TRANSPARENT};
use vello_cpu::color::{AlphaColor, Srgb};
use vello_cpu::peniko::{Compose, Fill, ImageQuality, ImageSampler, Mix};
use vello_cpu::{
  Image, ImageSource, Mask, PaintType, Pixmap, RenderContext, RenderSettings, peniko,
};

const MAX_IMAGE_PIXELS: u64 = 1 << 28;

enum StrokeTarget {
  Text,
  Path,
}

enum Surface {
  Canvas,
  PatternCell,
}

enum GlyphContext {
  Ordinary,
  Type3,
}

#[derive(Clone, Copy)]
enum Transparency {
  Opaque,
  Possible,
}

impl From<bool> for Transparency {
  fn from(possible: bool) -> Self {
    match bool::then_some(possible, ()) {
      Some(()) => Self::Possible,
      None => Self::Opaque,
    }
  }
}

impl From<Transparency> for bool {
  fn from(transparency: Transparency) -> Self {
    match transparency {
      Transparency::Possible => true,
      Transparency::Opaque => false,
    }
  }
}

enum AlphaLayout {
  Inline,
  SeparateMask,
}

impl From<bool> for AlphaLayout {
  fn from(separate: bool) -> Self {
    match bool::then_some(separate, ()) {
      Some(()) => Self::SeparateMask,
      None => Self::Inline,
    }
  }
}

#[derive(Clone, Copy)]
enum AlphaChannel {
  Present,
  Absent,
}

#[derive(Clone, Copy)]
enum Resampling {
  Downsample,
  Original,
}

impl From<bool> for Resampling {
  fn from(downsample: bool) -> Self {
    match bool::then_some(downsample, ()) {
      Some(()) => Self::Downsample,
      None => Self::Original,
    }
  }
}

enum Compositing {
  Layered,
  Direct,
}

impl From<bool> for Compositing {
  fn from(layered: bool) -> Self {
    match bool::then_some(layered, ()) {
      Some(()) => Self::Layered,
      None => Self::Direct,
    }
  }
}

pub(crate) struct Renderer<'cache> {
  pub(crate) ctx: RenderContext,
  surface: Surface,
  pub(crate) soft_mask_cache: HashMap<u128, Mask>,
  outlines: &'cache mut HashMap<u128, BezPath>,
  pub(crate) cur_mask: Option<Mask>,
  glyph: GlyphContext,
  image_transparency_stack: Vec<Transparency>,
}

impl<'cache> Renderer<'cache> {
  pub(crate) fn new(
    size: Size<u16>,
    settings: RenderSettings,
    outlines: &'cache mut HashMap<u128, BezPath>,
  ) -> Result<Self, Never> {
    Ok(Self {
      ctx: RenderContext::new_with(size.width, size.height, settings),
      surface: Surface::Canvas,
      soft_mask_cache: HashMap::default(),
      outlines,
      cur_mask: None,
      glyph: GlyphContext::Ordinary,

      image_transparency_stack: Vec::new(),
    })
  }

  fn set_stroke_properties(
    &mut self,
    stroke_props: &StrokeProps,
    target: StrokeTarget,
  ) -> Result<(), Never> {
    let threshold = match target {
      StrokeTarget::Text => 0.25,
      StrokeTarget::Path => 1.0,
    };

    let Ok(factor) = max_factor(self.ctx.transform());
    let min_factor = factor.0;
    let mut line_width = stroke_props.line_width.max(0.01);
    let transformed_width = line_width * min_factor;

    match ::core::primitive::bool::then_some(
      {
        transformed_width < threshold
          && matches!(self.surface, Surface::Canvas)
          && matches!(self.glyph, GlyphContext::Ordinary)
      },
      (),
    ) {
      ::core::option::Option::Some(()) => {
        line_width /= transformed_width;
        line_width *= threshold;
      }
      ::core::option::Option::None => {}
    }

    let widest = 4.0 * (f32::from(self.ctx.width()) + f32::from(self.ctx.height()));

    match ::core::primitive::bool::then_some(
      min_factor.is_finite() && min_factor > 0.0 && line_width * min_factor > widest,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        line_width = widest / min_factor;
      }
      ::core::option::Option::None => {}
    }

    let stroke = kurbo::Stroke {
      width: <f64 as ::core::convert::From<::core::primitive::f32>>::from(line_width),
      join: stroke_props.line_join,
      miter_limit: <f64 as ::core::convert::From<::core::primitive::f32>>::from(
        stroke_props.miter_limit,
      ),
      start_cap: stroke_props.line_cap,
      end_cap: stroke_props.line_cap,
      dash_pattern: stroke_props
        .dash_array
        .iter()
        .map(|n| <f64 as ::core::convert::From<::core::primitive::f32>>::from(*n))
        .collect(),
      dash_offset: <f64 as ::core::convert::From<::core::primitive::f32>>::from(
        stroke_props.dash_offset,
      ),
    };

    self.ctx.set_stroke(stroke);

    Ok(())
  }

  fn draw_image_with_alpha_mask(
    &mut self,
    image: ImageData,
    alpha: LumaData,
  ) -> Result<(), Never> {
    let Ok(rgb) = mask_color(&alpha, 0);
    let rgb_3 = match rgb {
      Some(rgb_2) => rgb_2,
      None => return Ok(()),
    };
    let Ok(width) = image.width();
    let Ok(height) = image.height();
    let transform = *self.ctx.transform()
      * Affine::scale_non_uniform(
        f64::from(width) / f64::from(alpha.width),
        f64::from(height) / f64::from(alpha.height),
      );
    let size = Size {
      width: self.ctx.width(),
      height: self.ctx.height(),
    };
    let Ok(settings) = derive_settings(self.ctx.render_settings());
    let Ok(mut renderer) = Renderer::new(size, settings, self.outlines);
    let mut mask_pix = Pixmap::new(size.width, size.height);
    renderer.ctx.set_transform(transform);
    let Ok(()) = renderer.draw_pixels(rgb_3, Some(alpha));
    renderer.ctx.flush();
    let mut resources = vello_cpu::Resources::default();
    renderer.ctx.render_to_pixmap(&mut resources, &mut mask_pix);

    self.ctx.push_mask_layer(Mask::new_alpha(&mask_pix));
    self.image_transparency_stack.push(Transparency::Possible);
    let Ok(()) = self.draw_pixels(image, None);
    self.image_transparency_stack.pop();
    self.ctx.pop_layer();

    Ok(())
  }

  fn draw_image(&mut self, image: ImageData, alpha: Option<LumaData>) -> Result<(), Never> {
    let Ok(width) = image.width();
    let Ok(height) = image.height();
    let Ok(interpolation) = image.interpolate();

    match alpha {
      Some(alpha_2) => match AlphaLayout::from(
        alpha_2.width != width
          || alpha_2.height != height
          || alpha_2.interpolation != interpolation,
      ) {
        AlphaLayout::Inline => self.draw_pixels(image, Some(alpha_2)),
        AlphaLayout::SeparateMask => self.draw_image_with_alpha_mask(image, alpha_2),
      },
      None => self.draw_pixels(image, None),
    }
  }

  fn draw_pixels(
    &mut self,
    image_data: ImageData,
    alpha_data: Option<LumaData>,
  ) -> Result<(), Never> {
    let cur_transform = *self.ctx.transform();
    let mut additional_transform = Affine::IDENTITY;

    let Ok((x, y)) = x_y_advances(&cur_transform);
    let Ok(x_scale) = nearest_f32_from_f64(x.length());
    let Ok(y_scale) = nearest_f32_from_f64(y.length());
    let Ok(mut img_width) = image_data.width();
    let Ok(mut img_height) = image_data.height();
    let Ok(count) = image_area(Size {
      width: img_width,
      height: img_height,
    });
    let count_2 = match count {
      Some(pixels) => pixels,
      None => return Ok(()),
    };
    let count_3 = u64::from(count_2);
    let Ok(rgb_samples) = fitted::<_, u64>(match &image_data {
      ImageData::Rgb(rgb) => rgb.data.len(),
      ImageData::Luma(luma) => luma.data.len(),
    });
    let expected = match image_data {
      ImageData::Rgb(_) => count_3.saturating_mul(3),
      ImageData::Luma(_) => count_3,
    };

    match (rgb_samples == expected).then_some(()) {
      Some(()) => {}
      None => return Ok(()),
    }

    match alpha_data.as_ref() {
      Some(alpha) => {
        let Ok(samples) = fitted::<_, u64>(alpha.data.len());

        match (samples == count_3).then_some(()) {
          Some(()) => {}
          None => return Ok(()),
        }
      }
      None => {}
    }

    let Ok(interpolation) = image_data.interpolate();
    let mut quality = match interpolation {
      Interpolation::Smooth => ImageQuality::Medium,
      Interpolation::Nearest => ImageQuality::Low,
    };

    let (alpha, mut transparency) = match &alpha_data {
      Some(_) => (AlphaChannel::Present, Transparency::Possible),
      None => {
        let Ok(transparency) = self.image_transparency();

        (AlphaChannel::Absent, transparency)
      }
    };
    let resampling = Resampling::from(x_scale < 1.0 || y_scale < 1.0);
    let (new_width, new_height) = match resampling {
      Resampling::Downsample => {
        let Ok(width) = nearest_f32_from_u64(u64::from(img_width));
        let Ok(height) = nearest_f32_from_u64(u64::from(img_height));
        const MAX_DOWNSAMPLED_DIMENSION: u16 = u16::MAX / 2;
        let maximum = f32::from(MAX_DOWNSAMPLED_DIMENSION);
        let Ok(w) = toward_zero_u32(f64::from((width * x_scale).ceil().max(1.0).min(maximum)));
        let Ok(h) = toward_zero_u32(f64::from((height * y_scale).ceil().max(1.0).min(maximum)));

        match self.glyph {
          GlyphContext::Type3 => quality = ImageQuality::High,
          GlyphContext::Ordinary => {}
        }

        (w, h)
      }
      Resampling::Original => (img_width, img_height),
    };

    let Ok(destination) = image_area(Size {
      width: new_width,
      height: new_height,
    });

    match destination {
      Some(_pixels) => {}
      None => return Ok(()),
    }

    let mut rgba_data = match (image_data, alpha, resampling) {
      (ImageData::Luma(luma), AlphaChannel::Absent, _) => {
        let samples = match resampling {
          Resampling::Original => luma.data,
          Resampling::Downsample => {
            let resized = match catmull_rom(
              luma.data,
              Size {
                width: img_width,
                height: img_height,
              },
              Size {
                width: new_width,
                height: new_height,
              },
              Channels::Grey,
            ) {
              Ok(pixels) => pixels,
              Err(_invalid_image) => return Ok(()),
            };

            additional_transform = Affine::scale_non_uniform(
              f64::from(img_width) / f64::from(new_width),
              f64::from(img_height) / f64::from(new_height),
            );
            img_width = new_width;
            img_height = new_height;
            resized
          }
        };

        samples
          .iter()
          .flat_map(|grey| [*grey, *grey, *grey, 255])
          .collect()
      }
      (ImageData::Rgb(rgb), AlphaChannel::Absent, Resampling::Downsample) => {
        let resized = match catmull_rom(
          rgb.data,
          Size {
            width: img_width,
            height: img_height,
          },
          Size {
            width: new_width,
            height: new_height,
          },
          Channels::Rgb,
        ) {
          Ok(pixels) => pixels,
          Err(_invalid_image) => return Ok(()),
        };
        additional_transform = Affine::scale_non_uniform(
          f64::from(img_width) / f64::from(new_width),
          f64::from(img_height) / f64::from(new_height),
        );
        img_width = new_width;
        img_height = new_height;

        resized
          .as_chunks::<3>()
          .0
          .iter()
          .flat_map(|&[red, green, blue]| [red, green, blue, 255])
          .collect()
      }
      (data, _, _) => {
        let rgb = match data {
          ImageData::Rgb(rgb) => rgb.data,
          ImageData::Luma(luma) => luma.data.iter().flat_map(|grey| [*grey; 3]).collect(),
        };
        let rgba = match alpha_data {
          None => rgb
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[red, green, blue]| [red, green, blue, 255])
            .collect(),
          Some(alpha_data_3) => rgb
            .as_chunks::<3>()
            .0
            .iter()
            .zip(alpha_data_3.data)
            .flat_map(|(&[red, green, blue], opacity)| [red, green, blue, opacity])
            .collect(),
        };

        match resampling {
          Resampling::Original => rgba,
          Resampling::Downsample => {
            let resized = match catmull_rom(
              rgba,
              Size {
                width: img_width,
                height: img_height,
              },
              Size {
                width: new_width,
                height: new_height,
              },
              Channels::Rgba,
            ) {
              Ok(pixels) => pixels,
              Err(_invalid_image) => return Ok(()),
            };

            additional_transform = Affine::scale_non_uniform(
              f64::from(img_width) / f64::from(new_width),
              f64::from(img_height) / f64::from(new_height),
            );
            img_width = new_width;
            img_height = new_height;
            resized
          }
        }
      }
    };

    match alpha {
      AlphaChannel::Present => {
        let (chunks, _) = rgba_data.as_chunks_mut::<4>();

        for chunk in chunks {
          let [red, green, blue, opacity] = *chunk;
          *chunk = AlphaColor::from_rgba8(red, green, blue, opacity)
            .premultiply()
            .to_rgba8()
            .to_u8_array();
        }
      }
      AlphaChannel::Absent => {}
    }

    match self.glyph {
      GlyphContext::Type3 => {
        let Ok(prepared) = padded_glyph(
          &rgba_data,
          Size {
            width: img_width,
            height: img_height,
          },
        );
        let image = match prepared {
          Some(image) => image,
          None => return Ok(()),
        };
        img_width = image.size.width;
        img_height = image.size.height;
        additional_transform *= Affine::translate((-2.0, -2.0));
        transparency = Transparency::Possible;
        rgba_data = image.pixels;
      }
      GlyphContext::Ordinary => {}
    }

    let (width, height) = match (u16::try_from(img_width), u16::try_from(img_height)) {
      (Ok(width), Ok(height)) => (width, height),
      _too_wide_or_tall => return Ok(()),
    };
    let rgba = match bytemuck::try_cast_vec(rgba_data) {
      Ok(rgba) => rgba,
      Err((_alignment_or_length, _bytes)) => return Ok(()),
    };
    let Ok(length) = fitted::<_, u64>(rgba.len());

    match (length == u64::from(width).saturating_mul(u64::from(height))).then_some(()) {
      Some(()) => {}
      None => return Ok(()),
    }

    let pixmap = Pixmap::from_parts_with_opacity(rgba, width, height, bool::from(transparency));

    self.draw_pixmap(pixmap, quality, cur_transform * additional_transform)
  }

  fn push_clip_path_inner(&mut self, clip_path: &BezPath, fill: FillRule) -> Result<(), Never> {
    let old_transform = *self.ctx.transform();

    let Ok(rule) = convert_fill_rule(fill);

    self.ctx.set_fill_rule(rule);
    self.ctx.set_transform(Affine::IDENTITY);
    self.ctx.push_clip_path(clip_path);

    self.ctx.set_transform(old_transform);

    Ok(())
  }

  fn draw_pixmap(
    &mut self,
    pixmap: Pixmap,
    quality: ImageQuality,
    transform: Affine,
  ) -> Result<(), Never> {
    let (width, height) = (pixmap.width(), pixmap.height());
    let Ok(image) = sampled_pixmap(
      pixmap,
      ImageSampler {
        x_extend: peniko::Extend::Pad,
        y_extend: peniko::Extend::Pad,
        quality,
        alpha: 1.0,
      },
    );

    self.ctx.set_transform(transform);
    self.ctx.set_paint(image);
    self
      .ctx
      .fill_rect(&Rect::new(0.0, 0.0, f64::from(width), f64::from(height)));

    Ok(())
  }

  fn draw_stencil(
    &mut self,
    stencil: LumaData,
    paint: &Paint<'_>,
    transform: Affine,
  ) -> Result<(), Never> {
    let displayed = transform
      * Affine::scale_non_uniform(
        <f64 as ::core::convert::From<::core::primitive::f32>>::from(stencil.scale_factors.0),
        <f64 as ::core::convert::From<::core::primitive::f32>>::from(stencil.scale_factors.1),
      );

    match paint {
      Paint::Color(c) => {
        let Ok(rgba) = c.to_rgba();
        let Ok(color) = rgba.to_rgba8();
        let (rgb_bytes, alpha) = (
          stencil
            .data
            .iter()
            .flat_map(|_| {
              [
                match color {
                  [_explicit_element, ..] => _explicit_element,
                },
                match color {
                  [_, _explicit_element, ..] => _explicit_element,
                },
                match color {
                  [_, _, _explicit_element, ..] => _explicit_element,
                },
              ]
            })
            .collect::<Vec<u8>>(),
          match color {
            [_, _, _, _explicit_element, ..] => _explicit_element,
          },
        );

        let blend_mode = self.ctx.blend_mode();
        let compositing =
          Compositing::from(alpha != 255 || blend_mode != peniko::BlendMode::default());
        self.ctx.set_transform(displayed);

        match compositing {
          Compositing::Layered => {
            self.ctx.push_layer(
              None,
              Some(blend_mode),
              Some(<f32 as ::core::convert::From<::core::primitive::u8>>::from(alpha) / 255.0),
              None,
              None,
            );
          }
          Compositing::Direct => {}
        }

        let old_rule = *self.ctx.fill_rule();
        self.ctx.set_fill_rule(Fill::NonZero);

        let rgb_data = ImageData::Rgb(RgbData {
          data: rgb_bytes,
          width: stencil.width,
          height: stencil.height,
          interpolation: stencil.interpolation,
          scale_factors: stencil.scale_factors,
        });
        let Ok(()) = self.draw_image(rgb_data, Some(stencil));

        match compositing {
          Compositing::Layered => {
            self.ctx.pop_layer();
          }
          Compositing::Direct => {}
        }

        self.ctx.set_fill_rule(old_rule);
      }
      Paint::Pattern(_) => {
        let (width, height) = (self.ctx.width(), self.ctx.height());
        let stencil_rect = Rect::new(
          0.0,
          0.0,
          <f64 as ::core::convert::From<::core::primitive::u32>>::from(stencil.width),
          <f64 as ::core::convert::From<::core::primitive::u32>>::from(stencil.height),
        );
        let mask_pix = {
          let Ok(colored) = mask_color(&stencil, 255);
          let rgb_bytes = match colored {
            Some(rgb) => rgb,
            None => return Ok(()),
          };
          let Ok(settings) = derive_settings(self.ctx.render_settings());
          let mut sub_renderer = Renderer {
            ctx: RenderContext::new_with(width, height, settings),
            surface: Surface::Canvas,
            soft_mask_cache: HashMap::default(),
            outlines: self.outlines,
            cur_mask: None,
            glyph: GlyphContext::Ordinary,

            image_transparency_stack: Vec::new(),
          };
          let mut sub_pix = Pixmap::new(width, height);
          sub_renderer.ctx.set_transform(displayed);
          let Ok(()) = sub_renderer.draw_pixels(rgb_bytes, Some(stencil));
          sub_renderer.ctx.flush();
          let mut resources = vello_cpu::Resources::default();
          sub_renderer
            .ctx
            .render_to_pixmap(&mut resources, &mut sub_pix);
          sub_pix
        };

        self.ctx.push_layer(
          None,
          Some(self.ctx.blend_mode()),
          None,
          Some(Mask::new_luminance(&mask_pix)),
          None,
        );
        self.ctx.set_transform(displayed);

        let Ok(clip_path) =
          self.set_paint(paint, &stencil_rect.to_path(0.1), PaintOperation::Stroke);

        match clip_path.as_ref() {
          Some(clip_path_2) => {
            let Ok(()) = self.push_clip_path_inner(clip_path_2, FillRule::NonZero);
          }
          ::core::option::Option::None => {}
        }

        self.ctx.fill_rect(&stencil_rect);

        match clip_path {
          Some(_) => {
            self.ctx.pop_clip_path();
          }
          None => {}
        }

        self.ctx.pop_layer();
      }
    };

    Ok(())
  }

  fn set_paint(
    &mut self,
    paint: &Paint<'_>,
    path: &BezPath,
    operation: PaintOperation,
  ) -> Result<Option<BezPath>, Never> {
    let Ok(prepared) = match paint {
      Paint::Color(color) => {
        let Ok(rgba) = color.to_rgba();
        let Ok([red, green, blue, alpha]) = rgba.to_rgba8();
        Ok(PreparedPaint {
          brush: PaintType::Solid(AlphaColor::from_rgba8(red, green, blue, alpha)),
          transform: Affine::IDENTITY,
          clip: None,
        })
      }
      Paint::Pattern(pattern) => match pattern.as_ref() {
        Pattern::Shading(pattern_2) => self.shading_paint(pattern_2, path, operation),
        Pattern::Tiling(pattern_3) => self.tiling_paint(pattern_3, operation),
      },
    };
    self.ctx.set_paint_transform(prepared.transform);
    self.ctx.set_paint(prepared.brush);

    Ok(prepared.clip)
  }

  fn shading_paint(
    &self,
    pattern: &ShadingPattern,
    path: &BezPath,
    operation: PaintOperation,
  ) -> Result<PreparedPaint, Never> {
    let transform = self.ctx.transform();
    let mut bbox = (*transform * path.clone()).bounding_box();

    match operation {
      PaintOperation::Stroke => {
        let Ok((x, y)) = x_y_advances(transform);
        let expansion = x.length().max(y.length()) * self.ctx.stroke().width;
        bbox = bbox.inflate(expansion, expansion);
      }
      PaintOperation::Fill => {}
    }

    let clipped = bbox.intersect(Rect::new(
      0.0,
      0.0,
      f64::from(self.ctx.width()),
      f64::from(self.ctx.height()),
    ));
    let Ok(encoded) = pattern.encode();
    let Ok(canvas_transparency) = self.image_transparency();
    let Ok(texture) = render_shading_texture(clipped, &encoded, canvas_transparency);
    let clip = pattern.shading.clip_path.clone();

    match texture {
      Some(pixmap) => {
        let placement = Affine::translate((clipped.x0, clipped.y0));
        let Ok(brush) = repeated_pixmap(pixmap);

        Ok(PreparedPaint {
          brush,
          transform: transform.inverse() * placement,
          clip,
        })
      }
      None => Ok(PreparedPaint {
        brush: PaintType::Solid(TRANSPARENT),
        transform: Affine::IDENTITY,
        clip,
      }),
    }
  }

  fn tiling_paint(
    &mut self,
    pattern: &TilingPattern<'_>,
    operation: PaintOperation,
  ) -> Result<PreparedPaint, Never> {
    let bbox = pattern.bbox;
    let Ok((x, y)) = x_y_advances(&pattern.matrix);
    let Ok(horizontal) = pattern_axis(PatternAxis {
      bounds: PatternPoints(bbox.width()),
      advance: x,
      step: PatternStep(pattern.x_step),
    });
    let Ok(vertical) = pattern_axis(PatternAxis {
      bounds: PatternPoints(bbox.height()),
      advance: y,
      step: PatternStep(pattern.y_step),
    });
    let size = Size {
      width: horizontal.pixels,
      height: vertical.pixels,
    };

    match (size.width, size.height) {
      (0, _) | (_, 0) => {
        return Ok(PreparedPaint {
          brush: PaintType::Solid(TRANSPARENT),
          transform: Affine::IDENTITY,
          clip: None,
        });
      }
      _ => {}
    }

    let transform = *self.ctx.transform();
    let Ok(settings) = derive_settings(self.ctx.render_settings());
    let Ok(mut renderer) = Renderer::new(size, settings, self.outlines);
    renderer.surface = Surface::PatternCell;
    let mut initial =
      Affine::scale_non_uniform(f64::from(horizontal.scale.0), f64::from(vertical.scale.0))
        * Affine::translate((-bbox.x0, -bbox.y0));
    let Ok(_) = pattern.interpret(&mut renderer, initial, operation);
    let mut pixmap = Pixmap::new(size.width, size.height);
    renderer.ctx.flush();
    let mut resources = vello_cpu::Resources::default();
    renderer.ctx.render_to_pixmap(&mut resources, &mut pixmap);

    match (horizontal.stride.0 < 0.0).then_some(()) {
      Some(()) => {
        initial *= Affine::new([-1.0, 0.0, 0.0, 1.0, f64::from(horizontal.scaled.0), 0.0])
      }
      None => {}
    }

    match (vertical.stride.0 < 0.0).then_some(()) {
      Some(()) => {
        initial *= Affine::new([1.0, 0.0, 0.0, -1.0, 0.0, f64::from(vertical.scaled.0)])
      }
      None => {}
    }

    let Ok(brush) = repeated_pixmap(pixmap);

    Ok(PreparedPaint {
      brush,
      transform: transform.inverse() * pattern.matrix * initial.inverse(),
      clip: None,
    })
  }

  fn stroke_path(
    &mut self,
    path: &BezPath,
    transform: Affine,
    paint: &Paint<'_>,
    stroke_props: &StrokeProps,
    target: StrokeTarget,
  ) -> Result<(), Never> {
    self.ctx.set_transform(transform);
    let Ok(()) = self.set_stroke_properties(stroke_props, target);

    let Ok(clip_path) = self.set_paint(paint, path, PaintOperation::Stroke);

    match clip_path.as_ref() {
      Some(clip_path_2) => {
        let Ok(()) = self.push_clip_path_inner(clip_path_2, FillRule::NonZero);
      }
      ::core::option::Option::None => {}
    }

    self.ctx.stroke_path(path);

    match clip_path {
      Some(_) => {
        self.ctx.pop_clip_path();
      }
      None => {}
    }

    Ok(())
  }

  fn fill_path(
    &mut self,
    path: &BezPath,
    transform: Affine,
    paint: &Paint<'_>,
    fill_rule: FillRule,
  ) -> Result<(), Never> {
    let Ok(rule) = convert_fill_rule(fill_rule);
    self.ctx.set_fill_rule(rule);
    self.ctx.set_transform(transform);

    let Ok(clip_path) = self.set_paint(paint, path, PaintOperation::Fill);

    match clip_path.as_ref() {
      Some(clip_path_2) => {
        let Ok(()) = self.push_clip_path_inner(clip_path_2, fill_rule);
      }
      ::core::option::Option::None => {}
    }

    self.ctx.fill_path(path);

    match clip_path {
      Some(_) => {
        self.ctx.pop_clip_path();
      }
      None => {}
    }

    Ok(())
  }

  fn fill_glyph<'a>(
    &mut self,
    glyph: &Glyph<'a>,
    transform: Affine,
    glyph_transform: Affine,
    paint: &Paint<'a>,
  ) -> Result<(), Never> {
    match glyph {
      Glyph::Outline(o) => {
        let Ok(base_outline) = self.cached_outline(o);

        let Ok(()) = self.fill_path(
          &base_outline,
          transform * glyph_transform,
          paint,
          FillRule::NonZero,
        );
      }
      Glyph::Type3(s) => {
        self.glyph = GlyphContext::Type3;
        let ::core::result::Result::Ok(()) = s.interpret(self, transform, glyph_transform, paint);
        self.glyph = GlyphContext::Ordinary;
      }
    }

    Ok(())
  }

  fn stroke_glyph<'a>(
    &mut self,
    glyph: &Glyph<'a>,
    transform: Affine,
    glyph_transform: Affine,
    paint: &Paint<'a>,
    stroke_props: &StrokeProps,
  ) -> Result<(), Never> {
    match glyph {
      Glyph::Outline(o) => {
        let Ok(base_outline) = self.cached_outline(o);

        let Ok(()) = self.stroke_path(
          &(glyph_transform * base_outline),
          transform,
          paint,
          stroke_props,
          StrokeTarget::Text,
        );
      }
      Glyph::Type3(s) => {
        let ::core::result::Result::Ok(()) = s.interpret(self, transform, glyph_transform, paint);
      }
    }

    Ok(())
  }

  fn cached_outline(
    &mut self,
    glyph: &console_core_portable_document_interpreter::font::OutlineGlyph,
  ) -> Result<BezPath, Never> {
    let Ok(identifier) = glyph.identifier();
    let id = identifier.cache_key();
    let outline = self.outlines.entry(id).or_insert_with(|| {
      let Ok(path) = glyph.outline();

      path
    });

    Ok(outline.clone())
  }

  fn cached_mask(&mut self, mask: &SoftMask<'_>) -> Result<Mask, Never> {
    let size = Size {
      width: self.ctx.width(),
      height: self.ctx.height(),
    };
    let settings = *self.ctx.render_settings();
    let held = self
      .soft_mask_cache
      .entry(mask.cache_key())
      .or_insert_with(|| {
        let Ok(raster) = draw_soft_mask(mask, settings, size);

        raster
      });

    Ok(held.clone())
  }

  fn image_transparency(&self) -> Result<Transparency, Never> {
    Ok(Transparency::from(
      self.cur_mask.is_some()
        || matches!(
          self.image_transparency_stack.last(),
          Some(Transparency::Possible)
        )
        || self.ctx.blend_mode() != peniko::BlendMode::default(),
    ))
  }
}

impl<'a> Device<'a> for Renderer<'_> {
  fn draw_image(
    &mut self,
    image: console_core_portable_document_interpreter::Image<'a, '_>,
    transform: Affine,
  ) {
    self.ctx.set_paint_transform(Affine::IDENTITY);
    self.ctx.set_aliasing_threshold(Some(1));

    let Ok(image_width) = image.width();
    let Ok(target_width) = toward_zero_u32(
      (transform * Point::new(f64::from(image_width), 0.0))
        .to_vec2()
        .length()
        .ceil(),
    );
    let Ok(image_height) = image.height();
    let Ok(target_height) = toward_zero_u32(
      (transform * Point::new(0.0, f64::from(image_height)))
        .to_vec2()
        .length()
        .ceil(),
    );

    let target = Some((target_width, target_height));

    match image {
      console_core_portable_document_interpreter::Image::Stencil(stencil) => {
        let Ok(decoded) = stencil.decoded_stencil(target);

        match decoded {
          Some((samples, paint)) => {
            let Ok(()) = self.draw_stencil(samples, paint, transform);
          }
          None => {}
        }
      }
      console_core_portable_document_interpreter::Image::Raster(raster) => {
        let Ok(decoded) = raster.decoded_rgba(target);

        match decoded {
          Some((image_2, alpha)) => {
            let Ok((sx, sy)) = image_2.scale_factors();
            let displayed = transform * Affine::scale_non_uniform(f64::from(sx), f64::from(sy));
            self.ctx.set_transform(displayed);
            let Ok(()) = self.draw_image(image_2, alpha);
          }
          None => {}
        }
      }
    }

    self.ctx.set_aliasing_threshold(None);
  }

  fn push_clip_path(&mut self, clip_path: &ClipPath) {
    let Ok(()) = self.push_clip_path_inner(&clip_path.path, clip_path.fill);
  }

  fn push_transparency_group(
    &mut self,
    opacity: f32,
    mask: Option<SoftMask<'_>>,
    blend_mode: BlendMode,
  ) {
    let transparency = Transparency::from(
      mask.is_some()
        || blend_mode != BlendMode::Normal
        || matches!(
          self.image_transparency_stack.last(),
          Some(Transparency::Possible)
        ),
    );
    let layer_mask = match mask {
      Some(mask_2) => {
        let Ok(raster) = self.cached_mask(&mask_2);

        Some(raster)
      }
      None => None,
    };
    let Ok(blend) = convert_blend_mode(blend_mode);
    self
      .ctx
      .push_layer(None, Some(blend), Some(opacity), layer_mask, None);
    self.image_transparency_stack.push(transparency);
  }

  fn pop_clip_path(&mut self) {
    self.ctx.pop_clip_path();
  }

  fn pop_transparency_group(&mut self) {
    self.image_transparency_stack.pop();
    self.ctx.pop_layer();
  }

  fn set_soft_mask(&mut self, mask: Option<SoftMask<'_>>) {
    self.cur_mask = match mask {
      Some(mask_3) => {
        let Ok(raster) = self.cached_mask(&mask_3);

        Some(raster)
      }
      None => None,
    };

    match self.cur_mask.clone() {
      Some(mask_2) => self.ctx.set_mask(mask_2),
      None => self.ctx.reset_mask(),
    }
  }

  fn draw_path(
    &mut self,
    path: &BezPath,
    transform: Affine,
    paint: &Paint<'_>,
    draw_mode: &PathDrawMode,
  ) {
    match draw_mode {
      PathDrawMode::Fill(f) => {
        let Ok(()) = Self::fill_path(self, path, transform, paint, *f);
      }
      PathDrawMode::Stroke(s) => {
        let Ok(()) = Self::stroke_path(self, path, transform, paint, s, StrokeTarget::Path);
      }
    }
  }

  fn draw_rect(
    &mut self,
    rect: &Rect,
    transform: Affine,
    paint: &Paint<'_>,
    draw_mode: &PathDrawMode,
  ) -> Result<(), console_core_never::Never> {
    let path = rect.to_path(0.1);

    match draw_mode {
      PathDrawMode::Fill(fill_rule) => {
        let Ok(rule) = convert_fill_rule(*fill_rule);
        self.ctx.set_fill_rule(rule);
        self.ctx.set_transform(transform);

        let Ok(clip_path) = self.set_paint(paint, &path, PaintOperation::Fill);

        match clip_path.as_ref() {
          Some(clip_path_2) => {
            let Ok(()) = self.push_clip_path_inner(clip_path_2, *fill_rule);
          }
          ::core::option::Option::None => {}
        }

        self.ctx.fill_rect(rect);

        match clip_path {
          Some(_) => {
            self.ctx.pop_clip_path();
          }
          None => {}
        }
      }
      PathDrawMode::Stroke(s) => {
        let Ok(()) = Self::stroke_path(self, &path, transform, paint, s, StrokeTarget::Path);
      }
    }

    Ok(())
  }

  fn draw_glyph(
    &mut self,
    glyph: &Glyph<'a>,
    transform: Affine,
    glyph_transform: Affine,
    paint: &Paint<'a>,
    draw_mode: &GlyphDrawMode,
  ) {
    match draw_mode {
      GlyphDrawMode::Fill => {
        let Ok(()) = Self::fill_glyph(self, glyph, transform, glyph_transform, paint);
      }
      GlyphDrawMode::Stroke(s) => {
        let Ok(()) = Self::stroke_glyph(self, glyph, transform, glyph_transform, paint, s);
      }
      GlyphDrawMode::Invisible => {}
    }
  }

  fn set_blend_mode(&mut self, blend_mode: BlendMode) {
    let Ok(blend) = convert_blend_mode(blend_mode);
    self.ctx.set_blend_mode(blend);
  }
}

fn image_area(size: Size<u32>) -> Result<Option<u32>, Never> {
  let count = u64::from(size.width).saturating_mul(u64::from(size.height));

  Ok(
    match (count != 0 && count <= MAX_IMAGE_PIXELS).then_some(()) {
      Some(()) => {
        let Ok(pixels) = fitted(count);

        Some(pixels)
      }
      None => None,
    },
  )
}

fn mask_color(mask: &LumaData, channel: u8) -> Result<Option<ImageData>, Never> {
  let Ok(area) = image_area(Size {
    width: mask.width,
    height: mask.height,
  });
  let Ok(length) = fitted::<_, u32>(mask.data.len());

  match area {
    Some(_area) => {}
    None => return Ok(None),
  }

  match (Some(length) == area).then_some(()) {
    Some(()) => {}
    None => return Ok(None),
  }

  Ok(Some(ImageData::Rgb(RgbData {
    data: mask.data.iter().flat_map(|_| [channel; 3]).collect(),
    width: mask.width,
    height: mask.height,
    interpolation: mask.interpolation,
    scale_factors: mask.scale_factors,
  })))
}

struct PaddedGlyph {
  pixels: Vec<u8>,
  size: Size<u32>,
}

fn padded_glyph(pixels: &[u8], size: Size<u32>) -> Result<Option<PaddedGlyph>, Never> {
  let padded = Size {
    width: size.width.saturating_add(4),
    height: size.height.saturating_add(4),
  };
  let Ok(area) = image_area(padded);

  match area {
    Some(_area) => {}
    None => return Ok(None),
  }

  let Ok(row_bytes) = index(size.width.saturating_mul(4));
  let first_row = match pixels.get(..row_bytes) {
    Some(row) => row,
    None => return Ok(None),
  };
  let mut output = Vec::new();
  let Ok(()) = pad_edge(&mut output, first_row);

  for row in pixels.chunks_exact(row_bytes.max(1)) {
    output.extend([0; 8]);
    output.extend_from_slice(row);
    output.extend([0; 8]);
  }

  let Ok(()) = pad_edge(&mut output, first_row);

  Ok(Some(PaddedGlyph {
    pixels: output,
    size: padded,
  }))
}

fn pad_edge(output: &mut Vec<u8>, row: &[u8]) -> Result<(), Never> {
  for _ in 0..2 {
    output.extend([0; 16]);
    output.extend(row.iter().map(|_| 0));
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Vello requires an Arc to install an owned CPU pixmap as a paint source"
  )
)]
fn sampled_pixmap(pixmap: Pixmap, sampler: ImageSampler) -> Result<Image, Never> {
  Ok(Image {
    image: ImageSource::Pixmap(Arc::new(pixmap)),
    sampler,
  })
}

fn repeated_pixmap(pixmap: Pixmap) -> Result<PaintType, Never> {
  let Ok(image) = sampled_pixmap(
    pixmap,
    ImageSampler {
      x_extend: peniko::Extend::Repeat,
      y_extend: peniko::Extend::Repeat,
      quality: ImageQuality::Medium,
      alpha: 1.0,
    },
  );

  Ok(PaintType::Image(image))
}

fn render_shading_texture(
  bounds: Rect,
  shading: &EncodedShadingPattern,
  mut transparency: Transparency,
) -> Result<Option<Pixmap>, Never> {
  let Ok(span_width) = nearest_f32_from_f64(bounds.width());
  let Ok(span_height) = nearest_f32_from_f64(bounds.height());
  let Ok(width) = toward_zero_u16(f64::from(span_width.max(1.0).ceil()));
  let Ok(height) = toward_zero_u16(f64::from(span_height.max(1.0).ceil()));
  let Ok(area) = image_area(Size {
    width: u32::from(width),
    height: u32::from(height),
  });

  match area {
    Some(_nonempty) => {}
    None => return Ok(None),
  }

  let Ok((x_advance, y_advance)) = x_y_advances(&shading.base_transform);
  let mut pixels = Vec::new();
  let mut start_point =
    shading.base_transform * Affine::translate((0.5, 0.5)) * Point::new(bounds.x0, bounds.y0);

  for _ in 0..height {
    let mut point = start_point;

    for _ in 0..width {
      let Ok(sample) = shading.sample(point);
      let pixel = AlphaColor::<Srgb>::new(sample).premultiply().to_rgba8();

      match pixel.a {
        255 => {}
        _ => transparency = Transparency::Possible,
      }

      pixels.push(pixel);
      point += x_advance;
    }

    start_point += y_advance;
  }

  Ok(Some(Pixmap::from_parts_with_opacity(
    pixels,
    width,
    height,
    bool::from(transparency),
  )))
}

struct PreparedPaint {
  brush: PaintType,
  transform: Affine,
  clip: Option<BezPath>,
}

#[doc = "Space between repeats of a tiling pattern, in pattern-space points."]
#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the wrapped float is a signed distance between pattern cells in PDF points"
  )
)]
pub struct PatternStep(#[doc = "The signed distance in points."] pub f32);

#[doc = "A tiling cell scale and its maximum image extent in device pixels."]
pub struct TileCell {
  #[doc = "Device pixels per pattern point."]
  pub scale: ScaleFactor,
  #[doc = "The signed spacing between cells in pattern points."]
  pub step: PatternStep,
  #[doc = "Maximum supported side length of the tile image, in pixels."]
  pub maximum_pixels: u16,
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the wrapped float measures a pattern-space extent in PDF points"
  )
)]
struct PatternPoints(f64);

struct PatternAxis {
  bounds: PatternPoints,
  advance: Vec2,
  step: PatternStep,
}

struct RasterAxis {
  scale: ScaleFactor,
  stride: crate::ViewOffset,
  scaled: crate::ViewOffset,
  pixels: u16,
}

fn pattern_axis(axis: PatternAxis) -> Result<RasterAxis, Never> {
  const MAXIMUM: u16 = 3000;
  let Ok(span) = nearest_f32_from_f64(axis.bounds.0);
  let Ok(advance) = nearest_f32_from_f64(axis.advance.length());
  let minimum_scale = 1.0 / span;
  let maximum_scale = f32::from(MAXIMUM) / span;
  let scale = ScaleFactor(advance.max(minimum_scale).min(maximum_scale));
  let Ok(scale_2) = tiling_cell_scale(TileCell {
    scale,
    step: axis.step,
    maximum_pixels: MAXIMUM,
  });
  let stride = crate::ViewOffset(scale_2.0 * axis.step.0);
  let scaled = crate::ViewOffset(span * scale_2.0);
  let Ok(pixels) = whole_u16(f64::from(stride.0.abs()));

  Ok(RasterAxis {
    scale: scale_2,
    stride,
    scaled,
    pixels,
  })
}

fn draw_soft_mask(
  mask: &SoftMask<'_>,
  settings: RenderSettings,
  Size { width, height }: Size<u16>,
) -> Result<Mask, Never> {
  let mut outlines = HashMap::new();
  let Ok(single_threaded) = derive_settings(&settings);
  let mut renderer = Renderer {
    ctx: RenderContext::new_with(width, height, single_threaded),
    surface: Surface::Canvas,
    cur_mask: None,
    soft_mask_cache: HashMap::default(),
    outlines: &mut outlines,
    glyph: GlyphContext::Ordinary,

    image_transparency_stack: Vec::new(),
  };

  let Ok(background_color) = mask.background_color();
  let Ok(bg_color) = background_color.to_rgba();
  let Ok(rgba8) = bg_color.to_rgba8();
  let compositing = Compositing::from(rgba8 != BLACK.to_rgba8().to_u8_array());

  match compositing {
    Compositing::Layered => {
      let Ok(components) = bg_color.components();
      renderer.ctx.set_paint(AlphaColor::<Srgb>::new(components));
      renderer.ctx.fill_rect(&Rect::new(
        0.0,
        0.0,
        <f64 as ::core::convert::From<::core::primitive::u16>>::from(width),
        <f64 as ::core::convert::From<::core::primitive::u16>>::from(height),
      ));
      renderer.ctx.push_layer(None, None, None, None, None);
    }
    Compositing::Direct => {}
  }

  let ::core::result::Result::Ok(()) = mask.interpret(&mut renderer);

  match compositing {
    Compositing::Layered => {
      renderer.ctx.pop_layer();
    }
    Compositing::Direct => {}
  }

  let mut pix = Pixmap::new(width, height);
  renderer.ctx.flush();
  let mut resources = vello_cpu::Resources::default();
  renderer.ctx.render_to_pixmap(&mut resources, &mut pix);

  let Ok(mask_type) = mask.mask_type();
  let mut rendered_mask = match mask_type {
    MaskType::Luminosity => Mask::new_luminance(&pix),
    MaskType::Alpha => Mask::new_alpha(&pix),
  };

  let Ok(mask_transfer_function) = mask.transfer_function();

  match mask_transfer_function {
    Some(transfer_function) => {
      let mut map = Vec::new();

      for y in 0..rendered_mask.height() {
        for x in 0..rendered_mask.width() {
          let Ok(apply) = transfer_function.apply(
            <f32 as ::core::convert::From<::core::primitive::u8>>::from(
              rendered_mask.sample(x, y),
            ) / 255.0,
          );
          let Ok(sample) = toward_zero_u8(f64::from(apply * 255.0 + 0.5));
          map.push(sample);
        }
      }

      rendered_mask = Mask::from_parts(map, rendered_mask.width(), rendered_mask.height());
    }
    ::core::option::Option::None => {}
  }

  Ok(rendered_mask)
}

fn max_factor(transform: &Affine) -> Result<ScaleFactor, Never> {
  let Ok((x, y)) = x_y_advances(transform);
  let Ok(scale) = nearest_f32_from_f64(x.length().max(y.length()));

  Ok(ScaleFactor(scale))
}

fn x_y_advances(transform: &Affine) -> Result<(Vec2, Vec2), Never> {
  let [xx, xy, yx, yy, _, _] = transform.as_coeffs();
  let linear = Affine::new([xx, xy, yx, yy, 0.0, 0.0]);
  let x = linear * Point::new(1.0, 0.0);
  let y = linear * Point::new(0.0, 1.0);

  Ok((x.to_vec2(), y.to_vec2()))
}

fn convert_fill_rule(fill_rule: FillRule) -> Result<Fill, Never> {
  Ok(match fill_rule {
    FillRule::NonZero => Fill::NonZero,
    FillRule::EvenOdd => Fill::EvenOdd,
  })
}

fn convert_blend_mode(blend_mode: BlendMode) -> Result<peniko::BlendMode, Never> {
  let mix = match blend_mode {
    BlendMode::Normal => Mix::Normal,
    BlendMode::Multiply => Mix::Multiply,
    BlendMode::Screen => Mix::Screen,
    BlendMode::Overlay => Mix::Overlay,
    BlendMode::Darken => Mix::Darken,
    BlendMode::Lighten => Mix::Lighten,
    BlendMode::ColorDodge => Mix::ColorDodge,
    BlendMode::ColorBurn => Mix::ColorBurn,
    BlendMode::HardLight => Mix::HardLight,
    BlendMode::SoftLight => Mix::SoftLight,
    BlendMode::Difference => Mix::Difference,
    BlendMode::Exclusion => Mix::Exclusion,
    BlendMode::Hue => Mix::Hue,
    BlendMode::Saturation => Mix::Saturation,
    BlendMode::Color => Mix::Color,
    BlendMode::Luminosity => Mix::Luminosity,
  };

  Ok(peniko::BlendMode::new(mix, Compose::SrcOver))
}

#[doc = "Lower the render scale of a tiling cell to fit its image limit without changing the geometry of the repeat."]
pub fn tiling_cell_scale(cell: TileCell) -> Result<ScaleFactor, Never> {
  let step = cell.step.0.abs();
  let cap = f32::from(cell.maximum_pixels);

  Ok(
    match (step.is_finite() && step > 0.0 && cell.scale.0 * step > cap).then_some(()) {
      Some(()) => ScaleFactor(cap / step),
      None => cell.scale,
    },
  )
}
