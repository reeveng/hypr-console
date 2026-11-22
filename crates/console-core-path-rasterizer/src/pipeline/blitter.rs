use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_size::Size;

use crate::alpha_runs::AlphaRun;
use crate::blitter::{Blitter, CoverageBlitter, Mask, SpanBlitter};
use crate::color::AlphaU8;
use crate::geom::{PixelPosition, ScreenIntRect};
use crate::mask::SubMaskRef;
use crate::pipeline::{self, RasterPipeline, RasterPipelineBuilder, Stage};
use crate::pixmap::SubPixmapMut;
use crate::{
  ALPHA_U8_OPAQUE, ALPHA_U8_TRANSPARENT, BlendMode, Color, ColorSpace, CoverageMode,
  IsOpaqueAnswer, LengthU32, Paint, PixmapRef, PremultipliedColorU8, PushStagesAnswer, Shader,
  ShouldPreScaleCoverageAnswer,
};

pub struct RasterPipelineBlitter<'a, 'b: 'a> {
  mask: Option<SubMaskRef<'a>>,
  pixmap_src: PixmapRef<'a>,
  pixmap: &'a mut SubPixmapMut<'b>,
  fill: Fill,
  blit_anti_h_rp: RasterPipeline,
  blit_rect_rp: RasterPipeline,
  blit_mask_rp: RasterPipeline,
}

enum Fill {
  Pipeline,
  Pixels(PremultipliedColorU8),
  Mask,
}

#[derive(Clone, Copy)]
enum Masking {
  None,
  Present,
}

#[derive(Clone, Copy)]
struct Blend {
  mode: BlendMode,
  mask: Masking,
}

enum Transfer {
  Rectangle,
  Coverage { scale: Stage, lerp: Stage },
}

impl<'a, 'b: 'a> RasterPipelineBlitter<'a, 'b> {
  pub fn new(
    paint: &Paint<'a>,
    mask: Option<SubMaskRef<'a>>,
    pixmap: &'a mut SubPixmapMut<'b>,
  ) -> Result<Option<Self>, Never> {
    let masking = match mask {
      Some(mask_2) => match (mask_2.size != pixmap.size).then_some(()) {
        Some(()) => {
          console_core_logging::warn!("Pixmap and Mask are expected to have the same size");
          return Ok(None);
        }
        None => Masking::Present,
      },
      None => Masking::None,
    };
    let Ok(planned) = blend_for(paint, masking);
    let (blend, fill) = match planned {
      Some(plan) => plan,
      None => return Ok(None),
    };
    let Ok(horizontal) = pipeline_for(
      paint,
      blend,
      Transfer::Coverage {
        scale: Stage::Scale1Float,
        lerp: Stage::Lerp1Float,
      },
    );
    let Ok(rectangle) = pipeline_for(paint, blend, Transfer::Rectangle);
    let Ok(masked) = pipeline_for(
      paint,
      blend,
      Transfer::Coverage {
        scale: Stage::ScaleU8,
        lerp: Stage::LerpU8,
      },
    );
    let (blit_anti_h_rp, blit_rect_rp, blit_mask_rp) = match (horizontal, rectangle, masked) {
      (Some(horizontal_2), Some(rectangle_2), Some(masked_2)) => {
        (horizontal_2, rectangle_2, masked_2)
      }
      (None, _, _) | (_, None, _) | (_, _, None) => return Ok(None),
    };
    let pixmap_src = match &paint.shader {
      Shader::Pattern(pattern) => pattern.pixmap,
      Shader::SolidColor(_)
      | Shader::LinearGradient(_)
      | Shader::RadialGradient(_)
      | Shader::SweepGradient(_) => {
        let Ok(empty) = PixmapRef::from_bytes(&[0, 0, 0, 0], 1, 1);

        match empty {
          Some(pixmap_ref) => pixmap_ref,
          None => return Ok(None),
        }
      }
    };

    Ok(Some(Self {
      mask,
      pixmap_src,
      pixmap,
      fill,
      blit_anti_h_rp,
      blit_rect_rp,
      blit_mask_rp,
    }))
  }

  pub fn new_mask(pixmap: &'a mut SubPixmapMut<'b>) -> Result<Option<Self>, Never> {
    let Ok(pixmap_ref) = PixmapRef::from_bytes(&[0, 0, 0, 0], 1, 1);
    let pixmap_src = match pixmap_ref {
      Some(source) => source,
      None => return Ok(None),
    };
    let Ok(color) = Color::WHITE.premultiply();

    let Ok(mut horizontal) = RasterPipelineBuilder::new();
    let Ok(()) = horizontal.push_uniform_color(color);
    let Ok(()) = horizontal.push(Stage::LoadDestinationU8);
    let Ok(()) = horizontal.push(Stage::Lerp1Float);
    let Ok(()) = horizontal.push(Stage::StoreU8);
    let Ok(blit_anti_h_rp) = horizontal.compile();

    let Ok(mut rectangle) = RasterPipelineBuilder::new();
    let Ok(()) = rectangle.push_uniform_color(color);
    let Ok(()) = rectangle.push(Stage::StoreU8);
    let Ok(blit_rect_rp) = rectangle.compile();

    let Ok(mut masked) = RasterPipelineBuilder::new();
    let Ok(()) = masked.push_uniform_color(color);
    let Ok(()) = masked.push(Stage::LoadDestinationU8);
    let Ok(()) = masked.push(Stage::LerpU8);
    let Ok(()) = masked.push(Stage::StoreU8);
    let Ok(blit_mask_rp) = masked.compile();

    Ok(Some(Self {
      mask: None,
      pixmap_src,
      pixmap,
      fill: Fill::Mask,
      blit_anti_h_rp,
      blit_rect_rp,
      blit_mask_rp,
    }))
  }

  fn mask_context(&self) -> Result<pipeline::MaskCtx<'a>, Never> {
    match self.mask {
      Some(mask) => mask.mask_ctx(),
      None => Ok(pipeline::MaskCtx::default()),
    }
  }
}

