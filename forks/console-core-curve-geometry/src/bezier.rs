//! Bézier arithmetic for callers that use their own coordinate precision or vector types.
//!
//! Where rendering depends on numerical parity, these operations retain the
//! caller's floating-point operation order.

use alloc::{vec, vec::Vec};
use core::ops::{Add, Div, Mul, Sub};

#[cfg(not(feature = "std"))]
use crate::common::FloatFuncs as _;
use crate::{CubicBez, Point, Rect};

/// Quadratic Bézier coefficients in descending order, in the caller's coordinate type.
#[inline]
pub fn quadratic_coefficients<T>([start, control, end]: [T; 3]) -> [T; 3]
where
  T: Copy + Add<Output = T> + Sub<Output = T>,
{
  let delta = control - start;
  [end - (control + control) + start, delta + delta, start]
}

/// Cubic Bézier coefficients in descending order, in the caller's coordinate type.
#[inline]
pub fn cubic_coefficients<T>([start, first, second, end]: [T; 4], three: T) -> [T; 4]
where
  T: Copy + Add<Output = T> + Sub<Output = T> + Mul<Output = T>,
{
  [
    end + three * (first - second) - start,
    three * (second - (first + first) + start),
    three * (first - start),
    start,
  ]
}

/// Evaluates precomputed polynomial coefficients in descending order.
#[inline]
pub fn evaluate<T, const N: usize>(leading: T, remaining: [T; N], t: T) -> T
where
  T: Copy + Add<Output = T> + Mul<Output = T>,
{
  remaining
    .into_iter()
    .fold(leading, |value, term| value * t + term)
}

/// Splits a quadratic using the caller's interpolation precision.
#[inline]
pub fn split_quadratic<T: Copy>(
  [start, control, end]: [T; 3],
  lerp: impl Fn(T, T) -> T,
) -> [T; 5] {
  let left = lerp(start, control);
  let right = lerp(control, end);
  [start, left, lerp(left, right), right, end]
}

/// Splits a cubic using the caller's interpolation precision.
#[inline]
pub fn split_cubic<T: Copy>(
  [start, first, second, end]: [T; 4],
  lerp: impl Fn(T, T) -> T,
) -> [T; 7] {
  let a = lerp(start, first);
  let b = lerp(first, second);
  let c = lerp(second, end);
  let left = lerp(a, b);
  let right = lerp(b, c);
  [start, a, left, lerp(left, right), right, c, end]
}

/// Raises a quadratic Bézier to a cubic on one coordinate axis.
#[inline]
pub fn raise_quadratic<T>([start, control, end]: [T; 3], two_thirds: T) -> [T; 4]
where
  T: Copy + Add<Output = T> + Sub<Output = T> + Mul<Output = T>,
{
  [
    start,
    start + two_thirds * (control - start),
    end + two_thirds * (control - end),
    end,
  ]
}

/// Raises a quadratic with multiplication before division at each control.
#[inline]
pub fn raise_quadratic_svg([start, control, end]: [f64; 3]) -> [f64; 4] {
  [
    start,
    start + 2.0 * (control - start) / 3.0,
    end + 2.0 * (control - end) / 3.0,
    end,
  ]
}

/// Reflects a control through a shared endpoint for a smooth SVG segment.
#[inline]
pub fn reflected_control(endpoint: f64, preceding: f64) -> f64 {
  endpoint + (endpoint - preceding)
}

/// Reflects an SVG control using twice the endpoint as the intermediate.
#[inline]
pub fn reflected_control_doubled(endpoint: f64, preceding: f64) -> f64 {
  endpoint * 2.0 - preceding
}

/// Returns the numerator and denominator of the quadratic extremum parameter.
#[inline]
pub fn quadratic_extremum<T>([start, control, end]: [T; 3]) -> [T; 2]
where
  T: Copy + Add<Output = T> + Sub<Output = T>,
{
  [start - control, start - control - control + end]
}

/// Quadratic axis coefficients relative to an intercept.
#[inline]
pub fn quadratic_intercept_coefficients(
  [start, control, end]: [f32; 3],
  intercept: f32,
) -> [f32; 3] {
  let [_, leading] = quadratic_extremum([start, control, end]);
  [leading, 2.0 * (control - start), start - intercept]
}

/// Quadratic coefficients after projecting the controls onto a line normal.
#[inline]
pub fn projected_quadratic_coefficients([start, control, end]: [f32; 3]) -> [f32; 3] {
  [
    end + (start - 2.0 * control),
    2.0 * (control - start),
    start,
  ]
}

