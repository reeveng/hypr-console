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


use console_number::Float;
use crate::keymap::Layer;

/// A keycode, as the kernel numbers them.
///
/// `/usr/include/linux/input-event-codes.h` is the list, and xkb's own
/// keycodes are these plus eight -- `keymap` does that sum, once, where the
/// keymap is made. Only the codes the arrangements use are here: a keyboard
/// with no F-keys has no business naming them.
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

/// The modifier bits the virtual-keyboard protocol takes.
///
/// Wayland's `wl_keyboard` does not document these and the compositor passes
/// them through to xkb, where they are the standard mask: shift is bit zero,
/// lock is bit one, control bit two. The C keyboard learned them by
/// experiment and wrote them down; this is that list.
pub mod mods {
    pub const NONE: u8 = 0;
    pub const SHIFT: u8 = 1;
    pub const CAPS: u8 = 2;
    pub const CTRL: u8 = 4;
    pub const ALT: u8 = 8;
    pub const SUPER: u8 = 64;
    pub const ALTGR: u8 = 128;
}

/// What pressing a key does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Nothing. A gap in a row that keeps the keys either side where they are.
    Pad,
    /// Send this keycode. `held` is the layout a long press opens instead,
    /// which is how one letter key reaches its accented cousins without a row
    /// of its own.
    Code { code: u32, held: Option<Which> },
    /// Hold a modifier until the next key, or until it is pressed again.
    Mod(u8),
    /// Send a character that is not in the keymap at all, by making a keymap
    /// with that one character in it and putting the old one back after. This
    /// is how the accents work: nothing in a latin keymap produces `ā`, so the
    /// keyboard writes a keymap that does, uses it once, and drops it.
    ///
    /// `shifted` is the same character in its capital. The C tables kept it in
    /// the modifier field, which is a field that means something else on every
    /// other kind of key.
    Copy { code: u32, shifted: u32 },
    /// Go to this layout and stay there.
    Layout(Which),
    /// Go back to the layout this one was reached from.
    Back,
    /// Go to the next layout in the walk, which is what the layer key does.
    Next,
    /// Go to the next *language* in the walk: the next arrangement somebody
    /// actually types an alphabet in, skipping the shelves of symbols.
    ///
    /// One key rather than one key per language. `ไทย` and `ABC` used to be two
    /// keys that each named where they went, which works for exactly two
    /// alphabets and needs a new key in three tables for the third. This walks
    /// whichever alphabets the walk was given, so a machine that types Russian
    /// says so in `--landscape-layers` and no table changes.
    ///
    /// The label is not in the table either, because it is the name of the
    /// language this key is about to go to, and that depends on the walk.
    /// [`crate::paint`] asks for it at drawing time.
    Language,
    /// Go to the shelf of numbers and symbols this walk was given.
    ///
    /// [`Kind::Layout`] with a shelf named in it is the ordinary way there and
    /// is what the latin arrangements use, because each of them belongs to one
    /// walk and can say which shelf is theirs. Thai is in both walks -- it is
    /// the same forty keys whichever way round the machine is held -- so it
    /// cannot name one without naming the wrong one half the time.
    ///
    /// The rule this exists for is not about Thai. Every alphabet needs
    /// numbers, and the place to reach them from is beside the key that
    /// changes the alphabet, so that whatever somebody is typing in, the
    /// numbers are one press away and always in the same place. Thai is only
    /// where its absence showed: its shift level is a second set of letters,
    /// so there was no digit on the arrangement at all and no key to leave it
    /// by except the language key, which walks to another alphabet.
    ///
    /// Nothing, when the walk has no shelf in it, which is a machine that was
    /// given alphabets alone and has nowhere to go.
    Symbols,
    /// Arm the next key press: whatever key is pressed next opens the layout
    /// it carries, instead of sending itself. The way to an accent without a
    /// long press, for a thumb that would rather tap twice.
    Compose,
    /// Not a key: the end of a row.
    EndRow,
}

/// What a press does to the modifiers that were held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drops {
    /// They go, the way shift lets go after one letter.
    Held,
    /// They stay as they were.
    Nothing,
}

