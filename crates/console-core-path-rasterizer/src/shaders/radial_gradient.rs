use alloc::vec::Vec;

use console_core_circle::Circle;
use console_core_never::Never;
use console_core_paths::{Scalar, ScalarProximity, ScaleFactors};
use console_core_point::Point as Coordinates;

use crate::{ColorSpace, GradientStop, Point, Shader, SpreadMode, Transform};

use super::gradient::{DEGENERATE_THRESHOLD, Gradient};
use crate::pipeline;
use crate::pipeline::RasterPipelineBuilder;
use crate::wide::u32x8;

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use console_core_paths::NoStdFloat;

#[derive(Copy, Clone, PartialEq, Debug, Default)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "radius and focal position are normalized to the distance between gradient centers"
  )
)]
struct FocalData {
  r1: f32,
  focal_x: f32,
  is_swapped: bool,
}

impl FocalData {
  fn set(
    &mut self,
    [mut r0, mut r1]: [f32; 2],
    matrix: &mut Transform,
  ) -> core::result::Result<crate::SetAnswer, console_core_never::Never> {
    let decided = {
      self.is_swapped = false;
      self.focal_x = r0 / (r0 - r1);

      match ::core::primitive::bool::then_some(
        (self.focal_x - 1.0).is_nearly_zero()
          == console_core_paths::ScalarProximity::WithinTolerance,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let Ok(post_translate) =
            matrix.post_translate_by(::console_core_paths::Point { x: -1.0, y: 0.0 });
          let Ok(post_scale) =
            post_translate.post_scale_by(::console_core_paths::ScaleFactors { x: -1.0, y: 1.0 });
          *matrix = post_scale;
          core::mem::swap(&mut r0, &mut r1);

          self.focal_x = 0.0;
          self.is_swapped = true;
        }
        ::core::option::Option::None => {}
      }

      let Ok(point) = Point::from_xy(self.focal_x, 0.0);
      let from = [
        point,
        ({
          let Ok(explicit_value) = Point::from_xy(1.0, 0.0);
          explicit_value
        }),
      ];
      let to = [
        ({
          let Ok(explicit_value) = Point::from_xy(0.0, 0.0);
          explicit_value
        }),
        ({
          let Ok(explicit_value) = Point::from_xy(1.0, 0.0);
          explicit_value
        }),
      ];

      let Ok(answer) = ts_from_poly_to_poly(
        match from {
          [_explicit_element, ..] => _explicit_element,
        },
        match from {
          [_, _explicit_element, ..] => _explicit_element,
        },
        match to {
          [_explicit_element, ..] => _explicit_element,
        },
        match to {
          [_, _explicit_element, ..] => _explicit_element,
        },
      );

      let focal_matrix = match answer {
        Some(m) => m,
        None => {
          return Ok(match ::core::primitive::bool::then_some(false, ()) {
            ::core::option::Option::Some(()) => crate::SetAnswer::Yes,
            ::core::option::Option::None => crate::SetAnswer::No,
          });
        }
      };

      let Ok(post_concat) = matrix.post_concat(focal_matrix);
      *matrix = post_concat;
      self.r1 = r1 / (1.0 - self.focal_x).abs();

      let Ok(is_focal_on_circle) = self.is_focal_on_circle();

      match is_focal_on_circle {
        crate::IsFocalOnCircleAnswer::Yes => {
          let Ok(post_scale) =
            matrix.post_scale_by(::console_core_paths::ScaleFactors { x: 0.5, y: 0.5 });
          *matrix = post_scale;
        }
        crate::IsFocalOnCircleAnswer::No => {
          let Ok(post_scale) = matrix.post_scale_by(::console_core_paths::ScaleFactors {
            x: self.r1 / (self.r1 * self.r1 - 1.0),
            y: 1.0 / (self.r1 * self.r1 - 1.0).abs().sqrt(),
          });
          *matrix = post_scale;
        }
      }

      let Ok(post_scale) = matrix.post_scale_by(::console_core_paths::ScaleFactors {
        x: (1.0 - self.focal_x).abs(),
        y: (1.0 - self.focal_x).abs(),
      });
      *matrix = post_scale;

      true
    };

