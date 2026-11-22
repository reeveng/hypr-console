//! A single-precision complex number, with the three operations the transform
//! is made of. Every other one the Symphonia original carried was unread.

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the real and imaginary components are scalar coefficients in the same complex-number representation"
  )
)]
#[derive(Copy, Clone, Default, Debug, PartialEq)]
pub(crate) struct Complex {
  pub(crate) re: f32,
  pub(crate) im: f32,
}

impl core::ops::Add for Complex {
  type Output = Complex;

  #[inline(always)]
  fn add(self, rhs: Self) -> Self::Output {
    Self::Output {
      re: self.re + rhs.re,
      im: self.im + rhs.im,
    }
  }
}

impl core::ops::Sub for Complex {
  type Output = Complex;

  #[inline(always)]
  fn sub(self, rhs: Self) -> Self::Output {
    Self::Output {
      re: self.re - rhs.re,
      im: self.im - rhs.im,
    }
  }
}

impl core::ops::Mul for Complex {
  type Output = Complex;

  #[inline(always)]
  fn mul(self, rhs: Self) -> Self::Output {
    Self::Output {
      re: (self.re * rhs.re) - (self.im * rhs.im),
      im: (self.re * rhs.im) + (self.im * rhs.re),
    }
  }
}
