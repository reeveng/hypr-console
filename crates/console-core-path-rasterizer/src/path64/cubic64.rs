use console_core_fixed_capacity_vectors::FixedVec;
use console_core_never::Never;
use console_core_paths::ScalarProximity;
use core::cmp::Ordering;
use core::f64::consts::PI;
use core::num::FpCategory;
use kurbo::{CubicBez, bezier};

use super::point64::{AxisIntercept, Point64, SearchAxis};
use super::quad64::{self, Parameters, RealRoot, RealRoots};
use super::{BoundsCheck, CubeRootArgument, Parameter, Scalar64, cube_root};

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use console_core_paths::NoStdFloat;

pub struct Cubic64 {
  points: [Point64; 4],
}

impl Cubic64 {
  pub fn new(points: [Point64; 4]) -> Result<Self, Never> {
    Ok(Self { points })
  }

  pub fn axis_coordinates(&self, axis: SearchAxis) -> Result<[f64; 4], Never> {
    Ok(self.points.map(|point| {
      let Ok(value) = point.axis_coord(axis);

      value
    }))
  }

  pub fn point_at(&self, Parameter(t): Parameter) -> Result<Point64, Never> {
    let [start, first, second, end] = self.points;

    match (t.classify(), t.to_bits() == 1.0_f64.to_bits()) {
      (FpCategory::Zero, _) => return Ok(start),
      (_, true) => return Ok(end),
      _ => {}
    }

    let curve = CubicBez::new(
      (start.x, start.y),
      (first.x, first.y),
      (second.x, second.y),
      (end.x, end.y),
    );
    let point = bezier::evaluate_cubic_bernstein(curve, t);
    Ok(Point64 {
      x: point.x,
      y: point.y,
    })
  }

  pub fn search_roots(&self, intercept: AxisIntercept) -> Result<Parameters, Never> {
    let Ok(axis) = self.axis_coordinates(intercept.axis);
    let Ok(extrema) =
      quad64::roots_valid_t(bezier::cubic_derivative_coefficients(axis, 2.0, 3.0));
    let Ok(inflections) = self.inflections();
    let mut samples = FixedVec::<Parameter, 6>::new();
    samples.extend(extrema);
    samples.extend(inflections);
    samples.push(Parameter(0.0));
    samples.sort_by(|left, right| match left.0.partial_cmp(&right.0) {
      Some(order) => order,
      None => Ordering::Equal,
    });
    samples.push(Parameter(1.0));

    let mut roots = Parameters::new();

    for &[min, max] in samples.array_windows::<2>() {
      match (min.0.partial_cmp(&max.0) == Some(Ordering::Equal)).then_some(()) {
        Some(()) => continue,
        None => {}
      }

      let Ok(candidate) = self.binary_search([min, max], intercept);

      match candidate {
        Some(root) => match roots.try_push(root) {
          Ok(()) => {}
          Err(_too_many_roots) => return Ok(Parameters::new()),
        },
        None => {}
      }
    }

    Ok(roots)
  }

  fn inflections(&self) -> Result<Parameters, Never> {
    let Ok(x) = self.axis_coordinates(SearchAxis::X);
    let Ok(y) = self.axis_coordinates(SearchAxis::Y);
    quad64::roots_valid_t(bezier::cubic_inflection_coefficients(x, y, 2.0, 3.0))
  }