/// Displacement of a quadratic control from the chord midpoint.
#[inline]
pub fn quadratic_control_offset([start, control, end]: [f32; 3]) -> f32 {
  (start + end) * 0.5 - control
}

/// Displacements of a cubic's controls from the trisection points of its chord.
#[inline]
pub fn cubic_control_offsets<T>([p0, p1, p2, p3]: [T; 4], third: T, two_thirds: T) -> [T; 2]
where
  T: Copy + Add<Output = T> + Sub<Output = T> + Mul<Output = T>,
{
  [
    p1 - (third * p3 + two_thirds * p0),
    p2 - (third * p0 + two_thirds * p3),
  ]
}

/// Returns the coefficients of the cubic derivative divided by three.
#[inline]
pub fn cubic_derivative_coefficients<T>([a, b, c, d]: [T; 4], two: T, three: T) -> [T; 3]
where
  T: Copy + Add<Output = T> + Sub<Output = T> + Mul<Output = T>,
{
  [d - a + three * (b - c), two * (a - b - b + c), b - a]
}

/// Evaluates the quadratic tangent at a parameter using the caller's rounding.
#[inline]
pub fn quadratic_tangent<T>([start, control, end]: [T; 3], t: T) -> T
where
  T: Copy + Add<Output = T> + Sub<Output = T> + Mul<Output = T>,
{
  let difference = control - start;
  let tangent = (end - control - difference) * t + difference;
  tangent + tangent
}

/// Evaluates the cubic tangent divided by three in the caller's coordinate type.
#[inline]
pub fn scaled_cubic_tangent<T>([start, first, second, end]: [T; 4], t: T, three: T) -> T
where
  T: Copy + Add<Output = T> + Sub<Output = T> + Mul<Output = T>,
{
  let leading = end + three * (first - second) - start;
  let middle = second - (first + first) + start;
  evaluate(leading, [middle + middle, first - start], t)
}

/// Returns the curvature-related dot product of the first and second derivatives
/// on one cubic axis, omitting a common scale factor.
#[inline]
pub fn cubic_speed_extrema_coefficients([start, first, second, end]: [f32; 4]) -> [f32; 4] {
  let a = first - start;
  let b = second - 2.0 * first + start;
  let c = end + 3.0 * (first - second) - start;
  [c * c, 3.0 * b * c, 2.0 * b * b + c * a, a * b]
}

/// Returns the coefficients of the cross product between a cubic's derivatives.
#[inline]
pub fn cubic_inflection_coefficients<T>(x: [T; 4], y: [T; 4], two: T, three: T) -> [T; 3]
where
  T: Copy + Add<Output = T> + Sub<Output = T> + Mul<Output = T>,
{
  let differences = |[start, first, second, end]: [T; 4]| {
    [
      first - start,
      second - two * first + start,
      end + three * (first - second) - start,
    ]
  };
  let [ax, bx, cx] = differences(x);
  let [ay, by, cy] = differences(y);
  [bx * cy - by * cx, ax * cy - ay * cx, ax * by - ay * bx]
}

/// Returns the bounds of a cubic using the power-basis arithmetic used for SVG
/// diagram serialization. This preserves the rounding of the serialized bounds.
pub fn cubic_bounds_for_svg(curve: CubicBez) -> Rect {
  let x = [curve.p0.x, curve.p1.x, curve.p2.x, curve.p3.x];
  let y = [curve.p0.y, curve.p1.y, curve.p2.y, curve.p3.y];
  let x = svg_coefficients(x);
  let y = svg_coefficients(y);
  let mut bounds = Rect::from_points(curve.p0, curve.p3);

  for axis in [x, y] {
    for t in svg_extrema(axis).into_iter().flatten() {
      let [a, b, c, d] = x;
      let [e, f, g, h] = y;
      let point = Point::new(evaluate(a, [b, c, d], t), evaluate(e, [f, g, h], t));
      bounds.x0 = bounds.x0.min(point.x);
      bounds.y0 = bounds.y0.min(point.y);
      bounds.x1 = bounds.x1.max(point.x);
      bounds.y1 = bounds.y1.max(point.y);
    }
  }

  bounds
}

