//! A collection of functions to work with Bezier paths.
//!
//! Mainly for internal use. Do not rely on it!

use crate::{Point, Transform};
use crate::{StrictNormalized, StrictNum};
use console_core_never::Never;
use console_core_number_conversion::{index, nearest_f32_from_f64};
use kurbo::bezier;

use crate::f32x2_t::f32x2;
use crate::floating_point::FLOAT_PI;
use crate::scalar::{SCALAR_NEARLY_ZERO, SCALAR_ROOT_2_OVER_2, Scalar};

use crate::floating_point::{NormalizedF32, NormalizedF32Exclusive};
use crate::path_builder::{ConicWeight, PathDirection};

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use crate::NoStdFloat;

#[derive(Clone, Copy, Default, Debug)]
pub struct QuadCoeff {
  pub a: f32x2,
  pub b: f32x2,
  pub c: f32x2,
}

impl QuadCoeff {
  pub fn from_points(points: &[Point; 3]) -> Result<Self, Never> {
    let Ok(lanes) = vectors(*points);
    let [a, b, c] = bezier::quadratic_coefficients(lanes);
    Ok(Self { a, b, c })
  }

  pub fn eval(&self, t: f32x2) -> Result<f32x2, Never> {
    Ok(bezier::evaluate(self.a, [self.b, self.c], t))
  }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct CubicCoeff {
  pub a: f32x2,
  pub b: f32x2,
  pub c: f32x2,
  pub d: f32x2,
}

impl CubicCoeff {
  pub fn from_points(points: &[Point; 4]) -> Result<Self, Never> {
    let Ok(three) = f32x2::splat(3.0);
    let Ok(lanes) = vectors(*points);
    let [a, b, c, d] = bezier::cubic_coefficients(lanes, three);
    Ok(Self { a, b, c, d })
  }

  pub fn eval(&self, t: f32x2) -> Result<f32x2, Never> {
    Ok(bezier::evaluate(self.a, [self.b, self.c, self.d], t))
  }
}

pub fn new_t_values() -> Result<[NormalizedF32Exclusive; 3], Never> {
  Ok([NormalizedF32Exclusive::ANY; 3])
}

pub fn chop_quad_at(
  src: &[Point],
  t: NormalizedF32Exclusive,
  dst: &mut [Point; 5],
) -> Result<(), Never> {
  let points = match src.first_chunk::<3>() {
    Some(points) => {
      let Ok(lanes) = vectors(*points);
      lanes
    }
    None => return Ok(()),
  };
  let Ok(parameter) = t.get();
  let Ok(parameter_2) = f32x2::splat(parameter);
  let split = bezier::split_quadratic(points, |left, right| left + (right - left) * parameter_2);
  let Ok(converted) = path_points(split);
  *dst = converted;

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The coefficients share a scale factor which cancels when solving for the dimensionless curve parameter"
  )
)]
pub(crate) struct QuadraticPolynomial {
  pub quadratic: f32,
  pub linear: f32,
  pub constant: f32,
}

#[cfg_attr(
  dylint_lib = "explicit024_no_swappable_params",
  expect(
    explicit024_no_swappable_params,
    reason = "The public solver takes the quadratic, linear, and constant coefficients in polynomial order"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The coefficient scale cancels when solving for the dimensionless curve parameter"
  )
)]
pub fn find_unit_quad_roots(
  a: f32,
  b: f32,
  c: f32,
  roots: &mut [NormalizedF32Exclusive; 3],
) -> Result<console_core_api_compatibility::NativeSize, Never> {
  let Ok(count) = quadratic_roots(
    QuadraticPolynomial {
      quadratic: a,
      linear: b,
      constant: c,
    },
    roots,
  );

  index(u64::from(count))
}

