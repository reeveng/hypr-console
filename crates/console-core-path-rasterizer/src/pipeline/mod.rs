/*!
A raster pipeline implementation.

Despite having a lot of changes compared to `SkRasterPipeline`,
the core principles are the same:

1. A pipeline consists of stages.
1. A pipeline has a global context shared by all stages.
   Unlike Skia, were each stage has it's own, possibly shared, context.
1. Each stage has a high precision implementation. See `highp.rs`.
1. Some stages have a low precision implementation. See `lowp.rs`.
1. Each stage calls the "next" stage after its done.
1. During pipeline "compilation", if **all** stages have a lowp implementation,
   the lowp pipeline will be used. Otherwise, the highp variant will be used.
1. Compilation produces function pointer tables for full and partial groups
   of pixels. Each stage consumes one entry from its remaining table.

This implementation is a bit tricky, but it gives the maximum performance.
A simple and straightforward implementation using traits and loops, like:

```ignore
trait StageTrait {
    fn apply(&mut self, pixels: &mut [Pixel]);
}

let stages: Vec<&mut dyn StageTrait>;
for stage in stages {
    stage.apply(pixels);
}
```

will be at least 20-30% slower. Not really sure why.

Also, since this module is all about performance, any kind of branching is
strictly forbidden. All stage functions must not use `if`, `match` or loops.
There are still some exceptions, which are basically an imperfect implementations
and should be optimized out in the future.
*/

use alloc::vec::Vec;

use console_core_fixed_capacity_vectors::FixedVec;
use console_core_never::Never;
use console_core_number_conversion::index;

use console_core_paths::NormalizedF32;

use crate::{Color, PremultipliedColor, PremultipliedColorU8, SpreadMode};
use crate::{PixmapRef, Transform};

pub use blitter::RasterPipelineBlitter;

use crate::geom::{PixelPosition, ScreenIntRect};
use crate::pixmap::SubPixmapMut;
use crate::wide::u32x8;

mod blitter;
#[rustfmt::skip]
mod highp;
#[rustfmt::skip]
mod lowp;

type StageList<T> = FixedVec<T, 32>;

#[expect(
  dead_code,
  reason = "both pipeline dispatches name every stage, including stages absent from the enabled features"
)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Stage {
  MoveSourceToDestination = 0,
  MoveDestinationToSource = 0x81,
  Clamp0 = 0x82,
  ClampA = 0x3,
  Premultiply = 0x84,
  UniformColor = 0x5,
  SeedShader = 0x6,
  LoadDestination = 0x87,
  Store = 0x88,
  LoadDestinationU8 = 0x9,
  StoreU8 = 0xa,
  Gather = 0x8b,
  LoadMaskU8 = 0xc,
  MaskU8 = 0x8d,
  ScaleU8 = 0x8e,
  LerpU8 = 0xf,
  Scale1Float = 0x90,
  Lerp1Float = 0x11,
  DestinationAtop = 0x12,
  DestinationIn = 0x93,
  DestinationOut = 0x14,
  DestinationOver = 0x95,
  SourceAtop = 0x96,
  SourceIn = 0x17,
  SourceOut = 0x18,
  SourceOver = 0x99,
  Clear = 0x9a,
  Modulate = 0x1b,
  Multiply = 0x9c,
  Plus = 0x1d,
  Screen = 0x1e,
  Xor = 0x9f,
  ColorBurn = 0xa0,
  ColorDodge = 0x21,
  Darken = 0x22,
  Difference = 0xa3,
  Exclusion = 0x24,
  HardLight = 0xa5,
  Lighten = 0xa6,
  Overlay = 0x27,
  SoftLight = 0x28,
  Hue = 0xa9,
  Saturation = 0xaa,
  Color = 0x2b,
  Luminosity = 0xac,
  SourceOverRgba = 0x2d,
  Transform = 0x2e,
  Reflect = 0xaf,
  Repeat = 0x30,
  Bilinear = 0xb1,
  Bicubic = 0xb2,
  PadX1 = 0x33,
  ReflectX1 = 0xb4,
  RepeatX1 = 0x35,
  Gradient = 0x36,
  EvenlySpaced2StopGradient = 0xb7,
  XYToUnitAngle = 0xb8,
  XYToRadius = 0x39,
  XYTo2PtConicalFocalOnCircle = 0x3a,
  XYTo2PtConicalWellBehaved = 0xbb,
  XYTo2PtConicalSmaller = 0x3c,
  XYTo2PtConicalGreater = 0xbd,
  XYTo2PtConicalStrip = 0xbe,
  Mask2PtConicalNan = 0x3f,
  Mask2PtConicalDegenerates = 0xc0,
  ApplyVectorMask = 0x41,
  Alter2PtConicalCompensateFocal = 0x42,
  Alter2PtConicalUnswap = 0xc3,
  NegateX = 0x44,
  ApplyConcentricScaleBias = 0xc5,
  GammaExpand2 = 0xc6,
  GammaExpandDestination2 = 0x47,
  GammaCompress2 = 0x48,
  GammaExpand22 = 0xc9,
  GammaExpandDestination22 = 0xca,
  GammaCompress22 = 0x4b,
  GammaExpandSrgb = 0xcc,
  GammaExpandDestinationSrgb = 0x4d,
  GammaCompressSrgb = 0x4e,
}