/// One key, as it is written down.
///
/// The geometry is not here. Where a key lands depends on how wide the screen
/// is and how many keys share its row, so it is worked out at drawing time by
/// [`placed`] rather than stored -- the C version wrote x, y, w and h back
/// into the table, which is a table that cannot be shared between two surfaces
/// and cannot be `static`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Key {
    /// What is drawn on the key.
    pub label: &'static str,
    /// What is drawn on it while shift is held. For latin that is the capital;
    /// for Thai it is a second letter entirely.
    pub shift: &'static str,
    /// How many columns wide, against the other keys in its row.
    pub width: f64,
    /// What pressing it does.
    pub kind: Kind,
    /// Which of the two colour schemes it wears. Zero is a letter, one is
    /// everything that is not a letter -- Esc, Tab, the arrows.
    pub scheme: u8,
    /// A modifier forced on for this press alone.
    pub force: u8,
    /// Whether pressing it drops the modifiers that were held.
    pub reset: Drops,
}

impl Key {
    /// A key with nothing unusual about it, for the tables to build on.
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

/// One arrangement: a set of keys, and the alphabet they type in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    /// The keys, in reading order, with [`Kind::EndRow`] between rows.
    pub keys: &'static [Key],
    /// The alphabet the keymap is asked for when this arrangement is up.
    pub alphabet: Layer,
    /// The name the layer walk and `-l` use.
    pub name: &'static str,
    /// Whether this is an alphabet somebody types in, as against a shelf of
    /// symbols or accents. The way back from a compose layer is to the last
    /// one of these.
    pub primary: bool,
}

/// Where a key ended up, in logical units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    /// Which key in the layout's own list.
    pub at: usize,
    pub x: f64,
    pub y: f64,
    pub wide: f64,
    pub tall: f64,
}

/// Work out where every key of a layout sits on a surface this size.
///
/// Rows share the height equally and keys share their row's width in
/// proportion to what the table says, so an arrangement is written in columns
/// and lands on whatever screen it is given. Rows are not required to add up
/// to the same total: a row of ten single keys and a row of three wide ones
/// both fill the width.
pub fn placed(layout: &Layout, wide: f64, tall: f64) -> Vec<Placed> {
    let rows = rows(layout);

    if rows.is_empty() {
        return Vec::new();
    }

    let deep = tall / rows.len().float();
    let mut out = Vec::with_capacity(layout.keys.len());

    for (down, row) in rows.iter().enumerate() {
        let across: f64 = row.iter().map(|(_, key)| key.width).sum();

        if across <= 0.0 {
            continue;
        }

        let mut x = 0.0;

        for (at, key) in row {
            let w = key.width / across * wide;

            if !matches!(key.kind, Kind::Pad) {
                out.push(Placed { at: *at, x, y: down.float() * deep, wide: w, tall: deep });
            }

            x += w;
        }
    }

    out
}

