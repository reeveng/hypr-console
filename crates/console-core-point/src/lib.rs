//! A position has horizontal and vertical coordinates in the same units.
//! [`Point::map`] applies the same conversion to each coordinate.
//! [`Point::midpoint`] and [`Point::interpolate`] work on both coordinates together.
//! Interpolation fractions outside 0 to 1 extrapolate.

#![no_std]
#![forbid(unsafe_code)]

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Point<T> {
  pub x: T,
  pub y: T,
}

impl<T> Point<T> {
  pub fn map<U>(self, mut each: impl FnMut(T) -> U) -> Result<Point<U>, Never> {
    Ok(Point {
      x: each(self.x),
      y: each(self.y),
    })
  }
}

impl Point<f64> {
  pub const fn midpoint(self, other: Self) -> Result<Self, Never> {
    Ok(Self {
      x: 0.5 * (self.x + other.x),
      y: 0.5 * (self.y + other.y),
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the interpolation fraction is a dimensionless multiple of the displacement"
    )
  )]
  pub const fn interpolate(self, other: Self, fraction: f64) -> Result<Self, Never> {
    Ok(Self {
      x: self.x + fraction * (other.x - self.x),
      y: self.y + fraction * (other.y - self.y),
    })
  }

  pub const fn distance_squared(self, other: Self) -> Result<f64, Never> {
    let x = self.x - other.x;
    let y = self.y - other.y;

    Ok(x * x + y * y)
  }
}

#[cfg(test)]
mod tests {
  use super::Point;

  #[test]
  fn a_point_carries_its_two_words_rather_than_an_order() {
    let at = Point { y: 4.0, x: 3.0 };

    assert_eq!(at.x, 3.0);
    assert_eq!(at.y, 4.0);
  }
}