  #[doc = "The parameter step is halved each iteration until it reaches f64 precision."]
  fn binary_search(
    &self,
    [Parameter(min), Parameter(max)]: [Parameter; 2],
    intercept: AxisIntercept,
  ) -> Result<Option<Parameter>, Never> {
    let mut t = (min + max) / 2.0;
    let mut step = (t - min) / 2.0;
    let Ok(mut point) = self.point_at(Parameter(t));
    let Ok(position) = point.axis_coord(intercept.axis);
    let mut distance = position - intercept.position;

    for _ in 0..1076 {
      let previous = min.max(t - step);
      let Ok(earlier) = self.point_at(Parameter(previous));

      let Ok(proximity) = earlier.near(point);

      match proximity {
        ScalarProximity::WithinTolerance => return Ok(None),
        ScalarProximity::OutsideTolerance => {}
      }

      let Ok(earlier_position) = earlier.axis_coord(intercept.axis);
      let earlier_distance = earlier_position - intercept.position;
      let prior_step = step;
      step /= 2.0;
      let try_earlier = match (distance > 0.0).then_some(()) {
        Some(()) => distance > earlier_distance,
        None => distance < earlier_distance,
      };
      let Ok(next) = match try_earlier.then_some(()) {
        Some(()) => Ok(SearchMove::Use(Parameter(previous))),
        None => self.try_later(
          [Parameter(t + prior_step), Parameter(max)],
          point,
          intercept,
          distance,
        ),
      };

      match next {
        SearchMove::Use(Parameter(value)) => t = value,
        SearchMove::ReduceStep => continue,
        SearchMove::Unreachable => return Ok(None),
      }

      let Ok(sample) = self.point_at(Parameter(t));
      point = sample;
      let Ok(current) = point.axis_coord(intercept.axis);
      distance = current - intercept.position;

      match current.approximately_equal(intercept.position) {
        ScalarProximity::WithinTolerance => return Ok(Some(Parameter(t))),
        ScalarProximity::OutsideTolerance => {}
      }
    }

    Ok(None)
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "distance is the signed difference in path coordinate units"
    )
  )]
  fn try_later(
    &self,
    [next, max]: [Parameter; 2],
    current: Point64,
    intercept: AxisIntercept,
    distance: f64,
  ) -> Result<SearchMove, Never> {
    match (next.0 > max.0).then_some(()) {
      Some(()) => return Ok(SearchMove::Unreachable),
      None => {}
    }

    let Ok(later) = self.point_at(next);

    let Ok(proximity) = later.near(current);

    match proximity {
      ScalarProximity::WithinTolerance => return Ok(SearchMove::Unreachable),
      ScalarProximity::OutsideTolerance => {}
    }

    let Ok(position) = later.axis_coord(intercept.axis);
    let later_distance = position - intercept.position;
    let no_improvement = match (distance > 0.0).then_some(()) {
      Some(()) => distance <= later_distance,
      None => distance >= later_distance,
    };

    Ok(match no_improvement.then_some(()) {
      Some(()) => SearchMove::ReduceStep,
      None => SearchMove::Use(next),
    })
  }

  pub fn chop_at(&self, parameter: Parameter) -> Result<[Point64; 7], Never> {
    let Ok(x) = self.axis_coordinates(SearchAxis::X);
    let Ok(y) = self.axis_coordinates(SearchAxis::Y);
    let Ok(columns) = split_coordinates(x, parameter);
    let Ok(rows) = split_coordinates(y, parameter);
    let mut points = [Point64::default(); 7];

    for (point, (column, row)) in points.iter_mut().zip(columns.into_iter().zip(rows)) {
      *point = Point64 { x: column, y: row };
    }

    Ok(points)
  }
}

enum SearchMove {
  Use(Parameter),
  ReduceStep,
  Unreachable,
}

fn split_coordinates(points: [f64; 4], Parameter(t): Parameter) -> Result<[f64; 7], Never> {
  Ok(match (t.to_bits() == 0.5_f64.to_bits()).then_some(()) {
    Some(()) => bezier::split_cubic_in_half(points),
    None => bezier::split_cubic(points, |left, right| left + (right - left) * t),
  })
}

pub(super) fn unit_roots(coefficients: [f64; 4]) -> Result<Parameters, Never> {
  let Ok(candidates) = roots_real(coefficients);
  let Ok(mut roots) = quad64::unit_parameters(&candidates);

  for &RealRoot(value) in &candidates {
    match (
      value.approximately_one_or_less() != BoundsCheck::Within
        && value.between([1.0, 1.00005]) == BoundsCheck::Within,
      value.approximately_zero_or_more() != BoundsCheck::Within
        && value.between([-0.00005, 0.0]) == BoundsCheck::Within,
    ) {
      (true, _) => {
        let Ok(()) = quad64::add_parameter(&mut roots, Parameter(1.0));
      }
      (false, true) => {
        let Ok(()) = quad64::add_parameter(&mut roots, Parameter(0.0));
      }
      (false, false) => {}
    }
  }

  Ok(roots)
}

