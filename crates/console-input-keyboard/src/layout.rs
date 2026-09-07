//! What keys there are, what is written on them, and where they sit.
//!
//! This is the half of a keyboard that no amount of system configuration can
//! supply. `keymap` asks xkbcommon what a key *produces* -- that is the
//! system's business and it has known every alphabet X ships symbols for since
//! before this device existed. What it cannot say is which of forty keys under
//! a thumb carries it, how wide that key is, what is drawn on its face, or
//! which of them is the one that switches alphabets, because xkb describes a
//! hundred-and-four-key board on a desk and this is a strip 260 units tall
//! with no relationship to one.
//!
//! So the arrangement is here and the alphabet is not. Adding a language that
//! fits the latin arrangement is a line in `keymap`; adding one that does not
//! -- Thai fills both shift levels with letters, so its digits and its Esc are
//! on the shelf next door rather than on the arrangement -- is an arrangement
//! of its own, written once, the way every on-screen keyboard that has ever
//! supported a script has had to. What holds that together is a rule rather
//! than a table: the key that changes the language has the way to the numbers
//! beside it, on every arrangement that offers one.
//!
//! ## Where these tables came from
//!
//! `layout.mobintl.h`, converted rather than retyped. The arrangements are
//! wvkbd's work and this desktop's Thai layer on top of them; what the
//! conversion dropped is the eight keymaps that used to sit beside them, which
//! are `keymap`'s job now.


use console_core_never::Never;
use console_core_number_conversion::Float;
use crate::keymap::Layer;

pub mod key {
    pub const ESC: u32 = 1;
    pub const ONE: u32 = 2;
    pub const TWO: u32 = 3;
    pub const THREE: u32 = 4;
    pub const FOUR: u32 = 5;
    pub const FIVE: u32 = 6;
    pub const SIX: u32 = 7;
    pub const SEVEN: u32 = 8;
    pub const EIGHT: u32 = 9;
    pub const NINE: u32 = 10;
    pub const ZERO: u32 = 11;
    pub const MINUS: u32 = 12;
    pub const EQUAL: u32 = 13;
    pub const BACKSPACE: u32 = 14;
    pub const TAB: u32 = 15;
    pub const Q: u32 = 16;
    pub const W: u32 = 17;
    pub const E: u32 = 18;
    pub const R: u32 = 19;
    pub const T: u32 = 20;
    pub const Y: u32 = 21;
    pub const U: u32 = 22;
    pub const I: u32 = 23;
    pub const O: u32 = 24;
    pub const P: u32 = 25;
    pub const LEFTBRACE: u32 = 26;
    pub const RIGHTBRACE: u32 = 27;
    pub const ENTER: u32 = 28;
    pub const A: u32 = 30;
    pub const S: u32 = 31;
    pub const D: u32 = 32;
    pub const F: u32 = 33;
    pub const G: u32 = 34;
    pub const H: u32 = 35;
    pub const J: u32 = 36;
    pub const K: u32 = 37;
    pub const L: u32 = 38;
    pub const SEMICOLON: u32 = 39;
    pub const APOSTROPHE: u32 = 40;
    pub const GRAVE: u32 = 41;
    pub const BACKSLASH: u32 = 43;
    pub const Z: u32 = 44;
    pub const X: u32 = 45;
    pub const C: u32 = 46;
    pub const V: u32 = 47;
    pub const B: u32 = 48;
    pub const N: u32 = 49;
    pub const M: u32 = 50;
    pub const COMMA: u32 = 51;
    pub const DOT: u32 = 52;
    pub const SLASH: u32 = 53;
    pub const KPASTERISK: u32 = 55;
    pub const SPACE: u32 = 57;
    pub const KPPLUS: u32 = 78;
    pub const HOME: u32 = 102;
    pub const UP: u32 = 103;
    pub const PAGEUP: u32 = 104;
    pub const LEFT: u32 = 105;
    pub const RIGHT: u32 = 106;
    pub const END: u32 = 107;
    pub const DOWN: u32 = 108;
    pub const PAGEDOWN: u32 = 109;
    pub const DELETE: u32 = 111;
    pub const MENU: u32 = 139;
}

