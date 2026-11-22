//! One of a few, picked in place: what the power button does, how big things
//! are drawn, which way the screen stands.
//!
//! A choice of three used to be a row that opened a list of three, which is a
//! press in, a walk, a press to take it and a press back out, to say a thing a
//! thumb could have said with one push sideways. A console's settings never
//! asked that: the choice stands on the row with an arrow either side, and
//! left and right are the choices. Apple calls the same thing a picker and
//! draws it two ways -- segmented, every choice side by side, or one at a time
//! -- and Blender draws an enum expanded when it fits and as a menu when it
//! does not. Both of those are the same decision this makes.
//!
//! **Which form is arithmetic, not taste.** The frame is shared out evenly
//! between the choices, and if the widest of them fits its share with room
//! either side it is a segmented control and every choice is in view. If it
//! does not, it is a wheel: the one chosen, in the middle, with a mark either
//! side saying there is more that way. Nobody picks the form at a call site,
//! so the same picker comes out segmented on a card held wide and as a wheel
//! on a narrow one, and a translation that made the words longer changes how
//! it is drawn rather than spilling out of it.
//!
//! **Both forms are pressed the same way.** Left and right are the choice
//! before and after, and they stop at the ends rather than going round,
//! because a wheel that wraps is a list whose first and last are next to each
//! other and a person stepping through cannot tell which end they are at. The
//! mark at an end that has nothing past it is drawn quiet, which is the end
//! saying so. A tap on a segment is that choice and a drag across them is the
//! one under the finger; a tap on a wheel's mark is a step, and a tap on the
//! choice in the middle of it is the row's, because that is where A goes.
//!
//! What a choice says and how wide it is are both handed in, and the words are
//! drawn in the face and weight they were measured in, which is the rule the
//! panel learnt from a tab measured plain and drawn bold.

use std::num::NonZeroU32;

use console_core_fonts::{Font, Weight};
use console_core_geometry::{Point, Rectangle, Size};
use console_core_highlight::{Answer, Heading, Press};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_shapes::{Edge, Line, Panel, Shape, Text};

