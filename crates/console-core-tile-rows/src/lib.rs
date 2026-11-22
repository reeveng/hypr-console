//! A row of tiles the highlight walks along, the highlighted one drawn larger
//! and the row moved so it is always in view.
//!
//! It is the shape a console opens into. The games along the top of a
//! PlayStation's home screen are a row of tiles, and so was the cross bar
//! before it: one row, one tile standing out, its name under it, and left and
//! right the only presses there are. Apple's television and Steam call the same
//! row a shelf, which is a word this tree already spends on a crate of helpers,
//! so it is named for what is on the screen instead. What all of them agree on
//! is the thing this answers -- **the row moves and the
//! highlight does not**. Pressing right does not send the eye hunting along the
//! row for where the highlight went; the next tile comes to where the eye
//! already is, at the front of the row, until the row has nothing left to
//! bring and the highlight walks the last few itself.
//!
//! It comes without the slide. A console eases the row across, and the
//! `console-ui.md` note in Orbit says what a surface here does instead: it is
//! up, or it is not there yet. So a press moves the row the whole way at once,
//! and the only motion on the screen is the one a finger makes.
//!
//! **A finger moves the row and a thumb moves the highlight.** A drag carries
//! the row along under the finger by however far it went, which the caller
//! keeps -- `Laid::scrolled` is where the row stood when the finger came down,
//! and `Scroll::By` is where it stands now -- and a press of the d-pad brings
//! the row back to the highlight. A tap is A on the tile it lands on, a pointer
//! standing on a tile highlights it the way it does on the home screen, and a
//! tile the row has carried out of view is not under anything, whatever is
//! drawn past the edge.
//!
//! The highlighted tile grows by a quarter and the tiles after it stand aside
//! to make room, so nothing is drawn over its neighbour. The row never scrolls
//! past its own end: the last tile highlighted stands at the far end of the
//! view, with no gap after it, rather than at the front with nothing behind.
//!
//! **A badge is a count in the corner of a tile.** What is waiting, what is
//! downloading. It sits inside the corner rather than over it, so a clip that
//! keeps the row in its view keeps the badge too, and it is never narrower than
//! it is tall, so one digit is a disc and four are a pill.

