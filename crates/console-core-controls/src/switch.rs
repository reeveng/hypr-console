//! A setting that is on or off, and the knob that says which.
//!
//! The knob stands at the right when it is on, which is where every switch a
//! person has used puts it and where right on the d-pad already sends a row's
//! level. So right turns it on and left turns it off, and pressing either
//! twice is the same as pressing it once: a thumb that does not know which
//! way the switch is set can say which way it wants it without reading it
//! first. A and a tap turn it over, because a switch is the one control whose
//! whole meaning is the other position.
//!
//! A drag is a finger taking the knob across, and where the finger is decides
//! it: past the middle is on and short of it is off, wherever the finger has
//! wandered up or down. It is never a turn, so a knob pushed to the side it is
//! already on stays there rather than coming back.

use console_core_geometry::{Point, Rectangle, Size};
use console_core_highlight::{Answer, Heading, Press};
use console_core_never::Never;
use console_core_shapes::{Edge, Panel, Shape};

use crate::{Inks, Inside, edges, inside, pill, rounded, toward, whole};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Switch {
  On,
  Off,
}

impl Switch {
  pub fn sized(tall: u32) -> Result<Size<u32>, Never> {
    Ok(Size {
      width: tall.saturating_mul(5).saturating_div(3),
      height: tall,
    })
  }

  pub fn turned(self) -> Result<Switch, Never> {
    Ok(match self {
      Switch::On => Switch::Off,
      Switch::Off => Switch::On,
    })
  }

  pub fn moved(self, heading: Heading) -> Result<Answer<Switch>, Never> {
    match heading {
      Heading::Right => toward(self, Switch::On),
      Heading::Left => toward(self, Switch::Off),
      Heading::Up | Heading::Down | Heading::Next | Heading::Previous => Ok(Answer::Passes),
    }
  }

  pub fn pressed(self, press: Press, frame: Rectangle<i32>) -> Result<Answer<Switch>, Never> {
    let Ok(possible_activation) = press.activation();

    match possible_activation {
      Some(activation) => {
        let Ok(turned) = activation.resolve(
          || Some(self),
          |point| {
            let Ok(landed) = inside(frame, point);

            (landed == Inside::Yes).then_some(self)
          },
        );
        return match turned {
          Some(held) => held.turned().map(Answer::Now),
          None => Ok(Answer::Passes),
        };
      }
      ::core::option::Option::None => {}
    }

    match press {
      Press::Toward(heading) => self.moved(heading),
      Press::Select | Press::Tap(_) | Press::Hover(_) => Ok(Answer::Passes),
      Press::Drag(at) => {
        let Ok(wanted) = side(frame, at);

        toward(self, wanted)
      }
    }
  }

  pub fn shapes(self, frame: Rectangle<i32>, inks: Inks) -> Result<Vec<Shape>, Never> {
    let Ok(edges) = edges(frame);

    let fill = match self {
      Switch::On => inks.set,
      Switch::Off => inks.track,
    };

    let Ok(track) = pill(frame, fill);
    let inset = edges.tall.saturating_div(8);
    let side = edges.tall.saturating_sub(inset.saturating_mul(2));
    let Ok(inset_across) = whole(inset);
    let Ok(knob_wide) = whole(side);

    let across = match self {
      Switch::On => edges
        .right
        .saturating_sub(inset_across)
        .saturating_sub(knob_wide),
      Switch::Off => edges.left.saturating_add(inset_across),
    };

    let Ok(round) = rounded(side);
    let knob = Panel {
      at: Point {
        x: across,
        y: edges.top.saturating_add(inset_across),
      },
      size: Size {
        width: side,
        height: side,
      },
      round,
      fill: inks.knob,
      edge: Edge::None,
    };

    Ok(vec![Shape::Panel(track), Shape::Panel(knob)])
  }
}

