//! Points, displacements, and rectangles with named coordinates and dimensions.
//!
//! A [`Point`] carries `x` and `y` in the caller's units, such as logical or
//! device pixels. A [`Rectangle`] combines a point as its origin with a
//! [`Size`] measured in the same units. Both are re-exported from their
//! shared crates. A [`Vector2`] carries a change in position and [`Insets`]
//! carry one offset for each rectangle edge.

#![no_std]

pub use console_core_point::Point;
pub use console_core_rectangle::{
  Bounds, Containment, Coordinate, CropMinimum, Insets, Rectangle,
};
pub use console_core_size::Size;
pub use console_core_triangle::Triangle;
pub use console_core_vector::Vector2;
