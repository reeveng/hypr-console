//! A point, or the vector between two: an `x` and a `y` of whatever a caller measures in.
//!
//! The arithmetic is each coordinate's own, so a point of 26.6 numbers wraps where they wrap and a
//! point of floats rounds where they round; a point times a single number scales both
//! coordinates.

use console_core_never::Never;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bytemuck", derive(bytemuck::AnyBitPattern))]
#[repr(C)]
pub struct Point<T> {
  pub x: T,
  pub y: T,
}

// SAFETY: `Point<T>` is `repr(C)` with two fields of the same `T` and nothing between them, so
// when `T` has no uninitialized bytes and no padding, neither does a pair of them.
#[cfg(feature = "bytemuck")]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "repr(C) gives the pair of identical field types no inter-field or trailing padding, and each field guarantees NoUninit"
  )
)]
unsafe impl<T> bytemuck::NoUninit for Point<T> where T: bytemuck::NoUninit {}

impl<T> Point<T> {
  #[inline(always)]
  pub const fn new(x: T, y: T) -> Result<Self, Never> {
    Ok(Self { x, y })
  }

  pub const fn broadcast(xy: T) -> Result<Self, Never>
  where
    T: Copy,
  {
    Ok(Self { x: xy, y: xy })
  }

  #[inline(always)]
  pub fn map<U>(self, mut f: impl FnMut(T) -> U) -> Result<Point<U>, Never> {
    Ok(Point {
      x: f(self.x),
      y: f(self.y),
    })
  }
}

impl<T> Add for Point<T>
where
  T: Add<Output = T>,
{
  type Output = Self;

  #[inline(always)]
  fn add(self, rhs: Self) -> Self::Output {
    Self {
      x: self.x + rhs.x,
      y: self.y + rhs.y,
    }
  }
}

impl<T> AddAssign for Point<T>
where
  T: AddAssign,
{
  #[inline(always)]
  fn add_assign(&mut self, rhs: Self) {
    self.x += rhs.x;
    self.y += rhs.y;
  }
}

impl<T> Sub for Point<T>
where
  T: Sub<Output = T>,
{
  type Output = Self;

  #[inline(always)]
  fn sub(self, rhs: Self) -> Self::Output {
    Self {
      x: self.x - rhs.x,
      y: self.y - rhs.y,
    }
  }
}

impl<T> SubAssign for Point<T>
where
  T: SubAssign,
{
  #[inline(always)]
  fn sub_assign(&mut self, rhs: Self) {
    self.x -= rhs.x;
    self.y -= rhs.y;
  }
}

impl<T> Mul for Point<T>
where
  T: Mul<Output = T>,
{
  type Output = Self;

  #[inline(always)]
  fn mul(self, rhs: Self) -> Self::Output {
    Self {
      x: self.x * rhs.x,
      y: self.y * rhs.y,
    }
  }
}

impl<T> Mul<T> for Point<T>
where
  T: Mul<Output = T> + Copy,
{
  type Output = Self;

  #[inline(always)]
  fn mul(self, rhs: T) -> Self::Output {
    Self {
      x: self.x * rhs,
      y: self.y * rhs,
    }
  }
}

impl<T> MulAssign for Point<T>
where
  T: MulAssign,
{
  #[inline(always)]
  fn mul_assign(&mut self, rhs: Self) {
    self.x *= rhs.x;
    self.y *= rhs.y;
  }
}

impl<T> MulAssign<T> for Point<T>
where
  T: MulAssign + Copy,
{
  #[inline(always)]
  fn mul_assign(&mut self, rhs: T) {
    self.x *= rhs;
    self.y *= rhs;
  }
}

impl<T> Div for Point<T>
where
  T: Div<Output = T>,
{
  type Output = Self;

  #[inline(always)]
  fn div(self, rhs: Self) -> Self::Output {
    Self {
      x: self.x / rhs.x,
      y: self.y / rhs.y,
    }
  }
}

impl<T> Div<T> for Point<T>
where
  T: Div<Output = T> + Copy,
{
  type Output = Self;

  #[inline(always)]
  fn div(self, rhs: T) -> Self::Output {
    Self {
      x: self.x / rhs,
      y: self.y / rhs,
    }
  }
}

impl<T> DivAssign for Point<T>
where
  T: DivAssign,
{
  #[inline(always)]
  fn div_assign(&mut self, rhs: Self) {
    self.x /= rhs.x;
    self.y /= rhs.y;
  }
}

impl<T> DivAssign<T> for Point<T>
where
  T: DivAssign + Copy,
{
  #[inline(always)]
  fn div_assign(&mut self, rhs: T) {
    self.x /= rhs;
    self.y /= rhs;
  }
}

impl<T> Neg for Point<T>
where
  T: Neg<Output = T>,
{
  type Output = Self;

  #[inline(always)]
  fn neg(self) -> Self::Output {
    Self {
      x: -self.x,
      y: -self.y,
    }
  }
}

impl<T> From<console_core_point::Point<T>> for Point<T> {
  fn from(point: console_core_point::Point<T>) -> Self {
    Self {
      x: point.x,
      y: point.y,
    }
  }
}

impl<T> From<Point<T>> for console_core_point::Point<T> {
  fn from(point: Point<T>) -> Self {
    Self {
      x: point.x,
      y: point.y,
    }
  }
}