fn side(frame: Rectangle<i32>, at: Point<i32>) -> Result<Switch, Never> {
  let Ok(edges) = edges(frame);
  let Ok(wide) = whole(edges.wide);
  let middle = edges.left.saturating_add(wide.saturating_div(2));

  Ok(
    match ::core::primitive::bool::then_some(at.x >= middle, ()) {
      ::core::option::Option::Some(()) => Switch::On,
      ::core::option::Option::None => Switch::Off,
    },
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  use crate::testing::{INKS, frame, panel_of};

  fn placed() -> Result<Rectangle<i32>, Never> {
    frame(
      Point { x: 100, y: 10 },
      Size {
        width: 80,
        height: 48,
      },
    )
  }

  #[test]
  fn right_turns_it_on_and_right_again_leaves_it_on() {
    let Ok(at) = placed();

    assert_eq!(
      Switch::Off.pressed(Press::Toward(Heading::Right), at),
      Ok(Answer::Now(Switch::On))
    );
    assert_eq!(
      Switch::On.pressed(Press::Toward(Heading::Right), at),
      Ok(Answer::Same)
    );
    assert_eq!(
      Switch::On.pressed(Press::Toward(Heading::Left), at),
      Ok(Answer::Now(Switch::Off))
    );
  }

  #[test]
  fn a_and_a_tap_on_it_turn_it_over_whichever_way_it_was() {
    let Ok(at) = placed();
    let on_it = Point { x: 110, y: 20 };

    assert_eq!(
      Switch::Off.pressed(Press::Select, at),
      Ok(Answer::Now(Switch::On))
    );
    assert_eq!(
      Switch::On.pressed(Press::Select, at),
      Ok(Answer::Now(Switch::Off))
    );
    assert_eq!(
      Switch::On.pressed(Press::Tap(on_it), at),
      Ok(Answer::Now(Switch::Off))
    );
  }

  #[test]
  fn a_tap_beside_it_is_not_the_switchs() {
    let Ok(at) = placed();

    assert_eq!(
      Switch::Off.pressed(Press::Tap(Point { x: 20, y: 20 }), at),
      Ok(Answer::Passes)
    );
  }

  #[test]
  fn up_and_down_belong_to_the_list_the_switch_is_on() {
    let Ok(at) = placed();

    for heading in [Heading::Up, Heading::Down, Heading::Next, Heading::Previous] {
      assert_eq!(
        Switch::On.pressed(Press::Toward(heading), at),
        Ok(Answer::Passes)
      );
    }
  }

  #[test]
  fn a_finger_dragging_the_knob_sets_it_by_which_half_it_is_over() {
    let Ok(at) = placed();

    assert_eq!(
      Switch::Off.pressed(Press::Drag(Point { x: 175, y: 300 }), at),
      Ok(Answer::Now(Switch::On))
    );
    assert_eq!(
      Switch::Off.pressed(Press::Drag(Point { x: 120, y: 20 }), at),
      Ok(Answer::Same)
    );
    assert_eq!(
      Switch::On.pressed(Press::Drag(Point { x: 0, y: 20 }), at),
      Ok(Answer::Now(Switch::Off))
    );
  }

  #[test]
  fn the_knob_stands_on_the_side_the_switch_is_set_to() -> Result<(), Box<dyn std::error::Error>>
  {
    let Ok(at) = placed();
    let Ok(on) = Switch::On.shapes(at, INKS);
    let Ok(off) = Switch::Off.shapes(at, INKS);

    let on_knob = panel_of(&on, 1)?;
    let off_knob = panel_of(&off, 1)?;

    assert!(on_knob.at.x > off_knob.at.x, "on is to the right of off");
    assert_eq!(
      on_knob.at.x.saturating_add(42),
      180,
      "the knob keeps its inset from the far end"
    );
    assert_eq!(off_knob.at.x, 106, "and from the near one");

    Ok(())
  }

  #[test]
  fn a_switch_that_is_on_is_filled_and_one_that_is_off_is_not()
  -> Result<(), Box<dyn std::error::Error>> {
    let Ok(at) = placed();
    let Ok(on) = Switch::On.shapes(at, INKS);
    let Ok(off) = Switch::Off.shapes(at, INKS);

    let on_track = panel_of(&on, 0)?;
    let off_track = panel_of(&off, 0)?;

    assert_eq!(on_track.fill, INKS.set);
    assert_eq!(off_track.fill, INKS.track);

    Ok(())
  }

  #[test]
  fn a_switch_is_wider_than_it_is_tall_by_the_same_share_at_every_size() {
    assert_eq!(
      Switch::sized(48),
      Ok(Size {
        width: 80,
        height: 48
      })
    );
    assert_eq!(
      Switch::sized(30),
      Ok(Size {
        width: 50,
        height: 30
      })
    );
  }
}