impl PixmapRef<'_> {
  #[inline(always)]
  fn gather(&self, offsets: u32x8) -> Result<highp::Lanes<PremultipliedColorU8>, Never> {
    let offsets_2: highp::Lanes<u32> = bytemuck::cast(offsets);
    let Ok(pixels) = self.pixels();

    Ok(offsets_2.map(|offset| {
      let Ok(position) = index(offset);

      match pixels.get(position) {
        Some(pixel) => *pixel,
        None => PremultipliedColorU8::TRANSPARENT,
      }
    }))
  }
}

impl SubPixmapMut<'_> {
  #[inline(always)]
  fn pixels_from(&mut self, at: PixelPosition) -> Result<&mut [PremultipliedColorU8], Never> {
    let Ok(position) = index(
      u64::from(self.real_width)
        .wrapping_mul(u64::from(at.y))
        .wrapping_add(u64::from(at.x)),
    );
    let Ok(pixels) = self.pixels_mut();

    Ok(match pixels.get_mut(position..) {
      Some(row) => row,
      None => &mut [],
    })
  }

  #[inline(always)]
  fn mask_from(&mut self, at: PixelPosition) -> Result<&mut [u8], Never> {
    let Ok(position) = index(
      u64::from(self.real_width)
        .wrapping_mul(u64::from(at.y))
        .wrapping_add(u64::from(at.x)),
    );

    Ok(match self.data.get_mut(position..) {
      Some(row) => row,
      None => &mut [],
    })
  }

  #[inline(always)]
  fn pixels8(
    &mut self,
    at: PixelPosition,
  ) -> Result<Option<&mut highp::Lanes<PremultipliedColorU8>>, Never> {
    let Ok(pixels) = self.pixels_from(at);

    Ok(pixels.first_chunk_mut())
  }

  #[inline(always)]
  fn pixels16(
    &mut self,
    at: PixelPosition,
  ) -> Result<Option<&mut lowp::Lanes<PremultipliedColorU8>>, Never> {
    let Ok(pixels) = self.pixels_from(at);

    Ok(pixels.first_chunk_mut())
  }

  #[inline(always)]
  fn mask16(&mut self, at: PixelPosition) -> Result<Option<&mut lowp::Lanes<u8>>, Never> {
    let Ok(mask) = self.mask_from(at);

    Ok(mask.first_chunk_mut())
  }
}

#[derive(Default, Debug)]
pub struct AAMaskCtx {
  pub pixels: [u8; 2],
  pub stride: u32,
  pub shift: u64,
}

