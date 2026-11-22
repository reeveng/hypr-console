//! Straight segments with two endpoints in the caller's coordinate space.

#![no_std]
#![forbid(unsafe_code)]

use console_core_never::Never;
use console_core_point::Point;
use console_core_rectangle::Bounds;

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

/// Fractions along the two extended lines at their crossing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Crossing {
  pub first_fraction: f64,
  pub second_fraction: f64,
}

/// Position of the nearest point along a segment and its squared distance to the query.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projection {
  pub fraction: f64,
  pub distance_squared: f64,
}

impl LineSegment<f64> {
  /// Returns the coordinate changes from the start to the end.
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

  /// Evaluates a fraction along the segment. Fractions outside 0 to 1 extend the line.
  #[inline]
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

  /// Projects onto the closed segment. Coincident endpoints project to the start.
  #[inline]
  pub fn nearest(&self, point: Point<f64>) -> Result<Projection, Never> {
    let Ok(displacement) = self.displacement();
    let from_start = Point {
      x: point.x - self.start.x,
      y: point.y - self.start.y,
    };
    let along = displacement.x * from_start.x + displacement.y * from_start.y;
    let Ok(length_squared) = self.length_squared();
    // An undefined fraction on a zero-length segment selects its start.
    #[expect(clippy::manual_clamp, reason = "NaN selects the start of the segment")]
    let fraction = (along / length_squared).max(0.0).min(1.0);
    let residual_x = from_start.x - fraction * displacement.x;
    let residual_y = from_start.y - fraction * displacement.y;

    Ok(Projection {
      fraction,
      distance_squared: residual_x * residual_x + residual_y * residual_y,
    })
  }

  /// Returns the crossing of the two extended lines. Parallel lines have no unique crossing.
  #[inline]
  pub fn crossing(&self, other: Self) -> Result<Option<Crossing>, Never> {
    let Ok(first) = self.displacement();
    let Ok(second) = other.displacement();
    let denominator = cross(first, second);

    match denominator == 0.0 {
      true => Ok(None),
      false => {
        let between = Point {
          x: other.start.x - self.start.x,
          y: other.start.y - self.start.y,
        };

        Ok(Some(Crossing {
          first_fraction: cross(between, second) / denominator,
          second_fraction: cross(between, first) / denominator,
        }))
      }
    }
  }
}

fn cross(first: Point<f64>, second: Point<f64>) -> f64 {
  first.x * second.y - first.y * second.x
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
