//! Straight segments with two endpoints in the caller's coordinate space.
//!
//! [`LineSegment::displacement`] gives the coordinate changes from start to end.
//! [`LineSegment::point_at`] evaluates a fraction along the segment. Fractions
//! outside 0 to 1 extend the line.
//! [`LineSegment::nearest`] projects onto the closed segment. Coincident
//! endpoints project to the start.
//! [`LineSegment::crossing`] finds the intersection of the two extended lines.
//! Parallel lines have no unique crossing. [`Crossing`] gives the fractions at
//! the intersection and [`Projection`] gives the nearest fraction and squared
//! distance to the query.

#![no_std]
#![forbid(unsafe_code)]

use console_core_never::Never;
use console_core_point::Point;
use console_core_rectangle::Bounds;
use core::num::FpCategory;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineSegment<T> {
  pub start: Point<T>,
  pub end: Point<T>,
}

impl<T> LineSegment<T> {
  pub const fn reversed(&self) -> Result<Self, Never>
  where
    T: Copy,
  {
    Ok(Self {
      start: self.end,
      end: self.start,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "crossings hold two dimensionless fractions along the corresponding lines"
  )
)]
pub struct Crossing {
  pub first_fraction: f64,
  pub second_fraction: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "projection returns a dimensionless fraction and squared distance in the caller's arbitrary coordinate units"
  )
)]
pub struct Projection {
  pub fraction: f64,
  pub distance_squared: f64,
}

impl LineSegment<f64> {
  #[inline]
  pub const fn displacement(&self) -> Result<Point<f64>, Never> {
    Ok(Point {
      x: self.end.x - self.start.x,
      y: self.end.y - self.start.y,
    })
  }

  #[inline]
  pub const fn midpoint(&self) -> Result<Point<f64>, Never> {
    self.start.midpoint(self.end)
  }

  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the interpolation fraction is a dimensionless multiple of the segment displacement"
    )
  )]
  pub const fn point_at(&self, fraction: f64) -> Result<Point<f64>, Never> {
    self.start.interpolate(self.end, fraction)
  }

  #[inline]
  pub const fn bounds(&self) -> Result<Bounds<f64>, Never> {
    Bounds::<f64>::from_points(self.start, self.end)
  }

  #[inline]
  pub const fn length_squared(&self) -> Result<f64, Never> {
    self.end.distance_squared(self.start)
  }

  #[inline]
  pub fn nearest(&self, point: Point<f64>) -> Result<Projection, Never> {
    let Ok(displacement) = self.displacement();
    let from_start = Point {
      x: point.x - self.start.x,
      y: point.y - self.start.y,
    };
    let along = displacement.x * from_start.x + displacement.y * from_start.y;
    let Ok(length_squared) = self.length_squared();

    #[expect(clippy::manual_clamp, reason = "NaN selects the start of the segment")]
    let fraction = (along / length_squared).max(0.0).min(1.0);
    let residual_x = from_start.x - fraction * displacement.x;
    let residual_y = from_start.y - fraction * displacement.y;

    Ok(Projection {
      fraction,
      distance_squared: residual_x * residual_x + residual_y * residual_y,
    })
  }

  #[inline]
  pub fn crossing(&self, other: Self) -> Result<Option<Crossing>, Never> {
    let Ok(first) = self.displacement();
    let Ok(second) = other.displacement();
    let Ok(denominator) = cross(first, second);

    match denominator.classify() {
      FpCategory::Zero => Ok(None),
      FpCategory::Nan | FpCategory::Infinite | FpCategory::Subnormal | FpCategory::Normal => {
        let between = Point {
          x: other.start.x - self.start.x,
          y: other.start.y - self.start.y,
        };

        let Ok(first_cross) = cross(between, second);
        let Ok(second_cross) = cross(between, first);

        Ok(Some(Crossing {
          first_fraction: first_cross / denominator,
          second_fraction: second_cross / denominator,
        }))
      }
    }
  }
}

#[must_use]
fn cross(first: Point<f64>, second: Point<f64>) -> Result<f64, Never> {
  Ok(first.x * second.y - first.y * second.x)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn crossings_can_lie_beyond_the_endpoints() {
    let first = LineSegment {
      start: Point { x: 0.0, y: 0.0 },
      end: Point { x: 2.0, y: 0.0 },
    };
    let second = LineSegment {
      start: Point { x: 4.0, y: -1.0 },
      end: Point { x: 4.0, y: 1.0 },
    };

    assert_eq!(first.crossing(first), Ok(None));
    assert_eq!(
      first.crossing(second),
      Ok(Some(Crossing {
        first_fraction: 2.0,
        second_fraction: 0.5
      }))
    );
    assert_eq!(first.point_at(2.0), second.midpoint());
  }

  #[test]
  fn coincident_endpoints_project_to_the_start() {
    let start = Point { x: 3.0, y: 2.0 };
    let line = LineSegment { start, end: start };

    assert_eq!(
      line.nearest(Point { x: 6.0, y: 6.0 }),
      Ok(Projection {
        fraction: 0.0,
        distance_squared: 25.0
      })
    );
  }
}
