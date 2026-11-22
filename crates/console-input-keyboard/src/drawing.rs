//! Drawing primitives for the keyboard.
//!
//! The shared raster canvas borrows the compositor's buffer while a frame is
//! painted. Key fills replace pixels and labels composite over them. The
//! canvas arranges source colors for Wayland before the frame is submitted.

use crate::configuration::Color;
use console_core_fonts::{Font, Weight};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{narrow_f32, toward_zero_i32};
use console_core_path_rasterizer::{BlendMode, Color as Ink, FillRule, Paint, Rect, Transform};
use console_draw_painting::raster::{Canvas, rounded};
use console_draw_painting::text;

pub struct FontDescription {
  pub font: Font,
  pub weight: Weight,
  pub style: text::Style,
}

pub type Rectangle = console_core_geometry::Rectangle<f64>;

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the scalar representation of a distance in logical pixels"
  )
)]
pub struct LogicalPixels(pub f64);

pub fn inset(
  rectangle: Rectangle,
  LogicalPixels(border): LogicalPixels,
) -> Result<Rectangle, Never> {
  Ok(Rectangle {
    origin: Point {
      x: rectangle.origin.x + border,
      y: rectangle.origin.y + border,
    },
    size: Size {
      width: (rectangle.size.width - 2.0 * border).max(0.0),
      height: (rectangle.size.height - 2.0 * border).max(0.0),
    },
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stride(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferScale(pub i32);

pub struct Surface<'a> {
  canvas: Canvas<'a>,
}

impl<'a> Surface<'a> {
  pub fn new(
    pixels: &'a mut [u8],
    stride: Stride,
    height: i32,
    scale: BufferScale,
  ) -> Result<Option<Self>, Never> {
    let (width, height_3) = match (u32::try_from(stride.0), u32::try_from(height)) {
      (Ok(stride_2), Ok(height_2)) => (stride_2.saturating_div(4), height_2),
      _invalid_size => return Ok(None),
    };
    let Ok(scale_2) = narrow_f32(f64::from(scale.0));
    let Ok(transform) =
      Transform::from_scale_factors(::console_core_path_rasterizer::ScaleFactors {
        x: scale_2,
        y: scale_2,
      });
    let canvas = match Canvas::new(
      pixels,
      Size {
        width,
        height: height_3,
      },
      transform,
    ) {
      Ok(canvas) => canvas,
      Err(fault) => {
        console_core_logging::warn!("no surface to draw the keyboard on: {fault}");
        return Ok(None);
      }
    };

    Ok(Some(Self { canvas }))
  }

  pub fn clear(&mut self, at: Rectangle) -> Result<(), Never> {
    self.rectangle(Ink::TRANSPARENT, at, 0, BlendMode::Clear)
  }

  pub fn fill_rectangle(
    &mut self,
    color: Color,
    at: Rectangle,
    rounding: i32,
  ) -> Result<(), Never> {
    let [blue, green, red, alpha] = color.0;

    let Ok(ink) = Ink::from_rgba8(red, green, blue, alpha);

    self.rectangle(ink, at, rounding, BlendMode::Source)
  }

  fn rectangle(
    &mut self,
    color: Ink,
    at: Rectangle,
    rounding: i32,
    blend_mode: BlendMode,
  ) -> Result<(), Never> {
    let Ok(x) = narrow_f32(at.origin.x);
    let Ok(y) = narrow_f32(at.origin.y);
    let Ok(width) = narrow_f32(at.size.width);
    let Ok(height) = narrow_f32(at.size.height);
    let rect = match {
      let Ok(explicit_value) = Rect::from_xywh(x, y, width, height);
      explicit_value
    } {
      Some(rect) => rect,
      None => return Ok(()),
    };
    let Ok(radius) = narrow_f32(f64::from(rounding));
    let Ok(path) = rounded(rect, radius);
    let path_3 = match path {
      Some(path_2) => path_2,
      None => return Ok(()),
    };
    let mut paint = Paint {
      coverage: console_core_path_rasterizer::CoverageMode::Binary,
      blend_mode,
      ..Paint::default()
    };

    let Ok(color_2) = console_draw_painting::raster::buffer_color(color);
    let ::core::result::Result::Ok(()) = paint.set_color(color_2);
    let Ok(()) = self.canvas.fill_path(&path_3, &paint, FillRule::Winding);

    Ok(())
  }

  pub fn draw_text(
    &mut self,
    color: Color,
    at: Rectangle,
    border: LogicalPixels,
    label: &str,
    font: &FontDescription,
  ) -> Result<(), Never> {
    let Ok(inner) = inset(at, border);
    let Ok(across) = toward_zero_i32(inner.size.width);
    let Ok(across_2) = console_core_number_conversion::fitted::<_, u32>(across);
    let Ok(layout) = text::styled_layout(label, &font.font, font.weight, across_2, font.style);
    let Ok(measured) = layout.measured();
    let at_2 = Point {
      x: inner.origin.x + (inner.size.width - f64::from(measured.width)) / 2.0,
      y: inner.origin.y + (inner.size.height - f64::from(measured.height)) / 2.0,
    };
    let [blue, green, red, alpha] = color.0;

    let Ok(ink) = Ink::from_rgba8(red, green, blue, alpha);

    match layout.draw(&mut self.canvas, at_2, ink) {
      Ok(()) => {}
      Err(fault) => console_core_logging::warn!("keyboard: {fault}"),
    }

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  type Failure = Box<dyn std::error::Error>;

  const TEN_SQUARE: Size<f64> = Size {
    width: 10.0,
    height: 10.0,
  };

  #[test]
  fn a_key_label_keeps_the_key_background_opaque() -> Result<(), Failure> {
    let mut bytes = vec![0_u8; 16_000];
    let Ok(made) = Surface::new(&mut bytes, Stride(400), 40, BufferScale(1));
    let mut surface = made.ok_or("a surface over the test buffer")?;
    let Ok(background) = Color::from_hex("ff0000");
    let Ok(ink) = Color::from_hex("ffffff");
    let at = Rectangle {
      origin: Point { x: 0.0, y: 0.0 },
      size: Size {
        width: 100.0,
        height: 40.0,
      },
    };
    let font = FontDescription {
      font: Font {
        family: "Noto Sans".to_string(),
        height: 20,
      },
      weight: Weight::Plain,
      style: text::Style::Normal,
    };
    let Ok(()) = surface.fill_rectangle(background, at, 0);
    let Ok(()) = surface.draw_text(ink, at, LogicalPixels(2.0), "Enter", &font);

    drop(surface);

    assert!(
      bytes
        .as_chunks::<4>()
        .0
        .iter()
        .all(|pixel| pixel.get(3) == Some(&255)),
      "a label cleared its key"
    );
    assert!(
      bytes
        .as_chunks::<4>()
        .0
        .iter()
        .any(|pixel| *pixel != [0, 0, 255, 255]),
      "the label drew no text"
    );
    assert_eq!(bytes.as_chunks::<4>().0.first(), Some(&[0, 0, 255, 255]));

    Ok(())
  }

  #[test]
  fn fill_rectangle_writes_some_non_zero_pixel() -> Result<(), Failure> {
    let mut bytes = vec![0_u8; 1600];
    let Ok(made) = Surface::new(&mut bytes, Stride(80), 20, BufferScale(1));
    let mut surface = made.ok_or("a surface over the test buffer")?;
    let Ok(red) = Color::from_hex("ff0000");
    let at = Rectangle {
      origin: Point { x: 5.0, y: 5.0 },
      size: TEN_SQUARE,
    };
    let Ok(()) = surface.fill_rectangle(red, at, 0);

    drop(surface);

    assert_eq!(
      bytes.as_chunks::<4>().0.get(210),
      Some(&[0x00, 0x00, 0xff, 0xff]),
      "the pixel at ten across and ten down is red"
    );

    Ok(())
  }

  #[test]
  fn clear_makes_the_pixel_transparent() -> Result<(), Failure> {
    let mut bytes = vec![0_u8; 400];
    let Ok(made) = Surface::new(&mut bytes, Stride(40), 10, BufferScale(1));
    let mut surface = made.ok_or("a surface over the test buffer")?;
    let Ok(white) = Color::from_hex("ffffff");
    let at = Rectangle {
      origin: Point { x: 0.0, y: 0.0 },
      size: TEN_SQUARE,
    };
    let Ok(()) = surface.fill_rectangle(white, at, 0);
    let Ok(()) = surface.clear(at);

    drop(surface);

    assert!(
      bytes.iter().all(|byte| *byte == 0),
      "a pixel was left behind"
    );

    Ok(())
  }

  #[test]
  fn insetting_past_the_middle_gives_nothing_rather_than_a_backwards_rectangle() {
    let cell = Rectangle {
      origin: Point { x: 10.0, y: 10.0 },
      size: Size {
        width: 8.0,
        height: 4.0,
      },
    };
    let Ok(inner) = inset(cell, LogicalPixels(6.0));

    assert_eq!(inner.size.width, 0.0, "the width went backwards: {inner:?}");
    assert_eq!(
      inner.size.height, 0.0,
      "the height went backwards: {inner:?}"
    );
    assert!(
      inner.origin.x >= cell.origin.x && inner.origin.y >= cell.origin.y,
      "the corner moved out: {inner:?}"
    );
  }

  #[test]
  fn an_inset_comes_off_both_sides() {
    let rect = Rectangle {
      origin: Point { x: 0.0, y: 0.0 },
      size: Size {
        width: 10.0,
        height: 6.0,
      },
    };

    assert_eq!(
      inset(rect, LogicalPixels(1.0)),
      Ok(Rectangle {
        origin: Point { x: 1.0, y: 1.0 },
        size: Size {
          width: 8.0,
          height: 4.0
        },
      })
    );
  }

  #[test]
  fn color_from_hex_round_trips_through_a_red_pixel() {
    assert_eq!(
      Color::from_hex("deadbe"),
      Ok(Color([0xbe, 0xad, 0xde, 0xff]))
    );
  }
}