impl AAMaskCtx {
  #[inline(always)]
  fn copy_at(&self, at: PixelPosition, lanes: u32) -> Result<[u8; 2], Never> {
    let offset = u64::from(self.stride)
      .wrapping_mul(u64::from(at.y))
      .wrapping_add(u64::from(at.x));
    let [first, second] = self.pixels;

    Ok(match (offset.checked_sub(self.shift), lanes) {
      (Some(0), 1) => [first, 0],
      (Some(0), 2) => [first, second],
      (Some(1), 1) => [second, 0],
      _ => [0, 0],
    })
  }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct MaskCtx<'a> {
  pub data: &'a [u8],
  pub real_width: u32,
}

impl MaskCtx<'_> {
  #[inline(always)]
  fn from_position(&self, at: PixelPosition) -> Result<&[u8], Never> {
    let Ok(position) = index(
      u64::from(self.real_width)
        .wrapping_mul(u64::from(at.y))
        .wrapping_add(u64::from(at.x)),
    );

    Ok(match self.data.get(position..) {
      Some(pixels) => pixels,
      None => &[],
    })
  }
}

pub struct Context {
  pub current_coverage: NormalizedF32,
  pub sampler: SamplerCtx,
  pub uniform_color: UniformColorCtx,
  pub evenly_spaced_2_stop_gradient: GradientCoefficients,
  pub gradient: Vec<GradientBand>,
  pub two_point_conical_gradient: TwoPointConicalGradientCtx,
  pub limit_x: TileCtx,
  pub limit_y: TileCtx,
  pub transform: Transform,
}

impl Default for Context {
  fn default() -> Self {
    Self {
      current_coverage: NormalizedF32::ZERO,
      sampler: SamplerCtx::default(),
      uniform_color: UniformColorCtx::default(),
      evenly_spaced_2_stop_gradient: GradientCoefficients::default(),
      gradient: Vec::new(),
      two_point_conical_gradient: TwoPointConicalGradientCtx::default(),
      limit_x: TileCtx::default(),
      limit_y: TileCtx::default(),
      transform: Transform::default(),
    }
  }
}

#[derive(Copy, Clone, Default, Debug)]
pub struct SamplerCtx {
  pub spread_mode: SpreadMode,
  pub horizontal: TileCtx,
  pub vertical: TileCtx,
}

#[derive(Copy, Clone, Default, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "each channel is a premultiplied color fraction in the pipeline color space"
  )
)]
pub struct UniformColorCtx {
  pub r: f32,
  pub g: f32,
  pub b: f32,
  pub a: f32,
  pub rgba: [u16; 4],
}

#[derive(Clone, Copy, Default, Debug)]
pub struct GradientColor(pub [f32; 4]);

impl From<Color> for GradientColor {
  fn from(color: Color) -> Self {
    let Ok(red) = color.red();
    let Ok(green) = color.green();
    let Ok(blue) = color.blue();
    let Ok(alpha) = color.alpha();

    Self([red, green, blue, alpha])
  }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct GradientCoefficients {
  pub factor: GradientColor,
  pub bias: GradientColor,
}

pub struct GradientBand {
  pub start: NormalizedF32,
  pub coeffs: GradientCoefficients,
}

#[derive(Copy, Clone, Default, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the stage stores dimensionless conical coefficients, or sweep scale and bias"
  )
)]
pub struct TwoPointConicalGradientCtx {
  pub mask: u32x8,
  pub p0: f32,
  pub p1: f32,
}

#[derive(Copy, Clone, Default, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "scale is the tile length in source pixels and inv_scale is its reciprocal"
  )
)]
pub struct TileCtx {
  pub scale: f32,
  pub inv_scale: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelinePrecision {
  Automatic,
  High,
}

enum StagePlan {
  Ready(StageList<Stage>),
  Overflow,
}

pub struct RasterPipelineBuilder {
  plan: StagePlan,
  precision: PipelinePrecision,
  pub ctx: Context,
}

impl RasterPipelineBuilder {
  pub fn new() -> core::result::Result<Self, console_core_never::Never> {
    Ok(RasterPipelineBuilder {
      plan: StagePlan::Ready(StageList::new()),
      precision: PipelinePrecision::Automatic,
      ctx: Context::default(),
    })
  }