fn svg_coefficients([p0, p1, p2, p3]: [f64; 4]) -> [f64; 4] {
  [
    -p0 + 3.0 * p1 - 3.0 * p2 + p3,
    3.0 * p0 - 6.0 * p1 + 3.0 * p2,
    -3.0 * p0 + 3.0 * p1,
    p0,
  ]
}

fn svg_extrema([a, b, c, _]: [f64; 4]) -> [Option<f64>; 2] {
  const EPSILON: f64 = 1e-12;
  let a = 3.0 * a;
  let b = 2.0 * b;
  let roots = if a.abs() <= EPSILON {
    [(b.abs() > EPSILON).then(|| -c / b), None]
  } else {
    let discriminant = b * b - 4.0 * a * c;
    let tolerance = EPSILON * (b * b + (4.0 * a * c).abs() + 1.0);
    if discriminant >= -tolerance {
      let root = discriminant.max(0.0).sqrt();
      [Some((-b + root) / (2.0 * a)), Some((-b - root) / (2.0 * a))]
    } else {
      [None, None]
    }
  };
  roots.map(|root| root.filter(|&t| t > 0.0 && t < 1.0))
}

/// Splits a cubic coordinate at its midpoint using the Bernstein weights directly.
#[inline]
pub fn split_cubic_in_half([a, b, c, d]: [f64; 4]) -> [f64; 7] {
  [
    a,
    (a + b) / 2.0,
    (a + 2.0 * b + c) / 4.0,
    (a + 3.0 * (b + c) + d) / 8.0,
    (b + 2.0 * c + d) / 4.0,
    (c + d) / 2.0,
    d,
  ]
}

/// Evaluates a cubic with separately rounded Bernstein weights.
#[inline]
pub fn evaluate_cubic_bernstein(curve: CubicBez, t: f64) -> Point {
  let opposite = 1.0 - t;
  let opposite2 = opposite * opposite;
  let a = opposite2 * opposite;
  let b = 3.0 * opposite2 * t;
  let t2 = t * t;
  let c = 3.0 * opposite * t2;
  let d = t2 * t;
  Point::new(
    a * curve.p0.x + b * curve.p1.x + c * curve.p2.x + d * curve.p3.x,
    a * curve.p0.y + b * curve.p1.y + c * curve.p2.y + d * curve.p3.y,
  )
}

/// Evaluates a bicubic Bézier patch with the first parameter along the rows.
pub fn evaluate_bicubic_patch(points: [[Point; 4]; 4], parameter: Point) -> Point {
  fn weights(t: f64) -> [f64; 4] {
    [
      (1.0 - t).powi(3),
      3.0 * t * (1.0 - t).powi(2),
      3.0 * t.powi(2) * (1.0 - t),
      t.powi(3),
    ]
  }

  let mut result = Point::ORIGIN;
  let across = weights(parameter.y);
  for (u, row) in weights(parameter.x).into_iter().zip(points) {
    for (v, point) in across.into_iter().zip(row) {
      let basis = u * v;
      result.x += point.x * basis;
      result.y += point.y * basis;
    }
  }
  result
}

/// Numerator and denominator for the quadratic's minimum-speed parameter.
#[inline]
pub fn quadratic_speed_extremum(x: [f32; 3], y: [f32; 3]) -> [f32; 2] {
  let [start_x, control_x, _] = x;
  let [start_y, control_y, _] = y;
  let [_, bx] = quadratic_extremum(x);
  let [_, by] = quadratic_extremum(y);
  let ax = control_x - start_x;
  let ay = control_y - start_y;
  [-(ax * bx + ay * by), bx * bx + by * by]
}

/// Returns the coefficients of a cubic coordinate minus an axis intercept.
#[inline]
pub fn cubic_intercept_coefficients(
  [start, first, second, end]: [f64; 4],
  intercept: f64,
) -> [f64; 4] {
  let mut a = end;
  let mut b = second * 3.0;
  let mut c = first * 3.0;
  a -= start - c + b;
  b += 3.0 * start - 2.0 * c;
  c -= 3.0 * start;
  [a, b, c, start - intercept]
}

/// Cubic axis coefficients after subtracting the start coordinate.
#[inline]
pub fn cubic_offset_coefficients([start, first, second, end]: [f32; 4]) -> [f32; 3] {
    [
        end + 3.0 * (first - second) - start,
        3.0 * (second - first - first + start),
        3.0 * (first - start),
    ]
}

