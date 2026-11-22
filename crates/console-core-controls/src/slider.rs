//! A level a thumb steps and a finger drags: the volume, the brightness, how
//! warm the screen goes at night.
//!
//! A level here was a bar and two marks either end of it, and the marks were the
//! only part a finger could change it by -- a target the width of a thumbnail,
//! pressed once per step. Every slider a person has used is the other way round:
//! the bar is what is pressed, a tap lands the knob where the finger is, and a
//! drag takes it along. Both are here, beside the d-pad's step, and they are
//! one answer: a place along the track is turned into the nearest step, and a
//! press of left or right is one step from where it stands.
//!
//! **The value is a whole number of steps.** A level is the far it has come and
//! the most it can be, as `console_panel::page::Bar` already says it, so a
//! slider never holds a fraction and never rounds the same value two ways. A
//! place is turned into a step by the nearest one, halves away from the start,
//! and a step back into a place by the share of the track it is; both are
//! integer arithmetic, so the knob a tap put somewhere is the knob drawn there.
//!
//! **The knob never leaves the frame.** The track the knob's middle can stand
//! on is the frame less half a knob at each end, so a level at nothing and a
//! level at the most both draw a whole knob, and a finger pressing the very
//! edge of the frame lands on the end rather than past it.
//!
//! A drag is the slider's wherever the finger goes after it started on it:
//! past either end holds at that end, and up or down off the row is still the
//! level being moved, which is how a scrub has always behaved here.

use std::num::NonZeroU32;

use console_core_geometry::{Point, Rectangle, Size};
use console_core_highlight::{Answer, Heading, Press};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_shapes::{Edge, Panel, Shape};

