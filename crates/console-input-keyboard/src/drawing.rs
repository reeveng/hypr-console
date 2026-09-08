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
//! swipe trail, the port draws neither, and it went with them. Every colour
//! here is opaque and every key covers its own cell, which is what lets the
//! whole strip be painted first and drawn over.


use console_core_never::Never;
use console_core_number_conversion::toward_zero_i32;
use cairo::{Context, Format, ImageSurface};
use pango::FontDescription;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Result<Self, Never> {
        Ok(Self { x, y, w, h })
    }

    pub fn inset(self, border: f64) -> Result<Self, Never> {
        Ok(Self {
            x: self.x + border,
            y: self.y + border,
            w: (self.w - 2.0 * border).max(0.0),
            h: (self.h - 2.0 * border).max(0.0),
        })
    }
}

pub struct Surface {
    pub cairo: Context,
    pub layout: pango::Layout,
    pub scale: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub [u8; 4]);

impl Color {
    pub const fn from_hex(six: &str) -> Result<Self, Never> {
        let bytes = six.as_bytes();
        let Ok(r) = band(bytes[0], bytes[1]);
        let Ok(g) = band(bytes[2], bytes[3]);
        let Ok(b) = band(bytes[4], bytes[5]);

        Ok(Color([b, g, r, 0xff]))
    }
}

const fn band(high: u8, low: u8) -> Result<u8, Never> {
    let Ok(high) = hex(high);
    let Ok(low) = hex(low);

    Ok(high * 16 + low)
}

const fn hex(byte: u8) -> Result<u8, Never> {
    Ok(match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => 0,
    })
}

