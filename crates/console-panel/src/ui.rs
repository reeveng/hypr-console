//! The Console UI used by applications that draw a window of their own.
//!
//! `strip` places titles, tabs and buttons with typed actions for input.
//! `field` supplies single-line and multiline editors, text selection and
//! clipboard actions. `list` lays out
//! a row's title and detail. `theme` provides colors by role. Painting and
//! surface primitives support custom content such as documents and previews.

pub mod field;
pub mod list;
pub mod theme;

pub use crate::{activity, canvas, icons, marks, page, storefront, strip, window};

pub mod shapes {
  pub use console_core_shapes::*;
}

pub mod painting {
  pub use console_draw_painting::*;
}

pub mod surface {
  pub use console_draw_surface::*;
}
