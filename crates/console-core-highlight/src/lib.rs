//! Where the highlight goes when somebody presses something, whatever they
//! pressed it with.
//!
//! Every surface here that is more than a list worked this out for itself.
//! The library walks its covers with a `Direction` of its own, the greeter with
//! another, the pattern lock with a third, and the home screen and the overview
//! each spell the arrow keys out again -- and each of them is a grid, which is
//! the one arrangement where "the next one" stops being a number. Down from the
//! end of a short last row has to land somewhere, and five surfaces had five
//! opinions about where.
//!
//! **The answer is the one a television already gives.** tvOS has no pointer
//! and no list order to fall back on: its focus engine looks at the frames on
//! the screen, keeps the ones that lie the way the press points, and takes the
//! nearest. A frame in line with the one you are on -- overlapping it across
//! the way you are not moving -- beats any frame out of line, however close,
//! because a thumb pressing right along a row means the row. Out of line, the
//! distance is weighted thirteen to one in favour of the way the press points,
//! which is the number Android's `FocusFinder` settled on for the same question
//! and the only one anyone has argued for in public. Equal frames go to the one
//! earlier in the list, so the answer is the same every time it is asked.
//!
//! **Every hand says one of five things.** The d-pad, the arrow keys and the
//! stick point somewhere. A, Return and Space select what is highlighted. A
//! finger or a click taps a place, a finger or a held button drags across one,
//! and a pointer with nothing held hovers over one. That is all a surface here
//! is ever told, so a grid answers a thumb, a finger, a mouse and a keyboard
//! with one function rather than four, and none of them can be forgotten.
//! Tab and Shift-Tab are `Next` and `Previous`, which walk the frames in the
//! order they were handed over -- reading order, the one thing a keyboard user
//! expects of Tab and the one thing a direction cannot say.
//!
//! [`Activation`] represents a recognized gesture. Its target can be a point
//! or a value saved when the pointer went down. [`Actions`] pairs each drawn
//! frame with its action and resolves navigation, selection, taps, and touches.
//!
//! **A press nothing here can answer is handed back.** Right off the end of a
//! row is not a mistake to swallow: it is the press the thing holding the row
//! wanted, a list stepping on to its next row or a shelf giving way to the page
//! under it. So the answer says which of three it was -- the highlight moved, the
//! press landed on what was already highlighted, or it passes to whoever holds
//! this -- and the third is never answered by staying put, which is how a
//! surface ends up with a corner the d-pad cannot leave.
//!
//! Nothing here keeps where the highlight is. It is handed in, with the frames,
//! and the answer is where it stands now; a surface that wants to remember it
//! remembers it in its own state, which is where the `programs.md` note in
//! Orbit says a thing like that lives.

use console_core_geometry::{Point, Rectangle};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use core::cmp::Ordering;

#[cfg(kani)]
#[path = "../proofs/actions.rs"]
mod proofs;