    Ok(match ::core::primitive::bool::then_some(decided, ()) {
      ::core::option::Option::Some(()) => crate::SetAnswer::Yes,
      ::core::option::Option::None => crate::SetAnswer::No,
    })
  }

  fn is_focal_on_circle(
    &self,
  ) -> core::result::Result<crate::IsFocalOnCircleAnswer, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        (1.0 - self.r1).is_nearly_zero() == console_core_paths::ScalarProximity::WithinTolerance,
        (),
      ) {
        ::core::option::Option::Some(()) => crate::IsFocalOnCircleAnswer::Yes,
        ::core::option::Option::None => crate::IsFocalOnCircleAnswer::No,
      },
    )
  }

  fn is_well_behaved(
    &self,
  ) -> core::result::Result<crate::IsWellBehavedAnswer, console_core_never::Never> {
    let decided = {
      let Ok(is_focal_on_circle) = self.is_focal_on_circle();

      !matches!(is_focal_on_circle, crate::IsFocalOnCircleAnswer::Yes) && self.r1 > 1.0
    };

    Ok(match ::core::primitive::bool::then_some(decided, ()) {
      ::core::option::Option::Some(()) => crate::IsWellBehavedAnswer::Yes,
      ::core::option::Option::None => crate::IsWellBehavedAnswer::No,
    })
  }

  fn is_natively_focal(
    &self,
  ) -> core::result::Result<crate::IsNativelyFocalAnswer, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        self.focal_x.is_nearly_zero() == console_core_paths::ScalarProximity::WithinTolerance,
        (),
      ) {
        ::core::option::Option::Some(()) => crate::IsNativelyFocalAnswer::Yes,
        ::core::option::Option::None => crate::IsNativelyFocalAnswer::No,
      },
    )
  }
}

#[derive(Clone, PartialEq, Debug)]
enum GradientType {
  Radial { radius1: f32, radius2: f32 },
  Band { scaled_r0: f32 },
  Focal(FocalData),
}

#[derive(Clone, PartialEq, Debug)]
pub struct RadialGradient {
  pub(crate) base: Gradient,
  gradient_type: GradientType,
}