fn blend_for(paint: &Paint<'_>, mask: Masking) -> Result<Option<(Blend, Fill)>, Never> {
  let Ok(opaque) = paint.shader.is_opaque();
  let blend_mode = match (
    paint.blend_mode,
    opaque,
    &paint.shader,
    mask,
    paint.coverage,
  ) {
    (BlendMode::Destination, _, _, _, _)
    | (BlendMode::DestinationIn, IsOpaqueAnswer::Yes, Shader::SolidColor(_), _, _) => {
      return Ok(None);
    }
    (BlendMode::SourceOver, IsOpaqueAnswer::Yes, _, Masking::None, _) => BlendMode::Source,
    (mode, _, _, _, _) => mode,
  };

  let (mode, fill) = match (blend_mode, &paint.shader, mask, paint.coverage) {
    (BlendMode::Source, Shader::SolidColor(color), Masking::None, _) => {
      let Ok(premultiplied) = color.premultiply();
      let Ok(pixel) = premultiplied.to_color_u8();

      (BlendMode::Source, Fill::Pixels(pixel))
    }
    (BlendMode::Clear, _, Masking::None, CoverageMode::Binary) => (
      BlendMode::Source,
      Fill::Pixels(PremultipliedColorU8::TRANSPARENT),
    ),
    (mode, _, _, _) => (mode, Fill::Pipeline),
  };

  Ok(Some((Blend { mode, mask }, fill)))
}

fn pipeline_for(
  paint: &Paint<'_>,
  blend: Blend,
  transfer: Transfer,
) -> Result<Option<RasterPipeline>, Never> {
  let Ok(mut pipeline) = RasterPipelineBuilder::new();
  let Ok(()) = pipeline.set_precision(paint.precision);
  let Ok(stages) = paint.shader.push_stages(paint.colorspace, &mut pipeline);

  match stages {
    PushStagesAnswer::No => return Ok(None),
    PushStagesAnswer::Yes => {}
  }

  match blend.mask {
    Masking::None => {}
    Masking::Present => {
      let Ok(()) = pipeline.push(Stage::MaskU8);
    }
  }

  let Ok(compress) = paint.colorspace.compress_stage();

  match (transfer, blend.mode, blend.mask) {
    (Transfer::Rectangle, BlendMode::SourceOver, Masking::None) => {
      let Ok(()) = push_optional(&mut pipeline, compress);
      let Ok(()) = pipeline.push(Stage::SourceOverRgba);
    }
    (Transfer::Rectangle, mode, _) => {
      let Ok(()) = rectangle_stages(&mut pipeline, paint.colorspace, mode);
      let Ok(()) = push_optional(&mut pipeline, compress);
      let Ok(()) = pipeline.push(Stage::Store);
    }
    (Transfer::Coverage { scale, lerp }, mode, _) => {
      let Ok(()) = coverage_stages(&mut pipeline, paint.colorspace, mode, [scale, lerp]);
      let Ok(()) = push_optional(&mut pipeline, compress);
      let Ok(()) = pipeline.push(Stage::Store);
    }
  }

  let Ok(ready) = pipeline.compile();

  Ok(Some(ready))
}

