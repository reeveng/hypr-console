//! Drawing one arrangement onto one frame.
//!
//! Everything here is decided somewhere else: `layout` says which keys there
//! are and `layout::layout_keys` says where, `configuration` says what color they are and
//! what font is on them, and `drawing` rasterizes them. This is the half page
//! that puts those together, and it is separate from all three because it is
//! the only part that has to happen inside a frame the compositor is waiting
//! for.

use console_core_fonts::{Font, Weight};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_draw_painting::text::Style;

use crate::configuration::{Configuration, Scheme};
use crate::drawing::{FontDescription, LogicalPixels, Rectangle, Surface, inset};
use crate::layout::{Key, Kind, Layout, Placed, modifiers};

const NO_LANGUAGE_NAMED: &str = "";

const EDGE: LogicalPixels = LogicalPixels(2.0);

pub struct Look<'a> {
  pub configuration: &'a Configuration,
  pub layout: &'a Layout,
  pub keys: &'a [Placed],
  pub pressed: Option<u32>,
  pub held: u8,
  pub selected: Option<u32>,
  pub language: Option<&'a str>,
  pub room: Size<f64>,
}

fn configured_font(description: &str) -> Result<FontDescription, Never> {
  let mut family = description.trim();
  let mut size = 16.0;
  let mut factor = 96.0 / 72.0;

  match family.rsplit_once(' ') {
    Some((name, last)) => {
      let number = match last.strip_suffix("px") {
        Some(number) => number,
        None => last,
      };

      match number.parse::<f64>() {
        Ok(parsed) => {
          family = name;
          size = parsed;
          factor = match ::core::primitive::bool::then_some(last.ends_with("px"), ()) {
            ::core::option::Option::Some(()) => 1.0,
            ::core::option::Option::None => factor,
          };
        }
        Err(fault) => {
          console_core_logging::debug!("using the default size for font {family:?}: {fault}");
        }
      }
    }
    None => {}
  }

  let mut weight = Weight::Plain;
  let mut style = Style::Normal;

  for _word in description.split_whitespace().rev() {
    let (before, last) = match family.rsplit_once(' ') {
      Some(parts) => parts,
      None => break,
    };

    match last.to_ascii_lowercase().as_str() {
      "bold" => weight = Weight::Bold,
      "italic" => style = Style::Italic,
      "oblique" => style = Style::Oblique,
      "normal" | "regular" => {}
      _family => break,
    }

    family = before;
  }

  let Ok(height) = console_core_number_conversion::whole_u32(size * factor);

  Ok(FontDescription {
    font: Font {
      family: family.to_string(),
      height: height.max(1),
    },
    weight,
    style,
  })
}

pub fn keyboard(onto: &mut Surface<'_>, look: &Look) -> Result<(), Never> {
  let Look {
    configuration,
    layout,
    keys,
    pressed,
    held,
    selected,
    language,
    room,
  } = *look;
  let Ok(font) = configured_font(&configuration.font);

  match configuration.schemes.first() {
    Some(first) => {
      let Ok(()) = onto.fill_rectangle(
        first.background,
        Rectangle {
          origin: Point { x: 0.0, y: 0.0 },
          size: room,
        },
        0,
      );
    }
    None => {}
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
    let Ok(()) = one(
      onto,
      key,
      placed,
      &DrawingContext {
        scheme,
        showing,
        held,
        language,
        font: &font,
        rounding,
      },
    );
  }

  Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Showing {
  Pressed,
  Under,
  Plain,
}

struct DrawingContext<'a> {
  scheme: &'a Scheme,
  showing: Showing,
  held: u8,
  language: Option<&'a str>,
  font: &'a FontDescription,
  rounding: i32,
}

fn one(
  onto: &mut Surface<'_>,
  key: &Key,
  placed: &Placed,
  ink: &DrawingContext,
) -> Result<(), Never> {
  let DrawingContext {
    scheme,
    showing,
    held,
    language,
    font,
    rounding,
  } = *ink;
  let at = placed.bounds;
  let face = match showing {
    Showing::Pressed => scheme.high,
    Showing::Under => scheme.selected,
    Showing::Plain => scheme.foreground,
  };
  let Ok(face_bounds) = inset(at, EDGE);
  let Ok(()) = onto.fill_rectangle(face, face_bounds, rounding);

  let shifted = held & (modifiers::SHIFT | modifiers::CAPS_LOCK) != 0;
  let label = match key.kind {
    Kind::Language => match language {
      Some(language_2) => language_2,
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
    | Kind::EndRow => match ::core::primitive::bool::then_some(shifted && !key.shift.is_empty(), ()) {
      ::core::option::Option::Some(()) => key.shift,
      ::core::option::Option::None => key.label,
    },
  };

  match ::core::primitive::bool::then_some(label.is_empty(), ()) {
    ::core::option::Option::Some(()) => return Ok(()),
    ::core::option::Option::None => {}
  }

  let ink_2 = match showing {
    Showing::Pressed => scheme.text_press,
    Showing::Under => scheme.text_selected,
    Showing::Plain => scheme.text,
  };
  let Ok(()) = onto.draw_text(ink_2, at, EDGE, label, font);

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::layout::{find_layout, layout_keys, of};

  #[test]
  fn configured_fonts_keep_their_family_size_and_style() {
    let Ok(default) = configured_font("Noto Sans 16");
    let Ok(pixels) = configured_font("Noto Sans Bold Italic 20px");
    let Ok(family) = configured_font("Noto Sans");

    assert_eq!(default.font.family, "Noto Sans");
    assert_eq!(default.font.height, 21);
    assert_eq!(
      pixels.font,
      Font {
        family: "Noto Sans".to_string(),
        height: 20
      }
    );
    assert_eq!(pixels.weight, Weight::Bold);
    assert_eq!(pixels.style, Style::Italic);
    assert_eq!(family.font.family, "Noto Sans");
  }

  #[test]
  fn what_is_drawn_is_inside_the_keyboard() -> Result<(), &'static str> {
    let Ok(name) = find_layout("full");
    let name_2 = name.ok_or("a layout called full")?;
    let Ok(layout) = of(name_2);
    let Ok(keys) = layout_keys(
      layout,
      Size {
        width: 1024.0,
        height: 260.0,
      },
    );

    for key in &keys {
      let Ok(Rectangle { origin, size }) = inset(key.bounds, EDGE);

      assert!(
        size.width > 0.0 && size.height > 0.0,
        "a key with no face left after its border"
      );
      assert!(origin.x >= 0.0 && origin.y >= 0.0);
      assert!(origin.x + size.width <= 1024.0);
      assert!(origin.y + size.height <= 260.0);
    }

    Ok(())
  }

  #[test]
  fn the_gap_between_two_keys_is_a_border_from_each() {
    let size = Size {
      width: 100.0,
      height: 50.0,
    };
    let Ok(left) = inset(
      Rectangle {
        origin: Point { x: 0.0, y: 0.0 },
        size,
      },
      EDGE,
    );
    let Ok(right) = inset(
      Rectangle {
        origin: Point { x: 100.0, y: 0.0 },
        size,
      },
      EDGE,
    );
    let gap = right.origin.x - (left.origin.x + left.size.width);
    assert!((gap - EDGE.0 * 2.0).abs() < 0.001, "the gap is {gap}");
  }
}
