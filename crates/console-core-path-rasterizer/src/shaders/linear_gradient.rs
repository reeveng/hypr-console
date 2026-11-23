use alloc::vec::Vec;
use core::num::FpCategory;

use console_core_never::Never;
use console_core_paths::{Scalar, ScalarProximity, ScaleFactors, StrictNum};

use crate::pipeline::RasterPipelineBuilder;
use crate::wide::f32x4;
use crate::{Color, ColorSpace, GradientStop, Point, Shader, SpreadMode, Transform};

use super::gradient::{DEGENERATE_THRESHOLD, Gradient};

#[derive(Clone, PartialEq, Debug)]
pub struct LinearGradient {
  pub(crate) base: Gradient,
}

impl LinearGradient {
  #[expect(
    clippy::new_ret_no_self,
    reason = "a degenerate gradient can be represented by a solid-color shader"
  )]
  pub fn new(
    start: Point,
    end: Point,
    stops: Vec<GradientStop>,
    mode: SpreadMode,
    transform: Transform,
  ) -> Result<Option<Shader<'static>>, Never> {
    let last = match stops.as_slice() {
      [] => return Ok(None),
      [stop] => return Ok(Some(Shader::SolidColor(stop.color))),
      [_, .., stop] => stop,
    };
    let mut direction = end - start;
    let Ok(length) = direction.length();

    match length.is_finite().then_some(()) {
      Some(()) => {}
      None => return Ok(None),
    }

    match length.is_nearly_zero_within_tolerance(DEGENERATE_THRESHOLD) {
      ScalarProximity::WithinTolerance => {
        let Ok(color) = match mode {
          SpreadMode::Pad => Ok(Some(last.color)),
          SpreadMode::Reflect | SpreadMode::Repeat => average_gradient_color(&stops),
        };

        return Ok(color.map(Shader::SolidColor));
      }
      ScalarProximity::OutsideTolerance => {}
    }

    let Ok(inverse) = transform.invert();

    match inverse {
      Some(_) => {}
      None => return Ok(None),
    }

    let reciprocal = match length.classify() {
      FpCategory::Zero => 0.0,
      FpCategory::Nan | FpCategory::Infinite | FpCategory::Subnormal | FpCategory::Normal => {
        length.invert()
      }
    };
    let Ok(()) = direction.scale(reciprocal);
    let sin = -direction.y;
    let cos = direction.x;
    let inverse_cos = 1.0 - cos;
    let Ok(rotation) = Transform::from_row(
      cos,
      sin,
      -sin,
      cos,
      sin * start.y + inverse_cos * start.x,
      -sin * start.x + inverse_cos * start.y,
    );
    let Ok(translated) = rotation.post_translate_by(Point {
      x: -start.x,
      y: -start.y,
    });
    let Ok(unit) = translated.post_scale_by(ScaleFactors {
      x: reciprocal,
      y: reciprocal,
    });
    let Ok(gradient) = Gradient::new(stops, mode, transform, unit);

    Ok(gradient.map(|base| Shader::LinearGradient(Self { base })))
  }

  pub(crate) fn is_opaque(&self) -> Result<crate::IsOpaqueAnswer, Never> {
    Ok(self.base.opacity)
  }

  pub(crate) fn push_stages(
    &self,
    cs: ColorSpace,
    pipeline: &mut RasterPipelineBuilder,
  ) -> Result<crate::PushStagesAnswer, Never> {
    self.base.push_stages(pipeline, cs, &|_| {}, &|_| {})
  }
}

fn load_color(color: Color) -> Result<f32x4, Never> {
  let Ok(red) = color.red();
  let Ok(green) = color.green();
  let Ok(blue) = color.blue();
  let Ok(alpha) = color.alpha();

  Ok(f32x4::from([red, green, blue, alpha]))
}

fn average_gradient_color(points: &[GradientStop]) -> Result<Option<Color>, Never> {
  let (first, last) = match points {
    [] => return Ok(None),
    [stop] => return Ok(Some(stop.color)),
    [start, .., end] => (start, end),
  };
  let Ok(mut blend) = f32x4::splat(0.0);
  let Ok(half) = f32x4::splat(0.5);

  for [start, end] in points.array_windows::<2>() {
    let Ok(start_color) = load_color(start.color);
    let Ok(end_color) = load_color(end.color);
    let Ok(width) = f32x4::splat(end.position.get() - start.position.get());
    blend += half * width * (end_color + start_color);
  }

  match (first.position.get() > 0.0).then_some(()) {
    Some(()) => {
      let Ok(color) = load_color(first.color);
      let Ok(coverage) = f32x4::splat(first.position.get());
      blend += coverage * color;
    }
    None => {}
  }

  match (last.position.get() < 1.0).then_some(()) {
    Some(()) => {
      let Ok(color) = load_color(last.color);
      let Ok(one) = f32x4::splat(1.0);
      let Ok(coverage) = f32x4::splat(last.position.get());
      blend += (one - coverage) * color;
    }
    None => {}
  }

  let [red, green, blue, alpha]: [f32; 4] =
    <_ as ::core::convert::Into<[::core::primitive::f32; 4]>>::into(blend);

  Color::from_rgba(red, green, blue, alpha)
}
