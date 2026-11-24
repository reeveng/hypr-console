//! Four independent edge offsets in the rectangle's coordinate units.
//!
//! `Insets::uniform` repeats one offset on each edge. Positive offsets move
//! edges inwards with `Rectangle::inset` and outwards with
//! `Bounds::expanded_by`. `Bounds::expansion_from` recovers edge offsets
//! between two bounds in their original coordinate order.
//! The integer inset uses wrapping arithmetic, including when an inset
//! leaves a rectangle with negative extents.

use console_core_never::Never;
use console_core_point::Point;
use console_core_size::Size;

use crate::{Bounds, Rectangle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Insets<T> {
  pub left: T,
  pub top: T,
  pub right: T,
  pub bottom: T,
}

impl<T: Copy> Insets<T> {
  pub const fn uniform(edge: T) -> Result<Self, Never> {
    Ok(Self {
      left: edge,
      top: edge,
      right: edge,
      bottom: edge,
    })
  }

  pub const fn by_axis(axes: Size<T>) -> Result<Self, Never> {
    Ok(Self {
      left: axes.width,
      top: axes.height,
      right: axes.width,
      bottom: axes.height,
    })
  }
}

impl<T: Default> Insets<T> {
  pub fn none() -> Result<Self, Never> {
    Ok(Self::default())
  }
}

impl Insets<f64> {
  pub const fn size(self) -> Result<Size<f64>, Never> {
    Ok(Size {
      width: self.left + self.right,
      height: self.top + self.bottom,
    })
  }

  pub const fn nonnegative(self) -> Result<Self, Never> {
    Ok(Self {
      left: self.left.max(0.0),
      top: self.top.max(0.0),
      right: self.right.max(0.0),
      bottom: self.bottom.max(0.0),
    })
  }

  pub const fn min(self, other: Self) -> Result<Self, Never> {
    Ok(Self {
      left: self.left.min(other.left),
      top: self.top.min(other.top),
      right: self.right.min(other.right),
      bottom: self.bottom.min(other.bottom),
    })
  }

  pub const fn max(self, other: Self) -> Result<Self, Never> {
    Ok(Self {
      left: self.left.max(other.left),
      top: self.top.max(other.top),
      right: self.right.max(other.right),
      bottom: self.bottom.max(other.bottom),
    })
  }
}

impl Bounds<f64> {
  pub const fn expanded_by(self, edges: Insets<f64>) -> Result<Self, Never> {
    let Ok(bounds) = self.normalized();
    Ok(Self {
      x0: bounds.x0 - edges.left,
      y0: bounds.y0 - edges.top,
      x1: bounds.x1 + edges.right,
      y1: bounds.y1 + edges.bottom,
    })
  }

  pub const fn expansion_from(self, base: Self) -> Result<Insets<f64>, Never> {
    Ok(Insets {
      left: base.x0 - self.x0,
      top: base.y0 - self.y0,
      right: self.x1 - base.x1,
      bottom: self.y1 - base.y1,
    })
  }
}

impl Rectangle<i32> {
  pub const fn inset(self, edges: Insets<i32>) -> Result<Self, Never> {
    Ok(Self {
      origin: Point {
        x: self.origin.x.wrapping_add(edges.left),
        y: self.origin.y.wrapping_add(edges.top),
      },
      size: Size {
        width: self
          .size
          .width
          .wrapping_sub(edges.left.wrapping_add(edges.right)),
        height: self
          .size
          .height
          .wrapping_sub(edges.top.wrapping_add(edges.bottom)),
      },
    })
  }
}
