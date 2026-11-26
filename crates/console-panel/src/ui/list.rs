//! A title and optional detail in a selectable list.

use console_core_fonts::TextStyle;
use console_core_geometry::{Point, Size};
use console_core_never::Never;

use crate::canvas::{Align, Area, Canvas, Face, RowColors};

pub struct Label<'a> {
  pub title: &'a str,
  pub detail: &'a str,
}

pub fn label<A: Copy + PartialEq>(
  canvas: &mut Canvas<A>,
  area: Area,
  label: Label<'_>,
  colors: &RowColors,
) -> Result<(), Never> {
  let Ok(headline) = TextStyle::Headline.height();

  match label.detail {
    "" => canvas.text(
      area,
      label.title,
      headline,
      Face::Medium,
      colors.ink,
      Align::Left,
    ),
    detail => {
      let height = area.size.height.saturating_div(2);
      let upper = Area {
        size: Size {
          height,
          ..area.size
        },
        ..area
      };
      let lower = Area {
        origin: Point {
          y: area.origin.y.saturating_add(height),
          ..area.origin
        },
        ..upper
      };
      let Ok(footnote) = TextStyle::Footnote.height();
      let Ok(()) = canvas.text(
        upper,
        label.title,
        headline,
        Face::Medium,
        colors.ink,
        Align::Left,
      );

      canvas.text(
        lower,
        detail,
        footnote,
        Face::Body,
        colors.quiet,
        Align::Left,
      )
    }
  }
}
