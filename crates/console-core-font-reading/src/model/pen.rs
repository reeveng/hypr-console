//! Types for collecting the output when drawing a glyph outline.

use alloc::{string::String, vec::Vec};
use core::fmt::{self, Write};
use types::BoundingBox;

pub trait OutlinePen {
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(explicit067_no_unit_less_quantity, reason = "the published OutlinePen move callback accepts f32 contour coordinates in the caller drawing space")
  )]
  fn move_to(&mut self, x: f32, y: f32);

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(explicit067_no_unit_less_quantity, reason = "the published OutlinePen line callback accepts f32 contour coordinates in the caller drawing space")
  )]
  fn line_to(&mut self, x: f32, y: f32);

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(explicit067_no_unit_less_quantity, reason = "the published OutlinePen quadratic callback accepts f32 control point and endpoint coordinates")
  )]
  fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32);

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(explicit067_no_unit_less_quantity, reason = "the published OutlinePen cubic callback accepts two f32 control points and an endpoint")
  )]
  fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32);

  fn close(&mut self);
}

#[derive(Copy, Clone, PartialEq, PartialOrd, Debug)]
pub enum PathElement {
  MoveTo {
    x: f32,
    y: f32,
  },
  LineTo {
    x: f32,
    y: f32,
  },
  QuadTo {
    cx0: f32,
    cy0: f32,
    x: f32,
    y: f32,
  },
  CurveTo {
    cx0: f32,
    cy0: f32,
    cx1: f32,
    cy1: f32,
    x: f32,
    y: f32,
  },
  Close,
}

impl OutlinePen for Vec<PathElement> {
  fn move_to(&mut self, x: f32, y: f32) {
    self.push(PathElement::MoveTo { x, y })
  }

  fn line_to(&mut self, x: f32, y: f32) {
    self.push(PathElement::LineTo { x, y })
  }

  fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
    self.push(PathElement::QuadTo { cx0, cy0, x, y })
  }

  fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
    self.push(PathElement::CurveTo {
      cx0,
      cy0,
      cx1,
      cy1,
      x,
      y,
    })
  }

  fn close(&mut self) {
    self.push(PathElement::Close)
  }
}

pub struct NullPen;

impl OutlinePen for NullPen {
  fn move_to(&mut self, _x: f32, _y: f32) {}

  fn line_to(&mut self, _x: f32, _y: f32) {}

  fn quad_to(&mut self, _cx0: f32, _cy0: f32, _x: f32, _y: f32) {}

  fn curve_to(&mut self, _cx0: f32, _cy0: f32, _cx1: f32, _cy1: f32, _x: f32, _y: f32) {}

  fn close(&mut self) {}
}

#[derive(Clone, Default, Debug)]
pub struct SvgPen(String, Option<u64>);

impl SvgPen {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `SvgPen::new` method supports direct construction or chaining with its existing Self return")
  )]
  pub fn new() -> Self {
    Self::default()
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `SvgPen::with_precision` method supports direct construction or chaining with its existing Self return")
  )]
  pub fn with_precision(precision: u64) -> Self {
    Self(String::default(), Some(precision))
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `SvgPen::clear` method is called through its existing unit-returning signature")
  )]
  pub fn clear(&mut self) {
    self.0.clear();
  }

  fn maybe_push_space(&mut self) -> core::result::Result<(), console_core_never::Never> {
    {
      match ::core::primitive::bool::then_some(!self.0.is_empty(), ()) {
        ::core::option::Option::Some(()) => {
          self.0.push(' ');
        }
        ::core::option::Option::None => {}
      }
    };

    Ok(())
  }
}

impl core::ops::Deref for SvgPen {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    self.0.as_str()
  }
}

impl OutlinePen for SvgPen {
  fn move_to(&mut self, x: f32, y: f32) {
    let Ok(()) = self.maybe_push_space();
    let _primitive_text_written = match self.1 {
      Some(prec) => {
        let Ok(prec_2) = console_core_number_conversion::index(prec);
        write!(self.0, "M{x:.0$},{y:.0$}", prec_2)
      }
      None => {
        write!(self.0, "M{x},{y}")
      }
    };
  }

  fn line_to(&mut self, x: f32, y: f32) {
    let Ok(()) = self.maybe_push_space();
    let _primitive_text_written = match self.1 {
      Some(prec) => {
        let Ok(prec_2) = console_core_number_conversion::index(prec);
        write!(self.0, "L{x:.0$},{y:.0$}", prec_2)
      }
      None => {
        write!(self.0, "L{x},{y}")
      }
    };
  }

  fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
    let Ok(()) = self.maybe_push_space();
    let _primitive_text_written = match self.1 {
      Some(prec) => {
        let Ok(prec_2) = console_core_number_conversion::index(prec);
        write!(self.0, "Q{cx0:.0$},{cy0:.0$} {x:.0$},{y:.0$}", prec_2)
      }
      None => {
        write!(self.0, "Q{cx0},{cy0} {x},{y}")
      }
    };
  }

  fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
    let Ok(()) = self.maybe_push_space();
    let _primitive_text_written = match self.1 {
      Some(prec) => {
        let Ok(prec_2) = console_core_number_conversion::index(prec);
        write!(
          self.0,
          "C{cx0:.0$},{cy0:.0$} {cx1:.0$},{cy1:.0$} {x:.0$},{y:.0$}",
          prec_2
        )
      }
      None => {
        write!(self.0, "C{cx0},{cy0} {cx1},{cy1} {x},{y}")
      }
    };
  }

  fn close(&mut self) {
    let Ok(()) = self.maybe_push_space();
    self.0.push('Z');
  }
}

impl AsRef<str> for SvgPen {
  fn as_ref(&self) -> &str {
    self.0.as_ref()
  }
}

impl From<String> for SvgPen {
  fn from(value: String) -> Self {
    Self(value, None)
  }
}

impl From<SvgPen> for String {
  fn from(value: SvgPen) -> Self {
    value.0
  }
}

impl fmt::Display for SvgPen {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.0)
  }
}

#[derive(Clone, Default, Debug)]
pub struct ControlBoundsPen(Option<BoundingBox<f32>>);

impl ControlBoundsPen {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `ControlBoundsPen::new` method supports direct construction or chaining with its existing Self return")
  )]
  pub fn new() -> Self {
    Self(None)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `ControlBoundsPen::bounding_box` method returns the optional value directly to callers that match its presence")
  )]
  pub fn bounding_box(&self) -> Option<BoundingBox<f32>> {
    self.0
  }

  fn update_bounds(
    &mut self,
    crate::types::Point { x, y }: crate::types::Point<f32>,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      match &mut self.0 {
        Some(bb) => {
          bb.x_min = bb.x_min.min(x);
          bb.y_min = bb.y_min.min(y);
          bb.x_max = bb.x_max.max(x);
          bb.y_max = bb.y_max.max(y);
        }
        None => {
          self.0 = Some(BoundingBox {
            x_min: x,
            y_min: y,
            x_max: x,
            y_max: y,
          });
        }
      }
    };

    Ok(())
  }
}

impl OutlinePen for ControlBoundsPen {
  fn move_to(&mut self, x: f32, y: f32) {
    let Ok(()) = self.update_bounds(crate::types::Point { x: x, y: y });
  }

  fn line_to(&mut self, x: f32, y: f32) {
    let Ok(()) = self.update_bounds(crate::types::Point { x: x, y: y });
  }

  fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
    let Ok(()) = self.update_bounds(crate::types::Point { x: cx0, y: cy0 });
    let Ok(()) = self.update_bounds(crate::types::Point { x: x, y: y });
  }

  fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
    let Ok(()) = self.update_bounds(crate::types::Point { x: cx0, y: cy0 });
    let Ok(()) = self.update_bounds(crate::types::Point { x: cx1, y: cy1 });
    let Ok(()) = self.update_bounds(crate::types::Point { x: x, y: y });
  }

  fn close(&mut self) {}
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn svg_pen_precision() {
    let svg_data = [None, Some(1), Some(4)].map(|prec| {
      let mut pen = match prec {
        None => SvgPen::new(),
        Some(prec_2) => SvgPen::with_precision(prec_2),
      };
      pen.move_to(1.0, 2.45556);
      pen.line_to(1.2, 4.0);
      pen.quad_to(2.0345, 3.56789, -0.157, -425.07);
      pen.curve_to(-37.0010, 4.5, 2.0, 1.0, -0.5, -0.25);
      pen.close();
      pen.to_string()
    });

    let expected = [
      "M1,2.45556 L1.2,4 Q2.0345,3.56789 -0.157,-425.07 C-37.001,4.5 2,1 -0.5,-0.25 Z",
      "M1.0,2.5 L1.2,4.0 Q2.0,3.6 -0.2,-425.1 C-37.0,4.5 2.0,1.0 -0.5,-0.2 Z",
      "M1.0000,2.4556 L1.2000,4.0000 Q2.0345,3.5679 -0.1570,-425.0700 C-37.0010,4.5000 2.0000,1.0000 -0.5000,-0.2500 Z",
    ];
    for (result, expected_2) in svg_data.iter().zip(&expected) {
      assert_eq!(result, expected_2);
    }
  }
}
