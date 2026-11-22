//! Rectangles expressed as an origin and size or as opposite corners.

#![no_std]
#![forbid(unsafe_code)]

use console_core_never::Never;
use console_core_point::Point;
use console_core_size::Size;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Rectangle<T> {
  pub origin: Point<T>,
  pub size: Size<T>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Containment {
  Inside,
  Outside,
}

/// Coordinates that can add an extent when finding the end of a rectangle.
///
/// Integer implementations saturate at the limits of their type. Floating-point
/// implementations use ordinary addition, including its infinity and NaN behavior.
pub trait Coordinate: Copy + PartialOrd {
  fn add_extent(self, extent: Self) -> Self;
}

impl<T: Coordinate> Rectangle<T> {
  /// Returns the exclusive corner in the same units as the origin.
  pub fn end(&self) -> Result<Point<T>, Never> {
    Ok(Point {
      x: self.origin.x.add_extent(self.size.width),
      y: self.origin.y.add_extent(self.size.height),
    })
  }

  /// Includes the origin edges and excludes the opposite edges.
  pub fn contains(&self, point: Point<T>) -> Result<Containment, Never> {
    let Ok(end) = self.end();
    let bounds = Bounds {
      x0: self.origin.x,
      y0: self.origin.y,
      x1: end.x,
      y1: end.y,
    };

    bounds.contains(point)
  }
}

/// Opposite corners in the caller's coordinate space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Bounds<T> {
  pub x0: T,
  pub y0: T,
  pub x1: T,
  pub y1: T,
}

impl<T> Bounds<T> {
  /// Stores corners in the given order, allowing inverted bounds.
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "PDF callers use x0, y0, x1, y1 order"
    )
  )]
  pub const fn new(x0: T, y0: T, x1: T, y1: T) -> Result<Self, Never> {
    Ok(Self { x0, y0, x1, y1 })
  }
}

impl<T: PartialOrd> Bounds<T> {
  /// Includes each minimum edge and excludes each maximum edge.
  pub fn contains(&self, point: Point<T>) -> Result<Containment, Never> {
    Ok(
      match point.x >= self.x0 && point.x < self.x1 && point.y >= self.y0 && point.y < self.y1 {
        true => Containment::Inside,
        false => Containment::Outside,
      },
    )
  }
}

// Each numeric implementation uses the same bounds operations. Keeping the scalar
// operations concrete lets callers such as curve geometry evaluate bounds in constants.
macro_rules! bounds_operations {
  ($coordinate:ty, $minimum:path, $maximum:path, $difference:path) => {
    impl Bounds<$coordinate> {
      /// Encloses both points, in either order.
      pub const fn from_points(
        first: Point<$coordinate>,
        second: Point<$coordinate>,
      ) -> Result<Self, Never> {
        Self {
          x0: first.x,
          y0: first.y,
          x1: second.x,
          y1: second.y,
        }
        .normalized()
      }

      /// Puts the lower coordinate first on each axis.
      pub const fn normalized(&self) -> Result<Self, Never> {
        Ok(Self {
          x0: $minimum(self.x0, self.x1),
          y0: $minimum(self.y0, self.y1),
          x1: $maximum(self.x0, self.x1),
          y1: $maximum(self.y0, self.y1),
        })
      }

      /// Returns the horizontal extent, saturating for integer coordinates.
      pub const fn width(&self) -> Result<$coordinate, Never> {
        Ok($difference(self.x1, self.x0))
      }

      /// Returns the vertical extent, saturating for integer coordinates.
      pub const fn height(&self) -> Result<$coordinate, Never> {
        Ok($difference(self.y1, self.y0))
      }

      pub const fn size(&self) -> Result<Size<$coordinate>, Never> {
        let Ok(width) = self.width();
        let Ok(height) = self.height();

        Ok(Size { width, height })
      }

      /// Finds the overlap, clamping a disjoint axis to a zero-length extent.
      pub const fn intersect(&self, other: Self) -> Result<Self, Never> {
        let x0 = $maximum(self.x0, other.x0);
        let y0 = $maximum(self.y0, other.y0);

        Ok(Self {
          x0,
          y0,
          x1: $maximum($minimum(self.x1, other.x1), x0),
          y1: $maximum($minimum(self.y1, other.y1), y0),
        })
      }

      /// Encloses the two rectangles. Supply normalized bounds to enclose both full extents.
      pub const fn union(&self, other: Self) -> Result<Self, Never> {
        Ok(Self {
          x0: $minimum(self.x0, other.x0),
          y0: $minimum(self.y0, other.y0),
          x1: $maximum(self.x1, other.x1),
          y1: $maximum(self.y1, other.y1),
        })
      }
    }
  };
}