const IN_LINE_WEIGHT: u128 = 13;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heading {
  Up,
  Down,
  Left,
  Right,
  Next,
  Previous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
  Toward(Heading),
  Select,
  Tap(Point<i32>),
  Drag(Point<i32>),
  Hover(Point<i32>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activation<P> {
  Focused,
  At(P),
}

impl<P> Activation<P> {
  pub fn resolve<A>(
    self,
    focused: impl FnOnce() -> Option<A>,
    target_at: impl FnOnce(P) -> Option<A>,
  ) -> Result<Option<A>, Never> {
    Ok(match self {
      Self::Focused => focused(),
      Self::At(point) => target_at(point),
    })
  }
}

impl Press {
  pub fn activation(self) -> Result<Option<Activation<Point<i32>>>, Never> {
    Ok(match self {
      Self::Select => Some(Activation::Focused),
      Self::Tap(point) => Some(Activation::At(point)),
      Self::Toward(_) | Self::Drag(_) | Self::Hover(_) => None,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer<T> {
  Now(T),
  Same,
  Passes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed<'a> {
  pub highlighted: u32,
  pub frames: &'a [Rectangle<i32>],
}

pub struct Actions<'a, A> {
  frames: &'a [Rectangle<i32>],
  actions: &'a [A],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionCountMismatch;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution<A> {
  Focus(u32),
  Activate { target: u32, action: A },
  Pass,
}

impl<'a, A: Copy> Actions<'a, A> {
  pub fn new(frames: &'a [Rectangle<i32>], actions: &'a [A]) -> Result<Self, ActionCountMismatch> {
    match frames.len().cmp(&actions.len()) {
      Ordering::Equal => Ok(Self { frames, actions }),
      Ordering::Less | Ordering::Greater => Err(ActionCountMismatch),
    }
  }

  pub fn resolve(&self, press: Press, focused: Option<u32>) -> Result<Resolution<A>, Never> {
    let hit = |point| {
      let Ok(found) = under(point, self.frames);

      found
    };

    let Ok(possible_activation) = press.activation();

    match possible_activation {
      Some(activation) => {
        let Ok(target) = activation.resolve(|| focused, hit);

        return Ok(
          match target.and_then(|target_2| {
            let Ok(position) = index(target_2);

            self
              .actions
              .get(position)
              .map(|action| Resolution::Activate {
                target: target_2,
                action: *action,
              })
          }) {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => Resolution::Pass,
          },
        );
      }
      ::core::option::Option::None => {}
    }

    let target = match press {
      Press::Toward(heading) => focused.and_then(|highlighted| {
        let placed = Placed {
          highlighted,
          frames: self.frames,
        };
        let Ok(next) = moved(placed, heading);

        next
      }),
      Press::Hover(at) => hit(at),
      Press::Select | Press::Tap(_) | Press::Drag(_) => None,
    };

    Ok(target.map_or(Resolution::Pass, Resolution::Focus))
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Compass {
  Up,
  Down,
  Left,
  Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Edges {
  left: i64,
  right: i64,
  top: i64,
  bottom: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Along {
  lead: i64,
  trail: i64,
  low: i64,
  high: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Line {
  In,
  Out,
}

impl From<bool> for Line {
  fn from(overlaps_across: bool) -> Self {
    match bool::then_some(overlaps_across, ()) {
      Some(()) => Self::In,
      None => Self::Out,
    }
  }
}

enum SearchSide {
  InDirection,
  OutOfDirection,
}

impl From<bool> for SearchSide {
  fn from(ahead: bool) -> Self {
    match bool::then_some(ahead, ()) {
      Some(()) => Self::InDirection,
      None => Self::OutOfDirection,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Rank {
  line: Line,
  distance: u128,
  at: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Inside {
  Yes,
  No,
}

impl From<bool> for Inside {
  fn from(inside: bool) -> Self {
    match bool::then_some(inside, ()) {
      Some(()) => Self::Yes,
      None => Self::No,
    }
  }
}

impl Edges {
  fn of(frame: Rectangle<i32>) -> Result<Edges, Never> {
    let left = i64::from(frame.origin.x);
    let top = i64::from(frame.origin.y);
    let wide = i64::from(frame.size.width.max(0));
    let tall = i64::from(frame.size.height.max(0));

    Ok(Edges {
      left,
      right: left.saturating_add(wide),
      top,
      bottom: top.saturating_add(tall),
    })
  }
}

fn along(frame: Rectangle<i32>, compass: Compass) -> Result<Along, Never> {
  let Ok(edges) = Edges::of(frame);

  Ok(match compass {
    Compass::Right => Along {
      lead: edges.right,
      trail: edges.left,
      low: edges.top,
      high: edges.bottom,
    },
    Compass::Left => Along {
      lead: edges.left.saturating_neg(),
      trail: edges.right.saturating_neg(),
      low: edges.top,
      high: edges.bottom,
    },
    Compass::Down => Along {
      lead: edges.bottom,
      trail: edges.top,
      low: edges.left,
      high: edges.right,
    },
    Compass::Up => Along {
      lead: edges.top.saturating_neg(),
      trail: edges.bottom.saturating_neg(),
      low: edges.left,
      high: edges.right,
    },
  })
}

fn squared(many: u64) -> Result<u128, Never> {
  Ok(u128::from(many).saturating_mul(u128::from(many)))
}

fn rank(from: Along, to: Along, at: u32) -> Result<Rank, Never> {
  let gap = to.trail.saturating_sub(from.lead).max(0);
  let ahead = gap.unsigned_abs().saturating_mul(2);
  let across = to
    .low
    .saturating_add(to.high)
    .saturating_sub(from.low.saturating_add(from.high))
    .unsigned_abs();
  let Ok(ahead_squared) = squared(ahead);
  let Ok(across_squared) = squared(across);

  let line = Line::from(to.high > from.low && to.low < from.high);

  Ok(Rank {
    line,
    distance: ahead_squared
      .saturating_mul(IN_LINE_WEIGHT)
      .saturating_add(across_squared),
    at,
  })
}

fn ranked(from: Along, to: Along, at: u32) -> Result<Option<Rank>, Never> {
  let starts_further = from.trail < to.trail;
  let starts_past_the_lead = from.lead <= to.trail;
  let ends_further = from.lead < to.lead;
  let side = SearchSide::from((starts_further || starts_past_the_lead) && ends_further);

  Ok(match side {
    SearchSide::InDirection => {
      let Ok(ranked) = rank(from, to, at);

      Some(ranked)
    }
    SearchSide::OutOfDirection => None,
  })
}

fn candidate(
  start: Along,
  compass: Compass,
  (at, frame): (u32, Rectangle<i32>),
) -> Result<Option<Rank>, Never> {
  let Ok(to) = along(frame, compass);

  ranked(start, to, at)
}

fn nearest(
  placed: Placed<'_>,
  from: Rectangle<i32>,
  compass: Compass,
) -> Result<Option<u32>, Never> {
  let Ok(start) = along(from, compass);
  let Placed {
    highlighted,
    frames,
  } = placed;

  Ok(
    frames
      .iter()
      .enumerate()
      .filter_map(|(at, frame)| {
        let Ok(position) = fitted::<_, u32>(at);
        let Ok(ranked) = candidate(start, compass, (position, *frame));

        ranked
      })
      .filter(|ranked| ranked.at != highlighted)
      .min()
      .map(|best| best.at),
  )
}

fn in_order(placed: Placed<'_>, heading: Heading) -> Result<Option<u32>, Never> {
  let Ok(many) = fitted::<_, u32>(placed.frames.len());

  let stepped = match heading {
    Heading::Next => placed.highlighted.checked_add(1),
    Heading::Previous => placed.highlighted.checked_sub(1),
    Heading::Up | Heading::Down | Heading::Left | Heading::Right => None,
  };

  Ok(stepped.filter(|landed| *landed < many))
}

pub fn moved(placed: Placed<'_>, heading: Heading) -> Result<Option<u32>, Never> {
  let Ok(standing) = index(placed.highlighted);

  let from = match placed.frames.get(standing) {
    Some(frame) => *frame,
    None => return Ok(None),
  };

  match heading {
    Heading::Next | Heading::Previous => in_order(placed, heading),
    Heading::Up => nearest(placed, from, Compass::Up),
    Heading::Down => nearest(placed, from, Compass::Down),
    Heading::Left => nearest(placed, from, Compass::Left),
    Heading::Right => nearest(placed, from, Compass::Right),
  }
}

fn inside(frame: Rectangle<i32>, at: Point<i32>) -> Result<Inside, Never> {
  let Ok(edges) = Edges::of(frame);
  let across = i64::from(at.x);
  let down = i64::from(at.y);

  Ok(Inside::from(
    across >= edges.left && across < edges.right && down >= edges.top && down < edges.bottom,
  ))
}

pub fn under(at: Point<i32>, frames: &[Rectangle<i32>]) -> Result<Option<u32>, Never> {
  Ok(
    frames
      .iter()
      .rposition(|frame| {
        let Ok(landed) = inside(*frame, at);

        landed == Inside::Yes
      })
      .map(|found| {
        let Ok(position) = fitted::<_, u32>(found);

        position
      }),
  )
}

fn landed(found: Option<u32>, highlighted: u32) -> Result<Answer<u32>, Never> {
  Ok(match found {
    Some(at) => match at.cmp(&highlighted) {
      Ordering::Equal => Answer::Same,
      Ordering::Less | Ordering::Greater => Answer::Now(at),
    },
    None => Answer::Passes,
  })
}

pub fn pressed(press: Press, placed: Placed<'_>) -> Result<Answer<u32>, Never> {
  Ok(match press {
    Press::Toward(heading) => {
      let Ok(went) = moved(placed, heading);
      let Ok(answer) = landed(went, placed.highlighted);

      answer
    }
    Press::Tap(at) | Press::Hover(at) => {
      let Ok(found) = under(at, placed.frames);
      let Ok(answer) = landed(found, placed.highlighted);

      answer
    }
    Press::Select | Press::Drag(_) => Answer::Passes,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  use console_core_geometry::Size;

  const SIDE: i32 = 100;

  const GAP: i32 = 10;

  #[test]
  fn activation_uses_only_the_target_of_its_own_input() {
    let unexpected_lookup = core::cell::Cell::new(false);
    let focused = Activation::<u32>::Focused.resolve(
      || Some(2),
      |_| {
        unexpected_lookup.set(true);

        None
      },
    );
    let tapped = Activation::At(3).resolve(
      || {
        unexpected_lookup.set(true);

        None
      },
      Some,
    );
    let missed = Activation::At(3).resolve(|| Some(2), |_| None);

    assert_eq!(focused, Ok(Some(2)));
    assert_eq!(tapped, Ok(Some(3)));
    assert_eq!(missed, Ok(None));
    assert!(!unexpected_lookup.get());
  }

  #[derive(Debug, Clone, Copy)]
  struct Cell {
    column: i32,
    row: i32,
  }

  fn square(cell: Cell) -> Result<Rectangle<i32>, Never> {
    let across = cell.column.saturating_mul(SIDE.saturating_add(GAP));
    let down = cell.row.saturating_mul(SIDE.saturating_add(GAP));

    Ok(Rectangle {
      origin: Point { x: across, y: down },
      size: Size {
        width: SIDE,
        height: SIDE,
      },
    })
  }

  fn at(origin: Point<i32>) -> Result<Rectangle<i32>, Never> {
    Ok(Rectangle {
      origin,
      size: Size {
        width: SIDE,
        height: SIDE,
      },
    })
  }

  fn grid() -> Result<Vec<Rectangle<i32>>, Never> {
    Ok(
      [
        (0, 0),
        (1, 0),
        (2, 0),
        (0, 1),
        (1, 1),
        (2, 1),
        (0, 2),
        (1, 2),
      ]
      .into_iter()
      .map(|(column, row)| {
        let Ok(frame) = square(Cell { column, row });

        frame
      })
      .collect(),
    )
  }

  #[test]
  fn right_along_a_row_lands_on_the_next_square_in_it() {
    let Ok(frames) = grid();

    assert_eq!(
      moved(
        Placed {
          highlighted: 0,
          frames: &frames
        },
        Heading::Right
      ),
      Ok(Some(1))
    );
    assert_eq!(
      moved(
        Placed {
          highlighted: 4,
          frames: &frames
        },
        Heading::Left
      ),
      Ok(Some(3))
    );
  }

  #[test]
  fn down_a_column_lands_on_the_square_under() {
    let Ok(frames) = grid();

    assert_eq!(
      moved(
        Placed {
          highlighted: 1,
          frames: &frames
        },
        Heading::Down
      ),
      Ok(Some(4))
    );
    assert_eq!(
      moved(
        Placed {
          highlighted: 7,
          frames: &frames
        },
        Heading::Up
      ),
      Ok(Some(4))
    );
  }

  #[test]
  fn down_onto_a_short_last_row_lands_on_the_nearest_square_of_it() {
    let Ok(frames) = grid();

    assert_eq!(
      moved(
        Placed {
          highlighted: 5,
          frames: &frames
        },
        Heading::Down
      ),
      Ok(Some(7))
    );
  }

  #[test]
  fn a_press_toward_nothing_is_handed_to_whoever_holds_the_grid() {
    let Ok(frames) = grid();

    assert_eq!(
      pressed(
        Press::Toward(Heading::Right),
        Placed {
          highlighted: 2,
          frames: &frames
        }
      ),
      Ok(Answer::Passes)
    );
    assert_eq!(
      pressed(
        Press::Toward(Heading::Up),
        Placed {
          highlighted: 0,
          frames: &frames
        }
      ),
      Ok(Answer::Passes)
    );
  }

  #[test]
  fn a_square_in_line_beats_a_nearer_one_out_of_line() {
    let Ok(start) = at(Point { x: 0, y: 0 });
    let Ok(far_in_line) = at(Point { x: 400, y: 0 });
    let Ok(near_below) = at(Point { x: 150, y: 120 });
    let frames = [start, near_below, far_in_line];

    assert_eq!(
      moved(
        Placed {
          highlighted: 0,
          frames: &frames
        },
        Heading::Right
      ),
      Ok(Some(2))
    );
  }

  #[test]
  fn two_squares_as_near_as_each_other_go_to_the_one_handed_over_first() {
    let Ok(start) = at(Point { x: 0, y: 100 });
    let Ok(above) = at(Point { x: 200, y: 0 });
    let Ok(below) = at(Point { x: 200, y: 200 });
    let frames = [start, below, above];

    assert_eq!(
      moved(
        Placed {
          highlighted: 0,
          frames: &frames
        },
        Heading::Right
      ),
      Ok(Some(1))
    );
  }

  #[test]
  fn tab_walks_the_order_the_frames_were_handed_over_in_and_stops_at_the_ends() {
    let Ok(frames) = grid();

    assert_eq!(
      moved(
        Placed {
          highlighted: 3,
          frames: &frames
        },
        Heading::Next
      ),
      Ok(Some(4))
    );
    assert_eq!(
      moved(
        Placed {
          highlighted: 3,
          frames: &frames
        },
        Heading::Previous
      ),
      Ok(Some(2))
    );
    assert_eq!(
      moved(
        Placed {
          highlighted: 7,
          frames: &frames
        },
        Heading::Next
      ),
      Ok(None)
    );
    assert_eq!(
      moved(
        Placed {
          highlighted: 0,
          frames: &frames
        },
        Heading::Previous
      ),
      Ok(None)
    );
  }

  #[test]
  fn a_pointer_over_a_square_highlights_it_and_over_a_gap_says_nothing() {
    let Ok(frames) = grid();
    let on_four = Point {
      x: SIDE.saturating_add(GAP).saturating_add(5),
      y: SIDE.saturating_add(GAP).saturating_add(5),
    };
    let in_a_gap = Point {
      x: SIDE.saturating_add(2),
      y: 5,
    };

    assert_eq!(
      pressed(
        Press::Hover(on_four),
        Placed {
          highlighted: 0,
          frames: &frames
        }
      ),
      Ok(Answer::Now(4))
    );
    assert_eq!(
      pressed(
        Press::Hover(in_a_gap),
        Placed {
          highlighted: 0,
          frames: &frames
        }
      ),
      Ok(Answer::Passes)
    );
  }

  #[test]
  fn a_tap_on_what_is_already_highlighted_is_the_same_square_rather_than_a_move() {
    let Ok(frames) = grid();

    assert_eq!(
      pressed(
        Press::Tap(Point { x: 5, y: 5 }),
        Placed {
          highlighted: 0,
          frames: &frames
        }
      ),
      Ok(Answer::Same)
    );
  }

  #[test]
  fn where_frames_overlap_the_one_drawn_last_is_the_one_under_the_finger() {
    let Ok(under_it) = at(Point { x: 0, y: 0 });
    let Ok(over_it) = at(Point { x: 50, y: 50 });

    assert_eq!(
      under(Point { x: 60, y: 60 }, &[under_it, over_it]),
      Ok(Some(1))
    );
    assert_eq!(
      under(Point { x: 10, y: 10 }, &[under_it, over_it]),
      Ok(Some(0))
    );
  }

  #[test]
  fn the_far_edge_of_a_frame_is_already_outside_it() {
    let Ok(only) = square(Cell { column: 0, row: 0 });
    let frames = [only];

    assert_eq!(under(Point { x: SIDE, y: 0 }, &frames), Ok(None));
    assert_eq!(
      under(
        Point {
          x: SIDE.saturating_sub(1),
          y: SIDE.saturating_sub(1)
        },
        &frames
      ),
      Ok(Some(0))
    );
  }

  #[test]
  fn a_highlight_that_is_not_among_the_frames_moves_nowhere() {
    let Ok(frames) = grid();

    assert_eq!(
      moved(
        Placed {
          highlighted: 9,
          frames: &frames
        },
        Heading::Right
      ),
      Ok(None)
    );
    assert_eq!(
      moved(
        Placed {
          highlighted: 8,
          frames: &frames
        },
        Heading::Previous
      ),
      Ok(None)
    );
  }

  #[test]
  fn selecting_and_dragging_are_never_the_highlights_to_answer() {
    let Ok(frames) = grid();

    assert_eq!(
      pressed(
        Press::Select,
        Placed {
          highlighted: 0,
          frames: &frames
        }
      ),
      Ok(Answer::Passes)
    );
    assert_eq!(
      pressed(
        Press::Drag(Point { x: 5, y: 5 }),
        Placed {
          highlighted: 0,
          frames: &frames
        }
      ),
      Ok(Answer::Passes)
    );
  }
}