use crate::{Inks, Inside, edges, inside, rounded, toward, whole};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slider {
  at: u32,
  most: NonZeroU32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Track {
  start: i32,
  span: u32,
  knob: u32,
  middle: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
  Less,
  More,
}

fn track(frame: Rectangle<i32>) -> Result<Track, Never> {
  let Ok(edges) = edges(frame);
  let knob = edges.tall.saturating_mul(2).saturating_div(3);
  let Ok(half_a_knob) = whole(knob.saturating_div(2));
  let Ok(half_tall) = whole(edges.tall.saturating_div(2));

  Ok(Track {
    start: edges.left.saturating_add(half_a_knob),
    span: edges.wide.saturating_sub(knob),
    knob,
    middle: edges.top.saturating_add(half_tall),
  })
}

impl Slider {
  pub fn new(at: u32, most: NonZeroU32) -> Result<Slider, Never> {
    Ok(Slider {
      at: at.min(most.get()),
      most,
    })
  }

  pub fn at(self) -> Result<u32, Never> {
    Ok(self.at)
  }

  pub fn most(self) -> Result<NonZeroU32, Never> {
    Ok(self.most)
  }

  fn stepped(self, step: Step) -> Result<Answer<Slider>, Never> {
    let went = match step {
      Step::Less => self.at.saturating_sub(1),
      Step::More => self.at.saturating_add(1).min(self.most.get()),
    };
    let Ok(now) = Slider::new(went, self.most);

    toward(self, now)
  }

  fn landed(self, frame: Rectangle<i32>, at: Point<i32>) -> Result<Answer<Slider>, Never> {
    let Ok(track) = track(frame);
    let Ok(span) = whole(track.span);
    let across = at.x.saturating_sub(track.start).clamp(0, span);
    let Ok(along) = fitted::<i32, u64>(across);
    let most = u64::from(self.most.get());
    let room = u64::from(track.span);
    let doubled = along
      .saturating_mul(most)
      .saturating_mul(2)
      .saturating_add(room);

    match doubled.checked_div(room.saturating_mul(2)) {
      Some(nearest) => {
        let Ok(step) = fitted::<u64, u32>(nearest);
        let Ok(now) = Slider::new(step, self.most);

        toward(self, now)
      }
      None => Ok(Answer::Same),
    }
  }

  pub fn moved(self, heading: Heading) -> Result<Answer<Slider>, Never> {
    match heading {
      Heading::Left => self.stepped(Step::Less),
      Heading::Right => self.stepped(Step::More),
      Heading::Up | Heading::Down | Heading::Next | Heading::Previous => Ok(Answer::Passes),
    }
  }

  pub fn pressed(self, press: Press, frame: Rectangle<i32>) -> Result<Answer<Slider>, Never> {
    match press {
      Press::Toward(heading) => self.moved(heading),
      Press::Select | Press::Hover(_) => Ok(Answer::Passes),
      Press::Tap(at) => {
        let Ok(landed) = inside(frame, at);

        match landed {
          Inside::Yes => self.landed(frame, at),
          Inside::No => Ok(Answer::Passes),
        }
      }
      Press::Drag(at) => self.landed(frame, at),
    }
  }

  fn knob_across(self, track: Track) -> Result<i32, Never> {
    let share = u64::from(self.at).saturating_mul(u64::from(track.span));
    let divided = share.checked_div(u64::from(self.most.get()));
    let Ok(into) = fitted::<u64, i32>(match divided {
      Some(quotient) => quotient,
      None => 0,
    });

    Ok(track.start.saturating_add(into))
  }

  pub fn shapes(self, frame: Rectangle<i32>, inks: Inks) -> Result<Vec<Shape>, Never> {
    let Ok(track) = track(frame);
    let Ok(edges) = edges(frame);
    let thick = edges.tall.saturating_div(6).max(2);
    let Ok(half_thick) = whole(thick.saturating_div(2));
    let Ok(round) = rounded(thick);
    let Ok(knob_across) = self.knob_across(track);
    let Ok(filled) = fitted::<i32, u32>(knob_across.saturating_sub(track.start).max(0));

    let rail = Panel {
      at: Point {
        x: track.start,
        y: track.middle.saturating_sub(half_thick),
      },
      size: Size {
        width: track.span,
        height: thick,
      },
      round,
      fill: inks.track,
      edge: Edge::None,
    };
    let mut shapes = vec![Shape::Panel(rail)];

    match ::core::primitive::bool::then_some(filled > 0, ()) {
      ::core::option::Option::Some(()) => shapes.push(Shape::Panel(Panel {
        size: Size {
          width: filled,
          height: thick,
        },
        fill: inks.set,
        ..rail
      })),
      ::core::option::Option::None => {}
    }

    let Ok(half_a_knob) = whole(track.knob.saturating_div(2));
    let Ok(knob_round) = rounded(track.knob);

    shapes.push(Shape::Panel(Panel {
      at: Point {
        x: knob_across.saturating_sub(half_a_knob),
        y: track.middle.saturating_sub(half_a_knob),
      },
      size: Size {
        width: track.knob,
        height: track.knob,
      },
      round: knob_round,
      fill: inks.knob,
      edge: Edge::None,
    }));

    Ok(shapes)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  use std::error::Error;

  use crate::testing::{INKS, frame, panel_of};

  const TEN: NonZeroU32 = match NonZeroU32::new(10) {
    Some(ten) => ten,
    None => NonZeroU32::MIN,
  };

  fn placed() -> Result<Rectangle<i32>, Never> {
    frame(
      Point { x: 0, y: 0 },
      Size {
        width: 230,
        height: 60,
      },
    )
  }

  fn at(step: u32) -> Result<Slider, Never> {
    Slider::new(step, TEN)
  }

  fn level(answer: Result<Answer<Slider>, Never>) -> Result<Option<u32>, Never> {
    Ok(match answer {
      Ok(Answer::Now(slider)) => Some(slider.at),
      Ok(Answer::Same | Answer::Passes) => None,
    })
  }

  #[test]
  fn right_and_left_are_one_step_each_and_the_ends_are_ends() {
    let Ok(frame) = placed();
    let Ok(three) = at(3);
    let Ok(top) = at(10);
    let Ok(bottom) = at(0);

    assert_eq!(
      level(three.pressed(Press::Toward(Heading::Right), frame)),
      Ok(Some(4))
    );
    assert_eq!(
      level(three.pressed(Press::Toward(Heading::Left), frame)),
      Ok(Some(2))
    );
    assert_eq!(
      top.pressed(Press::Toward(Heading::Right), frame),
      Ok(Answer::Same)
    );
    assert_eq!(
      bottom.pressed(Press::Toward(Heading::Left), frame),
      Ok(Answer::Same)
    );
  }

  #[test]
  fn a_tap_on_the_track_lands_the_knob_on_the_nearest_step_under_the_finger() {
    let Ok(frame) = placed();
    let Ok(nothing) = at(0);

    assert_eq!(
      level(nothing.pressed(Press::Tap(Point { x: 120, y: 30 }), frame)),
      Ok(Some(5))
    );
    assert_eq!(
      level(nothing.pressed(Press::Tap(Point { x: 229, y: 30 }), frame)),
      Ok(Some(10))
    );
    assert_eq!(
      level(nothing.pressed(Press::Tap(Point { x: 25, y: 30 }), frame)),
      Ok(None)
    );
  }

  #[test]
  fn a_drag_past_either_end_holds_at_that_end_however_far_it_goes() {
    let Ok(frame) = placed();
    let Ok(middle) = at(5);

    assert_eq!(
      level(middle.pressed(Press::Drag(Point { x: 9000, y: -400 }), frame)),
      Ok(Some(10))
    );
    assert_eq!(
      level(middle.pressed(Press::Drag(Point { x: -9000, y: 400 }), frame)),
      Ok(Some(0))
    );
  }

  #[test]
  fn what_is_not_a_place_on_it_or_a_step_along_it_passes() {
    let Ok(frame) = placed();
    let Ok(middle) = at(5);

    assert_eq!(
      middle.pressed(Press::Tap(Point { x: 120, y: 90 }), frame),
      Ok(Answer::Passes)
    );
    assert_eq!(middle.pressed(Press::Select, frame), Ok(Answer::Passes));
    assert_eq!(
      middle.pressed(Press::Toward(Heading::Down), frame),
      Ok(Answer::Passes)
    );
    assert_eq!(
      middle.pressed(Press::Hover(Point { x: 120, y: 30 }), frame),
      Ok(Answer::Passes)
    );
  }

  #[test]
  fn a_level_asked_for_past_its_most_is_its_most() {
    let Ok(past) = at(40);

    assert_eq!(past.at(), Ok(10));
  }

  #[test]
  fn the_knob_is_whole_inside_the_frame_at_nothing_and_at_the_most() -> Result<(), Box<dyn Error>>
  {
    let Ok(frame) = placed();
    let Ok(nothing) = at(0);
    let Ok(most) = at(10);
    let Ok(low) = nothing.shapes(frame, INKS);
    let Ok(high) = most.shapes(frame, INKS);
    let low_knob = panel_of(&low, 1)?;
    let high_knob = panel_of(&high, 2)?;

    assert_eq!(low_knob.at.x, 0);
    assert_eq!(high_knob.at.x.saturating_add(40), 230);

    Ok(())
  }

  #[test]
  fn nothing_is_filled_at_nothing_and_the_whole_rail_is_at_the_most() -> Result<(), Box<dyn Error>>
  {
    let Ok(frame) = placed();
    let Ok(nothing) = at(0);
    let Ok(most) = at(10);
    let Ok(low) = nothing.shapes(frame, INKS);
    let Ok(high) = most.shapes(frame, INKS);
    let rail = panel_of(&high, 0)?;
    let filled = panel_of(&high, 1)?;

    assert_eq!(low.len(), 2, "a rail and a knob, and no fill");
    assert_eq!(filled.size.width, rail.size.width);
    assert_eq!(filled.fill, INKS.set);

    Ok(())
  }
}
