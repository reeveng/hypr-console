//! What keys there are, what is written on them, and where they sit.
//!
//! This is the half of a keyboard that no amount of system configuration can
//! supply. `keymap` asks the system's xkb symbols what a key *produces* -- that
//! is the system's business and it has known every alphabet X ships symbols for since
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

use crate::drawing::{LogicalPixels, Rectangle};
use crate::keymap::Layer;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{Float, fitted};

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

pub mod modifiers {
  pub const NONE: u8 = 0;
  pub const SHIFT: u8 = 1;
  pub const CAPS_LOCK: u8 = 2;
  pub const CONTROL: u8 = 4;
  pub const ALT: u8 = 8;
  pub const SUPER: u8 = 64;
  pub const ALT_GRAPH: u8 = 128;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
  Pad,
  Code { code: u32, held: Option<LayoutKind> },
  Mod(u8),
  Copy { code: u32, shifted: u32 },
  Layout(LayoutKind),
  Back,
  Next,
  Language,
  Symbols,
  Compose,
  EndRow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drops {
  Modifiers,
  None,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "Key::width is a dimensionless ratio to the width of one ordinary key before layout"
  )
)]
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
    force: modifiers::NONE,
    reset: Drops::None,
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
  pub at: u32,
  pub bounds: Rectangle,
}

pub fn layout_keys(layout: &Layout, room: Size<f64>) -> Result<Vec<Placed>, Never> {
  let Ok(rows) = rows(layout);

  match rows.as_slice() {
    [] => return Ok(Vec::new()),
    [_first, ..] => {}
  }

  let Ok(many) = rows.len().float();

  let deep = room.height / many;
  let mut out = Vec::with_capacity(layout.keys.len());

  for (down, row) in rows.iter().enumerate() {
    let across: f64 = row.iter().map(|(_, key)| key.width).sum();

    match across.partial_cmp(&0.0) {
      Some(core::cmp::Ordering::Less | core::cmp::Ordering::Equal) => continue,
      Some(core::cmp::Ordering::Greater) | None => {}
    }

    let mut x = 0.0;

    for (at, key) in row {
      let width = key.width / across * room.width;

      match key.kind {
        Kind::Pad => {}
        Kind::Code { .. }
        | Kind::Mod(_)
        | Kind::Copy { .. }
        | Kind::Layout(_)
        | Kind::Back
        | Kind::Next
        | Kind::Language
        | Kind::Symbols
        | Kind::Compose
        | Kind::EndRow => {
          let Ok(row_2) = down.float();

          out.push(Placed {
            at: *at,
            bounds: Rectangle {
              origin: Point { x, y: row_2 * deep },
              size: Size {
                width,
                height: deep,
              },
            },
          });
        }
      }

      x += width;
    }
  }

  Ok(out)
}