pub(crate) fn quadratic_roots(
  QuadraticPolynomial {
    quadratic,
    linear,
    constant,
  }: QuadraticPolynomial,
  roots: &mut [NormalizedF32Exclusive; 3],
) -> Result<u8, Never> {
  let [left, right, _] = roots;

  match quadratic.classify() {
    core::num::FpCategory::Zero => {
      let Ok(linear_root) = divide_within_unit_interval(Quotient {
        numerator: -constant,
        denominator: linear,
      });
      *left = match linear_root {
        Some(linear_root_2) => linear_root_2,
        None => return Ok(0),
      };
      return Ok(1);
    }
    core::num::FpCategory::Nan
    | core::num::FpCategory::Infinite
    | core::num::FpCategory::Subnormal
    | core::num::FpCategory::Normal => {}
  }

  let discriminant =
    f64::from(linear) * f64::from(linear) - 4.0 * f64::from(quadratic) * f64::from(constant);

  match ::core::primitive::bool::then_some(discriminant < 0.0, ()) {
    ::core::option::Option::Some(()) => return Ok(0),
    ::core::option::Option::None => {}
  }

  let Ok(root_discriminant) = nearest_f32_from_f64(discriminant.sqrt());

  match ::core::primitive::bool::then_some(root_discriminant.is_finite(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Ok(0),
  }

  let q = match ::core::primitive::bool::then_some(linear < 0.0, ()) {
    ::core::option::Option::Some(()) => -(linear - root_discriminant) / 2.0,
    ::core::option::Option::None => -(linear + root_discriminant) / 2.0,
  };
  let Ok(first) = divide_within_unit_interval(Quotient {
    numerator: q,
    denominator: quadratic,
  });
  let Ok(second) = divide_within_unit_interval(Quotient {
    numerator: constant,
    denominator: q,
  });

  Ok(match (first, second) {
    (Some(first_2), Some(second_2)) => {
      let Ok(first_parameter) = first_2.get();
      let Ok(second_parameter) = second_2.get();

      match first_parameter.total_cmp(&second_parameter) {
        core::cmp::Ordering::Less => {
          *left = first_2;
          *right = second_2;
          2
        }
        core::cmp::Ordering::Equal => {
          *left = first_2;
          *right = second_2;
          1
        }
        core::cmp::Ordering::Greater => {
          *left = second_2;
          *right = first_2;
          2
        }
      }
    }
    (Some(root), None) | (None, Some(root)) => {
      *left = root;
      1
    }
    (None, None) => 0,
  })
}

pub fn chop_cubic_at2(
  src: &[Point; 4],
  t: NormalizedF32Exclusive,
  dst: &mut [Point],
) -> Result<(), Never> {
  let output = {
    let optional_output = dst.first_chunk_mut::<7>();

    match optional_output {
      Some(matched_output) => matched_output,
      ::core::option::Option::None => return Ok(()),
    }
  };

  let Ok(parameter) = t.get();
  let Ok(parameter_2) = f32x2::splat(parameter);
  let Ok(lanes) = vectors(*src);
  let split = bezier::split_cubic(lanes, |left, right| left + (right - left) * parameter_2);
  let Ok(converted) = path_points(split);
  *output = converted;

  Ok(())
}

pub(crate) fn find_quad_extrema(
  points: [f32; 3],
) -> Result<Option<NormalizedF32Exclusive>, Never> {
  let [numerator, denominator] = bezier::quadratic_extremum(points);
  divide_within_unit_interval(Quotient {
    numerator,
    denominator,
  })
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The numerator and denominator share units which cancel in the dimensionless quotient"
  )
)]
struct Quotient {
  numerator: f32,
  denominator: f32,
}

#[cfg_attr(
  dylint_lib = "explicit024_no_swappable_params",
  expect(
    explicit024_no_swappable_params,
    reason = "The public quotient takes its numerator before its denominator"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The numerator and denominator share units which cancel in the dimensionless quotient"
  )
)]
pub fn valid_unit_divide(
  numerator: f32,
  denominator: f32,
) -> Result<Option<NormalizedF32Exclusive>, Never> {
  divide_within_unit_interval(Quotient {
    numerator,
    denominator,
  })
}

fn divide_within_unit_interval(
  Quotient {
    numerator,
    denominator,
  }: Quotient,
) -> Result<Option<NormalizedF32Exclusive>, Never> {
  use core::num::FpCategory::Zero;

  let (numer, denom) = match ::core::primitive::bool::then_some(numerator < 0.0, ()) {
    ::core::option::Option::Some(()) => (-numerator, -denominator),
    ::core::option::Option::None => (numerator, denominator),
  };

  match (numer.classify(), denom.classify(), numer >= denom) {
    (Zero, _, _) | (_, Zero, _) | (_, _, true) => Ok(None),
    _ => NormalizedF32Exclusive::new(numer / denom),
  }
}

fn vectors<const N: usize>(points: [Point; N]) -> Result<[f32x2; N], Never> {
  Ok(points.map(|point| {
    let Ok(lanes) = point.to_f32x2();
    lanes
  }))
}

fn path_points<const N: usize>(vectors: [f32x2; N]) -> Result<[Point; N], Never> {
  Ok(vectors.map(|vector| {
    let Ok(point) = Point::from_f32x2(vector);
    point
  }))
}

