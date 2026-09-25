//! Drawing one arrangement onto one frame.
//!
//! Everything here is decided somewhere else: `layout` says which keys there
//! are and `layout::placed` says where, `configuration` says what color they are and
//! what font is on them, and `drawing` does the cairo. This is the half page
//! that puts those together, and it is separate from all three because it is
//! the only part that has to happen inside a frame the compositor is waiting
//! for.


use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use pango::FontDescription;

use crate::configuration::{Configuration, Scheme};
use crate::drawing::{Rectangle, Surface};
use crate::layout::{Key, Kind, Layout, Placed, mods};

const NO_LANGUAGE_NAMED: &str = "";


const EDGE: f64 = 2.0;

pub struct Look<'a> {
    pub configuration: &'a Configuration,
    pub layout: &'a Layout,
    pub keys: &'a [Placed],
    pub pressed: Option<u32>,
    pub held: u8,
    pub selected: Option<u32>,
    pub language: Option<&'a str>,
    pub width: f64,
    pub height: f64,
}

pub fn keyboard(onto: &Surface, look: &Look) -> Result<(), Never> {
    let Look { configuration, layout, keys, pressed, held, selected, language, width: wide, height: tall } = *look;
    let font = FontDescription::from_string(&configuration.font);

    match configuration.schemes.first() {
        Some(first) => {
            let Ok(()) = onto.fill_rectangle(first.bg, Rectangle { x: 0.0, y: 0.0, w: wide, h: tall }, 0);
        }
        None => {},
    }

    for placed in keys {
        let Ok(at) = index(placed.at);

        let key = match layout.keys.get(at) {
            Some(key) => key,
            None => continue,
        };

        let Ok(schemes) = fitted::<_, u32>(configuration.schemes.len());
        let Ok(wanted) = index(u32::from(key.scheme).min(schemes.saturating_sub(1)));

        let scheme = match configuration.schemes.get(wanted) {
            Some(scheme) => scheme,
            None => continue,
        };

        let showing = match (pressed == Some(placed.at), selected == Some(placed.at)) {
            (true, _) => Showing::Pressed,
            (false, true) => Showing::Under,
            (false, false) => Showing::Plain,
        };

        let Ok(rounding) = fitted(configuration.rounding);
        let Ok(()) = one(onto, key, placed, &HexColor {
            scheme,
            showing,
            held,
            language,
            font: &font,
            rounding,
        });
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Showing {
    Pressed,
    Under,
    Plain,
}

struct HexColor<'a> {
    scheme: &'a Scheme,
    showing: Showing,
    held: u8,
    language: Option<&'a str>,
    font: &'a FontDescription,
    rounding: i32,
}

fn one(onto: &Surface, key: &Key, placed: &Placed, ink: &HexColor) -> Result<(), Never> {
    let HexColor { scheme, showing, held, language, font, rounding } = *ink;
    let at = Rectangle { x: placed.x, y: placed.y, w: placed.width, h: placed.height };
    let face = match showing {
        Showing::Pressed => scheme.high,
        Showing::Under => scheme.sel,
        Showing::Plain => scheme.fg,
    };
    let Ok(inset) = at.inset(EDGE);
    let Ok(()) = onto.fill_rectangle(face, inset, rounding);

    let shifted = held & (mods::SHIFT | mods::CAPS) != 0;
    let label = match key.kind {
        Kind::Language => match language {
            Some(language) => language,
            None => NO_LANGUAGE_NAMED,
        },
        Kind::Pad
        | Kind::Code { .. }
        | Kind::Mod(_)
        | Kind::Copy { .. }
        | Kind::Layout(_)
        | Kind::Back
        | Kind::Next
        | Kind::Symbols
        | Kind::Compose
        | Kind::EndRow => match shifted && !key.shift.is_empty() {
            true => key.shift,
            false => key.label,
        },
    };

    match label.is_empty() {
        true => return Ok(()),
        false => {},
    }

    let ink = match showing {
        Showing::Pressed => scheme.text_press,
        Showing::Under => scheme.text_sel,
        Showing::Plain => scheme.text,
    };
    let Ok(()) = onto.draw_text(ink, at, EDGE, label, font);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_core_geometry::Size;
    use crate::layout::{named, of, placed};

    #[test]
    fn what_is_drawn_is_inside_the_keyboard() {
        let Ok(name) = named("full");
        let Ok(layout) = of(name.expect("full"));
        let Ok(keys) = placed(layout, Size { width: 1024.0, height: 260.0 });
        for key in &keys {
            let Ok(cell) = Rectangle { x: key.x, y: key.y, w: key.width, h: key.height }.inset(EDGE);
            assert!(cell.w > 0.0 && cell.h > 0.0, "a key with no face left after its border");
            assert!(cell.x >= 0.0 && cell.y >= 0.0);
            assert!(cell.x + cell.w <= 1024.0);
            assert!(cell.y + cell.h <= 260.0);
        }
    }

    #[test]
    fn the_gap_between_two_keys_is_a_border_from_each() {
        let Ok(left) = Rectangle { x: 0.0, y: 0.0, w: 100.0, h: 50.0 }.inset(EDGE);
        let Ok(right) = Rectangle { x: 100.0, y: 0.0, w: 100.0, h: 50.0 }.inset(EDGE);
        let gap = right.x - (left.x + left.w);
        assert!((gap - EDGE * 2.0).abs() < 0.001, "the gap is {gap}");
    }
}
