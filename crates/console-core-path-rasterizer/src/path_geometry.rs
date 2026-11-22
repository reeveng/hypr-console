use kurbo::bezier;

use core::num::FpCategory;

use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};
use console_core_paths::path_geometry::valid_unit_divide;
use console_core_paths::{NormalizedF32, NormalizedF32Exclusive, Point, StrictNum};

use crate::line_clipper::{Column, Row};
use crate::path64::{
  cubic64::Cubic64,
  line_cubic_intersections,
  point64::{AxisIntercept, Point64, SearchAxis},
};

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use console_core_paths::NoStdFloat;

pub use console_core_paths::path_geometry::{
  CubicCoeff, QuadCoeff, chop_cubic_at2, chop_quad_at, find_cubic_max_curvature,
  find_unit_quad_roots, new_t_values,
};

#[derive(Clone, Copy)]
pub enum Boundary {
  Vertical(Column),
  Horizontal(Row),
}

impl Boundary {
  pub fn position(self) -> Result<f32, Never> {
    Ok(match self {
      Self::Vertical(Column(x)) => x,
      Self::Horizontal(Row(y)) => y,
    })
  }

  pub fn coordinate(self, point: Point) -> Result<f32, Never> {
    Ok(match self {
      Self::Vertical(_) => point.x,
      Self::Horizontal(_) => point.y,
    })
  }
}

pub fn chop_quad_at_x_extrema(src: &[Point; 3], dst: &mut [Point; 5]) -> Result<u8, Never> {
  chop_quad_at_extrema(src, dst, |point| &mut point.x)
}

pub fn chop_quad_at_y_extrema(src: &[Point; 3], dst: &mut [Point; 5]) -> Result<u8, Never> {
  chop_quad_at_extrema(src, dst, |point| &mut point.y)
}

fn chop_quad_at_extrema(
  src: &[Point; 3],
  dst: &mut [Point; 5],
  coordinate: impl Fn(&mut Point) -> &mut f32,
) -> Result<u8, Never> {
  let [mut first, mut control, mut last] = *src;
  let a = *coordinate(&mut first);
  let b = *coordinate(&mut control);
  let c = *coordinate(&mut last);
  let ab = a - b;
  let bc = match (ab < 0.0).then_some(()) {
    Some(()) => -(b - c),
    None => b - c,
  };

  match (matches!(ab.classify(), FpCategory::Zero) || bc < 0.0).then_some(()) {
    Some(()) => {
      let [numerator, denominator] = bezier::quadratic_extremum([a, b, c]);
      let Ok(parameter) = valid_unit_divide(numerator, denominator);

      match parameter {
        Some(t) => {
          let Ok(()) = chop_quad_at(src, t, dst);
          let [_, before, extremum, after, _] = dst;
          let value = *coordinate(extremum);
          *coordinate(before) = value;
          *coordinate(after) = value;

          return Ok(1);
        }
        None => {
          *coordinate(&mut control) = match ((a - b).abs() < (b - c).abs()).then_some(()) {
            Some(()) => a,
            None => c,
          };
        }
      }
    }
    None => {}
  }

  let [start, handle, end, ..] = dst;
  *start = first;
  *handle = control;
  *end = last;

  Ok(0)
}

pub fn chop_cubic_at_x_extrema(src: &[Point; 4], dst: &mut [Point; 10]) -> Result<u8, Never> {
  chop_cubic_at_extrema(src, dst, |point| &mut point.x)
}

pub fn chop_cubic_at_y_extrema(src: &[Point; 4], dst: &mut [Point; 10]) -> Result<u8, Never> {
  chop_cubic_at_extrema(src, dst, |point| &mut point.y)
}

fn chop_cubic_at_extrema(
  src: &[Point; 4],
  dst: &mut [Point; 10],
  coordinate: impl Fn(&mut Point) -> &mut f32,
) -> Result<u8, Never> {
  let axis = src.map(|mut point| *coordinate(&mut point));
  let [na, nb, nc] = bezier::cubic_derivative_coefficients(axis, 2.0, 3.0);
  let Ok(mut roots) = new_t_values();
  let Ok(raw_count) = find_unit_quad_roots(na, nb, nc, &mut roots).and_then(fixed_index);
  let [first_root, second_root, _] = roots;
  let both = [first_root, second_root];
  let (parameters, count): (&[NormalizedF32Exclusive], u8) = match raw_count {
    0 => (&[], 0),
    1 => (core::slice::from_ref(&first_root), 1),
    _ => (both.as_slice(), 2),
  };
  let Ok(()) = chop_cubic_at(src, parameters, dst);
  let [
    _,
    _,
    before,
    pivot,
    after,
    before_second,
    pivot_second,
    after_second,
    _,
    _,
  ] = dst;

  match count {
    0 => {}
    _ => {
      let value = *coordinate(pivot);
      *coordinate(before) = value;
      *coordinate(after) = value;
    }
  }

  match count {
    2 => {
      let value = *coordinate(pivot_second);
      *coordinate(before_second) = value;
      *coordinate(after_second) = value;
    }
    _ => {}
  }

  Ok(count)
}