use console_core_color::Oklch;
use console_core_geometry::{Point, Rectangle, Size};
use console_core_highlight::{Answer, Heading, Press, under};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_shapes::{Edge, Panel, Round};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileRow {
  pub many: u32,
  pub highlighted: u32,
  pub tile: Size<u32>,
  pub gap: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scroll {
  ToTheHighlight,
  By(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tile {
  pub at: u32,
  pub frame: Rectangle<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Laid {
  pub tiles: Vec<Tile>,
  pub scrolled: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Badge {
  pub plate: Panel,
  pub words: Point<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct View {
  left: i64,
  right: i64,
  top: i32,
  wide: u64,
}

fn view_of(view: Rectangle<i32>) -> Result<View, Never> {
  let left = i64::from(view.origin.x);
  let Ok(wide) = fitted::<i32, u64>(view.size.width.max(0));

  Ok(View {
    left,
    right: left.saturating_add(i64::from(view.size.width.max(0))),
    top: view.origin.y,
    wide,
  })
}

fn quarter_up(many: u32) -> Result<u32, Never> {
  Ok(many.saturating_add(many.saturating_div(4)))
}

enum Highlight {
  Selected,
  Other,
}

impl From<bool> for Highlight {
  fn from(selected: bool) -> Self {
    match bool::then_some(selected, ()) {
      Some(()) => Self::Selected,
      None => Self::Other,
    }
  }
}

enum Visibility {
  Visible,
  Outside,
}

impl From<bool> for Visibility {
  fn from(in_view: bool) -> Self {
    match bool::then_some(in_view, ()) {
      Some(()) => Self::Visible,
      None => Self::Outside,
    }
  }
}

impl TileRow {
  fn standing(self) -> Result<Option<u32>, Never> {
    Ok(
      self
        .many
        .checked_sub(1)
        .map(|last| self.highlighted.min(last)),
    )
  }

  fn lifted(self) -> Result<Size<u32>, Never> {
    let Ok(width) = quarter_up(self.tile.width);
    let Ok(height) = quarter_up(self.tile.height);

    Ok(Size { width, height })
  }

  fn sized(self, at: u32) -> Result<Size<u32>, Never> {
    let Ok(standing) = self.standing();

    match Highlight::from(standing == Some(at)) {
      Highlight::Selected => self.lifted(),
      Highlight::Other => Ok(self.tile),
    }
  }

  fn left_of(self, at: u32) -> Result<u64, Never> {
    let Ok(highlight) = self.standing();
    let Ok(lifted) = self.lifted();
    let step = u64::from(self.tile.width).saturating_add(u64::from(self.gap));
    let before = u64::from(at).saturating_mul(step);

    let aside = match highlight {
      Some(standing) => match at.cmp(&standing) {
        core::cmp::Ordering::Greater => u64::from(lifted.width.saturating_sub(self.tile.width)),
        core::cmp::Ordering::Less | core::cmp::Ordering::Equal => 0,
      },
      None => 0,
    };

    Ok(before.saturating_add(aside))
  }

  fn furthest(self, view: View) -> Result<u64, Never> {
    let Ok(end) = self.left_of(self.many);
    let whole = end.saturating_sub(u64::from(self.gap));

    Ok(whole.saturating_sub(view.wide))
  }

  fn scrolled(self, view: View, scroll: Scroll) -> Result<u64, Never> {
    let Ok(furthest) = self.furthest(view);
    let Ok(highlight) = self.standing();

    let wanted = match (scroll, highlight) {
      (Scroll::By(by), _) => u64::from(by),
      (Scroll::ToTheHighlight, Some(standing)) => {
        let Ok(left) = self.left_of(standing);

        left
      }
      (Scroll::ToTheHighlight, None) => 0,
    };

    Ok(wanted.min(furthest))
  }

  fn tile(self, at: u32, (view, scrolled): (View, u64)) -> Result<Option<Tile>, Never> {
    let Ok(left_in_row) = self.left_of(at);
    let Ok(size) = self.sized(at);
    let Ok(offset) = fitted::<u64, i64>(left_in_row.saturating_sub(scrolled));
    let Ok(behind) = fitted::<u64, i64>(scrolled.saturating_sub(left_in_row));
    let left = view.left.saturating_add(offset).saturating_sub(behind);
    let right = left.saturating_add(i64::from(size.width));

    Ok(match Visibility::from(right > view.left && left < view.right) {
      Visibility::Visible => {
        let Ok(across) = fitted::<i64, i32>(left);
        let Ok(width) = fitted::<u32, i32>(size.width);
        let Ok(height) = fitted::<u32, i32>(size.height);

        Some(Tile {
          at,
          frame: Rectangle {
            origin: Point {
              x: across,
              y: view.top,
            },
            size: Size { width, height },
          },
        })
      }
      Visibility::Outside => None,
    })
  }

  pub fn laid(self, view: Rectangle<i32>, scroll: Scroll) -> Result<Laid, Never> {
    let Ok(seen) = view_of(view);
    let Ok(scrolled) = self.scrolled(seen, scroll);
    let Ok(held) = fitted::<u64, u32>(scrolled);

    let tiles = (0..self.many)
      .filter_map(|at| {
        let Ok(tile) = self.tile(at, (seen, scrolled));

        tile
      })
      .collect();

    Ok(Laid {
      tiles,
      scrolled: held,
    })
  }

  fn stepped(self, heading: Heading) -> Result<Answer<u32>, Never> {
    let Ok(highlight) = self.standing();

    let went = match (highlight, heading) {
      (Some(standing), Heading::Left | Heading::Previous) => standing.checked_sub(1),
      (Some(standing), Heading::Right | Heading::Next) => {
        standing.checked_add(1).filter(|after| *after < self.many)
      }
      (Some(_), Heading::Up | Heading::Down) | (None, _) => None,
    };

    Ok(match went {
      Some(next) => Answer::Now(next),
      None => Answer::Passes,
    })
  }

  fn landed(
    self,
    at: Point<i32>,
    (view, scroll): (Rectangle<i32>, Scroll),
  ) -> Result<Answer<u32>, Never> {
    let Ok(in_view) = under(at, std::slice::from_ref(&view));
    let Ok(laid) = self.laid(view, scroll);
    let frames: Vec<Rectangle<i32>> = laid.tiles.iter().map(|tile| tile.frame).collect();
    let Ok(under_finger) = under(at, &frames);
    let Ok(standing) = self.standing();

    let tile = match (in_view, under_finger) {
      (Some(_), Some(found)) => {
        let Ok(position) = console_core_number_conversion::index(found);

        laid.tiles.get(position).map(|tile| tile.at)
      }
      (None, _) | (_, None) => None,
    };

    Ok(match tile {
      Some(landed_on) => match Highlight::from(Some(landed_on) == standing) {
        Highlight::Selected => Answer::Same,
        Highlight::Other => Answer::Now(landed_on),
      },
      None => Answer::Passes,
    })
  }

  pub fn pressed(
    self,
    press: Press,
    view: Rectangle<i32>,
    scroll: Scroll,
  ) -> Result<Answer<u32>, Never> {
    match press {
      Press::Toward(heading) => self.stepped(heading),
      Press::Tap(at) | Press::Hover(at) => self.landed(at, (view, scroll)),
      Press::Select | Press::Drag(_) => Ok(Answer::Passes),
    }
  }
}

pub fn badge(on: Rectangle<i32>, said: Size<u32>, fill: Oklch) -> Result<Badge, Never> {
  let pad = said.height.saturating_div(4);
  let tall = said.height.saturating_add(pad.saturating_mul(2));
  let wide = said.width.saturating_add(tall.saturating_div(2)).max(tall);
  let Ok(inset) = fitted::<u32, i32>(tall.saturating_div(4));
  let Ok(across) = fitted::<u32, i32>(wide);
  let Ok(pad_down) = fitted::<u32, i32>(pad);
  let Ok(said_across) = fitted::<u32, i32>(said.width);
  let right = on.origin.x.saturating_add(on.size.width.max(0));
  let left = right.saturating_sub(inset).saturating_sub(across);
  let top = on.origin.y.saturating_add(inset);

  Ok(Badge {
    plate: Panel {
      at: Point { x: left, y: top },
      size: Size {
        width: wide,
        height: tall,
      },
      round: Round(tall.saturating_div(2)),
      fill,
      edge: Edge::None,
    },
    words: Point {
      x: left.saturating_add(across.saturating_sub(said_across).saturating_div(2)),
      y: top.saturating_add(pad_down),
    },
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  const VIEW: Rectangle<i32> = Rectangle {
    origin: Point { x: 50, y: 100 },
    size: Size {
      width: 600,
      height: 300,
    },
  };

  const INK: Oklch = Oklch {
    lightness: 0.8,
    chroma: 0.1,
    hue: console_core_color::Hue(58.0),
  };

  fn row_at(highlighted: u32) -> Result<TileRow, Never> {
    Ok(TileRow {
      many: 10,
      highlighted,
      tile: Size {
        width: 160,
        height: 160,
      },
      gap: 20,
    })
  }

  fn frame_of(laid: &Laid, at: u32) -> Result<Option<Rectangle<i32>>, Never> {
    Ok(
      laid
        .tiles
        .iter()
        .find(|tile| tile.at == at)
        .map(|tile| tile.frame),
    )
  }

  #[test]
  fn the_highlighted_tile_is_larger_and_the_tiles_after_it_stand_aside() {
    let Ok(row) = row_at(0);
    let Ok(laid) = row.laid(VIEW, Scroll::ToTheHighlight);

    assert_eq!(
      frame_of(&laid, 0).map(|frame| frame.map(|lit| lit.size)),
      Ok(Some(Size {
        width: 200,
        height: 200
      }))
    );
    assert_eq!(
      frame_of(&laid, 1).map(|frame| frame.map(|next| next.origin.x)),
      Ok(Some(270))
    );
  }

  #[test]
  fn the_row_brings_the_highlighted_tile_to_the_front_of_the_view() {
    let Ok(row) = row_at(3);
    let Ok(laid) = row.laid(VIEW, Scroll::ToTheHighlight);

    assert_eq!(
      frame_of(&laid, 3).map(|frame| frame.map(|lit| lit.origin.x)),
      Ok(Some(50))
    );
    assert_eq!(laid.scrolled, 540);
  }

  #[test]
  fn the_row_never_scrolls_past_its_last_tile() {
    let Ok(row) = row_at(9);
    let Ok(laid) = row.laid(VIEW, Scroll::ToTheHighlight);
    let Ok(last) = frame_of(&laid, 9);
    let far_end = last.map(|lit| lit.origin.x.saturating_add(lit.size.width));

    assert_eq!(far_end, Some(650), "the last tile ends where the view does");
  }

  #[test]
  fn a_drag_past_either_end_of_the_row_holds_at_that_end() {
    let Ok(row) = row_at(0);
    let Ok(far) = row.laid(VIEW, Scroll::By(90_000));
    let Ok(none) = row.laid(VIEW, Scroll::By(0));
    let Ok(furthest) = row.laid(VIEW, Scroll::ToTheHighlight);
    let Ok(last) = row_at(9);
    let Ok(at_the_end) = last.laid(VIEW, Scroll::ToTheHighlight);

    assert_eq!(far.scrolled, at_the_end.scrolled);
    assert_eq!(none.scrolled, 0);
    assert_eq!(furthest.scrolled, 0);
  }

  #[test]
  fn only_tiles_at_least_partly_in_view_are_laid() {
    let Ok(row) = row_at(0);
    let Ok(laid) = row.laid(VIEW, Scroll::ToTheHighlight);
    let at: Vec<u32> = laid.tiles.iter().map(|tile| tile.at).collect();

    assert_eq!(at, vec![0, 1, 2, 3]);
  }

  #[test]
  fn left_and_right_walk_the_row_and_its_ends_are_handed_back() {
    let Ok(first) = row_at(0);
    let Ok(last) = row_at(9);

    assert_eq!(
      first.pressed(Press::Toward(Heading::Right), VIEW, Scroll::ToTheHighlight),
      Ok(Answer::Now(1))
    );
    assert_eq!(
      first.pressed(Press::Toward(Heading::Left), VIEW, Scroll::ToTheHighlight),
      Ok(Answer::Passes)
    );
    assert_eq!(
      last.pressed(Press::Toward(Heading::Next), VIEW, Scroll::ToTheHighlight),
      Ok(Answer::Passes)
    );
    assert_eq!(
      last.pressed(
        Press::Toward(Heading::Previous),
        VIEW,
        Scroll::ToTheHighlight
      ),
      Ok(Answer::Now(8))
    );
  }

  #[test]
  fn up_and_down_leave_the_row_for_whatever_is_above_and_below_it() {
    let Ok(row) = row_at(4);

    assert_eq!(
      row.pressed(Press::Toward(Heading::Up), VIEW, Scroll::ToTheHighlight),
      Ok(Answer::Passes)
    );
    assert_eq!(
      row.pressed(Press::Toward(Heading::Down), VIEW, Scroll::ToTheHighlight),
      Ok(Answer::Passes)
    );
  }

  #[test]
  fn a_tap_or_a_pointer_on_a_tile_is_that_tile() {
    let Ok(row) = row_at(0);
    let on_the_second = Point { x: 300, y: 150 };
    let on_the_first = Point { x: 60, y: 110 };

    assert_eq!(
      row.pressed(Press::Tap(on_the_second), VIEW, Scroll::ToTheHighlight),
      Ok(Answer::Now(1))
    );
    assert_eq!(
      row.pressed(Press::Hover(on_the_second), VIEW, Scroll::ToTheHighlight),
      Ok(Answer::Now(1))
    );
    assert_eq!(
      row.pressed(Press::Tap(on_the_first), VIEW, Scroll::ToTheHighlight),
      Ok(Answer::Same)
    );
  }

  #[test]
  fn a_tile_carried_out_of_view_is_not_under_a_finger_past_the_edge() {
    let Ok(row) = row_at(0);
    let past_the_edge = Point { x: 660, y: 150 };

    assert_eq!(
      row.pressed(Press::Tap(past_the_edge), VIEW, Scroll::ToTheHighlight),
      Ok(Answer::Passes)
    );
  }

  #[test]
  fn a_row_of_nothing_lays_nothing_and_hands_every_press_back() {
    let empty = TileRow {
      many: 0,
      highlighted: 0,
      tile: Size {
        width: 160,
        height: 160,
      },
      gap: 20,
    };
    let Ok(laid) = empty.laid(VIEW, Scroll::ToTheHighlight);

    assert_eq!(laid.tiles, Vec::new());
    assert_eq!(
      empty.pressed(Press::Toward(Heading::Right), VIEW, Scroll::ToTheHighlight),
      Ok(Answer::Passes)
    );
  }

  #[test]
  fn a_badge_sits_inside_the_corner_of_its_tile() {
    let tile = Rectangle {
      origin: Point { x: 0, y: 0 },
      size: Size {
        width: 160,
        height: 160,
      },
    };
    let Ok(badge) = badge(
      tile,
      Size {
        width: 30,
        height: 16,
      },
      INK,
    );

    assert_eq!(badge.plate.at.y, 6);
    assert_eq!(badge.plate.at.x.saturating_add(42), 154);
    assert_eq!(badge.words, Point { x: 118, y: 10 });
  }

  #[test]
  fn a_badge_with_one_digit_is_a_disc_rather_than_a_sliver() {
    let tile = Rectangle {
      origin: Point { x: 0, y: 0 },
      size: Size {
        width: 160,
        height: 160,
      },
    };
    let Ok(one) = badge(
      tile,
      Size {
        width: 6,
        height: 16,
      },
      INK,
    );

    assert_eq!(one.plate.size.width, one.plate.size.height);
  }
}
