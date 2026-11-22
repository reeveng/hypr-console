mod gradient;
mod linear_gradient;
mod pattern;
mod radial_gradient;
mod sweep_gradient;

use console_core_paths::StrictNum;
use console_core_paths::{NormalizedF32, Scalar};

pub use gradient::GradientStop;
pub use linear_gradient::LinearGradient;
pub use pattern::{FilterQuality, Pattern, PixmapPaint};
pub use radial_gradient::RadialGradient;
pub use sweep_gradient::SweepGradient;

use crate::{Color, ColorSpace, Transform};

use crate::pipeline::RasterPipelineBuilder;

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub enum SpreadMode {
  #[default]
  Pad,

  Reflect,

  Repeat,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Shader<'a> {
  SolidColor(Color),

  LinearGradient(LinearGradient),

  RadialGradient(RadialGradient),

  SweepGradient(SweepGradient),

  Pattern(Pattern<'a>),
}

impl Shader<'_> {
  pub fn is_opaque(
    &self,
  ) -> core::result::Result<crate::IsOpaqueAnswer, console_core_never::Never> {
    let decided = {
      match self {
        Shader::SolidColor(c) => {
          let Ok(is_opaque) = c.is_opaque();

          matches!(is_opaque, crate::IsOpaqueAnswer::Yes)
        }
        Shader::LinearGradient(g) => {
          let Ok(is_opaque) = g.is_opaque();

          matches!(is_opaque, crate::IsOpaqueAnswer::Yes)
        }

        Shader::RadialGradient(_) => false,
        Shader::SweepGradient(g) => {
          let Ok(is_opaque) = g.is_opaque();

          matches!(is_opaque, crate::IsOpaqueAnswer::Yes)
        }
        Shader::Pattern(_) => false,
      }
    };

    Ok(match ::core::primitive::bool::then_some(decided, ()) {
      ::core::option::Option::Some(()) => crate::IsOpaqueAnswer::Yes,
      ::core::option::Option::None => crate::IsOpaqueAnswer::No,
    })
  }

  #[must_use]
  pub(crate) fn push_stages(
    &self,
    cs: ColorSpace,
    p: &mut RasterPipelineBuilder,
  ) -> core::result::Result<crate::PushStagesAnswer, console_core_never::Never> {
    let decided = {
      match self {
        Shader::SolidColor(color) => {
          let Ok(expand_color) = cs.expand_color(*color);
          let Ok(color_2) = expand_color.premultiply();
          let Ok(()) = p.push_uniform_color(color_2);
          true
        }
        Shader::LinearGradient(g) => {
          let Ok(push_stages) = g.push_stages(cs, p);

          matches!(push_stages, crate::PushStagesAnswer::Yes)
        }
        Shader::RadialGradient(g) => {
          let Ok(push_stages) = g.push_stages(cs, p);

          matches!(push_stages, crate::PushStagesAnswer::Yes)
        }
        Shader::SweepGradient(g) => {
          let Ok(push_stages) = g.push_stages(cs, p);

          matches!(push_stages, crate::PushStagesAnswer::Yes)
        }
        Shader::Pattern(patt) => {
          let Ok(push_stages) = patt.push_stages(cs, p);

          matches!(push_stages, crate::PushStagesAnswer::Yes)
        }
      }
    };

    Ok(match ::core::primitive::bool::then_some(decided, ()) {
      ::core::option::Option::Some(()) => crate::PushStagesAnswer::Yes,
      ::core::option::Option::None => crate::PushStagesAnswer::No,
    })
  }

  pub fn transform(
    &mut self,
    ts: Transform,
  ) -> core::result::Result<(), console_core_never::Never> {
    match self {
      Shader::SolidColor(_) => {}
      Shader::LinearGradient(g) => {
        let Ok(post_concat) = g.base.transform.post_concat(ts);
        g.base.transform = post_concat;
      }
      Shader::RadialGradient(g) => {
        let Ok(post_concat) = g.base.transform.post_concat(ts);
        g.base.transform = post_concat;
      }
      Shader::SweepGradient(g) => {
        let Ok(post_concat) = g.base.transform.post_concat(ts);
        g.base.transform = post_concat;
      }
      Shader::Pattern(p) => {
        let Ok(post_concat) = p.transform.post_concat(ts);
        p.transform = post_concat;
      }
    }

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the public opacity multiplier is a normalized alpha fraction"
    )
  )]
  pub fn apply_opacity(
    &mut self,
    opacity: f32,
  ) -> core::result::Result<(), console_core_never::Never> {
    match self {
      Shader::SolidColor(c) => {
        let Ok(()) = c.apply_opacity(opacity);
      }
      Shader::LinearGradient(g) => {
        let Ok(()) = g.base.apply_opacity(opacity);
      }
      Shader::RadialGradient(g) => {
        let Ok(()) = g.base.apply_opacity(opacity);
      }
      Shader::SweepGradient(g) => {
        let Ok(()) = g.base.apply_opacity(opacity);
      }
      Shader::Pattern(p) => {
        let Ok(product) = NormalizedF32::nearest(p.opacity.get() * opacity.bound(0.0, 1.0));
        p.opacity = product;
      }
    }

    Ok(())
  }
}