/// Squared deviation of a cubic axis's controls, used to choose a subdivision.
#[inline]
pub fn cubic_control_deviation<T>([p0, p1, p2, p3]: [T; 4], two: T, three: T) -> T
where
  T: Copy + PartialOrd + Sub<Output = T> + Mul<Output = T>,
{
  let first = three * p1 - two * p0 - p3;
  let last = three * p2 - two * p3 - p0;
  let first = first * first;
  let last = last * last;
  if first < last { last } else { first }
}

/// Controls for the middle span of a cardinal spline along one coordinate.
#[inline]
pub fn cardinal_controls<T>([p0, p1, p2, p3]: [T; 4], scale: T, divisor: T) -> [T; 2]
where
  T: Copy + Add<Output = T> + Sub<Output = T> + Mul<Output = T> + Div<Output = T>,
{
  [
    p1 + (scale * p2 - scale * p0) / divisor,
    p2 + (scale * p1 - scale * p3) / divisor,
  ]
}

/// Controls for a uniform Catmull–Rom span along one coordinate.
#[inline]
pub fn catmull_rom_controls([p0, p1, p2, p3]: [f64; 4], tension: f64) -> [f64; 2] {
  [p1 + (p2 - p0) * tension, p2 - (p3 - p1) * tension]
}

/// Cardinal controls with the tension already divided by the span's scale.
#[inline]
pub fn scaled_cardinal_controls([p0, p1, p2, p3]: [f64; 4], scale: f64) -> [f64; 2] {
  [p1 + scale * (p2 - p0), p2 + scale * (p1 - p3)]
}

/// Controls of a bowed cubic with its chord running from start to end.
#[inline]
pub fn bowed_line_controls<T>(
  [start, end]: [T; 2],
  displaced_start: T,
  position: T,
  two: T,
) -> [T; 2]
where
  T: Copy + Add<Output = T> + Sub<Output = T> + Mul<Output = T>,
{
  let chord = end - start;
  [
    displaced_start + chord * position,
    displaced_start + two * chord * position,
  ]
}

/// Controls and midpoint of a rational quadratic split at one half.
#[inline]
pub fn rational_quadratic_half<T>(
  [start, control, end]: [T; 3],
  weight: T,
  scale: T,
  half: T,
) -> [T; 3]
where
  T: Copy + Add<Output = T> + Mul<Output = T>,
{
  let weighted = weight * control;
  [
    (start + weighted) * scale,
    (start + (weighted + weighted) + end) * scale * half,
    (weighted + end) * scale,
  ]
}

/// Widens the rational quadratic midpoint arithmetic for large coordinates.
#[inline]
pub fn rational_quadratic_midpoint([start, control, end]: [f32; 3], weight: f32) -> f64 {
  let weight = f64::from(weight);
  let doubled = weight * 2.0;
  let scale = 1.0 / (1.0 + weight) * 0.5;
  (f64::from(start) + doubled * f64::from(control) + f64::from(end)) * scale
}

/// Estimates the distance between a conic and its quadratic approximation.
#[inline]
pub fn rational_quadratic_error(x: [f32; 3], y: [f32; 3], weight: f32) -> f32 {
  let offset = weight - 1.0;
  let factor = offset / (4.0 * (2.0 + offset));
  let deviation = |[start, control, end]: [f32; 3]| factor * (start - 2.0 * control + end);
  let x = deviation(x);
  let y = deviation(y);
  (x * x + y * y).sqrt()
}

/// Cubic controls for an axis-aligned elliptical span with known endpoints.
///
/// The first control is reflected through the start point, in the order used
/// by legacy SVG arc adapters that serialize the resulting coordinates.
pub fn elliptical_controls(
  start: Point,
  end: Point,
  radii: crate::Vec2,
  angles: [f64; 2],
) -> [Point; 2] {
  let [first, last] = angles;
  let tangent = ((last - first) / 4.0).tan();
  let hx = 4.0 / 3.0 * radii.x * tangent;
  let hy = 4.0 / 3.0 * radii.y * tangent;
  [
    Point::new(
      2.0 * start.x - (start.x + hx * first.sin()),
      2.0 * start.y - (start.y - hy * first.cos()),
    ),
    Point::new(end.x + hx * last.sin(), end.y - hy * last.cos()),
  ]
}

/// Initial join point of an open uniform cubic basis spline.
#[inline]
pub fn basis_start([first, second]: [f64; 2]) -> f64 {
  (5.0 * first + second) / 6.0
}

