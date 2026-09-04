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


use console_number::toward_zero_i32;
use cairo::{Context, Format, ImageSurface};
use pango::FontDescription;

/// A rectangle on the surface, in logical units.
///
/// The four numbers go everywhere together, and a call that takes them apart
/// is a call where `w` and `h` can be handed over the wrong way round without
/// anything noticing. Named, so they cannot be.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    /// The rectangle at a corner, of a size.
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    /// The same rectangle, pulled in by `border` on every side.
    ///
    /// Never inside out: a border wider than half the rectangle gives an
    /// empty one rather than a negative one, because Pango is handed this and
    /// a negative width there means "unbounded" rather than "nothing".
    pub fn inset(self, border: f64) -> Self {
        Self {
            x: self.x + border,
            y: self.y + border,
            w: (self.w - 2.0 * border).max(0.0),
            h: (self.h - 2.0 * border).max(0.0),
        }
    }
}

/// A Cairo surface the keyboard draws into, plus a Pango layout for the text.
///
/// `pixels` is the buffer the compositor will see — it must outlive the
/// surface. The size and the format are ARGB32, matching what Wayland
/// `wl_shm_format::ARGB8888` and Cairo's `Format::ARgb32` both encode.
///
/// Construction takes the buffer rather than allocating it, because the
/// Wayland shm pool hands us the buffer's bytes and we draw into them in
/// place. Calling `ImageSurface::create_for_data` would be the C version's
/// `cairo_image_surface_create_for_data`, only with a name that fits Rust.
pub struct Surface {
    /// The Cairo drawing context for the buffer.
    pub cairo: Context,
    /// The Pango layout, sharing the same cairo surface. One per surface;
    /// the keyboard never needs more.
    pub layout: pango::Layout,
    /// Pixels per logical unit. The compositor hands us a scale and we
    /// scale the Cairo transform accordingly.
    pub scale: f64,
}

/// A 32-bit ARGB colour, the way Cairo takes it. The C version stored
/// colours as BGRA because wl_shm's `ARGB8888` is little-endian BGRA on
/// the wire; we keep that ordering so the wire bytes are correct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub [u8; 4]);

impl Color {
    pub const fn from_hex(six: &str) -> Self {
        let bytes = six.as_bytes();
        let r = hex(bytes[0]) * 16 + hex(bytes[1]);
        let g = hex(bytes[2]) * 16 + hex(bytes[3]);
        let b = hex(bytes[4]) * 16 + hex(bytes[5]);
        Color([b, g, r, 0xff])
    }
}

const fn hex(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => 0,
    }
}

impl Surface {
    /// Build a drawing surface over an existing pixel buffer.
    ///
    /// `pixels` must be at least `stride * height` bytes; `stride` is the
    /// number of bytes per row (Cairo wants it in bytes, the C version
    /// passed `width * 4` because there is no row padding). The surface
    /// does not take ownership of `pixels`; the caller does, and the caller
    /// also owns the wl_buffer that wraps them.
    /// Nothing where cairo will not give a surface or a context for it.
    ///
    /// A frame that cannot be started is a frame that is not drawn, which is
    /// the same cost as a paint call that fails part way through it -- see
    /// `clear` below. The keyboard is the only way to type on this machine, so
    /// neither is worth dying over.
    pub fn new(pixels: &mut [u8], stride: i32, height: i32, scale: f64) -> Option<Self> {
        // SAFETY: Cairo does not mutate `pixels` until we draw into the
        // returned surface, and `pixels` lives as long as the surface does.
        // The caller has promised the buffer is large enough.
        let made = unsafe {
            ImageSurface::create_for_data_unsafe(
                pixels.as_mut_ptr(),
                Format::ARgb32,
                stride / 4,
                height,
                stride,
            )
        };
        let image = match made {
            Ok(image) => image,
            // Not worth dying over, and not worth going quiet about either:
            // what a person sees is a keyboard that did not appear, and the
            // reason it did not is only in here.
            Err(fault) => {
                eprintln!("no surface to draw the keyboard on: {fault}");
                return None;
            },
        };
        let cairo = match Context::new(&image) {
            Ok(cairo) => cairo,
            Err(fault) => {
                eprintln!("nothing to draw the keyboard with: {fault}");
                return None;
            },
        };
        // The C version scales the cairo context by `scale` so all
        // coordinates are in logical pixels, and the surface bytes are
        // physical pixels. We do the same.
        cairo.scale(scale, scale);
        cairo.set_antialias(cairo::Antialias::None);
        // The layout has to come from cairo's own font map, not from a bare
        // `pango::Context`: a context made with `Context::new` has no font map
        // at all, and every call that would lay out text fails an assertion
        // and draws nothing. What that looks like is a keyboard with keys and
        // no letters on them.
        let layout = pango::Layout::new(&pangocairo::functions::create_context(&cairo));
        layout.set_auto_dir(false);
        Some(Surface {
            cairo,
            layout,
            scale,
        })
    }

