//! The controls a value is changed with in place: a switch, a slider and a
//! picker, each said without a screen.
//!
//! A row here used to change a setting by being chosen. Wi-Fi said `On` in the
//! corner and a press of A ran the command that turned it off, so the row was a
//! button wearing a word, and a finger had to know that the word was the
//! switch. Every console anyone arrives from draws the switch instead --
//! PlayStation's settings, the control centre on every phone -- because a knob
//! on the side it is set to is read at arm's length and a word in the corner is
//! not. The same goes for a level with nothing to drag and a choice of three
//! that opens a list to pick one.
//!
//! **A control is not a widget.** It holds nothing. It is handed the value it
//! is showing and the frame it is drawn in, and it answers with shapes; it is
//! handed a press, and it answers with the value the press leaves it at. Where
//! the frame is, is the business of whatever arranges the page, so nothing here
//! is a layout -- which is the line the `no-gtk.md` note in Orbit drew when it
//! put a toolkit down, and the line `console-core-shapes` keeps by being a list
//! rather than an engine. What is kept is iced's arrangement and Elm's before
//! it: the state belongs to the program, the view is a function of it, and an
//! update is a function from the state and a message to the next state.
//!
//! **One press is the same press from every hand.** A control is told what
//! `console_core_highlight::Press` says -- pointed, selected, tapped, dragged,
//! hovered -- and the d-pad, the keys, a finger and a mouse are all one of
//! those before a control hears of them. That is Blender's rule about a value:
//! a number field there answers a click, a drag along it, the wheel and a typed
//! number, and none of them is a second control. Here a slider is stepped by
//! the d-pad, jumped by a tap, scrubbed by a drag, and the three meet in one
//! function that cannot answer one hand and forget another.
//!
//! **Up and down are never a control's.** A control lives on a row in a list,
//! and the list is what up and down walk. So every control hands them back,
//! with Tab and Shift-Tab, and keeps left and right -- which on this desktop
//! are already what a row's level is moved by, and what a question's answers
//! are walked with. A press a control has no use for passes, rather than being
//! taken and ignored, so a row cannot become the place the d-pad stops
//! working.
//!
//! **What is measured is handed in.** A picker's words are as wide as the face
//! says they are, and the face is not in this room, so each choice arrives
//! with its width beside it. A control that measured would be a control that
//! needed a screen to be asked anything, and the answers here are the ones a
//! check asserts against with no screen at all.
//!
//! The inks are chosen in one place, `Inks::worn`, rather than by each control
//! or each caller. A control is cut into the row it sits on -- a track of the
//! deepest ground, the part that is set in the colour a filled length already
//! wears, a knob in the colour of everything that is read -- so it reads the
//! same on a row the highlight is standing on and on one it is not.

use console_core_color::Oklch;
use console_core_color::palette::Wearing;
use console_core_geometry::{Point, Rectangle, Size};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_shapes::{Edge, Panel, Round};

pub use console_core_highlight::{Answer, Heading, Press};

pub mod picker;
pub mod slider;
pub mod switch;

pub use picker::{Choice, Form, Lettering, Picker};
pub use slider::Slider;
pub use switch::Switch;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Inks {
  pub track: Oklch,
  pub set: Oklch,
  pub knob: Oklch,
  pub said: Oklch,
  pub quiet: Oklch,
}

impl Inks {
  pub fn worn(wearing: &Wearing) -> Result<Inks, Never> {
    Ok(Inks {
      track: wearing.night,
      set: wearing.fill,
      knob: wearing.text,
      said: wearing.text,
      quiet: wearing.edge,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Inside {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Edges {
  pub(crate) left: i32,
  pub(crate) right: i32,
  pub(crate) top: i32,
  pub(crate) wide: u32,
  pub(crate) tall: u32,
}

pub(crate) fn edges(frame: Rectangle<i32>) -> Result<Edges, Never> {
  let Ok(wide) = fitted::<i32, u32>(frame.size.width.max(0));
  let Ok(tall) = fitted::<i32, u32>(frame.size.height.max(0));

  Ok(Edges {
    left: frame.origin.x,
    right: frame.origin.x.saturating_add(frame.size.width.max(0)),
    top: frame.origin.y,
    wide,
    tall,
  })
}

pub(crate) fn inside(frame: Rectangle<i32>, at: Point<i32>) -> Result<Inside, Never> {
  let Ok(found) = console_core_highlight::under(at, std::slice::from_ref(&frame));

  Ok(match found {
    Some(_) => Inside::Yes,
    None => Inside::No,
  })
}

pub(crate) fn pill(frame: Rectangle<i32>, fill: Oklch) -> Result<Panel, Never> {
  let Ok(edges) = edges(frame);
  let Ok(round) = rounded(edges.tall);

  Ok(Panel {
    at: frame.origin,
    size: Size {
      width: edges.wide,
      height: edges.tall,
    },
    round,
    fill,
    edge: Edge::None,
  })
}

pub(crate) fn rounded(tall: u32) -> Result<Round, Never> {
  Ok(Round(tall.saturating_div(2)))
}

pub(crate) fn whole(many: u32) -> Result<i32, Never> {
  fitted::<u32, i32>(many)
}

pub(crate) fn toward<T: PartialEq>(now: T, wanted: T) -> Result<Answer<T>, Never> {
  Ok(
    match ::core::primitive::bool::then_some(now == wanted, ()) {
      ::core::option::Option::Some(()) => Answer::Same,
      ::core::option::Option::None => Answer::Now(wanted),
    },
  )
}

#[cfg(test)]
pub(crate) mod testing {
  use super::*;

  pub(crate) const INKS: Inks = Inks {
    track: Oklch {
      lightness: 0.1,
      ..Oklch::BLACK
    },
    set: Oklch {
      lightness: 0.3,
      ..Oklch::BLACK
    },
    knob: Oklch {
      lightness: 0.9,
      ..Oklch::BLACK
    },
    said: Oklch {
      lightness: 0.8,
      ..Oklch::BLACK
    },
    quiet: Oklch {
      lightness: 0.5,
      ..Oklch::BLACK
    },
  };

  pub(crate) fn frame(at: Point<i32>, size: Size<i32>) -> Result<Rectangle<i32>, Never> {
    Ok(Rectangle { origin: at, size })
  }

  pub(crate) fn panel_of(
    shapes: &[console_core_shapes::Shape],
    at: u32,
  ) -> Result<Panel, Box<dyn std::error::Error>> {
    let Ok(standing) = console_core_number_conversion::index(at);

    match shapes.get(standing) {
      Some(console_core_shapes::Shape::Panel(panel)) => Ok(*panel),
      Some(other) => Err(Box::from(format!("shape {at} is not a panel: {other:?}"))),
      None => Err(Box::from(format!("there is no shape {at}"))),
    }
  }
}
