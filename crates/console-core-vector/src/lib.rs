//! Two-dimensional displacements and directions.
//!
//! `between([start, end])` gives the displacement from start to end.
//! `from_origin` and `to_point` interpret a displacement relative to zero.
//! `try_map` applies one checked conversion to both coordinates.
//! `cross` is positive for the ordered pair (1, 0), (0, 1).
//! `length_squared` uses the vector's coordinate precision. With `std`,
//! `length` on `Vector2<f64>` uses hypot to avoid squaring large coordinates.

#![no_std]
#![forbid(unsafe_code)]

#[cfg(feature = "std")]
extern crate std;

use console_core_never::Never;
use console_core_point::Point;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Vector2<T> {
  pub x: T,
  pub y: T,
}

impl<T> Vector2<T> {
  pub fn try_map<U, E>(
    self,
    mut convert: impl FnMut(T) -> Result<U, E>,
  ) -> Result<Vector2<U>, E> {
    let x = convert(self.x)?;
    let y = convert(self.y)?;
    Ok(Vector2 { x, y })
  }
}

impl<T: Copy> Vector2<T> {
  #[inline]
  pub const fn from_origin(point: Point<T>) -> Result<Self, Never> {
    Ok(Self {
      x: point.x,
      y: point.y,
    })
  }

  #[inline]
  pub const fn to_point(self) -> Result<Point<T>, Never> {
    Ok(Point {
      x: self.x,
      y: self.y,
    })
  }
}

impl<T> From<(T, T)> for Vector2<T> {
  fn from((x, y): (T, T)) -> Self {
    Self { x, y }
  }
}

impl<T> From<Vector2<T>> for (T, T) {
  fn from(vector: Vector2<T>) -> Self {
    (vector.x, vector.y)
  }
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "f32 path geometry and f64 desktop geometry use the same arithmetic at their own precision"
  )
)]
macro_rules! float_vector {
  ($coordinate:ty) => {
    impl Vector2<$coordinate> {
      pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

      #[inline]
      pub const fn between([start, end]: [Point<$coordinate>; 2]) -> Result<Self, Never> {
        Ok(Self {
          x: end.x - start.x,
          y: end.y - start.y,
        })
      }

      #[inline]
      pub const fn dot(self, other: Self) -> Result<$coordinate, Never> {
        Ok(self.x * other.x + self.y * other.y)
      }

      #[inline]
      pub const fn cross(self, other: Self) -> Result<$coordinate, Never> {
        Ok(self.x * other.y - self.y * other.x)
      }

      #[inline]
      pub const fn length_squared(self) -> Result<$coordinate, Never> {
        self.dot(self)
      }

      #[inline]
      pub const fn turned_quarter(self) -> Result<Self, Never> {
        Ok(Self {
          x: -self.y,
          y: self.x,
        })
      }

      #[inline]
      pub const fn rotate_scale(self, other: Self) -> Result<Self, Never> {
        Ok(Self {
          x: self.x * other.x - self.y * other.y,
          y: self.x * other.y + self.y * other.x,
        })
      }

      #[inline]
      pub const fn translated(
        self,
        origin: Point<$coordinate>,
      ) -> Result<Point<$coordinate>, Never> {
        Ok(Point {
          x: origin.x + self.x,
          y: origin.y + self.y,
        })
      }
    }
  };
}

float_vector!(f32);
float_vector!(f64);

#[cfg(feature = "std")]
impl Vector2<f64> {
  pub fn length(self) -> Result<f64, Never> {
    Ok(self.x.hypot(self.y))
  }
}
