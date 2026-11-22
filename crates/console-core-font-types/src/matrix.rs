//! An affine transform, as the six numbers of a CFF or Type 1 font matrix and of a variable
//! composite glyph's transform: the two columns of a linear map and a translation, in the order
//! `xx yx xy yy dx dy`.
//!
//! Multiplying two applies the right one first, as a matrix product does, and multiplying a point
//! moves it. The elements are any [`MatrixElement`] -- a 16.16 number as the font writes it, or a
//! float once a caller has left the font's numbers behind.

use super::{Fixed, Point};
use console_core_never::Never;
use core::ops::{Add, Mul, MulAssign};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bytemuck", derive(bytemuck::AnyBitPattern))]
#[repr(C)]
pub struct Matrix<T> {
  pub xx: T,
  pub yx: T,
  pub xy: T,
  pub yy: T,
  pub dx: T,
  pub dy: T,
}

impl<T: MatrixElement> Default for Matrix<T> {
  fn default() -> Self {
    Self::IDENTITY
  }
}

impl<T: Copy> Matrix<T> {
  pub const fn from_elements(elements: [T; 6]) -> Result<Self, Never> {
    let [xx, yx, xy, yy, dx, dy] = elements;

    Ok(Self {
      xx,
      yx,
      xy,
      yy,
      dx,
      dy,
    })
  }

  pub const fn elements(&self) -> Result<[T; 6], Never> {
    Ok([self.xx, self.yx, self.xy, self.yy, self.dx, self.dy])
  }

  #[inline(always)]
  pub fn map<U: Copy>(self, f: impl FnMut(T) -> U) -> Result<Matrix<U>, Never> {
    let Ok(elements) = self.elements();

    Matrix::from_elements(elements.map(f))
  }
}

impl<T: MatrixElement> Matrix<T> {
  pub const IDENTITY: Self = Self {
    xx: T::ONE,
    yx: T::ZERO,
    xy: T::ZERO,
    yy: T::ONE,
    dx: T::ZERO,
    dy: T::ZERO,
  };

  pub fn transform(&self, x: T, y: T) -> Result<(T, T), Never> {
    Ok((
      self.xx * x + self.xy * y + self.dx,
      self.yx * x + self.yy * y + self.dy,
    ))
  }
}

impl<T: MatrixElement> Mul for Matrix<T> {
  type Output = Self;

  fn mul(self, rhs: Self) -> Self::Output {
    let first_row = [self.xx, self.xy];
    let second_row = [self.yx, self.yy];
    let first_column = [rhs.xx, rhs.yx];
    let second_column = [rhs.xy, rhs.yy];
    let translation = [rhs.dx, rhs.dy];
    let Ok(xx) = dot(first_row, first_column);
    let Ok(yx) = dot(second_row, first_column);
    let Ok(xy) = dot(first_row, second_column);
    let Ok(yy) = dot(second_row, second_column);
    let Ok(dx) = dot(first_row, translation);
    let Ok(dy) = dot(second_row, translation);

    Self {
      xx,
      yx,
      xy,
      yy,
      dx: dx + self.dx,
      dy: dy + self.dy,
    }
  }
}

fn dot<T: MatrixElement>([first, second]: [T; 2], [third, fourth]: [T; 2]) -> Result<T, Never> {
  Ok(first * third + second * fourth)
}

impl<T: MatrixElement> MulAssign for Matrix<T> {
  fn mul_assign(&mut self, rhs: Self) {
    *self = *self * rhs;
  }
}

impl<T: MatrixElement> Mul<Point<T>> for Matrix<T> {
  type Output = Point<T>;

  fn mul(self, rhs: Point<T>) -> Self::Output {
    let Ok((x, y)) = self.transform(rhs.x, rhs.y);

    Point { x, y }
  }
}

pub trait MatrixElement: Copy + Add<Output = Self> + Mul<Output = Self> {
  const ZERO: Self;
  const ONE: Self;
}

impl MatrixElement for Fixed {
  const ONE: Self = Fixed::ONE;
  const ZERO: Self = Fixed::ZERO;
}

impl MatrixElement for f32 {
  const ONE: Self = 1.0;
  const ZERO: Self = 0.0;
}

impl MatrixElement for f64 {
  const ONE: Self = 1.0;
  const ZERO: Self = 0.0;
}
