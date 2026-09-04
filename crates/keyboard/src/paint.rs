//! Drawing one arrangement onto one frame.
//!
//! Everything here is decided somewhere else: `layout` says which keys there
//! are and `layout::placed` says where, `config` says what colour they are and
//! what font is on them, and `drawing` does the cairo. This is the half page
//! that puts those together, and it is separate from all three because it is
//! the only part that has to happen inside a frame the compositor is waiting
//! for.


use console_number::fitted;
use pango::FontDescription;

use crate::config::{Config, Scheme};
use crate::drawing::{Color, Rect, Surface};
use crate::layout::{Key, Kind, Layout, Placed, mods};

/// How much of a key is border rather than face.
///
/// Small, and not nothing: two keys with no gap between them read as one wide
/// key, and a thumb aiming at the edge of a letter hits the letter beside it.
///
/// `KBD_KEY_BORDER` in the C's `layout.mobintl.h`. Two rather than one, and it
/// shows: at one, every key is a pixel larger on each side and the gaps between
/// them are half what they were drawn to be.
const EDGE: f64 = 2.0;

/// Everything a frame is drawn from.
///
/// One argument rather than eight, because seven of them travel together
/// everywhere and the two that are easy to swap -- `wide` and `tall` -- are
/// numbers of the same type.
pub struct Look<'a> {
    /// The colours, the font and the rounding.
    pub config: &'a Config,
    /// The arrangement being drawn.
    pub layout: &'a Layout,
    /// Where its keys landed on this surface.
    pub keys: &'a [Placed],
    /// The key a thumb is on, drawn in the pressed colours so that somebody
    /// who cannot see under their own finger can still tell it landed.
    pub pressed: Option<usize>,
    /// The modifiers held, which decides which face of each key is shown.
    pub held: u8,
    /// The key the pad's selection is sitting on, drawn in a colour of its own.
    /// A thumb on a stick cannot see where it is any other way.
    pub selected: Option<usize>,
    /// What the language key says: the name, in its own script, of the
    /// language that key is about to go to. `None` when the machine types one
    /// alphabet, and then the key is drawn blank because it does nothing.
    pub language: Option<&'a str>,
    /// The surface, in logical units.
    pub wide: f64,
    pub tall: f64,
}

/// Draw the whole keyboard.
pub fn keyboard(onto: &Surface, look: &Look) {
    let Look { config, layout, keys, pressed, held, selected, language, wide, tall } = *look;
    let font = FontDescription::from_string(&config.font);
    // The whole strip first, because a key that does not cover its cell would
    // otherwise show the last frame through the gap.
    onto.fill_rectangle(colour(config.schemes[0].bg), Rect { x: 0.0, y: 0.0, w: wide, h: tall }, 0);

    for placed in keys {
        let Some(key) = layout.keys.get(placed.at) else { continue };

        let scheme = &config.schemes[usize::from(key.scheme).min(config.schemes.len() - 1)];
        let down = pressed == Some(placed.at);
        one(onto, key, placed, &Ink {
            scheme,
            pressed: down,
            under: selected == Some(placed.at),
            held,
            language,
            font: &font,
            rounding: fitted(config.rounding),
        });
    }
}

/// How one key is to be drawn: everything about it that is not where it is.
struct Ink<'a> {
    scheme: &'a Scheme,
    pressed: bool,
    /// Where the pad's selection is sitting.
    under: bool,
    held: u8,
    language: Option<&'a str>,
    font: &'a FontDescription,
    rounding: i32,
}

/// One key: its face, and what is written on it.
fn one(onto: &Surface, key: &Key, placed: &Placed, ink: &Ink) {
    let Ink { scheme, pressed, under, held, language, font, rounding } = *ink;
    let at = Rect { x: placed.x, y: placed.y, w: placed.wide, h: placed.tall };
    // `fg` is the key, not the ink on it. The scheme is named from the C, where
    // `bg` is the slab the keys lie on and `fg` is the key lying on it -- so the
    // face of a key at rest is `fg`, and `text` is what is written across it.
    // Drawn from `bg` instead, every key is painted the colour of the strip that
    // was just painted behind it, and the keyboard is labels floating on black.
    // Three states and not two. Where the stick is sitting is deliberately not
    // the pressed colour: a thumb needs to tell "I am here" from "I just typed
    // this", and with one colour for both, crossing the keyboard looks like
    // typing every key on the way. Pressed wins over selected, because the
    // key being typed is the one the selection is on and the press is the
    // thing that just happened.
    let face = match (pressed, under) {
        (true, _) => scheme.high,
        (false, true) => scheme.sel,
        (false, false) => scheme.fg,
    };
    onto.fill_rectangle(colour(face), at.inset(EDGE), rounding);

    // The shifted face is what the key will produce if it is pressed now, not
    // what it produces normally: a keyboard that goes on drawing lower case
    // with shift held is one you have to remember the state of.
    let shifted = held & (mods::SHIFT | mods::CAPS) != 0;
    let label = match key.kind {
        // The one key whose face is not in the table. It says where it goes,
        // and where it goes depends on which languages this machine was given
        // rather than on which table the key was written in.
        Kind::Language => language.unwrap_or(""),
        _ => match shifted && !key.shift.is_empty() {
            true => key.shift,
            false => key.label,
        },
    };

    if label.is_empty() {
        return;
    }

    let ink = match (pressed, under) {
        (true, _) => scheme.text_press,
        (false, true) => scheme.text_sel,
        (false, false) => scheme.text,
    };
    onto.draw_text(colour(ink), at, EDGE, label, font);
}

/// The palette's colours and the drawing's are the same four bytes in the same
/// order; this is the one place that says so.
fn colour(from: crate::config::Colour) -> Color {
    Color(from.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{named, of, placed};

    /// Every key that is drawn has a cell inside the keyboard, and the cells do
    /// not overlap. Drawing itself wants a cairo surface; what can be checked
    /// without one is the arithmetic that decides where the ink goes, and that
    /// is where a keyboard goes wrong in a way a person sees.
    #[test]
    fn what_is_drawn_is_inside_the_keyboard() {
        let layout = of(named("full").expect("full"));
        let keys = placed(layout, 1024.0, 260.0);
        for key in &keys {
            let cell = Rect { x: key.x, y: key.y, w: key.wide, h: key.tall }.inset(EDGE);
            assert!(cell.w > 0.0 && cell.h > 0.0, "a key with no face left after its border");
            assert!(cell.x >= 0.0 && cell.y >= 0.0);
            assert!(cell.x + cell.w <= 1024.0);
            assert!(cell.y + cell.h <= 260.0);
        }
    }

    /// The border comes off both sides, so two keys side by side have a gap of
    /// two borders between them and neither loses more than the other.
    #[test]
    fn the_gap_between_two_keys_is_a_border_from_each() {
        let left = Rect { x: 0.0, y: 0.0, w: 100.0, h: 50.0 }.inset(EDGE);
        let right = Rect { x: 100.0, y: 0.0, w: 100.0, h: 50.0 }.inset(EDGE);
        let gap = right.x - (left.x + left.w);
        assert!((gap - EDGE * 2.0).abs() < 0.001, "the gap is {gap}");
    }
}