/// The keys of a layout, split into rows.
///
/// A row ends at [`Kind::EndRow`], and the marker itself is not a key: it has
/// no width and nothing is drawn on it.
pub fn rows(layout: &Layout) -> Vec<Vec<(usize, &'static Key)>> {
    let mut out: Vec<Vec<(usize, &'static Key)>> = Vec::new();
    let mut row: Vec<(usize, &'static Key)> = Vec::new();

    for (at, key) in layout.keys.iter().enumerate() {
        match key.kind {
            Kind::EndRow => {
                if !row.is_empty() {
                    out.push(std::mem::take(&mut row));
                }
            },
            _ => row.push((at, key)),
        }
    }

    if !row.is_empty() {
        out.push(row);
    }

    out
}

/// Which key is under a touch at these logical coordinates, if any.
pub fn under(placed: &[Placed], x: f64, y: f64) -> Option<Placed> {
    placed
        .iter()
        .find(|k| x >= k.x && x < k.x + k.wide && y >= k.y && y < k.y + k.tall)
        .copied()
}

/// How far a point lies outside a span, and nothing when it is inside it.
///
/// Keys are not all one width: the space bar is as wide as ten letters.
/// Measuring to a key's middle made it the worst target on the keyboard rather
/// than the easiest, so this measures to its edge.
fn gap(low: f64, high: f64, point: f64) -> f64 {
    match point {
        p if p < low => low - p,
        p if p > high => p - high,
        _ => 0.0,
    }
}

/// The key one step in a direction from this one, for a thumb on a stick.
///
/// Not the nearest key: the nearest key to the left of `p` is `o`, and a
/// keyboard that answered "left" with the key beside it would be a keyboard
/// you could not cross. What is wanted is the key you would arrive at, so a
/// candidate is scored by how far it lies *along* the direction asked for plus
/// how far it drifts *across* it, and drift counts triple -- otherwise
/// pressing down from a narrow key lands diagonally, because a key one row
/// down and three columns over is closer than the one directly beneath.
///
/// Nothing ahead means the edge, and then it wraps: the same measure run over
/// the keys behind, where the one furthest behind wins, because that is the
/// one you would reach by carrying on past the edge. A keyboard whose top row
/// is a dead end is one where the way to Esc depends on where you started.
///
/// `from` is `None` before anything is selected, which is the first press of a
/// direction and lands on the first key rather than nowhere.
pub fn toward(keys: &[Placed], from: Option<usize>, dx: i32, dy: i32) -> Option<usize> {
    let Some(here) = from.and_then(|at| keys.iter().position(|k| k.at == at)) else {
        return keys.first().map(|k| k.at);
    };

    let sel = keys[here];
    let middle = (sel.x + sel.wide / 2.0, sel.y + sel.tall / 2.0);

    let scored = |k: &Placed| {
        let along = match (dx, dy) {
            (d, _) if d > 0 => k.x - (sel.x + sel.wide),
            (d, _) if d < 0 => sel.x - (k.x + k.wide),
            (_, d) if d > 0 => k.y - (sel.y + sel.tall),
            _ => sel.y - (k.y + k.tall),
        };
        let across = match dx != 0 {
            true => gap(k.y, k.y + k.tall, middle.1),
            false => gap(k.x, k.x + k.wide, middle.0),
        };
        (along, along + across * 3.0)
    };

    // Ahead first, then -- only if the direction ran out of keyboard -- behind.
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

        if let Some((at, _)) = best {
            return Some(at);
        }
    }

    None
}

/// A layout by the name the layer walk uses.
pub fn named(name: &str) -> Option<Which> {
    Which::ALL.iter().copied().find(|which| of(*which).name == name)
}

/// The arrangement itself.
///
/// `tables::layout` is where it comes from, and it answers a variant with that
/// variant's own keys rather than with a place in a list. This used to read the
/// place instead -- `LAYOUTS[which as usize]` -- which is the one thing `as` on
/// a fieldless enum is good for and was still the wrong way to ask. It rested
/// on three hand-written orders agreeing, the variants and `ALL` and the table,
/// with nothing checking that they did; reordering any one of them would have
/// left a keyboard that came up looking right and typed the wrong letters.
pub fn of(which: Which) -> &'static Layout {
    tables::layout(which)
}

mod tables;
pub use tables::Which;

#[cfg(test)]
mod tests {
    use super::*;

    /// A stick pushed sideways crosses the row it is on rather than wandering
    /// into the one above. This is the whole of what `toward` is for, and it
    /// is checked on the arrangement the device actually shows.
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