  pub fn set_precision(
    &mut self,
    precision: PipelinePrecision,
  ) -> core::result::Result<(), console_core_never::Never> {
    self.precision = precision;

    Ok(())
  }

  pub fn push(&mut self, stage: Stage) -> core::result::Result<(), console_core_never::Never> {
    match &mut self.plan {
      StagePlan::Ready(stages) => match stages.try_push(stage) {
        Ok(()) => {}
        Err(_over_capacity) => {
          console_core_logging::warn!("raster pipeline exceeded its stage capacity");
          self.plan = StagePlan::Overflow;
        }
      },
      StagePlan::Overflow => {}
    }

    Ok(())
  }

  pub fn push_transform(
    &mut self,
    ts: Transform,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(is_finite) = ts.is_finite();

    match ::core::primitive::bool::then_some(
      matches!(is_finite, console_core_paths::IsFiniteAnswer::Yes)
        && !matches!(
          {
            let Ok(explicit_value) = ts.is_identity();
            explicit_value
          },
          console_core_paths::IsIdentityAnswer::Yes
        ),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.push(Stage::Transform);
        self.ctx.transform = ts;
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }

  pub fn push_uniform_color(
    &mut self,
    c: PremultipliedColor,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(r) = c.red();
    let Ok(g) = c.green();
    let Ok(b) = c.blue();
    let Ok(a) = c.alpha();
    let rgba = [
      ({
        let Ok(numeric_value) =
          console_core_number_conversion::toward_zero_u16(f64::from(r * 255.0 + 0.5));
        numeric_value
      }),
      ({
        let Ok(numeric_value) =
          console_core_number_conversion::toward_zero_u16(f64::from(g * 255.0 + 0.5));
        numeric_value
      }),
      ({
        let Ok(numeric_value) =
          console_core_number_conversion::toward_zero_u16(f64::from(b * 255.0 + 0.5));
        numeric_value
      }),
      ({
        let Ok(numeric_value) =
          console_core_number_conversion::toward_zero_u16(f64::from(a * 255.0 + 0.5));
        numeric_value
      }),
    ];

    let ctx = UniformColorCtx { r, g, b, a, rgba };

    let Ok(()) = self.push(Stage::UniformColor);
    self.ctx.uniform_color = ctx;

    Ok(())
  }

  pub fn compile(self) -> Result<RasterPipeline, Never> {
    let stages = match self.plan {
      StagePlan::Ready(stages) => stages,
      StagePlan::Overflow => StageList::new(),
    };

    match stages.is_empty().then_some(()) {
      Some(()) => {
        return Ok(RasterPipeline {
          #[cfg(feature = "simd")]
          simd: console_core_simd::Simd::SCALAR,
          kind: RasterPipelineKind::High {
            functions: StageList::new(),
            tail_functions: StageList::new(),
          },
          ctx: Context::default(),
        });
      }
      None => {}
    }

    let low = match self.precision {
      PipelinePrecision::High => None,
      PipelinePrecision::Automatic => stages
        .iter()
        .map(|stage| {
          let Ok(function) = lowp::stage_function(*stage);

          function
        })
        .collect::<Option<StageList<_>>>(),
    };

    let kind = match low {
      Some(functions) => {
        let substitutions: [(Stage, lowp::StageFn); 5] = [
          (Stage::LoadDestination, lowp::load_dst_tail),
          (Stage::Store, lowp::store_tail),
          (Stage::LoadDestinationU8, lowp::load_dst_u8_tail),
          (Stage::StoreU8, lowp::store_u8_tail),
          (Stage::SourceOverRgba, lowp::source_over_rgba_tail),
        ];
        let Ok(tail_functions) = compile_tail(&stages, &functions, substitutions);

        RasterPipelineKind::Low {
          functions,
          tail_functions,
        }
      }
      None => {
        let functions = stages
          .iter()
          .map(|stage| {
            let Ok(function) = highp::stage_function(*stage);

            function
          })
          .collect();
        let substitutions: [(Stage, highp::StageFn); 5] = [
          (Stage::LoadDestination, highp::load_dst_tail),
          (Stage::Store, highp::store_tail),
          (Stage::LoadDestinationU8, highp::load_dst_u8_tail),
          (Stage::StoreU8, highp::store_u8_tail),
          (Stage::SourceOverRgba, highp::source_over_rgba_tail),
        ];
        let Ok(tail_functions) = compile_tail(&stages, &functions, substitutions);

        RasterPipelineKind::High {
          functions,
          tail_functions,
        }
      }
    };

    #[cfg(feature = "simd")]
    let Ok(simd) = console_core_simd::Simd::detected();

    Ok(RasterPipeline {
      #[cfg(feature = "simd")]
      simd,
      kind,
      ctx: self.ctx,
    })
  }
}

fn compile_tail<F: Copy>(
  stages: &StageList<Stage>,
  functions: &StageList<F>,
  substitutions: [(Stage, F); 5],
) -> Result<StageList<F>, Never> {
  Ok(
    stages
      .iter()
      .zip(functions)
      .map(|(stage, function)| {
        match substitutions.iter().find(|(original, _)| original == stage) {
          Some((_, replacement)) => *replacement,
          None => *function,
        }
      })
      .collect(),
  )
}

pub enum RasterPipelineKind {
  High {
    functions: StageList<highp::StageFn>,
    tail_functions: StageList<highp::StageFn>,
  },
  Low {
    functions: StageList<lowp::StageFn>,
    tail_functions: StageList<lowp::StageFn>,
  },
}

pub struct RasterPipeline {
  #[cfg(feature = "simd")]
  simd: console_core_simd::Simd,
  kind: RasterPipelineKind,
  pub ctx: Context,
}

impl RasterPipeline {
  pub fn run(
    &mut self,
    rect: &ScreenIntRect,
    aa_mask_ctx: AAMaskCtx,
    mask_ctx: MaskCtx,
    pixmap_src: PixmapRef,
    pixmap_dst: &mut SubPixmapMut,
  ) -> core::result::Result<(), console_core_never::Never> {
    match self.kind {
      RasterPipelineKind::High {
        ref functions,
        ref tail_functions,
      } => {
        let Ok(()) = highp::start(
          functions.as_slice(),
          tail_functions.as_slice(),
          rect,
          aa_mask_ctx,
          mask_ctx,
          &mut self.ctx,
          pixmap_src,
          pixmap_dst,
        );
      }
      RasterPipelineKind::Low {
        ref functions,
        ref tail_functions,
      } => {
        let Ok(()) = lowp::start(
          #[cfg(feature = "simd")]
          self.simd,
          functions.as_slice(),
          tail_functions.as_slice(),
          rect,
          aa_mask_ctx,
          mask_ctx,
          &mut self.ctx,
          pixmap_dst,
        );
      }
    }

    Ok(())
  }
}

#[rustfmt::skip]
#[cfg(test)]
mod blend_tests {

