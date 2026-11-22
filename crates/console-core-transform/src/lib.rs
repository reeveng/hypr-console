//! Two-dimensional affine transformations in double precision.

#![no_std]
#![forbid(unsafe_code)]

use console_core_never::Never;
use console_core_point::Point;
use console_core_size::Size;
use core::ops::Mul;

/// Coefficients `[a, b, c, d, e, f]` map `(x, y)` to `(a*x + c*y + e, b*x + d*y + f)`.
/// In `a * b`, `b` acts first.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename = "Affine"))]
pub struct AffineTransform([f64; 6]);

impl AffineTransform {
  pub const IDENTITY: Self = Self([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

  pub const fn from_coefficients(coefficients: [f64; 6]) -> Result<Self, Never> {
    Ok(Self(coefficients))
  }

  pub const fn coefficients(self) -> Result<[f64; 6], Never> {
    Ok(self.0)
  }

  pub const fn translate(offset: Point<f64>) -> Result<Self, Never> {
    Ok(Self([1.0, 0.0, 0.0, 1.0, offset.x, offset.y]))
  }

  pub const fn scale(factors: Size<f64>) -> Result<Self, Never> {
    Ok(Self([factors.width, 0.0, 0.0, factors.height, 0.0, 0.0]))
  }

  pub const fn determinant(self) -> Result<f64, Never> {
    let [a, b, c, d, _, _] = self.0;

    Ok(a * d - b * c)
  }

  /// Applies the linear part to a displacement.
  pub const fn apply_linear(self, displacement: Point<f64>) -> Result<Point<f64>, Never> {
    let [a, b, c, d, _, _] = self.0;

    Ok(Point {
      x: a * displacement.x + c * displacement.y,
      y: b * displacement.x + d * displacement.y,
    })
  }

  /// Applies the linear part and then the translation to a position.
  pub const fn apply(self, point: Point<f64>) -> Result<Point<f64>, Never> {
    let Ok(linear) = self.apply_linear(point);
    let [_, _, _, _, e, f] = self.0;

    Ok(Point {
      x: linear.x + e,
      y: linear.y + f,
    })
  }
}

impl Default for AffineTransform {
  fn default() -> Self {
    Self::IDENTITY
  }
}

impl Mul for AffineTransform {
  type Output = Self;

  fn mul(self, earlier: Self) -> Self {
    let [a, b, c, d, e, f] = earlier.0;
    let Ok(first) = self.apply_linear(Point { x: a, y: b });
    let Ok(second) = self.apply_linear(Point { x: c, y: d });
    let Ok(offset) = self.apply(Point { x: e, y: f });

    Self([first.x, first.y, second.x, second.y, offset.x, offset.y])
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn composition_applies_the_right_operand_first() {
    let Ok(scale) = AffineTransform::scale(Size {
      width: 2.0,
      height: 3.0,
    });
    let Ok(translate) = AffineTransform::translate(Point { x: 5.0, y: 7.0 });
    let point = Point { x: 11.0, y: 13.0 };

    assert_eq!(
      (translate * scale).apply(point),
      Ok(Point { x: 27.0, y: 46.0 })
    );
    assert_eq!(
      (scale * translate).apply(point),
      Ok(Point { x: 32.0, y: 60.0 })
    );
  }
}