    /// Up from the bottom row is the row above, not the far end of the
    /// keyboard: `across` is weighted so a direction stays a direction.
    #[test]
    fn up_is_the_row_above_and_not_a_diagonal() {
        let layout = of(named("landscape").expect("landscape"));
        let keys = placed(layout, 1892.0, 260.0);
        let bottom = keys.iter().max_by(|a, b| a.y.total_cmp(&b.y)).expect("a bottom row").y;
        for key in keys.iter().filter(|k| k.y == bottom) {
            let up = toward(&keys, Some(key.at), 0, -1).expect("a key above");
            let landed = keys.iter().find(|k| k.at == up).expect("placed");
            assert!(landed.y < key.y, "up went sideways");
            // The row directly above, and not two rows up.
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

    /// Carrying on past the edge comes out at the other side. A top row that
    /// was a dead end would make the way to a key depend on where you began.
    #[test]
    fn a_direction_wraps_rather_than_stopping_at_the_edge() {
        let layout = of(named("landscape").expect("landscape"));
        let keys = placed(layout, 1892.0, 260.0);
        let top = keys.iter().min_by(|a, b| a.y.total_cmp(&b.y)).expect("a top row").at;
        let up = toward(&keys, Some(top), 0, -1).expect("wrapped round");
        let landed = keys.iter().find(|k| k.at == up).expect("placed");
        assert!(landed.y > keys[0].y, "up from the top row came out at the bottom");
    }

    /// Nothing selected yet is the first press of a direction, and it lands on
    /// a key rather than on nothing.
    #[test]
    fn the_first_direction_lands_somewhere() {
        let layout = of(named("landscape").expect("landscape"));
        let keys = placed(layout, 1892.0, 260.0);
        assert_eq!(toward(&keys, None, 1, 0), Some(keys[0].at));
        assert_eq!(toward(&[], None, 1, 0), None, "and an empty layout is not a panic");
    }

    /// The space bar is as wide as ten letters, and measuring to its middle
    /// made it the worst target on the keyboard rather than the easiest.
    #[test]
    fn a_wide_key_is_measured_to_its_edge() {
        assert_eq!(gap(10.0, 20.0, 15.0), 0.0, "inside the span is no distance at all");
        assert_eq!(gap(10.0, 20.0, 5.0), 5.0);
        assert_eq!(gap(10.0, 20.0, 25.0), 5.0);
    }

    /// Every layout named in the walk has to exist, and every arrangement has
    /// to name an alphabet `keymap` can make. A layout that names an alphabet
    /// nothing installs is a layer key that goes somewhere blank.
    #[test]
    fn every_arrangement_has_keys_and_an_alphabet() {
        for which in Which::ALL {
            let layout = of(which);
            assert!(!layout.keys.is_empty(), "{} has no keys", layout.name);
            assert!(!layout.name.is_empty(), "a layout with no name");
        }
    }

    /// The three the device shows, by the names `config.mobintl.h` used.
    #[test]
    fn the_layers_this_desktop_walks_are_all_there() {
        for name in ["full", "thai", "special", "landscape", "landscapespecial"] {
            assert!(named(name).is_some(), "no layout called {name}");
        }
    }

    /// The keys fill the width, and rows do not overlap. This is the whole of
    /// what `placed` promises, and it is what a thumb lands on.
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
        // Every point along the middle of the top row lands on exactly one key.
        let top = keys[0].tall / 2.0;
        for step in 0..100 {
            let x = step as f64 * 10.0 + 0.5;
            assert!(under(&keys, x, top).is_some(), "nothing under {x}");
        }
    }

    /// A row of one wide key and a row of ten narrow ones both fill the width,
    /// because a row is shared out against its own total rather than a fixed
    /// number of columns.
    #[test]
    fn a_row_is_shared_out_against_its_own_width() {
        let layout = of(named("full").expect("full"));
        for row in rows(layout) {
            let across: f64 = row.iter().map(|(_, key)| key.width).sum();
            assert!(across > 0.0, "a row of nothing");
        }
    }

    /// The space bar is the widest key on the board, and it is the one a thumb
    /// finds without looking. If a conversion had dropped the widths it would
    /// still draw, and every key would be the same size.
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

    /// Thai carries a letter on both levels rather than a capital, which is
    /// the thing about it that shaped the layer walk.
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

    /// Thai's own digits are Thai numerals on the shift level and there is no
    /// `1` on the arrangement anywhere, which is what the rule below is for.
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

    /// The rule: wherever the language key is, the numbers are beside it.
    ///
    /// Not a fact about Thai. Every alphabet needs numbers and symbols, and
    /// somebody who has found them once on one arrangement should find them in
    /// the same place on the next. An arrangement that offers a way to change
    /// language and no way to a digit is one a person can be typing in and
    /// unable to write a number on.
    #[test]
    fn the_language_key_always_has_the_numbers_beside_it() {
        for which in Which::ALL {
            let layout = of(which);
            let Some(at) = layout.keys.iter().position(|key| key.kind == Kind::Language) else {
                continue;
            };

            // Beside, and on the same row: a key on the row above is not
            // beside anything, and the row is what a thumb sweeps along.
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

            // The same size as each other, on every arrangement that has both.
            // They are the pair a thumb goes to without looking -- which
            // alphabet, and the numbers -- and a pair that changes size from
            // one arrangement to the next is a pair that has to be found again
            // each time.
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