    /// Clear a rectangle to transparent.
    ///
    /// Cairo's own calls are discarded rather than unwrapped, here and in every
    /// painting method below. A cairo error is sticky: the context remembers it
    /// and every call after it becomes a no-op, so the worst a failure here can
    /// do is a frame that does not draw. That is the right cost. This is the
    /// on-screen keyboard, and it is the only way to type on this machine --
    /// a panic in a paint call is a device with no keyboard, over one frame.
    pub fn clear(&self, at: Rect) {
        let _ = self.cairo.save();
        self.cairo.set_operator(cairo::Operator::Clear);
        self.cairo.rectangle(at.x, at.y, at.w, at.h);
        let _ = self.cairo.fill();
        let _ = self.cairo.restore();
    }

    /// Trace the rectangle, with its corners rounded or not.
    ///
    /// The C version traces the rounded path with four arcs. Both painting
    /// operations wanted the same tracing and had a copy of it each, which is
    /// two places for a corner to come out different.
    fn trace(&self, at: Rect, rounding: i32) {
        let Rect { x, y, w, h } = at;

        if rounding <= 0 {
            self.cairo.rectangle(x, y, w, h);
            return;
        }

        let r = f64::from(rounding);
        let pi = std::f64::consts::PI;
        self.cairo.new_sub_path();
        self.cairo.arc(x + w - r, y + r, r, -pi / 2.0, 0.0);
        self.cairo.arc(x + w - r, y + h - r, r, 0.0, pi / 2.0);
        self.cairo.arc(x + r, y + h - r, r, pi / 2.0, pi);
        self.cairo.arc(x + r, y + r, r, pi, 3.0 * pi / 2.0);
        self.cairo.close_path();
    }

    /// Paint a rectangle with the colour, replacing whatever was there.
    ///
    /// `rounding` rounds the corners; the keyboard passes `0` for sharp keys
    /// and the height-divided-by-something for the slab.
    ///
    /// SOURCE and not OVER, which is the whole of what this does differently
    /// from painting normally: it replaces the pixel rather than compositing
    /// onto it, so a frame drawn into the buffer the compositor is already
    /// holding does not accumulate the frame before it. There was a second
    /// version of this taking the operator, from when the highlight and the
    /// swipe trail were composited over the keys; neither is drawn here.
    pub fn fill_rectangle(&self, colour: Color, at: Rect, rounding: i32) {
        let _ = self.cairo.save();
        self.cairo.set_operator(cairo::Operator::Source);
        self.set_source(colour);
        self.trace(at, rounding);
        let _ = self.cairo.fill();
        let _ = self.cairo.restore();
    }

    /// Centre a string of text inside `at` and paint it in `colour`.
    /// `border` is the padding around the text.
    ///
    /// Pango handles wrapping and shaping; we set the layout's width and
    /// height to the inner rectangle, ask Pango for the laid-out size in
    /// pixels, and offset the move-to so the centre of the laid-out text
    /// matches the centre of the cell.
    pub fn draw_text(
        &self,
        colour: Color,
        at: Rect,
        border: f64,
        label: &str,
        font: &FontDescription,
    ) {
        let _ = self.cairo.save();
        self.set_source(colour);
        self.layout.set_font_description(Some(font));
        self.layout.set_text(label);
        let (text_w, text_h) = self.layout.pixel_size();
        // Centre the laid-out text in the cell. Pango returns 0 if the text
        // did not lay out (an empty string), in which case the offset is half
        // the cell -- the caller's problem, not ours.
        let dx = (at.w - f64::from(text_w)) / 2.0;
        let dy = (at.h - f64::from(text_h)) / 2.0;
        let inner = at.inset(border);
        self.layout.set_width(toward_zero_i32(inner.w) * pango::SCALE);
        self.layout.set_height(toward_zero_i32(inner.h) * pango::SCALE);
        self.cairo.move_to(inner.x + dx, inner.y + dy);
        pangocairo::functions::show_layout(&self.cairo, &self.layout);
        let _ = self.cairo.restore();
    }