/// Control points and end of one uniform cubic basis span on a coordinate axis.
#[inline]
pub fn basis_span([previous, current, next]: [f64; 3]) -> [f64; 3] {
  [
    (2.0 * previous + current) / 3.0,
    (previous + 2.0 * current) / 3.0,
    (previous + 4.0 * current + next) / 6.0,
  ]
}

/// Controls of a Hermite span defined by its endpoint slopes.
#[inline]
pub fn hermite_controls(start: Point, end: Point, slopes: [f64; 2]) -> [Point; 2] {
  let [first, second] = slopes;
  let dx = (end.x - start.x) / 3.0;
  [
    Point::new(start.x + dx, start.y + dx * first),
    Point::new(end.x - dx, end.y - dx * second),
  ]
}

/// Controls of a cubic that approaches and leaves with horizontal tangents.
#[inline]
pub fn horizontal_controls(start: Point, end: Point) -> [Point; 2] {
  let midpoint = (start.x + end.x) / 2.0;
  [Point::new(midpoint, start.y), Point::new(midpoint, end.y)]
}

/// Controls of a cubic that approaches and leaves with vertical tangents.
#[inline]
pub fn vertical_controls(start: Point, end: Point) -> [Point; 2] {
  let midpoint = (start.y + end.y) / 2.0;
  [Point::new(start.x, midpoint), Point::new(end.x, midpoint)]
}

/// Controls for a natural cubic spline along one coordinate.
pub fn natural_controls(knots: &[f64]) -> (Vec<f64>, Vec<f64>) {
  let count = knots.len().saturating_sub(1);
  if count == 0 {
    return (Vec::new(), Vec::new());
  }

  let mut diagonal = vec![4.0; count];
  let mut rhs: Vec<_> = knots
    .windows(2)
    .map(|pair| 4.0 * pair[0] + 2.0 * pair[1])
    .collect();
  diagonal[0] = 2.0;
  rhs[0] = knots[0] + 2.0 * knots[1];
  if count == 1 {
    rhs[0] = knots[0] + knots[1];
  } else {
    diagonal[count - 1] = 7.0;
    rhs[count - 1] = 8.0 * knots[count - 1] + knots[count];
  }

  // Eliminate the lower diagonal, then solve the upper triangular system.
  for i in 1..count {
    let lower = if i == count - 1 { 2.0 } else { 1.0 };
    let pivot = lower / diagonal[i - 1];
    diagonal[i] -= pivot;
    rhs[i] -= pivot * rhs[i - 1];
  }

  let mut first = vec![0.0; count];
  let mut second = vec![0.0; count];
  first[count - 1] = rhs[count - 1] / diagonal[count - 1];
  second[count - 1] = (knots[count] + first[count - 1]) / 2.0;
  for i in (0..count - 1).rev() {
    first[i] = (rhs[i] - first[i + 1]) / diagonal[i];
    second[i] = 2.0 * knots[i + 1] - first[i + 1];
  }

  (first, second)
}

/// Controls of a nonuniform Catmull–Rom span.
///
/// Each length is given as `[length.powf(alpha), length.powf(2.0 * alpha)]`.
pub fn nonuniform_catmull_rom_controls(
  [previous, start, end, next]: [Point; 4],
  [[l01, l01_sq], [l12, l12_sq], [l23, l23_sq]]: [[f64; 2]; 3],
) -> [Point; 2] {
  let mut first = start;
  let mut second = end;
  if l01 > 1e-12 {
    let scale = 2.0 * l01_sq + 3.0 * l01 * l12 + l12_sq;
    let divisor = 3.0 * l01 * (l01 + l12);
    if divisor != 0.0 && divisor.is_finite() {
      first = Point::new(
        (start.x * scale - previous.x * l12_sq + end.x * l01_sq) / divisor,
        (start.y * scale - previous.y * l12_sq + end.y * l01_sq) / divisor,
      );
    }
  }
  if l23 > 1e-12 {
    let scale = 2.0 * l23_sq + 3.0 * l23 * l12 + l12_sq;
    let divisor = 3.0 * l23 * (l23 + l12);
    if divisor != 0.0 && divisor.is_finite() {
      second = Point::new(
        (end.x * scale + start.x * l23_sq - next.x * l12_sq) / divisor,
        (end.y * scale + start.y * l23_sq - next.y * l12_sq) / divisor,
      );
    }
  }
  [first, second]
}
