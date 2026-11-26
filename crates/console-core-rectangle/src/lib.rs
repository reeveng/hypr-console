//! Rectangles expressed as an origin and size or as opposite corners.
//!
//! [`Coordinate`] lets an origin add an extent to find its exclusive corner.
//! Integer extents saturate at the limits of the type. Floating-point extents
//! use ordinary addition, including infinity and NaN.
//! Both containment methods include the lower edges and exclude the upper edges.
//! [`Rectangle::from_pixels`] fits unsigned pixel dimensions to signed coordinates.
//!
//! [`Bounds::new`] stores the corners in input order and allows inverted bounds.
//! `from_points` and `normalized` put the lower coordinate first on each axis.
//! Named corner accessors use the stored coordinate order.
//! `width` and `height` give the extents, which saturate for integer coordinates.
//! `intersect` clamps a disjoint axis to zero extent. `union` encloses both
//! rectangles when the input bounds are normalized.
//! [`Rectangle::side_center`] gives the midpoint of a rectangle side.
//!
//! [`Rectangle::centered_cover`] takes positive source and destination sizes.
//! It gives the source region that fills the destination without stretching.
//! [`CropMinimum::OnePixel`] preserves a nonempty viewport after quantization.

#![no_std]
#![forbid(unsafe_code)]

mod insets;

pub use insets::Insets;

use console_core_never::Never;
use console_core_number_conversion::fitted;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
  Top,
  Bottom,
  Left,
  Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropMinimum {
  Subpixel,
  OnePixel,
}

impl Rectangle<i32> {
  pub fn from_pixels(origin: Point<i32>, size: Size<u32>) -> Result<Self, Never> {
    let Ok(width) = fitted::<u32, i32>(size.width);
    let Ok(height) = fitted::<u32, i32>(size.height);

    Ok(Self {
      origin,
      size: Size { width, height },
    })
  }
}

impl Rectangle<f64> {
  pub fn side_center(&self, side: Side) -> Result<Point<f64>, Never> {
    let Ok(bounds) = self.bounds();

    Ok(match side {
      Side::Top => Point {
        x: bounds.x0.midpoint(bounds.x1),
        y: bounds.y0,
      },
      Side::Bottom => Point {
        x: bounds.x0.midpoint(bounds.x1),
        y: bounds.y1,
      },
      Side::Left => Point {
        x: bounds.x0,
        y: bounds.y0.midpoint(bounds.y1),
      },
      Side::Right => Point {
        x: bounds.x1,
        y: bounds.y0.midpoint(bounds.y1),
      },
    })
  }

  pub fn centered_cover(
    source: Size<u32>,
    destination: Size<f64>,
    minimum: CropMinimum,
  ) -> Result<Self, Never> {
    let Ok(pixels) = source.map(f64::from);
    let ratio = destination.width / destination.height;
    let width = pixels.width.min(pixels.height * ratio);
    let height = pixels.height.min(pixels.width / ratio);
    let size = match minimum {
      CropMinimum::Subpixel => Size { width, height },
      CropMinimum::OnePixel => Size {
        width: width.max(1.0),
        height: height.max(1.0),
      },
    };
    Ok(Self {
      origin: Point {
        x: (pixels.width - size.width) * 0.5,
        y: (pixels.height - size.height) * 0.5,
      },
      size,
    })
  }
}

pub trait Coordinate: Copy + PartialOrd {
  fn add_extent(self, extent: Self) -> Self;
}

impl<T: Coordinate> Rectangle<T> {
  pub fn end(&self) -> Result<Point<T>, Never> {
    Ok(Point {
      x: self.origin.x.add_extent(self.size.width),
      y: self.origin.y.add_extent(self.size.height),
    })
  }

  pub fn bounds(&self) -> Result<Bounds<T>, Never> {
    let Ok(end) = self.end();
    Ok(Bounds {
      x0: self.origin.x,
      y0: self.origin.y,
      x1: end.x,
      y1: end.y,
    })
  }