    /// Set the Cairo source colour from a `Color`. The C version reads
    /// `bgra[2]/255, bgra[1]/255, bgra[0]/255, bgra[3]/255` because the
    /// `Color` union is stored as BGRA on little-endian; we read the same
    /// fields in the same order.
    fn set_source(&self, colour: Color) {
        let [b, g, r, a] = colour.0;
        self.cairo.set_source_rgba(
            f64::from(r) / 255.0,
            f64::from(g) / 255.0,
            f64::from(b) / 255.0,
            f64::from(a) / 255.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// A buffer the surface can draw into. Held in a `RefCell` because
    /// Cairo takes a mutable pointer to it and Rust wants to know the
    /// borrow is unique.
    fn buffer(w: i32, h: i32) -> RefCell<Vec<u8>> {
        RefCell::new(vec![0; (w * 4 * h) as usize])
    }

    #[test]
    fn fill_rectangle_writes_some_non_zero_pixel() {
        let buf = buffer(20, 20);
        {
            let mut bytes = buf.borrow_mut();
            let surface = Surface::new(&mut bytes, 20 * 4, 20, 1.0).expect("a surface over the test buffer");
            surface.fill_rectangle(Color::from_hex("ff0000"), Rect::new(5.0, 5.0, 10.0, 10.0), 0);
        }
        let bytes = buf.borrow();
        // The painted rectangle should be red. ARGB32 stores bytes
        // little-endian: blue, green, red, alpha. A red pixel is
        // [0xff, 0x00, 0x00, 0xff] (BGRA bytes on the wire).
        assert_eq!(bytes[(10 * 20 * 4 + 10 * 4) as usize], 0x00); // B
        assert_eq!(bytes[(10 * 20 * 4 + 10 * 4 + 1) as usize], 0x00); // G
        assert_eq!(bytes[(10 * 20 * 4 + 10 * 4 + 2) as usize], 0xff); // R
        assert_eq!(bytes[(10 * 20 * 4 + 10 * 4 + 3) as usize], 0xff); // A
    }

    #[test]
    fn clear_makes_the_pixel_transparent() {
        let buf = buffer(10, 10);
        {
            let mut bytes = buf.borrow_mut();
            let surface = Surface::new(&mut bytes, 10 * 4, 10, 1.0).expect("a surface over the test buffer");
            surface.fill_rectangle(Color::from_hex("ffffff"), Rect::new(0.0, 0.0, 10.0, 10.0), 0);
            surface.clear(Rect::new(0.0, 0.0, 10.0, 10.0));
        }
        let bytes = buf.borrow();
        // All zeros is the cleared state.
        for byte in bytes.iter() {
            assert_eq!(*byte, 0);
        }
    }

    /// A border wider than the cell gives an empty rectangle, not a
    /// backwards one.
    ///
    /// Pango reads a negative width as "unbounded" rather than as "nothing",
    /// so an inset that went past itself would not draw a smaller label -- it
    /// would take the wrapping off and let a long key name run across the
    /// keyboard. The keys the keyboard draws are small and the border is
    /// fixed, so this is one arithmetic mistake away rather than impossible.
    #[test]
    fn insetting_past_the_middle_gives_nothing_rather_than_a_backwards_rectangle() {
        let cell = Rect::new(10.0, 10.0, 8.0, 4.0);
        let inner = cell.inset(6.0);
        assert_eq!(inner.w, 0.0, "the width went backwards: {inner:?}");
        assert_eq!(inner.h, 0.0, "the height went backwards: {inner:?}");
        assert!(inner.x >= cell.x && inner.y >= cell.y, "the corner moved out: {inner:?}");
    }

    /// An ordinary inset takes the border off each side, so a cell inset by
    /// one is two narrower and not one.
    #[test]
    fn an_inset_comes_off_both_sides() {
        let inner = Rect::new(0.0, 0.0, 10.0, 6.0).inset(1.0);
        assert_eq!(inner, Rect::new(1.0, 1.0, 8.0, 4.0));
    }

    #[test]
    fn colour_from_hex_round_trips_through_a_red_pixel() {
        let c = Color::from_hex("deadbe");
        assert_eq!(c.0[0], 0xbe); // B
        assert_eq!(c.0[1], 0xad); // G
        assert_eq!(c.0[2], 0xde); // R
        assert_eq!(c.0[3], 0xff); // A
    }
}
