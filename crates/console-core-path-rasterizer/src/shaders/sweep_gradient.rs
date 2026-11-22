use alloc::vec::Vec;

use console_core_never::Never;
use console_core_paths::{Scalar, ScalarProximity};

use crate::{ColorSpace, GradientStop, Point, Shader, SpreadMode, Transform};

use super::gradient::{DEGENERATE_THRESHOLD, Gradient};
use crate::pipeline::{RasterPipelineBuilder, Stage};

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use console_core_paths::NoStdFloat;

#[derive(Clone, PartialEq, Debug)]
pub struct SweepGradient {
  pub(crate) base: Gradient,
  turns: SweepTurns,
}

#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "each endpoint is measured in turns around the sweep center"
  )
)]
struct SweepTurns {
  start: f32,
  end: f32,
}

impl SweepGradient {
  #[expect(
    clippy::new_ret_no_self,
    reason = "a degenerate gradient can be represented by a solid-color shader"
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "callers supply sweep endpoints in established start and end order"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the public constructor takes sweep angles in degrees"
    )
  )]
  pub fn new(
    center: Point,
    mut start_angle: f32,
    end_angle: f32,
    mut stops: Vec<GradientStop>,
    mode: SpreadMode,
    transform: Transform,
  ) -> Result<Option<Shader<'static>>, Never> {
    match (!start_angle.is_finite() || !end_angle.is_finite() || start_angle > end_angle)
      .then_some(())
    {
      Some(()) => return Ok(None),
      None => {}
    }

    let (start_color, end_color) = match stops.as_slice() {
      [] => return Ok(None),
      [stop] => return Ok(Some(Shader::SolidColor(stop.color))),
      [first, .., last] => (first.color, last.color),
    };
    let Ok(inverse) = transform.invert();

    match inverse {
      Some(_) => {}
      None => return Ok(None),
    }

    let proximity = start_angle.is_nearly_equal_within_tolerance(end_angle, DEGENERATE_THRESHOLD);

    match (proximity, mode, end_angle > DEGENERATE_THRESHOLD) {
      (ScalarProximity::OutsideTolerance, _, _) => {}
      (ScalarProximity::WithinTolerance, SpreadMode::Pad, true) => {
        let Ok(first) = GradientStop::new(0.0, start_color);
        let Ok(mid) = GradientStop::new(1.0, start_color);
        let Ok(last) = GradientStop::new(1.0, end_color);
        stops.clear();
        stops.extend([first, mid, last]);
        start_angle = 0.0;
      }
      (ScalarProximity::WithinTolerance, _, _) => return Ok(None),
    }

    let spread = match (start_angle <= 0.0 && end_angle >= 360.0).then_some(()) {
      Some(()) => SpreadMode::Pad,
      None => mode,
    };
    let turns = SweepTurns {
      start: start_angle / 360.0,
      end: end_angle / 360.0,
    };
    let Ok(origin) = Transform::from_translation(Point {
      x: -center.x,
      y: -center.y,
    });
    let Ok(gradient) = Gradient::new(stops, spread, transform, origin);

    Ok(gradient.map(|base| Shader::SweepGradient(Self { base, turns })))
  }

  pub(crate) fn is_opaque(
    &self,
  ) -> core::result::Result<crate::IsOpaqueAnswer, console_core_never::Never> {
    Ok(self.base.opacity)
  }

  pub(crate) fn push_stages(
    &self,
    cs: ColorSpace,
    p: &mut RasterPipelineBuilder,
  ) -> core::result::Result<crate::PushStagesAnswer, console_core_never::Never> {
    let decided = {
      let scale = 1.0 / (self.turns.end - self.turns.start);
      let bias = -scale * self.turns.start;
      p.ctx.two_point_conical_gradient.p0 = scale;
      p.ctx.two_point_conical_gradient.p1 = bias;

      let Ok(push_stages) = self.base.push_stages(
        p,
        cs,
        &|p_2| {
          let Ok(()) = p_2.push(Stage::XYToUnitAngle);

          match ::core::primitive::bool::then_some(
            ::core::primitive::f32::to_bits(scale) != ::core::primitive::f32::to_bits(1.0)
              && !::core::matches!(
                ::core::primitive::f32::classify(bias),
                ::core::num::FpCategory::Zero
              ),
            (),
          ) {
            ::core::option::Option::Some(()) => {
              let Ok(push) = p_2.push(Stage::ApplyConcentricScaleBias);

              push
            }
            ::core::option::Option::None => {}
          }
        },
        &|_| {},
      );

      matches!(push_stages, crate::PushStagesAnswer::Yes)
    };

    Ok(match ::core::primitive::bool::then_some(decided, ()) {
      ::core::option::Option::Some(()) => crate::PushStagesAnswer::Yes,
      ::core::option::Option::None => crate::PushStagesAnswer::No,
    })
  }
}