fn rectangle_stages(
  pipeline: &mut RasterPipelineBuilder,
  colorspace: ColorSpace,
  mode: BlendMode,
) -> Result<(), Never> {
  match (mode != BlendMode::Source).then_some(()) {
    Some(()) => {
      let Ok(()) = pipeline.push(Stage::LoadDestination);
    }
    None => return Ok(()),
  }

  let Ok(stage) = mode.to_stage();

  match stage {
    Some(blend) => {
      let Ok(expand) = colorspace.expand_dest_stage();
      let Ok(()) = push_optional(pipeline, expand);

      pipeline.push(blend)
    }
    None => Ok(()),
  }
}

fn coverage_stages(
  pipeline: &mut RasterPipelineBuilder,
  colorspace: ColorSpace,
  mode: BlendMode,
  [scale, lerp]: [Stage; 2],
) -> Result<(), Never> {
  let Ok(prescale) = mode.should_pre_scale_coverage();

  match prescale {
    ShouldPreScaleCoverageAnswer::Yes => {
      let Ok(()) = pipeline.push(scale);
    }
    ShouldPreScaleCoverageAnswer::No => {}
  }

  let Ok(()) = pipeline.push(Stage::LoadDestination);
  let Ok(expand) = colorspace.expand_dest_stage();
  let Ok(()) = push_optional(pipeline, expand);
  let Ok(blend) = mode.to_stage();
  let Ok(()) = push_optional(pipeline, blend);

  match prescale {
    ShouldPreScaleCoverageAnswer::Yes => Ok(()),
    ShouldPreScaleCoverageAnswer::No => pipeline.push(lerp),
  }
}

fn push_optional(
  pipeline: &mut RasterPipelineBuilder,
  stage: Option<Stage>,
) -> Result<(), Never> {
  match stage {
    Some(stage_2) => pipeline.push(stage_2),
    None => Ok(()),
  }
}

impl SpanBlitter for RasterPipelineBlitter<'_, '_> {
  fn blit_h(&mut self, at: PixelPosition, width: LengthU32) -> Result<(), Never> {
    let Ok(bounds) = ScreenIntRect::new(
      at,
      Size {
        width: width.get(),
        height: 1,
      },
    );

    match bounds {
      Some(rect) => self.blit_rect(&rect),
      None => Ok(()),
    }
  }
}

impl CoverageBlitter for RasterPipelineBlitter<'_, '_> {
  fn blit_anti_h(
    &mut self,
    mut at: PixelPosition,
    alpha: &mut [AlphaU8],
    runs: &mut [AlphaRun],
  ) -> Result<(), Never> {
    let Ok(mask_ctx) = self.mask_context();
    let mut offset = 0u32;

    for _ in &*runs {
      let Ok(indexed) = index(offset);
      let (run, alpha_2) = match (runs.get(indexed).copied().flatten(), alpha.get(indexed)) {
        (Some(run), Some(&alpha_2)) => (run, alpha_2),
        (None, _) | (_, None) => break,
      };
      let width = LengthU32::from(run);

      match alpha_2 {
        ALPHA_U8_TRANSPARENT => {}
        ALPHA_U8_OPAQUE => {
          let Ok(()) = self.blit_h(at, width);
        }
        partial => {
          let Ok(coverage) =
            console_core_paths::NormalizedF32::nearest(f32::from(partial) * (1.0 / 255.0));
          self.blit_anti_h_rp.ctx.current_coverage = coverage;

          let Ok(bounds) = ScreenIntRect::new(
            at,
            Size {
              width: width.get(),
              height: 1,
            },
          );

          match bounds {
            Some(rect) => {
              let Ok(()) = self.blit_anti_h_rp.run(
                &rect,
                pipeline::AAMaskCtx::default(),
                mask_ctx,
                self.pixmap_src,
                self.pixmap,
              );
            }
            None => {}
          }
        }
      }

      at.x = at.x.wrapping_add(width.get());
      offset = offset.saturating_add(width.get());
    }

    Ok(())
  }