pub mod mods {
    pub const NONE: u8 = 0;
    pub const SHIFT: u8 = 1;
    pub const CAPS: u8 = 2;
    pub const CTRL: u8 = 4;
    pub const ALT: u8 = 8;
    pub const SUPER: u8 = 64;
    pub const ALTGR: u8 = 128;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Pad,
    Code { code: u32, held: Option<Which> },
    Mod(u8),
    Copy { code: u32, shifted: u32 },
    Layout(Which),
    Back,
    Next,
    Language,
    Symbols,
    Compose,
    EndRow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drops {
    Held,
    Nothing,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Key {
    pub label: &'static str,
    pub shift: &'static str,
    pub width: f64,
    pub kind: Kind,
    pub scheme: u8,
    pub force: u8,
    pub reset: Drops,
}

impl Key {
    pub const PLAIN: Key = Key {
        label: "",
        shift: "",
        width: 1.0,
        kind: Kind::Pad,
        scheme: 0,
        force: mods::NONE,
        reset: Drops::Nothing,
    };
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    pub keys: &'static [Key],
    pub alphabet: Layer,
    pub name: &'static str,
    pub primary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    pub at: usize,
    pub x: f64,
    pub y: f64,
    pub wide: f64,
    pub tall: f64,
}

pub fn placed(layout: &Layout, wide: f64, tall: f64) -> Result<Vec<Placed>, Never> {
    let Ok(rows) = rows(layout);

    match rows.is_empty() {
        true => return Ok(Vec::new()),
        false => {},
    }

    let Ok(many) = rows.len().float();

    let deep = tall / many;
    let mut out = Vec::with_capacity(layout.keys.len());

    for (down, row) in rows.iter().enumerate() {
        let across: f64 = row.iter().map(|(_, key)| key.width).sum();

        match across <= 0.0 {
            true => continue,
            false => {},
        }

        let mut x = 0.0;

        for (at, key) in row {
            let w = key.width / across * wide;

            match matches!(key.kind, Kind::Pad) {
                true => {},
                false => {
                    let Ok(row) = down.float();

                    out.push(Placed { at: *at, x, y: row * deep, wide: w, tall: deep });
                }
            }

            x += w;
        }
    }

    Ok(out)
}

pub fn rows(layout: &Layout) -> Result<Vec<Vec<(usize, &'static Key)>>, Never> {
    let mut out: Vec<Vec<(usize, &'static Key)>> = Vec::new();
    let mut row: Vec<(usize, &'static Key)> = Vec::new();

    for (at, key) in layout.keys.iter().enumerate() {
        match key.kind {
            Kind::EndRow => match row.is_empty() {
                true => {},
                false => out.push(std::mem::take(&mut row)),
            },
            Kind::Pad
            | Kind::Code { .. }
            | Kind::Mod(_)
            | Kind::Copy { .. }
            | Kind::Layout(_)
            | Kind::Back
            | Kind::Next
            | Kind::Language
            | Kind::Symbols
            | Kind::Compose => row.push((at, key)),
        }
    }

    match row.is_empty() {
        true => {},
        false => out.push(row),
    }

    Ok(out)
}

pub fn under(placed: &[Placed], x: f64, y: f64) -> Result<Option<Placed>, Never> {
    Ok(placed
        .iter()
        .find(|k| x >= k.x && x < k.x + k.wide && y >= k.y && y < k.y + k.tall)
        .copied())
}

fn gap(low: f64, high: f64, point: f64) -> Result<f64, Never> {
    Ok(match point {
        p if p < low => low - p,
        p if p > high => p - high,
        _ => 0.0,
    })
}

pub fn toward(keys: &[Placed], from: Option<usize>, dx: i32, dy: i32) -> Result<Option<usize>, Never> {
    let Some(here) = from.and_then(|at| keys.iter().position(|k| k.at == at)) else {
        return Ok(keys.first().map(|k| k.at));
    };

    let Some(sel) = keys.get(here).copied() else { return Ok(keys.first().map(|k| k.at)) };

    let middle = (sel.x + sel.wide / 2.0, sel.y + sel.tall / 2.0);

    let scored = |k: &Placed| {
        let along = match (dx, dy) {
            (d, _) if d > 0 => k.x - (sel.x + sel.wide),
            (d, _) if d < 0 => sel.x - (k.x + k.wide),
            (_, d) if d > 0 => k.y - (sel.y + sel.tall),
            _ => sel.y - (k.y + k.tall),
        };
        let Ok(across) = match dx != 0 {
            true => gap(k.y, k.y + k.tall, middle.1),
            false => gap(k.x, k.x + k.wide, middle.0),
        };

        (along, along + across * 3.0)
    };

    for ahead in [true, false] {
        let best = keys
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != here)
            .map(|(_, k)| (k.at, scored(k)))
            .filter(|(_, (along, _))| match ahead {
                true => *along >= 0.0,
                false => *along < 0.0,
            })
            .min_by(|a, b| a.1.1.total_cmp(&b.1.1));

        match best {
            Some((at, _)) => return Ok(Some(at)),
            None => {},
        }
    }

    Ok(None)
}

pub fn named(name: &str) -> Result<Option<Which>, Never> {
    Ok(Which::ALL.iter().copied().find(|which| {
        let Ok(of) = of(*which);

        of.name == name
    }))
}

pub fn of(which: Which) -> Result<&'static Layout, Never> {
    tables::layout(which)
}

mod tables;
pub use tables::Which;

#[cfg(test)]
mod tests {
    use super::*;

    fn of(which: Which) -> &'static Layout {
        let Ok(of) = super::of(which);

        of
    }

    fn named(name: &str) -> Option<Which> {
        let Ok(named) = super::named(name);

        named
    }

    fn placed(layout: &Layout, wide: f64, tall: f64) -> Vec<Placed> {
        let Ok(placed) = super::placed(layout, wide, tall);

        placed
    }

    fn rows(layout: &Layout) -> Vec<Vec<(usize, &'static Key)>> {
        let Ok(rows) = super::rows(layout);

        rows
    }

    fn under(keys: &[Placed], x: f64, y: f64) -> Option<Placed> {
        let Ok(under) = super::under(keys, x, y);

        under
    }

    fn gap(low: f64, high: f64, point: f64) -> f64 {
        let Ok(gap) = super::gap(low, high, point);

        gap
    }

    fn toward(keys: &[Placed], from: Option<usize>, dx: i32, dy: i32) -> Option<usize> {
        let Ok(toward) = super::toward(keys, from, dx, dy);

        toward
    }

    #[test]
    fn a_direction_crosses_the_row_it_started_on() {
        let layout = of(named("landscape").expect("landscape"));
        let keys = placed(layout, 1892.0, 260.0);
        let start = keys[0].at;
        let row = keys[0].y;
        let mut at = start;
        for step in 0..8 {
            at = toward(&keys, Some(at), 1, 0).expect("somewhere to the right");
            let now = keys.iter().find(|k| k.at == at).expect("placed");
            assert_eq!(now.y, row, "step {step} left the row it started on");
        }
    }

    #[test]
    fn up_is_the_row_above_and_not_a_diagonal() {
        let layout = of(named("landscape").expect("landscape"));
        let keys = placed(layout, 1892.0, 260.0);
        let bottom = keys.iter().max_by(|a, b| a.y.total_cmp(&b.y)).expect("a bottom row").y;
        for key in keys.iter().filter(|k| k.y == bottom) {
            let up = toward(&keys, Some(key.at), 0, -1).expect("a key above");
            let landed = keys.iter().find(|k| k.at == up).expect("placed");
            assert!(landed.y < key.y, "up went sideways");
            let rows: Vec<f64> = {
                let mut ys: Vec<f64> = keys.iter().map(|k| k.y).collect();
                ys.sort_by(f64::total_cmp);
                ys.dedup_by(|a, b| a == b);
                ys
            };
            let above = rows.iter().rev().find(|y| **y < key.y).expect("a row above");
            assert_eq!(landed.y, *above, "up skipped a row");
        }
    }

    #[test]
    fn a_direction_wraps_rather_than_stopping_at_the_edge() {
        let layout = of(named("landscape").expect("landscape"));
        let keys = placed(layout, 1892.0, 260.0);
        let top = keys.iter().min_by(|a, b| a.y.total_cmp(&b.y)).expect("a top row").at;
        let up = toward(&keys, Some(top), 0, -1).expect("wrapped round");
        let landed = keys.iter().find(|k| k.at == up).expect("placed");
        assert!(landed.y > keys[0].y, "up from the top row came out at the bottom");
    }

    #[test]
    fn the_first_direction_lands_somewhere() {
        let layout = of(named("landscape").expect("landscape"));
        let keys = placed(layout, 1892.0, 260.0);
        assert_eq!(toward(&keys, None, 1, 0), Some(keys[0].at));
        assert_eq!(toward(&[], None, 1, 0), None, "and an empty layout is not a panic");
    }

    #[test]
    fn a_wide_key_is_measured_to_its_edge() {
        assert_eq!(gap(10.0, 20.0, 15.0), 0.0, "inside the span is no distance at all");
        assert_eq!(gap(10.0, 20.0, 5.0), 5.0);
        assert_eq!(gap(10.0, 20.0, 25.0), 5.0);
    }

    #[test]
    fn every_arrangement_has_keys_and_an_alphabet() {
        for which in Which::ALL {
            let layout = of(which);
            assert!(!layout.keys.is_empty(), "{} has no keys", layout.name);
            assert!(!layout.name.is_empty(), "a layout with no name");
        }
    }

    #[test]
    fn the_layers_this_desktop_walks_are_all_there() {
        for name in ["full", "thai", "special", "landscape", "landscapespecial"] {
            assert!(named(name).is_some(), "no layout called {name}");
        }
    }

    #[test]
    fn the_keys_fill_the_surface_without_overlapping() {
        let layout = of(named("full").expect("full"));
        let keys = placed(layout, 1000.0, 260.0);
        assert!(!keys.is_empty());
        for key in &keys {
            assert!(key.x >= -0.001, "a key off the left");
            assert!(key.x + key.wide <= 1000.001, "a key off the right: {key:?}");
            assert!(key.y + key.tall <= 260.001, "a key below the keyboard: {key:?}");
        }
        let top = keys[0].tall / 2.0;
        for step in 0..100 {
            let x = step as f64 * 10.0 + 0.5;
            assert!(under(&keys, x, top).is_some(), "nothing under {x}");
        }
    }

    #[test]
    fn a_row_is_shared_out_against_its_own_width() {
        let layout = of(named("full").expect("full"));
        for row in rows(layout) {
            let across: f64 = row.iter().map(|(_, key)| key.width).sum();
            assert!(across > 0.0, "a row of nothing");
        }
    }

    #[test]
    fn the_space_bar_is_wider_than_a_letter() {
        let layout = of(named("full").expect("full"));
        let space = layout
            .keys
            .iter()
            .find(|key| matches!(key.kind, Kind::Code { code: key::SPACE, .. }))
            .expect("a space bar");
        assert!(space.width > 1.0, "the space bar is one column wide");
    }

    #[test]
    fn thai_holds_a_second_letter_where_latin_holds_a_capital() {
        let thai = of(named("thai").expect("thai"));
        let doubled = thai
            .keys
            .iter()
            .filter(|key| matches!(key.kind, Kind::Code { .. }))
            .filter(|key| !key.shift.is_empty() && key.shift != key.label)
            .count();
        assert!(doubled > 20, "only {doubled} Thai keys carry a second letter");
    }

    #[test]
    fn thai_carries_no_digit_of_its_own() {
        let thai = of(named("thai").expect("thai"));
        let digits = thai
            .keys
            .iter()
            .filter(|key| key.label.chars().all(|one| one.is_ascii_digit()) && !key.label.is_empty())
            .count();
        assert_eq!(digits, 0, "Thai draws a digit, so it does not need the rule below");
    }

    #[test]
    fn the_language_key_always_has_the_numbers_beside_it() {
        for which in Which::ALL {
            let layout = of(which);
            let Some(at) = layout.keys.iter().position(|key| key.kind == Kind::Language) else {
                continue;
            };

            let row = rows(layout)
                .into_iter()
                .find(|row| row.iter().any(|(where_, _)| *where_ == at))
                .expect("the row the language key is on");

            let numbers: Vec<&Key> = row
                .iter()
                .map(|(_, key)| *key)
                .filter(|key| match key.kind {
                    Kind::Symbols => true,
                    Kind::Layout(shelf) => !of(shelf).primary,
                    _ => false,
                })
                .collect();

            assert!(!numbers.is_empty(), "{} can change language and cannot type a digit", layout.name);

            let language = layout.keys[at].width;
            for key in numbers {
                assert!(
                    (key.width - language).abs() < 0.001,
                    "on {} the language key is {language} wide and the numbers key is {}",
                    layout.name,
                    key.width,
                );
            }
        }
    }
}
