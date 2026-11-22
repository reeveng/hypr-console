//! The smallest upright rectangle holding an outline, as its least and greatest `x` and `y`.
//!
//! A box's corners are in order when each least is no greater than its greatest, which is the
//! [`CornerOrder`] it answers, and scaling one by a negative factor puts them out of order, which
//! `normalize` puts right by swapping the ends that crossed.

use console_core_never::Never;
use core::ops::Mul;

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BoundingBox<T> {
  pub x_min: T,
  pub y_min: T,
  pub x_max: T,
  pub y_max: T,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CornerOrder {
  InOrder,
  OutOfOrder,
}

impl From<bool> for CornerOrder {
  fn from(in_order: bool) -> Self {
    match in_order.then_some(()) {
      Some(()) => Self::InOrder,
      None => Self::OutOfOrder,
    }
  }
}

impl<T> BoundingBox<T>
where
  T: Mul<Output = T> + Copy,
{
  pub fn scale(&self, factor: T) -> Result<Self, Never> {
    Ok(Self {
      x_min: self.x_min * factor,
      y_min: self.y_min * factor,
      x_max: self.x_max * factor,
      y_max: self.y_max * factor,
    })
  }
}

impl<T> BoundingBox<T>
where
  T: PartialOrd + Copy,
{
  pub fn corner_order(&self) -> Result<CornerOrder, Never> {
    Ok(CornerOrder::from(
      (self.x_min <= self.x_max) & (self.y_min <= self.y_max),
    ))
  }

  pub fn normalize(&self) -> Result<Self, Never> {
    let (x_min, x_max) = match CornerOrder::from(self.x_min <= self.x_max) {
      CornerOrder::InOrder => (self.x_min, self.x_max),
      CornerOrder::OutOfOrder => (self.x_max, self.x_min),
    };
    let (y_min, y_max) = match CornerOrder::from(self.y_min <= self.y_max) {
      CornerOrder::InOrder => (self.y_min, self.y_max),
      CornerOrder::OutOfOrder => (self.y_max, self.y_min),
    };

    Ok(Self {
      x_min,
      y_min,
      x_max,
      y_max,
    })
  }
}