impl RadialGradient {
  #[expect(
    clippy::new_ret_no_self,
    reason = "a degenerate gradient can be represented by a solid-color shader"
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the public constructor takes a radius after each center in start and end order"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "each public radius argument is measured in path coordinate units"
    )
  )]
  pub fn new(
    start_point: Point,
    start_radius: f32,
    end_point: Point,
    end_radius: f32,
    mut stops: Vec<GradientStop>,
    mode: SpreadMode,
    transform: Transform,
  ) -> Result<Option<Shader<'static>>, Never> {
    match (start_radius < 0.0 || end_radius < 0.0).then_some(()) {
      Some(()) => return Ok(None),
      None => {}
    }

    let (first_color, last_color) = match stops.as_slice() {
      [] => return Ok(None),
      [stop] => return Ok(Some(Shader::SolidColor(stop.color))),
      [first, .., last] => (first.color, last.color),
    };
    let Ok(inverse) = transform.invert();

    match inverse {
      Some(_) => {}
      None => return Ok(None),
    }

    let Ok(length) = (start_point - end_point).length();

    match length.is_finite().then_some(()) {
      Some(()) => {}
      None => return Ok(None),
    }

    let centers = length.is_nearly_zero_within_tolerance(DEGENERATE_THRESHOLD);
    let radii = start_radius.is_nearly_equal_within_tolerance(end_radius, DEGENERATE_THRESHOLD);
    let initial_radius = start_radius.is_nearly_zero_within_tolerance(DEGENERATE_THRESHOLD);

    match (centers, radii, initial_radius) {
      (ScalarProximity::WithinTolerance, ScalarProximity::WithinTolerance, _) => {
        match (mode, end_radius > DEGENERATE_THRESHOLD) {
          (SpreadMode::Pad, true) => {}
          _ => return Ok(None),
        }

        let Ok(first) = GradientStop::new(0.0, first_color);
        let Ok(mid) = GradientStop::new(1.0, first_color);
        let Ok(last) = GradientStop::new(1.0, last_color);
        stops.clear();
        stops.extend([first, mid, last]);

        let Ok(circle) = Circle::new(Coordinates::from(start_point), end_radius);
        Self::new_radial_unchecked(circle, stops, mode, transform)
      }
      (ScalarProximity::WithinTolerance, _, ScalarProximity::WithinTolerance) => {
        let Ok(circle) = Circle::new(Coordinates::from(start_point), end_radius);
        Self::new_radial_unchecked(circle, stops, mode, transform)
      }
      _ => {
        let Ok(first) = Circle::new(Coordinates::from(start_point), start_radius);
        let Ok(last) = Circle::new(Coordinates::from(end_point), end_radius);
        create([first, last], stops, mode, transform)
      }
    }
  }

  fn new_radial_unchecked(
    Circle { center, radius }: Circle<f32>,
    stops: Vec<GradientStop>,
    mode: SpreadMode,
    transform: Transform,
  ) -> Result<Option<Shader<'static>>, Never> {
    let inv = radius.invert();
    let Ok(centered) = Transform::from_translation(Point {
      x: -center.x,
      y: -center.y,
    });
    let Ok(unit) = centered.post_scale_by(ScaleFactors { x: inv, y: inv });
    let Ok(gradient) = Gradient::new(stops, mode, transform, unit);

    Ok(gradient.map(|base| {
      Shader::RadialGradient(Self {
        base,
        gradient_type: GradientType::Radial {
          radius1: 0.0,
          radius2: radius,
        },
      })
    }))
  }

  pub(crate) fn push_stages(
    &self,
    cs: ColorSpace,
    p: &mut RasterPipelineBuilder,
  ) -> core::result::Result<crate::PushStagesAnswer, console_core_never::Never> {
    let decided = {
      let (p0, p1) = match self.gradient_type {
        GradientType::Radial { radius1, radius2 } => {
          match ::core::primitive::f32::classify(radius1) {
            ::core::num::FpCategory::Zero => (1.0, 0.0),
            ::core::num::FpCategory::Nan
            | ::core::num::FpCategory::Infinite
            | ::core::num::FpCategory::Subnormal
            | ::core::num::FpCategory::Normal => {
              let d_radius = radius2 - radius1;

              let p0 = radius1.max(radius2) / d_radius;
              let p1 = -radius1 / d_radius;
              (p0, p1)
            }
          }
        }
        GradientType::Band { scaled_r0 } => (scaled_r0 * scaled_r0, 0.0),
        GradientType::Focal(fd) => (1.0 / fd.r1, fd.focal_x),
      };

      p.ctx.two_point_conical_gradient = pipeline::TwoPointConicalGradientCtx {
        mask: u32x8::default(),
        p0,
        p1,
      };

      let Ok(push_stages) = self.base.push_stages(
        p,
        cs,
        &|p_2| match self.gradient_type {
          GradientType::Radial { .. } => {
            let Ok(()) = p_2.push(pipeline::Stage::XYToRadius);

            match ::core::primitive::bool::then_some((p0, p1) != (1.0, 0.0), ()) {
              ::core::option::Option::Some(()) => {
                let Ok(()) = p_2.push(pipeline::Stage::ApplyConcentricScaleBias);
              }
              ::core::option::Option::None => {}
            }
          }
          GradientType::Band { .. } => {
            let Ok(()) = p_2.push(pipeline::Stage::XYTo2PtConicalStrip);
            let Ok(()) = p_2.push(pipeline::Stage::Mask2PtConicalNan);
          }
          GradientType::Focal(fd) => {
            let Ok(is_focal_on_circle) = fd.is_focal_on_circle();

            match is_focal_on_circle {
              crate::IsFocalOnCircleAnswer::Yes => {
                let Ok(()) = p_2.push(pipeline::Stage::XYTo2PtConicalFocalOnCircle);
              }
              crate::IsFocalOnCircleAnswer::No => {
                let Ok(is_well_behaved) = fd.is_well_behaved();

                match is_well_behaved {
                  crate::IsWellBehavedAnswer::Yes => {
                    let Ok(()) = p_2.push(pipeline::Stage::XYTo2PtConicalWellBehaved);
                  }
                  crate::IsWellBehavedAnswer::No => match ::core::primitive::bool::then_some(
                    fd.is_swapped || (1.0 - fd.focal_x) < 0.0,
                    (),
                  ) {
                    ::core::option::Option::Some(()) => {
                      let Ok(()) = p_2.push(pipeline::Stage::XYTo2PtConicalSmaller);
                    }
                    ::core::option::Option::None => {
                      let Ok(()) = p_2.push(pipeline::Stage::XYTo2PtConicalGreater);
                    }
                  },
                }
              }
            }

            let Ok(is_well_behaved) = fd.is_well_behaved();

            match ::core::primitive::bool::then_some(
              !matches!(is_well_behaved, crate::IsWellBehavedAnswer::Yes),
              (),
            ) {
              ::core::option::Option::Some(()) => {
                let Ok(()) = p_2.push(pipeline::Stage::Mask2PtConicalDegenerates);
              }
              ::core::option::Option::None => {}
            }

            match ::core::primitive::bool::then_some((1.0 - fd.focal_x) < 0.0, ()) {
              ::core::option::Option::Some(()) => {
                let Ok(()) = p_2.push(pipeline::Stage::NegateX);
              }
              ::core::option::Option::None => {}
            }

            let Ok(is_natively_focal) = fd.is_natively_focal();

            match ::core::primitive::bool::then_some(
              !matches!(is_natively_focal, crate::IsNativelyFocalAnswer::Yes),
              (),
            ) {
              ::core::option::Option::Some(()) => {
                let Ok(()) = p_2.push(pipeline::Stage::Alter2PtConicalCompensateFocal);
              }
              ::core::option::Option::None => {}
            }

            match ::core::primitive::bool::then_some(fd.is_swapped, ()) {
              ::core::option::Option::Some(()) => {
                let Ok(()) = p_2.push(pipeline::Stage::Alter2PtConicalUnswap);
              }
              ::core::option::Option::None => {}
            }
          }
        },
        &|p_3| match self.gradient_type {
          GradientType::Band { .. } => {
            let Ok(push) = p_3.push(pipeline::Stage::ApplyVectorMask);

            push
          }
          GradientType::Focal(fd) => {
            let Ok(behavior) = fd.is_well_behaved();

            match behavior {
              crate::IsWellBehavedAnswer::Yes => {}
              crate::IsWellBehavedAnswer::No => {
                let Ok(()) = p_3.push(pipeline::Stage::ApplyVectorMask);
              }
            }
          }
          GradientType::Radial { .. } => {}
        },
      );

      matches!(push_stages, crate::PushStagesAnswer::Yes)
    };

    Ok(match ::core::primitive::bool::then_some(decided, ()) {
      ::core::option::Option::Some(()) => crate::PushStagesAnswer::Yes,
      ::core::option::Option::None => crate::PushStagesAnswer::No,
    })
  }
}

