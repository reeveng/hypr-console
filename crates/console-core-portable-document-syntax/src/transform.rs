//! 2D affine transformations.

use console_core_never::Never;
use console_core_point::Point;
use console_core_size::Size;
use console_core_transform::AffineTransform;
use core::ops::Mul;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform(AffineTransform);

impl Transform {
  pub const IDENTITY: Self = Self(AffineTransform::IDENTITY);

  #[inline(always)]
  pub(crate) const fn new(c: [f64; 6]) -> Result<Self, Never> {
    let Ok(transform) = AffineTransform::from_coefficients(c);

    Ok(Self(transform))
  }

  #[inline(always)]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "Uniform scaling is deliberately a dimensionless ratio"
    )
  )]
  pub(crate) const fn scale(s: f64) -> Result<Self, Never> {
    let Ok(transform) = AffineTransform::scale(Size {
      width: s,
      height: s,
    });

    Ok(Self(transform))
  }

  pub(crate) const ROTATE_CW_90: Self = ({
    let Ok(explicit_value) = Self::new([0.0, 1.0, -1.0, 0.0, 0.0, 0.0]);
    explicit_value
  });

  pub(crate) const ROTATE_CCW_90: Self = ({
    let Ok(explicit_value) = Self::new([0.0, -1.0, 1.0, 0.0, 0.0, 0.0]);
    explicit_value
  });

  #[inline(always)]
  pub(crate) const fn translate(p: (f64, f64)) -> Result<Self, Never> {
    let Ok(transform) = AffineTransform::translate(Point { x: p.0, y: p.1 });

    Ok(Self(transform))
  }

  #[inline(always)]
  pub const fn as_coeffs(self) -> Result<[f64; 6], Never> {
    self.0.coefficients()
  }
}

impl Mul for Transform {
  type Output = Self;

  #[inline]
  fn mul(self, other: Self) -> Self {
    Self(self.0 * other.0)
  }
}