pub(crate) fn find_quad_max_curvature(src: &[Point; 3]) -> Result<NormalizedF32, Never> {
  let [mut numer, mut denom] =
    bezier::quadratic_speed_extremum(src.map(|point| point.x), src.map(|point| point.y));

  match ::core::primitive::bool::then_some(denom < 0.0, ()) {
    ::core::option::Option::Some(()) => {
      numer = -numer;
      denom = -denom;
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(numer <= 0.0, ()) {
    ::core::option::Option::Some(()) => {
      return Ok(NormalizedF32::ZERO);
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(numer >= denom, ()) {
    ::core::option::Option::Some(()) => {
      return Ok(NormalizedF32::ONE);
    }
    ::core::option::Option::None => {}
  }

  let t = numer / denom;

  Ok(NormalizedF32::new_clamped(t))
}

pub(crate) fn eval_quad_at(src: &[Point; 3], t: NormalizedF32) -> Result<Point, Never> {
  let Ok(quad_coeff) = QuadCoeff::from_points(src);
  let Ok(splat) = f32x2::splat(t.get());
  let Ok(eval) = quad_coeff.eval(splat);
  let Ok(point) = Point::from_f32x2(eval);

  Ok(point)
}

pub(crate) fn eval_quad_tangent_at(src: &[Point; 3], tol: NormalizedF32) -> Result<Point, Never> {
  match ::core::primitive::bool::then_some(
    (tol == NormalizedF32::ZERO
      && (*{
        let [projected_element, ..] = src;
        projected_element
      }) == (*{
        let [_, projected_element, ..] = src;
        projected_element
      }))
      || (tol == NormalizedF32::ONE
        && (*{
          let [_, projected_element, ..] = src;
          projected_element
        }) == (*{
          let [_, _, projected_element, ..] = src;
          projected_element
        })),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(
        (*{
          let [_, _, projected_element, ..] = src;
          projected_element
        }) - (*{
          let [projected_element, ..] = src;
          projected_element
        }),
      );
    }
    ::core::option::Option::None => {}
  }

  let Ok(parameter) = f32x2::splat(tol.get());
  let Ok(lanes) = vectors(*src);
  Point::from_f32x2(bezier::quadratic_tangent(lanes, parameter))
}

pub fn find_cubic_max_curvature<'a>(
  src: &[Point; 4],
  t_values: &'a mut [NormalizedF32; 3],
) -> Result<&'a [NormalizedF32], Never> {
  let [start, first, second, end] = *src;
  let mut horizontal =
    bezier::cubic_speed_extrema_coefficients([start.x, first.x, second.x, end.x]);
  let vertical = bezier::cubic_speed_extrema_coefficients([start.y, first.y, second.y, end.y]);

  for (coefficient, extra) in horizontal.iter_mut().zip(vertical) {
    *coefficient += extra;
  }

  let Ok(count) = solve_cubic_poly(horizontal, t_values);
  let Ok(length) = index(u64::from(count));

  Ok(match t_values.get(..length) {
    Some(occupied) => occupied,
    None => &[],
  })
}

fn solve_cubic_poly(
  [leading, one, two, three]: [f32; 4],
  t_values: &mut [NormalizedF32; 3],
) -> Result<u8, Never> {
  match leading.is_nearly_zero() {
    crate::ScalarProximity::WithinTolerance => {
      let Ok(mut quadratic) = new_t_values();
      let Ok(count) = quadratic_roots(
        QuadraticPolynomial {
          quadratic: one,
          linear: two,
          constant: three,
        },
        &mut quadratic,
      );
      let Ok(length) = index(u64::from(count));

      for (parameter, root) in t_values.iter_mut().zip(quadratic).take(length) {
        let Ok(normalized) = root.to_normalized();
        *parameter = normalized;
      }

      return Ok(count);
    }
    crate::ScalarProximity::OutsideTolerance => {}
  }

  let inverse = leading.invert();
  let a = one * inverse;
  let b = two * inverse;
  let c = three * inverse;

  let q = (a * a - b * 3.0) / 9.0;
  let r = (2.0 * a * a * a - 9.0 * a * b + 27.0 * c) / 54.0;
  let q3 = q * q * q;
  let r2_minus_q3 = r * r - q3;
  let adiv3 = a / 3.0;

  match ::core::primitive::bool::then_some(r2_minus_q3 < 0.0, ()) {
    ::core::option::Option::Some(()) => {
      let theta = (r / q3.sqrt()).bound(-1.0, 1.0).acos();
      let neg2_root_q = -2.0 * q.sqrt();
      *t_values = [
        NormalizedF32::new_clamped(neg2_root_q * (theta / 3.0).cos() - adiv3),
        NormalizedF32::new_clamped(neg2_root_q * ((theta + 2.0 * FLOAT_PI) / 3.0).cos() - adiv3),
        NormalizedF32::new_clamped(neg2_root_q * ((theta - 2.0 * FLOAT_PI) / 3.0).cos() - adiv3),
      ];

      let Ok(()) = sort_array3(t_values);
      collapse_duplicates3(t_values)
    }
    ::core::option::Option::None => {
      let mut root = (r.abs() + r2_minus_q3.sqrt()).powf(0.3333333);

      match ::core::primitive::bool::then_some(r > 0.0, ()) {
        ::core::option::Option::Some(()) => root = -root,
        ::core::option::Option::None => {}
      }

      match root.classify() {
        core::num::FpCategory::Zero => {}
        core::num::FpCategory::Nan
        | core::num::FpCategory::Infinite
        | core::num::FpCategory::Subnormal
        | core::num::FpCategory::Normal => root += q / root,
      }

      let [first, ..] = t_values;
      *first = NormalizedF32::new_clamped(root - adiv3);

      Ok(1)
    }
  }
}

fn sort_array3(array: &mut [NormalizedF32; 3]) -> Result<(), Never> {
  match ::core::primitive::bool::then_some(
    (*{
      let [projected_element, ..] = &*array;
      projected_element
    }) > (*{
      let [_, projected_element, ..] = &*array;
      projected_element
    }),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      {
        let [swap_first, swap_second, ..] = &mut *array;
        ::core::mem::swap(swap_first, swap_second);
      };
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(
    (*{
      let [_, projected_element, ..] = &*array;
      projected_element
    }) > (*{
      let [_, _, projected_element, ..] = &*array;
      projected_element
    }),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      {
        let [_, swap_first, swap_second, ..] = &mut *array;
        ::core::mem::swap(swap_first, swap_second);
      };
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(
    (*{
      let [projected_element, ..] = &*array;
      projected_element
    }) > (*{
      let [_, projected_element, ..] = &*array;
      projected_element
    }),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      {
        let [swap_first, swap_second, ..] = &mut *array;
        ::core::mem::swap(swap_first, swap_second);
      };
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

fn collapse_duplicates3([first, second, third]: &[NormalizedF32; 3]) -> Result<u8, Never> {
  Ok(match (first == second, second == third) {
    (true, _) => 1,
    (false, true) => 2,
    (false, false) => 3,
  })
}

pub(crate) fn eval_cubic_pos_at(src: &[Point; 4], t: NormalizedF32) -> Result<Point, Never> {
  let Ok(cubic_coeff) = CubicCoeff::from_points(src);
  let Ok(splat) = f32x2::splat(t.get());
  let Ok(eval) = cubic_coeff.eval(splat);
  let Ok(point) = Point::from_f32x2(eval);

  Ok(point)
}

pub(crate) fn eval_cubic_tangent_at(src: &[Point; 4], t: NormalizedF32) -> Result<Point, Never> {
  Ok(
    match ::core::primitive::bool::then_some(
      (::core::matches!(
        ::core::primitive::f32::classify(t.get()),
        ::core::num::FpCategory::Zero
      ) && (*{
        let [projected_element, ..] = src;
        projected_element
      }) == (*{
        let [_, projected_element, ..] = src;
        projected_element
      }))
        || (::core::primitive::f32::to_bits(t.get()) == ::core::primitive::f32::to_bits(1.0)
          && (*{
            let [_, _, projected_element, ..] = src;
            projected_element
          }) == (*{
            let [_, _, _, projected_element, ..] = src;
            projected_element
          })),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let mut tangent = match ::core::primitive::f32::classify(t.get()) {
          ::core::num::FpCategory::Zero => {
            (*{
              let [_, _, projected_element, ..] = src;
              projected_element
            }) - (*{
              let [projected_element, ..] = src;
              projected_element
            })
          }
          ::core::num::FpCategory::Nan
          | ::core::num::FpCategory::Infinite
          | ::core::num::FpCategory::Subnormal
          | ::core::num::FpCategory::Normal => {
            (*{
              let [_, _, _, projected_element, ..] = src;
              projected_element
            }) - (*{
              let [_, projected_element, ..] = src;
              projected_element
            })
          }
        };

        match ::core::primitive::bool::then_some(
          ::core::matches!(
            ::core::primitive::f32::classify(tangent.x),
            ::core::num::FpCategory::Zero
          ) && ::core::matches!(
            ::core::primitive::f32::classify(tangent.y),
            ::core::num::FpCategory::Zero
          ),
          (),
        ) {
          ::core::option::Option::Some(()) => {
            tangent = (*{
              let [_, _, _, projected_element, ..] = src;
              projected_element
            }) - (*{
              let [projected_element, ..] = src;
              projected_element
            });
          }
          ::core::option::Option::None => {}
        }

        tangent
      }
      ::core::option::Option::None => {
        let Ok(explicit_value) = eval_cubic_derivative(src, t);
        explicit_value
      }
    },
  )
}

fn eval_cubic_derivative(src: &[Point; 4], t: NormalizedF32) -> Result<Point, Never> {
  let Ok(parameter) = f32x2::splat(t.get());
  let Ok(three) = f32x2::splat(3.0);
  let Ok(lanes) = vectors(*src);
  Point::from_f32x2(bezier::scaled_cubic_tangent(lanes, parameter, three))
}

pub(crate) fn find_cubic_extrema(
  points: [f32; 4],
  t_values: &mut [NormalizedF32Exclusive; 3],
) -> Result<u8, Never> {
  let [quadratic, linear, constant] = bezier::cubic_derivative_coefficients(points, 2.0, 3.0);
  quadratic_roots(
    QuadraticPolynomial {
      quadratic,
      linear,
      constant,
    },
    t_values,
  )
}

pub(crate) fn find_cubic_inflections<'a>(
  src: &[Point; 4],
  t_values: &'a mut [NormalizedF32Exclusive; 3],
) -> Result<&'a [NormalizedF32Exclusive], Never> {
  let [quadratic, linear, constant] = bezier::cubic_inflection_coefficients(
    src.map(|point| point.x),
    src.map(|point| point.y),
    2.0,
    3.0,
  );
  let Ok(count) = quadratic_roots(
    QuadraticPolynomial {
      quadratic,
      linear,
      constant,
    },
    t_values,
  );
  let Ok(length) = index(u64::from(count));

  Ok(match t_values.get(..length) {
    Some(occupied) => occupied,
    None => &[],
  })
}

pub(crate) fn find_cubic_cusp(src: &[Point; 4]) -> Result<Option<NormalizedF32Exclusive>, Never> {
  let [start, first, second, end] = *src;

  match ::core::primitive::bool::then_some(
    (*{
      let [projected_element, ..] = src;
      projected_element
    }) == (*{
      let [_, projected_element, ..] = src;
      projected_element
    }),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(
    (*{
      let [_, _, projected_element, ..] = src;
      projected_element
    }) == (*{
      let [_, _, _, projected_element, ..] = src;
      projected_element
    }),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(
    matches!(
      {
        let Ok(explicit_value) = on_same_side(EdgePair {
          axis: [second, end],
          probes: [start, first],
        });
        explicit_value
      },
      crate::OnSameSideAnswer::Yes
    ) || matches!(
      {
        let Ok(explicit_value) = on_same_side(EdgePair {
          axis: [start, first],
          probes: [second, end],
        });
        explicit_value
      },
      crate::OnSameSideAnswer::Yes
    ),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  let mut t_values = [NormalizedF32::ZERO; 3];
  let Ok(max_curvature) = find_cubic_max_curvature(src, &mut t_values);

  for test_t in max_curvature {
    match ::core::primitive::bool::then_some(0.0 >= test_t.get() || test_t.get() >= 1.0, ()) {
      ::core::option::Option::Some(()) => {
        continue;
      }
      ::core::option::Option::None => {}
    }

    let Ok(d_pt) = eval_cubic_derivative(src, *test_t);
    let Ok(d_pt_magnitude) = d_pt.length_sqd();
    let Ok(precision) = calc_cubic_precision(src);

    match ::core::primitive::bool::then_some(d_pt_magnitude < precision, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(normalized_f32exclusive) = NormalizedF32Exclusive::new_bounded(test_t.get());
        return Ok(Some(normalized_f32exclusive));
      }
      ::core::option::Option::None => {}
    }
  }

  Ok(None)
}

struct EdgePair {
  axis: [Point; 2],
  probes: [Point; 2],
}

fn on_same_side(
  EdgePair {
    axis: [origin, end],
    probes: [first, second],
  }: EdgePair,
) -> Result<crate::OnSameSideAnswer, Never> {
  let line = end - origin;
  let Ok(first_cross) = line.cross(first - origin);
  let Ok(second_cross) = line.cross(second - origin);

  Ok(
    match ::core::primitive::bool::then_some(first_cross * second_cross >= 0.0, ()) {
      ::core::option::Option::Some(()) => crate::OnSameSideAnswer::Yes,
      ::core::option::Option::None => crate::OnSameSideAnswer::No,
    },
  )
}

fn calc_cubic_precision(src: &[Point; 4]) -> Result<f32, Never> {
  let Ok(distance_to_sqd) = (*{
    let [_, projected_element, ..] = src;
    projected_element
  })
  .distance_to_sqd(match src {
    &[_explicit_element, ..] => _explicit_element,
  });
  let Ok(src_distance_to_sqd) = (*{
    let [_, _, projected_element, ..] = src;
    projected_element
  })
  .distance_to_sqd(match src {
    &[_, _explicit_element, ..] => _explicit_element,
  });

  Ok(
    (distance_to_sqd
      + src_distance_to_sqd
      + ({
        let Ok(explicit_value) = (*{
          let [_, _, _, projected_element, ..] = src;
          projected_element
        })
        .distance_to_sqd(match src {
          &[_, _, _explicit_element, ..] => _explicit_element,
        });
        explicit_value
      }))
      * 1e-8,
  )
}

const MAX_CONIC_SUBDIVISIONS: u8 = 4;

#[derive(Copy, Clone, Default, Debug)]
pub(crate) struct Conic {
  pub points: [Point; 3],
  pub weight: ConicWeight,
}

impl Conic {
  fn approximation_depth(&self) -> Result<Option<u8>, Never> {
    const MAX_ERROR_IN_PATH_UNITS: f32 = 0.25;

    for point in self.points {
      let Ok(finite) = point.is_finite();

      match finite {
        crate::IsFiniteAnswer::Yes => {}
        crate::IsFiniteAnswer::No => return Ok(None),
      }
    }

    let mut error = bezier::rational_quadratic_error(
      self.points.map(|point| point.x),
      self.points.map(|point| point.y),
      self.weight.0,
    );
    let mut depth = 0_u8;

    for _ in 0..MAX_CONIC_SUBDIVISIONS {
      match ::core::primitive::bool::then_some(error <= MAX_ERROR_IN_PATH_UNITS, ()) {
        ::core::option::Option::Some(()) => break,
        ::core::option::Option::None => {
          error *= 0.25;
          depth = depth.saturating_add(1);
        }
      }
    }

    Ok(Some(depth.max(1)))
  }

  fn approximate(&self, depth: u8, points: &mut [Point; 33]) -> Result<u8, Never> {
    let mut conics = [*self; 16];
    let mut count = 1_u8;

    for _ in 0..depth.min(MAX_CONIC_SUBDIVISIONS) {
      let mut next = conics;
      let Ok(length) = index(u64::from(count));

      for (conic, [left, right]) in conics.iter().take(length).zip(next.as_chunks_mut::<2>().0) {
        let Ok((first, second)) = conic.chop_monotonically();
        *left = first;
        *right = second;
      }

      conics = next;
      count = count.saturating_mul(2);
    }

    let [start, rest @ ..] = points;
    let [original_start, original_control, original_end] = self.points;
    *start = original_start;
    let Ok(length) = index(u64::from(count));
    let pairs = rest.as_chunks_mut::<2>().0;
    let mut finite = crate::IsFiniteAnswer::Yes;

    for (conic, [off_curve, end]) in conics.iter().zip(pairs.iter_mut()).take(length) {
      let [_, control, endpoint] = conic.points;
      *off_curve = control;
      *end = endpoint;
      let Ok(finite_control) = control.is_finite();
      let Ok(finite_end) = endpoint.is_finite();

      match (finite_control, finite_end) {
        (crate::IsFiniteAnswer::Yes, crate::IsFiniteAnswer::Yes) => {}
        _ => finite = crate::IsFiniteAnswer::No,
      }
    }

    match (matches!(finite, crate::IsFiniteAnswer::No)).then_some(()) {
      Some(()) => {
        for [off_curve, end] in pairs.iter_mut().take(length) {
          *off_curve = original_control;
          *end = original_control;
        }

        let Ok(last) = index(u64::from(count.saturating_sub(1)));

        {
          match pairs.get_mut(last) {
            Some([_, end]) => {
              *end = original_end;
            }
            ::core::option::Option::None => {}
          };
        }
      }
      None => {}
    }

    Ok(count)
  }

  fn chop_monotonically(&self) -> Result<(Conic, Conic), Never> {
    let Ok((mut left, mut right)) = self.chop();
    let [start, original, end] = self.points;
    let Ok(ordered) = between([start.y, original.y, end.y]);

    match ordered {
      crate::BetweenAnswer::Yes => {}
      crate::BetweenAnswer::No => return Ok((left, right)),
    }

    let [_, before, left_end] = &mut left.points;
    let [right_start, after, _] = &mut right.points;
    let Ok(midpoint_order) = between([start.y, left_end.y, end.y]);

    match (matches!(midpoint_order, crate::BetweenAnswer::No)).then_some(()) {
      Some(()) => {
        let closer_y = match ::core::primitive::bool::then_some(
          (left_end.y - start.y).abs() < (left_end.y - end.y).abs(),
          (),
        ) {
          ::core::option::Option::Some(()) => start.y,
          ::core::option::Option::None => end.y,
        };
        left_end.y = closer_y;
        right_start.y = closer_y;
      }
      None => {}
    }

    let Ok(left_order) = between([start.y, before.y, left_end.y]);
    let Ok(right_order) = between([right_start.y, after.y, end.y]);

    match (matches!(left_order, crate::BetweenAnswer::No)).then_some(()) {
      Some(()) => {
        before.y = start.y;
      }
      None => {}
    }

    match (matches!(right_order, crate::BetweenAnswer::No)).then_some(()) {
      Some(()) => {
        after.y = end.y;
      }
      None => {}
    }

    Ok((left, right))
  }

  fn chop(&self) -> Result<(Conic, Conic), Never> {
    let [start, _, end] = self.points;
    let weight = self.weight.0;
    let Ok(scale) = f32x2::splat((1.0 + weight).invert());
    let half_weight = ConicWeight((0.5 + weight * 0.5).sqrt());
    let Ok(ww) = f32x2::splat(weight);
    let Ok(half) = f32x2::splat(0.5);
    let Ok(lanes) = vectors(self.points);
    let [left, middle, right] = bezier::rational_quadratic_half(lanes, ww, scale, half);
    let Ok(mut midpoint) = Point::from_f32x2(middle);
    let Ok(finite) = midpoint.is_finite();

    match finite {
      crate::IsFiniteAnswer::No => {
        let Ok(x) = nearest_f32_from_f64(bezier::rational_quadratic_midpoint(
          self.points.map(|point| point.x),
          weight,
        ));
        let Ok(y) = nearest_f32_from_f64(bezier::rational_quadratic_midpoint(
          self.points.map(|point| point.y),
          weight,
        ));
        midpoint = Point { x, y };
      }
      crate::IsFiniteAnswer::Yes => {}
    }

    let Ok(left_control) = Point::from_f32x2(left);
    let Ok(right_control) = Point::from_f32x2(right);

    Ok((
      Self {
        points: [start, left_control, midpoint],
        weight: half_weight,
      },
      Self {
        points: [midpoint, right_control, end],
        weight: half_weight,
      },
    ))
  }

  pub fn build_unit_arc(
    u_start: Point,
    u_stop: Point,
    dir: PathDirection,
    user_transform: Transform,
    dst: &mut [Conic; 5],
  ) -> Result<Option<&[Conic]>, Never> {
    let Ok(x) = u_start.dot(u_stop);
    let Ok(cross) = u_start.cross(u_stop);
    let y = match dir {
      PathDirection::Clockwise => cross,
      PathDirection::CounterClockwise => -cross,
    };

    match ::core::primitive::bool::then_some(
      y.abs() <= SCALAR_NEARLY_ZERO && x > 0.0 && y >= 0.0,
      (),
    ) {
      ::core::option::Option::Some(()) => return Ok(None),
      ::core::option::Option::None => {}
    }

    let endpoint = Point { x, y };
    let Ok(quadrant) = arc_quadrant(endpoint);
    let quarter = ConicWeight(SCALAR_ROOT_2_OVER_2);
    let east = Point { x: 1.0, y: 0.0 };
    let south = Point { x: 0.0, y: 1.0 };
    let west = Point { x: -1.0, y: 0.0 };
    let north = Point { x: 0.0, y: -1.0 };
    let sections = [
      [east, Point { x: 1.0, y: 1.0 }, south],
      [south, Point { x: -1.0, y: 1.0 }, west],
      [west, Point { x: -1.0, y: -1.0 }, north],
    ];
    let cardinal = match quadrant {
      0 => east,
      1 => south,
      2 => west,
      _ => north,
    };
    let Ok(full_sections) = index(u64::from(quadrant));
    let mut output = dst.iter_mut();

    for (slot, points) in output.by_ref().zip(sections).take(full_sections) {
      *slot = Self {
        points,
        weight: quarter,
      };
    }

    let Ok(partial) = partial_arc([cardinal, endpoint]);
    let count = match (partial, output.next()) {
      (Some(conic), Some(slot)) => {
        *slot = conic;
        quadrant.saturating_add(1)
      }
      (None, _) | (Some(_), None) => quadrant,
    };
    let Ok(length) = index(u64::from(count));
    let used = match dst.get_mut(..length) {
      Some(used @ [_, ..]) => used,
      Some([]) | None => return Ok(None),
    };

    let Ok(mut transform) = Transform::from_unit_direction(u_start);

    match dir {
      PathDirection::Clockwise => {}
      PathDirection::CounterClockwise => {
        let Ok(flipped) = transform.pre_scale_by(crate::ScaleFactors { x: 1.0, y: -1.0 });
        transform = flipped;
      }
    }

    let Ok(position) = transform.post_concat(user_transform);

    for conic in used.iter_mut() {
      let Ok(()) = position.map_points(&mut conic.points);
    }

    Ok(Some(used))
  }
}

fn arc_quadrant(Point { x, y }: Point) -> Result<u8, Never> {
  use core::num::FpCategory::Zero;
  Ok(match (x.classify(), y.classify(), x < 0.0, y < 0.0) {
    (_, Zero, _, _) => 2,
    (Zero, _, _, false) => 1,
    (Zero, _, _, true) => 3,
    (_, _, false, false) => 0,
    (_, _, true, false) => 1,
    (_, _, true, true) => 2,
    (_, _, false, true) => 3,
  })
}

fn partial_arc([start, end]: [Point; 2]) -> Result<Option<Conic>, Never> {
  let Ok(dot) = start.dot(end);

  match ::core::primitive::bool::then_some((0.0..1.0).contains(&dot), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Ok(None),
  }

  let mut control = start + end;
  let weight = ((1.0 + dot) / 2.0).sqrt();
  let Ok(_set_length) = control.set_length(weight.invert());
  let Ok(coincident) = start.almost_equal(control);

  Ok(match coincident {
    crate::AlmostEqualAnswer::Yes => None,
    crate::AlmostEqualAnswer::No => Some(Conic {
      points: [start, control, end],
      weight: ConicWeight(weight),
    }),
  })
}

fn between([start, point, end]: [f32; 3]) -> Result<crate::BetweenAnswer, Never> {
  Ok(
    match ::core::primitive::bool::then_some((start - point) * (end - point) <= 0.0, ()) {
      ::core::option::Option::Some(()) => crate::BetweenAnswer::Yes,
      ::core::option::Option::None => crate::BetweenAnswer::No,
    },
  )
}

pub(crate) struct AutoConicToQuads {
  pub points: [Point; 33],
  pub len: u8,
}

impl AutoConicToQuads {
  pub fn compute(points: [Point; 3], weight: ConicWeight) -> Result<Option<Self>, Never> {
    let conic = Conic { points, weight };
    let Ok(approximation) = conic.approximation_depth();
    let depth = match approximation {
      Some(depth) => depth,
      None => return Ok(None),
    };

    let Ok(zero) = Point::zero();
    let mut output = [zero; 33];
    let Ok(len) = conic.approximate(depth, &mut output);

    Ok(Some(Self {
      points: output,
      len,
    }))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn eval_cubic_at_1() {
    let src = [
      Point { x: 30.0, y: 40.0 },
      Point { x: 30.0, y: 40.0 },
      Point { x: 171.0, y: 45.0 },
      Point { x: 180.0, y: 155.0 },
    ];

    assert_eq!(
      eval_cubic_pos_at(&src, NormalizedF32::ZERO),
      Point::from_xy(30.0, 40.0)
    );
    assert_eq!(
      eval_cubic_tangent_at(&src, NormalizedF32::ZERO),
      Point::from_xy(141.0, 5.0)
    );
  }

  #[test]
  fn find_cubic_max_curvature_1() {
    let src = [
      Point { x: 20.0, y: 160.0 },
      Point {
        x: 20.0001,
        y: 160.0,
      },
      Point { x: 160.0, y: 20.0 },
      Point {
        x: 160.0001,
        y: 20.0,
      },
    ];

    let mut t_values = [NormalizedF32::ZERO; 3];
    let Ok(found) = find_cubic_max_curvature(&src, &mut t_values);

    assert_eq!(
      found,
      &[
        NormalizedF32::ZERO,
        NormalizedF32::new_clamped(0.5),
        NormalizedF32::ONE
      ]
    );
  }
}
