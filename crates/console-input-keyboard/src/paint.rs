//! Drawing one arrangement onto one frame.
//!
//! Everything here is decided somewhere else: `layout` says which keys there
//! are and `layout::placed` says where, `config` says what colour they are and
//! what font is on them, and `drawing` does the cairo. This is the half page
//! that puts those together, and it is separate from all three because it is
//! the only part that has to happen inside a frame the compositor is waiting
//! for.


use console_core_never::Never;
use console_core_number_conversion::fitted;
use pango::FontDescription;

use crate::config::{Config, Scheme};
use crate::drawing::{Color, Rect, Surface};
use crate::layout::{Key, Kind, Layout, Placed, mods};

const EDGE: f64 = 2.0;

pub struct Look<'a> {
    pub config: &'a Config,
    pub layout: &'a Layout,
    pub keys: &'a [Placed],
    pub pressed: Option<usize>,
    pub held: u8,
    pub selected: Option<usize>,
    pub language: Option<&'a str>,
    pub wide: f64,
    pub tall: f64,
}

pub fn keyboard(onto: &Surface, look: &Look) -> Result<(), Never> {
    let Look { config, layout, keys, pressed, held, selected, language, wide, tall } = *look;
    let font = FontDescription::from_string(&config.font);

    match config.schemes.first() {
        Some(first) => {
            let Ok(bg) = colour(first.bg);
            let Ok(()) = onto.fill_rectangle(bg, Rect { x: 0.0, y: 0.0, w: wide, h: tall }, 0);
        }
        None => {},
    }

    for placed in keys {
        let Some(key) = layout.keys.get(placed.at) else { continue };

        let wanted = usize::from(key.scheme).min(config.schemes.len().saturating_sub(1));

        let Some(scheme) = config.schemes.get(wanted) else { continue };

        let down = pressed == Some(placed.at);
        let Ok(rounding) = fitted(config.rounding);
        let Ok(()) = one(onto, key, placed, &Ink {
            scheme,
            pressed: down,
            under: selected == Some(placed.at),
            held,
            language,
            font: &font,
            rounding,
        });
    }

    Ok(())
}

struct Ink<'a> {
    scheme: &'a Scheme,
    pressed: bool,
    under: bool,
    held: u8,
    language: Option<&'a str>,
    font: &'a FontDescription,
    rounding: i32,
}

fn one(onto: &Surface, key: &Key, placed: &Placed, ink: &Ink) -> Result<(), Never> {
    let Ink { scheme, pressed, under, held, language, font, rounding } = *ink;
    let at = Rect { x: placed.x, y: placed.y, w: placed.wide, h: placed.tall };
    let face = match (pressed, under) {
        (true, _) => scheme.high,
        (false, true) => scheme.sel,
        (false, false) => scheme.fg,
    };
    let Ok(face) = colour(face);
    let Ok(inset) = at.inset(EDGE);
    let Ok(()) = onto.fill_rectangle(face, inset, rounding);

    let shifted = held & (mods::SHIFT | mods::CAPS) != 0;
    let label = match key.kind {
        Kind::Language => language.unwrap_or(""),
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

    let ink = match (pressed, under) {
        (true, _) => scheme.text_press,
        (false, true) => scheme.text_sel,
        (false, false) => scheme.text,
    };
    let Ok(ink) = colour(ink);
    let Ok(()) = onto.draw_text(ink, at, EDGE, label, font);

    Ok(())
}

fn colour(from: crate::config::Colour) -> Result<Color, Never> {
    Ok(Color(from.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{named, of, placed};

    #[test]
    fn what_is_drawn_is_inside_the_keyboard() {
        let Ok(name) = named("full");
        let Ok(layout) = of(name.expect("full"));
        let Ok(keys) = placed(layout, 1024.0, 260.0);
        for key in &keys {
            let Ok(cell) = Rect { x: key.x, y: key.y, w: key.wide, h: key.tall }.inset(EDGE);
            assert!(cell.w > 0.0 && cell.h > 0.0, "a key with no face left after its border");
            assert!(cell.x >= 0.0 && cell.y >= 0.0);
            assert!(cell.x + cell.w <= 1024.0);
            assert!(cell.y + cell.h <= 260.0);
        }
    }

    #[test]
    fn the_gap_between_two_keys_is_a_border_from_each() {
        let Ok(left) = Rect { x: 0.0, y: 0.0, w: 100.0, h: 50.0 }.inset(EDGE);
        let Ok(right) = Rect { x: 100.0, y: 0.0, w: 100.0, h: 50.0 }.inset(EDGE);
        let gap = right.x - (left.x + left.w);
        assert!((gap - EDGE * 2.0).abs() < 0.001, "the gap is {gap}");
    }
}