    use super::*;
    use crate::{BlendMode, Color, Pixmap, PremultipliedColorU8};
    use crate::geom::IntSizeExt;
    use crate::test_support::{Required, TestFailure};

    #[cfg_attr(
      dylint_lib = "explicit080_macro_budget",
      expect(
        explicit080_macro_budget,
        reason = "more than fifty named blend fixtures share the same setup and differ in their pipeline precision and expected RGBA bytes"
      )
    )]
    macro_rules! test_blend {
        ($name:ident, $mode:expr, $precision:expr, $r:expr, $g:expr, $b:expr, $a:expr) => {
            #[test]
            fn $name() -> Result<(), TestFailure> {
                let Ok(made) = Pixmap::new(1, 1);
                let mut pixmap = made.required()?;
                let Ok(background) = Color::from_rgba8(50, 127, 150, 200);
                let Ok(()) = pixmap.fill(background);

                let source = [0, 0, 0, 0];
                let Ok(viewed) = PixmapRef::from_bytes(&source, 1, 1);
                let pixmap_src = viewed.required()?;

                let Ok(mut p) = RasterPipelineBuilder::new();
                let Ok(()) = p.set_precision($precision);
                let Ok(foreground) = Color::from_rgba8(220, 140, 75, 180);
                let Ok(premultiplied) = foreground.premultiply();
                let Ok(()) = p.push_uniform_color(premultiplied);
                let Ok(()) = p.push(Stage::LoadDestination);
                let Ok(staged) = $mode.to_stage();
                let blend = staged.required()?;
                let Ok(()) = p.push(blend);
                let Ok(()) = p.push(Stage::Store);
                let Ok(mut compiled) = p.compile();
                let Ok(size) = pixmap.size();
                let Ok(bounds) = size.at_origin();
                let rect = bounds.required()?;
                let Ok(mut target) = pixmap.as_mut();
                let Ok(mut subpixmap) = target.as_subpixmap();
                let Ok(()) = compiled.run(&rect, AAMaskCtx::default(), MaskCtx::default(), pixmap_src, &mut subpixmap);

                let Ok(painted) = pixmap.as_ref();
                assert_eq!(painted.pixel(0, 0), PremultipliedColorU8::from_rgba($r, $g, $b, $a));

                Ok(())
            }
        };
    }