  pub fn contains(&self, point: Point<T>) -> Result<Containment, Never> {
    let Ok(bounds) = self.bounds();
    bounds.contains(point)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Bounds<T> {
  pub x0: T,
  pub y0: T,
  pub x1: T,
  pub y1: T,
}

impl<T> Bounds<T> {
  pub const fn new(x0: T, y0: T, x1: T, y1: T) -> Result<Self, Never> {
    Ok(Self { x0, y0, x1, y1 })
  }
}

impl<T: Copy> Bounds<T> {
  pub const fn top_left(&self) -> Result<Point<T>, Never> {
    Ok(Point {
      x: self.x0,
      y: self.y0,
    })
  }

  pub const fn top_right(&self) -> Result<Point<T>, Never> {
    Ok(Point {
      x: self.x1,
      y: self.y0,
    })
  }

  pub const fn bottom_left(&self) -> Result<Point<T>, Never> {
    Ok(Point {
      x: self.x0,
      y: self.y1,
    })
  }

  pub const fn bottom_right(&self) -> Result<Point<T>, Never> {
    Ok(Point {
      x: self.x1,
      y: self.y1,
    })
  }
}

impl<T: PartialOrd> Bounds<T> {
  pub fn contains(&self, point: Point<T>) -> Result<Containment, Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        point.x >= self.x0 && point.x < self.x1 && point.y >= self.y0 && point.y < self.y1,
        (),
      ) {
        ::core::option::Option::Some(()) => Containment::Inside,
        ::core::option::Option::None => Containment::Outside,
      },
    )
  }
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "each coordinate representation uses the same rectangle operations with its own arithmetic"
  )
)]
macro_rules! bounds_operations {
  ($coordinate:ty) => {
    impl Bounds<$coordinate> {
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

      pub const fn normalized(&self) -> Result<Self, Never> {
        let Ok(x0) = Self::minimum([self.x0, self.x1]);
        let Ok(y0) = Self::minimum([self.y0, self.y1]);
        let Ok(x1) = Self::maximum([self.x0, self.x1]);
        let Ok(y1) = Self::maximum([self.y0, self.y1]);

        Ok(Self { x0, y0, x1, y1 })
      }

      pub const fn width(&self) -> Result<$coordinate, Never> {
        Self::extent([self.x0, self.x1])
      }

      pub const fn height(&self) -> Result<$coordinate, Never> {
        Self::extent([self.y0, self.y1])
      }

      pub const fn size(&self) -> Result<Size<$coordinate>, Never> {
        let Ok(width) = self.width();
        let Ok(height) = self.height();

        Ok(Size { width, height })
      }

      pub const fn intersect(&self, other: Self) -> Result<Self, Never> {
        let Ok(x0) = Self::maximum([self.x0, other.x0]);
        let Ok(y0) = Self::maximum([self.y0, other.y0]);
        let Ok(right) = Self::minimum([self.x1, other.x1]);
        let Ok(bottom) = Self::minimum([self.y1, other.y1]);
        let Ok(x1) = Self::maximum([right, x0]);
        let Ok(y1) = Self::maximum([bottom, y0]);

        Ok(Self { x0, y0, x1, y1 })
      }

      pub const fn union(&self, other: Self) -> Result<Self, Never> {
        let Ok(x0) = Self::minimum([self.x0, other.x0]);
        let Ok(y0) = Self::minimum([self.y0, other.y0]);
        let Ok(x1) = Self::maximum([self.x1, other.x1]);
        let Ok(y1) = Self::maximum([self.y1, other.y1]);

        Ok(Self { x0, y0, x1, y1 })
      }
    }
  };
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "every integer coordinate uses the same ordering and saturating extent arithmetic"
  )
)]
macro_rules! integer_coordinates {
  ($($coordinate:ident),+ $(,)?) => {
    $(
      impl Coordinate for $coordinate {
        fn add_extent(self, extent: Self) -> Self {
          self.saturating_add(extent)
        }
      }

      impl Bounds<$coordinate> {
        const fn minimum([first, second]: [$coordinate; 2]) -> Result<$coordinate, Never> {
          Ok(match first <= second {
            true => first,
            false => second,
          })
        }

        const fn maximum([first, second]: [$coordinate; 2]) -> Result<$coordinate, Never> {
          Ok(match first >= second {
            true => first,
            false => second,
          })
        }

        const fn extent([start, end]: [$coordinate; 2]) -> Result<$coordinate, Never> {
          Ok(end.saturating_sub(start))
        }
      }

      bounds_operations!($coordinate);
    )+
  };
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "both floating coordinate widths use the same floating-point ordering and extent arithmetic"
  )
)]
macro_rules! floating_point_coordinates {
  ($($coordinate:ident),+ $(,)?) => {
    $(
      impl Coordinate for $coordinate {
        fn add_extent(self, extent: Self) -> Self {
          self + extent
        }
      }

      impl Bounds<$coordinate> {
        const fn minimum([first, second]: [$coordinate; 2]) -> Result<$coordinate, Never> {
          Ok(first.min(second))
        }

        const fn maximum([first, second]: [$coordinate; 2]) -> Result<$coordinate, Never> {
          Ok(first.max(second))
        }

        const fn extent([start, end]: [$coordinate; 2]) -> Result<$coordinate, Never> {
          Ok(end - start)
        }
      }

      bounds_operations!($coordinate);
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
  fn centered_crop_preserves_each_callers_minimum_extent() {
    let source = Size {
      width: 4,
      height: 2,
    };
    let destination = Size {
      width: 0.25,
      height: 1.0,
    };

    assert_eq!(
      Rectangle::centered_cover(source, destination, CropMinimum::Subpixel),
      Ok(Rectangle {
        origin: Point { x: 1.75, y: 0.0 },
        size: Size {
          width: 0.5,
          height: 2.0
        }
      })
    );
    assert_eq!(
      Rectangle::centered_cover(source, destination, CropMinimum::OnePixel),
      Ok(Rectangle {
        origin: Point { x: 1.5, y: 0.0 },
        size: Size {
          width: 1.0,
          height: 2.0
        }
      })
    );
  }

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
        x: ::core::primitive::u32::wrapping_sub(u32::MAX, 1),
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