  fn blit_v(
    &mut self,
    at: PixelPosition,
    height: LengthU32,
    alpha: AlphaU8,
  ) -> Result<(), Never> {
    let Ok(rect) = ScreenIntRect::new(
      at,
      Size {
        width: 1,
        height: height.get(),
      },
    );

    match rect {
      Some(bounds) => self.blit_mask(&Mask {
        image: [alpha, alpha],
        bounds,
        row_bytes: 0,
      }),
      None => Ok(()),
    }
  }

  fn blit_anti_h2(&mut self, at: PixelPosition, image: [AlphaU8; 2]) -> Result<(), Never> {
    let Ok(rect) = ScreenIntRect::new(
      at,
      Size {
        width: 2,
        height: 1,
      },
    );

    match rect {
      Some(bounds) => self.blit_mask(&Mask {
        image,
        bounds,
        row_bytes: 2,
      }),
      None => Ok(()),
    }
  }

  fn blit_anti_v2(&mut self, at: PixelPosition, image: [AlphaU8; 2]) -> Result<(), Never> {
    let Ok(rect) = ScreenIntRect::new(
      at,
      Size {
        width: 1,
        height: 2,
      },
    );

    match rect {
      Some(bounds) => self.blit_mask(&Mask {
        image,
        bounds,
        row_bytes: 1,
      }),
      None => Ok(()),
    }
  }
}

impl Blitter for RasterPipelineBlitter<'_, '_> {
  fn blit_rect(&mut self, rect: &ScreenIntRect) -> Result<(), Never> {
    let stride = u64::from(self.pixmap.real_width);

    match self.fill {
      Fill::Mask => fill_solid(self.pixmap.data, rect, stride, ALPHA_U8_OPAQUE),
      Fill::Pixels(color) => {
        let Ok(pixels) = self.pixmap.pixels_mut();

        fill_solid(pixels, rect, stride, color)
      }
      Fill::Pipeline => {
        let Ok(mask) = self.mask_context();

        self.blit_rect_rp.run(
          rect,
          pipeline::AAMaskCtx::default(),
          mask,
          self.pixmap_src,
          self.pixmap,
        )
      }
    }
  }
}

fn fill_solid<T: Copy>(
  pixels: &mut [T],
  rect: &ScreenIntRect,
  stride: u64,
  value: T,
) -> Result<(), Never> {
  let Ok(row_stride) = index(stride);
  let Ok(top) = rect.top();
  let Ok(height) = rect.height();
  let Ok(left) = rect.left();
  let Ok(right) = rect.right();
  let Ok(skip) = index(top);
  let Ok(count) = index(height);
  let Ok(first) = index(left);
  let Ok(last) = index(right);

  for row in pixels.chunks_mut(row_stride.max(1)).skip(skip).take(count) {
    match row.get_mut(first..last) {
      Some(span) => span.fill(value),
      None => {}
    }
  }

  Ok(())
}

impl RasterPipelineBlitter<'_, '_> {
  fn blit_mask(&mut self, covered: &Mask) -> Result<(), Never> {
    let Ok(x) = covered.bounds.left();
    let Ok(y) = covered.bounds.top();
    let shift =
      u64::from(x).wrapping_add(u64::from(y).wrapping_mul(u64::from(covered.row_bytes)));
    let coverage = pipeline::AAMaskCtx {
      pixels: covered.image,
      stride: covered.row_bytes,
      shift,
    };
    let Ok(mask_ctx) = self.mask_context();

    self.blit_mask_rp.run(
      &covered.bounds,
      coverage,
      mask_ctx,
      self.pixmap_src,
      self.pixmap,
    )
  }
}