use crate::{Edges, Inks, Inside, edges, inside, pill, rounded, toward, whole};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choice<'a> {
  pub said: &'a str,
  pub wide: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lettering {
  pub font: Font,
  pub weight: Weight,
  pub tall: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Picker<'a> {
  pub choices: &'a [Choice<'a>],
  pub chosen: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
  Segmented,
  Wheel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zone {
  Before,
  Middle,
  After,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Points {
  Back,
  On,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Sixteenths(i32);

fn part(side: i32, sixteenths: Sixteenths) -> Result<i32, Never> {
  Ok(side.saturating_mul(sixteenths.0).saturating_div(16))
}

impl Picker<'_> {
  fn many(&self) -> Result<Option<NonZeroU32>, Never> {
    let Ok(many) = fitted::<_, u32>(self.choices.len());

    Ok(NonZeroU32::new(many))
  }

  fn standing(&self, many: NonZeroU32) -> Result<u32, Never> {
    Ok(self.chosen.min(many.get().saturating_sub(1)))
  }

  fn segment(&self, tall: u32) -> Result<Option<u32>, Never> {
    Ok(
      self
        .choices
        .iter()
        .map(|choice| choice.wide)
        .max()
        .map(|widest| widest.saturating_add(tall)),
    )
  }

  pub fn segmented_wide(&self, tall: u32) -> Result<u32, Never> {
    let Ok(segment_width) = self.segment(tall);
    let Ok(counted) = self.many();

    Ok(match (segment_width, counted) {
      (Some(segment), Some(many)) => segment.saturating_mul(many.get()),
      (None, _) | (_, None) => 0,
    })
  }

  pub fn form(&self, frame: Rectangle<i32>) -> Result<Form, Never> {
    let Ok(edges) = edges(frame);
    let Ok(counted) = self.many();
    let Ok(segment_width) = self.segment(edges.tall);

    let shared = match counted {
      Some(many) => edges.wide.checked_div(many.get()),
      None => None,
    };

    Ok(match (shared, segment_width) {
      (Some(share), Some(segment)) => match ::core::primitive::bool::then_some(segment <= share, ()) {
        ::core::option::Option::Some(()) => Form::Segmented,
        ::core::option::Option::None => Form::Wheel,
      },
      (None, _) | (_, None) => Form::Wheel,
    })
  }

  pub fn segments(&self, frame: Rectangle<i32>) -> Result<Vec<Rectangle<i32>>, Never> {
    let Ok(edges) = edges(frame);
    let Ok(counted) = self.many();

    let many = match counted {
      Some(many) => many,
      None => return Ok(Vec::new()),
    };

    let each = edges.wide.saturating_div(many.get());
    let Ok(each_across) = whole(each);
    let Ok(tall) = whole(edges.tall);
    let last = many.get().saturating_sub(1);

    Ok(
      (0..many.get())
        .map(|at| {
          let Ok(steps) = whole(at);
          let left = edges.left.saturating_add(steps.saturating_mul(each_across));

          let right = match ::core::primitive::bool::then_some(at == last, ()) {
            ::core::option::Option::Some(()) => edges.right,
            ::core::option::Option::None => left.saturating_add(each_across),
          };

          Rectangle {
            origin: Point {
              x: left,
              y: edges.top,
            },
            size: Size {
              width: right.saturating_sub(left),
              height: tall,
            },
          }
        })
        .collect(),
    )
  }

  fn under(&self, frame: Rectangle<i32>, across: i32) -> Result<Option<u32>, Never> {
    let Ok(edges) = edges(frame);
    let Ok(counted) = self.many();

    Ok(match counted {
      Some(many) => {
        let Ok(along) = fitted::<i32, u32>(across.saturating_sub(edges.left).max(0));
        let each = edges.wide.saturating_div(many.get()).max(1);
        let last = many.get().saturating_sub(1);

        Some(along.saturating_div(each).min(last))
      }
      None => None,
    })
  }

  fn stepped(&self, points: Points) -> Result<Answer<u32>, Never> {
    let Ok(counted) = self.many();

    let many = match counted {
      Some(many) => many,
      None => return Ok(Answer::Passes),
    };

    let Ok(standing) = self.standing(many);

    let went = match points {
      Points::Back => standing.saturating_sub(1),
      Points::On => standing.saturating_add(1).min(many.get().saturating_sub(1)),
    };

    toward(standing, went)
  }

  fn picked(&self, frame: Rectangle<i32>, across: i32) -> Result<Answer<u32>, Never> {
    let Ok(under_finger) = self.under(frame, across);
    let Ok(counted) = self.many();

    match (under_finger, counted) {
      (Some(found), Some(many)) => {
        let Ok(standing) = self.standing(many);

        toward(standing, found)
      }
      (None, _) | (_, None) => Ok(Answer::Passes),
    }
  }

  fn tapped(&self, frame: Rectangle<i32>, at: Point<i32>) -> Result<Answer<u32>, Never> {
    let Ok(landed) = inside(frame, at);
    let Ok(form) = self.form(frame);

    match (landed, form) {
      (Inside::No, _) => Ok(Answer::Passes),
      (Inside::Yes, Form::Segmented) => self.picked(frame, at.x),
      (Inside::Yes, Form::Wheel) => {
        let Ok(zone) = zone(frame, at.x);

        match zone {
          Zone::Before => self.stepped(Points::Back),
          Zone::After => self.stepped(Points::On),
          Zone::Middle => Ok(Answer::Passes),
        }
      }
    }
  }

  pub fn moved(&self, heading: Heading) -> Result<Answer<u32>, Never> {
    match heading {
      Heading::Left => self.stepped(Points::Back),
      Heading::Right => self.stepped(Points::On),
      Heading::Up | Heading::Down | Heading::Next | Heading::Previous => Ok(Answer::Passes),
    }
  }

  pub fn pressed(&self, press: Press, frame: Rectangle<i32>) -> Result<Answer<u32>, Never> {
    match press {
      Press::Toward(heading) => self.moved(heading),
      Press::Select | Press::Hover(_) => Ok(Answer::Passes),
      Press::Tap(at) => self.tapped(frame, at),
      Press::Drag(at) => {
        let Ok(form) = self.form(frame);

        match form {
          Form::Segmented => self.picked(frame, at.x),
          Form::Wheel => Ok(Answer::Passes),
        }
      }
    }
  }

  pub fn shapes(
    &self,
    frame: Rectangle<i32>,
    inks: Inks,
    lettering: &Lettering,
  ) -> Result<Vec<Shape>, Never> {
    let Ok(counted) = self.many();

    let many = match counted {
      Some(many) => many,
      None => return Ok(Vec::new()),
    };

    let Ok(standing) = self.standing(many);
    let Ok(form) = self.form(frame);
    let Ok(track) = pill(frame, inks.track);
    let mut shapes = vec![Shape::Panel(track)];
    let Ok(drawn) = match form {
      Form::Segmented => self.segmented(frame, (standing, inks), lettering),
      Form::Wheel => self.wheel(frame, (standing, inks), lettering),
    };

    shapes.extend(drawn);

    Ok(shapes)
  }

  fn segmented(
    &self,
    frame: Rectangle<i32>,
    (standing, inks): (u32, Inks),
    lettering: &Lettering,
  ) -> Result<Vec<Shape>, Never> {
    let Ok(segments) = self.segments(frame);
    let Ok(chosen) = index(standing);
    let mut shapes = Vec::new();

    match segments.get(chosen) {
      Some(segment) => {
        let Ok(lit) = inset(*segment, inks);

        shapes.push(Shape::Panel(lit));
      }
      None => {}
    }

    for (choice, segment) in self.choices.iter().zip(segments.iter()) {
      let Ok(words) = centred(*choice, *segment, (lettering, inks.said));

      shapes.push(Shape::Text(words));
    }

    Ok(shapes)
  }

  fn wheel(
    &self,
    frame: Rectangle<i32>,
    (standing, inks): (u32, Inks),
    lettering: &Lettering,
  ) -> Result<Vec<Shape>, Never> {
    let Ok(edges) = edges(frame);
    let Ok(counted) = self.many();
    let last = match counted {
      Some(many) => many.get().saturating_sub(1),
      None => 0,
    };

    let before = match ::core::primitive::bool::then_some(standing == 0, ()) {
      ::core::option::Option::Some(()) => inks.quiet,
      ::core::option::Option::None => inks.said,
    };
    let after = match ::core::primitive::bool::then_some(standing >= last, ()) {
      ::core::option::Option::Some(()) => inks.quiet,
      ::core::option::Option::None => inks.said,
    };

    let Ok(tall) = whole(edges.tall);
    let Ok(mut shapes) = chevron(
      Point {
        x: edges.left,
        y: edges.top,
      },
      (tall, Points::Back),
      before,
    );
    let Ok(on) = chevron(
      Point {
        x: edges.right.saturating_sub(tall),
        y: edges.top,
      },
      (tall, Points::On),
      after,
    );

    shapes.extend(on);

    let middle = Rectangle {
      origin: Point {
        x: edges.left.saturating_add(tall),
        y: edges.top,
      },
      size: Size {
        width: edges
          .right
          .saturating_sub(tall)
          .saturating_sub(edges.left.saturating_add(tall))
          .max(0),
        height: tall,
      },
    };
    let Ok(chosen) = index(standing);

    match self.choices.get(chosen) {
      Some(choice) => {
        let Ok(words) = centred(*choice, middle, (lettering, inks.said));

        shapes.push(Shape::Text(words));
      }
      None => {}
    }

    Ok(shapes)
  }
}

fn zone(frame: Rectangle<i32>, across: i32) -> Result<Zone, Never> {
  let Ok(edges) = edges(frame);
  let Ok(tall) = whole(edges.tall);

  Ok(
    match (
      across < edges.left.saturating_add(tall),
      across >= edges.right.saturating_sub(tall),
    ) {
      (true, _) => Zone::Before,
      (false, true) => Zone::After,
      (false, false) => Zone::Middle,
    },
  )
}

fn inset(segment: Rectangle<i32>, inks: Inks) -> Result<Panel, Never> {
  let Ok(Edges {
    left,
    top,
    wide,
    tall,
    ..
  }) = edges(segment);
  let margin = tall.saturating_div(10);
  let Ok(margin_across) = whole(margin);
  let height = tall.saturating_sub(margin.saturating_mul(2));
  let Ok(round) = rounded(height);

  Ok(Panel {
    at: Point {
      x: left.saturating_add(margin_across),
      y: top.saturating_add(margin_across),
    },
    size: Size {
      width: wide.saturating_sub(margin.saturating_mul(2)),
      height,
    },
    round,
    fill: inks.set,
    edge: Edge::None,
  })
}

fn centred(
  choice: Choice<'_>,
  within: Rectangle<i32>,
  (lettering, ink): (&Lettering, console_core_color::Oklch),
) -> Result<Text, Never> {
  let Ok(edges) = edges(within);
  let Ok(room) = whole(edges.wide);
  let Ok(said_wide) = whole(choice.wide);
  let Ok(tall) = whole(edges.tall);
  let Ok(said_tall) = whole(lettering.tall);

  Ok(Text {
    at: Point {
      x: edges
        .left
        .saturating_add(room.saturating_sub(said_wide).max(0).saturating_div(2)),
      y: edges
        .top
        .saturating_add(tall.saturating_sub(said_tall).max(0).saturating_div(2)),
    },
    width: choice.wide.saturating_add(2).min(edges.wide),
    said: choice.said.to_string(),
    weight: lettering.weight,
    font: lettering.font.clone(),
    ink,
  })
}

fn chevron(
  corner: Point<i32>,
  (side, points): (i32, Points),
  color: console_core_color::Oklch,
) -> Result<Vec<Shape>, Never> {
  let (arms, point) = match points {
    Points::Back => (Sixteenths(10), Sixteenths(6)),
    Points::On => (Sixteenths(6), Sixteenths(10)),
  };

  let Ok(arms_across) = part(side, arms);
  let Ok(tip_across) = part(side, point);
  let Ok(upper) = part(side, Sixteenths(5));
  let Ok(middle) = part(side, Sixteenths(8));
  let Ok(lower) = part(side, Sixteenths(11));
  let Ok(thick) = fitted::<i32, u32>(side.saturating_div(12).max(2));
  let tip = Point {
    x: corner.x.saturating_add(tip_across),
    y: corner.y.saturating_add(middle),
  };
  let top = Point {
    x: corner.x.saturating_add(arms_across),
    y: corner.y.saturating_add(upper),
  };
  let bottom = Point {
    x: corner.x.saturating_add(arms_across),
    y: corner.y.saturating_add(lower),
  };

  Ok(vec![
    Shape::Line(Line {
      from: top,
      to: tip,
      width: thick,
      color,
    }),
    Shape::Line(Line {
      from: tip,
      to: bottom,
      width: thick,
      color,
    }),
  ])
}

#[cfg(test)]
mod tests {
  use super::*;

  use std::error::Error;

  use console_core_fonts::LETTERS;

  use crate::testing::{INKS, frame, panel_of};

  const SHORT: [Choice<'static>; 3] = [
    Choice {
      said: "Small",
      wide: 40,
    },
    Choice {
      said: "Usual",
      wide: 42,
    },
    Choice {
      said: "Large",
      wide: 40,
    },
  ];

  const LONG: [Choice<'static>; 3] = [
    Choice {
      said: "Put the screen to sleep",
      wide: 170,
    },
    Choice {
      said: "Shut the machine down",
      wide: 160,
    },
    Choice {
      said: "Ask every time",
      wide: 110,
    },
  ];

  fn placed() -> Result<Rectangle<i32>, Never> {
    frame(
      Point { x: 0, y: 0 },
      Size {
        width: 300,
        height: 40,
      },
    )
  }

  fn lettering() -> Result<Lettering, Never> {
    Ok(Lettering {
      font: Font {
        family: LETTERS.to_string(),
        height: 16,
      },
      weight: Weight::Plain,
      tall: 20,
    })
  }

  fn texts(shapes: &[Shape]) -> Result<Vec<String>, Never> {
    Ok(
      shapes
        .iter()
        .filter_map(|shape| match shape {
          Shape::Text(text) => Some(text.said.clone()),
          Shape::Panel(_)
          | Shape::Picture(_)
          | Shape::Cropped(_)
          | Shape::Line(_)
          | Shape::Clip(_) => None,
        })
        .collect(),
    )
  }

  fn lines(shapes: &[Shape]) -> Result<Vec<Line>, Never> {
    Ok(
      shapes
        .iter()
        .filter_map(|shape| match shape {
          Shape::Line(line) => Some(*line),
          Shape::Panel(_)
          | Shape::Picture(_)
          | Shape::Cropped(_)
          | Shape::Text(_)
          | Shape::Clip(_) => None,
        })
        .collect(),
    )
  }

  #[test]
  fn the_width_it_asks_for_to_stand_side_by_side_is_the_width_it_does()
  -> Result<(), Box<dyn std::error::Error>> {
    let choices = [
      Choice {
        said: "Low",
        wide: 30,
      },
      Choice {
        said: "Balanced",
        wide: 70,
      },
      Choice {
        said: "High",
        wide: 36,
      },
    ];
    let picker = Picker {
      choices: &choices,
      chosen: 0,
    };
    let Ok(wanted) = picker.segmented_wide(20);
    let Ok(across) = fitted::<u32, i32>(wanted);
    let Ok(roomy) = frame(
      Point { x: 0, y: 0 },
      Size {
        width: across,
        height: 20,
      },
    );
    let Ok(short) = frame(
      Point { x: 0, y: 0 },
      Size {
        width: across.saturating_sub(1),
        height: 20,
      },
    );

    assert_eq!(wanted, 270);
    assert_eq!(picker.form(roomy), Ok(Form::Segmented));
    assert_eq!(picker.form(short), Ok(Form::Wheel));

    Ok(())
  }

  #[test]
  fn short_choices_that_fit_their_share_stand_side_by_side() {
    let Ok(frame) = placed();

    assert_eq!(
      Picker {
        choices: &SHORT,
        chosen: 1
      }
      .form(frame),
      Ok(Form::Segmented)
    );
  }

  #[test]
  fn choices_too_long_for_their_share_are_shown_one_at_a_time() {
    let Ok(frame) = placed();

    assert_eq!(
      Picker {
        choices: &LONG,
        chosen: 1
      }
      .form(frame),
      Ok(Form::Wheel)
    );
  }

  #[test]
  fn left_and_right_are_the_choices_either_side_in_both_forms_and_stop_at_the_ends() {
    let Ok(frame) = placed();

    for choices in [&SHORT, &LONG] {
      let middle = Picker { choices, chosen: 1 };
      let first = Picker { choices, chosen: 0 };
      let last = Picker { choices, chosen: 2 };

      assert_eq!(
        middle.pressed(Press::Toward(Heading::Right), frame),
        Ok(Answer::Now(2))
      );
      assert_eq!(
        middle.pressed(Press::Toward(Heading::Left), frame),
        Ok(Answer::Now(0))
      );
      assert_eq!(
        first.pressed(Press::Toward(Heading::Left), frame),
        Ok(Answer::Same)
      );
      assert_eq!(
        last.pressed(Press::Toward(Heading::Right), frame),
        Ok(Answer::Same)
      );
    }
  }

  #[test]
  fn a_tap_on_a_segment_is_that_choice() {
    let Ok(frame) = placed();
    let picker = Picker {
      choices: &SHORT,
      chosen: 0,
    };

    assert_eq!(
      picker.pressed(Press::Tap(Point { x: 250, y: 20 }), frame),
      Ok(Answer::Now(2))
    );
    assert_eq!(
      picker.pressed(Press::Tap(Point { x: 10, y: 20 }), frame),
      Ok(Answer::Same)
    );
  }

  #[test]
  fn a_finger_dragged_across_the_segments_picks_the_one_under_it_and_holds_past_the_ends() {
    let Ok(frame) = placed();
    let picker = Picker {
      choices: &SHORT,
      chosen: 0,
    };

    assert_eq!(
      picker.pressed(Press::Drag(Point { x: 150, y: 90 }), frame),
      Ok(Answer::Now(1))
    );
    assert_eq!(
      picker.pressed(Press::Drag(Point { x: 4000, y: 20 }), frame),
      Ok(Answer::Now(2))
    );
    assert_eq!(
      picker.pressed(Press::Drag(Point { x: -40, y: 20 }), frame),
      Ok(Answer::Same)
    );
  }

  #[test]
  fn a_tap_on_either_mark_of_a_wheel_steps_and_a_tap_on_its_middle_is_the_rows() {
    let Ok(frame) = placed();
    let picker = Picker {
      choices: &LONG,
      chosen: 1,
    };

    assert_eq!(
      picker.pressed(Press::Tap(Point { x: 10, y: 20 }), frame),
      Ok(Answer::Now(0))
    );
    assert_eq!(
      picker.pressed(Press::Tap(Point { x: 290, y: 20 }), frame),
      Ok(Answer::Now(2))
    );
    assert_eq!(
      picker.pressed(Press::Tap(Point { x: 150, y: 20 }), frame),
      Ok(Answer::Passes)
    );
    assert_eq!(
      picker.pressed(Press::Drag(Point { x: 290, y: 20 }), frame),
      Ok(Answer::Passes)
    );
  }

  #[test]
  fn up_down_and_a_belong_to_the_row_the_picker_is_on() {
    let Ok(frame) = placed();
    let picker = Picker {
      choices: &SHORT,
      chosen: 1,
    };

    assert_eq!(
      picker.pressed(Press::Toward(Heading::Up), frame),
      Ok(Answer::Passes)
    );
    assert_eq!(
      picker.pressed(Press::Toward(Heading::Next), frame),
      Ok(Answer::Passes)
    );
    assert_eq!(picker.pressed(Press::Select, frame), Ok(Answer::Passes));
  }

  #[test]
  fn the_segments_share_out_the_whole_frame_and_the_last_takes_what_is_left() {
    let Ok(frame) = frame(
      Point { x: 10, y: 0 },
      Size {
        width: 301,
        height: 40,
      },
    );
    let Ok(segments) = (Picker {
      choices: &SHORT,
      chosen: 0,
    })
    .segments(frame);
    let widths: Vec<i32> = segments.iter().map(|segment| segment.size.width).collect();

    assert_eq!(widths, vec![100, 100, 101]);
    assert_eq!(segments.first().map(|segment| segment.origin.x), Some(10));
  }

  #[test]
  fn a_segmented_picker_fills_the_chosen_segment_and_writes_every_choice()
  -> Result<(), Box<dyn Error>> {
    let Ok(frame) = placed();
    let Ok(lettering) = lettering();
    let Ok(shapes) = (Picker {
      choices: &SHORT,
      chosen: 2,
    })
    .shapes(frame, INKS, &lettering);
    let lit = panel_of(&shapes, 1)?;
    let Ok(said) = texts(&shapes);

    assert_eq!(lit.fill, INKS.set);
    assert!(lit.at.x >= 200, "the third segment is the one filled");
    assert_eq!(said, vec!["Small", "Usual", "Large"]);

    Ok(())
  }

  #[test]
  fn a_wheel_writes_only_the_chosen_one_and_quietens_the_mark_with_nothing_past_it() {
    let Ok(frame) = placed();
    let Ok(lettering) = lettering();
    let Ok(first) = (Picker {
      choices: &LONG,
      chosen: 0,
    })
    .shapes(frame, INKS, &lettering);
    let Ok(said) = texts(&first);
    let Ok(marks) = lines(&first);
    let colors: Vec<_> = marks.iter().map(|line| line.color).collect();

    assert_eq!(said, vec!["Put the screen to sleep"]);
    assert_eq!(colors, vec![INKS.quiet, INKS.quiet, INKS.said, INKS.said]);
  }

  #[test]
  fn a_picker_of_nothing_draws_nothing_and_answers_nothing() {
    let Ok(frame) = placed();
    let Ok(lettering) = lettering();
    let nothing = Picker {
      choices: &[],
      chosen: 0,
    };

    assert_eq!(nothing.shapes(frame, INKS, &lettering), Ok(Vec::new()));
    assert_eq!(
      nothing.pressed(Press::Toward(Heading::Right), frame),
      Ok(Answer::Passes)
    );
    assert_eq!(
      nothing.pressed(Press::Tap(Point { x: 150, y: 20 }), frame),
      Ok(Answer::Passes)
    );
  }
}