macro_rules! integer_coordinates {
  ($($coordinate:ident),+ $(,)?) => {
    $(
      impl Coordinate for $coordinate {
        fn add_extent(self, extent: Self) -> Self {
          self.saturating_add(extent)
        }
      }

      impl Bounds<$coordinate> {
        const fn minimum(first: $coordinate, second: $coordinate) -> $coordinate {
          match first <= second {
            true => first,
            false => second,
          }
        }

        const fn maximum(first: $coordinate, second: $coordinate) -> $coordinate {
          match first >= second {
            true => first,
            false => second,
          }
        }
      }

      bounds_operations!($coordinate, Self::minimum, Self::maximum, $coordinate::saturating_sub);
    )+
  };
}

macro_rules! floating_point_coordinates {
  ($($coordinate:ident),+ $(,)?) => {
    $(
      impl Coordinate for $coordinate {
        fn add_extent(self, extent: Self) -> Self {
          self + extent
        }
      }

      impl Bounds<$coordinate> {
        const fn difference(end: $coordinate, start: $coordinate) -> $coordinate {
          end - start
        }
      }

      bounds_operations!($coordinate, $coordinate::min, $coordinate::max, Self::difference);
    )+
  };
}

integer_coordinates!(
  u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize
);
floating_point_coordinates!(f32, f64);

impl Bounds<f64> {
  pub const ZERO: Self = Self {
    x0: 0.0,
    y0: 0.0,
    x1: 0.0,
    y1: 0.0,
  };
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn floating_point_rectangles_keep_fractional_ends() {
    let rectangle = Rectangle {
      origin: Point {
        x: -2.5_f32,
        y: 1.0,
      },
      size: Size {
        width: 3.25,
        height: 1.5,
      },
    };

    assert_eq!(rectangle.end(), Ok(Point { x: 0.75, y: 2.5 }));
    assert_eq!(
      rectangle.contains(Point { x: -2.5, y: 1.0 }),
      Ok(Containment::Inside)
    );
    assert_eq!(
      rectangle.contains(Point { x: 0.75, y: 1.0 }),
      Ok(Containment::Outside)
    );
  }

  #[test]
  fn integer_extents_saturate_at_the_coordinate_limits() {
    let rectangle = Rectangle {
      origin: Point {
        x: u32::MAX - 1,
        y: 2,
      },
      size: Size {
        width: 4,
        height: 7,
      },
    };

    assert_eq!(rectangle.end(), Ok(Point { x: u32::MAX, y: 9 }));
    assert_eq!(
      rectangle.contains(Point { x: u32::MAX, y: 2 }),
      Ok(Containment::Outside)
    );

    let Ok(bounds) =
      Bounds::<i32>::from_points(Point { x: i32::MAX, y: 10 }, Point { x: i32::MIN, y: -5 });
    assert_eq!(
      bounds.size(),
      Ok(Size {
        width: i32::MAX,
        height: 15
      })
    );
  }

  #[test]
  fn opposite_corners_can_arrive_in_either_order() {
    let first = Point { x: 8.0, y: -3.0 };
    let second = Point { x: 2.0, y: 4.0 };
    let Ok(bounds) = Bounds::<f64>::from_points(first, second);

    assert_eq!(
      bounds,
      Bounds {
        x0: 2.0,
        y0: -3.0,
        x1: 8.0,
        y1: 4.0
      }
    );
    assert_eq!(Bounds::<f64>::from_points(second, first), Ok(bounds));
    assert_eq!(
      bounds.size(),
      Ok(Size {
        width: 6.0,
        height: 7.0
      })
    );
  }

  #[test]
  fn disjoint_bounds_keep_the_anchor_of_the_empty_intersection() {
    let Ok(first) = Bounds::<f64>::new(0.0, 0.0, 1.0, 2.0);
    let Ok(second) = Bounds::<f64>::new(3.0, 1.0, 4.0, 3.0);
    let Ok(overlap) = first.intersect(second);

    assert_eq!(
      overlap,
      Bounds {
        x0: 3.0,
        y0: 1.0,
        x1: 3.0,
        y1: 2.0
      }
    );
    assert_eq!(
      overlap.contains(Point { x: 3.0, y: 1.0 }),
      Ok(Containment::Outside)
    );
  }
}