    #[cfg_attr(
      dylint_lib = "explicit080_macro_budget",
      expect(
        explicit080_macro_budget,
        reason = "the named low-precision fixtures select the automatic pipeline while keeping mode and expected output visible"
      )
    )]
    macro_rules! test_blend_lowp {
        ($name:ident, $mode:expr, $r:expr, $g:expr, $b:expr, $a:expr) => (
            test_blend!{$name, $mode, PipelinePrecision::Automatic, $r, $g, $b, $a}
        )
    }

    #[cfg_attr(
      dylint_lib = "explicit080_macro_budget",
      expect(
        explicit080_macro_budget,
        reason = "the named high-precision fixtures select the float pipeline while keeping mode and expected output visible"
      )
    )]
    macro_rules! test_blend_highp {
        ($name:ident, $mode:expr, $r:expr, $g:expr, $b:expr, $a:expr) => (
            test_blend!{$name, $mode, PipelinePrecision::High, $r, $g, $b, $a}
        )
    }

    test_blend_lowp!(clear_lowp,              BlendMode::Clear,                 0,   0,   0,   0);

    test_blend_lowp!(destination_lowp,        BlendMode::Destination,          39, 100, 118, 200);
    test_blend_lowp!(source_over_lowp,        BlendMode::SourceOver,          167, 129,  88, 239);
    test_blend_lowp!(destination_over_lowp,   BlendMode::DestinationOver,      73, 122, 130, 239);
    test_blend_lowp!(source_in_lowp,          BlendMode::SourceIn,            122,  78,  42, 141);
    test_blend_lowp!(destination_in_lowp,     BlendMode::DestinationIn,        28,  71,  83, 141);
    test_blend_lowp!(source_out_lowp,         BlendMode::SourceOut,            34,  22,  12,  39);
    test_blend_lowp!(destination_out_lowp,    BlendMode::DestinationOut,       12,  30,  35,  59);
    test_blend_lowp!(source_atop_lowp,        BlendMode::SourceAtop,          133, 107,  76, 200);
    test_blend_lowp!(destination_atop_lowp,   BlendMode::DestinationAtop,      61,  92,  95, 180);
    test_blend_lowp!(xor_lowp,                BlendMode::Xor,                  45,  51,  46,  98);
    test_blend_lowp!(plus_lowp,               BlendMode::Plus,                194, 199, 171, 255);
    test_blend_lowp!(modulate_lowp,           BlendMode::Modulate,             24,  39,  25, 141);
    test_blend_lowp!(screen_lowp,             BlendMode::Screen,              170, 160, 146, 239);
    test_blend_lowp!(overlay_lowp,            BlendMode::Overlay,              92, 128, 106, 239);
    test_blend_lowp!(darken_lowp,             BlendMode::Darken,               72, 121,  88, 239);
    test_blend_lowp!(lighten_lowp,            BlendMode::Lighten,             166, 128, 129, 239);

    test_blend_lowp!(hard_light_lowp,         BlendMode::HardLight,           154, 128,  95, 239);

    test_blend_lowp!(difference_lowp,         BlendMode::Difference,          138,  57,  87, 239);
    test_blend_lowp!(exclusion_lowp,          BlendMode::Exclusion,           146, 121, 121, 239);
    test_blend_lowp!(multiply_lowp,           BlendMode::Multiply,             69,  90,  71, 238);

    test_blend_highp!(clear_highp,            BlendMode::Clear,                 0,   0,   0,   0);

    test_blend_highp!(destination_highp,      BlendMode::Destination,          39, 100, 118, 200);
    test_blend_highp!(source_over_highp,      BlendMode::SourceOver,          167, 128,  88, 239);
    test_blend_highp!(destination_over_highp, BlendMode::DestinationOver,      72, 121, 129, 239);
    test_blend_highp!(source_in_highp,        BlendMode::SourceIn,            122,  78,  42, 141);
    test_blend_highp!(destination_in_highp,   BlendMode::DestinationIn,        28,  71,  83, 141);
    test_blend_highp!(source_out_highp,       BlendMode::SourceOut,            33,  21,  11,  39);
    test_blend_highp!(destination_out_highp,  BlendMode::DestinationOut,       11,  29,  35,  59);
    test_blend_highp!(source_atop_highp,      BlendMode::SourceAtop,          133, 107,  76, 200);
    test_blend_highp!(destination_atop_highp, BlendMode::DestinationAtop,      61,  92,  95, 180);
    test_blend_highp!(xor_highp,              BlendMode::Xor,                  45,  51,  46,  98);
    test_blend_highp!(plus_highp,             BlendMode::Plus,                194, 199, 171, 255);
    test_blend_highp!(modulate_highp,         BlendMode::Modulate,             24,  39,  24, 141);
    test_blend_highp!(screen_highp,           BlendMode::Screen,              171, 160, 146, 239);
    test_blend_highp!(overlay_highp,          BlendMode::Overlay,              92, 128, 106, 239);
    test_blend_highp!(darken_highp,           BlendMode::Darken,               72, 121,  88, 239);
    test_blend_highp!(lighten_highp,          BlendMode::Lighten,             167, 128, 129, 239);
    test_blend_highp!(color_dodge_highp,      BlendMode::ColorDodge,          186, 192, 164, 239);
    test_blend_highp!(color_burn_highp,       BlendMode::ColorBurn,            54,  63,  46, 239);
    test_blend_highp!(hard_light_highp,       BlendMode::HardLight,           155, 128,  95, 239);
    test_blend_highp!(soft_light_highp,       BlendMode::SoftLight,            98, 124, 115, 239);
    test_blend_highp!(difference_highp,       BlendMode::Difference,          139,  58,  88, 239);
    test_blend_highp!(exclusion_highp,        BlendMode::Exclusion,           147, 121, 122, 239);
    test_blend_highp!(multiply_highp,         BlendMode::Multiply,             69,  89,  71, 239);
    test_blend_highp!(hue_highp,              BlendMode::Hue,                 128, 103,  74, 239);
    test_blend_highp!(saturation_highp,       BlendMode::Saturation,           59, 126, 140, 239);
    test_blend_highp!(color_highp,            BlendMode::Color,               139, 100,  60, 239);
    test_blend_highp!(luminosity_highp,       BlendMode::Luminosity,          100, 149, 157, 239);
}
