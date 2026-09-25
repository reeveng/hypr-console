//! Drawing primitives for the keyboard.
//!
//! The C version (`drw.c`) is a Cairo surface that writes into a wl_shm_pool
//! buffer, plus the Wayland protocol glue that attaches the buffer to a
//! wl_surface each frame. The protocol glue belongs in `main` — it is
//! about talking to the compositor — but the painting itself does not
//! change, and that is what this module holds.
//!
//! `Surface` here is a thin wrapper over a Cairo image surface: the pixel
//! buffer is owned by the caller (the Wayland shm pool in `main`), the
//! drawing primitives write into it, and the buffer is then handed to the
//! compositor. The C version's double-buffering, damage tracking, and frame
//! callbacks are also `main`'s concern.
//!
//! The three operations the keyboard needs are `clear`, `fill_rectangle` and
//! `draw_text`, and `fill` paints with SOURCE: it replaces the pixel rather
//! than compositing onto it. There was a fourth, `over_rectangle`, which
//! alpha-composited; the two things the C composited were the highlight and the
//! swipe trail, the port draws neither, and it went with them. Every color
//! here is opaque and every key covers its own cell, which is what lets the
//! whole strip be painted first and drawn over.


use crate::configuration::Color;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::toward_zero_i32;
use cairo::{Context, Format, ImageSurface};
use pango::FontDescription;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rectangle {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rectangle {
    pub const fn new(at: Point<f64>, size: Size<f64>) -> Result<Self, Never> {
        Ok(Self { x: at.x, y: at.y, width: size.width, height: size.height })
    }

    pub fn inset(self, border: f64) -> Result<Self, Never> {
        Ok(Self {
            x: self.x + border,
            y: self.y + border,
            width: (self.width - 2.0 * border).max(0.0),
            height: (self.height - 2.0 * border).max(0.0),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stride(pub i32);

pub struct Surface {
    pub cairo: Context,
    pub layout: pango::Layout,
    pub scale: f64,
}

impl Surface {
    pub fn new(
        pixels: &mut [u8],
        stride: Stride,
        height: i32,
        scale: f64,
    ) -> Result<Option<Self>, Never> {
        // SAFETY: Cairo does not mutate `pixels` until we draw into the
        let made = unsafe {
            ImageSurface::create_for_data_unsafe(
                pixels.as_mut_ptr(),
                Format::ARgb32,
                stride.0.saturating_div(4),
                height,
                stride.0,
            )
        };
        let image = match made {
            Ok(image) => image,
            Err(fault) => {
                eprintln!("no surface to draw the keyboard on: {fault}");
                return Ok(None);
            },
        };
        let cairo = match Context::new(&image) {
            Ok(cairo) => cairo,
            Err(fault) => {
                eprintln!("nothing to draw the keyboard with: {fault}");
                return Ok(None);
            },
        };
        cairo.scale(scale, scale);
        cairo.set_antialias(cairo::Antialias::None);
        let layout = pango::Layout::new(&pangocairo::functions::create_context(&cairo));
        layout.set_auto_dir(false);
        Ok(Some(Surface {
            cairo,
            layout,
            scale,
        }))
    }

    pub fn clear(&self, at: Rectangle) -> Result<(), Never> {
        let _ = self.cairo.save();
        self.cairo.set_operator(cairo::Operator::Clear);
        self.cairo.rectangle(at.x, at.y, at.width, at.height);
        let _ = self.cairo.fill();
        let _ = self.cairo.restore();

        Ok(())
    }

    fn trace(&self, at: Rectangle, rounding: i32) -> Result<(), Never> {
        let Rectangle { x, y, width, height } = at;

        match rounding <= 0 {
            true => {
                self.cairo.rectangle(x, y, width, height);
                return Ok(());
            }
            false => {},
        }

        let radius = f64::from(rounding);
        let pi = std::f64::consts::PI;
        self.cairo.new_sub_path();
        self.cairo.arc(x + width - radius, y + radius, radius, -pi / 2.0, 0.0);
        self.cairo.arc(x + width - radius, y + height - radius, radius, 0.0, pi / 2.0);
        self.cairo.arc(x + radius, y + height - radius, radius, pi / 2.0, pi);
        self.cairo.arc(x + radius, y + radius, radius, pi, 3.0 * pi / 2.0);
        self.cairo.close_path();

        Ok(())
    }

    pub fn fill_rectangle(&self, color: Color, at: Rectangle, rounding: i32) -> Result<(), Never> {
        let _ = self.cairo.save();
        self.cairo.set_operator(cairo::Operator::Source);
        let Ok(()) = self.set_source(color);
        let Ok(()) = self.trace(at, rounding);
        let _ = self.cairo.fill();
        let _ = self.cairo.restore();

        Ok(())
    }

    pub fn draw_text(
        &self,
        color: Color,
        at: Rectangle,
        border: f64,
        label: &str,
        font: &FontDescription,
    ) -> Result<(), Never> {
        let _ = self.cairo.save();
        let Ok(()) = self.set_source(color);
        self.layout.set_font_description(Some(font));
        self.layout.set_text(label);
        let (text_width, text_height) = self.layout.pixel_size();
        let offset_x = (at.width - f64::from(text_width)) / 2.0;
        let offset_y = (at.height - f64::from(text_height)) / 2.0;
        let Ok(inner) = at.inset(border);
        let Ok(across) = toward_zero_i32(inner.width);
        let Ok(down) = toward_zero_i32(inner.height);

        self.layout.set_width(across.saturating_mul(pango::SCALE));
        self.layout.set_height(down.saturating_mul(pango::SCALE));
        self.cairo.move_to(inner.x + offset_x, inner.y + offset_y);
        pangocairo::functions::show_layout(&self.cairo, &self.layout);
        let _ = self.cairo.restore();

        Ok(())
    }

    fn set_source(&self, color: Color) -> Result<(), Never> {
        let [blue, green, red, alpha] = color.0;
        self.cairo.set_source_rgba(
            f64::from(red) / 255.0,
            f64::from(green) / 255.0,
            f64::from(blue) / 255.0,
            f64::from(alpha) / 255.0,
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn buffer(width: i32, height: i32) -> RefCell<Vec<u8>> {
        RefCell::new(vec![0; (width * 4 * height).try_into().unwrap()])
    }

    #[test]
    fn fill_rectangle_writes_some_non_zero_pixel() {
        let buffer = buffer(20, 20);
        {
            let mut bytes = buffer.borrow_mut();
            let surface = match Surface::new(&mut bytes, Stride(20 * 4), 20, 1.0) {
                Ok(Some(surface)) => surface,
                Ok(None) | Err(_) => panic!("a surface over the test buffer"),
            };
            let Ok(red) = Color::from_hex("ff0000");
            let Ok(at) = Rectangle::new(Point { x: 5.0, y: 5.0 }, Size { width: 10.0, height: 10.0 });
            let Ok(()) = surface.fill_rectangle(red, at, 0);
        }
        let bytes = buffer.borrow();
        assert_eq!(bytes[10 * 20 * 4 + 10 * 4], 0x00);
        assert_eq!(bytes[10 * 20 * 4 + 10 * 4 + 1], 0x00);
        assert_eq!(bytes[10 * 20 * 4 + 10 * 4 + 2], 0xff);
        assert_eq!(bytes[10 * 20 * 4 + 10 * 4 + 3], 0xff);
    }

    #[test]
    fn clear_makes_the_pixel_transparent() {
        let buffer = buffer(10, 10);
        {
            let mut bytes = buffer.borrow_mut();
            let surface = match Surface::new(&mut bytes, Stride(10 * 4), 10, 1.0) {
                Ok(Some(surface)) => surface,
                Ok(None) | Err(_) => panic!("a surface over the test buffer"),
            };
            let Ok(white) = Color::from_hex("ffffff");
            let Ok(at) = Rectangle::new(Point { x: 0.0, y: 0.0 }, Size { width: 10.0, height: 10.0 });
            let Ok(()) = surface.fill_rectangle(white, at, 0);
            let Ok(()) = surface.clear(at);
        }
        let bytes = buffer.borrow();
        for byte in bytes.iter() {
            assert_eq!(*byte, 0);
        }
    }

    #[test]
    fn insetting_past_the_middle_gives_nothing_rather_than_a_backwards_rectangle() {
        let Ok(cell) = Rectangle::new(Point { x: 10.0, y: 10.0 }, Size { width: 8.0, height: 4.0 });
        let Ok(inner) = cell.inset(6.0);
        assert_eq!(inner.width, 0.0, "the width went backwards: {inner:?}");
        assert_eq!(inner.height, 0.0, "the height went backwards: {inner:?}");
        assert!(inner.x >= cell.x && inner.y >= cell.y, "the corner moved out: {inner:?}");
    }

    #[test]
    fn an_inset_comes_off_both_sides() {
        let Ok(rect) = Rectangle::new(Point { x: 0.0, y: 0.0 }, Size { width: 10.0, height: 6.0 });
        let Ok(inner) = rect.inset(1.0);
        assert_eq!(Ok(inner), Rectangle::new(Point { x: 1.0, y: 1.0 }, Size { width: 8.0, height: 4.0 }));
    }

    #[test]
    fn color_from_hex_round_trips_through_a_red_pixel() {
        let Ok(color) = Color::from_hex("deadbe");
        assert_eq!(color.0[0], 0xbe);
        assert_eq!(color.0[1], 0xad);
        assert_eq!(color.0[2], 0xde);
        assert_eq!(color.0[3], 0xff);
    }
}