fn create(
  [first, last]: [Circle<f32>; 2],
  stops: Vec<GradientStop>,
  mode: SpreadMode,
  transform: Transform,
) -> Result<Option<Shader<'static>>, Never> {
  let c0 = Point::from(first.center);
  let c1 = Point::from(last.center);
  let r0 = first.radius;
  let r1 = last.radius;
  let Ok(distance) = (c0 - c1).length();

  let (gradient_type, matrix) = match distance.is_nearly_zero() {
    ScalarProximity::WithinTolerance => {
      let largest = r0.max(r1);

      match (largest.is_nearly_zero(), r0.is_nearly_equal(r1)) {
        (ScalarProximity::WithinTolerance, _) | (_, ScalarProximity::WithinTolerance) => {
          return Ok(None);
        }
        (ScalarProximity::OutsideTolerance, ScalarProximity::OutsideTolerance) => {}
      }

      let scale = 1.0 / largest;
      let Ok(centered) = Transform::from_translation(Point { x: -c1.x, y: -c1.y });
      let Ok(scaled) = centered.post_scale_by(ScaleFactors { x: scale, y: scale });

      (
        GradientType::Radial {
          radius1: r0,
          radius2: r1,
        },
        scaled,
      )
    }
    ScalarProximity::OutsideTolerance => {
      let Ok(unit) = map_to_unit_x(c0, c1);
      let mut matrix = match unit {
        Some(value) => value,
        None => return Ok(None),
      };

      let gradient_type = match (r0 - r1).is_nearly_zero() {
        ScalarProximity::WithinTolerance => GradientType::Band {
          scaled_r0: r0 / distance,
        },
        ScalarProximity::OutsideTolerance => {
          let mut focal = FocalData::default();
          let Ok(answer) = focal.set([r0 / distance, r1 / distance], &mut matrix);

          match answer {
            crate::SetAnswer::Yes => GradientType::Focal(focal),
            crate::SetAnswer::No => return Ok(None),
          }
        }
      };

      (gradient_type, matrix)
    }
  };
  let Ok(gradient) = Gradient::new(stops, mode, transform, matrix);

  Ok(gradient.map(|base| {
    Shader::RadialGradient(RadialGradient {
      base,
      gradient_type,
    })
  }))
}

fn map_to_unit_x(
  origin: Point,
  x_is_one: Point,
) -> core::result::Result<Option<Transform>, console_core_never::Never> {
  let Ok(point) = Point::from_xy(0.0, 0.0);

  Ok({
    let Ok(explicit_value) = ts_from_poly_to_poly(origin, x_is_one, point, {
      let Ok(explicit_value) = Point::from_xy(1.0, 0.0);
      explicit_value
    });
    explicit_value
  })
}

fn ts_from_poly_to_poly(
  src1: Point,
  src2: Point,
  dst1: Point,
  dst2: Point,
) -> core::result::Result<Option<Transform>, console_core_never::Never> {
  let Ok(tmp) = from_poly2(src1, src2);
  let Ok(invert) = tmp.invert();

  let res = match invert {
    Some(value) => value,
    None => return Ok(None),
  };
  let Ok(tmp_2) = from_poly2(dst1, dst2);

  let Ok(pre_concat) = tmp_2.pre_concat(res);

  Ok(Some(pre_concat))
}

fn from_poly2(
  p0: Point,
  p1: Point,
) -> core::result::Result<Transform, console_core_never::Never> {
  let Ok(transform) = Transform::from_row(
    p1.y - p0.y,
    p0.x - p1.x,
    p1.x - p0.x,
    p1.y - p0.y,
    p0.x,
    p0.y,
  );

  Ok(transform)
}