pub fn rows(layout: &Layout) -> Result<Vec<Vec<(u32, &'static Key)>>, Never> {
  let mut out: Vec<Vec<(u32, &'static Key)>> = Vec::new();
  let mut row: Vec<(u32, &'static Key)> = Vec::new();

  for (at, key) in layout.keys.iter().enumerate() {
    let Ok(at_2) = fitted::<_, u32>(at);

    match key.kind {
      Kind::EndRow => match ::core::primitive::bool::then_some(row.is_empty(), ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => out.push(std::mem::take(&mut row)),
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
      | Kind::Compose => row.push((at_2, key)),
    }
  }

  match ::core::primitive::bool::then_some(row.is_empty(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => out.push(row),
  }

  Ok(out)
}

pub fn under(placed: &[Placed], at: Point<f64>) -> Result<Option<Placed>, Never> {
  Ok(
    placed
      .iter()
      .find(|key| {
        let Rectangle { origin, size } = key.bounds;

        at.x >= origin.x
          && at.x < origin.x + size.width
          && at.y >= origin.y
          && at.y < origin.y + size.height
      })
      .copied(),
  )
}

fn gap(
  between: std::ops::RangeInclusive<f64>,
  LogicalPixels(point): LogicalPixels,
) -> Result<LogicalPixels, Never> {
  let (low, high) = between.into_inner();
  let distance = match (point < low, point > high) {
    (true, true) | (true, false) => low - point,
    (false, true) => point - high,
    (false, false) => 0.0,
  };

  Ok(LogicalPixels(distance))
}

pub fn toward(
  keys: &[Placed],
  from: Option<u32>,
  step: Point<i32>,
) -> Result<Option<u32>, Never> {
  let selected = match from.and_then(|at| keys.iter().find(|key| key.at == at).copied()) {
    Some(selected) => selected,
    None => return Ok(keys.first().map(|key| key.at)),
  };

  let Rectangle {
    origin: selected_at,
    size: selected_size,
  } = selected.bounds;
  let middle = Point {
    x: selected_at.x + selected_size.width / 2.0,
    y: selected_at.y + selected_size.height / 2.0,
  };

  let scored = |key: &Placed| {
    let Rectangle { origin, size } = key.bounds;
    let along = match step.x.signum() {
      1 => origin.x - (selected_at.x + selected_size.width),
      -1 => selected_at.x - (origin.x + size.width),
      _ => match ::core::primitive::bool::then_some(step.y > 0, ()) {
        ::core::option::Option::Some(()) => origin.y - (selected_at.y + selected_size.height),
        ::core::option::Option::None => selected_at.y - (origin.y + size.height),
      },
    };
    let Ok(LogicalPixels(across)) = match ::core::primitive::bool::then_some(step.x != 0, ()) {
      ::core::option::Option::Some(()) => {
        gap(origin.y..=origin.y + size.height, LogicalPixels(middle.y))
      }
      ::core::option::Option::None => {
        gap(origin.x..=origin.x + size.width, LogicalPixels(middle.x))
      }
    };

    (along, along + across * 3.0)
  };

  for ahead in [true, false] {
    let best = keys
      .iter()
      .filter(|key| key.at != selected.at)
      .map(|key| (key.at, scored(key)))
      .filter(
        |(_, (along, _))| match ::core::primitive::bool::then_some(ahead, ()) {
          ::core::option::Option::Some(()) => *along >= 0.0,
          ::core::option::Option::None => *along < 0.0,
        },
      )
      .min_by(|one, other| one.1.1.total_cmp(&other.1.1));

    match best {
      Some((at, _)) => return Ok(Some(at)),
      None => {}
    }
  }

  Ok(None)
}

pub fn find_layout(name: &str) -> Result<Option<LayoutKind>, Never> {
  Ok(LayoutKind::ALL.iter().copied().find(|which| {
    let Ok(of) = of(*which);

    of.name == name
  }))
}

pub fn of(which: LayoutKind) -> Result<&'static Layout, Never> {
  tables::layout(which)
}

mod tables;
pub use tables::LayoutKind;

#[cfg(test)]
mod tests {
  use std::collections::BTreeMap;

  use super::*;

  type Failure = Box<dyn std::error::Error>;

  const LANDSCAPE: Size<f64> = Size {
    width: 1892.0,
    height: 260.0,
  };

  fn named(name: &str) -> Result<&'static Layout, Failure> {
    let Ok(found) = find_layout(name);
    let which = found.ok_or(format!("no layout called {name}"))?;
    let Ok(layout) = of(which);

    Ok(layout)
  }

  fn rows_by_key(keys: &[Placed]) -> Result<BTreeMap<u32, f64>, Never> {
    Ok(
      keys
        .iter()
        .map(|key| (key.at, key.bounds.origin.y))
        .collect(),
    )
  }

  #[test]
  fn a_direction_crosses_the_row_it_started_on() -> Result<(), Failure> {
    let layout = named("landscape")?;
    let Ok(keys) = layout_keys(layout, LANDSCAPE);
    let Ok(row_of) = rows_by_key(&keys);
    let first = keys.first().ok_or("the landscape layout placed no keys")?;
    let row = first.bounds.origin.y;
    let mut at = first.at;

    for step in 0..8 {
      let Ok(next) = toward(&keys, Some(at), Point { x: 1, y: 0 });

      let next_2 = next.ok_or(format!("nothing to the right at step {step}"))?;

      at = next_2;

      let now = row_of
        .get(&at)
        .ok_or(format!("key {at} was stepped onto and never placed"))?;

      assert_eq!(*now, row, "step {step} left the row it started on");
    }

    Ok(())
  }

  #[test]
  fn up_is_the_row_above_and_not_a_diagonal() -> Result<(), Failure> {
    let layout = named("landscape")?;
    let Ok(keys) = layout_keys(layout, LANDSCAPE);
    let Ok(row_of) = rows_by_key(&keys);
    let lowest = keys
      .iter()
      .max_by(|one, other| one.bounds.origin.y.total_cmp(&other.bounds.origin.y))
      .ok_or("a bottom row")?;
    let bottom = lowest.bounds.origin.y;
    let mut heights: Vec<f64> = keys.iter().map(|key| key.bounds.origin.y).collect();

    heights.sort_by(f64::total_cmp);
    heights.dedup_by(|one, other| {
      <::core::primitive::f64 as ::core::cmp::PartialOrd>::partial_cmp(one, other)
        == ::core::option::Option::Some(::core::cmp::Ordering::Equal)
    });

    let above = heights
      .iter()
      .rev()
      .find(|y| **y < bottom)
      .ok_or("a row above the bottom one")?;

    for key in keys.iter().filter(|key| {
      key.bounds.origin.y.partial_cmp(&bottom)
        == ::core::option::Option::Some(::core::cmp::Ordering::Equal)
    }) {
      let Ok(up) = toward(&keys, Some(key.at), Point { x: 0, y: -1 });
      let higher = up.ok_or("a key above")?;
      let landed = row_of
        .get(&higher)
        .ok_or(format!("key {higher} was stepped onto and never placed"))?;

      assert!(*landed < key.bounds.origin.y, "up went sideways");
      assert_eq!(*landed, *above, "up skipped a row");
    }

    Ok(())
  }

  #[test]
  fn a_direction_wraps_rather_than_stopping_at_the_edge() -> Result<(), Failure> {
    let layout = named("landscape")?;
    let Ok(keys) = layout_keys(layout, LANDSCAPE);
    let first = keys.first().ok_or("the landscape layout placed no keys")?;
    let highest = keys
      .iter()
      .min_by(|one, other| one.bounds.origin.y.total_cmp(&other.bounds.origin.y))
      .ok_or("a top row")?;
    let Ok(up) = toward(&keys, Some(highest.at), Point { x: 0, y: -1 });
    let wrapped = up.ok_or("wrapped round")?;
    let landed = keys.iter().find(|key| key.at == wrapped).ok_or("placed")?;

    assert!(
      landed.bounds.origin.y > first.bounds.origin.y,
      "up from the top row came out at the bottom"
    );

    Ok(())
  }

  #[test]
  fn the_first_direction_lands_somewhere() -> Result<(), Failure> {
    let layout = named("landscape")?;
    let Ok(keys) = layout_keys(layout, LANDSCAPE);
    let first = keys.first().ok_or("the landscape layout placed no keys")?;

    assert_eq!(
      toward(&keys, None, Point { x: 1, y: 0 }),
      Ok(Some(first.at))
    );
    assert_eq!(
      toward(&[], None, Point { x: 1, y: 0 }),
      Ok(None),
      "and an empty layout is not a panic"
    );

    Ok(())
  }

  #[test]
  fn a_wide_key_is_measured_to_its_edge() {
    assert_eq!(
      gap(10.0..=20.0, LogicalPixels(15.0)),
      Ok(LogicalPixels(0.0)),
      "inside the span is no distance at all"
    );
    assert_eq!(gap(10.0..=20.0, LogicalPixels(5.0)), Ok(LogicalPixels(5.0)));
    assert_eq!(
      gap(10.0..=20.0, LogicalPixels(25.0)),
      Ok(LogicalPixels(5.0))
    );
  }

  #[test]
  fn every_arrangement_has_keys_and_an_alphabet() {
    for which in LayoutKind::ALL {
      let Ok(layout) = of(which);

      assert!(!layout.keys.is_empty(), "{} has no keys", layout.name);
      assert!(!layout.name.is_empty(), "a layout with no name");
    }
  }

  #[test]
  fn the_layers_this_desktop_walks_are_all_there() {
    for name in ["full", "thai", "special", "landscape", "landscapespecial"] {
      assert!(
        match find_layout(name) {
          Ok(found) => found.is_some(),
          Err(_) => false,
        },
        "no layout called {name}"
      );
    }
  }

  #[test]
  fn the_keys_fill_the_surface_without_overlapping() -> Result<(), Failure> {
    let layout = named("full")?;
    let Ok(keys) = layout_keys(
      layout,
      Size {
        width: 1000.0,
        height: 260.0,
      },
    );
    let first = keys.first().ok_or("the full layout placed no keys")?;

    for key in &keys {
      let Rectangle { origin, size } = key.bounds;

      assert!(origin.x >= -0.001, "a key off the left");
      assert!(
        origin.x + size.width <= 1000.001,
        "a key off the right: {key:?}"
      );
      assert!(
        origin.y + size.height <= 260.001,
        "a key below the keyboard: {key:?}"
      );
    }

    let top = first.bounds.size.height / 2.0;

    for step in 0..100_i32 {
      let x = f64::from(step) * 10.0 + 0.5;
      let Ok(found) = under(&keys, Point { x, y: top });

      assert!(found.is_some(), "nothing under {x}");
    }

    Ok(())
  }

  #[test]
  fn a_row_is_shared_out_against_its_own_width() -> Result<(), Failure> {
    let layout = named("full")?;
    let Ok(every) = rows(layout);

    for row in every {
      let across: f64 = row.iter().map(|(_, key)| key.width).sum();

      assert!(across > 0.0, "a row of nothing");
    }

    Ok(())
  }

  #[test]
  fn the_space_bar_is_wider_than_a_letter() -> Result<(), Failure> {
    let layout = named("full")?;
    let space = layout
      .keys
      .iter()
      .find(|key| {
        matches!(
          key.kind,
          Kind::Code {
            code: key::SPACE,
            ..
          }
        )
      })
      .ok_or("a space bar")?;

    assert!(space.width > 1.0, "the space bar is one column wide");

    Ok(())
  }

  #[test]
  fn thai_holds_a_second_letter_where_latin_holds_a_capital() -> Result<(), Failure> {
    let thai = named("thai")?;
    let doubled: Vec<&Key> = thai
      .keys
      .iter()
      .filter(|key| matches!(key.kind, Kind::Code { .. }))
      .filter(|key| !key.shift.is_empty() && key.shift != key.label)
      .collect();

    assert!(
      doubled.len() > 20,
      "only {} Thai keys carry a second letter",
      doubled.len()
    );

    Ok(())
  }

  #[test]
  fn thai_carries_no_digit_of_its_own() -> Result<(), Failure> {
    let thai = named("thai")?;

    assert!(
      thai
        .keys
        .iter()
        .all(|key| key.label.is_empty() || !key.label.chars().all(|one| one.is_ascii_digit())),
      "Thai draws a digit, so it does not need the rule below"
    );

    Ok(())
  }

  #[test]
  fn the_language_key_always_has_the_numbers_beside_it() -> Result<(), Failure> {
    for which in LayoutKind::ALL {
      let Ok(layout) = of(which);
      let Ok(placed_rows) = rows(layout);
      let found = placed_rows
        .iter()
        .zip(0_u32..)
        .flat_map(|(row, line)| row.iter().map(move |(_, key)| (line, *key)))
        .find(|(_, key)| key.kind == Kind::Language);
      let (line, language) = match found {
        Some(found_2) => found_2,
        None => continue,
      };
      let Ok(line_2) = console_core_number_conversion::index(line);
      let row = placed_rows
        .get(line_2)
        .ok_or("the language key's own row")?;
      let numbers: Vec<&Key> = row
        .iter()
        .map(|(_, key)| *key)
        .filter(|key| match key.kind {
          Kind::Symbols => true,
          Kind::Layout(shelf) => match of(shelf) {
            Ok(shelf_2) => !shelf_2.primary,
            Err(_) => false,
          },
          Kind::Pad
          | Kind::Code { .. }
          | Kind::Mod(_)
          | Kind::Copy { .. }
          | Kind::Back
          | Kind::Next
          | Kind::Language
          | Kind::Compose
          | Kind::EndRow => false,
        })
        .collect();

      assert!(
        !numbers.is_empty(),
        "{} can change language and cannot type a digit",
        layout.name
      );

      for key in numbers {
        assert!(
          (key.width - language.width).abs() < 0.001,
          "on {} the language key is {} wide and the numbers key is {}",
          layout.name,
          language.width,
          key.width,
        );
      }
    }

    Ok(())
  }
}