fn chop_cubic_at(
  source: &[Point; 4],
  parameters: &[NormalizedF32Exclusive],
  mut remaining: &mut [Point],
) -> Result<(), Never> {
  let (first, subsequent) = match parameters.split_first() {
    Some(split) => split,
    None => {
      match remaining.first_chunk_mut::<4>() {
        Some(cubic) => *cubic = *source,
        None => {}
      }

      return Ok(());
    }
  };

  let mut previous = *first;
  let mut local = *first;
  let mut piece = *source;

  for parameter in subsequent
    .iter()
    .copied()
    .map(Some)
    .chain(core::iter::once(None))
  {
    let output = match remaining.first_chunk_mut::<7>() {
      Some(split) => split,
      None => return Ok(()),
    };
    let Ok(()) = chop_cubic_at2(&piece, local, output);
    let [_, _, _, cross, first_handle, second_handle, end] = *output;
    let next = match parameter {
      Some(value) => value,
      None => return Ok(()),
    };
    let tail = match remaining.split_at_mut_checked(3) {
      Some((_, tail)) => tail,
      None => return Ok(()),
    };
    let Ok(before) = previous.get();
    let Ok(after) = next.get();
    let Ok(relative) = valid_unit_divide(after - before, 1.0 - before);

    match relative {
      Some(value) => {
        remaining = tail;
        previous = next;
        local = value;
        piece = [cross, first_handle, second_handle, end];
      }
      None => {
        match tail {
          [_, _, _, _, rest_first, rest_second, rest_end, ..] => {
            *rest_first = end;
            *rest_second = end;
            *rest_end = end;
          }
          _ => {}
        }

        return Ok(());
      }
    }
  }

  Ok(())
}

pub fn chop_cubic_at_max_curvature(src: &[Point; 4], dst: &mut [Point; 13]) -> Result<u8, Never> {
  let mut roots = [NormalizedF32::ZERO; 3];
  let Ok(candidates) = find_cubic_max_curvature(src, &mut roots);
  let mut parameters = [NormalizedF32Exclusive::ANY; 3];
  let mut available = parameters.iter_mut();
  let mut count = 0u8;

  for root in candidates {
    let value = root.get();

    match (0.0 < value && value < 1.0).then_some(()) {
      Some(()) => {
        let Ok(parameter) = NormalizedF32Exclusive::new_bounded(value);

        match available.next() {
          Some(output) => {
            *output = parameter;
            count = count.saturating_add(1);
          }
          None => break,
        }
      }
      None => {}
    }
  }

  let Ok(length) = index(count);
  let parts = match parameters.get(..length) {
    Some(occupied) => occupied,
    None => &[],
  };
  let Ok(()) = chop_cubic_at(src, parts, dst);

  Ok(count.saturating_add(1))
}

pub fn chop_mono_cubic(
  src: &[Point; 4],
  boundary: Boundary,
  dst: &mut [Point; 7],
) -> Result<crate::CubicDchopAtInterceptAnswer, Never> {
  let coordinates = src.map(|point| {
    let Ok(converted) = Point64::from_point(point);

    converted
  });
  let Ok(cubic) = Cubic64::new(coordinates);
  let intercept = match boundary {
    Boundary::Vertical(Column(x)) => AxisIntercept {
      axis: SearchAxis::X,
      position: f64::from(x),
    },
    Boundary::Horizontal(Row(y)) => AxisIntercept {
      axis: SearchAxis::Y,
      position: f64::from(y),
    },
  };
  let Ok(roots) = line_cubic_intersections::intersect(&cubic, intercept);
  let parameter = match roots.first() {
    Some(root) => *root,
    None => return Ok(crate::CubicDchopAtInterceptAnswer::No),
  };
  let Ok(pair) = cubic.chop_at(parameter);

  for (output, point) in dst.iter_mut().zip(pair) {
    let Ok(converted) = point.to_point();
    *output = converted;
  }

  Ok(crate::CubicDchopAtInterceptAnswer::Yes)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn chop_cubic_at_y_extrema_1() {
    let src = [
      Point { x: 10.0, y: 20.0 },
      Point { x: 67.0, y: 437.0 },
      Point { x: 298.0, y: 213.0 },
      Point { x: 401.0, y: 214.0 },
    ];

    let mut dst = [Point { x: 0.0, y: 0.0 }; 10];
    assert_eq!(chop_cubic_at_y_extrema(&src, &mut dst), Ok(2));
    assert_eq!(
      dst,
      [
        Point { x: 10.0, y: 20.0 },
        Point {
          x: 37.508274,
          y: 221.24475
        },
        Point {
          x: 105.541855,
          y: 273.19803
        },
        Point {
          x: 180.15599,
          y: 273.19803
        },
        Point {
          x: 259.80502,
          y: 273.19803
        },
        Point {
          x: 346.9527,
          y: 213.99666
        },
        Point {
          x: 400.30844,
          y: 213.99666
        },
        Point {
          x: 400.53958,
          y: 213.99666
        },
        Point {
          x: 400.7701,
          y: 213.99777
        },
        Point { x: 401.0, y: 214.0 },
      ]
    );
  }
}
