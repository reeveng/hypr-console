use console_core_never::Never;
use console_core_number_conversion::nearest_f32_from_u64;
use console_core_paths::{IsIdentityAnswer, IsTranslateAnswer, NormalizedF32, StrictNormalized};

use crate::{BlendMode, ColorSpace, PixmapRef, Shader, SpreadMode, Transform};

use crate::pipeline::{RasterPipelineBuilder, SamplerCtx, Stage, TileCtx};

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum FilterQuality {
  Nearest,

  Bilinear,

  Bicubic,
}

#[derive(Copy, Clone, PartialEq, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "opacity is the public normalized alpha fraction"
  )
)]
pub struct PixmapPaint {
  pub opacity: f32,

  pub blend_mode: BlendMode,

  pub quality: FilterQuality,
}

impl Default for PixmapPaint {
  fn default() -> Self {
    PixmapPaint {
      opacity: 1.0,
      blend_mode: BlendMode::default(),
      quality: FilterQuality::Nearest,
    }
  }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Pattern<'a> {
  pub(crate) pixmap: PixmapRef<'a>,
  quality: FilterQuality,
  spread_mode: SpreadMode,
  pub(crate) opacity: NormalizedF32,
  pub(crate) transform: Transform,
}

impl<'a> Pattern<'a> {
  #[expect(
    clippy::new_ret_no_self,
    reason = "a pattern is only ever painted as one of the shaders a Paint holds, so new answers that Shader rather than Self"
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "opacity is expressed as a normalized alpha fraction"
    )
  )]
  pub fn new(
    pixmap: PixmapRef<'a>,
    spread_mode: SpreadMode,
    quality: FilterQuality,
    opacity: f32,
    transform: Transform,
  ) -> core::result::Result<Shader<'a>, console_core_never::Never> {
    Ok(Shader::Pattern(Pattern {
      pixmap,
      spread_mode,
      quality,
      opacity: NormalizedF32::new_clamped(opacity),
      transform,
    }))
  }

  pub(crate) fn push_stages(
    &self,
    color_space: ColorSpace,
    builder: &mut RasterPipelineBuilder,
  ) -> Result<crate::PushStagesAnswer, Never> {
    let Ok(inverse) = self.transform.invert();
    let transform = match inverse {
      Some(value) => value,
      None => {
        console_core_logging::warn!(
          "failed to invert a pattern transform. Nothing will be rendered"
        );

        return Ok(crate::PushStagesAnswer::No);
      }
    };
    let Ok(()) = builder.push(Stage::SeedShader);
    let Ok(()) = builder.push_transform(transform);
    let Ok(identity) = transform.is_identity();
    let Ok(translation) = transform.is_translate();
    let filter = match (identity, translation) {
      (IsIdentityAnswer::Yes, _) | (_, IsTranslateAnswer::Yes) => FilterQuality::Nearest,
      (IsIdentityAnswer::No, IsTranslateAnswer::No) => self.quality,
    };
    let Ok(()) = self.sample(filter, builder);

    match (self.opacity != NormalizedF32::ONE).then_some(()) {
      Some(()) => {
        builder.ctx.current_coverage = self.opacity;
        let Ok(()) = builder.push(Stage::Scale1Float);
      }
      None => {}
    }

    let Ok(expand) = color_space.expand_stage();

    match expand {
      Some(stage) => {
        let Ok(()) = builder.push(stage);
      }
      None => {}
    }

    Ok(crate::PushStagesAnswer::Yes)
  }

  fn sample(
    &self,
    filter: FilterQuality,
    builder: &mut RasterPipelineBuilder,
  ) -> Result<(), Never> {
    let Ok(width) = self.pixmap.width();
    let Ok(height) = self.pixmap.height();
    let Ok(columns) = nearest_f32_from_u64(u64::from(width));
    let Ok(rows) = nearest_f32_from_u64(u64::from(height));
    let horizontal = TileCtx {
      scale: columns,
      inv_scale: 1.0 / columns,
    };
    let vertical = TileCtx {
      scale: rows,
      inv_scale: 1.0 / rows,
    };

    match filter {
      FilterQuality::Nearest => {
        builder.ctx.limit_x = horizontal;
        builder.ctx.limit_y = vertical;

        match self.spread_mode {
          SpreadMode::Pad => {}
          SpreadMode::Repeat => {
            let Ok(()) = builder.push(Stage::Repeat);
          }
          SpreadMode::Reflect => {
            let Ok(()) = builder.push(Stage::Reflect);
          }
        }

        builder.push(Stage::Gather)
      }
      FilterQuality::Bilinear => {
        builder.ctx.sampler = SamplerCtx {
          spread_mode: self.spread_mode,
          horizontal,
          vertical,
        };

        builder.push(Stage::Bilinear)
      }
      FilterQuality::Bicubic => {
        builder.ctx.sampler = SamplerCtx {
          spread_mode: self.spread_mode,
          horizontal,
          vertical,
        };
        let Ok(()) = builder.push(Stage::Bicubic);
        let Ok(()) = builder.push(Stage::Clamp0);

        builder.push(Stage::ClampA)
      }
    }
  }
}
