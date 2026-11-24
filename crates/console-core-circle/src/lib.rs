//! Circles with their center and radius in the caller's coordinate units.
//!
//! A signed radius has the same geometric extent as its absolute value.
//! Containment uses the open interior, excluding the circumference.

#![no_std]
#![forbid(unsafe_code)]

use core::f64::consts::PI;

use console_core_never::Never;
use console_core_point::Point;
use console_core_rectangle::{Bounds, Containment};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Circle<T> {
  pub center: Point<T>,
  pub radius: T,
}

impl<T: Copy> Circle<T> {
  pub const fn new(center: Point<T>, radius: T) -> Result<Self, Never> {
    Ok(Self { center, radius })
  }
}

impl Circle<f64> {
  pub const fn area(&self) -> Result<f64, Never> {
    Ok(PI * (self.radius * self.radius))
  }

  pub const fn perimeter(&self) -> Result<f64, Never> {
    Ok((2.0 * PI * self.radius).abs())
  }

  pub fn contains(&self, point: Point<f64>) -> Result<Containment, Never> {
    let Ok(distance_squared) = point.distance_squared(self.center);
    let radius_squared = self.radius * self.radius;
    Ok(match (distance_squared < radius_squared).then_some(()) {
      Some(()) => Containment::Inside,
      None => Containment::Outside,
    })
  }

  pub const fn bounds(&self) -> Result<Bounds<f64>, Never> {
    let radius = self.radius.abs();
    let Point { x, y } = self.center;
    Ok(Bounds {
      x0: x - radius,
      y0: y - radius,
      x1: x + radius,
      y1: y + radius,
    })
  }
}