impl Surface {
    pub fn new(pixels: &mut [u8], stride: i32, height: i32, scale: f64) -> Result<Option<Self>, Never> {
        // SAFETY: Cairo does not mutate `pixels` until we draw into the
        let made = unsafe {
            ImageSurface::create_for_data_unsafe(
                pixels.as_mut_ptr(),
                Format::ARgb32,
                stride.saturating_div(4),
                height,
                stride,
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

    pub fn clear(&self, at: Rect) -> Result<(), Never> {
        let _ = self.cairo.save();
        self.cairo.set_operator(cairo::Operator::Clear);
        self.cairo.rectangle(at.x, at.y, at.w, at.h);
        let _ = self.cairo.fill();
        let _ = self.cairo.restore();

        Ok(())
    }

    fn trace(&self, at: Rect, rounding: i32) -> Result<(), Never> {
        let Rect { x, y, w, h } = at;

        match rounding <= 0 {
            true => {
                self.cairo.rectangle(x, y, w, h);
                return Ok(());
            }
            false => {},
        }

        let r = f64::from(rounding);
        let pi = std::f64::consts::PI;
        self.cairo.new_sub_path();
        self.cairo.arc(x + w - r, y + r, r, -pi / 2.0, 0.0);
        self.cairo.arc(x + w - r, y + h - r, r, 0.0, pi / 2.0);
        self.cairo.arc(x + r, y + h - r, r, pi / 2.0, pi);
        self.cairo.arc(x + r, y + r, r, pi, 3.0 * pi / 2.0);
        self.cairo.close_path();

        Ok(())
    }

    pub fn fill_rectangle(&self, colour: Color, at: Rect, rounding: i32) -> Result<(), Never> {
        let _ = self.cairo.save();
        self.cairo.set_operator(cairo::Operator::Source);
        let Ok(()) = self.set_source(colour);
        let Ok(()) = self.trace(at, rounding);
        let _ = self.cairo.fill();
        let _ = self.cairo.restore();

        Ok(())
    }

    pub fn draw_text(
        &self,
        colour: Color,
        at: Rect,
        border: f64,
        label: &str,
        font: &FontDescription,
    ) -> Result<(), Never> {
        let _ = self.cairo.save();
        let Ok(()) = self.set_source(colour);
        self.layout.set_font_description(Some(font));
        self.layout.set_text(label);
        let (text_w, text_h) = self.layout.pixel_size();
        let dx = (at.w - f64::from(text_w)) / 2.0;
        let dy = (at.h - f64::from(text_h)) / 2.0;
        let Ok(inner) = at.inset(border);
        let Ok(across) = toward_zero_i32(inner.w);
        let Ok(down) = toward_zero_i32(inner.h);

        self.layout.set_width(across.saturating_mul(pango::SCALE));
        self.layout.set_height(down.saturating_mul(pango::SCALE));
        self.cairo.move_to(inner.x + dx, inner.y + dy);
        pangocairo::functions::show_layout(&self.cairo, &self.layout);
        let _ = self.cairo.restore();

        Ok(())
    }

    fn set_source(&self, colour: Color) -> Result<(), Never> {
        let [b, g, r, a] = colour.0;
        self.cairo.set_source_rgba(
            f64::from(r) / 255.0,
            f64::from(g) / 255.0,
            f64::from(b) / 255.0,
            f64::from(a) / 255.0,
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn buffer(w: i32, h: i32) -> RefCell<Vec<u8>> {
        RefCell::new(vec![0; (w * 4 * h) as usize])
    }

    #[test]
    fn fill_rectangle_writes_some_non_zero_pixel() {
        let buf = buffer(20, 20);
        {
            let mut bytes = buf.borrow_mut();
            let surface = match Surface::new(&mut bytes, 20 * 4, 20, 1.0) {
                Ok(Some(surface)) => surface,
                Ok(None) | Err(_) => panic!("a surface over the test buffer"),
            };
            let Ok(red) = Color::from_hex("ff0000");
            let Ok(at) = Rect::new(5.0, 5.0, 10.0, 10.0);
            let Ok(()) = surface.fill_rectangle(red, at, 0);
        }
        let bytes = buf.borrow();
        assert_eq!(bytes[(10 * 20 * 4 + 10 * 4) as usize], 0x00);
        assert_eq!(bytes[(10 * 20 * 4 + 10 * 4 + 1) as usize], 0x00);
        assert_eq!(bytes[(10 * 20 * 4 + 10 * 4 + 2) as usize], 0xff);
        assert_eq!(bytes[(10 * 20 * 4 + 10 * 4 + 3) as usize], 0xff);
    }

    #[test]
    fn clear_makes_the_pixel_transparent() {
        let buf = buffer(10, 10);
        {
            let mut bytes = buf.borrow_mut();
            let surface = match Surface::new(&mut bytes, 10 * 4, 10, 1.0) {
                Ok(Some(surface)) => surface,
                Ok(None) | Err(_) => panic!("a surface over the test buffer"),
            };
            let Ok(white) = Color::from_hex("ffffff");
            let Ok(at) = Rect::new(0.0, 0.0, 10.0, 10.0);
            let Ok(()) = surface.fill_rectangle(white, at, 0);
            let Ok(()) = surface.clear(at);
        }
        let bytes = buf.borrow();
        for byte in bytes.iter() {
            assert_eq!(*byte, 0);
        }
    }

    #[test]
    fn insetting_past_the_middle_gives_nothing_rather_than_a_backwards_rectangle() {
        let Ok(cell) = Rect::new(10.0, 10.0, 8.0, 4.0);
        let Ok(inner) = cell.inset(6.0);
        assert_eq!(inner.w, 0.0, "the width went backwards: {inner:?}");
        assert_eq!(inner.h, 0.0, "the height went backwards: {inner:?}");
        assert!(inner.x >= cell.x && inner.y >= cell.y, "the corner moved out: {inner:?}");
    }

    #[test]
    fn an_inset_comes_off_both_sides() {
        let Ok(rect) = Rect::new(0.0, 0.0, 10.0, 6.0);
        let Ok(inner) = rect.inset(1.0);
        assert_eq!(Ok(inner), Rect::new(1.0, 1.0, 8.0, 4.0));
    }

    #[test]
    fn colour_from_hex_round_trips_through_a_red_pixel() {
        let Ok(c) = Color::from_hex("deadbe");
        assert_eq!(c.0[0], 0xbe);
        assert_eq!(c.0[1], 0xad);
        assert_eq!(c.0[2], 0xde);
        assert_eq!(c.0[3], 0xff);
    }
}