fn roots_real([a, b, c, d]: [f64; 4]) -> Result<RealRoots, Never> {
  let nearly_quadratic = a.approximately_zero() == ScalarProximity::WithinTolerance
    && a.approximately_zero_when_compared_to(b) == ScalarProximity::WithinTolerance
    && a.approximately_zero_when_compared_to(c) == ScalarProximity::WithinTolerance
    && a.approximately_zero_when_compared_to(d) == ScalarProximity::WithinTolerance;

  match nearly_quadratic.then_some(()) {
    Some(()) => return quad64::roots_real([b, c, d]),
    None => {}
  }

  let zero_root = d.approximately_zero_when_compared_to(a) == ScalarProximity::WithinTolerance
    && d.approximately_zero_when_compared_to(b) == ScalarProximity::WithinTolerance
    && d.approximately_zero_when_compared_to(c) == ScalarProximity::WithinTolerance;

  match zero_root.then_some(()) {
    Some(()) => {
      let Ok(mut roots) = quad64::roots_real([a, b, c]);

      match roots
        .iter()
        .any(|root| root.0.approximately_zero() == ScalarProximity::WithinTolerance)
        .then_some(())
      {
        Some(()) => {}
        None => roots.push(RealRoot(0.0)),
      }

      return Ok(roots);
    }
    None => {}
  }

  match (a + b + c + d).approximately_zero() {
    ScalarProximity::WithinTolerance => {
      let Ok(mut roots) = quad64::roots_real([a, a + b, -d]);
      let Ok(()) = push_distinct(&mut roots, RealRoot(1.0));

      Ok(roots)
    }
    ScalarProximity::OutsideTolerance => {
      let inv_a = 1.0 / a;

      solve_normalized([b * inv_a, c * inv_a, d * inv_a])
    }
  }
}

fn solve_normalized([a, b, c]: [f64; 3]) -> Result<RealRoots, Never> {
  let a2 = a * a;
  let q = (a2 - b * 3.0) / 9.0;
  let r = (2.0 * a2 * a - 9.0 * a * b + 27.0 * c) / 54.0;
  let q3 = q * q * q;
  let discriminant = r * r - q3;
  let adiv3 = a / 3.0;
  let mut roots = RealRoots::new();

  match (discriminant < 0.0).then_some(()) {
    Some(()) => {
      let theta = (r / q3.sqrt()).bound(-1.0, 1.0).acos();
      let scale = -2.0 * q.sqrt();
      roots.push(RealRoot(scale * (theta / 3.0).cos() - adiv3));
      let Ok(()) = push_distinct(
        &mut roots,
        RealRoot(scale * ((theta + 2.0 * PI) / 3.0).cos() - adiv3),
      );
      let Ok(()) = push_distinct(
        &mut roots,
        RealRoot(scale * ((theta - 2.0 * PI) / 3.0).cos() - adiv3),
      );
    }
    None => {
      let Ok(root) = cube_root(CubeRootArgument(r.abs() + discriminant.sqrt()));
      let mut signed = match (r > 0.0).then_some(()) {
        Some(()) => -root,
        None => root,
      };

      match signed.classify() {
        FpCategory::Zero => {}
        FpCategory::Nan | FpCategory::Infinite | FpCategory::Subnormal | FpCategory::Normal => {
          signed += q / signed;
        }
      }

      let first = signed - adiv3;
      roots.push(RealRoot(first));

      match first.almost_dequal_ulps(q3) {
        ScalarProximity::WithinTolerance => {
          let Ok(()) = push_distinct(&mut roots, RealRoot(-signed / 2.0 - adiv3));
        }
        ScalarProximity::OutsideTolerance => {}
      }
    }
  }

  Ok(roots)
}

fn push_distinct(roots: &mut RealRoots, candidate: RealRoot) -> Result<(), Never> {
  match roots
    .iter()
    .any(|root| root.0.almost_dequal_ulps(candidate.0) == ScalarProximity::WithinTolerance)
    .then_some(())
  {
    Some(()) => {}
    None => roots.push(candidate),
  }

  Ok(())
}
